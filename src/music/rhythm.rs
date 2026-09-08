use serde::{Deserialize, Serialize};

pub mod literal;

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
    Literal {
        pattern: String,
        #[serde(default = "literal_cycle")]
        cycle_bars: u32,
        #[serde(default)]
        rotate: i32,
        #[serde(skip)]
        prepared: Option<literal::Prepared>,
    },
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
fn literal_cycle() -> u32 {
    1
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RhythmTrace {
    Literal {
        pattern: String,
        cycle_bars: u32,
        rotate: i32,
        active: bool,
        attack: Option<literal::Attack>,
    },
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
            Self::Literal { active, .. }
            | Self::Euclidean { active, .. }
            | Self::Binary { active, .. }
            | Self::Part { active, .. } => *active,
        }
    }
    pub fn literal_attack(&self) -> Option<literal::Attack> {
        match self {
            Self::Literal { attack, .. } => *attack,
            _ => None,
        }
    }
}
impl Expression {
    /// Prepare literal material at composition load, never during a playback lookup.
    pub fn prepare(&mut self, cell: super::time::NoteValue) -> Result<(), String> {
        match self {
            Self::Literal {
                pattern,
                cycle_bars,
                rotate,
                prepared,
            } => {
                *prepared = Some(literal::Prepared::new(pattern, *cycle_bars, cell, *rotate)?);
            }
            Self::Binary { .. } if self.contains_literal() => {
                return Err("literal material must be the trigger root; boolean combinations do not define which event span and ratchet to keep".into());
            }
            _ => {}
        }
        Ok(())
    }
    pub fn contains_literal(&self) -> bool {
        match self {
            Self::Literal { .. } => true,
            Self::Binary { a, b, .. } => a.contains_literal() || b.contains_literal(),
            _ => false,
        }
    }
    pub fn literal_schedule(&self) -> Option<&literal::Prepared> {
        match self {
            Self::Literal { prepared, .. } => prepared.as_ref(),
            _ => None,
        }
    }
    pub fn validate(&self, depth: usize) -> Result<(), String> {
        if depth > 32 {
            return Err("rhythm expression nesting exceeds 32".into());
        }
        match self {
            Self::Literal {
                pattern,
                cycle_bars,
                rotate,
                prepared,
            } => {
                if depth != 0
                    || !prepared
                        .as_ref()
                        .is_some_and(|p| p.matches(pattern, *cycle_bars, *rotate))
                {
                    return Err("literal material must be prepared as the trigger root for this pattern before playback".into());
                }
            }
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
            Self::Euclidean { .. } | Self::Literal { .. } => vec![],
        }
    }
    pub fn evaluate(
        &self,
        absolute_step: u64,
        phrase_steps: u64,
        reference: &dyn Fn(&str, ReferenceMode) -> bool,
    ) -> RhythmTrace {
        self.evaluate_position(absolute_step, absolute_step % phrase_steps, reference)
    }
    pub fn evaluate_position(
        &self,
        absolute_step: u64,
        phrase_position: u64,
        reference: &dyn Fn(&str, ReferenceMode) -> bool,
    ) -> RhythmTrace {
        match self {
            Self::Literal {
                pattern,
                cycle_bars,
                rotate,
                prepared,
            } => {
                let attack = prepared.as_ref().and_then(|p| p.at_step(absolute_step));
                RhythmTrace::Literal {
                    pattern: pattern.clone(),
                    cycle_bars: *cycle_bars,
                    rotate: *rotate,
                    active: attack.is_some(),
                    attack,
                }
            }
            Self::Euclidean {
                steps,
                pulses,
                rotation,
                reset_on_phrase,
            } => {
                let position = if *reset_on_phrase {
                    phrase_position
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
                let a = Box::new(a.evaluate_position(absolute_step, phrase_position, reference));
                let b = Box::new(b.evaluate_position(absolute_step, phrase_position, reference));
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
