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
/// Why an authored field did or did not supply a draw in the inspection window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowOutcome {
    OutsideWindow,
    Suspended,
    AdmissionRefused,
    Landed,
}
#[derive(Clone, Debug, PartialEq)]
pub struct InspectedPin {
    pub pin: usize,
    pub tick: u64,
    pub decision: MutationDecision,
    pub outcome: WindowOutcome,
    /// Present when the opportunity falls inside the window, including suspension.
    pub scene: Option<String>,
    /// Absent outside the window or in a suspended scene; zero is a real probability.
    pub chance: Option<f64>,
    pub endpoint_mismatch: bool,
}
/// One record per authored field, including pins that never reached a draw.
/// Each absolute mutation address can land at most once in this window.
#[derive(Clone, Debug, PartialEq)]
pub struct PinInspection {
    pub pattern: String,
    /// Half-open absolute sixteenth-slot window, not mutation opportunity indices.
    pub window: std::ops::Range<u64>,
    pub replayed_slots: u64,
    pub fields: Vec<InspectedPin>,
    /// Join to fields by (tick, admission_pin/selection_pin), never by draw order.
    pub lints: Vec<SelectionUnderFalse>,
}
impl PinInspection {
    pub fn zero_landings(&self) -> impl Iterator<Item = &InspectedPin> {
        self.fields
            .iter()
            .filter(|field| field.outcome != WindowOutcome::Landed)
    }
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

    /// Inspect without advancing this source. Replay a clone from its checkpoint,
    /// including the prefix before `window.start`, but report only inside the window.
    /// The caller supplies the same seed and scene history used for the checkpoint.
    /// `max_replay_slots` bounds prefix + window work and is checked before callbacks.
    /// Storage is bounded by source material and authored fields, not window length.
    /// Empty windows require no replay. A window before the checkpoint is an error.
    pub fn inspect_window(
        &self,
        window: std::ops::Range<u64>,
        seed: u64,
        max_replay_slots: u64,
        mut scene_at: impl FnMut(u64) -> Result<String, String>,
    ) -> Result<PinInspection, String> {
        let start = self.source.state().next_slot();
        if window.start > window.end || window.start < start {
            return Err(format!(
                "retained inspection requires checkpoint {start} <= window start <= window end"
            ));
        }
        let replayed_slots = if window.is_empty() {
            0
        } else {
            window.end - start
        };
        if replayed_slots > max_replay_slots {
            return Err(format!(
                "retained inspection needs {replayed_slots} replay slots, budget is {max_replay_slots}"
            ));
        }
        let mut report = PinInspection {
            pattern: self.source.name.clone(),
            window: window.clone(),
            replayed_slots,
            fields: self
                .pins
                .iter()
                .map(|pin| InspectedPin {
                    pin: pin.pin,
                    tick: pin.tick,
                    decision: pin.decision,
                    outcome: WindowOutcome::OutsideWindow,
                    scene: None,
                    chance: None,
                    endpoint_mismatch: false,
                })
                .collect(),
            lints: self.lints.clone(),
        };
        if replayed_slots == 0 {
            return Ok(report);
        }
        let mut replay = self.clone();
        for slot in start..window.end {
            let scene = scene_at(slot)
                .map_err(|error| format!("retained inspection slot {slot}: {error}"))?;
            let sample = replay
                .advance(&scene, seed)
                .map_err(|error| format!("retained inspection slot {slot}: {error}"))?;
            if slot < window.start {
                continue;
            }
            for landing in sample.landings {
                let field = report
                    .fields
                    .iter_mut()
                    .find(|field| field.pin == landing.pin && field.decision == landing.decision)
                    .expect("landing belongs to a resolved authored field");
                field.outcome = if landing.drawn {
                    WindowOutcome::Landed
                } else if matches!(sample.sample.step.outcome, MutationOutcome::Suspended) {
                    WindowOutcome::Suspended
                } else {
                    WindowOutcome::AdmissionRefused
                };
                field.scene = Some(sample.sample.scene.clone());
                field.chance = sample.sample.chance;
                field.endpoint_mismatch = landing.endpoint_mismatch;
            }
        }
        Ok(report)
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
