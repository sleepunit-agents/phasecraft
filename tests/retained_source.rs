use phasecraft::music::{
    process::{
        MutationDecision as D, MutationOutcome as O, mutation_index_roll,
        retained::{RetainedPattern, RetainedSource},
    },
    resolve::{Address, Dice, Pin, PinAt, decision_roll},
};

// Verbatim companion source at until-stop main 0a72b1480e6a244078bf162e93e9778e0a0c64f2.
const HAT: &str = include_str!("fixtures/hat-memory.toml");
const SCENES: &[&str] = &["crowded", "hollow", "exposed"];
fn source(text: &str) -> RetainedSource {
    toml::from_str::<RetainedPattern>(text)
        .unwrap()
        .bind("hat-memory", SCENES)
        .unwrap()
}
fn dice(seed: u64) -> Dice<'static> {
    Dice { seed, pins: &[] }
}
// Manually resolved API input. The native authored PinAt parser cannot yet admit
// mutate/pattern/tick; no claim of Composition pin authoring is made by this test.
fn pin(event: u64, decision: &str, u: f64) -> Pin {
    Pin {
        at: PinAt {
            roll: "mutate".into(),
            voice: None,
            accent: None,
            bar: 1,
            slot: 1,
        },
        u,
        address: Address {
            part: "hat-memory".into(),
            lane: "mutate".into(),
            event,
            decision: decision.into(),
        },
    }
}

#[test]
fn companion_source_round_trips_and_replays_a_scene_journey() {
    let parsed: RetainedPattern = toml::from_str(HAT).unwrap();
    let serialized = toml::to_string(&parsed).unwrap();
    let mut a = parsed.bind("hat-memory", SCENES).unwrap();
    let mut b = source(&serialized);
    let initial = a.state().material().to_vec();
    for slot in 0..=1000 {
        let scene = match slot {
            0..256 => "crowded",
            256..720 => "hollow",
            _ => "exposed",
        };
        let sample = a.advance(scene, dice(91827)).unwrap();
        assert_eq!(sample, b.advance(scene, dice(91827)).unwrap());
        assert_eq!(a.state().material(), b.state().material());
        assert_eq!(a.state().pending(), b.state().pending());
        assert_eq!(a.state().material().iter().filter(|&&hit| hit).count(), 6);
        assert_eq!(sample.step.slot, slot);
        assert_eq!(sample.step.occurrence, (slot % 5 == 0).then_some(slot / 5));
        if slot < 16 {
            assert_eq!(a.state().material(), initial);
        }
        if scene == "hollow" {
            assert!(sample.draws.is_empty());
            assert_eq!(sample.chance, None);
        }
        for recorded in sample.draws {
            let key = match recorded.decision {
                D::Admit => "admit",
                D::RestIndex => "rest_index",
                D::HitIndex => "hit_index",
            };
            assert_eq!(
                recorded.draw.u,
                decision_roll(91827, "hat-memory", "mutate", slot / 5, key)
            );
            assert_eq!(recorded.draw.pinned, None);
        }
    }
}

#[test]
fn resolved_pin_52_selects_pending_indices_under_different_histories() {
    let pins = [
        pin(52, "admit", 0.0),
        pin(52, "rest_index", mutation_index_roll(2, 10).unwrap()),
        pin(52, "hit_index", mutation_index_roll(0, 6).unwrap()),
    ];
    let mut selected = Vec::new();
    for seed in [91827, 4471, 1, 2] {
        let mut state = source(HAT);
        for _ in 0..260 {
            state.advance("crowded", dice(seed)).unwrap();
        }
        let before = state.state().pending().unwrap_or(state.state().material());
        let rest = before
            .iter()
            .enumerate()
            .filter(|(_, hit)| !**hit)
            .nth(2)
            .unwrap()
            .0;
        let hit = before.iter().position(|&hit| hit).unwrap();
        let sample = state
            .advance("crowded", Dice { seed, pins: &pins })
            .unwrap();
        assert_eq!(sample.step.occurrence, Some(52));
        assert_eq!(
            sample
                .draws
                .iter()
                .map(|d| d.draw.pinned)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1), Some(2)]
        );
        assert!(
            matches!(sample.step.outcome, O::Swapped {rest_index: 2, hit_index: 0, rest_slot, hit_slot, ..} if (rest_slot, hit_slot) == (rest, hit))
        );
        selected.push((rest, hit));
    }
    assert!(
        selected.windows(2).any(|pair| pair[0] != pair[1]),
        "a pinned index does not freeze material history"
    );
}

#[test]
fn zero_draws_refusal_absence_draws_nothing_and_transport_addresses_agree() {
    let text = HAT.replace("crowded = 0.23", "crowded = 0.0");
    let mut zero = source(&text);
    let mut absent = source(&text);
    for _ in 0..=80 {
        let a = zero.advance("crowded", dice(91827)).unwrap();
        let b = absent.advance("hollow", dice(91827)).unwrap();
        assert_eq!(a.step.occurrence, b.step.occurrence);
        if a.step.occurrence.is_some() {
            assert!(matches!(a.step.outcome, O::Refused { .. }));
            assert_eq!(a.draws.len(), 1);
            assert_eq!(b.step.outcome, O::Suspended);
        }
        assert!(b.draws.is_empty());
    }
    for _ in 81..=100 {
        assert_eq!(
            zero.advance("exposed", dice(91827)).unwrap(),
            absent.advance("exposed", dice(91827)).unwrap()
        );
    }
}

#[test]
fn unknown_scene_and_invalid_pin_do_not_spend_or_commit() {
    let text = HAT.replace("crowded = 0.23", "crowded = 1.0");
    let mut state = source(&text);
    for _ in 0..80 {
        state.advance("crowded", dice(91827)).unwrap();
    }
    assert!(state.state().pending().is_some());
    let mut control = state.clone();
    assert!(
        state
            .advance("crowde", dice(91827))
            .unwrap_err()
            .contains("unknown scene")
    );
    assert_eq!(state.state().next_slot(), 80);
    let pins = [pin(16, "hit_index", f64::NAN)];
    assert!(
        state
            .advance(
                "crowded",
                Dice {
                    seed: 91827,
                    pins: &pins
                }
            )
            .is_err()
    );
    assert_eq!(state.state().next_slot(), 80);
    assert_eq!(state.state().material(), control.state().material());
    assert_eq!(state.state().pending(), control.state().pending());
    let sample = state.advance("hollow", dice(91827)).unwrap();
    assert_eq!(sample, control.advance("hollow", dice(91827)).unwrap());
    assert!(sample.step.committed);
    assert_eq!(sample.step.outcome, O::Suspended);
    assert!(sample.draws.is_empty());
}

#[test]
fn preparation_refuses_unsupported_shapes_and_out_of_bounds_values() {
    for (from, to) in [
        ("elsewhere = \"hold\"", "elsewhere = \"reset\""),
        ("carry     = \"always\"", "carry = \"scene\""),
        ("slots = 5", "slots = 0"),
        ("slots = 5", "slots = 65537"),
        ("slots = 5", "bars = 5"),
        ("crowded = 0.23", "crowded = nan"),
        ("crowded = 0.23", "crowded = inf"),
        ("crowded = 0.23", "crowded = -0.01"),
        ("crowded = 0.23", "crowded = 1.01"),
        ("moves  = [\"swap\"]", "moves = []"),
        ("moves  = [\"swap\"]", "moves = [\"swap\", \"swap\"]"),
        ("moves  = [\"swap\"]", "moves = [\"shift\"]"),
        ("on = \"cycle\"", "on = \"return\""),
        ("x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~", "x? ~"),
        ("x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~", "[x ~]"),
        ("x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~", "x*2 ~"),
        ("x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~", "~ ~"),
        ("x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~", "x x"),
        ("x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~", ""),
    ] {
        assert!(HAT.contains(from), "bad test mutation: {from}");
        assert!(
            toml::from_str::<RetainedPattern>(&HAT.replace(from, to)).is_err(),
            "accepted {to}"
        );
    }
    assert!(toml::from_str::<RetainedPattern>(&format!("unknown = true\n{HAT}")).is_err());
    let initial = "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~";
    assert!(
        toml::from_str::<RetainedPattern>(
            &HAT.replace(initial, &format!("x {}", "~ ".repeat(4096)))
        )
        .is_err()
    );
    let max: RetainedPattern =
        toml::from_str(&HAT.replace(initial, &format!("x {}", "~ ".repeat(4095)))).unwrap();
    assert_eq!(
        max.bind("hat-memory", SCENES)
            .unwrap()
            .state()
            .material()
            .len(),
        4096
    );
    let bar: RetainedPattern =
        toml::from_str(&HAT.replace("on = \"cycle\"", "on = \"bar\"")).unwrap();
    assert!(bar.bind("hat-memory", SCENES).is_ok());
}

#[test]
fn binding_validates_complete_scene_vocabulary_without_requiring_chance_entries() {
    let pattern: RetainedPattern = toml::from_str(HAT).unwrap();
    assert!(pattern.bind("hat-memory", SCENES).is_ok());
    assert!(
        pattern
            .bind("hat-memory", &["crowded", "hollow"])
            .unwrap_err()
            .contains("unknown chance scene \"exposed\"")
    );
    for scenes in [&[][..], &[""], &["crowded", "crowded"], &["crowded"; 65]] {
        assert!(pattern.bind("hat-memory", scenes).is_err());
    }
    assert!(pattern.bind(" ", SCENES).is_err());
}

#[test]
fn source_addresses_have_a_fixed_native_hash_and_numeric_pins_do_not_force_outcomes() {
    let text = HAT.replace("crowded = 0.23", "crowded = 1.0");
    let mut source = source(&text);
    for _ in 0..260 {
        source.advance("hollow", dice(91827)).unwrap();
    }
    let sample = source.advance("crowded", dice(91827)).unwrap();
    // Independent Python hashlib calculation of the versioned length-framed address.
    assert_eq!(
        sample.draws.iter().map(|d| d.draw.u).collect::<Vec<_>>(),
        vec![0.9672704411349887, 0.5003669634275622, 0.14269710662317947,]
    );
    let pins = [pin(53, "admit", 0.0), pin(53, "rest_index", 0.25)];
    let zero = HAT.replace("crowded = 0.23", "crowded = 0.0");
    let mut source = self::source(&zero);
    for _ in 0..265 {
        source.advance("hollow", dice(91827)).unwrap();
    }
    let sample = source
        .advance(
            "crowded",
            Dice {
                seed: 91827,
                pins: &pins,
            },
        )
        .unwrap();
    assert_eq!(sample.step.outcome, O::Refused { admit: 0.0 });
    assert_eq!(sample.draws.len(), 1);
    assert_eq!(sample.draws[0].draw.pinned, Some(0));
}
