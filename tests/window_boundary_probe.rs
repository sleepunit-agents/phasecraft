//! Local-only probe (NOT for commit as-is): does closing the evaluation window at every
//! return preserve every gate and history-dependent decision, or only the run?
//! The distinguishing case is an all-stays router: the scene never changes, so a
//! router-clocked piece must be indistinguishable from the same piece with no router.
use phasecraft::music::{Composition, STEP_TICKS, cycle, resolve::Compiled};

const HEAD: &str = r#"
tempo = 126
seed = 91827
[parts.hat]
use = "techno.closed_hat"
trigger.rhythm = { steps = 16, pulses = 6 }
trigger.probability = 0.5
[parts.memory]
use = "techno.rim"
trigger.rhythm = { steps = 5, pulses = 1 }
[parts.drift]
use = "techno.clap"
trigger.rhythm = { steps = 7, pulses = 1 }
"#;
const RETURNS: &str = r#"
[returns.hat_system]
align = ["voices.hat.trigger.cycle", "voices.memory.trigger.cycle", "voices.drift.trigger.cycle"]
"#;

fn all_stays() -> Composition {
    Composition::parse(&format!(
        "start = \"crowded\"\n{HEAD}\n[scenes]\ncrowded = {{}}\n[router]\nevery = {{ returns = \"hat_system\" }}\n[router.routes]\ncrowded = {{ crowded = 1.0 }}\n{RETURNS}"
    ))
    .unwrap()
}
fn no_router() -> Composition {
    Composition::parse(&format!("{HEAD}{RETURNS}")).unwrap()
}

#[test]
fn an_all_stays_router_changes_nothing_a_listener_can_hear() {
    let routed = all_stays();
    let plain = no_router();
    let r = routed.router.as_ref().unwrap();
    let period_steps = r.period_ticks(&routed).unwrap() / STEP_TICKS;
    let span = period_steps * 4; // four returns
    let mut a = Compiled::new(&routed);
    let mut b = Compiled::new(&plain);
    let mut differ = 0;
    for step in 0..span {
        let (_, ea) = a.resolve_step(step);
        let (_, eb) = b.resolve_step(step);
        if ea != eb {
            if differ == 0 {
                println!(
                    "first divergence at step {step} (return {}, offset {})",
                    step / period_steps,
                    step % period_steps
                );
            }
            differ += 1;
        }
    }
    println!("EVENTS: {differ} of {span} steps differ between all-stays router and no router");

    // The same question asked of the analysis surface, which is what closes at a return.
    let sa = cycle::spans(&routed, "hat", 0, span);
    let sb = cycle::spans(&plain, "hat", 0, span);
    println!(
        "SPANS: all-stays router = {} span(s), no router = {} span(s)",
        sa.len(),
        sb.len()
    );
    for s in &sa {
        println!(
            "  [{} .. {}) origin {} phrase {}",
            s.start_tick, s.end_tick, s.phase_origin_tick, s.phrase_steps
        );
    }
    for s in &sb {
        println!(
            "  plain [{} .. {}) origin {} phrase {}",
            s.start_tick, s.end_tick, s.phase_origin_tick, s.phrase_steps
        );
    }
}

/// The other half: a router that MOVES at every return, between two scenes whose bodies are
/// identical. Here `moved` is true and `entered_tick` resets at every return, so the run is
/// NOT preserved — but nothing a listener can hear changed. If the event stream still matches
/// the unrouted piece, the gates are keyed by something other than the run.
fn twins() -> phasecraft::music::Composition {
    phasecraft::music::Composition::parse(&format!(
        "start = \"a\"\n{HEAD}\n[scenes]\na = {{}}\nb = {{}}\n[router]\nevery = {{ returns = \"hat_system\" }}\n[router.routes]\na = {{ b = 1.0 }}\nb = {{ a = 1.0 }}\n{RETURNS}"
    ))
    .unwrap()
}

#[test]
fn a_move_between_identical_scenes_is_also_inaudible_but_resets_the_run() {
    let routed = twins();
    let plain = no_router();
    let r = routed.router.as_ref().unwrap();
    let period = r.period_ticks(&routed).unwrap();
    let period_steps = period / STEP_TICKS;
    let span = period_steps * 4;
    let mut log = Vec::new();
    r.extend_moves(routed.seed, period, &mut log, 4);
    println!(
        "moves: {:?}",
        log.iter()
            .map(|m| (m.index, m.moved, m.to.as_str()))
            .collect::<Vec<_>>()
    );
    for i in 1..=4u64 {
        let v = r.visit_at(routed.seed, period, i * period);
        println!(
            "  return {i}: scene {} entered_tick {}",
            v.scene, v.entered_tick
        );
    }
    let mut a = Compiled::new(&routed);
    let mut b = Compiled::new(&plain);
    let mut differ = 0;
    for step in 0..span {
        if a.resolve_step(step).1 != b.resolve_step(step).1 {
            if differ == 0 {
                println!("first divergence at step {step}");
            }
            differ += 1;
        }
    }
    println!("EVENTS: {differ} of {span} steps differ between twin-scene router and no router");
    println!(
        "SPANS: {} (twins) vs {} (plain)",
        cycle::spans(&routed, "hat", 0, span).len(),
        cycle::spans(&plain, "hat", 0, span).len()
    );
}
