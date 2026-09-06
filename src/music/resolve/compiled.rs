use super::*;

/// Compiled dependency order and bounded raw-decision cache, owned by one immutable snapshot.
pub struct Compiled {
    composition: Composition,
    order: Vec<usize>,
    indices: std::collections::BTreeMap<String, usize>,
    raw: std::collections::BTreeMap<u64, std::sync::Arc<Vec<StepTrace>>>,
    sections: Vec<Compiled>,
    cell_keys: std::collections::VecDeque<(usize, u64, u64, u64)>,
    cells: std::collections::BTreeMap<(usize, u64, u64, u64), StepTrace>,
}
impl Compiled {
    pub fn new(c: &Composition) -> Self {
        let indices: std::collections::BTreeMap<_, _> = c
            .parts
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        let order = c
            .evaluation_order()
            .expect("validated composition")
            .iter()
            .map(|p| indices[&p.id])
            .collect();
        let sections = c
            .arrangement
            .as_ref()
            .map(|a| {
                a.sections
                    .iter()
                    .map(|s| Self::new(&s.composition))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            composition: c.clone(),
            order,
            indices,
            raw: Default::default(),
            cells: Default::default(),
            cell_keys: Default::default(),
            sections,
        }
    }
    fn raw_at(&mut self, tick: u64) -> std::sync::Arc<Vec<StepTrace>> {
        if let Some(traces) = self.raw.get(&tick) {
            return traces.clone();
        }
        let c = &self.composition;
        let mut resolved = vec![None; c.parts.len()];
        for &index in &self.order {
            let part = &c.parts[index];
            let reference = |id: &str, mode| {
                let index = self.indices[id];
                let target: &StepTrace = resolved[index].as_ref().unwrap();
                tick.is_multiple_of(c.parts[index].subdivision.0)
                    && match mode {
                        ReferenceMode::Structural => target.trigger.rhythm.active(),
                        ReferenceMode::Hits => target.trigger.admitted,
                    }
            };
            resolved[index] = Some(resolve_part(c, part, tick / part.subdivision.0, &reference));
        }
        let traces =
            std::sync::Arc::new(resolved.into_iter().map(Option::unwrap).collect::<Vec<_>>());
        // Bounded even under random seeks; large LCMs never become buffers.
        if self.raw.len() >= 4096 {
            self.raw.pop_first();
        }
        self.raw.insert(tick, traces.clone());
        traces
    }
    pub fn resolve_step(&mut self, step: u64) -> (Vec<StepTrace>, Vec<MidiEvent>) {
        if let Some(a) = &self.composition.arrangement {
            let Some(located) = a.locate(step) else {
                return (vec![], vec![]);
            };
            let index = located.position.index - 1;
            let shift = (step - located.musical_step) * STEP_TICKS;
            let lower = (located.musical_step - located.local_step) * STEP_TICKS;
            let upper = lower + u64::from(located.section.bars) * 16 * STEP_TICKS;
            let position = located.position.clone();
            let local_step = located.local_step;
            let musical_step = located.musical_step;
            let previous = (local_step == 0 && step > 0)
                .then(|| a.locate(step - 1))
                .flatten()
                .map(|s| {
                    crate::music::arrangement::resets(&s.section.composition, step * STEP_TICKS)
                });
            let (mut traces, mut midi) = self.sections[index].window(musical_step, lower, upper);
            for e in &mut midi {
                e.tick += shift;
            }
            for t in &mut traces {
                t.tick += shift;
                t.step = t.tick / STEP_TICKS;
                t.position = position_text(t.tick);
                t.section = Some(position.clone());
                for e in t.event.iter_mut().chain(t.extra_events.iter_mut()) {
                    e.tick += shift;
                }
                for e in t.sounding.iter_mut().flatten() {
                    e.tick += shift;
                }
                for p in &mut t.parameters {
                    for sample in &mut p.samples {
                        sample.tick += shift;
                    }
                }
            }
            midi.extend(previous.into_iter().flatten());
            midi.sort_by_key(midi_order);
            return (traces, midi);
        }
        self.window(step, 0, u64::MAX)
    }
    fn window(&mut self, step: u64, lower: u64, upper: u64) -> (Vec<StepTrace>, Vec<MidiEvent>) {
        let start = step * STEP_TICKS;
        let end = ((step + 1) * STEP_TICKS).min(upper);
        let mut traces = Vec::new();
        let mut midi = Vec::new();
        // Sorted IDs preserve provenance order independently of display order.
        let indices: Vec<_> = self.indices.values().copied().collect();
        for index in indices {
            let part = self.composition.parts[index].clone();
            let cell = part.subdivision.0;
            let envelope_steps = part
                .profile
                .controls
                .values()
                .filter_map(|r| r.envelope.as_ref().map(|e| e.history_steps()))
                .max()
                .unwrap_or(0);
            let first = start
                .saturating_sub((envelope_steps + 1) * STEP_TICKS)
                .max(lower)
                / cell;
            let last = (end + cell + 240).min(upper.saturating_sub(1)) / cell;
            let mut audible = Vec::new();
            let mut history = Vec::new();
            let mut cells = Vec::new();
            let mut attacks = Vec::new();
            for local in first..=last {
                let source = local * cell;
                if source < lower || source >= upper {
                    continue;
                }
                let mut trace = self.cell(index, local, lower, upper);
                if let Some(event) = trace.event.take() {
                    attacks.push((cells.len(), true, event));
                }
                for event in trace.extra_events.drain(..) {
                    attacks.push((cells.len(), false, event));
                }
                cells.push(trace);
            }
            // A Part owns one MIDI voice. Resolve neighboring ornaments together:
            // coincident attacks merge into the strongest, and the next attack ends the gate.
            attacks.sort_by_key(|(owner, main, e)| {
                (
                    e.tick,
                    std::cmp::Reverse(midi_velocity(&part, e)),
                    std::cmp::Reverse(*main),
                    *owner,
                )
            });
            attacks.dedup_by_key(|(_, _, e)| e.tick);
            for i in 0..attacks.len() {
                if let Some(next) = attacks.get(i + 1).map(|(_, _, e)| e.tick) {
                    attacks[i].2.duration_ticks = attacks[i]
                        .2
                        .duration_ticks
                        .min((next - attacks[i].2.tick).saturating_sub(1).max(1));
                }
            }
            for (owner, main, event) in attacks {
                if event.accent.active {
                    history.push((event.tick, event.accent.amount));
                }
                if event.tick < end && event.tick + event.duration_ticks >= start {
                    midi.extend(
                        to_midi(&part, &event)
                            .into_iter()
                            .filter(|m| m.tick >= start && m.tick < end),
                    );
                    audible.push(event.clone());
                }
                if main {
                    cells[owner].event = Some(event);
                } else {
                    cells[owner].extra_events.push(event);
                }
            }
            let mut part_traces: Vec<_> = cells
                .into_iter()
                .filter(|t| t.tick >= start && t.tick < end)
                .map(|mut t| {
                    t.step = t.tick / STEP_TICKS;
                    t.position = position_text(t.tick);
                    t
                })
                .collect();
            history.sort_by_key(|h| h.0);
            let (parameters, controls) =
                crate::music::parameter::resolve_window(&part, start, end, &audible, &history);
            midi.extend(controls);
            if !start.is_multiple_of(cell) || part_traces.is_empty() {
                // A display snapshot between slow-grid onsets is explicitly a rest.
                let mut trace = self.raw_at(start)[index].clone();
                trace.event = None;
                trace.tick = start / cell * cell;
                trace.step = step;
                trace.position = position_text(start);
                trace.trigger.admitted = false;
                part_traces.insert(0, trace);
            }
            if cell != STEP_TICKS || !part.ornaments.is_default() || part.groove.delay_ticks < 0 {
                part_traces[0].sounding = Some(audible);
            }
            part_traces[0].parameters = parameters;
            traces.extend(part_traces);
        }
        midi.sort_by_key(midi_order);
        (traces, midi)
    }
    fn cell(&mut self, index: usize, step: u64, lower: u64, upper: u64) -> StepTrace {
        let key = (index, step, lower, upper);
        if let Some(trace) = self.cells.get(&key) {
            return trace.clone();
        }
        let trace = self.compute_cell(index, step, lower, upper);
        if self.cells.len() >= 4096
            && let Some(old) = self.cell_keys.pop_front()
        {
            self.cells.remove(&old);
        }
        self.cells.insert(key, trace.clone());
        self.cell_keys.push_back(key);
        trace
    }
    fn compute_cell(&mut self, index: usize, step: u64, lower: u64, upper: u64) -> StepTrace {
        let part = self.composition.parts[index].clone();
        let cell = part.subdivision.0;
        // Bars own their attacks and releases, so snapshot swaps never strand a tail.
        let bar = crate::music::PPQN * 4;
        let lower = lower.max(step * cell / bar * bar);
        let upper = upper.min((step * cell / bar + 1) * bar);
        let mut trace = self.raw_at(step * cell)[index].clone();
        use crate::music::groove::{GrooveTrace, RunContour};
        let run_context = part.groove.run != RunContour::None;
        let lookbehind = part
            .groove
            .after_gap
            .as_ref()
            .map_or(0, |g| u64::from(g.steps))
            .max(if run_context { 2 } else { 0 });
        let mut neighbors = std::collections::BTreeMap::new();
        for s in (1..=lookbehind)
            .filter_map(|n| step.checked_sub(n))
            .chain((1..=if run_context { 2 } else { 0 }).map(|n| step + n))
        {
            neighbors.insert(s, self.raw_at(s * cell)[index].clone());
        }
        let c = &self.composition;
        if let Some(event) = &mut trace.event {
            let g = &part.groove;
            if !g.is_default() {
                let fired = |s: Option<u64>| {
                    s.and_then(|s| neighbors.get(&s))
                        .is_some_and(|traces| traces.trigger.admitted)
                };
                let mut before = 0;
                let mut after = 0;
                if g.run != RunContour::None {
                    for n in 1..=2 {
                        if fired(step.checked_sub(n)) {
                            before += 1
                        } else {
                            break;
                        }
                    }
                    for n in 1..=2 {
                        if fired(step.checked_add(n)) {
                            after += 1
                        } else {
                            break;
                        }
                    }
                }
                let identity = match g.ghost_mode {
                    ProbabilityMode::PhraseLocked => {
                        decision_identity(c, cell, step, ProbabilityMode::PhraseLocked)
                    }
                    ProbabilityMode::Continuous => step,
                };
                let roll = decision_roll(c.seed, &part.id, "groove", identity, "ghost");
                let ghost = !event.accent.active && roll < g.ghost_probability;
                let touch = (g.offbeat_gain != 1.0
                    || g.after_gap.is_some()
                    || g.humanize.is_some())
                .then(|| {
                    let offbeat = (step * cell) % crate::music::PPQN == crate::music::PPQN / 2;
                    let after_gap = g.after_gap.as_ref().is_some_and(|gap| {
                        step >= u64::from(gap.steps)
                            && (1..=u64::from(gap.steps)).all(|n| !fired(step.checked_sub(n)))
                    });
                    let h = g.humanize.clone().unwrap_or_default();
                    let identity = match h.mode {
                        ProbabilityMode::PhraseLocked => {
                            decision_identity(c, cell, step, ProbabilityMode::PhraseLocked)
                        }
                        ProbabilityMode::Continuous => step,
                    };
                    let (timing_roll, requested_jitter_ticks) = g.timing_jitter_identity(
                        c.seed,
                        &part.id,
                        decision_identity(c, cell, step, h.mode),
                    );
                    let velocity_roll =
                        decision_roll(c.seed, &part.id, "groove", identity, "humanize_velocity");
                    crate::music::groove::TouchTrace {
                        offbeat,
                        offbeat_factor: if offbeat { g.offbeat_gain } else { 1.0 },
                        after_gap,
                        gap_factor: if after_gap {
                            g.after_gap.as_ref().unwrap().gain
                        } else {
                            1.0
                        },
                        timing_roll,
                        velocity_roll,
                        requested_jitter_ticks,
                        velocity_jitter_factor: 1.0 + (velocity_roll * 2.0 - 1.0) * h.velocity,
                    }
                });
                let requested = offset(&part, c, step);
                let onset =
                    (event.tick as i128 + i128::from(requested)).max(i128::from(lower)) as u64;
                let offset = onset as i64 - event.tick as i64;
                event.tick = onset;
                // Positive groove retains the original gate boundary. Anticipation may cross it.
                event.duration_ticks = event.duration_ticks.min((step + 1) * cell - event.tick - 1);
                event.groove = Some(GrooveTrace {
                    offset_ticks: offset.max(0) as u64,
                    advance_ticks: (-offset).max(0) as u64,
                    requested_gate_ticks: part.output.gate_ticks,
                    ghost_roll: roll,
                    ghost,
                    run_before: before,
                    run_after: after,
                    velocity_factor: g.contour(before, after)
                        * if ghost { g.ghost_gain } else { 1.0 }
                        * touch.as_ref().map_or(1.0, |t| {
                            t.offbeat_factor * t.gap_factor * t.velocity_jitter_factor
                        }),
                    touch,
                });
            }
        }
        if let Some(event) = &mut trace.event {
            // Reserve the next cell's earliest onset (including a possible grace hit).
            let next_tick = ((step + 1) * cell) as i128 + i128::from(offset(&part, c, step + 1));
            let next_first = (next_tick
                - i128::from(part.ornaments.flam.as_ref().map_or(0, |f| f.spacing.0)))
            .max(i128::from(event.tick + 2)) as u64;
            let end = upper.min(next_first);
            event.duration_ticks = event
                .duration_ticks
                .min(cell - 1)
                .min(end.saturating_sub(event.tick + 1))
                .max(1);
            if !part.ornaments.is_default() {
                let (mut hits, ornaments) = part.ornaments.expand(
                    c.seed,
                    &part.id,
                    |mode| decision_identity(c, cell, step, mode),
                    event,
                    cell,
                    lower..end,
                );
                // Store the main hit separately for existing consumers; extras are audible events too.
                if let Some(main) = hits.iter().position(|h| h.tick == event.tick) {
                    *event = hits.remove(main);
                }
                trace.extra_events = hits;
                trace.ornaments = Some(ornaments);
            }
        }
        trace
    }
}
fn position_text(tick: u64) -> String {
    let step = tick / STEP_TICKS;
    format!("{}.{}.{}", step / 16 + 1, step / 4 % 4 + 1, step % 4 + 1)
}
fn offset(part: &Part, c: &Composition, step: u64) -> i64 {
    let cell = part.subdivision.0;
    let swing = if step % 2 == 1 {
        ((part.groove.swing - 0.5) * 2.0 * cell as f64).round() as i64
    } else {
        0
    };
    let mode = part
        .groove
        .humanize
        .as_ref()
        .map_or(ProbabilityMode::PhraseLocked, |h| h.mode);
    let jitter = part
        .groove
        .timing_jitter_identity(c.seed, &part.id, decision_identity(c, cell, step, mode))
        .1;
    (part.groove.delay_ticks + swing + jitter).clamp(
        if part.groove.delay_ticks < 0 {
            -(cell as i64 / 4)
        } else {
            0
        },
        cell as i64 - 2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequential_windows_reuse_decisions_and_interpretation() {
        let c = Composition::parse(
            "tempo=132\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\ngroove.run='ramp_up'",
        )
        .unwrap();
        let mut compiled = Compiled::new(&c);
        compiled.resolve_step(5);
        let raw = compiled.raw.len();
        let cells = compiled.cells.len();
        compiled.resolve_step(6);
        assert_eq!(compiled.raw.len(), raw + 1);
        assert_eq!(compiled.cells.len(), cells + 1);
        let raw = compiled.raw.len();
        compiled.resolve_step(6);
        assert_eq!(compiled.raw.len(), raw);
    }
}
