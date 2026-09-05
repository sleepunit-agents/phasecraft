//! Explicit setup through Microsoft's installed MIDI tools, never on the dispatch thread.
pub const SEND: &str = "Phasecraft Send";
pub const RECEIVE: &str = "Phasecraft Receive";
pub const TOOLS_URL: &str = "https://microsoft.github.io/MIDI/";

pub fn setup() -> Result<String, String> {
    #[cfg(windows)]
    {
        windows::setup()
    }
    #[cfg(not(windows))]
    {
        Err("Windows MIDI setup is only needed on Windows. Use the virtual MIDI destination on this platform.".into())
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        fs::File,
        io::Read,
        os::windows::process::CommandExt,
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    fn console() -> Result<PathBuf, String> {
        ["ProgramW6432", "ProgramFiles"].into_iter()
            .filter_map(std::env::var_os)
            .map(|p| PathBuf::from(p).join("Windows MIDI Services/Tools/Console/midi.exe"))
            .find(|p| p.is_file())
            .ok_or_else(|| "Install Microsoft's Windows MIDI Services SDK Runtime and Tools (x64) using Get MIDI tools, then try again. Keep Windows 11 updated. No loopMIDI driver is needed for this route.".into())
    }
    fn existing() -> Result<Option<String>, String> {
        let ports = super::super::ports::list()?;
        let find = |wanted: &str| ports.iter().find(|p| p.as_str() == wanted);
        match (find(SEND), find(RECEIVE)) {
            (Some(send), Some(_)) => Ok(Some(send.clone())),
            (None, None) => Ok(None),
            _ => Err("Only one Phasecraft endpoint is visible. Check the pair in Windows MIDI Settings before retrying; existing ports were left unchanged.".into()),
        }
    }
    pub fn setup() -> Result<String, String> {
        if let Some(port) = existing()? {
            return Ok(port);
        }
        let log = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        let output = log.reopen().map_err(|e| e.to_string())?;
        let error_output = output.try_clone().map_err(|e| e.to_string())?;
        let mut child = Command::new(console()?)
            .args([
                "loopback",
                "create",
                "--name-a",
                SEND,
                "--name-b",
                RECEIVE,
                "--unique-identifier",
                "phasecraft",
                "--association-id",
                "dc906905-047b-40b1-b0b3-fd6ef4fbe634",
            ])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .stdin(Stdio::null())
            .stdout(output)
            .stderr(error_output)
            .spawn()
            .map_err(|e| format!("Could not launch Microsoft MIDI tools: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                result => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "Windows MIDI setup did not finish. Check Windows MIDI Settings and the installed SDK Runtime and Tools, then retry. {result:?}"
                    ));
                }
            }
        };
        if !status.success() {
            let mut details = String::new();
            let _ = File::open(log.path()).map(|file| file.take(4096).read_to_string(&mut details));
            return Err(format!(
                "Windows MIDI could not create the connection. Check that the MIDI service is enabled and Windows is fully updated. {details}"
            ));
        }
        // Endpoint publication and legacy WinMM enumeration are asynchronous.
        for _ in 0..100 {
            if let Ok(Some(port)) = existing() {
                return Ok(port);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err("Microsoft created the connection, but its MIDI 1.0 ports are not visible yet. Refresh outputs or inspect Windows MIDI Settings. Do not create duplicate pairs.".into())
    }
}
