use phasecraft::{
    config::Composition,
    engine::{BooleanOp, Expression, ReferenceMode, resolve_step},
};
use std::path::PathBuf;
fn example(name: &str) -> Composition {
    Composition::read(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(format!("{name}.toml")),
    )
    .unwrap()
}
fn trace(c: &Composition, step: u64, id: &str) -> String {
    serde_json::to_string(
        &resolve_step(c, step)
            .0
            .into_iter()
            .find(|t| t.part == id)
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn named_behaviors_exactly_reproduce_validated_genre_examples() {
    for genre in ["techno", "dnb"] {
        let old = example(genre);
        let new = example(&format!("{genre}-reuse"));
        for step in 0..560 {
            assert_eq!(resolve_step(&old, step).1, resolve_step(&new, step).1);
            assert_eq!(
                serde_json::to_string(&resolve_step(&old, step).0).unwrap(),
                serde_json::to_string(&resolve_step(&new, step).0).unwrap()
            );
        }
    }
}
#[test]
fn overrides_merge_fields_but_replace_different_expression_types() {
    let c = Composition::parse(
        r#"
        tempo=132
        seed=1
        [part]
        id="my_hat"
        use="techno.closed_hat"
        [part.trigger.rhythm]
        pulses=3
        [part.profile]
        use="accent.punch"
        boost=20
    "#,
    )
    .unwrap();
    assert_eq!(c.parts[0].output.note, 42);
    assert_eq!(c.parts[0].profile.base, 72);
    assert_eq!(c.parts[0].profile.boost, 20);
    assert!(matches!(
        c.parts[0].trigger.rhythm,
        Expression::Euclidean {
            steps: 16,
            pulses: 3,
            ..
        }
    ));
    let c = Composition::parse(
        r#"
        tempo=132
        seed=1
        [part]
        id="kick"
        use="dnb.kick"
        [part.trigger.rhythm]
        type="euclidean"
        steps=16
        pulses=4
    "#,
    )
    .unwrap();
    assert!(matches!(
        c.parts[0].trigger.rhythm,
        Expression::Euclidean { pulses: 4, .. }
    ));
}
#[test]
fn component_composition_is_ordered_and_instance_identity_is_required() {
    let source = r#"
        tempo=132
        seed=1
        [part]
        id="backbeat"
        compose=["std.backbeat","std.no_accent","kit.909.snare","kit.909.clap"]
    "#;
    let c = Composition::parse(source).unwrap();
    assert_eq!(c.parts[0].output.note, 39);
    assert_eq!(resolve_step(&c, 4).1.len(), 2);
    assert!(resolve_step(&c, 0).1.is_empty());
    assert!(Composition::parse(&source.replace("id=\"backbeat\"", "")).is_err());
}
#[test]
fn library_errors_are_not_silently_ignored() {
    let base = "tempo=132\nseed=1\n[part]\nid='hat'\nuse='techno.closed_hat'\n";
    for source in [
        base.replace("techno.closed_hat","missing.behavior"),
        format!("{base}compose=['techno.kick']\n"),
        format!("{base}[part.profile]\nuse='missing.profile'\n"),
        format!("{base}[part.trigger]\nprobabilty=0.5\n"),
        format!("{base}[library.behaviors.'techno.closed_hat']\nuse='techno.kick'\n"),
        format!("{base}[library.unknown]\nfoo={{}}\n"),
        base.replace("use='techno.closed_hat'", "use='my.a'\n[library.behaviors.'my.a']\nuse='my.b'\n[library.behaviors.'my.b']\nuse='my.a'"),
        format!("{base}[part.profile]\nuse='my.a'\n[library.profiles.'my.a']\nuse='my.a'"),
    ] {assert!(Composition::parse(&source).is_err(),"accepted {source}");}
}
#[test]
fn imported_personal_library_expands_and_round_trips() {
    let c = example("showcase");
    assert_eq!(c.parts.len(), 6);
    let rim = c.parts.iter().find(|p| p.id == "rim").unwrap();
    assert_eq!((rim.profile.base, rim.profile.boost), (48, 26));
    let expanded = toml::to_string_pretty(&c).unwrap();
    let replay = Composition::parse(&expanded).unwrap();
    for step in 0..128 {
        assert_eq!(resolve_step(&c, step).1, resolve_step(&replay, step).1);
    }
}
#[test]
fn references_distinguish_actual_admission_from_structure() {
    let mut hits = example("interlock-hits");
    let mut structure = example("interlock-structural");
    for c in [&mut hits, &mut structure] {
        c.parts
            .iter_mut()
            .find(|p| p.id == "kick")
            .unwrap()
            .trigger
            .probability = 0.0;
    }
    let h = resolve_step(&hits, 0).0;
    let s = resolve_step(&structure, 0).0;
    assert!(h.iter().find(|p| p.part == "rim").unwrap().event.is_some());
    assert!(s.iter().find(|p| p.part == "rim").unwrap().event.is_none());
    let c = example("interlock-hits");
    for step in 0..560 {
        let traces = resolve_step(&c, step).0;
        let fired = |id| {
            traces
                .iter()
                .find(|p| p.part == id)
                .unwrap()
                .event
                .is_some()
        };
        assert!(!(fired("kick") && fired("rim")));
    }
}
#[test]
fn dependencies_and_reference_probability_are_order_independent() {
    let c = example("showcase");
    let mut changed = c.clone();
    changed.parts.reverse();
    for step in 0..128 {
        assert_eq!(resolve_step(&c, step).1, resolve_step(&changed, step).1);
        assert_eq!(trace(&c, step, "rim"), trace(&changed, step, "rim"));
    }
    changed
        .parts
        .iter_mut()
        .find(|p| p.id == "rim")
        .unwrap()
        .accent
        .probability = 0.0;
    for step in 0..128 {
        assert_eq!(trace(&c, step, "kick"), trace(&changed, step, "kick"));
    }
}
#[test]
fn missing_references_and_cycles_fail_before_playback() {
    let mut c = example("interlock-hits");
    c.parts
        .iter_mut()
        .find(|p| p.id == "kick")
        .unwrap()
        .accent
        .rhythm = Expression::Part {
        id: "rim".into(),
        mode: ReferenceMode::Hits,
    };
    assert!(c.validate().unwrap_err().contains("cycle"));
    let mut c = example("interlock-hits");
    c.parts.retain(|p| p.id != "kick");
    assert!(c.validate().unwrap_err().contains("missing Part"));
    let mut c = example("techno-reuse");
    c.parts[0].trigger.rhythm = Expression::Part {
        id: c.parts[0].id.clone(),
        mode: ReferenceMode::Structural,
    };
    assert!(c.validate().is_err());
}
#[test]
fn example_pairs_isolate_the_intended_musical_change() {
    let locked = example("probability-locked");
    let flowing = example("probability-continuous");
    let accent = example("probability-accent-only");
    for step in 0..64 {
        let get = |c: &Composition, s| {
            resolve_step(c, s)
                .0
                .into_iter()
                .find(|t| t.part == "hat")
                .unwrap()
        };
        let a = get(&locked, step);
        let repeat = get(&locked, step + 64);
        assert_eq!(a.trigger.admitted, repeat.trigger.admitted);
        assert_eq!(a.accent.admitted, repeat.accent.admitted);
        assert_eq!(a.trigger.roll, get(&accent, step).trigger.roll);
        assert_eq!(a.trigger.admitted, get(&accent, step).trigger.admitted);
        assert_ne!(
            get(&flowing, step).trigger.roll,
            get(&flowing, step + 64).trigger.roll
        );
    }
    let reset = example("phase-reset");
    let advancing = example("phase-continue");
    let pattern = |c: &Composition, start: u64| {
        (start..start + 64)
            .map(|s| {
                resolve_step(c, s)
                    .0
                    .into_iter()
                    .find(|t| t.part == "hat")
                    .unwrap()
                    .trigger
                    .admitted
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(pattern(&reset, 0), pattern(&reset, 64));
    assert_ne!(pattern(&advancing, 0), pattern(&advancing, 64));
    let subtle = example("emphasis-subtle");
    let punch = example("emphasis-punch");
    for step in 0..128 {
        assert_eq!(trace(&subtle, step, "hat"), trace(&punch, step, "hat"));
    }
    assert_ne!(resolve_step(&subtle, 0).1, resolve_step(&punch, 0).1);
}
#[test]
fn gallery_exercises_every_boolean_operator() {
    fn ops(e: &Expression, set: &mut std::collections::HashSet<&'static str>) {
        if let Expression::Binary { op, a, b } = e {
            set.insert(match op {
                BooleanOp::And => "and",
                BooleanOp::Or => "or",
                BooleanOp::Xor => "xor",
                BooleanOp::ANotB => "a_not_b",
                BooleanOp::BNotA => "b_not_a",
            });
            ops(a, set);
            ops(b, set);
        }
    }
    let mut used = std::collections::HashSet::new();
    for p in example("algebra").parts {
        ops(&p.trigger.rhythm, &mut used);
    }
    assert_eq!(used.len(), 5);
}

struct TempTree(PathBuf);
impl TempTree {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("phasecraft-authoring-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn imports_are_relative_and_detect_cycles_missing_files_and_duplicates() {
    let temp = TempTree::new();
    let scene = temp.write(
        "scene.toml",
        "imports=['lib/root.toml']\ntempo=132\nseed=1\n[part]\nid='hat'\nuse='my.hat'\n",
    );
    temp.write("lib/root.toml","imports=['child.toml']\n[library.behaviors.'my.hat']\nuse='techno.closed_hat'\n[ library.behaviors.'my.hat'.profile]\nuse='my.profile'\n");
    temp.write(
        "lib/child.toml",
        "[library.profiles.'my.profile']\nuse='accent.subtle'\nboost=19\n",
    );
    assert_eq!(
        Composition::read(&scene).unwrap().parts[0].profile.boost,
        19
    );
    temp.write("lib/child.toml", "imports=['root.toml']\n");
    assert!(Composition::read(&scene).unwrap_err().contains("cycle"));
    temp.write("lib/child.toml", "imports=['missing.toml']\n");
    assert!(Composition::read(&scene).is_err());
    temp.write(
        "lib/child.toml",
        "[library.behaviors.'techno.kick']\nuse='techno.clap'\n",
    );
    assert!(Composition::read(&scene).unwrap_err().contains("duplicate"));
}
#[test]
fn all_packaged_examples_have_ordered_complete_midi_pairs() {
    for file in
        std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")).unwrap()
    {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let c = Composition::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for step in 0..64 {
            let (_, events) = resolve_step(&c, step);
            assert!(events.windows(2).all(|e| e[0].tick <= e[1].tick));
            let mut active = std::collections::HashMap::new();
            for e in events {
                let key = (e.bytes[0] & 15, e.bytes[1]);
                if e.bytes[0] & 0xf0 == 0x90 {
                    assert!(active.insert(key, e.tick).is_none());
                } else {
                    let on = active
                        .remove(&key)
                        .expect("note-off must follow its note-on");
                    assert!(e.tick > on && e.tick < on + 240);
                }
            }
            assert!(active.is_empty());
        }
    }
}
#[test]
fn watched_import_edits_apply_atomically_and_invalid_edits_keep_playing() {
    use std::io::BufRead;
    let temp = TempTree::new();
    let scene=temp.write("scene.toml","imports=['library.toml']\ntempo=400\nseed=1\nphrase_bars=1\n[part]\nid='hat'\nuse='my.hat'\n");
    let library = "[library.behaviors.'my.hat']\nuse='techno.closed_hat'\n[library.behaviors.'my.hat'.trigger]\nprobability=1.0\n";
    temp.write("library.toml", library);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_phasecraft"))
        .args([
            "play",
            scene.to_str().unwrap(),
            "--dry-run",
            "--bars",
            "4",
            "--watch",
            "--trace",
            "--lookahead-ms",
            "50",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut rows = vec![];
    for line in std::io::BufReader::new(child.stdout.take().unwrap()).lines() {
        let row: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        let step = row["step"].as_u64().unwrap();
        rows.push(row);
        if step == 0 {
            temp.write(
                "library.toml",
                &library.replace("probability=1.0", "probability=0.0"),
            );
        }
        if step == 16 {
            temp.write("library.toml", "invalid toml !");
        }
        if step == 32 {
            temp.write("library.toml", library);
        }
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

#[test]
fn alternate_seed_changes_probability_without_moving_fixed_kick() {
    let a = example("probability-locked");
    let b = example("probability-new-seed");
    let hats = |c: &Composition| {
        (0..64)
            .map(|s| {
                resolve_step(c, s)
                    .0
                    .into_iter()
                    .find(|t| t.part == "hat")
                    .unwrap()
                    .trigger
                    .admitted
            })
            .collect::<Vec<_>>()
    };
    assert_ne!(hats(&a), hats(&b));
    for step in 0..64 {
        let hits = |c: &Composition| {
            resolve_step(c, step)
                .1
                .into_iter()
                .filter(|e| e.bytes[1] == 36)
                .collect::<Vec<_>>()
        };
        assert_eq!(hits(&a), hits(&b));
    }
}
