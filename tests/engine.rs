use phasecraft::{
    music::time::NoteValue,
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
    // The gate's own record says the roll was authored: a pinned refusal is still refused by
    // probability, and the flag is what separates that from chance. Unpinned records omit it.
    use phasecraft::music::ornament::SuppressionReason::Probability;
    let forced_gate = forced.ratchet.as_ref().unwrap();
    assert!(forced_gate.pinned && forced_gate.admitted_count == 3);
    let denied_gate = denied.ratchet.as_ref().unwrap();
    assert!(denied_gate.pinned && denied_gate.suppression_reason == Some(Probability));
    assert!(
        serde_json::to_string(&denied)
            .unwrap()
            .contains("\"pinned\":true")
    );
    let free = resolve(&c, 3).ornaments.unwrap();
    assert!(!free.ratchet.as_ref().unwrap().pinned);
    assert!(!serde_json::to_string(&free).unwrap().contains("pinned"));
    // A triplet-sixteenth Part has 24 slots per bar; slot 24 is its step 23.
    let triplet = format!("subdivision='1/16T'\n{HAT}");
    assert!(
        pinned(
            &triplet,
            "[[pins]]\nat={roll='fire',voice='hat',bar=1,slot=25}\nu=0.5"
        )
        .unwrap_err()
        .contains("24 slots in bar 1")
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
            "16 slots in bar 1",
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

// A grid whose cell does not divide the bar: `1/8.` is 720 ticks, so bar 1 holds six onsets
// (0..3600) and bar 2 holds five, the first of them at tick 4320 — not on the bar line.
const DOTTED: &str = "subdivision='1/8.'\ntrigger.rhythm={steps=1,pulses=1}\ntrigger.probability=1.0\ntrigger.probability_mode='continuous'\naccent.probability=0.0";
#[test]
fn a_pin_on_a_dotted_grid_is_counted_inside_the_bar_it_names() {
    // `resolve_step` is indexed by sixteenth; a 720-tick cell fires on its own ticks, so the
    // addresses are compared where they are audible.
    fn pinned_ticks(c: &Composition, sixteenths: u64) -> Vec<u64> {
        let mut ticks: Vec<u64> = (0..sixteenths)
            .flat_map(|s| resolve_step(c, s).0)
            .filter(|t| t.trigger.pinned)
            .map(|t| t.tick)
            .collect();
        ticks.dedup();
        ticks
    }
    let c = pinned(
        DOTTED,
        "[[pins]]\nat={roll='fire',voice='hat',bar=2,slot=1}\nu=0.123",
    )
    .unwrap();
    assert_eq!(c.parts[0].subdivision.0, 720);
    // Bar 2 starts at tick 3840 and its first onset is 4320. Multiplying a truncated
    // steps-per-bar (3840/720 = 5) put the address at tick 3600 — inside bar 1.
    assert_eq!(pinned_ticks(&c, 24), vec![4320]);
    // Bar 1 really does hold a sixth onset, at 3600; the truncated count rejected it.
    let sixth = pinned(
        DOTTED,
        "[[pins]]\nat={roll='fire',voice='hat',bar=1,slot=6}\nu=0.123",
    )
    .unwrap();
    assert_eq!(pinned_ticks(&sixth, 24), vec![3600]);
    // Bar 2 holds five, not six: the error names the count for the bar it was asked about.
    let err = pinned(
        DOTTED,
        "[[pins]]\nat={roll='fire',voice='hat',bar=2,slot=6}\nu=0.123",
    )
    .unwrap_err();
    assert!(err.contains("5 slots in bar 2"), "{err}");
    // Dividing grids are unchanged: sixteen slots in every bar, each bar on the bar line.
    let straight = pinned(
        &HAT.replace("'phrase_locked'", "'continuous'"),
        "[[pins]]\nat={roll='fire',voice='hat',bar=3,slot=4}\nu=0.5",
    )
    .unwrap();
    assert_eq!(pinned_ticks(&straight, 64), vec![35 * 240]);
}
// The one supported cell longer than a bar: `1/1.` is 5760 ticks against a 3840-tick bar, so
// its onsets fall at 0, 5760, 11520, ... and every third bar holds none at all.
const DOTTED_WHOLE: &str = "subdivision='1/1.'\ntrigger.rhythm={steps=1,pulses=1}";
#[test]
fn a_bar_the_grid_steps_over_is_named_not_panicked_on() {
    // Counting slots down from the last inclusive index — `(bar_end - 1) / cell - first + 1` —
    // underflows here: bar 3 spans [7680, 11520), `first` is 2, and the last index is 1. Under
    // overflow checks that panics before the invalid-slot error can name the empty bar.
    assert_eq!(NoteValue::parse("1/1.").unwrap().0, 5760);
    for bar in [3, 6] {
        let err = pinned(
            DOTTED_WHOLE,
            &format!("[[pins]]\nat={{roll='fire',voice='hat',bar={bar},slot=1}}\nu=0.123"),
        )
        .unwrap_err();
        assert!(err.contains(&format!("no onset in bar {bar}")), "{err}");
        // The error still carries the pin's shape, like every other refusal on this path.
        assert!(err.contains("roll = \"fire\""), "{err}");
    }
    // The bars that do hold their one onset are unaffected, and slot 2 is refused by count.
    for bar in [1, 2, 4, 5, 7] {
        let at = |slot| {
            format!("[[pins]]\nat={{roll='fire',voice='hat',bar={bar},slot={slot}}}\nu=0.123")
        };
        assert!(pinned(DOTTED_WHOLE, &at(1)).is_ok(), "bar {bar} slot 1");
        let err = pinned(DOTTED_WHOLE, &at(2)).unwrap_err();
        assert!(err.contains(&format!("1 slots in bar {bar}")), "{err}");
    }
    // Every supported grid, over two phrases of bars, at the slots where a count is decided:
    // the first, the last, and the one past it. No shape panics, and a pin is accepted exactly
    // where the bar it names really holds that slot — including when it holds none.
    //
    // The expected count is the grid's onsets in the bar, enumerated tick by tick — not the
    // engine's closed form (`end.div_ceil(cell) - start.div_ceil(cell)`). Restating that form
    // here would still catch the engine drifting away from it; what it could not catch is the
    // form being wrong in both places at once. Test and engine were written in one commit from
    // one derivation, so that is the failure this sweep was open to. Enumeration owes the
    // derivation nothing.
    for cell in [1, 2, 4, 8, 16, 32, 64]
        .iter()
        .flat_map(|d| ["", "T", "."].iter().map(move |s| format!("1/{d}{s}")))
    {
        let part = format!("subdivision='{cell}'\ntrigger.rhythm={{steps=1,pulses=1}}");
        let ticks = NoteValue::parse(&cell).unwrap().0;
        for bar in 1..=8u64 {
            let start = (bar - 1) * 3840;
            let slots = (start..start + 3840).filter(|t| t % ticks == 0).count() as u64;
            for slot in [1, slots.max(1), slots + 1] {
                let pin = format!(
                    "[[pins]]\nat={{roll='fire',voice='hat',bar={bar},slot={slot}}}\nu=0.123"
                );
                assert_eq!(
                    pinned(&part, &pin).is_ok(),
                    slot <= slots,
                    "{cell} bar {bar} slot {slot} of {slots}"
                );
            }
        }
    }
}
#[test]
fn the_pin_cap_is_enforced_on_the_authored_list() {
    // `Composition::validate` runs before pins are resolved, so its own copy of this bound never
    // sees the file's pins: without a check on the authored list, parse accepted 257 and the
    // returned composition then failed the validate it had already passed.
    let part = HAT.replace("'phrase_locked'", "'continuous'");
    let pins = |n: usize| {
        (0..n)
            .map(|i| {
                format!(
                    "[[pins]]\nat={{roll='fire',voice='hat',bar={},slot={}}}\nu=0.5",
                    i / 16 + 1,
                    i % 16 + 1
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let ok = pinned(&part, &pins(256)).unwrap();
    assert_eq!(ok.pins.len(), 256);
    assert!(ok.validate().is_ok());
    let err = pinned(&part, &pins(257)).unwrap_err();
    assert!(err.contains("at most 256 pins are supported"), "{err}");
}
#[test]
fn a_velocity_pin_is_admitted_wherever_the_touch_closure_draws() {
    // The closure that draws `humanize_velocity` opens on offbeat gain, a gap response OR
    // humanize; the loader used to accept only the third and refused a pin at an address the
    // engine really does roll. A forced draw may be neutral — with no `humanize` the velocity
    // factor is 1.0 — but the die is rolled, and a pin names the die.
    for groove in [
        "groove.offbeat_gain=1.1",
        "groove.after_gap={steps=2,gain=1.2}",
        "groove.humanize={timing_ticks=6,velocity=0.2}",
    ] {
        let part = format!("{DENSE}\n{groove}");
        for roll in ["velocity", "timing"] {
            let c = pinned(
                &part,
                &format!("[[pins]]\nat={{roll='{roll}',voice='hat',bar=1,slot=1}}\nu=0.75"),
            )
            .unwrap_or_else(|e| panic!("{groove} / {roll}: {e}"));
            let touch = |step: u64| {
                resolve_step(&c, step).0[0]
                    .event
                    .as_ref()
                    .unwrap()
                    .groove
                    .as_ref()
                    .unwrap()
                    .touch
                    .clone()
                    .unwrap()
            };
            let (at_pin, elsewhere) = (touch(0), touch(1));
            let (drawn, other) = match roll {
                "velocity" => (at_pin.velocity_roll, elsewhere.velocity_roll),
                _ => (at_pin.timing_roll, elsewhere.timing_roll),
            };
            assert_eq!(drawn, 0.75, "{groove} / {roll} at the pinned address");
            assert_ne!(other, 0.75, "{groove} / {roll} one step later");
        }
    }
    // A groove that opens nothing still refuses the pin, and now names what would open it.
    let err = pinned(
        DENSE,
        "[[pins]]\nat={roll='velocity',voice='hat',bar=1,slot=1}\nu=0.75",
    )
    .unwrap_err();
    assert!(err.contains("groove.offbeat_gain"), "{err}");
}
const DENSE: &str = "trigger.rhythm={steps=1,pulses=1}\ntrigger.probability=1.0\ntrigger.probability_mode='continuous'\naccent.probability=0.0";
#[test]
fn a_continuous_pin_lands_once_per_pass_of_its_step_on_the_clock_it_is_counted_against() {
    // `continuous` means the address is the step index, and a section with phase = "restart"
    // restarts that index: the same pin is consulted again at the head of every restarting
    // section. Under phase = "continue" the clock is transport-relative and the pin is passed
    // once. "Lands once" is a claim about a non-restarting clock only.
    let c = Composition::parse(&format!(
        "tempo=132\nseed=91827\nphrase_bars=1\n[parts.hat]\nuse='techno.closed_hat'\n{DENSE}\n[phrases.A]\n[arrangement]\nrepeat=true\nsections=[{{phrase='A',bars=2,phase='restart'}},{{phrase='A',bars=2,phase='continue'}}]\n[[pins]]\nat={{roll='fire',voice='hat',bar=1,slot=1}}\nu=0.321"
    ))
    .unwrap();
    let pinned_steps: Vec<u64> = (0..192)
        .filter(|s| resolve_step(&c, *s).0[0].trigger.pinned)
        .collect();
    assert_eq!(pinned_steps, vec![0, 64, 128]);
    // The same pin under no arrangement at all is consulted exactly once.
    let plain = Composition::parse(&format!(
        "tempo=132\nseed=91827\nphrase_bars=1\n[parts.hat]\nuse='techno.closed_hat'\n{DENSE}\n[[pins]]\nat={{roll='fire',voice='hat',bar=1,slot=1}}\nu=0.321"
    ))
    .unwrap();
    assert_eq!(
        (0..192)
            .filter(|s| resolve_step(&plain, *s).0[0].trigger.pinned)
            .count(),
        1
    );
}

// t-514: Draw.pinned carries the pin's index into the authored list, not merely a bool.
// This lets consumers join back to the authored pin without re-resolving the address.
#[test]
fn draw_pinned_carries_the_authored_pin_index_not_a_bare_bool() {
    use phasecraft::music::resolve::{Address, Dice, Draw, Pin, PinAt};
    let pins = vec![
        Pin {
            at: PinAt {
                roll: "fire".into(),
                voice: Some("kick".into()),
                accent: None,
                bar: 1,
                slot: 1,
            },
            u: 0.1,
            address: Address {
                part: "kick".into(),
                lane: "trigger".into(),
                event: 0,
                decision: "admission".into(),
            },
        },
        Pin {
            at: PinAt {
                roll: "fire".into(),
                voice: Some("kick".into()),
                accent: None,
                bar: 1,
                slot: 2,
            },
            u: 0.2,
            address: Address {
                part: "kick".into(),
                lane: "trigger".into(),
                event: 1,
                decision: "admission".into(),
            },
        },
    ];
    let dice = Dice {
        seed: 42,
        pins: &pins,
    };
    // The first pin (index 0) should be returned with pinned = Some(0).
    let hit0 = dice.roll("kick", "trigger", 0, "admission");
    assert_eq!(
        hit0,
        Draw {
            u: 0.1,
            pinned: Some(0)
        }
    );
    // The second pin (index 1) should be returned with pinned = Some(1).
    let hit1 = dice.roll("kick", "trigger", 1, "admission");
    assert_eq!(
        hit1,
        Draw {
            u: 0.2,
            pinned: Some(1)
        }
    );
    // A roll that matches no pin returns None.
    let miss = dice.roll("kick", "trigger", 99, "admission");
    assert!(miss.pinned.is_none());
}

// t-511: ghost_pinned, timing_pinned, and velocity_pinned appear on the groove/touch trace
// when those draws are forced by authored pins. Written only when true; omitted otherwise.
#[test]
fn touch_trace_carries_pinned_flags_for_ghost_timing_and_velocity() {
    // A hat that fires every step with ghost, humanize timing, and humanize velocity active —
    // the full set of touch draws — so all three flags can be exercised.
    let c = Composition::parse(
        "tempo=132\nseed=91827\nphrase_bars=4\n\
         [parts.hat]\nuse='techno.closed_hat'\n\
         trigger.rhythm={steps=1,pulses=1}\ntrigger.probability=1.0\n\
         groove.ghost_probability=0.5\n\
         groove.humanize={timing_ticks=10,velocity=0.3}\n\
         [[pins]]\nat={roll='ghost',voice='hat',bar=1,slot=1}\nu=0.99\n\
         [[pins]]\nat={roll='timing',voice='hat',bar=1,slot=1}\nu=0.123\n\
         [[pins]]\nat={roll='velocity',voice='hat',bar=1,slot=1}\nu=0.456",
    )
    .unwrap();
    let step0_json = serde_json::to_string(&resolve(&c, 0)).unwrap();
    // All three pins are at step 0: ghost_pinned on GrooveTrace, timing_pinned and
    // velocity_pinned on TouchTrace.
    assert!(
        step0_json.contains("\"ghost_pinned\":true"),
        "ghost_pinned missing: {step0_json}"
    );
    assert!(
        step0_json.contains("\"timing_pinned\":true"),
        "timing_pinned missing: {step0_json}"
    );
    assert!(
        step0_json.contains("\"velocity_pinned\":true"),
        "velocity_pinned missing: {step0_json}"
    );
    // An unpinned step carries no pin flags at all.
    let step1_json = serde_json::to_string(&resolve(&c, 1)).unwrap();
    assert!(
        !step1_json.contains("ghost_pinned"),
        "unexpected ghost_pinned at step 1: {step1_json}"
    );
    assert!(
        !step1_json.contains("timing_pinned"),
        "unexpected timing_pinned at step 1: {step1_json}"
    );
    assert!(
        !step1_json.contains("velocity_pinned"),
        "unexpected velocity_pinned at step 1: {step1_json}"
    );
}

// t-517: A timing pin at step e is consumed by up to three decisions: the touch closure at
// e, the onset offset of e, and the gate length of the event that reserves e (offset(e) bounds
// it). gate_timing_pinned on the event makes the third recoverable without re-deriving the
// address at the consumer.
#[test]
fn gate_timing_pinned_names_the_consuming_decision_when_next_step_pin_bounds_this_gate() {
    // Step 1 (bar 1 slot 2) has an event. The gate-bounding call reads offset(step=2),
    // which consults the timing pin at bar 1 slot 3. So step 1's event should carry
    // gate_timing_pinned=true even though step 1 itself has no timing pin.
    let c = Composition::parse(
        "tempo=132\nseed=91827\nphrase_bars=4\n\
         [parts.hat]\nuse='techno.closed_hat'\n\
         trigger.rhythm={steps=1,pulses=1}\ntrigger.probability=1.0\n\
         groove.humanize={timing_ticks=10,velocity=0.0}\n\
         [[pins]]\nat={roll='timing',voice='hat',bar=1,slot=3}\nu=0.789",
    )
    .unwrap();
    // Step 1's main event groove should have gate_timing_pinned set — the gate-bounding
    // call consumed the timing pin at slot 3 (step 2).
    let step1 = resolve(&c, 1);
    let step1_event = step1.event.as_ref().expect("step 1 must have an event");
    let step1_groove = step1_event
        .groove
        .as_ref()
        .expect("step 1 must have a groove trace");
    assert!(
        step1_event.gate_timing_pinned,
        "gate_timing_pinned should be true on step 1 (next step's pin bounded its gate)"
    );
    // Step 1's own onset draw is not pinned: neither flag for its own address is set.
    assert!(
        !step1_groove.touch.as_ref().is_some_and(|t| t.timing_pinned),
        "step 1 should not have timing_pinned (pin is at slot 3, not slot 2)"
    );
    assert!(
        !step1_event.onset_timing_pinned,
        "step 1's own onset draw is not pinned (pin is at slot 3, not slot 2)"
    );
    // Step 2 (slot 3) is where the pin's address lives: timing_pinned appears on its event.
    let step2 = resolve(&c, 2);
    let step2_groove = step2
        .event
        .as_ref()
        .and_then(|e| e.groove.as_ref())
        .expect("step 2 must have an event with a groove trace");
    assert!(
        step2_groove.touch.as_ref().is_some_and(|t| t.timing_pinned),
        "step 2 must carry timing_pinned (the pin is at bar 1 slot 3)"
    );
    assert!(
        step2.event.as_ref().is_some_and(|e| e.onset_timing_pinned),
        "step 2's onset offset consumed the same pin"
    );
    // Step 0's event should NOT have gate_timing_pinned — the next-step timing
    // draw for step 1 (slot 2) is not pinned; only slot 3 is.
    let step0 = resolve(&c, 0);
    assert!(
        !step0.event.as_ref().is_some_and(|e| e.gate_timing_pinned),
        "step 0 should not have gate_timing_pinned (slot 2 / step 1 has no timing pin)"
    );
}

// Mark's review of #22 at 98ff64cc, P2: a timing pin is consumed by `compiled::offset` on
// every admitted event and again for the next-onset reservation — neither site requires a
// touch configuration, and the gate site does not require a groove at all. Both consumers
// must record the provenance where they consume it, or a neutral-amount pin (no humanize:
// zero jitter ticks, unchanged output) vanishes from the trace entirely.
#[test]
fn onset_timing_pin_is_recorded_without_a_touch_configuration() {
    // delay_ticks alone: the groove is non-default (so a GrooveTrace exists) but draws_touch()
    // is false, so the touch closure never runs. The onset offset draw still consults the pin.
    let c = Composition::parse(
        "tempo=132\nseed=91827\nphrase_bars=4\n\
         [parts.hat]\nuse='techno.closed_hat'\n\
         trigger.rhythm={steps=1,pulses=1}\ntrigger.probability=1.0\n\
         groove.delay_ticks=5\n\
         [[pins]]\nat={roll='timing',voice='hat',bar=1,slot=2}\nu=0.5",
    )
    .unwrap();
    let step1 = resolve(&c, 1);
    let event = step1.event.as_ref().expect("step 1 must have an event");
    let groove = event
        .groove
        .as_ref()
        .expect("delay_ticks makes the groove non-default, so a trace exists");
    assert!(
        groove.touch.is_none(),
        "this configuration must not open the touch closure, or it tests the wrong site"
    );
    assert_eq!(
        groove.offset_ticks, 5,
        "the pin is neutral: no humanize means zero jitter ticks and the offset is the delay"
    );
    assert!(
        event.onset_timing_pinned,
        "step 1's onset offset consumed the timing pin at bar 1 slot 2: {}",
        serde_json::to_string(&step1).unwrap()
    );
    // An unpinned neighbour carries no flag.
    let step3 = resolve(&c, 3);
    assert!(
        !step3.event.as_ref().is_some_and(|e| e.onset_timing_pinned),
        "step 3 has no timing pin"
    );
}

#[test]
fn gate_timing_pin_is_recorded_under_a_default_groove() {
    // No groove at all: `event.groove` is None, so there is no GrooveTrace to hang the flag
    // on — but the gate-bounding call still reads offset(step 2) and still consults the pin.
    let c = Composition::parse(
        "tempo=132\nseed=91827\nphrase_bars=4\n\
         [parts.hat]\nuse='techno.closed_hat'\n\
         trigger.rhythm={steps=1,pulses=1}\ntrigger.probability=1.0\n\
         [[pins]]\nat={roll='timing',voice='hat',bar=1,slot=3}\nu=0.789",
    )
    .unwrap();
    let step1 = resolve(&c, 1);
    let event = step1.event.as_ref().expect("step 1 must have an event");
    assert!(
        event.groove.is_none(),
        "a default groove writes no GrooveTrace, or this tests the wrong site"
    );
    assert!(
        event.gate_timing_pinned,
        "step 1's gate reserved the next onset and consumed the pin at slot 3: {}",
        serde_json::to_string(&step1).unwrap()
    );
    // The pin's own address: step 2's onset draw is not made under a default groove
    // (`offset` is only called for the onset when the groove is non-default), so the only
    // consumer is step 1's gate. Step 2 itself carries no onset flag.
    let step2 = resolve(&c, 2);
    assert!(
        !step2.event.as_ref().is_some_and(|e| e.onset_timing_pinned),
        "a default groove makes no onset offset draw, so nothing is pinned at step 2"
    );
    // Step 0's gate reserves step 1, which has no pin.
    let step0 = resolve(&c, 0);
    assert!(
        !step0.event.as_ref().is_some_and(|e| e.gate_timing_pinned),
        "step 0 reserves step 1, which carries no timing pin"
    );
}

// t-517, audible counterfactual (Mark's review of #22): the consultation tests above prove the
// flag is written; this one proves the flag names a decision that is heard. A silent step's
// timing pin still shortens the *sounding* previous event's gate, because the gate reserves the
// next structural onset whether or not that onset fires. Only the pin's value changes between
// the two compositions.
#[test]
fn a_silent_steps_timing_pin_shortens_the_previous_sounding_gate() {
    let sounding = |u: &str| {
        let c = Composition::parse(&format!(
            "tempo=132\nseed=91827\nphrase_bars=4\n\
             [parts.hat]\nuse='techno.closed_hat'\n\
             trigger.rhythm={{steps=2,pulses=1,rotation=1}}\ntrigger.probability=1.0\n\
             output.gate_ticks=239\n\
             groove.humanize={{timing_ticks=30,velocity=0.0}}\n\
             [[pins]]\nat={{roll='timing',voice='hat',bar=1,slot=3}}\nu={u}"
        ))
        .unwrap();
        // Step 2 (bar 1 slot 3) is where the pin lives, and it never fires.
        assert!(
            !resolve(&c, 2).trigger.admitted,
            "step 2 must stay silent, or this tests an ordinary onset pin"
        );
        let step1 = resolve(&c, 1);
        let event = step1.event.expect("step 1 sounds");
        (event.duration_ticks, event.gate_timing_pinned)
    };
    // u = 0.999 pushes the reserved onset late: the gate keeps its full requested length.
    assert_eq!(sounding("0.999"), (239, true));
    // u = 0.0 pulls it 30 ticks early: the same authored gate is cut to 232.
    assert_eq!(sounding("0.0"), (232, true));
    // The flag records consultation, not proof of shortening — it is true in both cases.
    // An event whose reserved onset carries no pin does not set it.
    let c = Composition::parse(
        "tempo=132\nseed=91827\nphrase_bars=4\n\
         [parts.hat]\nuse='techno.closed_hat'\n\
         trigger.rhythm={steps=2,pulses=1,rotation=1}\ntrigger.probability=1.0\n\
         output.gate_ticks=239\n\
         groove.humanize={timing_ticks=30,velocity=0.0}\n\
         [[pins]]\nat={roll='timing',voice='hat',bar=1,slot=3}\nu=0.0",
    )
    .unwrap();
    assert!(
        !resolve(&c, 3).event.is_some_and(|e| e.gate_timing_pinned),
        "step 3 reserves step 4, which carries no pin"
    );
}
