use phasecraft::{
    music::{Composition, ProbabilityMode},
    music::{resolve::*, rhythm::*},
};
fn config() -> Composition {
    Composition::parse(include_str!("../examples/quickstart/hat.toml")).unwrap()
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
        .map(|s| {
            if e.evaluate(s, 64, &|_, _| unreachable!()).active() {
                'x'
            } else {
                '.'
            }
        })
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
                .filter(|s| {
                    e.evaluate(u64::from(*s), 64, &|_, _| unreachable!())
                        .active()
                })
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
        .map(|s| expression.evaluate(s, 64, &|_, _| unreachable!()).active())
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
    assert!(!continuing.evaluate(64, 64, &|_, _| unreachable!()).active());
    assert!(resetting.evaluate(64, 64, &|_, _| unreachable!()).active());
    let mut c = config();
    c.parts[0].trigger.rhythm = continuing;
    assert_eq!(resolve(&c, 0).trigger.roll, resolve(&c, 64).trigger.roll);
    assert_ne!(
        resolve(&c, 0).trigger.rhythm.active(),
        resolve(&c, 64).trigger.rhythm.active()
    );
    c.parts[0].trigger.probability_mode = ProbabilityMode::Continuous;
    assert_ne!(resolve(&c, 0).trigger.roll, resolve(&c, 64).trigger.roll);
}
#[test]
fn deterministic_replay_and_accent_changes_leave_trigger_intact() {
    let c = config();
    let mut other = c.clone();
    other.parts[0].accent.probability = 0.1;
    other.parts[0].accent.rhythm = euclid(11, 9, 3, true);
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
    c.parts[0].trigger.rhythm = euclid(1, 1, 0, false);
    c.parts[0].accent.rhythm = euclid(1, 1, 0, false);
    c.parts[0].accent.probability = 1.0;
    c.parts[0].trigger.probability = 0.0;
    for step in 0..128 {
        let trace = resolve(&c, step);
        assert!(trace.accent.admitted);
        assert!(trace.event.is_none());
    }
    c.parts[0].trigger.probability = 1.0;
    for step in 0..128 {
        assert!(resolve(&c, step).event.unwrap().accent.active);
    }
    c.parts[0].accent.probability = 0.0;
    for step in 0..128 {
        assert!(!resolve(&c, step).event.unwrap().accent.active);
    }
}
#[test]
fn realization_and_midi_preserve_musical_times_and_emphasis() {
    let mut c = config();
    c.parts[0].trigger.rhythm = euclid(1, 1, 0, false);
    c.parts[0].trigger.probability = 1.0;
    c.parts[0].accent.rhythm = euclid(1, 1, 0, false);
    c.parts[0].accent.probability = 1.0;
    let p = realize(&c, &c.parts[0], 4, 2);
    assert_eq!((p.start_tick, p.end_tick, p.events.len()), (960, 1440, 2));
    assert_eq!(p.events[0].accent.amount, 0.8);
    assert_eq!(
        to_midi(&c.parts[0], &p.events[0]),
        [
            MidiEvent {
                stop_value: None,
                reset_value: None,
                boundary_reset: false,
                parameter: false,
                tick: 960,
                bytes: [0x99, 42, 108]
            },
            MidiEvent {
                stop_value: None,
                reset_value: None,
                boundary_reset: false,
                parameter: false,
                tick: 1080,
                bytes: [0x89, 42, 0]
            }
        ]
    );
}
#[test]
fn malformed_configs_fail_before_playback() {
    let source = include_str!("../examples/quickstart/hat.toml");
    for text in [
        source.replace("tempo = 132", "tempo = nan"),
        source.replace("steps = 16", "steps = 0"),
        source.replace("pulses = 7", "pulses = 17"),
        source.replace("probability = 0.85", "probability = 1.5"),
        source.replace("channel = 10", "channel = 0"),
        source.replace("note = 42", "note = 128"),
        source.replace("amount = 0.8", "amount = nan"),
        source.replace("probability = 0.85", "probabilty = 0.85"),
        source.replace("note = 42", "note = 42\ngate_ticks = 5761"),
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

// Existing one-Part invariants continue to exercise the same resolver.
fn resolve(c: &Composition, step: u64) -> StepTrace {
    phasecraft::music::resolve::resolve(c, &c.parts[0], step)
}

// Pins: a forced draw at one address, consulted before the hash; everything else unchanged.
fn pinned(part: &str, pins: &str) -> Result<Composition, String> {
    Composition::parse(&format!(
        "tempo=132\nseed=91827\nphrase_bars=4\n[parts.hat]\nuse='techno.closed_hat'\n{part}\n{pins}"
    ))
}
const HAT: &str = "trigger.rhythm={steps=16,pulses=7}\ntrigger.probability=0.85\ntrigger.probability_mode='phrase_locked'\naccent.rhythm={steps=7,pulses=3}\naccent.probability=0.75";
#[test]
fn a_pin_forces_one_draw_and_leaves_every_other_draw_alone() {
    let plain = pinned(HAT, "").unwrap();
    // The first written hit in bar 2 (steps 16..32), named as bar 2 slot n; phrase-locked,
    // so the same dice is rolled again every 64 steps.
    let target = (16..32)
        .find(|s| resolve(&plain, *s).trigger.rhythm.active())
        .unwrap();
    let c = pinned(
        HAT,
        &format!(
            "[[pins]]\nat={{roll='fire',voice='hat',bar=2,slot={}}}\nu=0.99",
            target - 16 + 1
        ),
    )
    .unwrap();
    assert_eq!(c.pins.len(), 1);
    let mut hits = 0;
    for step in 0..640 {
        let a = resolve(&plain, step);
        let b = resolve(&c, step);
        if step % 64 == target {
            hits += 1;
            assert!(b.trigger.pinned, "step {step}");
            assert_eq!(b.trigger.roll, 0.99);
            assert!(!b.trigger.admitted, "0.99 never clears an 0.85 gate");
            assert_ne!(a.trigger.roll, 0.99);
            // The pin reaches one dice: the accent draw at the same step is untouched.
            assert_eq!(
                serde_json::to_string(&a.accent).unwrap(),
                serde_json::to_string(&b.accent).unwrap()
            );
            assert!(!serde_json::to_string(&a).unwrap().contains("pinned"));
        } else {
            assert_eq!(
                serde_json::to_string(&a).unwrap(),
                serde_json::to_string(&b).unwrap(),
                "step {step}"
            );
        }
    }
    assert_eq!(hits, 10);
}
#[test]
fn a_continuous_pin_lands_on_one_step_and_survives_a_round_trip() {
    let part = HAT.replace("'phrase_locked'", "'continuous'");
    let c = pinned(
        &part,
        "[[pins]]\nat={roll='fire',voice='hat',bar=2,slot=3}\nu=0.0",
    )
    .unwrap();
    for step in 0..640 {
        let t = resolve(&c, step);
        assert_eq!(t.trigger.pinned, step == 18, "step {step}");
        if step == 18 {
            assert_eq!(t.trigger.roll, 0.0);
            // u = 0 clears any gate; the written rhythm still decides whether there is a hit.
            assert_eq!(t.trigger.admitted, t.trigger.rhythm.active());
        }
    }
    // The expanded snapshot carries the shape, not the resolved address; reading it back
    // resolves again to the same dice.
    let text = toml::to_string(&c).unwrap();
    assert!(text.contains("[[pins]]"));
    assert!(!text.contains("address"));
    let again = Composition::parse(&text).unwrap();
    assert_eq!(again.pins, c.pins);
    assert_eq!(
        serde_json::to_string(&resolve(&again, 18)).unwrap(),
        serde_json::to_string(&resolve(&c, 18)).unwrap()
    );
}
#[test]
fn pins_reach_shared_accents_ratchets_and_a_subdivided_grid() {
    let c = Composition::parse(
        "tempo=132\nseed=99\nphrase_bars=4\n[accents.drums]\nrhythm={steps=1,pulses=1}\nprobability=0.5\namount=0.7\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.rhythm={steps=1,pulses=1}\ntrigger.probability=1.0\naccent.sources=['drums']\naccent.probability=0.0\nornaments.ratchet={count=3,probability=0.4}\n[[pins]]\nat={roll='accent',accent='drums',bar=1,slot=1}\nu=0.0\n[[pins]]\nat={roll='burst',voice='hat',bar=1,slot=2}\nu=0.0\n[[pins]]\nat={roll='burst',voice='hat',bar=1,slot=3}\nu=0.95",
    )
    .unwrap();
    for step in [0, 64] {
        let t = resolve(&c, step);
        assert!(t.shared_accents[0].decision.pinned);
        assert_eq!(t.shared_accents[0].decision.roll, 0.0);
        assert!(t.shared_accents[0].decision.admitted);
    }
    assert!(!resolve(&c, 1).shared_accents[0].decision.pinned);
    let forced = resolve(&c, 1).ornaments.unwrap();
    assert_eq!((forced.ratchet_roll, forced.ratchet_count), (Some(0.0), 3));
    let denied = resolve(&c, 2).ornaments.unwrap();
    assert_eq!((denied.ratchet_roll, denied.ratchet_count), (Some(0.95), 1));
    // A triplet-sixteenth Part has 24 slots per bar; slot 24 is its step 23.
    let triplet = format!("subdivision='1/16T'\n{HAT}");
    assert!(
        pinned(
            &triplet,
            "[[pins]]\nat={roll='fire',voice='hat',bar=1,slot=25}\nu=0.5"
        )
        .unwrap_err()
        .contains("24 slots per bar")
    );
    let c = pinned(
        &triplet,
        "[[pins]]\nat={roll='fire',voice='hat',bar=1,slot=24}\nu=0.5",
    )
    .unwrap();
    // resolve_step is indexed by sixteenth; find the triplet cell by its own tick.
    let cell = c.parts[0].subdivision.0;
    let at = |tick: u64| {
        (0..16)
            .flat_map(|s| resolve_step(&c, s).0)
            .find(|t| t.tick == tick)
            .unwrap()
    };
    assert!(at(23 * cell).trigger.pinned);
    assert!(!at(22 * cell).trigger.pinned);
}
#[test]
fn pins_that_name_no_draw_are_rejected_by_shape() {
    for (pins, expected) in [
        (
            "at={roll='door',voice='hat',bar=1,slot=1}\nu=0.5",
            "unknown dice",
        ),
        (
            "at={roll='fire',voice='kick',bar=1,slot=1}\nu=0.5",
            "no Part \"kick\"",
        ),
        (
            "at={roll='fire',voice='hat',bar=1,slot=17}\nu=0.5",
            "16 slots per bar",
        ),
        (
            "at={roll='fire',voice='hat',bar=0,slot=1}\nu=0.5",
            "bar is 1-based",
        ),
        (
            "at={roll='fire',voice='hat',bar=1,slot=1}\nu=1.0",
            "0 <= u < 1",
        ),
        (
            "at={roll='fire',voice='hat',bar=1,slot=1}\nu=nan",
            "0 <= u < 1",
        ),
        (
            "at={roll='burst',voice='hat',bar=1,slot=1}\nu=0.5",
            "no ratchet",
        ),
        (
            "at={roll='ghost',voice='hat',bar=1,slot=1}\nu=0.5",
            "no groove",
        ),
        (
            "at={roll='fire',voice='hat',accent='x',bar=1,slot=1}\nu=0.5",
            "exactly one owner",
        ),
        ("at={roll='fire',bar=1,slot=1}\nu=0.5", "exactly one owner"),
        (
            "at={roll='accent',accent='drums',bar=1,slot=1}\nu=0.5",
            "no shared accent",
        ),
        (
            "at={roll='fire',voice='hat',bar=1,slot=1,tick=4}\nu=0.5",
            "unknown field `tick`",
        ),
        // Two shapes, one dice: bar 5 slot 3 is bar 1 slot 3 again under a four-bar phrase lock.
        (
            "at={roll='fire',voice='hat',bar=1,slot=3}\nu=0.5\n[[pins]]\nat={roll='fire',voice='hat',bar=5,slot=3}\nu=0.6",
            "names the same dice",
        ),
    ] {
        let err = pinned(HAT, &format!("[[pins]]\n{pins}")).unwrap_err();
        assert!(err.contains(expected), "{pins}: {err}");
    }
    // The same two shapes are two dice once the lane is continuous.
    let part = HAT.replace("'phrase_locked'", "'continuous'");
    assert!(pinned(&part, "[[pins]]\nat={roll='fire',voice='hat',bar=1,slot=3}\nu=0.5\n[[pins]]\nat={roll='fire',voice='hat',bar=5,slot=3}\nu=0.6").is_ok());
}
