use phasecraft::{
    music::Composition,
    music::{
        resolve::resolve_step,
        rhythm::{BooleanOp, Expression, ReferenceMode},
    },
};
use std::path::PathBuf;
fn example(name: &str) -> Composition {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let path = ["quickstart", "studies", "showcases"]
        .iter()
        .map(|dir| root.join(dir).join(format!("{name}.toml")))
        .find(|path| path.is_file())
        .expect("example exists");
    Composition::read(&path).unwrap()
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
    for file in ["quickstart", "studies", "showcases"]
        .into_iter()
        .flat_map(|dir| {
            std::fs::read_dir(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("examples")
                    .join(dir),
            )
            .unwrap()
        })
    {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let c = Composition::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // Pairs are complete across the run, not per window: an anticipated hit (negative
        // delay, or humanize jitter) opens in one sixteenth and closes in the next.
        let mut active = std::collections::HashMap::new();
        let mut controls = std::collections::HashMap::new();
        for step in 0..64 {
            let (_, events) = resolve_step(&c, step);
            assert!(events.windows(2).all(|e| e[0].tick <= e[1].tick));
            for e in events {
                if e.parameter {
                    assert_eq!(e.bytes[0] & 0xf0, 0xb0);
                    continue;
                }
                let key = (e.bytes[0] & 15, e.bytes[1]);
                if e.bytes[0] & 0xf0 == 0xb0 {
                    if let Some(reset) = e.reset_value {
                        assert!(controls.insert(key, (e.tick, reset)).is_none());
                    } else {
                        let (on, reset) = controls.remove(&key).expect("CC reset follows onset");
                        assert!(e.tick > on && e.tick < on + 240);
                        assert_eq!(e.bytes[2], reset);
                    }
                } else if e.bytes[0] & 0xf0 == 0x90 {
                    assert!(active.insert(key, e.tick).is_none());
                } else {
                    let on = active
                        .remove(&key)
                        .expect("note-off must follow its note-on");
                    assert!(e.tick > on && e.tick < on + 240);
                }
            }
        }
        // Only hits anticipated from the first unrendered step may still be open.
        assert!(
            active.values().all(|&on| on >= 63 * 240),
            "{}",
            path.display()
        );
        assert!(
            controls.values().all(|&(on, _)| on >= 63 * 240),
            "{}",
            path.display()
        );
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

// ---- M1.7: the kit is read, not re-typed (until-stop TO-PHASECRAFT § E, rows B18 B19) ----

/// A project in the shape until-stop will take: the prepared kit imported verbatim, plus a
/// local kit file that names the piece's instruments — aliases into the prepared table, and
/// one instrument (`sub`) phasecraft has never heard of.
fn kit_project(temp: &TempTree, voice: &str) -> PathBuf {
    let prepared =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/project/kits/909-prepared.toml");
    temp.write(
        "kits/909-prepared.toml",
        &std::fs::read_to_string(prepared).unwrap(),
    );
    temp.write(
        "kits/until-stop.toml",
        r#"
[library.kit]
kick       = "kit.prepared.kick"
snare      = "kit.prepared.snare"
closed_hat = "kit.prepared.closed_hat"
metal      = "kit.prepared.ride"
[library.kit.sub]
note = 48
channel = 2
"#,
    );
    temp.write("config/midi.toml", "port='Phasecraft'\n");
    temp.write(
        "phasecraft.toml",
        "name='until-stop'\ndefault='until-stop.toml'\ncompositions=['until-stop.toml']\nlibraries=['kits/909-prepared.toml','kits/until-stop.toml']\nmidi='config/midi.toml'\n",
    );
    temp.write(
        "until-stop.toml",
        &format!("tempo=126\nseed=91827\n{voice}"),
    )
}
#[test]
fn kit_binds_a_voice_to_the_imported_prepared_table_verbatim() {
    let temp = TempTree::new();
    let file = kit_project(
        &temp,
        "[parts.hat]\nkit='closed_hat'\ncompose=['std.backbeat','std.no_accent']\n[parts.hat.parameters.decay]\nvalue=0.5\n\
         [parts.metal]\nkit='metal'\ncompose=['std.backbeat','std.no_accent']\n[parts.metal.output]\ngate='1/16'\n\
         [parts.sub]\nkit='sub'\ncompose=['std.backbeat','std.no_accent']\n",
    );
    let loaded = phasecraft::authoring::project::load(&file).unwrap();
    assert!(loaded.gaps.is_empty());
    let part = |id: &str| {
        loaded
            .composition
            .parts
            .iter()
            .find(|p| p.id == id)
            .unwrap()
    };
    // The alias reads the prepared table: nothing was re-typed, so every field is the template's.
    let hat = part("hat");
    assert_eq!((hat.output.note, hat.output.channel), (42, 10));
    let decay = &hat.output.controls["decay"];
    assert_eq!((decay.cc, decay.channel), (76, Some(15)));
    assert_eq!(decay.default, Some(0.7007874015748031));
    assert_eq!(hat.output.controls.len(), 4);
    // The piece's word for the ride is `metal`; the binding is the ride's, the gate is the voice's.
    let metal = part("metal");
    assert_eq!((metal.output.note, metal.output.channel), (51, 10));
    assert_eq!(metal.output.controls["cutoff"].cc, 83);
    assert_eq!(metal.output.gate_ticks, 240);
    // An instrument declared locally, with no controls at all — and nothing here asks for one.
    let sub = part("sub");
    assert_eq!((sub.output.note, sub.output.channel), (48, 2));
    assert!(sub.output.controls.is_empty());
    // A composition-level read is the same read.
    assert_eq!(Composition::read(&file).unwrap().parts.len(), 3);
}
#[test]
fn kit_replaces_a_composed_output_and_the_voice_may_overlay_it() {
    let base = "tempo=132\nseed=1\n[library.kit.sub]\nnote=48\nchannel=2\n[library.kit]\nclap='kit.909.clap'\n";
    // A composed behavior brought a snare binding; the kit is the binding and replaces it whole.
    let c = Composition::parse(&format!(
        "{base}[part]\nid='low'\ncompose=['std.backbeat','std.no_accent','kit.909.snare']\nkit='sub'\n"
    ))
    .unwrap();
    assert_eq!((c.parts[0].output.note, c.parts[0].output.channel), (48, 2));
    // The voice's own output fields overlay the kit's: a pitched voice moves the note.
    let c = Composition::parse(&format!(
        "{base}[part]\nid='low'\nkit='sub'\ncompose=['std.backbeat','std.no_accent']\n[part.output]\nnote=50\n"
    ))
    .unwrap();
    assert_eq!((c.parts[0].output.note, c.parts[0].output.channel), (50, 2));
    // An alias into the built-in 909 reads its note.
    let c = Composition::parse(&format!(
        "{base}[part]\nid='clap'\nkit='clap'\ncompose=['std.backbeat','std.no_accent']\n"
    ))
    .unwrap();
    assert_eq!(c.parts[0].output.note, 39);
}
#[test]
fn kit_errors_name_the_instrument_and_the_place() {
    let base = "tempo=132\nseed=1\n[library.kit.sub]\nnote=48\nchannel=2\n[part]\nid='x'\ncompose=['std.backbeat','std.no_accent']\n";
    let err = |source: String| Composition::parse(&source).unwrap_err();
    // Unknown instrument: the error lists what the kit does declare (check.py's refusal).
    let e = err(format!("{base}kit='amen'\n"));
    assert!(
        e.contains("unknown instrument \"amen\"") && e.contains("\"sub\""),
        "{e}"
    );
    // A kit entry is checked where it is written, not at the first voice that uses it.
    let e = err("tempo=132\nseed=1\n[library.kit.bad]\nnote=300\nchannel=2\n[part]\nid='x'\ncompose=['std.backbeat','std.no_accent']\n".into());
    assert!(e.contains("kit.bad"), "{e}");
    let e = err("tempo=132\nseed=1\n[library.kit.bad]\nnote=48\nchannel=2\nnotes=1\n[part]\nid='x'\ncompose=['std.backbeat','std.no_accent']\n".into());
    assert!(e.contains("kit.bad") && e.contains("notes"), "{e}");
    // An alias to a behavior that has no output, or that does not exist.
    let e = err(format!("{base}kit='b'\n[library.kit]\nb='std.backbeat'\n"));
    assert!(e.contains("has no output"), "{e}");
    let e = err(format!("{base}kit='b'\n[library.kit]\nb='kit.nowhere'\n"));
    assert!(e.contains("unknown behavior \"kit.nowhere\""), "{e}");
    // A profile is not a Part.
    let e = err(format!("{base}[part.profile]\nkit='sub'\n"));
    assert!(e.contains("kit binds a Part"), "{e}");
    // Two kit files may not both declare an instrument.
    let e = err(format!(
        "{base}[library.kit]\nsub2='kit.909.kick'\n[library.kit.sub2]\nnote=1\n"
    ));
    assert!(e.contains("duplicate"), "{e}");
    // A declared controls table still refuses a name it does not list — that is the error half
    // of B19 and it did not move: the gap mode is only for an instrument that declares nothing.
    let e = err(format!(
        "{base}kit='k2'\n[library.kit.k2]\nnote=1\ncontrols.level={{cc=22}}\n[part.parameters.cutoff]\nvalue=0.5\n"
    ));
    assert!(
        e.contains("parameter \"cutoff\" requires an output.controls mapping"),
        "{e}"
    );
}

// ---- M1.7 follow-up (t-504, t-505): every kit entry is valid where it is written ----

#[test]
fn kit_entries_are_checked_where_written_whether_or_not_a_voice_uses_them() {
    // A composition whose one voice never binds to the kit: the entries below are UNUSED.
    let piece = |kit: &str| {
        format!(
            "tempo=132\nseed=1\n{kit}[part]\nid='x'\ncompose=['std.backbeat','std.no_accent']\n[part.output]\nnote=36\n"
        )
    };
    let err = |kit: &str| Composition::parse(&piece(kit)).unwrap_err();
    // Range, not just shape: `note = 200` and `channel = 99` are both valid u8s.
    let e = err("[library.kit.bad]\nnote=200\n");
    assert!(
        e.contains("kit.bad") && e.contains("note 0..127") && e.contains("note 200"),
        "{e}"
    );
    let e = err("[library.kit.bad]\nnote=36\nchannel=99\n");
    assert!(e.contains("kit.bad") && e.contains("channel 99"), "{e}");
    let e = err("[library.kit.bad]\nnote=36\ngate_ticks=0\n");
    assert!(e.contains("kit.bad") && e.contains("gate_ticks"), "{e}");
    // An unused alias must still resolve, and the error names the entry and its target.
    let e = err("[library.kit]\nbad='kit.nowhere'\n");
    assert!(
        e.contains("kit.bad") && e.contains("unknown behavior \"kit.nowhere\""),
        "{e}"
    );
    let e = err("[library.kit]\nbad='std.backbeat'\n");
    assert!(e.contains("kit.bad") && e.contains("has no output"), "{e}");
    // The good entries still load, used or not.
    Composition::parse(&piece(
        "[library.kit.sub]\nnote=48\nchannel=2\n[library.kit]\nclap='kit.909.clap'\n",
    ))
    .unwrap();
}

#[test]
fn a_voice_cannot_rescue_a_bad_kit_entry_by_overlaying_it() {
    // Before t-504 this loaded: the Part's own `output.note = 50` overlaid the kit's 200 and
    // only the merged output was ever checked. The entry is now refused where it is written.
    let e = Composition::parse(
        "tempo=132\nseed=1\n[library.kit.sub]\nnote=200\nchannel=2\n[part]\nid='low'\nkit='sub'\ncompose=['std.backbeat','std.no_accent']\n[part.output]\nnote=50\n",
    )
    .unwrap_err();
    assert!(e.contains("kit.sub") && e.contains("note 200"), "{e}");
}

#[test]
fn kit_errors_in_a_library_file_name_that_file_not_the_composition() {
    let temp = TempTree::new();
    let piece = "tempo=132\nseed=1\nimports=['kits/local.toml']\n[part]\nid='x'\ncompose=['std.backbeat','std.no_accent']\n[part.output]\nnote=36\n";
    let read = |kit: &str| {
        temp.write("kits/local.toml", kit);
        let file = temp.write("piece.toml", piece);
        Composition::read(&file)
    };
    let lib = temp.0.join("kits/local.toml").display().to_string();
    // A shape error raised while the file is being added (t-505's first class).
    let e = read("[library.kit.bad]\nnote='not a number'\n").unwrap_err();
    assert!(e.contains(&lib) && e.contains("kit.bad"), "{e}");
    // A range error, where it is written.
    let e = read("[library.kit.bad]\nnote=200\n").unwrap_err();
    assert!(
        e.contains(&lib) && e.contains("kit.bad") && e.contains("note 200"),
        "{e}"
    );
    // An alias checked after the registry is complete, when no call site is left to name
    // the file: the entry's recorded origin names it (t-505's second class).
    let e = read("[library.kit]\nbad='kit.nowhere'\n").unwrap_err();
    assert!(
        e.contains(&lib) && e.contains("kit.bad") && e.contains("kit.nowhere"),
        "{e}"
    );
    // A duplicate names the file that declared the second copy.
    temp.write("kits/first.toml", "[library.kit.thud]\nnote=40\n");
    temp.write("kits/second.toml", "[library.kit.thud]\nnote=41\n");
    let file = temp.write(
        "twice.toml",
        "tempo=132\nseed=1\nimports=['kits/first.toml','kits/second.toml']\n[part]\nid='x'\nkit='thud'\ncompose=['std.backbeat','std.no_accent']\n",
    );
    let e = Composition::read(&file).unwrap_err();
    let second = temp.0.join("kits/second.toml").display().to_string();
    assert!(e.contains(&second) && e.contains("duplicate"), "{e}");
}

#[test]
fn a_kit_alias_may_name_a_behavior_a_later_library_declares() {
    // Libraries load in order: built-ins, `libraries`, imports, then the composition's own
    // table. The alias below is written in an import and resolves to a behavior declared in
    // the composition, so an eager check at add time would refuse a valid kit. The check runs
    // once, on the complete registry.
    let temp = TempTree::new();
    temp.write("kits/local.toml", "[library.kit]\nthud='local.thud'\n");
    let file = temp.write(
        "piece.toml",
        "tempo=132\nseed=1\nimports=['kits/local.toml']\n[library.behaviors.'local.thud']\noutput={note=40,channel=3}\n[part]\nid='x'\nkit='thud'\ncompose=['std.backbeat','std.no_accent']\n",
    );
    let c = Composition::read(&file).unwrap();
    assert_eq!((c.parts[0].output.note, c.parts[0].output.channel), (40, 3));
}
#[test]
fn an_instrument_that_declares_nothing_is_a_listed_gap_that_never_plays() {
    use phasecraft::authoring::project;
    let temp = TempTree::new();
    // `sub` declares no controls; the voice follows one and sets a profile response on another.
    let file = kit_project(
        &temp,
        "[parts.sub]\nkit='sub'\ncompose=['std.backbeat','std.no_accent']\n[parts.sub.parameters.cutoff]\nvalue=0.5\n[parts.sub.profile.controls.level]\nboost=0.2\n\
         [parts.hat]\nkit='closed_hat'\ncompose=['std.backbeat','std.no_accent']\n[parts.hat.parameters.decay]\nvalue=0.5\n",
    );
    // The draft reads: the unbound controls are listed once, by name, and set aside.
    let draft = project::load_draft(&file).unwrap();
    assert_eq!(draft.gaps.len(), 1, "{:?}", draft.gaps);
    let gap = &draft.gaps[0];
    assert_eq!((gap.part.as_str(), gap.instrument.as_str()), ("sub", "sub"));
    assert_eq!(gap.controls, ["cutoff", "level"]);
    let sub = draft
        .composition
        .parts
        .iter()
        .find(|p| p.id == "sub")
        .unwrap();
    assert!(sub.parameters.is_empty() && sub.profile.controls.is_empty());
    // The hat's declared binding is untouched by the sub's gap.
    let hat = draft
        .composition
        .parts
        .iter()
        .find(|p| p.id == "hat")
        .unwrap();
    assert_eq!(hat.parameters.len(), 1);
    // `validate` stays valid and carries the gap, with the file it belongs to.
    let report = project::validate(&file);
    assert!(
        report.valid && report.errors.is_empty(),
        "{:?}",
        report.errors
    );
    assert_eq!(report.gaps.len(), 1);
    assert!(
        report.gaps[0].contains("until-stop.toml") && report.gaps[0].contains("cutoff, level"),
        "{}",
        report.gaps[0]
    );
    // Every playback door refuses it: the project load, the composition read, the string parse.
    for result in [
        project::load(&file).map(|_| ()),
        Composition::read(&file).map(|_| ()),
        Composition::parse("tempo=132\nseed=1\n[library.kit.sub]\nnote=48\nchannel=2\n[part]\nid='sub'\nkit='sub'\ncompose=['std.backbeat','std.no_accent']\n[part.parameters.cutoff]\nvalue=0.5\n").map(|_| ()),
    ] {
        let e = result.unwrap_err();
        assert!(e.contains("unbound control") && e.contains("\"sub\"") && e.contains("cutoff"), "{e}");
    }
    // An explicit, empty controls table is a declaration, and it declares no `cutoff`: error, not gap.
    let e = Composition::parse("tempo=132\nseed=1\n[library.kit.sub]\nnote=48\nchannel=2\ncontrols={}\n[part]\nid='sub'\nkit='sub'\ncompose=['std.backbeat','std.no_accent']\n[part.parameters.cutoff]\nvalue=0.5\n").unwrap_err();
    assert!(e.contains("requires an output.controls mapping"), "{e}");
    // The voice may close its own gap by declaring the control on its output.
    let ok = Composition::parse("tempo=132\nseed=1\n[library.kit.sub]\nnote=48\nchannel=2\n[part]\nid='sub'\nkit='sub'\ncompose=['std.backbeat','std.no_accent']\n[part.output.controls.cutoff]\ncc=74\n[part.parameters.cutoff]\nvalue=0.5\n").unwrap();
    assert_eq!(ok.parts[0].output.controls["cutoff"].cc, 74);
}
