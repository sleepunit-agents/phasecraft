//! Deterministic event interpretation, after trigger/accent decisions and before MIDI.
use super::{ProbabilityMode, STEP_TICKS};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunContour {
    #[default]
    None,
    RampUp,
    LowHighLow,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Groove {
    /// Share of each eighth-note pair occupied by its first sixteenth.
    pub swing: f64,
    /// Additional laid-back offset, in musical ticks (non-negative).
    pub delay_ticks: u64,
    pub run: RunContour,
    pub ghost_probability: f64,
    pub ghost_gain: f64,
    pub ghost_mode: ProbabilityMode,
    #[serde(skip_serializing_if = "is_one")]
    pub offbeat_gain: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_gap: Option<GapResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub humanize: Option<Humanize>,
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GapResponse {
    pub steps: u32,
    pub gain: f64,
}
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Humanize {
    pub timing_ticks: u64,
    pub velocity: f64,
    pub mode: ProbabilityMode,
}
#[derive(Clone, Debug, Serialize)]
pub struct TouchTrace {
    pub offbeat: bool,
    pub offbeat_factor: f64,
    pub after_gap: bool,
    pub gap_factor: f64,
    pub timing_roll: f64,
    pub velocity_roll: f64,
    pub requested_jitter_ticks: i64,
    pub velocity_jitter_factor: f64,
}
impl Default for Groove {
    fn default() -> Self {
        Self {
            swing: 0.5,
            delay_ticks: 0,
            run: RunContour::None,
            ghost_probability: 0.0,
            ghost_gain: 0.45,
            ghost_mode: ProbabilityMode::PhraseLocked,
            offbeat_gain: 1.0,
            after_gap: None,
            humanize: None,
        }
    }
}
impl Groove {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.swing.is_finite() || !(0.5..=0.75).contains(&self.swing) {
            return Err("groove.swing must be within 0.5..0.75 (0.5 is straight)".into());
        }
        if self.delay_ticks > 60 {
            return Err("groove.delay_ticks must be within 0..60".into());
        }
        for (name, value) in [
            ("ghost_probability", self.ghost_probability),
            ("ghost_gain", self.ghost_gain),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(format!("groove.{name} must be within 0..1"));
            }
        }
        if !self.offbeat_gain.is_finite() || !(0.0..=2.0).contains(&self.offbeat_gain) {
            return Err("groove.offbeat_gain must be finite within 0..2".into());
        }
        if self.after_gap.as_ref().is_some_and(|g| {
            !(1..=32).contains(&g.steps) || !g.gain.is_finite() || !(0.0..=2.0).contains(&g.gain)
        }) {
            return Err("groove.after_gap requires steps 1..32 and gain 0..2".into());
        }
        if self.humanize.as_ref().is_some_and(|h| {
            h.timing_ticks > 30 || !h.velocity.is_finite() || !(0.0..=0.5).contains(&h.velocity)
        }) {
            return Err("groove.humanize requires timing_ticks 0..30 and velocity 0..0.5".into());
        }
        Ok(())
    }
    pub fn timing_jitter(&self, seed: u64, id: &str, step: u64, phrase_steps: u64) -> (f64, i64) {
        let h = self.humanize.clone().unwrap_or_default();
        let identity = match h.mode {
            ProbabilityMode::PhraseLocked => step % phrase_steps,
            ProbabilityMode::Continuous => step,
        };
        let roll = super::resolve::decision_roll(seed, id, "groove", identity, "humanize_timing");
        (
            roll,
            ((roll * 2.0 - 1.0) * h.timing_ticks as f64).round() as i64,
        )
    }
    pub fn onset_offset(&self, seed: u64, id: &str, step: u64, phrase_steps: u64) -> u64 {
        (self.offset(step) as i64 + self.timing_jitter(seed, id, step, phrase_steps).1)
            .clamp(0, super::STEP_TICKS as i64 - 2) as u64
    }
    pub fn offset(&self, step: u64) -> u64 {
        self.delay_ticks
            + if step % 2 == 1 {
                ((self.swing - 0.5) * 2.0 * STEP_TICKS as f64).round() as u64
            } else {
                0
            }
    }
    pub fn contour(&self, before: usize, after: usize) -> f64 {
        if before + 1 + after < 3 {
            return 1.0;
        }
        // For longer runs, hold the third value; bounded context never resets mid-run.
        let index = before.min(2);
        match self.run {
            RunContour::None => 1.0,
            RunContour::RampUp => [0.75, 0.875, 1.0][index],
            RunContour::LowHighLow => [0.8, 1.0, 0.8][index],
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct GrooveTrace {
    pub offset_ticks: u64,
    pub requested_gate_ticks: u64,
    pub ghost_roll: f64,
    pub ghost: bool,
    pub run_before: usize,
    pub run_after: usize,
    pub velocity_factor: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub touch: Option<TouchTrace>,
}
