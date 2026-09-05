use crate::config::{Composition, ProbabilityMode, STEP_TICKS};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOp {
    Or,
    And,
    Xor,
    ANotB,
    BNotA,
}
impl BooleanOp {
    pub fn apply(self, a: bool, b: bool) -> bool {
        match self {
            Self::Or => a || b,
            Self::And => a && b,
            Self::Xor => a ^ b,
            Self::ANotB => a && !b,
            Self::BNotA => b && !a,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expression {
    Euclidean {
        steps: u32,
        pulses: u32,
        #[serde(default)]
        rotation: i32,
        #[serde(default)]
        reset_on_phrase: bool,
    },
    Binary {
        op: BooleanOp,
        a: Box<Expression>,
        b: Box<Expression>,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RhythmTrace {
    Euclidean {
        steps: u32,
        pulses: u32,
        rotation: i32,
        phase: u64,
        active: bool,
    },
    Binary {
        op: BooleanOp,
        a: Box<RhythmTrace>,
        b: Box<RhythmTrace>,
        active: bool,
    },
}
impl RhythmTrace {
    pub fn active(&self) -> bool {
        match self {
            Self::Euclidean { active, .. } | Self::Binary { active, .. } => *active,
        }
    }
}
impl Expression {
    pub fn validate(&self, depth: usize) -> Result<(), String> {
        if depth > 32 {
            return Err("rhythm expression nesting exceeds 32".into());
        }
        match self {
            Self::Euclidean { steps, pulses, .. } => {
                if *steps == 0 || *steps > 65536 || pulses > steps {
                    return Err("Euclidean requires 1 <= steps <= 65536 and pulses <= steps".into());
                }
            }
            Self::Binary { a, b, .. } => {
                a.validate(depth + 1)?;
                b.validate(depth + 1)?;
            }
        }
        Ok(())
    }
    pub fn evaluate(&self, absolute_step: u64, phrase_steps: u64) -> RhythmTrace {
        match self {
            Self::Euclidean {
                steps,
                pulses,
                rotation,
                reset_on_phrase,
            } => {
                let position = if *reset_on_phrase {
                    absolute_step % phrase_steps
                } else {
                    absolute_step
                };
                let phase = position % u64::from(*steps);
                let index =
                    (phase as i64 - i64::from(*rotation)).rem_euclid(i64::from(*steps)) as u64;
                // Balanced modular Euclidean convention: first pulse at zero;
                // positive rotation delays. No LCM-sized pattern allocation.
                let active = (index * u64::from(*pulses)) % u64::from(*steps) < u64::from(*pulses);
                RhythmTrace::Euclidean {
                    steps: *steps,
                    pulses: *pulses,
                    rotation: *rotation,
                    phase,
                    active,
                }
            }
            Self::Binary { op, a, b } => {
                let a = Box::new(a.evaluate(absolute_step, phrase_steps));
                let b = Box::new(b.evaluate(absolute_step, phrase_steps));
                let active = op.apply(a.active(), b.active());
                RhythmTrace::Binary {
                    op: *op,
                    a,
                    b,
                    active,
                }
            }
        }
    }
}

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
    step: u64,
    name: &str,
    expression: &Expression,
    probability: f64,
    mode: ProbabilityMode,
) -> DecisionTrace {
    let event_identity = match mode {
        ProbabilityMode::PhraseLocked => step % c.phrase_steps(),
        ProbabilityMode::Continuous => step,
    };
    let rhythm = expression.evaluate(step, c.phrase_steps());
    let roll = decision_roll(c.seed, &c.part.id, name, event_identity, "admission");
    let admitted = rhythm.active() && roll < probability;
    DecisionTrace {
        rhythm,
        event_identity,
        probability,
        roll,
        admitted,
    }
}
pub fn resolve(c: &Composition, step: u64) -> StepTrace {
    let trigger = admission(
        c,
        step,
        "trigger",
        &c.part.trigger.rhythm,
        c.part.trigger.probability,
        c.part.trigger.probability_mode,
    );
    let accent = admission(
        c,
        step,
        "accent",
        &c.part.accent.rhythm,
        c.part.accent.probability,
        c.part.accent.probability_mode,
    );
    let event = trigger.admitted.then(|| MusicalEvent {
        tick: step * STEP_TICKS,
        duration_ticks: c.part.output.gate_ticks,
        accent: Accent {
            active: accent.admitted,
            amount: if accent.admitted {
                c.part.accent.amount
            } else {
                0.0
            },
        },
    });
    StepTrace {
        step,
        tick: step * STEP_TICKS,
        position: format!("{}.{}.{}", step / 16 + 1, step / 4 % 4 + 1, step % 4 + 1),
        part: c.part.id.clone(),
        trigger,
        accent,
        event,
    }
}
pub fn realize(c: &Composition, start_step: u64, steps: u64) -> RhythmPattern {
    RhythmPattern {
        start_tick: start_step * STEP_TICKS,
        end_tick: (start_step + steps) * STEP_TICKS,
        events: (start_step..start_step + steps)
            .filter_map(|s| resolve(c, s).event)
            .collect(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    pub tick: u64,
    pub bytes: [u8; 3],
}
pub fn to_midi(c: &Composition, event: &MusicalEvent) -> [MidiEvent; 2] {
    let output = &c.part.output;
    let velocity = (f64::from(c.part.profile.base)
        + if event.accent.active {
            event.accent.amount * f64::from(c.part.profile.boost)
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
