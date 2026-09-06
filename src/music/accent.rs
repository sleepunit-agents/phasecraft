//! Semantic emphasis responses, independent of the target's MIDI assignments.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_CONTROLS: usize = 8;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlResponse {
    /// Normalized resting value and signed emphasis contribution.
    #[serde(default)]
    pub base: f64,
    pub boost: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope: Option<Envelope>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub decay_beats: f64,
    #[serde(default = "unit")]
    pub accumulation: f64,
}
fn unit() -> f64 {
    1.0
}
#[derive(Clone, Debug, Serialize)]
pub struct EnvelopeTrace {
    pub level: f64,
    pub impulses: usize,
}
impl Envelope {
    pub fn history_steps(&self) -> u64 {
        (self.decay_beats * 4.0).round() as u64
    }
    fn validate(&self) -> Result<(), String> {
        let steps = self.decay_beats * 4.0;
        if !steps.is_finite()
            || !(1.0..=32.0).contains(&steps)
            || (steps - steps.round()).abs() > 1e-9
            || !self.accumulation.is_finite()
            || !(0.0..=1.0).contains(&self.accumulation)
        {
            return Err("control envelope requires decay_beats 0.25..8 in sixteenth increments and accumulation 0..1".into());
        }
        Ok(())
    }
    pub fn evaluate(&self, tick: u64, history: &[(u64, f64)]) -> EnvelopeTrace {
        let duration = self.history_steps() * super::STEP_TICKS;
        let mut level = 0.0;
        let mut impulses = 0;
        for &(onset, amount) in history {
            if tick >= onset && tick - onset < duration {
                level +=
                    amount * self.accumulation * (1.0 - (tick - onset) as f64 / duration as f64);
                impulses += 1;
            }
        }
        EnvelopeTrace {
            level: level.clamp(0.0, 1.0),
            impulses,
        }
    }
}
impl ControlResponse {
    pub fn value(&self, amount: f64) -> f64 {
        (self.base + self.boost * amount).clamp(0.0, 1.0)
    }
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlOutput {
    /// Neutral kit value restored when transport stops; musical values are separate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<f64>,
    pub cc: u8,
    /// Defaults to the Part's note channel.
    pub channel: Option<u8>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedControl {
    pub name: String,
    pub amount: f64,
    pub channel: u8,
    pub cc: u8,
    pub value: u8,
    pub reset: u8,
}
pub fn midi_value(value: f64) -> u8 {
    (value * 127.0).round().clamp(0.0, 127.0) as u8
}
pub fn validate(
    responses: &BTreeMap<String, ControlResponse>,
    outputs: &BTreeMap<String, ControlOutput>,
) -> Result<(), String> {
    if outputs.len() > MAX_CONTROLS {
        return Err(format!(
            "accent profiles support at most {MAX_CONTROLS} controls"
        ));
    }
    if responses.keys().any(|name| !outputs.contains_key(name)) {
        return Err("profile.controls names require matching output.controls mappings".into());
    }
    for (name, response) in responses {
        if let Some(envelope) = &response.envelope {
            envelope.validate()?;
        }
        if name.trim().is_empty()
            || !response.base.is_finite()
            || !(0.0..=1.0).contains(&response.base)
            || !response.boost.is_finite()
            || !(-1.0..=1.0).contains(&response.boost)
        {
            return Err(format!(
                "control {name:?} requires a name, finite base 0..1 and boost -1..1"
            ));
        }
    }
    for (name, output) in outputs {
        if output
            .default
            .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err(format!(
                "control {name:?}: default must be finite within 0..1"
            ));
        }
        if name.trim().is_empty() {
            return Err("output control names cannot be empty".into());
        }
        // Exclude bank selection, pedals, increment/decrement, parameter selection
        // and channel-mode commands; these are continuous momentary controls.
        if !(1..=31).contains(&output.cc)
            && !(33..=63).contains(&output.cc)
            && !(70..=95).contains(&output.cc)
        {
            return Err(format!(
                "control {name:?}: use a continuous CC in 1..31, 33..63 or 70..95"
            ));
        }
        if output.channel.is_some_and(|c| !(1..=16).contains(&c)) {
            return Err(format!("control {name:?}: MIDI channel must be 1..16"));
        }
    }
    Ok(())
}
