use phasecraft::music::{
    Composition, STEP_TICKS,
    groove::{Groove, RunContour},
    resolve::resolve_step,
};
fn song() -> Composition {
    Composition::parse(
        r#"
        tempo=132
        seed=123
        [parts.hat]
        use="techno.closed_hat"
        trigger.rhythm={steps=1,pulses=1}
        accent.rhythm={steps=4,pulses=1}
    "#,
    )
    .unwrap()
}
#[test]
fn timing_and_ghosting_preserve_trigger_accent_and_rng_identities() {
    let straight = song();
    let mut grooved = straight.clone();
    grooved.parts[0].groove = Groove {
        swing: 0.58,
        ghost_probability: 1.,
        delay_ticks: 12,
        ..Groove::default()
    };
    for step in 0..560 {
        let (a, _) = resolve_step(&straight, step);
        let (b, midi) = resolve_step(&grooved, step);
        assert_eq!(
            serde_json::to_value(&a[0].trigger).unwrap(),
            serde_json::to_value(&b[0].trigger).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&a[0].accent).unwrap(),
            serde_json::to_value(&b[0].accent).unwrap()
        );
        let e = b[0].event.as_ref().unwrap();
        let g = e.groove.as_ref().unwrap();
        assert_eq!(g.ghost, !e.accent.active);
        assert_eq!(g.offset_ticks, if step % 2 == 1 { 50 } else { 12 });
        assert_eq!(e.tick, step * STEP_TICKS + g.offset_ticks);
        assert!(midi.iter().all(|m| m.tick < (step + 1) * STEP_TICKS));
    }
}
#[test]
fn contour_uses_actual_neighbor_admissions_and_is_bounded() {
    let mut c = song();
    c.parts[0].groove.run = RunContour::RampUp;
    let factors: Vec<_> = (0..6)
        .map(|s| {
            resolve_step(&c, s).0[0]
                .event
                .as_ref()
                .unwrap()
                .groove
                .as_ref()
                .unwrap()
                .velocity_factor
        })
        .collect();
    assert_eq!(factors, vec![0.75, 0.875, 1., 1., 1., 1.]);
    c.parts[0].trigger.probability = 0.;
    assert!(resolve_step(&c, 2).0[0].event.is_none());
    assert_eq!(
        Groove {
            run: RunContour::RampUp,
            ..Groove::default()
        }
        .contour(0, 1),
        1.
    );
    assert_eq!(
        Groove {
            run: RunContour::LowHighLow,
            ..Groove::default()
        }
        .contour(1, 1),
        1.
    );
}
#[test]
fn delayed_long_gates_end_inside_their_source_step_and_merge_in_time_order() {
    let mut c = song();
    c.parts[0].groove.swing = 0.75;
    c.parts[0].groove.delay_ticks = 60;
    c.parts[0].output.gate_ticks = 239;
    c.validate().unwrap();
    let (e, m) = resolve_step(&c, 1);
    assert_eq!(e[0].event.as_ref().unwrap().duration_ticks, 59);
    assert_eq!(m[0].tick, 420);
    assert_eq!(m[1].tick, 479);
    for bad in [0.49, 0.76, f64::NAN] {
        c.parts[0].groove.swing = bad;
        assert!(c.validate().is_err());
    }
}
#[test]
fn garage_is_deterministic_has_ghosts_and_preserves_909_backbeats() {
    let c = Composition::parse(include_str!("../examples/quickstart/garage.toml")).unwrap();
    for step in 0..560 {
        let (traces, midi) = resolve_step(&c, step);
        assert_eq!(midi, resolve_step(&c, step).1);
        assert!(midi.windows(2).all(|w| w[0].tick <= w[1].tick));
        let snare = traces.iter().find(|t| t.part == "snare").unwrap();
        if step % 16 == 4 || step % 16 == 12 {
            let e = snare.event.as_ref().unwrap();
            assert!(e.accent.active);
            assert!(!e.groove.as_ref().unwrap().ghost);
        }
        for event in traces.iter().filter_map(|t| t.event.as_ref()) {
            assert!(event.tick + event.duration_ticks < (step + 1) * STEP_TICKS);
        }
    }
}

#[test]
fn groove_does_not_change_cross_part_references_or_depend_on_part_order() {
    let c = Composition::parse(include_str!("../examples/quickstart/garage.toml")).unwrap();
    let mut straight = c.clone();
    let mut reordered = c.clone();
    reordered.parts.reverse();
    for p in &mut straight.parts {
        p.groove = Groove::default();
    }
    for step in 0..80 {
        let (a, midi) = resolve_step(&c, step);
        assert_eq!(midi, resolve_step(&reordered, step).1);
        let (b, _) = resolve_step(&straight, step);
        for (a, b) in a.iter().zip(b.iter()) {
            assert_eq!(a.part, b.part);
            assert_eq!(
                serde_json::to_value(&a.trigger).unwrap(),
                serde_json::to_value(&b.trigger).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&a.accent).unwrap(),
                serde_json::to_value(&b.accent).unwrap()
            );
        }
    }
}
