//! Local-only timing probe for Router::visit_at (NOT for commit as-is).
//! Answers: what does one visit_at cost at the tick a long run reaches,
//! and what does the transport's per-step pattern cost after that long?
use phasecraft::music::{Composition, STEP_TICKS};
use std::time::Instant;

fn until_stop(seed: u64) -> String {
    format!(
        r#"
tempo = 126
seed = {seed}
start = "crowded"
[parts.hat]
use = "techno.closed_hat"
trigger.rhythm = {{ steps = 16, pulses = 6 }}
[parts.memory]
use = "techno.rim"
trigger.rhythm = {{ steps = 5, pulses = 1 }}
[parts.drift]
use = "techno.clap"
trigger.rhythm = {{ steps = 7, pulses = 1 }}
[scenes]
crowded = {{}}
exposed = {{ parts.hat.trigger.probability = 0.5 }}
hollow = {{ parts.hat.trigger.probability = 0.2 }}
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
fn bench_visit_at() {
    let c = Composition::parse(&until_stop(91827)).unwrap();
    let r = c.router.as_ref().unwrap();
    let period = r.period_ticks(&c).unwrap();
    // step duration at tempo 126, 16th-note steps
    let step_secs = 60.0 / 126.0 / 4.0;
    let period_steps = period / STEP_TICKS;
    println!(
        "period = {period} ticks = {period_steps} steps = {:.2} s of playback",
        period_steps as f64 * step_secs
    );
    for hours in [1.0f64, 8.0, 24.0, 24.0 * 7.0] {
        let secs = hours * 3600.0;
        let step = (secs / step_secs) as u64;
        let tick = step * STEP_TICKS;
        let index = tick / period;
        // one call
        let n = 200u32;
        let t = Instant::now();
        let mut sink = 0u64;
        for _ in 0..n {
            sink += r.visit_at(c.seed, period, tick).index;
        }
        let per_call = t.elapsed() / n;
        assert!(sink > 0 || index == 0);
        println!(
            "after {hours:>5.0} h: step {step:>9}  return index {index:>7}  one visit_at = {:?}",
            per_call
        );
    }
}

#[test]
fn bench_transport_day() {
    // The transport's pattern: two visit_at per scheduled step. Walk a real day of steps
    // and report the wall time actually spent inside visit_at.
    let c = Composition::parse(&until_stop(91827)).unwrap();
    let r = c.router.as_ref().unwrap();
    let period = r.period_ticks(&c).unwrap();
    let step_secs = 60.0 / 126.0 / 4.0;
    let day_steps = (86400.0 / step_secs) as u64;
    // sample: measure a contiguous window of steps at several depths, extrapolate the day
    let mut total = 0.0f64;
    let window = 200u64;
    let samples = 60u64;
    for s in 0..samples {
        let base = day_steps * s / samples;
        let t = Instant::now();
        let mut sink = 0u64;
        for k in 0..window {
            let tick = (base + k) * STEP_TICKS;
            sink += r.visit_at(c.seed, period, tick).index;
            sink += r.visit_at(c.seed, period, tick).index;
        }
        let e = t.elapsed().as_secs_f64() / window as f64;
        assert!(sink > 0 || base == 0);
        total += e * (day_steps as f64 / samples as f64);
    }
    println!(
        "day = {day_steps} steps; extrapolated visit_at cost over 24 h of playback = {total:.1} s \
         ({:.4}% of wall clock)",
        total / 86400.0 * 100.0
    );
}
