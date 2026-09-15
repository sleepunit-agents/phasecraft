use phasecraft::music::process::{
    Apply, MutationDecision as D, MutationOutcome as O, RetainedSwap,
};

const HAT: [bool; 16] = [
    true, false, true, false, false, true, false, true, false, false, true, false, true, false,
    false, false,
];

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
