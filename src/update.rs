//! Explicit, commit-addressed updates. Never called by the realtime transport.
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

pub const COMMIT: &str = env!("PHASECRAFT_COMMIT");
pub const PLATFORM: &str = env!("PHASECRAFT_PLATFORM");
const REPO: &str = "sleepunit-agents/phasecraft";
const MAX_BINARY: u64 = 100 * 1024 * 1024;
const AUTH_HELP: &str = "For this private repo, run `gh auth login` with an account that has repository access, or set GH_TOKEN (or GITHUB_TOKEN) to a token with Contents: read access.";

#[derive(Serialize, Deserialize)]
pub struct Version {
    pub version: String,
    pub commit: String,
    pub platform: String,
}
pub fn version() -> Version {
    Version {
        version: env!("CARGO_PKG_VERSION").into(),
        commit: COMMIT.into(),
        platform: PLATFORM.into(),
    }
}
#[derive(Deserialize)]
struct Asset {
    id: u64,
    name: String,
}
#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    commit: String,
    targets: BTreeMap<String, Target>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    asset: String,
    sha256: String,
    size: u64,
}
impl Manifest {
    fn target(&self, platform: &str) -> Result<&Target, String> {
        if self.schema != 1
            || self.commit.len() != 40
            || !self.commit.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("unsupported or malformed update manifest".into());
        }
        let target = self
            .targets
            .get(platform)
            .ok_or_else(|| format!("no update for platform {platform}"))?;
        let extension = if platform.starts_with("windows") {
            "exe"
        } else {
            "bin"
        };
        if target.asset != format!("phasecraft-update-{platform}.{extension}")
            || target.size == 0
            || target.size > MAX_BINARY
            || target.sha256.len() != 64
            || !target.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid update asset metadata".into());
        }
        Ok(target)
    }
}
fn token() -> Option<String> {
    for name in ["GH_TOKEN", "GITHUB_TOKEN"] {
        if let Ok(value) = std::env::var(name)
            && !value.trim().is_empty()
        {
            return Some(value.trim().into());
        }
    }
    let output = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|v| !v.is_empty())
}
struct Api {
    client: Client,
    token: Option<String>,
    base: String,
}
impl Api {
    fn get(&self, path: &str, binary: bool, limit: u64) -> Result<Vec<u8>, String> {
        let mut request = self.client.get(format!("{}{path}", self.base)).header(
            "Accept",
            if binary {
                "application/octet-stream"
            } else {
                "application/vnd.github+json"
            },
        );
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|e| format!("GitHub request failed: {}", e.without_url()))?;
        if !response.status().is_success() {
            return Err(format!(
                "GitHub returned HTTP {}. {AUTH_HELP} If a release is being published, retry shortly.",
                response.status().as_u16()
            ));
        }
        let mut bytes = vec![];
        response
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("download interrupted: {e}"))?;
        if bytes.len() as u64 > limit {
            return Err("download exceeds expected size".into());
        }
        Ok(bytes)
    }
    fn asset(&self, release: &Release, name: &str, limit: u64) -> Result<Vec<u8>, String> {
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| {
                format!("release asset {name} is unavailable; retry after publication finishes")
            })?;
        self.get(
            &format!("/repos/{REPO}/releases/assets/{}", asset.id),
            true,
            limit,
        )
    }
}
fn verify(bytes: &[u8], target: &Target) -> Result<(), String> {
    if bytes.len() as u64 != target.size
        || format!("{:x}", Sha256::digest(bytes)) != target.sha256.to_lowercase()
    {
        return Err("update checksum/size mismatch; executable unchanged. The rolling release may be changing; retry shortly".into());
    }
    Ok(())
}
/// Replace only the executable, using native replacement/cleanup on Windows and Unix.
pub fn install(candidate: &Path) -> Result<(), String> {
    self_replace::self_replace(candidate).map_err(|e| format!("could not replace executable: {e}. Close other Phasecraft processes and check directory permissions"))
}
pub fn run(check: bool, force: bool) -> Result<(), String> {
    if PLATFORM == "unsupported" {
        return Err("no published updater for this platform".into());
    }
    // reqwest strips Authorization on cross-host redirects; HTTPS-only prevents downgrade.
    let client = Client::builder()
        .user_agent(concat!("phasecraft/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string())?;
    let api = Api {
        client,
        token: token(),
        base: "https://api.github.com".into(),
    };
    let release: Release = serde_json::from_slice(&api.get(
        &format!("/repos/{REPO}/releases/tags/dev"),
        false,
        1024 * 1024,
    )?)
    .map_err(|e| format!("invalid release metadata: {e}"))?;
    let manifest: Manifest =
        serde_json::from_slice(&api.asset(&release, "update.json", 64 * 1024)?)
            .map_err(|e| format!("invalid update manifest: {e}"))?;
    let target = manifest.target(PLATFORM)?;
    println!(
        "Installed: {COMMIT}\nAvailable: {} ({PLATFORM}, dev)",
        manifest.commit
    );
    if manifest.commit == COMMIT && !force {
        println!("Already up to date.");
        return Ok(());
    }
    if check {
        println!("Update available. Run `phasecraft update` to install.");
        return Ok(());
    }
    println!("Downloading and verifying {}…", target.asset);
    let bytes = api.asset(&release, &target.asset, target.size)?;
    verify(&bytes, target)?;
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let candidate = temp.path().join(if cfg!(windows) {
        "phasecraft.exe"
    } else {
        "phasecraft"
    });
    let mut file = std::fs::File::create(&candidate).map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    let output = Command::new(&candidate)
        .args(["version", "--json"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("downloaded executable could not run: {e}"))?;
    let new: Version = serde_json::from_slice(&output.stdout)
        .map_err(|_| "downloaded executable returned invalid version metadata")?;
    if !output.status.success() || new.commit != manifest.commit || new.platform != PLATFORM {
        return Err(
            "downloaded executable does not match release commit/platform; executable unchanged"
                .into(),
        );
    }
    install(&candidate)?;
    println!(
        "Updated to {}. Your next command uses the new executable.",
        manifest.commit
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_or_mixed_release_is_rejected() {
        let mut target = Target {
            asset: "phasecraft-update-linux-x64.bin".into(),
            sha256: format!("{:x}", Sha256::digest(b"new binary")),
            size: 10,
        };
        assert!(verify(b"new binary", &target).is_ok());
        assert!(verify(b"old binary", &target).is_err());
        target.size = 9;
        assert!(verify(b"new binary", &target).is_err());
    }
    #[test]
    fn manifest_requires_known_schema_commit_and_platform_asset() {
        let mut manifest = Manifest {
            schema: 1,
            commit: "a".repeat(40),
            targets: BTreeMap::from([(
                "linux-x64".into(),
                Target {
                    asset: "phasecraft-update-linux-x64.bin".into(),
                    sha256: "b".repeat(64),
                    size: 10,
                },
            )]),
        };
        assert!(manifest.target("linux-x64").is_ok());
        assert!(manifest.target("windows-x64").is_err());
        manifest.schema = 2;
        assert!(manifest.target("linux-x64").is_err());
        manifest.schema = 1;
        manifest.commit = "dev".into();
        assert!(manifest.target("linux-x64").is_err());
    }
    #[test]
    fn http_errors_limits_and_redirect_credentials() {
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;
        fn serve(reply: String) -> (String, std::thread::JoinHandle<String>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = format!("http://{}", listener.local_addr().unwrap());
            let thread = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = String::new();
                let mut reader = BufReader::new(&stream);
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    request.push_str(&line);
                }
                stream.write_all(reply.as_bytes()).unwrap();
                request
            });
            (address, thread)
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        for (status, body, limit) in [("404 Not Found", "private", 100), ("200 OK", "too big", 2)] {
            let (base, thread) = serve(format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ));
            let api = Api {
                client: client.clone(),
                token: Some("test-secret".into()),
                base,
            };
            let error = api.get("/asset", true, limit).unwrap_err();
            assert!(!error.contains("test-secret"));
            thread.join().unwrap();
        }
        let (destination, second) =
            serve("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".into());
        let (base, first) = serve(format!(
            "HTTP/1.1 302 Found\r\nLocation: {destination}/download\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ));
        let api = Api {
            client,
            token: Some("test-secret".into()),
            base,
        };
        assert_eq!(api.get("/asset", true, 10).unwrap(), b"ok");
        assert!(
            first
                .join()
                .unwrap()
                .to_lowercase()
                .contains("authorization: bearer test-secret")
        );
        assert!(
            !second
                .join()
                .unwrap()
                .to_lowercase()
                .contains("authorization")
        );
    }
}
