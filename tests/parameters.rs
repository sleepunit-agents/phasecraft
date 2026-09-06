use phasecraft::{
    music::{
        Composition,
        parameter::{ParameterLane, Ramp},
        resolve::resolve_step,
    },
    playback::{EventDispatcher, MidiOutput},
};
use std::time::Duration;
#[derive(Default)]
struct Recording(Vec<Vec<u8>>);
impl MidiOutput for Recording {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0.push(bytes.to_vec());
        Ok(())
    }
}
fn system() -> Composition {
    Composition::parse(
        r#"
    tempo=132
    seed=1
    phrase_bars=4
    [parts.hat]
    use="techno.closed_hat"
    trigger.rhythm={steps=1,pulses=1}
    accent.rhythm={steps=1,pulses=1}
    accent.probability=1.0
    accent.amount=1.0
    output.controls.cutoff={cc=20,channel=16}
    parameters.cutoff={value=0.2,ramp={to=1.0,over_bars=8}}
    "#,
    )
    .unwrap()
}
#[test]
fn eight_bar_ramp_crosses_phrases_and_holds_endpoint_in_musical_time() {
    let c = system();
    let lane = &c.parts[0].parameters["cutoff"];
    assert_eq!(lane.at(0), 0.2);
    assert!((lane.at(4 * 3840) - 0.6).abs() < 1e-12);
    assert_eq!(lane.at(8 * 3840), 1.0);
    assert_eq!(lane.at(100 * 3840), 1.0);
    assert_eq!(lane.at(u64::MAX), 1.0);
    let delayed = ParameterLane {
        automation: None,
        value: 1.0,
        ramp: Some(Ramp {
            to: 0.0,
            start_bar: 3,
            over_bars: 8,
        }),
    };
    assert_eq!(delayed.at(3840), 1.0);
    assert_eq!(delayed.at(2 * 3840), 1.0);
    assert_eq!(delayed.at(6 * 3840), 0.5);
    assert_eq!(delayed.at(10 * 3840), 0.0);
    let mut tempo = c.clone();
    tempo.tempo = 172.0;
    for step in [0, 64, 127, 128, 129, 200] {
        assert_eq!(resolve_step(&c, step).1, resolve_step(&tempo, step).1);
    }
}
#[test]
fn held_parameter_initializes_on_rests_deduplicates_and_resets_on_stop() {
    let mut c = system();
    c.parts[0].trigger.probability = 0.0;
    c.parts[0].parameters.get_mut("cutoff").unwrap().ramp = None;
    // Declaring other available kit bindings does not require enabling all responses.
    c.parts[0].output.controls.insert(
        "unused".into(),
        phasecraft::music::accent::ControlOutput {
            default: None,
            cc: 21,
            channel: Some(16),
        },
    );
    c.validate().unwrap();
    let mut d = EventDispatcher::new(Recording::default());
    for step in 0..128 {
        let (traces, events) = resolve_step(&c, step);
        assert!(traces[0].event.is_none());
        assert_eq!(traces[0].parameters.len(), 1);
        for e in events {
            d.dispatch(&e, Duration::ZERO, Duration::ZERO).unwrap();
        }
    }
    d.cleanup().unwrap();
    assert_eq!(d.sink.0, vec![vec![0xbf, 20, 25], vec![0xbf, 20, 25]]);
    assert_eq!(d.stats.controls_sent, 1);
    assert_eq!(d.stats.sent, 0);
}
#[test]
fn emphasis_tracks_the_moving_base_and_stop_restores_start_value_without_kit_default() {
    let mut c = system();
    let p = &mut c.parts[0];
    p.parameters.insert(
        "cutoff".into(),
        ParameterLane {
            automation: None,
            value: 0.0,
            ramp: Some(Ramp {
                to: 1.0,
                over_bars: 1,
                start_bar: 1,
            }),
        },
    );
    p.profile.controls.insert(
        "cutoff".into(),
        phasecraft::music::accent::ControlResponse {
            envelope: None,
            base: 0.9,
            boost: 0.5,
        },
    );
    let (traces, events) = resolve_step(&c, 0);
    assert!(traces[0].event.as_ref().unwrap().controls.is_empty()); // no competing legacy writer
    assert_eq!(events[0].bytes, [0xbf, 20, 64]); // current base, not the legacy profile base
    assert_eq!(events[1].bytes[0], 0x99); // controls precede note
    let release = events
        .iter()
        .find(|e| e.tick == 120 && e.parameter)
        .unwrap();
    assert_eq!(release.bytes, [0xbf, 20, 4]);
    assert_eq!(release.reset_value, None);
    let mut d = EventDispatcher::new(Recording::default());
    for e in events.iter().filter(|e| e.tick <= 80) {
        d.dispatch(e, Duration::ZERO, Duration::ZERO).unwrap();
    }
    d.cleanup().unwrap();
    assert_eq!(d.sink.0.last().unwrap(), &vec![0xbf, 20, 0]);
    // Cleanup updates the dedup cache to the actual restored value.
    d.dispatch(&events[0], Duration::ZERO, Duration::ZERO)
        .unwrap();
    assert_eq!(d.sink.0.last().unwrap(), &vec![0xbf, 20, 64]);
}
#[test]
fn timeline_changes_preserve_rhythm_and_use_actual_swung_note_boundaries() {
    let c = system();
    let mut changed = c.clone();
    changed.parts[0].parameters.get_mut("cutoff").unwrap().value = 0.0;
    changed.parts[0].profile.controls.insert(
        "cutoff".into(),
        phasecraft::music::accent::ControlResponse {
            envelope: None,
            base: 0.0,
            boost: 0.2,
        },
    );
    changed.parts[0].groove.swing = 0.75;
    changed.parts[0].groove.delay_ticks = 60;
    for step in 0..130 {
        let (a, _) = resolve_step(&c, step);
        let (b, events) = resolve_step(&changed, step);
        assert_eq!(
            serde_json::to_string(&a[0].trigger).unwrap(),
            serde_json::to_string(&b[0].trigger).unwrap()
        );
        assert_eq!(
            serde_json::to_string(&a[0].accent).unwrap(),
            serde_json::to_string(&b[0].accent).unwrap()
        );
        let e = b[0].event.as_ref().unwrap();
        let samples = &b[0].parameters[0].samples;
        for sample in samples {
            assert_eq!(
                sample.emphasis > 0.0,
                sample.tick >= e.tick && sample.tick < e.tick + e.duration_ticks
            );
        }
        assert!(samples.iter().any(|s| s.tick == e.tick));
        assert!(samples.iter().any(|s| s.tick == e.tick + e.duration_ticks));
        assert!(events.windows(2).all(|p| p[0].tick <= p[1].tick));
        assert_eq!(events, resolve_step(&changed, step).1);
    }
}
#[test]
fn invalid_timelines_fail_and_late_samples_do_not_burst() {
    let c = system();
    for (v, to, start, bars) in [
        (f64::NAN, 1.0, 1, 8),
        (0.0, 1.1, 1, 8),
        (0.0, 1.0, 0, 8),
        (0.0, 1.0, 1, 0),
    ] {
        let mut bad = c.clone();
        bad.parts[0].parameters.insert(
            "cutoff".into(),
            ParameterLane {
                automation: None,
                value: v,
                ramp: Some(Ramp {
                    to,
                    start_bar: start,
                    over_bars: bars,
                }),
            },
        );
        assert!(bad.validate().is_err());
    }
    let mut bad = c.clone();
    bad.parts[0].output.controls.clear();
    assert!(bad.validate().is_err());
    let mut silent = c.clone();
    silent.parts[0].trigger.probability = 0.0;
    let events = resolve_step(&silent, 0).1;
    let mut d = EventDispatcher::new(Recording::default());
    for e in &events {
        d.dispatch(e, Duration::from_secs(1), Duration::ZERO)
            .unwrap();
    }
    assert!(d.sink.0.is_empty());
    d.dispatch(&events[0], Duration::ZERO, Duration::ZERO)
        .unwrap();
    assert_eq!(d.sink.0.len(), 1);
}

#[test]
fn watched_held_value_changes_at_phrase_boundary_even_with_no_notes() {
    use phasecraft::playback::transport::{PlayOptions, run_controlled};
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    use std::time::Instant;
    #[derive(Clone)]
    struct Shared(Arc<Mutex<Vec<Vec<u8>>>>);
    impl MidiOutput for Shared {
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0.lock().unwrap().push(bytes.to_vec());
            Ok(())
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("scene.toml");
    let mut c = system();
    c.tempo = 400.0;
    c.phrase_bars = 1;
    c.parts[0].trigger.probability = 0.0;
    c.parts[0].parameters.get_mut("cutoff").unwrap().ramp = None;
    std::fs::write(&path, toml::to_string(&c).unwrap()).unwrap();
    let messages = Arc::new(Mutex::new(Vec::new()));
    let sink = Shared(messages.clone());
    let file = path.clone();
    let initial = c.clone();
    let worker = std::thread::spawn(move || {
        run_controlled(
            initial,
            sink,
            PlayOptions {
                file,
                steps: Some(48),
                watch: true,
                trace: false,
                send_clock: false,
                lookahead: Duration::from_millis(100),
            },
            Arc::new(AtomicBool::new(true)),
            None,
        )
    });
    let until = Instant::now() + Duration::from_secs(5);
    while messages.lock().unwrap().is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!messages.lock().unwrap().is_empty());
    c.parts[0].parameters.get_mut("cutoff").unwrap().value = 0.7;
    std::fs::write(&path, toml::to_string(&c).unwrap()).unwrap();
    worker.join().unwrap().unwrap();
    assert_eq!(
        *messages.lock().unwrap(),
        vec![vec![0xbf, 20, 25], vec![0xbf, 20, 89], vec![0xbf, 20, 89]]
    );
}

#[test]
fn stop_restores_kit_defaults_mid_ramp_after_gate_and_before_plain_composition() {
    let mut c = system();
    c.parts[0]
        .output
        .controls
        .get_mut("cutoff")
        .unwrap()
        .default = Some(1.0);
    c.parts[0].profile.controls.insert(
        "cutoff".into(),
        phasecraft::music::accent::ControlResponse {
            envelope: None,
            base: 0.0,
            boost: 0.1,
        },
    );
    for through in [80, 200] {
        // both during emphasis and after note-off
        let mut d = EventDispatcher::new(Recording::default());
        let events = resolve_step(&c, 64).1;
        for e in events.iter().filter(|e| e.tick <= 64 * 240 + through) {
            d.dispatch(e, Duration::ZERO, Duration::ZERO).unwrap();
        }
        assert_ne!(d.sink.0.last().unwrap(), &vec![0xbf, 20, 127]);
        d.cleanup().unwrap();
        assert_eq!(d.sink.0.last().unwrap(), &vec![0xbf, 20, 127]);
        let count = d.sink.0.len();
        d.cleanup().unwrap();
        assert_eq!(d.sink.0.len(), count);
        let mut plain = c.clone();
        plain.parts[0].parameters.clear();
        plain.parts[0].profile.controls.clear();
        for e in resolve_step(&plain, 0).1 {
            d.dispatch(&e, Duration::ZERO, Duration::ZERO).unwrap();
        }
        assert!(d.sink.0[count..].iter().all(|e| e[0] & 0xf0 != 0xb0));
    }
    for invalid in [-0.1, 1.1, f64::NAN] {
        c.parts[0]
            .output
            .controls
            .get_mut("cutoff")
            .unwrap()
            .default = Some(invalid);
        assert!(c.validate().is_err());
    }
}

#[test]
fn partial_control_send_failure_still_restores_kit_default() {
    struct Failing {
        calls: Vec<Vec<u8>>,
    }
    impl MidiOutput for Failing {
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.calls.push(bytes.to_vec());
            if self.calls.len() == 1 {
                Err("partial failure".into())
            } else {
                Ok(())
            }
        }
    }
    let mut c = system();
    c.parts[0]
        .output
        .controls
        .get_mut("cutoff")
        .unwrap()
        .default = Some(1.0);
    let mut d = EventDispatcher::new(Failing { calls: vec![] });
    assert!(
        d.dispatch(&resolve_step(&c, 0).1[0], Duration::ZERO, Duration::ZERO)
            .is_err()
    );
    d.cleanup().unwrap();
    assert_eq!(d.sink.calls.last().unwrap(), &vec![0xbf, 20, 127]);
}

#[test]
fn automation_segments_curves_holds_cycles_and_delayed_start() {
    use phasecraft::music::parameter::{Automation, Curve, Segment};
    let mut lane = ParameterLane {
        value: 0.0,
        ramp: None,
        automation: Some(Automation {
            start_bar: 2,
            repeat: false,
            segments: vec![
                Segment {
                    to: 1.0,
                    over_bars: 1.0,
                    curve: Curve::Smooth,
                },
                Segment {
                    to: 0.5,
                    over_bars: 0.5,
                    curve: Curve::Hold,
                },
                Segment {
                    to: 0.0,
                    over_bars: 0.5,
                    curve: Curve::Linear,
                },
            ],
        }),
    };
    lane.validate().unwrap();
    assert_eq!(lane.at(0), 0.0);
    assert_eq!(lane.at(3840), 0.0);
    assert_eq!(lane.at(3840 + 960), 0.15625);
    assert_eq!(lane.at(7680), 1.0);
    assert_eq!(lane.at(9599), 1.0);
    assert_eq!(lane.at(9600), 0.5);
    assert_eq!(lane.at(10560), 0.25);
    assert_eq!(lane.at(11520), 0.0);
    assert_eq!(lane.at(u64::MAX), 0.0);
    lane.automation.as_mut().unwrap().repeat = true;
    for t in 0..7680 {
        assert_eq!(lane.at(3840 + t), lane.at(3840 + t + 7680));
    }
    assert!(lane.at(u64::MAX).is_finite());
}

#[test]
fn automation_is_validated_and_overrides_a_reusable_ramp() {
    let source = r#"
    tempo=132
    seed=1
    [library.behaviors."my.old".parameters.cutoff]
    value=0.2
    ramp={to=1.0,over_bars=8}
    [parts.hat]
    compose=["techno.closed_hat","my.old"]
    output.controls.cutoff={cc=75,channel=15,default=1.0}
    parameters.cutoff.automation={repeat=true,segments=[{to=1.0,over_bars=1.0},{to=0.2,over_bars=1.0}]}
    "#;
    let c = Composition::parse(source).unwrap();
    let lane = &c.parts[0].parameters["cutoff"];
    assert!(lane.ramp.is_none());
    assert_eq!(lane.at(3840), 1.0);
    for invalid in [
        source.replace("over_bars=1.0", "over_bars=0.0"),
        source.replace("over_bars=1.0", "over_bars=0.1"),
        source.replace("over_bars=1.0", "over_bars=65536.0"),
        source.replace("repeat=true", "start_bar=0"),
        source.replace(
            "segments=[{to=1.0,over_bars=1.0},{to=0.2,over_bars=1.0}]",
            "segments=[]",
        ),
    ] {
        assert!(Composition::parse(&invalid).is_err());
    }
    let mut lane = lane.clone();
    lane.ramp = Some(Ramp {
        to: 1.0,
        over_bars: 1,
        start_bar: 1,
    });
    assert!(lane.validate().is_err());
    let mut d = EventDispatcher::new(Recording::default());
    for e in resolve_step(&c, 8).1 {
        d.dispatch(&e, Duration::ZERO, Duration::ZERO).unwrap();
    }
    d.cleanup().unwrap();
    assert_eq!(d.sink.0.last().unwrap(), &vec![0xbe, 75, 127]);
    let mut tempo = c.clone();
    tempo.tempo = 172.0;
    assert_eq!(resolve_step(&c, 8).1, resolve_step(&tempo, 8).1);
    let trace = resolve_step(&c, 40).0;
    let position = trace[0].parameters[0].samples[0]
        .automation
        .as_ref()
        .unwrap();
    assert_eq!(position.cycle, 1);
    assert_eq!(position.segment, 1);
    assert_eq!(position.progress, 0.5);
}
