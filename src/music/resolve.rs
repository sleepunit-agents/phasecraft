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
}
/// A realized window is generic; source cycles need not fit in the window.
#[derive(Clone, Debug, Serialize)]
pub struct RhythmPattern {
    pub start_tick: u64,
    pub end_tick: u64,
    pub events: Vec<MusicalEvent>,
}
#[derive(Clone, Debug, Serialize)]
pub struct StepTrace {
    pub step: u64,
    pub tick: u64,
    pub position: String,
    pub part: String,
    pub trigger: DecisionTrace,
    pub accent: DecisionTrace,
    pub event: Option<MusicalEvent>,
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
    let event = trigger.admitted.then(|| MusicalEvent {
        tick: step * STEP_TICKS,
        duration_ticks: part.output.gate_ticks,
        accent: Accent {
            active: accent.admitted,
            amount: if accent.admitted {
                part.accent.amount
            } else {
                0.0
            },
        },
    });
    StepTrace {
        step,
        tick: step * STEP_TICKS,
        position: format!("{}.{}.{}", step / 16 + 1, step / 4 % 4 + 1, step % 4 + 1),
        part: part.id.clone(),
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
}
pub fn to_midi(part: &Part, event: &MusicalEvent) -> [MidiEvent; 2] {
    let output = &part.output;
    let velocity = (f64::from(part.profile.base)
        + if event.accent.active {
            event.accent.amount * f64::from(part.profile.boost)
        } else {
            0.0
        })
    .round()
    .clamp(1.0, 127.0) as u8;
    [
        MidiEvent {
            tick: event.tick,
            bytes: [0x90 | (output.channel - 1), output.note, velocity],
        },
        MidiEvent {
            tick: event.tick + event.duration_ticks,
            bytes: [0x80 | (output.channel - 1), output.note, 0],
        },
    ]
}

/// Merge one grid position before dispatch. Sending each Part's on/off pair
/// immediately would delay simultaneous hits behind the first Part's note-off.
pub fn resolve_step(c: &Composition, step: u64) -> (Vec<StepTrace>, Vec<MidiEvent>) {
    let parts = c
        .evaluation_order()
        .expect("resolve_step requires a validated composition");
    let mut resolved = std::collections::BTreeMap::<String, StepTrace>::new();
    let mut midi = Vec::with_capacity(parts.len() * 2);
    for part in parts {
        let reference = |id: &str, mode| {
            let target = &resolved[id];
            match mode {
                ReferenceMode::Structural => target.trigger.rhythm.active(),
                ReferenceMode::Hits => target.trigger.admitted,
            }
        };
        let trace = resolve_part(c, part, step, &reference);
        if let Some(event) = &trace.event {
            midi.extend(to_midi(part, event));
        }
        resolved.insert(part.id.clone(), trace);
    }
    let traces = resolved.into_values().collect();
    // Gates are shorter than a step, so successive step batches cannot overlap.
    // Equal deadlines are ordered by MIDI bytes (off before on, then route).
    midi.sort_by_key(|event| (event.tick, event.bytes));
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
