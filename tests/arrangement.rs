use phasecraft::music::{Composition, STEP_TICKS, resolve::resolve_step};

const BASE: &str = r#"
tempo=400
seed=73
phrase_bars=1
[parts.hat]
use="techno.closed_hat"
trigger.rhythm={steps=5,pulses=2,rotation=0}
trigger.probability=1.0
accent.probability=0.0
[phrases.A]
[phrases.A2]
use="A"
seed=74
parts.hat.trigger.probability=0.4
"#;
fn sequence(sections: &str, repeat: bool) -> Composition {
    Composition::parse(&format!(
        "{BASE}\n[arrangement]\nrepeat={repeat}\nsections={sections}"
    ))
    .unwrap()
}
#[test]
fn inheritance_expansion_and_roundtrip_keep_procedural_identity() {
    let c = sequence("[{phrase='A'},{phrase='A2',bars=2,repeat=2}]", false);
    let a = c.arrangement.as_ref().unwrap();
    assert_eq!(a.sections.len(), 3);
    assert_eq!(c.end_step(), Some(80));
    assert_eq!(a.sections[0].composition.seed, 73);
    assert_eq!(a.sections[1].composition.seed, 74);
    assert_eq!(a.sections[1].composition.parts[0].trigger.probability, 0.4);
    let copy = Composition::parse(&toml::to_string(&c).unwrap()).unwrap();
    for step in 0..96 {
        assert_eq!(resolve_step(&c, step).1, resolve_step(&copy, step).1);
    }
    assert!(resolve_step(&c, 80).0.is_empty());
}
#[test]
fn restart_and_continue_have_explicit_clocks_and_global_deadlines() {
    let c = sequence(
        "[{phrase='A',phase='restart'},{phrase='A',phase='continue'},{phrase='A',phase='restart'}]",
        true,
    );
    for step in 0..144 {
        let local = if step % 48 / 16 == 1 { step } else { step % 16 };
        let (_, expected) = resolve_step(&c.parts_only(), local);
        let (traces, events) = resolve_step(&c, step);
        assert_eq!(events.len(), expected.len());
        for (actual, original) in events.iter().zip(expected) {
            assert_eq!(actual.bytes, original.bytes);
            assert_eq!(actual.tick, original.tick + (step - local) * STEP_TICKS);
        }
        assert_eq!(traces[0].step, step);
        assert_eq!(traces[0].section.as_ref().unwrap().cycle, step / 48);
    }
}
trait PartsOnly {
    fn parts_only(&self) -> Composition;
}
impl PartsOnly for Composition {
    fn parts_only(&self) -> Composition {
        let mut c = self.clone();
        c.arrangement = None;
        c
    }
}
#[test]
fn section_defaults_precede_incoming_values_and_survive_a_silent_section() {
    let c = Composition::parse(
        r#"
        tempo=400
        seed=1
        [parts.hat]
        use="techno.closed_hat"
        output.controls.cutoff={cc=74,default=1.0}
        parameters.cutoff={value=0.2,ramp={to=0.8,over_bars=1}}
        [phrases.A]
        [phrases.B]
        parts.hat.parameters.cutoff={value=0.3,ramp={to=0.3,over_bars=1}}
        [phrases.C.parts.hat]
        use="techno.closed_hat"
        trigger.probability=0.0
        [arrangement]
        sections=[{phrase="A",bars=1},{phrase="B",bars=1},{phrase="C",bars=1}]
    "#,
    )
    .unwrap();
    let (_, events) = resolve_step(&c, 16);
    let cc: Vec<_> = events
        .iter()
        .filter(|e| e.tick == 16 * STEP_TICKS && e.bytes[0] & 0xf0 == 0xb0)
        .collect();
    assert!(cc[0].boundary_reset);
    assert_eq!(cc[0].bytes[2], 127);
    assert!(!cc[1].boundary_reset);
    assert_eq!(cc[1].bytes[2], 38);
    let (_, events) = resolve_step(&c, 32);
    assert_eq!(events.len(), 1);
    assert!(events[0].boundary_reset);
    assert_eq!(events[0].bytes[2], 127);
}
#[test]
fn invalid_or_ambiguous_phrase_definitions_fail_before_playback() {
    for extra in [
        "[phrases.bad]\nuse='missing'",
        "[phrases.bad]\nuse='bad'",
        "[phrases.bad]\ntempo=132",
        "[phrases.bad.parts.hat.trigger]\nprobability=2.0",
        "[arrangement]\nsections=[]",
        "[arrangement]\nsections=[{phrase='missing'}]",
        "[arrangement]\nsections=[{phrase='A',repeat=0}]",
        "[arrangement]\nsections=[{phrase='A',bars=0}]",
        "[arrangement]\nsections=[{phrase='A',bars=65536,repeat=2}]",
    ] {
        assert!(
            Composition::parse(&format!("{BASE}\n{extra}")).is_err(),
            "{extra}"
        );
    }
}
#[test]
fn musical_edits_can_reload_but_structural_and_routing_edits_cannot() {
    let c = sequence("[{phrase='A'},{phrase='A2'}]", true);
    let mut next = c.clone();
    next.arrangement.as_mut().unwrap().sections[0]
        .composition
        .seed = 99;
    assert!(c.same_arrangement_layout(&next));
    next.arrangement.as_mut().unwrap().sections[0]
        .composition
        .parts[0]
        .output
        .note = 43;
    assert!(!c.same_arrangement_layout(&next));
    let next = sequence("[{phrase='A',bars=2},{phrase='A2'}]", true);
    assert!(!c.same_arrangement_layout(&next));
}
#[test]
fn finite_transport_stops_on_its_own_with_exact_clock_count() {
    use phasecraft::playback::{
        MidiOutput,
        transport::{PlayOptions, run_controlled},
    };
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<Vec<u8>>>>);
    impl MidiOutput for Sink {
        fn send(&mut self, b: &[u8]) -> Result<(), String> {
            self.0.lock().unwrap().push(b.to_vec());
            Ok(())
        }
    }
    let c = sequence("[{phrase='A'},{phrase='A2'}]", false);
    let sink = Sink(Arc::new(Mutex::new(vec![])));
    let messages = sink.0.clone();
    let started = std::time::Instant::now();
    let clock = phasecraft::playback::MusicalClock { tempo: c.tempo };
    let end_tick = c.end_step().unwrap() * STEP_TICKS;
    run_controlled(
        c,
        sink,
        PlayOptions {
            file: "unused".into(),
            steps: None,
            watch: false,
            trace: false,
            send_clock: false,
            lookahead: std::time::Duration::from_millis(50),
        },
        Arc::new(AtomicBool::new(true)),
        None,
    )
    .unwrap();
    assert!(started.elapsed() >= clock.time_at_tick(end_tick));
    assert!(messages.lock().unwrap().iter().any(|b| b[0] & 0xf0 == 0x90));
    assert_eq!(messages.lock().unwrap().last().unwrap()[0] & 0xf0, 0x80);
    // Exact clock semantics use explicit deadlines, independent of CI host jitter.
    let origin = std::time::Instant::now();
    let mut sync = phasecraft::playback::sync::ClockOutput::new(
        origin,
        clock,
        phasecraft::playback::sync::SyncOptions {
            enabled: true,
            end_tick: Some(end_tick),
        },
    );
    let mut pulses = Sink(Arc::new(Mutex::new(vec![])));
    for pulse in 0..192 {
        let deadline = origin + clock.time_at_tick(pulse * phasecraft::playback::sync::CLOCK_TICKS);
        sync.send(&mut pulses, deadline).unwrap();
    }
    assert!(sync.deadline().is_none());
    sync.stop(&mut pulses).unwrap();
    let pulses = pulses.0.lock().unwrap();
    assert_eq!(pulses.first().unwrap(), &[0xfa]);
    assert_eq!(pulses.last().unwrap(), &[0xfc]);
    assert_eq!(
        pulses.iter().filter(|b| b.as_slice() == [0xf8]).count(),
        192
    );
}

#[test]
fn realized_windows_handle_absent_parts_and_a_finite_end() {
    let c = sequence("[{phrase='A'}]", false);
    let pattern = phasecraft::music::resolve::realize(&c, &c.parts[0], 0, 32);
    assert!(!pattern.events.is_empty());
    assert!(pattern.events.iter().all(|e| e.tick < 16 * STEP_TICKS));
    let mut absent = c.parts[0].clone();
    absent.id = "absent".into();
    assert!(
        phasecraft::music::resolve::realize(&c, &absent, 0, 32)
            .events
            .is_empty()
    );
}

#[test]
fn watched_arrangements_reject_layout_edits_and_accept_musical_edits() {
    use std::{
        fs,
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sequence.toml");
    let source = format!("{BASE}\n[arrangement]\nsections=[{{phrase='A',bars=4}}]");
    fs::write(&file, &source).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_phasecraft"))
        .arg("play")
        .arg(&file)
        .args(["--dry-run", "--watch", "--trace", "--lookahead-ms", "50"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut rows = vec![];
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let row: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        match row["step"].as_u64().unwrap() {
            0 => fs::write(&file, source.replace("bars=4", "bars=2")).unwrap(),
            16 => fs::write(
                &file,
                source.replace("trigger.probability=1.0", "trigger.probability=0.0"),
            )
            .unwrap(),
            _ => (),
        }
        rows.push(row);
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(rows.len(), 64);
    assert!(rows.iter().all(|r| r["section"]["bars"] == 4));
    assert!(rows[32..].iter().all(|r| r["event"].is_null()));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("restart playback to change arrangement layout")
    );
}

#[test]
fn human_inspection_uses_the_incoming_part_set() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sequence.toml");
    let source = format!(
        "{BASE}\n[phrases.B]\n[[phrases.B.parts]]\nid='kick'\nuse='techno.kick'\n[arrangement]\nsections=[{{phrase='A',bars=1}},{{phrase='B',bars=1}}]"
    );
    std::fs::write(&file, source).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phasecraft"))
        .arg("inspect")
        .arg(file)
        .args(["--human", "--start", "16", "--steps", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("B section 2/2"));
    assert!(text.contains("note=36 channel=10"));
}
