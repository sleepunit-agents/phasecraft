use phasecraft::music::process::{
    MutationDecision as D, MutationOutcome as O,
    retained::{RetainedPattern, pins::MutationPin},
};
const HAT: &str = include_str!("fixtures/hat-memory.toml");
const SCENES: &[&str] = &["crowded", "hollow", "exposed"];
fn pattern(chance: f64) -> RetainedPattern {
    toml::from_str(&HAT.replace("crowded = 0.23", &format!("crowded = {chance}"))).unwrap()
}
fn pin(tick: u64, fields: &str) -> MutationPin {
    toml::from_str(&format!(
        "at = {{ roll = \"mutate\", pattern = \"hat-memory\", tick = {tick} }}\n{fields}"
    ))
    .unwrap()
}

#[test]
fn authored_pin_52_preserves_indices_and_authored_provenance_under_rerolls() {
    let pins = [pin(52, "admit = true\nrest_index = 2\nhit_index = 0")];
    let mut selected = Vec::new();
    for seed in [91827, 4471, 1, 2] {
        let mut source = pattern(0.23)
            .bind_pinned("hat-memory", SCENES, &pins)
            .unwrap();
        for _ in 0..260 {
            source.advance("crowded", seed).unwrap();
        }
        let before = source
            .source()
            .state()
            .pending()
            .unwrap_or(source.source().state().material());
        let rest = before
            .iter()
            .enumerate()
            .filter(|(_, h)| !**h)
            .nth(2)
            .unwrap()
            .0;
        let hit = before.iter().position(|&h| h).unwrap();
        let result = source.advance("crowded", seed).unwrap();
        assert!(
            matches!(result.sample.step.outcome, O::Swapped {rest_index: 2, hit_index: 0, rest_slot, hit_slot, ..} if (rest_slot, hit_slot) == (rest, hit))
        );
        assert_eq!(
            result
                .sample
                .draws
                .iter()
                .map(|d| d.draw.pinned)
                .collect::<Vec<_>>(),
            [Some(0); 3]
        );
        assert_eq!(result.landings.len(), 3);
        assert!(
            result
                .landings
                .iter()
                .all(|p| p.drawn && !p.endpoint_mismatch)
        );
        selected.push((rest, hit));
    }
    assert!(selected.windows(2).any(|p| p[0] != p[1]));
}

#[test]
fn endpoint_sugar_reports_disagreement_and_refusal_skips_selections() {
    for (chance, admit, admitted, mismatch) in [
        (0.0, true, false, true),
        (1.0, false, true, true),
        (0.23, true, true, false),
        (0.23, false, false, false),
        (0.0, false, false, false),
        (1.0, true, true, false),
    ] {
        let pins = [pin(
            0,
            &format!("admit = {admit}\nrest_index = 2\nhit_index = 0"),
        )];
        let mut source = pattern(chance)
            .bind_pinned("hat-memory", SCENES, &pins)
            .unwrap();
        assert_eq!(source.lints().len(), usize::from(!admit));
        let result = source.advance("crowded", 91827).unwrap();
        assert_eq!(result.sample.chance, Some(chance));
        assert_eq!(result.sample.scene, "crowded");
        assert_eq!(
            matches!(result.sample.step.outcome, O::Swapped { .. }),
            admitted
        );
        assert!(result.landings[0].drawn);
        assert_eq!(result.landings[0].endpoint_mismatch, mismatch);
        assert_eq!(result.landings[1].drawn, admitted);
        assert_eq!(result.landings[2].drawn, admitted);
        assert_eq!(
            result.sample.draws[0].draw.u,
            if admit { 0.0 } else { 1.0_f64.next_down() }
        );
    }
}

#[test]
fn suspended_and_not_due_report_no_draws_and_unknown_scene_rolls_back() {
    let pins = [
        pin(0, "admit = true\nrest_index = 2"),
        pin(1, "hit_index = 0"),
    ];
    let mut source = pattern(1.0)
        .bind_pinned("hat-memory", SCENES, &pins)
        .unwrap();
    assert!(source.advance("typo", 91827).is_err());
    assert_eq!(source.source().state().next_slot(), 0);
    let result = source.advance("hollow", 91827).unwrap();
    assert_eq!(result.sample.step.outcome, O::Suspended);
    assert!(result.sample.draws.is_empty());
    assert_eq!(result.landings.len(), 2);
    assert!(
        result
            .landings
            .iter()
            .all(|p| !p.drawn && !p.endpoint_mismatch)
    );
    for _ in 1..5 {
        assert!(
            source
                .advance("crowded", 91827)
                .unwrap()
                .landings
                .is_empty()
        );
    }
    let mut checkpoint = source.clone();
    let result = source.advance("crowded", 91827).unwrap();
    assert_eq!(result, checkpoint.advance("crowded", 91827).unwrap());
    assert_eq!(result.landings.len(), 1);
    assert_eq!(result.landings[0].pin, 1);
    assert_eq!(result.landings[0].decision, D::HitIndex);
    assert!(result.landings[0].drawn);
    assert_eq!(result.sample.draws[0].draw.pinned, None);
}

#[test]
fn split_fields_lint_across_entries_but_repeated_addresses_are_rejected() {
    let pins = [
        pin(0, "rest_index = 2\nhit_index = 0"),
        pin(0, "admit = false"),
    ];
    let mut source = pattern(1.0)
        .bind_pinned("hat-memory", SCENES, &pins)
        .unwrap();
    assert_eq!(source.lints().len(), 1);
    assert_eq!(source.lints()[0].admission_pin, 1);
    assert_eq!(source.lints()[0].selection_pin, 0);
    let result = source.advance("crowded", 91827).unwrap();
    assert_eq!(
        result
            .sample
            .draws
            .iter()
            .map(|d| d.draw.pinned)
            .collect::<Vec<_>>(),
        [Some(1), Some(0), Some(0)]
    );
    let dup = [pin(0, "rest_index = 2"), pin(0, "rest_index = 3")];
    assert!(
        pattern(1.0)
            .bind_pinned("hat-memory", SCENES, &dup)
            .unwrap_err()
            .contains("duplicate")
    );
}

#[test]
fn closed_shape_pattern_cardinality_and_transport_bounds_are_checked() {
    for fields in ["", "rest_index = 10", "hit_index = 6"] {
        assert!(
            pattern(1.0)
                .bind_pinned("hat-memory", SCENES, &[pin(0, fields)])
                .is_err()
        );
    }
    let pins = [pin(0, "admit = true")];
    assert!(pattern(1.0).bind_pinned("other", SCENES, &pins).is_err());
    assert!(
        pattern(1.0)
            .bind_pinned(
                "hat-memory",
                SCENES,
                &[pin(i64::MAX as u64, "admit = true")]
            )
            .is_err()
    );
    assert!(
        pattern(1.0)
            .bind_pinned("hat-memory", SCENES, &vec![pins[0].clone(); 257])
            .is_err()
    );
    let base = "at = { roll = \"mutate\", pattern = \"hat-memory\", tick = 0 }\nadmit = true";
    for (a, b) in [
        ("mutate", "door"),
        ("tick = 0", "tick = -1"),
        ("tick = 0", "bar = 1"),
        ("admit = true", "admit = 0.0"),
        ("admit = true", "u = 0.0"),
    ] {
        assert!(toml::from_str::<MutationPin>(&base.replace(a, b)).is_err());
    }
    assert!(
        pattern(1.0)
            .bind_pinned("hat-memory", SCENES, &[pin(u64::MAX / 5, "admit = true")])
            .is_err()
    );
    let serialized = toml::to_string(&pins[0]).unwrap();
    let roundtrip: MutationPin = toml::from_str(&serialized).unwrap();
    assert!(
        pattern(1.0)
            .bind_pinned("hat-memory", SCENES, &[roundtrip])
            .is_ok()
    );
}

#[test]
fn source_cardinality_drives_resolution_and_maximum_pin_list_is_accepted() {
    let initial = "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~";
    let wide: RetainedPattern = toml::from_str(&HAT.replace(initial, &"x ~ ".repeat(22))).unwrap();
    let pins = [pin(0, "admit = true\nrest_index = 15\nhit_index = 21")];
    let mut source = wide.bind_pinned("hat-memory", SCENES, &pins).unwrap();
    let result = source.advance("crowded", 91827).unwrap();
    assert!(matches!(
        result.sample.step.outcome,
        O::Swapped {
            rest_index: 15,
            hit_index: 21,
            rest_slot: 31,
            hit_slot: 42,
            ..
        }
    ));
    let pins: Vec<_> = (0..256).map(|tick| pin(tick, "admit = true")).collect();
    assert!(wide.bind_pinned("hat-memory", SCENES, &pins).is_ok());
}

#[test]
fn empty_pin_slice_matches_existing_source_across_commits_and_scene_changes() {
    use phasecraft::music::resolve::Dice;
    let pattern = pattern(0.23);
    let mut plain = pattern.bind("hat-memory", SCENES).unwrap();
    let mut pinned = pattern.bind_pinned("hat-memory", SCENES, &[]).unwrap();
    for slot in 0..=1000 {
        let scene = if (256..720).contains(&slot) {
            "hollow"
        } else {
            "crowded"
        };
        let result = pinned.advance(scene, 91827).unwrap();
        assert_eq!(
            result.sample,
            plain
                .advance(
                    scene,
                    Dice {
                        seed: 91827,
                        pins: &[]
                    }
                )
                .unwrap()
        );
        assert!(result.landings.is_empty());
        assert_eq!(pinned.source().state().material(), plain.state().material());
        assert_eq!(pinned.source().state().pending(), plain.state().pending());
    }
}
