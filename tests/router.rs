//! The until-stop shape, end to end: a return group over three voices' clocks and a router
//! that rolls at every return. The pattern and lane clocks are stood in for by Parts with the
//! same periods until M1.4/M1.5 own those keys.
use phasecraft::music::{Composition, STEP_TICKS, resolve::Compiled, router};

/// piece.toml's rows and return group, with 16/5/7-slot Parts standing in for the members.
fn until_stop(seed: u64) -> String {
    format!(
        r#"
tempo = 126
seed = {seed}
start = "crowded"
[parts.hat]
use = "techno.closed_hat"
trigger.rhythm = {{ steps = 16, pulses = 6 }}
output.controls.cutoff = {{ cc = 74, default = 1.0 }}
parameters.cutoff = {{ value = 0.2, ramp = {{ to = 0.8, over_bars = 4 }} }}
[parts.memory]
use = "techno.rim"
trigger.rhythm = {{ steps = 5, pulses = 1 }}
[parts.drift]
use = "techno.clap"
trigger.rhythm = {{ steps = 7, pulses = 1 }}
[scenes]
crowded = {{}}
exposed = {{ parts.hat.trigger.probability = 0.5 }}
hollow = {{ parts.hat.trigger.probability = 0.2, parts.hat.parameters.cutoff = {{ value = 0.3 }} }}
[router]
every = {{ returns = "hat_system" }}
[router.routes]
crowded = {{ crowded = 0.70, exposed = 0.25, hollow = 0.05 }}
exposed = {{ crowded = 0.30, exposed = 0.55, hollow = 0.15 }}
hollow  = {{ crowded = 0.30, exposed = 0.45, hollow = 0.25 }}
[returns.hat_system]
align = ["voices.hat.trigger.cycle", "voices.memory.trigger.cycle", "voices.drift.trigger.cycle"]
"#
    )
}
#[test]
fn the_group_returns_every_thirty_five_bars_and_the_engine_computed_that() {
    let c = Composition::parse(&until_stop(91827)).unwrap();
    let r = c.router.as_ref().unwrap();
    assert_eq!(r.scene_names(), ["crowded", "exposed", "hollow"]);
    assert_eq!(r.start, "crowded");
    let period = r.period_ticks(&c).unwrap();
    assert_eq!(period, 560 * STEP_TICKS);
    assert_eq!(period / (16 * STEP_TICKS), 35);
    // TRACE.md: return r is the downbeat of bar 35r + 1 — bars 36, 71, 106.
    let mut compiled = Compiled::new(&c);
    for r in 1..=3u64 {
        let step = r * 560;
        let visit = compiled.resolve_step(step).0[0].scene.clone().unwrap();
        assert_eq!((visit.index, visit.start_tick), (r, r * period));
        assert_eq!(step / 16 + 1, 35 * r + 1);
        let before = compiled.resolve_step(step - 1).0[0].scene.clone().unwrap();
        assert_eq!(before.index, r - 1);
    }
    assert_eq!(c.end_step(), None);
}
#[test]
fn every_return_is_rolled_once_at_moves_r_and_the_trace_says_where_the_piece_is() {
    let c = Composition::parse(&until_stop(91827)).unwrap();
    let r = c.router.as_ref().unwrap();
    let period = r.period_ticks(&c).unwrap();
    let mut compiled = Compiled::new(&c);
    let log = compiled.moves_through(18 * period).to_vec();
    assert_eq!(log.len(), 18);
    let mut scene = "crowded".to_string();
    for (i, m) in log.iter().enumerate() {
        assert_eq!(m.index as usize, i + 1);
        assert_eq!(m.tick, m.index * period);
        assert_eq!(m.from, scene);
        assert_eq!(m.roll, router::roll(91827, m.index));
        assert_eq!(m.moved, m.from != m.to);
        scene = m.to.clone();
    }
    for step in [0, 559, 560, 1000, 1120, 5000] {
        let traces = compiled.resolve_step(step).0;
        let visit = traces[0].scene.as_ref().unwrap();
        assert!(traces.iter().all(|t| t.scene.as_ref() == Some(visit)));
        assert_eq!(visit.index, step * STEP_TICKS / period);
        let expected = if visit.index == 0 {
            "crowded"
        } else {
            &log[visit.index as usize - 1].to
        };
        assert_eq!(visit.scene, expected);
        assert_eq!(
            c.at_step(step).parts[0].trigger.probability,
            r.scene(expected).unwrap().composition.parts[0]
                .trigger
                .probability
        );
        assert!(traces.iter().all(|t| t.section.is_none()));
    }
}
#[test]
fn a_move_resets_the_outgoing_scene_and_a_stay_resets_nothing() {
    // Search seeds for a log with both a stay and a move in its first returns; both exist.
    let (c, log) = (1u64..)
        .find_map(|seed| {
            let c = Composition::parse(&until_stop(seed)).unwrap();
            let period = c.router.as_ref().unwrap().period_ticks(&c).unwrap();
            let log = Compiled::new(&c).moves_through(6 * period).to_vec();
            (log.iter().any(|m| m.moved) && log.iter().any(|m| !m.moved)).then_some((c, log))
        })
        .unwrap();
    let mut compiled = Compiled::new(&c);
    for m in &log {
        let step = m.tick / STEP_TICKS;
        let (traces, midi) = compiled.resolve_step(step);
        let resets = midi.iter().filter(|e| e.boundary_reset).count();
        let visit = traces[0].scene.as_ref().unwrap();
        if m.moved {
            assert!(
                resets > 0,
                "return {} moved {} -> {} without resets",
                m.index,
                m.from,
                m.to
            );
            assert_eq!(
                (visit.entered_tick, visit.from.as_deref()),
                (m.tick, Some(m.from.as_str()))
            );
        } else {
            assert_eq!(
                resets, 0,
                "return {} stayed in {} and reset something",
                m.index, m.to
            );
            assert!(visit.entered_tick < m.tick);
        }
        // The step after a boundary carries no reset either way.
        assert_eq!(
            compiled
                .resolve_step(step + 1)
                .1
                .iter()
                .filter(|e| e.boundary_reset)
                .count(),
            0
        );
    }
}
#[test]
fn the_expanded_form_round_trips_and_plays_the_same_events() {
    let c = Composition::parse(&until_stop(91827)).unwrap();
    let text = toml::to_string(&c).unwrap();
    assert!(
        text.contains("[[router.scenes]]")
            && text.contains("align = [")
            && !text.contains("members")
    );
    let copy = Composition::parse(&text).unwrap();
    assert_eq!(
        copy.router.as_ref().unwrap().scene_names(),
        ["crowded", "exposed", "hollow"]
    );
    assert!(
        copy.router
            .as_ref()
            .unwrap()
            .scenes
            .iter()
            .all(|s| s.composition.returns.is_empty())
    );
    let (mut a, mut b) = (Compiled::new(&c), Compiled::new(&copy));
    for step in (0..48).chain(556..564).chain(1116..1124) {
        assert_eq!(
            a.resolve_step(step).1,
            b.resolve_step(step).1,
            "step {step}"
        );
    }
    assert!(c.same_arrangement_layout(&copy));
    // A different route weight is a different piece: structural, not a live edit.
    let edited = Composition::parse(
        &until_stop(91827)
            .replace("hollow = 0.05", "hollow = 0.10")
            .replace("exposed = 0.25", "exposed = 0.20"),
    )
    .unwrap();
    assert!(!c.same_arrangement_layout(&edited));
    let musical =
        Composition::parse(&until_stop(91827).replace("probability = 0.5", "probability = 0.6"))
            .unwrap();
    assert!(c.same_arrangement_layout(&musical));
    // So are the seed and the period: either one re-rolls the log.
    assert!(!c.same_arrangement_layout(&Composition::parse(&until_stop(4471)).unwrap()));
    let shorter = Composition::parse(
        &until_stop(91827).replace("steps = 7, pulses = 1", "steps = 8, pulses = 1"),
    )
    .unwrap();
    assert_eq!(
        shorter
            .router
            .as_ref()
            .unwrap()
            .period_ticks(&shorter)
            .unwrap(),
        80 * STEP_TICKS
    );
    assert!(!c.same_arrangement_layout(&shorter));
}
#[test]
fn phrase_bars_is_not_the_router_and_scenes_continue_the_transport_phase() {
    // B28: the router's cadence does not touch phrase_bars, and every visit is continue-phase —
    // a scene evaluated at step s sees the same decisions the plain composition sees at s.
    let c = Composition::parse(&until_stop(91827)).unwrap();
    assert_eq!(c.phrase_bars, 4);
    let mut compiled = Compiled::new(&c);
    let r = c.router.as_ref().unwrap();
    for step in [560, 561, 1120, 1130] {
        let traces = compiled.resolve_step(step).0;
        let scene = &traces[0].scene.as_ref().unwrap().scene;
        let mut plain = r.scene(scene).unwrap().composition.as_ref().clone();
        plain.returns.clear();
        let expected = Compiled::new(&plain).resolve_step(step).0;
        for (a, b) in traces.iter().zip(&expected) {
            assert_eq!(
                (a.part.as_str(), a.step, a.trigger.roll, a.trigger.admitted),
                (b.part.as_str(), b.step, b.trigger.roll, b.trigger.admitted)
            );
        }
    }
}
