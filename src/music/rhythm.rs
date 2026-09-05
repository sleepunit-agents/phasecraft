use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceMode {
    Structural,
    Hits,
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
    Part {
        id: String,
        mode: ReferenceMode,
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
    Part {
        id: String,
        mode: ReferenceMode,
        active: bool,
    },
}
impl RhythmTrace {
    pub fn active(&self) -> bool {
        match self {
            Self::Euclidean { active, .. }
            | Self::Binary { active, .. }
            | Self::Part { active, .. } => *active,
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
            Self::Part { id, .. } => {
                if id.trim().is_empty() {
                    return Err("Part reference id cannot be empty".into());
                }
            }
            Self::Binary { a, b, .. } => {
                a.validate(depth + 1)?;
                b.validate(depth + 1)?;
            }
        }
        Ok(())
    }
    pub fn references(&self) -> Vec<&str> {
        match self {
            Self::Part { id, .. } => vec![id],
            Self::Binary { a, b, .. } => {
                let mut refs = a.references();
                refs.extend(b.references());
                refs
            }
            Self::Euclidean { .. } => vec![],
        }
    }
    pub fn evaluate(
        &self,
        absolute_step: u64,
        phrase_steps: u64,
        reference: &dyn Fn(&str, ReferenceMode) -> bool,
    ) -> RhythmTrace {
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
            Self::Part { id, mode } => RhythmTrace::Part {
                id: id.clone(),
                mode: *mode,
                active: reference(id, *mode),
            },
            Self::Binary { op, a, b } => {
                let a = Box::new(a.evaluate(absolute_step, phrase_steps, reference));
                let b = Box::new(b.evaluate(absolute_step, phrase_steps, reference));
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
