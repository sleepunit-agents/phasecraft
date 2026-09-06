use phasecraft::{
    authoring::project,
    control::{Live, Parameter, removed_control_resets},
    music::{Composition, STEP_TICKS, resolve::resolve_step},
    playback::{
        MidiOutput,
        transport::{PlayOptions, run_with_controls},
    },
};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool, mpsc},
    time::Duration,
};
fn score() -> Composition {
    project::load(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/controllers/kick"
    )))
    .unwrap()
    .composition
}
fn live() -> Live {
    let mut l = Live::default();
    l.load(Some(score()));
    l
}
#[test]
fn edits_preserve_other_decisions_and_valid_cycle_lengths() {
    let mut l = live();
    let original = l.composition().unwrap();
    l.change("kick", Parameter::AccentProbability, -50).unwrap();
    let c = l.composition().unwrap();
    for step in 0..560 {
        let notes = |c: &Composition| {
            resolve_step(c, step)
                .1
                .into_iter()
                .filter(|e| e.bytes[0] & 0xf0 == 0x90)
                .map(|e| (e.tick, e.bytes[1]))
                .collect::<Vec<_>>()
        };
        assert_eq!(notes(&original), notes(&c));
    }
    l.change("kick", Parameter::ASteps, -64).unwrap();
    l.composition().unwrap().validate().unwrap();
    assert_eq!(l.view("kick").values[4].value, 1.);
    assert_eq!(l.view("kick").values[5].value, 1.);
    l.change("kick", Parameter::ARotation, -1).unwrap();
    assert_eq!(l.view("kick").values[6].value, 0.);
    l.change("kick", Parameter::Operator, -64).unwrap();
    assert_eq!(l.view("kick").values[7].value, 0.);
    l.change("kick", Parameter::BPulses, 1).unwrap();
    assert_eq!(l.view("kick").values[7].value, 1.);
    l.reset();
    assert_eq!(
        serde_json::to_string(&l.composition()).unwrap(),
        serde_json::to_string(&Some(original)).unwrap()
    );
}
#[test]
fn reset_restores_newly_latched_cc_even_on_a_rest() {
    let mut l = live();
    let base = l.composition().unwrap();
    l.change("kick", Parameter::Cutoff, -40).unwrap();
    let changed = l.composition().unwrap();
    assert!(removed_control_resets(&changed, &changed, 0).is_empty());
    let resets = removed_control_resets(&changed, &base, 16 * STEP_TICKS);
    assert_eq!(resets.len(), 1);
    assert_eq!(resets[0].bytes, [0xbe, 20, 127]);
    assert!(resets[0].boundary_reset);
    assert_eq!(resets[0].tick, 16 * STEP_TICKS);
}
#[test]
fn file_rebase_discards_stale_edits_and_unsupported_arrangement_controls() {
    let mut l = live();
    l.change("kick", Parameter::Cutoff, -40).unwrap();
    let old = l.generation;
    l.rebase(score());
    assert!(l.view("kick").edited);
    assert_ne!(l.generation, old);
    let mut changed = score();
    changed.seed += 1;
    l.rebase(changed);
    assert!(!l.view("kick").edited);
    let c = Composition::read(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/quickstart/techno-journey.toml"
    )))
    .unwrap();
    l.load(Some(c));
    assert!(l.view("kick").values.iter().all(|v| !v.enabled));
}
#[derive(Clone)]
struct Sink(Arc<Mutex<Vec<Vec<u8>>>>);
impl MidiOutput for Sink {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
}
#[test]
fn live_changes_commit_at_bar_and_reset_sends_default_during_silence() {
    let mut c = score();
    c.tempo = 400.;
    c.parts[0].trigger.probability = 0.;
    let mut l = Live::default();
    l.load(Some(c.clone()));
    let live = Arc::new(Mutex::new(l));
    let worker_live = live.clone();
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let sink = Sink(bytes.clone());
    let (tx, rx) = mpsc::sync_channel(128);
    let worker = std::thread::spawn(move || {
        run_with_controls(
            c,
            sink,
            PlayOptions {
                file: Default::default(),
                steps: Some(48),
                watch: false,
                trace: false,
                send_clock: false,
                lookahead: Duration::from_millis(100),
            },
            Arc::new(AtomicBool::new(true)),
            Some(tx),
            Some(worker_live),
        )
    });
    let mut edited = false;
    let mut reset = false;
    let mut changed_bar = false;
    let mut restored_bar = false;
    while let Ok(frame) = rx.recv_timeout(Duration::from_secs(5)) {
        if frame.step < 16 {
            assert!(!frame.composition.parts[0].parameters.contains_key("cutoff"));
        }
        if frame.step >= 16 && frame.step < 32 {
            assert_eq!(frame.composition.parts[0].parameters["cutoff"].value, 0.6);
            changed_bar = true;
        }
        if frame.step >= 32 {
            assert!(!frame.composition.parts[0].parameters.contains_key("cutoff"));
            restored_bar = true;
        }
        if !edited && frame.step >= 1 {
            live.lock()
                .unwrap()
                .change("kick", Parameter::Cutoff, -40)
                .unwrap();
            edited = true;
        }
        if !reset && frame.step >= 17 {
            live.lock().unwrap().reset();
            reset = true;
        }
    }
    worker.join().unwrap().unwrap();
    assert!(changed_bar && restored_bar);
    let bytes = bytes.lock().unwrap();
    assert!(bytes.iter().all(|b| b[0] & 0xf0 == 0xb0));
    assert!(
        bytes
            .windows(2)
            .any(|pair| pair[0] == [0xbe, 20, 76] && pair[1] == [0xbe, 20, 127]),
        "{bytes:?}"
    );
}

#[test]
fn authored_order_and_independent_part_edits_survive_selection() {
    let c = project::load(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/templates/project"
    )))
    .unwrap()
    .composition;
    assert_eq!(c.parts[0].id, "kick");
    let custom = Composition::parse(
        "tempo=132\nseed=1\n[parts.zebra]\nuse='techno.kick'\n[parts.alpha]\nuse='techno.clap'\n",
    )
    .unwrap();
    assert_eq!(
        custom
            .parts
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        vec!["zebra", "alpha"]
    );
    let mut l = Live::default();
    l.load(Some(c));
    l.change("kick", Parameter::TriggerProbability, -30)
        .unwrap();
    l.select("clap").unwrap();
    l.change("clap", Parameter::TriggerProbability, -50)
        .unwrap();
    assert_eq!(l.view("kick").values[2].value, 0.7);
    assert_eq!(l.view("clap").values[2].value, 0.5);
    assert!(l.view("clap").selected);
    l.reset();
    assert!(!l.view("kick").edited);
    assert!(!l.view("clap").edited);
    for name in ["techno", "dnb", "garage"] {
        let c = project::load(
            &std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/controllers/kit"
            ))
            .join(format!("{name}.toml")),
        )
        .unwrap()
        .composition;
        l.load(Some(c));
        assert!(l.views().iter().all(|v| v.values[0].enabled));
    }
}
