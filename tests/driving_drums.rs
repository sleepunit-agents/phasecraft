use phasecraft::playback::{EventDispatcher, MidiOutput};
use phasecraft::{authoring::project, music::resolve::Compiled};
use std::{path::Path, time::Duration};

#[derive(Default)]
struct Recording(Vec<Vec<u8>>);
impl MidiOutput for Recording {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0.push(bytes.to_vec());
        Ok(())
    }
}

#[test]
fn driving_drums_project_matches_the_eight_bar_audition_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/run-the-line-driving-drums");
    let loaded = project::load(&root).unwrap();
    assert!(loaded.gaps.is_empty());
    assert_eq!(loaded.midi.port.as_deref(), Some("Phasecraft"));
    assert!(loaded.midi.send_clock);
    assert_eq!(loaded.composition.tempo, 170.0);
    assert_eq!(loaded.composition.seed, 4471);
    let mut compiled = Compiled::new(&loaded.composition);
    let actual: Vec<_> = (0..128)
        .flat_map(|step| compiled.resolve_step(step).1)
        .map(|event| (event.tick, event.bytes))
        .collect();
    let mut expected = Vec::new();
    for bar in 0..8 {
        for (offset, note, velocity) in [
            (0, 36, 127),
            (960, 38, 121),
            (2400, 36, 114),
            (2880, 38, 121),
        ] {
            let tick = bar * 3840 + offset;
            expected.push((tick, [0x99, note, velocity]));
            expected.push((tick + 240, [0x89, note, 0]));
        }
    }
    assert_eq!(actual, expected); // Includes every note-off and rejects unexpected CCs/hits.
    let from_manifest = project::load(&root.join("phasecraft.toml")).unwrap();
    assert_eq!(from_manifest.composition.tempo, 170.0);
}

#[test]
fn audition_dispatch_and_stop_leave_kit_controls_untouched() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/run-the-line-driving-drums");
    let loaded = project::load(&root).unwrap();
    // Include a phrase boundary and Stop both before a scheduled note-off and after it.
    for stop_after_step in [0, 1, 64, 127] {
        let mut compiled = Compiled::new(&loaded.composition);
        let mut wire = EventDispatcher::new(Recording::default());
        for step in 0..=stop_after_step {
            for event in compiled.resolve_step(step).1 {
                // Stop before this cell boundary, leaving a note active in the first case.
                if event.tick < (stop_after_step + 1) * 240 {
                    wire.dispatch(&event, Duration::ZERO, Duration::ZERO)
                        .unwrap();
                }
            }
        }
        wire.cleanup().unwrap();
        let after_stop = wire.sink.0.clone();
        wire.cleanup().unwrap();
        assert_eq!(wire.sink.0, after_stop);
        assert!(!after_stop.is_empty());
        let mut active = std::collections::BTreeSet::new();
        for bytes in after_stop {
            assert!(bytes[1] == 36 || bytes[1] == 38);
            match bytes[0] {
                0x99 => assert!(active.insert(bytes[1])),
                0x89 => assert!(active.remove(&bytes[1])),
                _ => panic!("unexpected audition output: {bytes:?}"),
            }
        }
        assert!(active.is_empty());
    }
}

// Exercises the real transport's setup, clock, dispatch and cleanup without a MIDI port.
// Explicit opt-in: exact real-time counts can fail when a host stalls past the late limit.
#[test]
#[ignore = "real-time eight-bar recording; run on an idle host"]
fn audition_transport_records_eight_bars_without_control_output() {
    use phasecraft::playback::transport::{PlayOptions, run_controlled};
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Recording>>);
    impl MidiOutput for Sink {
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0.lock().unwrap().send(bytes)
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/run-the-line-driving-drums");
    let loaded = project::load(&root).unwrap();
    let sink = Sink::default();
    let captured = sink.0.clone();
    run_controlled(
        loaded.composition,
        sink,
        PlayOptions {
            file: root,
            steps: Some(128),
            watch: false,
            trace: false,
            send_clock: loaded.midi.send_clock,
            lookahead: Duration::from_millis(100),
        },
        Arc::new(AtomicBool::new(true)),
        None,
    )
    .unwrap();
    let captured = &captured.lock().unwrap().0;
    assert_eq!(captured.first().unwrap(), &[0xfa]);
    assert_eq!(captured.last().unwrap(), &[0xfc]);
    assert_eq!(
        captured.iter().filter(|b| b.as_slice() == [0xf8]).count(),
        768
    );
    let notes: Vec<_> = captured.iter().filter(|b| b.len() != 1).cloned().collect();
    let bar = vec![
        vec![0x99, 36, 127],
        vec![0x89, 36, 0],
        vec![0x99, 38, 121],
        vec![0x89, 38, 0],
        vec![0x99, 36, 114],
        vec![0x89, 36, 0],
        vec![0x99, 38, 121],
        vec![0x89, 38, 0],
    ];
    let expected: Vec<_> = (0..8).flat_map(|_| bar.clone()).collect();
    assert_eq!(notes, expected); // Also excludes setup, running and Stop CCs.
    assert_eq!(captured.len(), 768 + 64 + 2);
}
