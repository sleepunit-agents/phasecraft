//! Phase metadata for a realized window; never materialize a full common cycle.
use super::{Composition, Part, STEP_TICKS, rhythm::Expression};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct CycleSpan {
    pub start_tick: u64,
    pub end_tick: u64,
    pub phase_origin_tick: u64,
    pub phrase_steps: u64,
    /// Common structural phase alignment, not a promise of identical realized events.
    /// None means the common multiple exceeds u64 (or a future source has no known cycle).
    pub phase_alignment_steps: Option<u64>,
    pub phase_alignment_ticks: Option<u64>,
}
fn lcm(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    let (a, b) = (a?, b?);
    let (mut x, mut y) = (a, b);
    while y != 0 {
        (x, y) = (y, x % y);
    }
    (a / x).checked_mul(b)
}
fn expression(
    e: &Expression,
    phrase: u64,
    cell: u64,
    references: &BTreeMap<&str, Option<u64>>,
) -> Option<u64> {
    match e {
        Expression::Euclidean {
            steps,
            reset_on_phrase,
            ..
        } => {
            if *reset_on_phrase {
                lcm(Some(phrase), Some(cell))
            } else {
                u64::from(*steps).checked_mul(cell)
            }
        }
        Expression::Binary { a, b, .. } => lcm(
            expression(a, phrase, cell, references),
            expression(b, phrase, cell, references),
        ),
        Expression::Part { id, .. } => lcm(references[id.as_str()], Some(cell)),
    }
}
fn alignment(c: &Composition, p: &Part) -> Option<u64> {
    let mut refs = BTreeMap::new();
    for part in c.evaluation_order().expect("validated composition") {
        let cycle = expression(
            &part.trigger.rhythm,
            c.phrase_steps() * STEP_TICKS,
            part.subdivision.0,
            &refs,
        );
        refs.insert(part.id.as_str(), cycle);
    }
    let mut cycle = lcm(
        refs[p.id.as_str()],
        expression(
            &p.accent.rhythm,
            c.phrase_steps() * STEP_TICKS,
            p.subdivision.0,
            &refs,
        ),
    );
    for name in &p.accent.sources {
        cycle = lcm(
            cycle,
            expression(
                &c.accents[name].rhythm,
                c.phrase_steps() * STEP_TICKS,
                STEP_TICKS,
                &refs,
            ),
        );
    }
    cycle
}
pub fn spans(c: &Composition, part_id: &str, start: u64, end: u64) -> Vec<CycleSpan> {
    let mut result = vec![];
    let mut step = start;
    while step < end {
        let (effective, origin, until) = if let Some(a) = &c.arrangement {
            let Some(s) = a.locate(step) else { break };
            (
                s.section.composition.as_ref(),
                step - s.musical_step,
                (s.start_step + u64::from(s.section.bars) * 16).min(end),
            )
        } else {
            (c, 0, end)
        };
        if let Some(p) = effective.parts.iter().find(|p| p.id == part_id) {
            result.push(CycleSpan {
                start_tick: step * STEP_TICKS,
                end_tick: until * STEP_TICKS,
                phase_origin_tick: origin * STEP_TICKS,
                phrase_steps: effective.phrase_steps(),
                phase_alignment_steps: alignment(effective, p)
                    .filter(|n| n.is_multiple_of(STEP_TICKS))
                    .map(|n| n / STEP_TICKS),
                phase_alignment_ticks: alignment(effective, p),
            });
        }
        step = until;
    }
    result
}
