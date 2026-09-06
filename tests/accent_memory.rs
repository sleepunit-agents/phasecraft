use phasecraft::music::{Composition, accent::Envelope, resolve::resolve_step};
use phasecraft::playback::{EventDispatcher, MidiOutput};
use std::time::Duration;

fn shared() -> Composition {
    Composition::parse(
        r#"
    tempo=132
    seed=99
    phrase_bars=4
    [accents.drums]
    rhythm={steps=5,pulses=3}
    probability=0.8
    amount=0.7
    [parts.hat]
    use="techno.closed_hat"
    trigger.rhythm={steps=1,pulses=1}
    trigger.probability=1.0
    accent.sources=["drums"]
    accent.probability=0.0
    [parts.rim]
    use="techno.rim"
    trigger.rhythm={steps=1,pulses=1}
    trigger.probability=1.0
    accent.sources=["drums"]
    accent.probability=0.0
    "#,
    )
    .unwrap()
}
#[test]
fn shared_emphasis_has_one_decision_and_cannot_create_notes() {
    let mut c = shared();
    for step in 0..80 {
        let t = resolve_step(&c, step).0;
        assert_eq!(
            t[0].event.as_ref().unwrap().accent.amount,
            t[1].event.as_ref().unwrap().accent.amount
        );
        assert_eq!(
            t[0].shared_accents[0].decision.roll,
            t[1].shared_accents[0].decision.roll
        );
        assert!(!t[0].accent.admitted);
    }
    let before = resolve_step(&c, 3).0;
    c.parts[0].trigger.probability = 0.0;
    let after = resolve_step(&c, 3).0;
    assert!(after[0].event.is_none());
    assert_eq!(
        serde_json::to_value(&before[1].trigger).unwrap(),
        serde_json::to_value(&after[1].trigger).unwrap()
    );
    assert_eq!(
        before[1].shared_accents[0].decision.roll,
        after[1].shared_accents[0].decision.roll
    );
    c.parts.reverse();
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(resolve_step(&c, 3).0).unwrap()
    );
}
#[test]
fn envelope_accumulates_fades_and_has_finite_memory() {
    let e = Envelope {
        decay_beats: 1.0,
        accumulation: 0.5,
    };
    let history = [(0, 1.0), (240, 1.0)];
    assert_eq!(e.evaluate(0, &history).level, 0.5);
    assert_eq!(e.evaluate(240, &history).level, 0.875);
    assert_eq!(e.evaluate(480, &history).level, 0.625);
    assert_eq!(e.evaluate(960, &history).level, 0.125);
    assert_eq!(e.evaluate(1200, &history).level, 0.0);
    assert_eq!(e.evaluate(u64::MAX, &history).level, 0.0);
    assert_eq!(e.evaluate(240, &[(0, 1.0), (0, 1.0), (0, 1.0)]).level, 1.0);
}
fn memory() -> Composition {
    Composition::parse(
        r#"
    tempo=132
    seed=1
    phrase_bars=1
    [parts.rim]
    use="techno.rim"
    trigger.rhythm={steps=8,pulses=1,rotation=0}
    trigger.probability=1.0
    accent.rhythm={steps=1,pulses=1}
    accent.amount=1.0
    accent.probability=1.0
    output.controls.cutoff={cc=28,channel=15,default=1.0}
    profile.controls.cutoff={base=0.2,boost=0.5,envelope={decay_beats=1.0,accumulation=0.5}}
    "#,
    )
    .unwrap()
}
#[derive(Default)]
struct Recording(Vec<Vec<u8>>);
impl MidiOutput for Recording {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0.push(bytes.to_vec());
        Ok(())
    }
}
#[test]
fn memory_resolves_on_rests_survives_gate_and_stops_at_kit_default() {
    let c = memory();
    let (start, midi) = resolve_step(&c, 0);
    assert!(start[0].event.as_ref().unwrap().controls.is_empty());
    assert_eq!(midi[0].bytes, [0xbe, 28, 57]);
    let after_gate = midi.iter().find(|e| e.parameter && e.tick == 120).unwrap();
    assert!(after_gate.bytes[2] > 25); // gate does not erase an envelope
    let (rest, events) = resolve_step(&c, 1);
    assert!(rest[0].event.is_none());
    assert_eq!(
        rest[0].parameters[0].samples[0]
            .envelope
            .as_ref()
            .unwrap()
            .level,
        0.375
    );
    assert!(events.iter().all(|e| e.bytes[0] == 0xbe));
    let end = resolve_step(&c, 4).0;
    assert_eq!(end[0].parameters[0].samples[0].value, 25);
    let mut d = EventDispatcher::new(Recording::default());
    for e in events {
        d.dispatch(&e, Duration::ZERO, Duration::ZERO).unwrap();
    }
    d.cleanup().unwrap();
    assert_eq!(d.sink.0.last().unwrap(), &vec![0xbe, 28, 127]);
    let mut tempo = c.clone();
    tempo.tempo = 172.0;
    assert_eq!(resolve_step(&c, 17).1, resolve_step(&tempo, 17).1);
}
#[test]
fn memory_uses_grooved_onset_and_is_query_order_independent() {
    let mut c = memory();
    c.parts[0].groove.delay_ticks = 60;
    c.parts[0].groove.humanize = Some(phasecraft::music::groove::Humanize {
        timing_ticks: 8,
        ..Default::default()
    });
    let first = resolve_step(&c, 0).0;
    let onset = first[0].event.as_ref().unwrap().tick;
    assert_eq!(
        first[0].parameters[0].samples[0]
            .envelope
            .as_ref()
            .unwrap()
            .level,
        0.0
    );
    let expected = 0.5 * (1.0 - (240 - onset) as f64 / 960.0);
    assert_eq!(
        resolve_step(&c, 1).0[0].parameters[0].samples[0]
            .envelope
            .as_ref()
            .unwrap()
            .level,
        expected
    );
    let baseline: Vec<_> = (0..80).map(|s| resolve_step(&c, s).1).collect();
    for s in (0..80).rev() {
        assert_eq!(baseline[s], resolve_step(&c, s as u64).1);
    }
    let mut edited = c.clone();
    edited.parts[0]
        .profile
        .controls
        .get_mut("cutoff")
        .unwrap()
        .envelope
        .as_mut()
        .unwrap()
        .decay_beats = 2.0;
    for step in 0..20 {
        let a = resolve_step(&c, step).0;
        let b = resolve_step(&edited, step).0;
        assert_eq!(
            serde_json::to_value(&a[0].trigger).unwrap(),
            serde_json::to_value(&b[0].trigger).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&a[0].accent).unwrap(),
            serde_json::to_value(&b[0].accent).unwrap()
        );
    }
}
#[test]
fn invalid_shared_sources_and_envelopes_fail_before_playback() {
    let mut c = shared();
    c.parts[0].accent.sources.push("missing".into());
    assert!(c.validate().is_err());
    let mut c = shared();
    c.parts[0].accent.sources.push("drums".into());
    assert!(c.validate().is_err());
    let mut c = shared();
    c.accents
        .get_mut("drums")
        .unwrap()
        .sources
        .push("drums".into());
    assert!(c.validate().is_err());
    for beats in [0.0, 0.1, 8.25, f64::NAN] {
        let mut c = memory();
        c.parts[0]
            .profile
            .controls
            .get_mut("cutoff")
            .unwrap()
            .envelope
            .as_mut()
            .unwrap()
            .decay_beats = beats;
        assert!(c.validate().is_err());
    }
}
