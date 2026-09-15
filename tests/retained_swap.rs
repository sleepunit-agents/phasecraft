use phasecraft::music::process::{
    Apply, MutationDecision as D, MutationOutcome as O, RetainedSwap, mutation_index_roll,
};

const HAT: [bool; 16] = [
    true, false, true, false, false, true, false, true, false, false, true, false, true, false,
    false, false,
];

#[test]
fn forced_indices_round_trip_for_every_bounded_bucket() {
    // Includes the first lower-edge rounding failure: index 15 at count 22.
    for count in 1..=4096 {
        for index in 0..count {
            let roll = mutation_index_roll(index, count).unwrap();
            assert!((0.0..1.0).contains(&roll));
            assert_eq!((roll * count as f64) as usize, index);
        }
    }
    for (index, count) in [(0, 0), (1, 1), (4096, 4096), (0, 4097), (usize::MAX, 2)] {
        assert!(mutation_index_roll(index, count).is_err());
    }
}

#[test]
fn forced_indices_report_the_pending_lists_before_each_swap() {
    let mut initial = vec![false; 44];
    for slot in (0..44).step_by(2) {
        initial[slot] = true;
    }
    let mut state = RetainedSwap::new(&initial, 1, Apply::Cycle).unwrap();
    let mut expected = initial;
    // Multiple accumulating edits and a coincident cycle commit. The same forced
    // index selects different slots as the pending material changes.
    for slot in 0..=44 {
        let rest_index = if slot % 2 == 0 { 15 } else { 21 };
        let hit_index = if slot % 2 == 0 { 0 } else { 15 };
        let rest_slot = (0..44).filter(|&i| !expected[i]).nth(rest_index).unwrap();
        let hit_slot = (0..44).filter(|&i| expected[i]).nth(hit_index).unwrap();
        let rest_roll = mutation_index_roll(rest_index, 22).unwrap();
        let hit_roll = mutation_index_roll(hit_index, 22).unwrap();
        let step = state
            .advance(Some(1.0), |_, decision| match decision {
                D::Admit => 0.0,
                D::RestIndex => rest_roll,
                D::HitIndex => hit_roll,
            })
            .unwrap();
        assert_eq!(step.committed, slot == 44);
        if step.committed {
            assert_eq!(state.material(), expected);
        }
        assert_eq!(
            step.outcome,
            O::Swapped {
                admit: 0.0,
                rest_roll,
                hit_roll,
                rest_index,
                hit_index,
                rest_slot,
                hit_slot,
            }
        );
        expected[rest_slot] = true;
        expected[hit_slot] = false;
        assert_eq!(state.pending().unwrap(), expected);
    }
}

#[test]
fn hat_accumulates_ascending_swaps_without_rewriting_the_audible_cycle() {
    let mut state = RetainedSwap::new(&HAT, 5, Apply::Cycle).unwrap();
    for slot in 0..16 {
        let step = state
            .advance(Some(1.0), |occurrence, decision| {
                assert_eq!(occurrence, slot / 5);
                match decision {
                    D::Admit | D::HitIndex => 0.0,
                    D::RestIndex => 0.25, // third of ten rests, in ascending order
                }
            })
            .unwrap();
        assert_eq!(state.material(), HAT);
        assert!(!step.committed);
        if slot % 5 == 0 {
            assert!(matches!(step.outcome, O::Swapped { .. }));
            assert_eq!(state.pending().unwrap().iter().filter(|&&x| x).count(), 6);
        } else {
            assert_eq!(step.outcome, O::NotDue);
        }
    }
    // Four edits, computed over each preceding edit, not four edits of the initial copy:
    // 1 -> 5, 3 -> 4, 4 -> 3, 3 -> 4 (one-based).
    let expected = [
        false, false, false, true, true, true, false, true, false, false, true, false, true, false,
        false, false,
    ];
    let step = state
        .advance(None, |_, _| panic!("suspended/non-due slot drew"))
        .unwrap();
    assert!(step.committed);
    assert_eq!(state.material(), expected);
    assert!(state.pending().is_none());
}

#[test]
fn coincident_boundary_commits_before_mutating_and_freeze_keeps_pending() {
    let mut state = RetainedSwap::new(&[true, false, false, false], 4, Apply::Cycle).unwrap();
    for slot in 0..=8 {
        let step = state
            .advance(if slot == 8 { None } else { Some(1.0) }, |_, _| 0.0)
            .unwrap();
        match slot {
            0 => {
                assert_eq!(state.material(), [true, false, false, false]);
                assert!(!step.committed);
            }
            4 => {
                assert!(step.committed);
                assert_eq!(state.material(), [false, true, false, false]);
                assert_eq!(state.pending().unwrap(), [true, false, false, false]);
            }
            8 => {
                assert!(step.committed);
                assert_eq!(step.outcome, O::Suspended);
                assert_eq!(state.material(), [true, false, false, false]);
            }
            _ => {}
        }
    }
}

#[test]
fn suspended_and_zero_chance_spend_different_draws_but_keep_transport_phase() {
    let mut state = RetainedSwap::new(&HAT, 5, Apply::Cycle).unwrap();
    let mut addresses = Vec::new();
    for slot in 0..=20 {
        let chance = if slot < 10 {
            None
        } else if slot < 20 {
            Some(0.0)
        } else {
            Some(1.0)
        };
        let step = state
            .advance(chance, |occurrence, decision| {
                addresses.push((occurrence, decision));
                0.0
            })
            .unwrap();
        if slot == 0 || slot == 5 {
            assert_eq!(step.outcome, O::Suspended);
        }
        if slot == 10 || slot == 15 {
            assert_eq!(step.outcome, O::Refused { admit: 0.0 });
        }
        assert_eq!(step.occurrence, (slot % 5 == 0).then_some(slot / 5));
    }
    assert_eq!(
        addresses,
        [
            (2, D::Admit),
            (3, D::Admit),
            (4, D::Admit),
            (4, D::RestIndex),
            (4, D::HitIndex)
        ]
    );
}

#[test]
fn apply_bar_and_cycle_publish_at_their_distinct_boundaries() {
    for (apply, boundary) in [(Apply::Bar, 16), (Apply::Cycle, 7)] {
        let mut state = RetainedSwap::new(
            &[true, false, false, false, false, false, false],
            65536,
            apply,
        )
        .unwrap();
        for slot in 0..=boundary {
            let step = state.advance(Some(1.0), |_, _| 0.0).unwrap();
            assert_eq!(step.committed, slot == boundary);
            assert_eq!(state.material()[0], slot < boundary);
        }
    }
}

#[test]
fn invalid_input_is_rejected_without_advancing_or_committing() {
    for initial in [
        &[][..],
        &[true][..],
        &[false, false][..],
        &vec![false; 4097][..],
    ] {
        assert!(RetainedSwap::new(initial, 1, Apply::Bar).is_err());
    }
    for every in [0, 65537] {
        assert!(RetainedSwap::new(&HAT, every, Apply::Bar).is_err());
    }
    let mut state = RetainedSwap::new(&[true, false], 1, Apply::Cycle).unwrap();
    for _ in 0..2 {
        state.advance(Some(1.0), |_, _| 0.0).unwrap();
    }
    let material = state.material().to_vec();
    let pending = state.pending().unwrap().to_vec();
    for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(
            state
                .advance(Some(invalid), |_, _| panic!("invalid probability drew"))
                .is_err()
        );
    }
    for decision in [D::Admit, D::RestIndex, D::HitIndex] {
        for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.0] {
            assert!(
                state
                    .advance(Some(1.0), |_, d| if d == decision { invalid } else { 0.0 })
                    .is_err()
            );
            assert_eq!(state.next_slot(), 2);
            assert_eq!(state.material(), material);
            assert_eq!(state.pending().unwrap(), pending);
        }
    }
    assert!(state.advance(Some(1.0), |_, _| 0.0).unwrap().committed);
}

#[test]
fn draw_edges_preserve_hit_count_across_many_commits_and_scene_changes() {
    let mut state = RetainedSwap::new(&HAT, 5, Apply::Cycle).unwrap();
    let last = f64::from_bits(1.0f64.to_bits() - 1);
    for slot in 0..10000 {
        let chance = if (slot / 112) % 3 == 1 {
            None
        } else {
            Some(1.0)
        };
        let step = state
            .advance(chance, |occurrence, decision| {
                if decision == D::Admit || occurrence % 2 == 0 {
                    0.0
                } else {
                    last
                }
            })
            .unwrap();
        assert_eq!(state.material().iter().filter(|&&x| x).count(), 6);
        if let Some(pending) = state.pending() {
            assert_eq!(pending.iter().filter(|&&x| x).count(), 6);
        }
        if let O::Swapped {
            rest_slot,
            hit_slot,
            ..
        } = step.outcome
        {
            assert_ne!(rest_slot, hit_slot);
        }
    }
}

#[test]
fn swaps_can_cancel_while_a_pending_commit_still_exists() {
    let initial = [true, false, false, false];
    let mut state = RetainedSwap::new(&initial, 1, Apply::Cycle).unwrap();
    for slot in 0..=4 {
        let step = state
            .advance(if slot < 2 { Some(1.0) } else { None }, |_, _| 0.0)
            .unwrap();
        assert_eq!(state.material(), initial);
        if slot == 2 {
            assert_eq!(state.pending().unwrap(), initial);
        }
        if slot == 4 {
            assert!(step.committed);
            assert!(state.pending().is_none());
        }
    }
}
