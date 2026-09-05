use std::process::Command;
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
fn main() {
    println!("cargo:rerun-if-env-changed=PHASECRAFT_BUILD_COMMIT");
    println!("cargo:rerun-if-changed=build.rs");
    for args in [
        vec!["rev-parse", "--git-path", "HEAD"],
        vec!["rev-parse", "--git-path", "packed-refs"],
    ] {
        if let Some(path) = git(&args) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(&["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={path}");
    }
    let commit = std::env::var("PHASECRAFT_BUILD_COMMIT")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".into());
    assert!(
        commit == "unknown"
            || (commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit())),
        "invalid build commit"
    );
    let platform = match (
        std::env::var("CARGO_CFG_TARGET_OS").unwrap().as_str(),
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str(),
    ) {
        ("windows", "x86_64") => "windows-x64",
        ("linux", "x86_64") => "linux-x64",
        ("macos", "aarch64") => "macos-arm64",
        ("macos", "x86_64") => "macos-x64",
        _ => "unsupported",
    };
    println!("cargo:rustc-env=PHASECRAFT_COMMIT={commit}");
    println!("cargo:rustc-env=PHASECRAFT_PLATFORM={platform}");
    println!(
        "cargo:rustc-env=PHASECRAFT_VERSION={} ({commit}; {platform})",
        std::env::var("CARGO_PKG_VERSION").unwrap()
    );
}
