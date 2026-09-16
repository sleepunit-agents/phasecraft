//! Prepared hat-memory source. The compiled transport supplies scene traversal
//! for Composition readers; standalone callers may drive the source directly.
pub mod pins;
pub(crate) mod playback;
use super::{Apply, Carry, MutationDecision, MutationStep, RetainedSwap};
use crate::music::resolve::{Dice, Draw};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The closed, flat swap source used by until-stop's hat-memory file.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "File", into = "File")]
pub struct RetainedPattern {
    file: File,
    initial: RetainedSwap,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct File {
    initial: String,
    elsewhere: Elsewhere,
    carry: Carry,
    change: Change,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Elsewhere {
    Hold,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Change {
    every: Every,
    chance: BTreeMap<String, f64>,
    moves: Vec<Move>,
    apply: Application,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Every {
    slots: u32,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Move {
    Swap,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Application {
    on: Apply,
}
impl TryFrom<File> for RetainedPattern {
    type Error = String;
    fn try_from(file: File) -> Result<Self, String> {
        // Bound collection before allocating material. This deliberately accepts only
        // flat sixteenth x/~ slots, not the literal grammar's nesting or ornaments.
        let mut slots = Vec::new();
        for token in file.initial.split_whitespace() {
            if slots.len() == 4096 {
                return Err("retained pattern initial supports at most 4096 slots".into());
            }
            slots.push(match token {
                "x" => true,
                "~" => false,
                _ => return Err("retained pattern initial requires flat x/~ slots".into()),
            });
        }
        if file.change.moves.len() != 1 {
            return Err("retained pattern moves must be exactly [\"swap\"]".into());
        }
        if file.change.chance.len() > 64 {
            return Err("retained pattern chance supports at most 64 scenes".into());
        }
        for (scene, probability) in &file.change.chance {
            if scene.trim().is_empty() {
                return Err("retained pattern chance requires nonempty scene names".into());
            }
            if !probability.is_finite() || !(0.0..=1.0).contains(probability) {
                return Err(format!(
                    "retained pattern chance for {scene:?} must be finite and within 0..1"
                ));
            }
        }
        let initial = RetainedSwap::new(&slots, file.change.every.slots, file.change.apply.on)?;
        Ok(Self { file, initial })
    }
}
impl From<RetainedPattern> for File {
    fn from(pattern: RetainedPattern) -> Self {
        pattern.file
    }
}
impl RetainedPattern {
    /// Bind the stable source name and the complete scene vocabulary once, at load.
    /// An absent chance entry suspends a declared scene; an unknown scene is an error.
    pub fn bind(&self, name: &str, scenes: &[&str]) -> Result<RetainedSource, String> {
        if name.trim().is_empty() {
            return Err("retained pattern requires a nonempty source name".into());
        }
        if scenes.is_empty() || scenes.len() > 64 {
            return Err("retained pattern requires 1..64 declared scenes".into());
        }
        let mut declared = BTreeSet::new();
        for scene in scenes {
            if scene.trim().is_empty() || !declared.insert((*scene).to_owned()) {
                return Err("retained pattern requires unique nonempty scene names".into());
            }
        }
        for scene in self.file.change.chance.keys() {
            if !declared.contains(scene) {
                return Err(format!(
                    "retained pattern {name:?}: unknown chance scene {scene:?}"
                ));
            }
        }
        Ok(RetainedSource {
            name: name.to_owned(),
            scenes: declared,
            chance: self.file.change.chance.clone(),
            state: self.initial.clone(),
        })
    }
}

/// One chronological source shared by all readers, including scenes with no reader.
#[derive(Clone, Debug)]
pub struct RetainedSource {
    name: String,
    scenes: BTreeSet<String>,
    chance: BTreeMap<String, f64>,
    state: RetainedSwap,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedDraw {
    pub decision: MutationDecision,
    /// Includes the caller's resolved pin-list index, if a pin supplied this draw.
    pub draw: Draw,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedSample {
    pub pattern: String,
    pub scene: String,
    pub chance: Option<f64>,
    pub step: MutationStep,
    /// Only decisions actually requested, in admission/rest/hit order (at most three).
    pub draws: Vec<RetainedDraw>,
}
impl RetainedSource {
    pub fn state(&self) -> &RetainedSwap {
        &self.state
    }

    /// Advance one absolute sixteenth. Hashes (name, "mutate", opportunity, decision)
    /// with the existing versioned decision hash. Seed/pins must be stable across replay.
    /// Invalid scenes or draws leave the state unchanged, including boundary commits.
    pub fn advance(&mut self, scene: &str, dice: Dice<'_>) -> Result<RetainedSample, String> {
        self.advance_draws(scene, |name, occurrence, decision| {
            dice.roll(name, "mutate", occurrence, decision.key())
        })
    }

    fn advance_draws(
        &mut self,
        scene: &str,
        mut draw: impl FnMut(&str, u64, MutationDecision) -> Draw,
    ) -> Result<RetainedSample, String> {
        if !self.scenes.contains(scene) {
            return Err(format!(
                "retained pattern {:?}: unknown scene {scene:?}",
                self.name
            ));
        }
        let chance = self.chance.get(scene).copied();
        let mut draws = Vec::new();
        let step = self.state.advance(chance, |occurrence, decision| {
            let draw = draw(&self.name, occurrence, decision);
            draws.push(RetainedDraw { decision, draw });
            draw.u
        })?;
        Ok(RetainedSample {
            pattern: self.name.clone(),
            scene: scene.to_owned(),
            chance,
            step,
            draws,
        })
    }
}

impl MutationDecision {
    fn key(self) -> &'static str {
        match self {
            Self::Admit => "admit",
            Self::RestIndex => "rest_index",
            Self::HitIndex => "hit_index",
        }
    }
}
