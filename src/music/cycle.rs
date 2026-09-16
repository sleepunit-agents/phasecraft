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
    /// None means the common multiple exceeds u64 or a source has no fixed structural cycle.
    pub phase_alignment_steps: Option<u64>,
    pub phase_alignment_ticks: Option<u64>,
}
/// Why a structural period cannot supply a fixed transport clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PeriodError {
    Retained,
    Overflow,
}
type Period = Result<u64, PeriodError>;
fn lcm(a: Period, b: Period) -> Period {
    // Evolving material has no period regardless of the other operand's size.
    if a == Err(PeriodError::Retained) || b == Err(PeriodError::Retained) {
        return Err(PeriodError::Retained);
    }
    let (a, b) = (a?, b?);
    let (mut x, mut y) = (a, b);
    while y != 0 {
        (x, y) = (y, x % y);
    }
    (a / x).checked_mul(b).ok_or(PeriodError::Overflow)
}
fn expression(
    e: &Expression,
    phrase: u64,
    cell: u64,
    references: &BTreeMap<&str, Period>,
) -> Period {
    match e {
        Expression::Retained { .. } => Err(PeriodError::Retained),
        Expression::Literal { .. } => e
            .literal_schedule()
            .map(|p| p.schedule().period_ticks())
            .ok_or(PeriodError::Overflow),
        Expression::Euclidean {
            steps,
            reset_on_phrase,
            ..
        } => {
            if *reset_on_phrase {
                lcm(Ok(phrase), Ok(cell))
            } else {
                u64::from(*steps)
                    .checked_mul(cell)
                    .ok_or(PeriodError::Overflow)
            }
        }
        Expression::Binary { a, b, .. } => lcm(
            expression(a, phrase, cell, references),
            expression(b, phrase, cell, references),
        ),
        Expression::Part { id, .. } => lcm(references[id.as_str()], Ok(cell)),
    }
}
/// Every Part's trigger period in ticks, in evaluation order so references resolve first.
fn trigger_periods(c: &Composition) -> Result<BTreeMap<&str, Period>, String> {
    let mut refs = BTreeMap::new();
    for part in c.evaluation_order()? {
        let cycle = expression(
            &part.trigger.rhythm,
            c.phrase_steps() * STEP_TICKS,
            part.subdivision.0,
            &refs,
        );
        refs.insert(part.id.as_str(), cycle);
    }
    Ok(refs)
}
/// The structural trigger period; None for evolving material or u64 overflow.
pub fn trigger_period_ticks(c: &Composition, p: &Part) -> Result<Option<u64>, String> {
    Ok(trigger_period(c, p)?.ok())
}
pub(crate) fn trigger_period(c: &Composition, p: &Part) -> Result<Period, String> {
    Ok(trigger_periods(c)?[p.id.as_str()])
}
/// The structural accent period; None for evolving material or u64 overflow.
pub fn accent_period_ticks(c: &Composition, p: &Part) -> Result<Option<u64>, String> {
    Ok(accent_period(c, p)?.ok())
}
pub(crate) fn accent_period(c: &Composition, p: &Part) -> Result<Period, String> {
    Ok(expression(
        &p.accent.rhythm,
        c.phrase_steps() * STEP_TICKS,
        p.subdivision.0,
        &trigger_periods(c)?,
    ))
}
fn alignment(c: &Composition, p: &Part) -> Option<u64> {
    let refs = trigger_periods(c).expect("validated composition");
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
    if let Some(value) = &p.velocity {
        cycle = if value.per() == super::process::Per::Step {
            lcm(cycle, Ok(value.cycle_ticks()))
        } else {
            return None;
        };
    }
    cycle.ok()
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
        } else if let Some(r) = &c.router {
            // Every visit continues the transport's phase; the window closes at the next return.
            let period = r.period_ticks(c).expect("validated composition");
            let visit = r.visit_at(c, period, step * STEP_TICKS);
            (
                r.scene(&visit.scene)
                    .expect("validated router")
                    .composition
                    .as_ref(),
                0,
                (visit.next_tick / STEP_TICKS).min(end),
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
