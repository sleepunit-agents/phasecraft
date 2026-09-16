//! Numeric velocity sequences and chronological retained rhythm mutation.
//! Structural admission owns the event clock; rendering does not.
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod retained;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Per {
    #[default]
    Step,
    Event,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    Main,
    #[default]
    Attacks,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Carry {
    Always,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "File", into = "File")]
pub struct ValuePattern {
    file: File,
    values: Arc<[f64]>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct File {
    pattern: String,
    #[serde(default)]
    per: Per,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clock: Option<Clock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    carry: Option<Carry>,
    #[serde(default = "cycle")]
    cycle: u32,
}
fn cycle() -> u32 {
    16
}
impl TryFrom<File> for ValuePattern {
    type Error = String;
    fn try_from(file: File) -> Result<Self, String> {
        if !(1..=65536).contains(&file.cycle) {
            return Err("velocity.cycle must be 1..65536 sixteenth-note slots".into());
        }
        if file.per != Per::Event && (file.clock.is_some() || file.carry.is_some()) {
            return Err("velocity clock/carry require per = \"event\"".into());
        }
        super::notation::Pattern::parse(&file.pattern)
            .map_err(|e| format!("velocity.pattern: {e}"))?;
        let mut values = Vec::new();
        for word in file.pattern.split_whitespace() {
            if values.len() == 4096 {
                return Err("velocity.pattern supports at most 4096 values".into());
            }
            let value: f64 = word.parse().map_err(|_| "velocity.pattern currently requires a flat list of numbers; nested notation, rests and draws are not supported")?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err("velocity.pattern values must be finite and within 0..1".into());
            }
            values.push(value);
        }
        if values.is_empty() {
            return Err("velocity.pattern must contain a value".into());
        }
        Ok(Self {
            file,
            values: values.into(),
        })
    }
}
impl From<ValuePattern> for File {
    fn from(p: ValuePattern) -> Self {
        p.file
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ValueRead {
    /// Main is 0; tails follow; an admitted flam is last, regardless of sounding time.
    pub child: u8,
    pub index: usize,
    pub value: f64,
}
impl ValuePattern {
    pub fn per(&self) -> Per {
        self.file.per
    }
    pub fn clock(&self) -> Clock {
        self.file.clock.unwrap_or_default()
    }
    pub fn carries(&self) -> bool {
        self.file.carry.is_some()
    }
    pub fn cycle_ticks(&self) -> u64 {
        u64::from(self.file.cycle) * super::STEP_TICKS
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn read(&self, child: u8, position: u64) -> ValueRead {
        let index = if self.per() == Per::Event {
            (position % self.len() as u64) as usize
        } else {
            // The notation floors each onset i * cycle / len. Invert that inequality,
            // not the unfloored phase: e.g. 7 values over 3840 ticks advance at 548.
            (((u128::from(position % self.cycle_ticks()) + 1) * self.len() as u128)
                .div_ceil(u128::from(self.cycle_ticks()))
                - 1) as usize
        };
        ValueRead {
            child,
            index,
            value: self.values[index],
        }
    }
}

/// A carried index belongs to the voice ID. Every active occurrence must retain a
/// compatible schema; absence freezes it. Values may change without changing the index space.
pub fn validate_carry(c: &super::Composition) -> Result<(), String> {
    let snapshots: Vec<_> = if let Some(a) = &c.arrangement {
        a.sections.iter().map(|s| s.composition.as_ref()).collect()
    } else if let Some(r) = &c.router {
        r.scenes.iter().map(|s| s.composition.as_ref()).collect()
    } else {
        return Ok(());
    };
    for owner in snapshots.iter().flat_map(|s| &s.parts) {
        let Some(value) = owner.velocity.as_ref().filter(|v| v.carries()) else {
            continue;
        };
        for other in snapshots
            .iter()
            .filter_map(|s| s.parts.iter().find(|p| p.id == owner.id))
        {
            if other.velocity.as_ref().is_none_or(|v| {
                !v.carries() || v.len() != value.len() || v.clock() != value.clock()
            }) {
                return Err(format!(
                    "Part {:?}: carried velocity requires the same value count and clock in every active scene/section; absent Parts freeze",
                    owner.id
                ));
            }
        }
    }
    Ok(())
}

/// Application boundaries for retained rhythm edits, measured from transport slot zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Apply {
    Bar,
    Cycle,
}

/// The caller supplies draws by (transport occurrence, decision), independently of history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationDecision {
    Admit,
    RestIndex,
    HitIndex,
}

/// Convert a forced zero-based selection index to a roll for `RetainedSwap::advance`.
/// `count` is the number of eligible rests or hits, not the material length.
/// The midpoint avoids rounding below the index's lower bucket boundary.
pub fn mutation_index_roll(index: usize, count: usize) -> Result<f64, String> {
    if !(1..=4096).contains(&count) || index >= count {
        return Err("retained swap index requires count 1..4096 and index < count".into());
    }
    Ok((index as f64 + 0.5) / count as f64)
}

#[derive(Clone, Debug, PartialEq)]
pub enum MutationOutcome {
    NotDue,
    Suspended,
    Refused {
        admit: f64,
    },
    Swapped {
        admit: f64,
        rest_roll: f64,
        hit_roll: f64,
        /// Zero-based indices in the ascending rest/hit lists before this swap.
        rest_index: usize,
        hit_index: usize,
        /// Zero-based material slots, selected in ascending order over pending material.
        rest_slot: usize,
        hit_slot: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MutationStep {
    pub slot: u64,
    /// Zero-based transport opportunity, including suspended and refused opportunities.
    pub occurrence: Option<u64>,
    /// A pending copy was published before this slot's mutation opportunity.
    pub committed: bool,
    pub outcome: MutationOutcome,
}

/// M1.5's bounded swap evaluator. One call advances exactly one sixteenth-note slot.
/// Composition playback shares this state through the retained transport.
#[derive(Clone, Debug)]
pub struct RetainedSwap {
    material: Vec<bool>,
    pending: Option<Vec<bool>>,
    every: u32,
    apply_slots: u32,
    next_slot: u64,
}

impl RetainedSwap {
    pub fn new(initial: &[bool], every: u32, apply: Apply) -> Result<Self, String> {
        if initial.is_empty() || initial.len() > 4096 {
            return Err("retained swap requires 1..4096 initial slots".into());
        }
        if !initial.contains(&true) || !initial.contains(&false) {
            return Err("retained swap requires at least one hit and one rest".into());
        }
        if !(1..=65536).contains(&every) {
            return Err("retained swap every must be 1..65536 slots".into());
        }
        Ok(Self {
            material: initial.to_vec(),
            pending: None,
            every,
            apply_slots: match apply {
                Apply::Bar => 16,
                Apply::Cycle => initial.len() as u32,
            },
            next_slot: 0,
        })
    }

    /// The pattern currently audible, never the uncommitted edits.
    pub fn material(&self) -> &[bool] {
        &self.material
    }

    /// Pending material is exposed for diagnostics; callers cannot mutate it.
    pub fn pending(&self) -> Option<&[bool]> {
        self.pending.as_deref()
    }

    pub fn next_slot(&self) -> u64 {
        self.next_slot
    }

    /// Supply the incoming scene's probability at every slot. None suspends without
    /// drawing; Some(0) still draws and refuses. Commits happen even in a suspended
    /// scene. At coincident boundaries the commit precedes the mutation, so that edit
    /// waits for the following boundary. Occurrence zero is at transport slot zero.
    ///
    /// Draws must be finite in [0, 1). Invalid inputs leave this evaluator unchanged;
    /// side effects in the caller's draw function cannot be rolled back. Playback
    /// integration must supply addressable draws, not a sequential random generator.
    pub fn advance(
        &mut self,
        chance: Option<f64>,
        mut draw: impl FnMut(u64, MutationDecision) -> f64,
    ) -> Result<MutationStep, String> {
        if chance.is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p)) {
            return Err("retained swap chance must be finite and within 0..1".into());
        }
        let slot = self.next_slot;
        let next_slot = slot
            .checked_add(1)
            .ok_or("retained swap transport exhausted")?;
        let boundary = slot.is_multiple_of(u64::from(self.apply_slots));
        let committed = boundary && self.pending.is_some();
        let occurrence = slot
            .is_multiple_of(u64::from(self.every))
            .then_some(slot / u64::from(self.every));
        // Both a boundary commit and an accumulating edit read the latest pending
        // copy. Delay all state changes until every required draw has been checked.
        let current = self.pending.as_deref().unwrap_or(&self.material);
        let mut checked_draw = |occurrence, decision| {
            let value = draw(occurrence, decision);
            if !value.is_finite() || !(0.0..1.0).contains(&value) {
                Err(format!(
                    "retained swap {decision:?} draw must be finite and within [0, 1)"
                ))
            } else {
                Ok(value)
            }
        };
        let outcome = match (occurrence, chance) {
            (None, _) => MutationOutcome::NotDue,
            (Some(_), None) => MutationOutcome::Suspended,
            (Some(occurrence), Some(chance)) => {
                let admit = checked_draw(occurrence, MutationDecision::Admit)?;
                if admit >= chance {
                    MutationOutcome::Refused { admit }
                } else {
                    let rest_roll = checked_draw(occurrence, MutationDecision::RestIndex)?;
                    let hit_roll = checked_draw(occurrence, MutationDecision::HitIndex)?;
                    let hits = current.iter().filter(|&&hit| hit).count();
                    let rests = current.len() - hits;
                    let rest_index = (rest_roll * rests as f64) as usize;
                    let hit_index = (hit_roll * hits as f64) as usize;
                    let select = |hit, index| {
                        current
                            .iter()
                            .enumerate()
                            .filter(|(_, value)| **value == hit)
                            .nth(index)
                            .unwrap()
                            .0
                    };
                    MutationOutcome::Swapped {
                        admit,
                        rest_roll,
                        hit_roll,
                        rest_index,
                        hit_index,
                        rest_slot: select(false, rest_index),
                        hit_slot: select(true, hit_index),
                    }
                }
            }
        };
        if committed {
            self.material = self.pending.take().unwrap();
        }
        if let MutationOutcome::Swapped {
            rest_slot,
            hit_slot,
            ..
        } = outcome
        {
            let pending = self.pending.get_or_insert_with(|| self.material.clone());
            pending[rest_slot] = true;
            pending[hit_slot] = false;
        }
        self.next_slot = next_slot;
        Ok(MutationStep {
            slot,
            occurrence,
            committed,
            outcome,
        })
    }
}
