use phasecraft::music::{Composition, resolve::resolve_step};
use phasecraft::playback::{EventDispatcher, MidiOutput};
use std::time::Duration;

fn system() -> Composition {
    Composition::parse(
        r#"
        tempo=132
        seed=303
        [parts.hat]
        use="techno.closed_hat"
        trigger.rhythm={steps=1,pulses=1}
        accent.rhythm={steps=2,pulses=1}
        accent.probability=1.0
        accent.amount=0.5
        profile.use="accent.filter_punch"
        output.controls.filter={cc=20,channel=16}
        output.controls.envelope={cc=21,channel=16}
    "#,
    )
    .unwrap()
}
#[derive(Default)]
struct Recording {
    messages: Vec<Vec<u8>>,
    fail_once: bool,
}
impl MidiOutput for Recording {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.messages.push(bytes.to_vec());
        if std::mem::take(&mut self.fail_once) {
            Err("disconnected".into())
        } else {
            Ok(())
        }
    }
}
#[test]
fn emphasis_drives_multiple_controls_before_attack_and_resets_after_release() {
    let c = system();
    let (traces, midi) = resolve_step(&c, 0);
    let event = traces[0].event.as_ref().unwrap();
    assert!(event.accent.active);
    assert_eq!(event.controls.len(), 2);
    assert_eq!(
        midi.iter().map(|m| m.bytes).collect::<Vec<_>>(),
        vec![
            [0xbf, 20, 67],
            [0xbf, 21, 54],
            [0x99, 42, 87],
            [0x89, 42, 0],
            [0xbf, 20, 25],
            [0xbf, 21, 19],
        ]
    );
    assert!(midi[..3].iter().all(|m| m.tick == 0));
    assert!(midi[3..].iter().all(|m| m.tick == 120));
    let mut dispatcher = EventDispatcher::new(Recording::default());
    for m in &midi {
        dispatcher
            .dispatch(m, Duration::ZERO, Duration::ZERO)
            .unwrap();
    }
    dispatcher.cleanup().unwrap();
    assert_eq!(dispatcher.sink.messages.len(), 6);
    assert_eq!(dispatcher.stats.sent, 2);
    assert_eq!(dispatcher.stats.controls_sent, 4);
    let plain = resolve_step(&c, 1).1;
    assert_eq!(plain[0].bytes, [0xbf, 20, 25]);
    assert_eq!(plain[1].bytes, [0xbf, 21, 19]);
}
#[test]
fn controls_follow_groove_and_cannot_create_hits_or_scramble_decisions() {
    let c = system();
    let mut changed = c.clone();
    changed.parts[0]
        .profile
        .controls
        .get_mut("filter")
        .unwrap()
        .boost = -1.0;
    changed.parts[0].groove.swing = 0.75;
    changed.parts[0].groove.delay_ticks = 60;
    for step in 0..560 {
        let (a, _) = resolve_step(&c, step);
        let (b, m) = resolve_step(&changed, step);
        assert_eq!(
            serde_json::to_string(&a[0].trigger).unwrap(),
            serde_json::to_string(&b[0].trigger).unwrap()
        );
        assert_eq!(
            serde_json::to_string(&a[0].accent).unwrap(),
            serde_json::to_string(&b[0].accent).unwrap()
        );
        let event = b[0].event.as_ref().unwrap();
        assert!(m[..3].iter().all(|m| m.tick == event.tick));
        assert!(
            m[3..]
                .iter()
                .all(|m| m.tick == event.tick + event.duration_ticks)
        );
        assert!(m.last().unwrap().tick < (step + 1) * 240);
        assert_eq!(m, resolve_step(&changed, step).1);
    }
    changed.parts[0].trigger.probability = 0.0;
    assert!(resolve_step(&changed, 0).1.is_empty());
}
#[test]
fn stop_errors_and_late_controls_restore_only_owned_values() {
    let midi = resolve_step(&system(), 0).1;
    let mut d = EventDispatcher::new(Recording::default());
    for m in &midi[..3] {
        d.dispatch(m, Duration::ZERO, Duration::ZERO).unwrap();
    }
    d.cleanup().unwrap();
    d.cleanup().unwrap();
    assert_eq!(
        &d.sink.messages[3..],
        &[vec![0x89, 42, 0], vec![0xbf, 20, 25], vec![0xbf, 21, 19]]
    );
    let mut d = EventDispatcher::new(Recording {
        fail_once: true,
        ..Default::default()
    });
    assert!(
        d.dispatch(&midi[0], Duration::ZERO, Duration::ZERO)
            .is_err()
    );
    d.cleanup().unwrap();
    assert_eq!(
        d.sink.messages,
        vec![vec![0xbf, 20, 67], vec![0xbf, 20, 25]]
    );
    let mut d = EventDispatcher::new(Recording::default());
    d.dispatch(&midi[0], Duration::from_secs(1), Duration::ZERO)
        .unwrap();
    d.cleanup().unwrap();
    assert!(d.sink.messages.is_empty());
    // A late reset still releases a previously sent emphasis.
    d.dispatch(&midi[0], Duration::ZERO, Duration::ZERO)
        .unwrap();
    d.dispatch(&midi[4], Duration::from_secs(1), Duration::ZERO)
        .unwrap();
    d.cleanup().unwrap();
    assert_eq!(
        d.sink.messages,
        vec![vec![0xbf, 20, 67], vec![0xbf, 20, 25]]
    );
}
#[test]
fn invalid_or_conflicting_routes_and_unbounded_responses_are_rejected() {
    let c = system();
    for cc in [0, 32, 64, 96, 120, 127] {
        let mut bad = c.clone();
        bad.parts[0].output.controls.get_mut("filter").unwrap().cc = cc;
        assert!(bad.validate().is_err());
    }
    let mut bad = c.clone();
    bad.parts[0].output.controls.remove("filter");
    assert!(bad.validate().is_err());
    let mut bad = c.clone();
    bad.parts[0].output.controls.get_mut("filter").unwrap().cc = 21;
    assert!(bad.validate().unwrap_err().contains("duplicate control"));
    let mut bad = c.clone();
    bad.parts[0]
        .output
        .controls
        .get_mut("filter")
        .unwrap()
        .channel = Some(17);
    assert!(bad.validate().is_err());
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        let mut bad = c.clone();
        bad.parts[0]
            .profile
            .controls
            .get_mut("filter")
            .unwrap()
            .base = value;
        assert!(bad.validate().is_err());
    }
    let mut bad = c.clone();
    let mut other = bad.parts[0].clone();
    other.id = "other".into();
    other.output.note = 37;
    bad.parts.push(other);
    assert!(bad.validate().unwrap_err().contains("duplicate control"));
    let mut good = c.clone();
    good.parts[0]
        .profile
        .controls
        .get_mut("filter")
        .unwrap()
        .boost = 1.0;
    good.parts[0].accent.amount = 1.0;
    good.validate().unwrap();
    assert_eq!(resolve_step(&good, 0).1[0].bytes[2], 127);
}

#[test]
fn maximum_control_load_is_bounded_and_every_attack_has_a_matching_reset() {
    use phasecraft::music::{
        MAX_PARTS,
        accent::{ControlOutput, ControlResponse, MAX_CONTROLS},
    };
    let mut c = system();
    let prototype = c.parts[0].clone();
    c.parts = (0..MAX_PARTS)
        .map(|i| {
            let mut part = prototype.clone();
            part.id = format!("part_{i}");
            part.output.note = i as u8;
            part.output.controls.clear();
            part.profile.controls.clear();
            for j in 0..MAX_CONTROLS {
                let name = format!("control_{j}");
                part.profile.controls.insert(
                    name.clone(),
                    ControlResponse {
                        envelope: None,
                        base: 0.2,
                        boost: 0.5,
                    },
                );
                part.output.controls.insert(
                    name,
                    ControlOutput {
                        default: None,
                        cc: (1 + j + (i / 16) * MAX_CONTROLS) as u8,
                        channel: Some((1 + i % 16) as u8),
                    },
                );
            }
            part
        })
        .collect();
    c.validate().unwrap();
    let events = resolve_step(&c, 0).1;
    assert_eq!(events.len(), MAX_PARTS * (2 + 2 * MAX_CONTROLS));
    let mut d = EventDispatcher::new(Recording::default());
    for event in &events {
        d.dispatch(event, Duration::ZERO, Duration::ZERO).unwrap();
    }
    d.cleanup().unwrap();
    assert_eq!(d.sink.messages.len(), events.len());
    let mut continuous = c.clone();
    for part in &mut continuous.parts {
        part.groove.swing = 0.75;
        part.groove.delay_ticks = 60;
        for name in part.output.controls.keys() {
            part.parameters.insert(
                name.clone(),
                phasecraft::music::parameter::ParameterLane {
                    automation: None,
                    value: 0.4,
                    ramp: None,
                },
            );
        }
    }
    continuous.validate().unwrap();
    let full = resolve_step(&continuous, 1).1;
    assert_eq!(
        full.len(),
        MAX_PARTS * (2 + MAX_CONTROLS * phasecraft::music::parameter::MAX_SAMPLES_PER_STEP)
    );
    assert!(full.iter().all(|e| e.tick >= 240 && e.tick < 480));
    for part in &mut continuous.parts {
        for response in part.profile.controls.values_mut() {
            response.envelope = Some(phasecraft::music::accent::Envelope {
                decay_beats: 8.0,
                accumulation: 0.5,
            });
        }
    }
    continuous.validate().unwrap();
    let (traces, events) = resolve_step(&continuous, 33);
    assert!(
        events.len()
            <= MAX_PARTS * (2 + MAX_CONTROLS * phasecraft::music::parameter::MAX_SAMPLES_PER_STEP)
    );
    assert!(traces.iter().all(|t| {
        t.parameters
            .iter()
            .all(|p| p.samples.iter().all(|s| s.envelope.is_some()))
    }));
    let p = &mut c.parts[0];
    p.profile.controls.insert(
        "excess".into(),
        ControlResponse {
            envelope: None,
            base: 0.2,
            boost: 0.5,
        },
    );
    p.output.controls.insert(
        "excess".into(),
        ControlOutput {
            default: None,
            cc: 30,
            channel: Some(16),
        },
    );
    assert!(c.validate().unwrap_err().contains("at most"));
}
