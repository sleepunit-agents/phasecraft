use super::rhythm::*;
use super::{Composition, Part, ProbabilityMode, STEP_TICKS};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Versioned, length-framed address: reproducible across platforms and builds.
/// Top 53 bits map to [0,1), with no mutable random state.
pub fn decision_roll(seed: u64, part: &str, lane: &str, event: u64, decision: &str) -> f64 {
    let mut hash = Sha256::new();
    hash.update(b"phasecraft/decision/v1\0");
    hash.update(seed.to_le_bytes());
    for value in [part, lane, decision] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(event.to_le_bytes());
    let bytes = hash.finalize();
    let bits = u64::from_le_bytes(bytes[..8].try_into().unwrap()) >> 11;
    bits as f64 / (1u64 << 53) as f64
}
#[derive(Clone, Debug, Serialize)]
pub struct DecisionTrace {
    pub rhythm: RhythmTrace,
    pub event_identity: u64,
    pub probability: f64,
    pub roll: f64,
    pub admitted: bool,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Accent {
    pub active: bool,
    pub amount: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct MusicalEvent {
    pub tick: u64,
    pub duration_ticks: u64,
    pub accent: Accent,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<super::accent::ResolvedControl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groove: Option<super::groove::GrooveTrace>,
}
/// A realized window is generic; source cycles need not fit in the window.
#[derive(Clone, Debug, Serialize)]
pub struct RhythmPattern {
    pub start_tick: u64,
    pub end_tick: u64,
    pub events: Vec<MusicalEvent>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SharedAccentTrace {
    pub name: String,
    pub decision: DecisionTrace,
    pub amount: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct StepTrace {
    pub step: u64,
    pub tick: u64,
    pub position: String,
    pub part: String,
    pub trigger: DecisionTrace,
    pub accent: DecisionTrace,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shared_accents: Vec<SharedAccentTrace>,
    pub event: Option<MusicalEvent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<super::parameter::ParameterTrace>,
}
fn admission(
    c: &Composition,
    part_id: &str,
    step: u64,
    name: &str,
    lane: (&Expression, f64, ProbabilityMode),
    reference: &dyn Fn(&str, ReferenceMode) -> bool,
) -> DecisionTrace {
    let (expression, probability, mode) = lane;
    let event_identity = match mode {
        ProbabilityMode::PhraseLocked => step % c.phrase_steps(),
        ProbabilityMode::Continuous => step,
    };
    let rhythm = expression.evaluate(step, c.phrase_steps(), reference);
    let roll = decision_roll(c.seed, part_id, name, event_identity, "admission");
    let admitted = rhythm.active() && roll < probability;
    DecisionTrace {
        rhythm,
        event_identity,
        probability,
        roll,
        admitted,
    }
}
fn resolve_part(
    c: &Composition,
    part: &Part,
    step: u64,
    reference: &dyn Fn(&str, ReferenceMode) -> bool,
) -> StepTrace {
    let trigger = admission(
        c,
        &part.id,
        step,
        "trigger",
        (
            &part.trigger.rhythm,
            part.trigger.probability,
            part.trigger.probability_mode,
        ),
        reference,
    );
    let accent = admission(
        c,
        &part.id,
        step,
        "accent",
        (
            &part.accent.rhythm,
            part.accent.probability,
            part.accent.probability_mode,
        ),
        reference,
    );
    let shared_accents: Vec<_> = part
        .accent
        .sources
        .iter()
        .map(|name| {
            let lane = &c.accents[name];
            let event_identity = match lane.probability_mode {
                ProbabilityMode::PhraseLocked => step % c.phrase_steps(),
                ProbabilityMode::Continuous => step,
            };
            let rhythm = lane.rhythm.evaluate(step, c.phrase_steps(), &|_, _| false);
            let roll = decision_roll(
                c.seed,
                name,
                "shared_accent",
                event_identity,
                "shared_admission",
            );
            let admitted = rhythm.active() && roll < lane.probability;
            SharedAccentTrace {
                name: name.clone(),
                amount: if admitted { lane.amount } else { 0.0 },
                decision: DecisionTrace {
                    rhythm,
                    event_identity,
                    probability: lane.probability,
                    roll,
                    admitted,
                },
            }
        })
        .collect();
    let combined_active = accent.admitted || shared_accents.iter().any(|s| s.decision.admitted);
    let combined_amount = shared_accents.iter().map(|s| s.amount).fold(
        if accent.admitted {
            part.accent.amount
        } else {
            0.0
        },
        f64::max,
    );
    let event = trigger.admitted.then(|| MusicalEvent {
        tick: step * STEP_TICKS,
        duration_ticks: part.output.gate_ticks,
        groove: None,
        controls: part
            .profile
            .controls
            .iter()
            .filter(|(name, response)| {
                !part.parameters.contains_key(*name) && response.envelope.is_none()
            })
            .map(|(name, response)| {
                let output = &part.output.controls[name];
                let amount = response.value(combined_amount);
                super::accent::ResolvedControl {
                    name: name.clone(),
                    amount,
                    channel: output.channel.unwrap_or(part.output.channel),
                    cc: output.cc,
                    value: super::accent::midi_value(amount),
                    reset: super::accent::midi_value(response.base),
                }
            })
            .collect(),
        accent: Accent {
            active: combined_active,
            amount: combined_amount,
        },
    });
    StepTrace {
        step,
        tick: step * STEP_TICKS,
        position: format!("{}.{}.{}", step / 16 + 1, step / 4 % 4 + 1, step % 4 + 1),
        part: part.id.clone(),
        parameters: Vec::new(),
        shared_accents,
        trigger,
        accent,
        event,
    }
}
pub fn realize(c: &Composition, part: &Part, start_step: u64, steps: u64) -> RhythmPattern {
    RhythmPattern {
        start_tick: start_step * STEP_TICKS,
        end_tick: (start_step + steps) * STEP_TICKS,
        events: (start_step..start_step + steps)
            .filter_map(|s| resolve(c, part, s).event)
            .collect(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    pub tick: u64,
    pub bytes: [u8; 3],
    /// CC onset registers a resting value for stop/error cleanup.
    pub reset_value: Option<u8>,
    /// Neutral target restored on Stop, independently of per-note emphasis resets.
    pub stop_value: Option<u8>,
    /// Timeline sample: deduplicate values and skip obsolete samples on dispatch.
    pub parameter: bool,
}
pub fn to_midi(part: &Part, event: &MusicalEvent) -> Vec<MidiEvent> {
    let output = &part.output;
    let velocity = (f64::from(part.profile.base)
        * event.groove.as_ref().map_or(1.0, |g| g.velocity_factor)
        + if event.accent.active {
            event.accent.amount * f64::from(part.profile.boost)
        } else {
            0.0
        })
    .round()
    .clamp(1.0, 127.0) as u8;
    let mut events = vec![
        MidiEvent {
            tick: event.tick,
            bytes: [0x90 | (output.channel - 1), output.note, velocity],
            stop_value: None,
            reset_value: None,
            parameter: false,
        },
        MidiEvent {
            tick: event.tick + event.duration_ticks,
            bytes: [0x80 | (output.channel - 1), output.note, 0],
            stop_value: None,
            reset_value: None,
            parameter: false,
        },
    ];
    for control in &event.controls {
        events.push(MidiEvent {
            tick: event.tick,
            bytes: [0xb0 | (control.channel - 1), control.cc, control.value],
            stop_value: output.controls[&control.name]
                .default
                .map(super::accent::midi_value),
            reset_value: Some(control.reset),
            parameter: false,
        });
        events.push(MidiEvent {
            tick: event.tick + event.duration_ticks,
            bytes: [0xb0 | (control.channel - 1), control.cc, control.reset],
            stop_value: None,
            reset_value: None,
            parameter: false,
        });
    }
    events
}

/// Merge one grid position before dispatch. Sending each Part's on/off pair
/// immediately would delay simultaneous hits behind the first Part's note-off.
fn resolve_raw(c: &Composition, step: u64) -> Vec<StepTrace> {
    let parts = c
        .evaluation_order()
        .expect("resolve_step requires a validated composition");
    let mut resolved = std::collections::BTreeMap::<String, StepTrace>::new();

    for part in parts {
        let reference = |id: &str, mode| {
            let target = &resolved[id];
            match mode {
                ReferenceMode::Structural => target.trigger.rhythm.active(),
                ReferenceMode::Hits => target.trigger.admitted,
            }
        };
        let trace = resolve_part(c, part, step, &reference);
        resolved.insert(part.id.clone(), trace);
    }
    resolved.into_values().collect()
}

/// Groove stays inside the source sixteenth; even long gates cannot overlap batches.
/// Part references always see source-grid admissions, independent of groove interpretation.
pub fn resolve_step(c: &Composition, step: u64) -> (Vec<StepTrace>, Vec<MidiEvent>) {
    use super::groove::{GrooveTrace, RunContour};
    let mut traces = resolve_raw(c, step);
    let run_context = c.parts.iter().any(|p| p.groove.run != RunContour::None);
    let lookbehind = c
        .parts
        .iter()
        .filter_map(|p| p.groove.after_gap.as_ref().map(|g| u64::from(g.steps)))
        .chain(
            c.parts
                .iter()
                .flat_map(|p| p.profile.controls.values())
                .filter_map(|r| r.envelope.as_ref().map(|e| e.history_steps())),
        )
        .max()
        .unwrap_or(0)
        .max(if run_context { 2 } else { 0 });
    let mut neighbors = std::collections::BTreeMap::new();
    let positions = (1..=lookbehind)
        .filter_map(|n| step.checked_sub(n))
        .chain((1..=if run_context { 2 } else { 0 }).filter_map(|n| step.checked_add(n)));
    for s in positions {
        if s < u64::MAX / STEP_TICKS {
            neighbors.insert(s, resolve_raw(c, s));
        }
    }
    let mut midi = Vec::with_capacity(c.parts.len() * 2);
    for trace in &mut traces {
        let part = c.parts.iter().find(|p| p.id == trace.part).unwrap();
        if let Some(event) = &mut trace.event {
            let g = &part.groove;
            if !g.is_default() {
                let fired = |s: Option<u64>| {
                    s.and_then(|s| neighbors.get(&s)).is_some_and(|traces| {
                        traces
                            .iter()
                            .any(|t| t.part == part.id && t.trigger.admitted)
                    })
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
                    ProbabilityMode::PhraseLocked => step % c.phrase_steps(),
                    ProbabilityMode::Continuous => step,
                };
                let roll = decision_roll(c.seed, &part.id, "groove", identity, "ghost");
                let ghost = !event.accent.active && roll < g.ghost_probability;
                let touch = (g.offbeat_gain != 1.0
                    || g.after_gap.is_some()
                    || g.humanize.is_some())
                .then(|| {
                    let offbeat = step % 4 == 2;
                    let after_gap = g.after_gap.as_ref().is_some_and(|gap| {
                        step >= u64::from(gap.steps)
                            && (1..=u64::from(gap.steps)).all(|n| !fired(step.checked_sub(n)))
                    });
                    let h = g.humanize.clone().unwrap_or_default();
                    let identity = match h.mode {
                        ProbabilityMode::PhraseLocked => step % c.phrase_steps(),
                        ProbabilityMode::Continuous => step,
                    };
                    let (timing_roll, requested_jitter_ticks) =
                        g.timing_jitter(c.seed, &part.id, step, c.phrase_steps());
                    let velocity_roll =
                        decision_roll(c.seed, &part.id, "groove", identity, "humanize_velocity");
                    super::groove::TouchTrace {
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
                let offset = (g.offset(step) as i64
                    + touch.as_ref().map_or(0, |t| t.requested_jitter_ticks))
                .clamp(0, STEP_TICKS as i64 - 2) as u64;
                event.tick += offset;
                event.duration_ticks = event.duration_ticks.min(STEP_TICKS - offset - 1);
                event.groove = Some(GrooveTrace {
                    offset_ticks: offset,
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
            midi.extend(to_midi(part, event));
        }
        let mut history = Vec::new();
        if part.profile.controls.values().any(|r| r.envelope.is_some()) {
            for (&s, past) in &neighbors {
                if s >= step {
                    continue;
                }
                if let Some(event) = past
                    .iter()
                    .find(|t| t.part == part.id)
                    .and_then(|t| t.event.as_ref())
                    .filter(|e| e.accent.active)
                {
                    history.push((
                        event.tick
                            + part
                                .groove
                                .onset_offset(c.seed, &part.id, s, c.phrase_steps()),
                        event.accent.amount,
                    ));
                }
            }
            if let Some(event) = trace.event.as_ref().filter(|e| e.accent.active) {
                history.push((event.tick, event.accent.amount));
            }
        }
        let (parameters, controls) =
            super::parameter::resolve(part, step, trace.event.as_ref(), &history);
        trace.parameters = parameters;
        midi.extend(controls);
    }
    midi.sort_by_key(|event| {
        let priority = match event.bytes[0] & 0xf0 {
            0x80 => 0, // release the previous note first
            0xb0 => 1, // establish controls before the next attack
            _ => 2,
        };
        (event.tick, priority, event.bytes)
    });
    (traces, midi)
}

/// Resolve one member Part with the same dependency graph used by live playback.
pub fn resolve(c: &Composition, part: &Part, step: u64) -> StepTrace {
    resolve_step(c, step)
        .0
        .into_iter()
        .find(|trace| trace.part == part.id)
        .expect("resolve requires a member of the validated composition")
}
