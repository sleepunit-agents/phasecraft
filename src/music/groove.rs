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
        Ok(())
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
}
