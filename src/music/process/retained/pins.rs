//! Authored mutation pins for the retained source API, not the Composition pin table.
use super::{MutationDecision, RetainedPattern, RetainedSample, RetainedSource};
use crate::music::{
    process::{MutationOutcome, mutation_index_roll},
    resolve::{Draw, MAX_PINS, decision_roll},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MutationPin {
    pub at: MutationPinAt,
    pub admit: Option<bool>,
    pub rest_index: Option<usize>,
    pub hit_index: Option<usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MutationPinAt {
    pub roll: MutationRoll,
    pub pattern: String,
    /// Zero-based transport opportunity, including suspended/refused opportunities.
    pub tick: u64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationRoll {
    Mutate,
}
#[derive(Clone, Debug)]
struct Resolved {
    pin: usize,
    tick: u64,
    decision: MutationDecision,
    u: f64,
    admit: Option<bool>,
}
/// A lint, not a rejection: selection draws can still land at chance = 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionUnderFalse {
    pub admission_pin: usize,
    pub selection_pin: usize,
    pub tick: u64,
}
/// A requested field at this opportunity, including fields skipped by suspension/refusal.
#[derive(Clone, Debug, PartialEq)]
pub struct PinLanding {
    /// Index in the authored mutation-pin slice passed to `bind_pinned`.
    pub pin: usize,
    pub decision: MutationDecision,
    pub drawn: bool,
    /// True only when a consulted boolean disagrees with admission at p=0 or p=1.
    /// The enclosing sample carries the actual scene, chance and outcome.
    pub endpoint_mismatch: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PinnedSample {
    pub sample: RetainedSample,
    pub landings: Vec<PinLanding>,
}
/// Prepared source and immutable, source-bound pins. Clone to preserve a replay checkpoint.
#[derive(Clone, Debug)]
pub struct PinnedSource {
    source: RetainedSource,
    pins: Vec<Resolved>,
    lints: Vec<SelectionUnderFalse>,
}
impl RetainedPattern {
    /// Resolve a mutation-only pin slice at load. Other patterns and duplicate draw
    /// addresses are errors; distinct fields may be split across authored entries.
    pub fn bind_pinned(
        &self,
        name: &str,
        scenes: &[&str],
        pins: &[MutationPin],
    ) -> Result<PinnedSource, String> {
        if pins.len() > MAX_PINS {
            return Err(format!(
                "retained source supports at most {MAX_PINS} mutation pins"
            ));
        }
        let source = self.bind(name, scenes)?;
        let hits = self.initial.material().iter().filter(|&&hit| hit).count();
        let rests = self.initial.material().len() - hits;
        let mut resolved = Vec::new();
        let mut addresses = BTreeSet::new();
        for (index, pin) in pins.iter().enumerate() {
            let error = |message: &str| format!("mutation pin {index}: {message}");
            if pin.at.pattern != name {
                return Err(error("unknown pattern for this source"));
            }
            if pin
                .at
                .tick
                .checked_mul(u64::from(self.file.change.every.slots))
                .and_then(|slot| slot.checked_add(1))
                .is_none()
            {
                return Err(error("tick exceeds transport slot range"));
            }
            if pin.admit.is_none() && pin.rest_index.is_none() && pin.hit_index.is_none() {
                return Err(error("requires at least one draw"));
            }
            let values = [
                (
                    MutationDecision::Admit,
                    pin.admit
                        .map(|admit| if admit { 0.0 } else { 1.0_f64.next_down() }),
                ),
                (
                    MutationDecision::RestIndex,
                    pin.rest_index
                        .map(|i| mutation_index_roll(i, rests))
                        .transpose()
                        .map_err(|e| error(&e))?,
                ),
                (
                    MutationDecision::HitIndex,
                    pin.hit_index
                        .map(|i| mutation_index_roll(i, hits))
                        .transpose()
                        .map_err(|e| error(&e))?,
                ),
            ];
            for (decision, u) in values {
                if let Some(u) = u {
                    if !addresses.insert((pin.at.tick, decision.key())) {
                        return Err(error("duplicate mutation draw address"));
                    }
                    resolved.push(Resolved {
                        pin: index,
                        tick: pin.at.tick,
                        decision,
                        u,
                        admit: pin.admit.filter(|_| decision == MutationDecision::Admit),
                    });
                }
            }
        }
        let mut lints = Vec::new();
        for admission in resolved.iter().filter(|p| p.admit == Some(false)) {
            let selection_pins: BTreeSet<_> = resolved
                .iter()
                .filter(|p| p.tick == admission.tick && p.decision != MutationDecision::Admit)
                .map(|p| p.pin)
                .collect();
            for selection_pin in selection_pins {
                lints.push(SelectionUnderFalse {
                    admission_pin: admission.pin,
                    selection_pin,
                    tick: admission.tick,
                });
            }
        }
        Ok(PinnedSource {
            source,
            pins: resolved,
            lints,
        })
    }
}
impl PinnedSource {
    pub fn source(&self) -> &RetainedSource {
        &self.source
    }
    pub fn lints(&self) -> &[SelectionUnderFalse] {
        &self.lints
    }

    /// Advance once per transport sixteenth. `Draw.pinned` now indexes the authored
    /// mutation-pin slice, so several decisions may carry the same index.
    /// Reports only this opportunity; callers aggregate zero-landings over their window.
    pub fn advance(&mut self, scene: &str, seed: u64) -> Result<PinnedSample, String> {
        let sample = self.source.advance_draws(scene, |name, tick, decision| {
            match self
                .pins
                .iter()
                .find(|pin| pin.tick == tick && pin.decision == decision)
            {
                Some(pin) => Draw {
                    u: pin.u,
                    pinned: Some(pin.pin),
                },
                None => Draw {
                    u: decision_roll(seed, name, "mutate", tick, decision.key()),
                    pinned: None,
                },
            }
        })?;
        let mut landings = Vec::new();
        if let Some(tick) = sample.step.occurrence {
            for pin in self.pins.iter().filter(|pin| pin.tick == tick) {
                let drawn = sample
                    .draws
                    .iter()
                    .any(|draw| draw.decision == pin.decision && draw.draw.pinned == Some(pin.pin));
                let admitted = matches!(sample.step.outcome, MutationOutcome::Swapped { .. });
                landings.push(PinLanding {
                    pin: pin.pin,
                    decision: pin.decision,
                    drawn,
                    endpoint_mismatch: drawn
                        && pin.admit.is_some_and(|expected| expected != admitted),
                });
            }
        }
        Ok(PinnedSample { sample, landings })
    }
}
