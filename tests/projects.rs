use phasecraft::{
    authoring::project,
    music::{Composition, resolve::resolve_step, rhythm::Expression},
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "phasecraft-project-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn create(&self) -> PathBuf {
        let p = self.0.join("my set");
        project::create(&p).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_phasecraft"))
}

#[test]
fn scaffold_is_portable_valid_and_preserves_proven_beats() {
    let tmp = Temp::new();
    let original = tmp.create();
    let p = tmp.0.join("moved set");
    fs::rename(original, &p).unwrap();
    let report = project::validate(&p);
    assert!(report.valid, "{:?}", report.errors);
    assert_eq!(report.files.len(), 7);
    assert_eq!(
        project::load(&p).unwrap().midi.port.as_deref(),
        Some("Phasecraft")
    );
    for genre in ["techno", "dnb", "garage", "accent-punch", "intro"] {
        let c = Composition::read(&p.join(format!("compositions/{genre}.toml"))).unwrap();
        let old = Composition::read(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("examples/quickstart/{genre}.toml")),
        )
        .unwrap();
        for step in 0..560 {
            assert_eq!(
                serde_json::to_string(&resolve_step(&c, step).0).unwrap(),
                serde_json::to_string(&resolve_step(&old, step).0).unwrap()
            );
        }
    }
    assert_eq!(
        Composition::read(&p.join("phasecraft.toml")).unwrap().tempo,
        132.0
    );
    let output = cli()
        .current_dir(&p)
        .args(["play", "--dry-run", "--bars", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn new_refuses_existing_paths_without_touching_contents() {
    let tmp = Temp::new();
    let p = tmp.create();
    fs::write(p.join("phasecraft.toml"), "precious").unwrap();
    assert!(project::create(&p).is_err());
    assert_eq!(
        fs::read_to_string(p.join("phasecraft.toml")).unwrap(),
        "precious"
    );
    assert!(project::create(&tmp.0).is_err());
}
#[test]
fn keyed_parts_and_shorthand_match_explicit_expressions() {
    let source = r#"
    tempo=132
    seed=91827
    [parts.hat]
    use="techno.closed_hat"
    [parts.hat.trigger]
    rhythm={op="xor", a={steps=16,pulses=7}, b={steps=5,pulses=2,rotation=-1}}
    "#;
    let c = Composition::parse(source).unwrap();
    let explicit = source
        .replace("[parts.hat]", "[[parts]]\nid=\"hat\"")
        .replace("parts.hat.trigger", "parts.trigger")
        .replace("{op=", "{type=\"binary\",op=")
        .replace("{steps=", "{type=\"euclidean\",steps=");
    let old = Composition::parse(&explicit).unwrap();
    for step in 0..560 {
        assert_eq!(resolve_step(&c, step).1, resolve_step(&old, step).1);
    }
    let replaced = Composition::parse(
        &(source.to_owned() + "\n[parts.hat.accent]\nrhythm={steps=7,pulses=3}\n"),
    )
    .unwrap();
    assert!(matches!(
        replaced.parts[0].accent.rhythm,
        Expression::Euclidean {
            steps: 7,
            pulses: 3,
            ..
        }
    ));
    let reference = Composition::parse(
        r#"
        tempo=132
        seed=1
        [parts.kick]
        use="techno.kick"
        [parts.hat]
        use="techno.closed_hat"
        [parts.hat.trigger]
        rhythm={part="kick",mode="hits"}
    "#,
    )
    .unwrap();
    assert!(matches!(
        reference
            .parts
            .iter()
            .find(|p| p.id == "hat")
            .unwrap()
            .trigger
            .rhythm,
        Expression::Part { .. }
    ));
}
#[test]
fn shorthand_kind_switch_discards_inherited_fields_and_partial_overrides_merge() {
    let source = r#"
        tempo=132
        seed=1
        [library.behaviors."my.binary"]
        use="techno.closed_hat"
        [library.behaviors."my.binary".trigger]
        rhythm={op="xor",a={steps=16,pulses=7},b={steps=5,pulses=2}}
        [parts.hat]
        use="my.binary"
        [parts.hat.trigger]
        rhythm={steps=11,pulses=4}
    "#;
    let c = Composition::parse(source).unwrap();
    assert!(matches!(
        c.parts[0].trigger.rhythm,
        Expression::Euclidean {
            steps: 11,
            pulses: 4,
            ..
        }
    ));
    let c =
        Composition::parse(&source.replace("rhythm={steps=11,pulses=4}", "rhythm={a={pulses=3}}"))
            .unwrap();
    assert!(
        matches!(c.parts[0].trigger.rhythm,Expression::Binary{ref a,..} if matches!(**a,Expression::Euclidean{steps:16,pulses:3,..}))
    );
}
#[test]
fn diagnostics_reject_ambiguous_or_misspelled_fields() {
    for body in [
        "id='different'",
        "triger={probability=0.2}",
        "trigger={rhythm={part='kick',id='other'}}",
    ] {
        let error = Composition::parse(&format!(
            "tempo=132\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\n{body}"
        ))
        .unwrap_err();
        assert!(error.contains("parts.hat"), "{error}");
    }
}
#[test]
fn validation_checks_all_compositions_and_produces_machine_readable_errors() {
    let tmp = Temp::new();
    let p = tmp.create();
    fs::write(p.join("compositions/dnb.toml"), "garbage").unwrap();
    let output = cli()
        .args(["validate", p.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["files"].as_array().unwrap().len(), 7);
    assert!(report["errors"][0].as_str().unwrap().contains("dnb.toml"));
    assert!(project::validate(&p.join("compositions/techno.toml")).valid);
}
#[test]
fn project_paths_and_midi_settings_are_validated() {
    let tmp = Temp::new();
    let p = tmp.create();
    for source in [
        "lookahead_ms=0",
        "port='x'\nvirtual_port=true",
        "port=''",
        "lookahed_ms=100",
    ] {
        fs::write(p.join("config/midi.toml"), source).unwrap();
        assert!(!project::validate(&p).valid);
    }
    let m = fs::read_to_string(p.join("phasecraft.toml"))
        .unwrap()
        .replace("config/midi.toml", "../midi.toml");
    fs::write(p.join("phasecraft.toml"), m).unwrap();
    assert!(project::validate(&p).errors[0].contains("relative"));
}
#[test]
fn shared_library_edits_are_resolved_for_every_composition() {
    let tmp = Temp::new();
    let p = tmp.create();
    let file = p.join("kits/909.toml");
    let source = fs::read_to_string(&file).unwrap();
    fs::write(file, source.replace("note = 36", "note = 35")).unwrap();
    for genre in ["techno", "dnb", "garage", "accent-punch", "intro"] {
        let c = Composition::read(&p.join(format!("compositions/{genre}.toml"))).unwrap();
        assert_eq!(
            c.parts.iter().find(|p| p.id == "kick").unwrap().output.note,
            35
        );
    }
}

#[test]
fn original_release_provenance_is_byte_identical_for_35_bars() {
    use sha2::{Digest, Sha256};
    let fixtures: std::collections::BTreeMap<String, String> =
        serde_json::from_str(include_str!("fixtures/pre-project-provenance.json")).unwrap();
    for (name, expected) in fixtures {
        let category = if name == "showcase" {
            "showcases"
        } else {
            "quickstart"
        };
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("examples/{category}/{name}.toml"));
        let output = cli()
            .arg("inspect")
            .arg(path)
            .args(["--steps", "560"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            format!("{:x}", Sha256::digest(&output.stdout)),
            expected,
            "{name}"
        );
    }
}

#[test]
fn watched_project_libraries_apply_at_boundaries_and_reject_invalid_edits() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let temp = Temp::new();
    let project = temp.create();
    fs::write(
        project.join("compositions/techno.toml"),
        "tempo=400\nseed=1\nphrase_bars=1\n[parts.kick]\nuse='my.techno_kick'\n",
    )
    .unwrap();
    let library = project.join("patterns/drums.toml");
    let original = fs::read_to_string(&library).unwrap();
    let mut child = cli()
        .arg("play")
        .arg(&project)
        .args([
            "--dry-run",
            "--watch",
            "--bars",
            "4",
            "--trace",
            "--lookahead-ms",
            "50",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut rows = vec![];
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let row: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        match row["step"].as_u64().unwrap() {
            0 => fs::write(
                &library,
                format!(
                    "{original}\n[library.behaviors.\"my.techno_kick\".trigger]\nprobability=0\n"
                ),
            )
            .unwrap(),
            16 => fs::write(&library, "invalid TOML !").unwrap(),
            32 => fs::write(&library, &original).unwrap(),
            _ => (),
        }
        rows.push(row);
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(rows.len(), 64);
    assert!(rows[0]["event"].is_object());
    assert!(rows[16]["event"].is_null());
    assert!(rows[32]["event"].is_null());
    assert!(rows[48]["event"].is_object());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Reload rejected"));
}
