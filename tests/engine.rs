use phasecraft::{
    config::{Composition, ProbabilityMode},
    engine::*,
};
fn config() -> Composition {
    Composition::parse(include_str!("../examples/hat.toml")).unwrap()
}
fn euclid(steps: u32, pulses: u32, rotation: i32, reset_on_phrase: bool) -> Expression {
    Expression::Euclidean {
        steps,
        pulses,
        rotation,
        reset_on_phrase,
    }
}
fn pattern(e: &Expression, length: u64) -> String {
    (0..length)
        .map(|s| if e.evaluate(s, 64).active() { 'x' } else { '.' })
        .collect()
}

#[test]
fn known_euclidean_patterns_and_rotation() {
    assert_eq!(pattern(&euclid(8, 3, 0, false), 8), "x..x..x.");
    assert_eq!(pattern(&euclid(16, 4, 0, false), 16), "x...x...x...x...");
    assert_eq!(pattern(&euclid(8, 3, 1, false), 8), ".x..x..x");
    assert_eq!(pattern(&euclid(8, 3, -1, false), 8), "..x..x.x");
    assert_eq!(pattern(&euclid(5, 0, 0, false), 5), ".....");
    assert_eq!(pattern(&euclid(5, 5, 0, false), 5), "xxxxx");
}
#[test]
fn all_small_euclidean_cycles_have_exact_pulse_counts_and_balanced_gaps() {
    for steps in 1..=64 {
        for pulses in 0..=steps {
            let e = euclid(steps, pulses, 0, false);
            let hits: Vec<u32> = (0..steps)
                .filter(|s| e.evaluate(u64::from(*s), 64).active())
                .collect();
            assert_eq!(hits.len(), pulses as usize);
            if pulses > 0 {
                let gaps: Vec<u32> = (0..hits.len())
                    .map(|i| {
                        if i + 1 == hits.len() {
                            steps + hits[0] - hits[i]
                        } else {
                            hits[i + 1] - hits[i]
                        }
                    })
                    .collect();
                assert!(gaps.iter().max().unwrap() - gaps.iter().min().unwrap() <= 1);
            }
        }
    }
}
#[test]
fn boolean_truth_tables() {
    for (a, b, expected) in [
        (false, false, [false, false, false, false, false]),
        (false, true, [true, false, true, false, true]),
        (true, false, [true, false, true, true, false]),
        (true, true, [true, true, false, false, false]),
    ] {
        for (op, result) in [
            BooleanOp::Or,
            BooleanOp::And,
            BooleanOp::Xor,
            BooleanOp::ANotB,
            BooleanOp::BNotA,
        ]
        .into_iter()
        .zip(expected)
        {
            assert_eq!(op.apply(a, b), result);
        }
    }
}
#[test]
fn nested_expressions_and_independent_cycles() {
    let expression = Expression::Binary {
        op: BooleanOp::And,
        a: Box::new(Expression::Binary {
            op: BooleanOp::Xor,
            a: Box::new(euclid(16, 7, 0, false)),
            b: Box::new(euclid(5, 2, 1, false)),
        }),
        b: Box::new(euclid(7, 3, 0, false)),
    };
    let values: Vec<_> = (0..1120)
        .map(|s| expression.evaluate(s, 64).active())
        .collect();
    assert_eq!(values[..560], values[560..]);
    for period in [5, 7, 16, 64, 80, 112, 280] {
        assert!((0..560).any(|s| values[s] != values[s + period]));
    }
}
#[test]
fn reset_policy_is_per_leaf_and_separate_from_probability_identity() {
    let continuing = euclid(5, 2, 0, false);
    let resetting = euclid(5, 2, 0, true);
    assert!(!continuing.evaluate(64, 64).active());
    assert!(resetting.evaluate(64, 64).active());
    let mut c = config();
    c.part.trigger.rhythm = continuing;
    assert_eq!(resolve(&c, 0).trigger.roll, resolve(&c, 64).trigger.roll);
    assert_ne!(
        resolve(&c, 0).trigger.rhythm.active(),
        resolve(&c, 64).trigger.rhythm.active()
    );
    c.part.trigger.probability_mode = ProbabilityMode::Continuous;
    assert_ne!(resolve(&c, 0).trigger.roll, resolve(&c, 64).trigger.roll);
}
#[test]
fn deterministic_replay_and_accent_changes_leave_trigger_intact() {
    let c = config();
    let mut other = c.clone();
    other.part.accent.probability = 0.1;
    other.part.accent.rhythm = euclid(11, 9, 3, true);
    for step in 0..5600 {
        let a = resolve(&c, step);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&resolve(&c, step)).unwrap()
        );
        assert_eq!(
            serde_json::to_string(&a.trigger).unwrap(),
            serde_json::to_string(&resolve(&other, step).trigger).unwrap()
        );
    }
}
#[test]
fn decision_addresses_are_isolated_and_unambiguous() {
    let baseline = decision_roll(1, "hat", "trigger", 3, "admission");
    for roll in [
        decision_roll(2, "hat", "trigger", 3, "admission"),
        decision_roll(1, "perc", "trigger", 3, "admission"),
        decision_roll(1, "hat", "accent", 3, "admission"),
        decision_roll(1, "hat", "trigger", 4, "admission"),
        decision_roll(1, "hat", "trigger", 3, "tie"),
    ] {
        assert_ne!(baseline, roll);
        assert!((0.0..1.0).contains(&roll));
    }
    assert_ne!(
        decision_roll(1, "ab", "c", 0, "d"),
        decision_roll(1, "a", "bc", 0, "d")
    );
}
#[test]
fn accent_never_creates_notes_and_probability_endpoints_are_exact() {
    let mut c = config();
    c.part.trigger.rhythm = euclid(1, 1, 0, false);
    c.part.accent.rhythm = euclid(1, 1, 0, false);
    c.part.accent.probability = 1.0;
    c.part.trigger.probability = 0.0;
    for step in 0..128 {
        let trace = resolve(&c, step);
        assert!(trace.accent.admitted);
        assert!(trace.event.is_none());
    }
    c.part.trigger.probability = 1.0;
    for step in 0..128 {
        assert!(resolve(&c, step).event.unwrap().accent.active);
    }
    c.part.accent.probability = 0.0;
    for step in 0..128 {
        assert!(!resolve(&c, step).event.unwrap().accent.active);
    }
}
#[test]
fn realization_and_midi_preserve_musical_times_and_emphasis() {
    let mut c = config();
    c.part.trigger.rhythm = euclid(1, 1, 0, false);
    c.part.trigger.probability = 1.0;
    c.part.accent.rhythm = euclid(1, 1, 0, false);
    c.part.accent.probability = 1.0;
    let p = realize(&c, 4, 2);
    assert_eq!((p.start_tick, p.end_tick, p.events.len()), (960, 1440, 2));
    assert_eq!(p.events[0].accent.amount, 0.8);
    assert_eq!(
        to_midi(&c, &p.events[0]),
        [
            MidiEvent {
                tick: 960,
                bytes: [0x99, 42, 108]
            },
            MidiEvent {
                tick: 1080,
                bytes: [0x89, 42, 0]
            }
        ]
    );
}
#[test]
fn malformed_configs_fail_before_playback() {
    let source = include_str!("../examples/hat.toml");
    for text in [
        source.replace("tempo = 132", "tempo = nan"),
        source.replace("steps = 16", "steps = 0"),
        source.replace("pulses = 7", "pulses = 17"),
        source.replace("probability = 0.85", "probability = 1.5"),
        source.replace("channel = 10", "channel = 0"),
        source.replace("note = 42", "note = 128"),
        source.replace("amount = 0.8", "amount = nan"),
        source.replace("probability = 0.85", "probabilty = 0.85"),
        source.replace("note = 42", "note = 42\ngate_ticks = 240"),
    ] {
        assert!(Composition::parse(&text).is_err(), "accepted: {text}");
    }
}

#[test]
fn decision_v1_golden_value_survives_build_and_platform_changes() {
    assert_eq!(
        decision_roll(1, "hat", "trigger", 3, "admission"),
        0.5363229830181838
    );
}
