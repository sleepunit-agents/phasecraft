use std::{fs, process::Command};

#[test]
fn replacement_child() {
    let Some(candidate) = std::env::var_os("PHASECRAFT_TEST_REPLACEMENT") else {
        return;
    };
    phasecraft::update::install(std::path::Path::new(&candidate)).unwrap();
}

#[test]
fn native_replacement_preserves_adjacent_project_files() {
    let temp = tempfile::tempdir().unwrap();
    let current = temp.path().join(if cfg!(windows) {
        "installed.exe"
    } else {
        "installed"
    });
    fs::copy(std::env::current_exe().unwrap(), &current).unwrap();
    let project = temp.path().join("composition.toml");
    fs::write(&project, "precious composition").unwrap();
    let child = Command::new(&current)
        .args(["--exact", "replacement_child", "--nocapture"])
        .env(
            "PHASECRAFT_TEST_REPLACEMENT",
            env!("CARGO_BIN_EXE_phasecraft"),
        )
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    let output = Command::new(&current)
        .args(["version", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let version: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(version["commit"], phasecraft::update::COMMIT);
    assert_eq!(version["platform"], phasecraft::update::PLATFORM);
    assert_eq!(fs::read_to_string(project).unwrap(), "precious composition");
}

#[test]
fn cli_exposes_commit_in_both_version_forms() {
    let output = Command::new(env!("CARGO_BIN_EXE_phasecraft"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(phasecraft::update::COMMIT));
}
