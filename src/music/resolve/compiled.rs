use super::*;

/// Compiled dependency order and bounded raw-decision cache, owned by one immutable snapshot.
pub struct Compiled {
    shared_seed: u64,
    shared_cache: crate::music::shared::Cache,
    transport_shift: u64,
    composition: Composition,
    order: Vec<usize>,
    indices: std::collections::BTreeMap<String, usize>,
    raw: std::collections::BTreeMap<u64, std::sync::Arc<Vec<StepTrace>>>,
    sections: Vec<Compiled>,
    scenes: Vec<Compiled>,
    period: u64,
    value_base: std::collections::BTreeMap<String, u64>,
    carry_history: std::collections::BTreeMap<u64, std::collections::BTreeMap<String, u64>>,
    value_prefix: std::collections::BTreeMap<(usize, u64, u64), u64>,
    moves: Vec<crate::music::router::Move>,
    cell_keys: std::collections::VecDeque<(usize, u64, u64, u64, u64)>,
    cells: std::collections::BTreeMap<(usize, u64, u64, u64, u64), StepTrace>,
}
impl Compiled {
    pub fn new(c: &Composition) -> Self {
        Self::with_shared_seed(c, c.seed)
    }
    fn with_shared_seed(c: &Composition, shared_seed: u64) -> Self {
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
                    .map(|s| Self::with_shared_seed(&s.composition, shared_seed))
                    .collect()
            })
            .unwrap_or_default();
        let scenes = c
            .router
            .as_ref()
            .map(|r| {
                r.scenes
                    .iter()
                    .map(|s| Self::with_shared_seed(&s.composition, shared_seed))
                    .collect()
            })
            .unwrap_or_default();
        let period = c
            .router
            .as_ref()
            .map_or(0, |r| r.period_ticks(c).expect("validated composition"));
        Self {
            shared_seed,
            shared_cache: Default::default(),
            transport_shift: 0,
            composition: c.clone(),
            order,
            indices,
            raw: Default::default(),
            cells: Default::default(),
            cell_keys: Default::default(),
            sections,
            scenes,
            period,
            value_prefix: Default::default(),
            value_base: Default::default(),
            carry_history: Default::default(),
            moves: Vec::new(),
        }
    }
    fn set_transport_shift(&mut self, shift: u64) {
        if self.transport_shift != shift {
            self.transport_shift = shift;
            self.cells.clear();
            self.cell_keys.clear();
            self.value_prefix.clear();
        }
    }
    fn lane_samples(&mut self, tick: u64) -> Vec<crate::music::shared::Sample> {
        self.composition
            .lanes
            .iter()
            .map(|(name, lane)| lane.sample(name, self.shared_seed, tick, &mut self.shared_cache))
            .collect()
    }
    /// The router's move log through the return that `tick` falls in.
    pub fn moves_through(&mut self, tick: u64) -> &[crate::music::router::Move] {
        if let Some(r) = &self.composition.router {
            r.extend_moves(
                self.composition.seed,
                self.period,
                &mut self.moves,
                tick / self.period,
            );
        }
        &self.moves
    }
    /// Reconstruct carried residues at a scene/section boundary. A missing Part consumes
    /// nothing. Checkpoints bound storage; evicted history is replayed, never approximated.
    fn carry_at(&mut self, target: u64) -> std::collections::BTreeMap<String, u64> {
        if !self.sections.iter().chain(self.scenes.iter()).any(|c| {
            c.composition
                .parts
                .iter()
                .any(|p| p.velocity.as_ref().is_some_and(|v| v.carries()))
        }) {
            return Default::default();
        }
        let (mut cursor, mut state) = self
            .carry_history
            .range(..=target)
            .next_back()
            .map(|(&t, s)| (t, s.clone()))
            .unwrap_or_default();
        while cursor < target {
            if let Some(a) = &self.composition.arrangement {
                let Some(located) = a.locate(cursor / STEP_TICKS) else {
                    break;
                };
                let index = located.position.index - 1;
                let length = u64::from(located.section.bars) * 16 - located.local_step;
                let end = (cursor + length * STEP_TICKS).min(target);
                let from = located.musical_step * STEP_TICKS;
                self.sections[index].set_transport_shift(cursor - from);
                self.sections[index].spend_carried(from, from + end - cursor, &mut state);
                cursor = end;
            } else if let Some(r) = &self.composition.router {
                r.extend_moves(
                    self.composition.seed,
                    self.period,
                    &mut self.moves,
                    cursor / self.period,
                );
                let visit = r.locate(self.period, cursor, &self.moves);
                let index = r.scenes.iter().position(|s| s.name == visit.scene).unwrap();
                let end = visit.next_tick.min(target);
                self.scenes[index].spend_carried(cursor, end, &mut state);
                cursor = end;
            } else {
                break;
            }
            if self.carry_history.len() >= 4096 {
                self.carry_history.pop_first();
            }
            self.carry_history.insert(cursor, state.clone());
        }
        state
    }
    fn spend_carried(
        &mut self,
        from: u64,
        to: u64,
        state: &mut std::collections::BTreeMap<String, u64>,
    ) {
        for index in 0..self.composition.parts.len() {
            let part = self.composition.parts[index].clone();
            let Some(value) = &part.velocity else {
                continue;
            };
            if !value.carries() {
                continue;
            }
            let n = self.value_position(index, from, to.div_ceil(part.subdivision.0));
            let entry = state.entry(part.id).or_default();
            *entry = (*entry + n) % value.len() as u64;
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
        if let Some(r) = &self.composition.router {
            // Every visit continues the transport's phase: no shift, no restart. The window
            // opens where this run of the scene was entered and closes at the next return.
            let tick = step * STEP_TICKS;
            r.extend_moves(
                self.composition.seed,
                self.period,
                &mut self.moves,
                tick / self.period,
            );
            let visit = r.locate(self.period, tick, &self.moves);
            let index = r
                .scenes
                .iter()
                .position(|s| s.name == visit.scene)
                .expect("validated router");
            // A move resets the outgoing scene's declared controls; a stay resets nothing.
            let previous = (tick == visit.entered_tick && visit.index > 0)
                .then(|| {
                    let from = visit.from.as_deref()?;
                    let scene = r.scene(from)?;
                    Some(crate::music::arrangement::resets(&scene.composition, tick))
                })
                .flatten();
            self.scenes[index].value_base = self.carry_at(visit.entered_tick);
            let (mut traces, mut midi) =
                self.scenes[index].window(step, visit.entered_tick, visit.next_tick);
            for t in &mut traces {
                t.scene = Some(visit.clone());
            }
            midi.extend(previous.into_iter().flatten());
            midi.sort_by_key(midi_order);
            return (traces, midi);
        }
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
            self.sections[index].value_base = self.carry_at((step - local_step) * STEP_TICKS);
            self.sections[index].set_transport_shift(shift);
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
        let samples = self.lane_samples(start + self.transport_shift);
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
            let mut first = start
                .saturating_sub((envelope_steps + 1) * STEP_TICKS)
                .max(lower)
                / cell;
            // Literal tails/gates may occupy their whole written span. Bars still own
            // them, so the current bar's structural sources are sufficient for any seek.
            if part.trigger.rhythm.literal_schedule().is_some() {
                first = first.min(start / (16 * STEP_TICKS) * (16 * STEP_TICKS) / cell);
            }
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
            for (owner, main, mut event) in attacks {
                // Every attack — main, tail, grace — has its final tick here, so this is where
                // the pitch lanes are sampled, once, and the trace and the wire read one field.
                event.note = crate::music::pitch::sounding_note(&part, event.tick);
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
            let (mut parameters, controls) =
                crate::music::parameter::resolve_window(&part, start, end, &audible, &history);
            midi.extend(controls);
            if (start + self.transport_shift).is_multiple_of(4 * crate::music::PPQN) {
                for (name, param) in &part.parameters {
                    let Some(f) = &param.follow else {
                        continue;
                    };
                    let output = &part.output.controls[name];
                    let lane = &self.composition.lanes[&f.follows];
                    let sample = samples.iter().find(|s| s.name == f.follows).unwrap();
                    let base = f.mapped(lane, sample.value, output.default.unwrap_or(1.0));
                    let amount = base.clamp(0.0, 1.0);
                    let value = crate::music::accent::midi_value(amount);
                    let channel = output.channel.unwrap_or(part.output.channel);
                    midi.push(MidiEvent {
                        tick: start,
                        bytes: [0xb0 | (channel - 1), output.cc, value],
                        reset_value: None,
                        boundary_reset: false,
                        parameter: true,
                        stop_value: Some(crate::music::accent::midi_value(
                            output.default.unwrap_or(0.0),
                        )),
                    });
                    parameters.push(crate::music::parameter::ParameterTrace {
                        name: name.clone(),
                        channel,
                        cc: output.cc,
                        samples: vec![crate::music::parameter::ParameterSample {
                            tick: start,
                            base,
                            emphasis: 0.0,
                            amount,
                            value,
                            automation: None,
                            envelope: None,
                        }],
                    });
                }
            }
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
            if cell != STEP_TICKS
                || !part.ornaments.is_default()
                || anticipates(&part)
                || part.trigger.rhythm.literal_schedule().is_some()
            {
                part_traces[0].sounding = Some(audible);
            }
            part_traces[0].parameters = parameters;
            part_traces[0].lanes = samples.clone();
            traces.extend(part_traces);
        }
        midi.sort_by_key(midi_order);
        (traces, midi)
    }
    fn cell(&mut self, index: usize, step: u64, lower: u64, upper: u64) -> StepTrace {
        let key = (
            index,
            step,
            lower,
            upper,
            *self
                .value_base
                .get(&self.composition.parts[index].id)
                .unwrap_or(&0),
        );
        if let Some(trace) = self.cells.get(&key) {
            return trace.clone();
        }
        let mut trace = self.compute_cell(index, step, lower, upper);
        self.sample_values(index, step, lower, &mut trace);
        if self.cells.len() >= 4096
            && let Some(old) = self.cell_keys.pop_front()
        {
            self.cells.remove(&old);
        }
        self.cells.insert(key, trace.clone());
        self.cell_keys.push_back(key);
        trace
    }
    // Structural counts are independent of value reads, timing and ownership. Prefixes
    // are residues, not a finite lookbehind: eviction can require replay but never resets history.
    fn value_position(&mut self, index: usize, origin: u64, step: u64) -> u64 {
        let part = self.composition.parts[index].clone();
        let value = part.velocity.as_ref().unwrap();
        let modulus = value.len() as u64;
        let first = origin.div_ceil(part.subdivision.0);
        let mut cursor = first;
        let mut position = 0;
        if let Some((&(i, o, s), &p)) = self
            .value_prefix
            .range(..=(index, origin, step))
            .next_back()
            && i == index
            && o == origin
            && s >= first
        {
            cursor = s;
            position = p;
        }
        while cursor < step {
            let trace = self.compute_cell(index, cursor, 0, u64::MAX);
            let (count, _) = structural_children(&trace);
            let spend = if value.clock() == super::super::process::Clock::Main {
                u64::from(count > 0)
            } else {
                u64::from(count)
            };
            position = (position + spend) % modulus;
            cursor += 1;
            if cursor.is_multiple_of(64) {
                self.save_value_prefix((index, origin, cursor), position);
            }
        }
        self.save_value_prefix((index, origin, step), position);
        position
    }
    fn save_value_prefix(&mut self, key: (usize, u64, u64), value: u64) {
        if self.value_prefix.len() >= 4096 {
            self.value_prefix.pop_first();
        }
        self.value_prefix.insert(key, value);
    }
    fn sample_values(&mut self, index: usize, step: u64, lower: u64, trace: &mut StepTrace) {
        use super::super::process::{Clock, Per};
        let part = self.composition.parts[index].clone();
        let Some(value) = &part.velocity else {
            return;
        };
        let (count, ratchets) = structural_children(trace);
        if count == 0 {
            return;
        }
        let source = step * part.subdivision.0;
        let span = trace
            .trigger
            .rhythm
            .literal_attack()
            .map_or(part.subdivision.0, |a| a.span_ticks);
        let origin = if value.carries() {
            lower
        } else {
            (source / value.cycle_ticks() * value.cycle_ticks()).max(lower)
        };
        let position = if value.per() == Per::Event {
            (self.value_position(index, origin, step)
                + if value.carries() {
                    *self.value_base.get(&part.id).unwrap_or(&0)
                } else {
                    0
                })
                % value.len() as u64
        } else {
            source
        };
        for child in 0..count {
            let at = if value.per() == Per::Event {
                position
                    + if value.clock() == Clock::Attacks {
                        u64::from(child)
                    } else {
                        0
                    }
            } else if child < ratchets {
                source + u64::from(child) * span / u64::from(ratchets)
            } else {
                source.saturating_sub(part.ornaments.flam.as_ref().unwrap().spacing.0)
            };
            trace.values.push(value.read(child, at));
        }
        for event in trace.event.iter_mut().chain(trace.extra_events.iter_mut()) {
            let read = trace.values[usize::from(event.structural_child)].clone();
            event.velocity_gain *= read.value;
            event.velocity = Some(read);
        }
    }
    fn compute_cell(&mut self, index: usize, step: u64, lower: u64, upper: u64) -> StepTrace {
        let part = self.composition.parts[index].clone();
        let cell = part.subdivision.0;
        // Bars own their attacks and releases, so snapshot swaps never strand a tail.
        let bar = crate::music::PPQN * 4;
        let lower = lower.max(step * cell / bar * bar);
        let upper = upper.min((step * cell / bar + 1) * bar);
        let mut trace = self.raw_at(step * cell)[index].clone();
        let gate_probability = part.ornaments.gate.as_ref().map(|f| {
            let lane = &self.composition.lanes[&f.follows];
            let value = lane
                .sample(
                    &f.follows,
                    self.shared_seed,
                    step * cell + self.transport_shift,
                    &mut self.shared_cache,
                )
                .value;
            f.mapped(lane, value, 1.0)
        });
        let literal = trace.trigger.rhythm.literal_attack();
        let span = literal.map_or(cell, |a| a.span_ticks);
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
                let roll = c.dice().roll(&part.id, "groove", identity, "ghost").u;
                let ghost = !event.accent.active && roll < g.ghost_probability;
                let touch = g.draws_touch().then(|| {
                    let offbeat = (step * cell) % crate::music::PPQN == crate::music::PPQN / 2;
                    let after_gap = g.after_gap.as_ref().is_some_and(|gap| {
                        step >= u64::from(gap.steps)
                            && (1..=u64::from(gap.steps)).all(|n| !fired(step.checked_sub(n)))
                    });
                    let h = g.humanize.clone().unwrap_or_default();
                    // One address for both touch draws, and the one `resolve_pins` resolves a
                    // `timing` or `velocity` pin to.
                    let identity = decision_identity(c, cell, step, g.touch_mode());
                    let (timing_roll, requested_jitter_ticks) =
                        g.timing_jitter_identity(c.dice(), &part.id, identity);
                    let velocity_roll = c
                        .dice()
                        .roll(&part.id, "groove", identity, "humanize_velocity")
                        .u;
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
                event.duration_ticks = event
                    .duration_ticks
                    .min((step * cell + span).saturating_sub(event.tick + 1).max(1));
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
            // Literal rests are not possible attacks. Reserve the next structural main
            // onset, including its possible grace, rather than an intervening empty cell.
            let next_source = match part.trigger.rhythm.literal_schedule() {
                Some(p) => p.schedule().next_after(step * cell).unwrap_or(upper),
                None => (step + 1) * cell,
            };
            let next_tick = next_source as i128 + i128::from(offset(&part, c, next_source / cell));
            let next_first = (next_tick
                - i128::from(part.ornaments.flam.as_ref().map_or(0, |f| f.spacing.0)))
            .max(i128::from(event.tick + 2)) as u64;
            let end = upper.min(next_first);
            event.duration_ticks = event
                .duration_ticks
                .min(span - 1)
                .min(end.saturating_sub(event.tick + 1))
                .max(1);
            let mut expansion = expansion_for(&part, literal);
            if let (Some(probability), Some(ratchet)) = (gate_probability, &mut expansion.ratchet) {
                ratchet.probability = probability;
                ratchet.probability_mode = ProbabilityMode::Continuous;
            }
            if !expansion.is_default() {
                let (mut hits, ornaments) = expansion.expand(
                    c.dice(),
                    &part.id,
                    |lane, mode| {
                        let source = if lane == "ratchet" && gate_probability.is_some() {
                            (step * cell + self.transport_shift) / cell
                        } else {
                            step
                        };
                        decision_identity(c, cell, source, mode)
                    },
                    event,
                    span,
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
    let jitter = part
        .groove
        .timing_jitter_identity(
            c.dice(),
            &part.id,
            decision_identity(c, cell, step, part.groove.touch_mode()),
        )
        .1;
    (part.groove.delay_ticks + swing + jitter).clamp(
        if anticipates(part) {
            -(cell as i64 / 4)
        } else {
            0
        },
        cell as i64 - 2,
    )
}
/// A groove that asks to move early opens the anticipation window (a quarter of the
/// subdivision): a negative delay, or humanize jitter, which is two-sided by contract.
/// Without either, the floor stays at the source onset. Bar ownership still applies
/// on top of this: a bar's first hit never reaches back into the previous bar.
fn anticipates(part: &Part) -> bool {
    part.groove.delay_ticks < 0
        || part
            .groove
            .humanize
            .as_ref()
            .is_some_and(|h| h.timing_ticks > 0)
}

/// Number of admitted structural children, including the main. A refused burst retains
/// the main; ownership suppression never changes these counts.
fn structural_children(trace: &StepTrace) -> (u8, u8) {
    if !trace.trigger.admitted {
        return (0, 0);
    }
    let ratchets = trace
        .ornaments
        .as_ref()
        .and_then(|o| o.ratchet.as_ref())
        .map_or(1, |r| r.admitted_count.max(1));
    let grace = trace
        .ornaments
        .as_ref()
        .and_then(|o| o.flam.as_ref())
        .map_or(0, |f| f.admitted_count);
    (ratchets + grace, ratchets)
}
fn expansion_for(
    part: &Part,
    literal: Option<crate::music::rhythm::literal::Attack>,
) -> crate::music::ornament::Ornaments {
    let mut expansion = part.ornaments.clone();
    if let Some(a) = literal {
        expansion.ratchet = (a.ratchet > 1).then(|| {
            let configured = part.ornaments.ratchet.as_ref();
            crate::music::ornament::Ratchet {
                count: a.ratchet,
                probability: configured.map_or(
                    if a.draw == crate::music::notation::Draw::Tail {
                        0.5
                    } else {
                        1.0
                    },
                    |r| r.probability,
                ),
                probability_mode: configured
                    .map_or(ProbabilityMode::PhraseLocked, |r| r.probability_mode),
            }
        });
    }
    expansion
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
