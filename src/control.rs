//! Controller-independent, temporary edits for a single selected Part.
//! The transport commits a complete validated Part at a bar boundary.
use crate::music::{
    Composition, Part,
    parameter::ParameterLane,
    rhythm::{BooleanOp, Expression},
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
pub type Shared = Arc<Mutex<Live>>;
#[derive(Clone, Default)]
pub struct Live {
    base: Option<Composition>,
    edited: Option<Part>,
    pub revision: u64,
    pub generation: u16,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Parameter {
    Level,
    Cutoff,
    TriggerProbability,
    AccentAmount,
    ASteps,
    APulses,
    ARotation,
    Operator,
    BSteps,
    BPulses,
    BRotation,
    AccentProbability,
    CSteps,
    CPulses,
    CRotation,
    Decay,
}
pub const PARAMETERS: [Parameter; 16] = [
    Parameter::Level,
    Parameter::Cutoff,
    Parameter::TriggerProbability,
    Parameter::AccentAmount,
    Parameter::ASteps,
    Parameter::APulses,
    Parameter::ARotation,
    Parameter::Operator,
    Parameter::BSteps,
    Parameter::BPulses,
    Parameter::BRotation,
    Parameter::AccentProbability,
    Parameter::CSteps,
    Parameter::CPulses,
    Parameter::CRotation,
    Parameter::Decay,
];
#[derive(Clone, Serialize)]
pub struct Value {
    pub parameter: Parameter,
    pub label: String,
    pub value: f64,
    pub maximum: f64,
    pub enabled: bool,
}
#[derive(Clone, Serialize)]
pub struct View {
    pub generation: u16,
    pub revision: u64,
    pub edited: bool,
    pub values: Vec<Value>,
}
fn euclid() -> Expression {
    Expression::Euclidean {
        steps: 16,
        pulses: 0,
        rotation: 0,
        reset_on_phrase: false,
    }
}
fn components(p: &Part) -> Option<(Expression, Expression, u8)> {
    match &p.trigger.rhythm {
        a @ Expression::Euclidean { .. } => Some((a.clone(), euclid(), 0)),
        Expression::Binary { op, a, b }
            if matches!(**a, Expression::Euclidean { .. })
                && matches!(**b, Expression::Euclidean { .. }) =>
        {
            Some((
                (**a).clone(),
                (**b).clone(),
                match op {
                    BooleanOp::Or => 1,
                    BooleanOp::And => 2,
                    BooleanOp::Xor => 3,
                    BooleanOp::ANotB => 4,
                    BooleanOp::BNotA => 5,
                },
            ))
        }
        _ => None,
    }
}
fn number(e: &Expression, k: usize) -> Option<(f64, f64)> {
    if let Expression::Euclidean {
        steps,
        pulses,
        rotation,
        ..
    } = e
    {
        Some(match k {
            0 => (*steps as f64, 65536.),
            1 => (*pulses as f64, *steps as f64),
            _ => (
                rotation.rem_euclid(*steps as i32) as f64,
                (*steps - 1) as f64,
            ),
        })
    } else {
        None
    }
}
fn adjust(e: &mut Expression, k: usize, delta: i32) {
    if let Expression::Euclidean {
        steps,
        pulses,
        rotation,
        ..
    } = e
    {
        match k {
            0 => {
                *steps = (*steps as i64 + i64::from(delta)).clamp(1, 65536) as u32;
                *pulses = (*pulses).min(*steps);
                *rotation = rotation.rem_euclid(*steps as i32);
            }
            1 => *pulses = (*pulses as i64 + i64::from(delta)).clamp(0, i64::from(*steps)) as u32,
            _ => *rotation = (*rotation + i32::from(delta as i16)).rem_euclid(*steps as i32),
        }
    }
}
impl Live {
    pub fn load(&mut self, c: Option<Composition>) {
        self.base = c;
        self.reset();
    }
    pub fn rebase(&mut self, c: Composition) {
        if self
            .base
            .as_ref()
            .is_none_or(|base| serde_json::to_vec(base).ok() != serde_json::to_vec(&c).ok())
        {
            self.edited = None;
        }
        self.base = Some(c);
        self.generation = (self.generation + 1) % 16384;
        self.revision += 1;
    }
    pub fn reset(&mut self) {
        self.edited = None;
        self.revision += 1;
        self.generation = (self.generation + 1) % 16384;
    }
    pub fn composition(&self) -> Option<Composition> {
        let mut c = self.base.clone()?;
        if let Some(p) = &self.edited
            && let Some(target) = c.parts.iter_mut().find(|v| v.id == p.id)
        {
            *target = p.clone();
        }
        Some(c)
    }
    pub fn view(&self, part: &str) -> View {
        let c = self.composition();
        let p = c
            .as_ref()
            .filter(|c| c.arrangement.is_none())
            .and_then(|c| c.parts.iter().find(|p| p.id == part));
        let values = PARAMETERS
            .iter()
            .enumerate()
            .map(|(i, &parameter)| {
                let label = [
                    "Levl", "Cut", "Trig", "Amt", "ASte", "APul", "ARot", "Comb", "BSte", "BPul",
                    "BRot", "Acc", "CSte", "CPul", "CRot", "Decy",
                ][i];
                let v = p.and_then(|p| match i {
                    0 | 1 | 15 => {
                        let n = match i {
                            0 => "level",
                            1 => "cutoff",
                            _ => "decay",
                        };
                        p.output.controls.get(n).and_then(|out| {
                            p.parameters
                                .get(n)
                                .map(|v| v.value)
                                .or_else(|| p.profile.controls.get(n).map(|v| v.base))
                                .or(out.default)
                                .map(|value| (value, 1.))
                        })
                    }
                    2 => Some((p.trigger.probability, 1.)),
                    3 => Some((p.accent.amount, 1.)),
                    11 => Some((p.accent.probability, 1.)),
                    4..=10 => components(p).and_then(|(a, b, op)| match i {
                        4..=6 => number(&a, i - 4),
                        7 => Some((f64::from(op), 5.)),
                        _ => number(&b, i - 8),
                    }),
                    12..=14 => number(&p.accent.rhythm, i - 12),
                    _ => None,
                });
                Value {
                    parameter,
                    label: label.into(),
                    value: v.map(|v| v.0).unwrap_or(0.),
                    maximum: v.map(|v| v.1).unwrap_or(1.),
                    enabled: v.is_some(),
                }
            })
            .collect();
        View {
            generation: self.generation,
            revision: self.revision,
            edited: self.edited.is_some(),
            values,
        }
    }
    pub fn change(&mut self, part: &str, parameter: Parameter, delta: i32) -> Result<(), String> {
        if delta == 0 {
            return Ok(());
        }
        if !(-64..=64).contains(&delta) {
            return Err("control delta out of range".into());
        }
        let i = PARAMETERS
            .iter()
            .position(|p| std::mem::discriminant(p) == std::mem::discriminant(&parameter))
            .unwrap();
        if !self.view(part).values[i].enabled {
            return Err(
                "parameter unavailable (first controller slice supports loop compositions)".into(),
            );
        }
        let mut c = self.composition().ok_or("open a project first")?;
        let p = c
            .parts
            .iter_mut()
            .find(|p| p.id == part)
            .ok_or("Part not found")?;
        match i {
            0 | 1 | 15 => {
                let n = match i {
                    0 => "level",
                    1 => "cutoff",
                    _ => "decay",
                };
                let value =
                    (self.view(part).values[i].value + f64::from(delta) / 100.).clamp(0., 1.);
                p.parameters.insert(
                    n.into(),
                    ParameterLane {
                        value,
                        ramp: None,
                        automation: None,
                    },
                );
            }
            2 => {
                p.trigger.probability =
                    (p.trigger.probability + f64::from(delta) / 100.).clamp(0., 1.)
            }
            3 => p.accent.amount = (p.accent.amount + f64::from(delta) / 100.).clamp(0., 1.),
            11 => {
                p.accent.probability =
                    (p.accent.probability + f64::from(delta) / 100.).clamp(0., 1.)
            }
            12..=14 => adjust(&mut p.accent.rhythm, i - 12, delta),
            4..=10 => {
                let (mut a, mut b, mut op) = components(p).ok_or("unsupported expression")?;
                match i {
                    4..=6 => adjust(&mut a, i - 4, delta),
                    7 => op = (i32::from(op) + delta).clamp(0, 5) as u8,
                    _ => {
                        adjust(&mut b, i - 8, delta);
                        if op == 0 {
                            op = 1;
                        }
                    }
                }
                p.trigger.rhythm = if op == 0 {
                    a
                } else {
                    Expression::Binary {
                        op: match op {
                            1 => BooleanOp::Or,
                            2 => BooleanOp::And,
                            3 => BooleanOp::Xor,
                            4 => BooleanOp::ANotB,
                            _ => BooleanOp::BNotA,
                        },
                        a: Box::new(a),
                        b: Box::new(b),
                    }
                };
            }
            _ => unreachable!(),
        }
        let edited = p.clone();
        c.validate()?;
        self.edited = Some(edited);
        self.revision += 1;
        Ok(())
    }
}
/// Restore controls whose live lane disappeared, even when the new score is silent.
/// The old snapshot is the last actually scheduled one, so producer stalls are safe.
pub fn removed_control_resets(
    previous: &Composition,
    next: &Composition,
    tick: u64,
) -> Vec<crate::music::resolve::MidiEvent> {
    let mut removed = previous.clone();
    for p in &mut removed.parts {
        let exists = |name: &str, parameter: bool| {
            let output = &p.output.controls[name];
            next.parts.iter().any(|n| {
                n.parameters
                    .keys()
                    .chain(n.profile.controls.keys().filter(|_| !parameter))
                    .any(|key| {
                        let o = &n.output.controls[key];
                        o.cc == output.cc
                            && o.channel.unwrap_or(n.output.channel)
                                == output.channel.unwrap_or(p.output.channel)
                    })
            })
        };
        p.parameters.retain(|name, _| !exists(name, true));
        p.profile.controls.retain(|name, _| !exists(name, false));
    }
    crate::music::arrangement::resets(&removed, tick)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn live() -> Live {
        let c = Composition::parse(include_str!("../examples/quickstart/hat.toml")).unwrap();
        let mut l = Live::default();
        l.load(Some(c));
        l
    }
    #[test]
    fn edits_validate_and_reset_without_changing_seed_or_other_lanes() {
        let mut l = live();
        let id = l.base.as_ref().unwrap().parts[0].id.clone();
        let original = l.composition().unwrap();
        l.change(&id, Parameter::TriggerProbability, -30).unwrap();
        let changed = l.composition().unwrap();
        assert_eq!(changed.seed, original.seed);
        assert_eq!(
            changed.parts[0].accent.probability,
            original.parts[0].accent.probability
        );
        assert!(changed.parts[0].trigger.probability < original.parts[0].trigger.probability);
        l.reset();
        assert_eq!(
            serde_json::to_string(&l.composition()).unwrap(),
            serde_json::to_string(&Some(original)).unwrap()
        );
    }
    #[test]
    fn unsupported_part_and_unmapped_volume_are_not_velocity() {
        let mut l = live();
        assert!(l.change("missing", Parameter::ASteps, 1).is_err());
        let id = l.base.as_ref().unwrap().parts[0].id.clone();
        assert!(l.change(&id, Parameter::Level, 1).is_err());
    }
}
