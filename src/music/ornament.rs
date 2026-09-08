//! Bounded, independently admitted expansions of a source hit.
use super::{
    ProbabilityMode,
    resolve::{Dice, Draw, MusicalEvent},
    time::NoteValue,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ornaments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<super::shared::Follower>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratchet: Option<Ratchet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flam: Option<Flam>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ratchet {
    pub count: u8,
    #[serde(default = "one")]
    pub probability: f64,
    #[serde(default)]
    pub probability_mode: ProbabilityMode,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flam {
    /// A grace hit before the main hit, in musical time.
    pub spacing: NoteValue,
    #[serde(default = "gain")]
    pub gain: f64,
    #[serde(default = "one")]
    pub probability: f64,
    #[serde(default)]
    pub probability_mode: ProbabilityMode,
}
fn one() -> f64 {
    1.0
}
fn gain() -> f64 {
    0.5
}
#[derive(Clone, Debug, Serialize)]
pub struct OrnamentTrace {
    pub ratchet_roll: Option<f64>,
    pub flam_roll: Option<f64>,
    pub ratchet_count: u8,
    pub flam_active: bool,
    pub ratchet: Option<ExpansionTrace>,
    pub flam: Option<ExpansionTrace>,
}
/// One ornament gate and its expansion, before neighboring attacks are merged.
/// A refused gate has zero counts; the unornamented source hit is not its output.
/// `pinned` is true when the gate's admission draw was forced by a `[[pins]]` entry:
/// `suppression_reason` still names the mechanism (`probability`), and the flag says
/// the roll it compared was authored, not rolled. Written only when true.
#[derive(Clone, Debug, Serialize)]
pub struct ExpansionTrace {
    pub probability: f64,
    pub admitted_count: u8,
    pub emitted_count: u8,
    pub suppression_reason: Option<SuppressionReason>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
}
fn is_false(v: &bool) -> bool {
    !*v
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionReason {
    Probability,
    LowerBound,
    UpperBound,
}
impl ExpansionTrace {
    fn new(probability: f64, draw: Draw, count: u8) -> Self {
        let admitted = draw.u < probability;
        Self {
            probability,
            admitted_count: if admitted { count } else { 0 },
            emitted_count: 0,
            suppression_reason: (!admitted).then_some(SuppressionReason::Probability),
            pinned: draw.pinned,
        }
    }
}
impl Ornaments {
    pub fn is_default(&self) -> bool {
        self.ratchet.is_none() && self.flam.is_none() && self.gate.is_none()
    }
    pub fn validate(&self, cell: u64) -> Result<(), String> {
        if let Some(r) = &self.ratchet
            && (!(2..=8).contains(&r.count)
                || !probability(r.probability)
                || cell / u64::from(r.count) < 2)
        {
            return Err(
                "ratchet requires count 2..8, probability 0..1 and at least two ticks per repeat"
                    .into(),
            );
        }
        if let Some(f) = &self.flam
            && (!f.spacing.valid()
                || f.spacing.0 >= cell
                || f.spacing.0 > 240
                || !probability(f.gain)
                || !probability(f.probability))
        {
            return Err("flam spacing must be shorter than the Part subdivision and at most 1/16; gain/probability must be 0..1".into());
        }
        Ok(())
    }
    pub fn expand(
        &self,
        dice: Dice,
        id: &str,
        identity: impl Fn(&str, ProbabilityMode) -> u64,
        event: &MusicalEvent,
        cell: u64,
        bounds: std::ops::Range<u64>,
    ) -> (Vec<MusicalEvent>, OrnamentTrace) {
        let (lower, upper) = (bounds.start, bounds.end);
        let roll = |lane, mode| dice.roll(id, lane, identity(lane, mode), "admission");
        let ratchet_draw = self
            .ratchet
            .as_ref()
            .map(|r| roll("ratchet", r.probability_mode));
        let flam_draw = self.flam.as_ref().map(|f| roll("flam", f.probability_mode));
        let ratchet_roll = ratchet_draw.map(|d| d.u);
        let flam_roll = flam_draw.map(|d| d.u);
        let mut ratchet = self
            .ratchet
            .as_ref()
            .map(|r| ExpansionTrace::new(r.probability, ratchet_draw.unwrap(), r.count));
        let mut flam = self
            .flam
            .as_ref()
            .map(|f| ExpansionTrace::new(f.probability, flam_draw.unwrap(), 1));
        let count = ratchet.as_ref().map_or(1, |r| r.admitted_count.max(1));
        let mut hits = Vec::new();
        for i in 0..count {
            let mut hit = event.clone();
            hit.structural_child = i;
            hit.tick = event.tick + u64::from(i) * cell / u64::from(count);
            if hit.tick + 1 >= upper {
                break;
            }
            hit.duration_ticks = hit
                .duration_ticks
                .min((cell / u64::from(count)).saturating_sub(1))
                .min(upper - hit.tick - 1)
                .max(1);
            hits.push(hit);
        }
        if let Some(r) = &mut ratchet
            && r.admitted_count > 0
        {
            r.emitted_count = hits.len() as u8;
            if r.emitted_count < r.admitted_count {
                r.suppression_reason = Some(SuppressionReason::UpperBound);
            }
        }
        let mut flam_active = false;
        if let Some(f) = self.flam.as_ref()
            && flam.as_ref().is_some_and(|f| f.admitted_count > 0)
            && let Some(tick) = event.tick.checked_sub(f.spacing.0).filter(|&t| t >= lower)
        {
            let mut grace = event.clone();
            grace.structural_child = count;
            grace.tick = tick;
            grace.duration_ticks = grace.duration_ticks.min(f.spacing.0 - 1);
            grace.velocity_gain *= f.gain;
            hits.insert(0, grace);
            flam_active = true;
        }
        if let Some(f) = &mut flam
            && f.admitted_count > 0
        {
            f.emitted_count = u8::from(flam_active);
            if !flam_active {
                f.suppression_reason = Some(SuppressionReason::LowerBound);
            }
        }
        (
            hits,
            OrnamentTrace {
                ratchet_roll,
                flam_roll,
                ratchet_count: count,
                flam_active,
                ratchet,
                flam,
            },
        )
    }
}
fn probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
