//! Semantic emphasis responses, independent of the target's MIDI assignments.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_CONTROLS: usize = 8;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlResponse {
    /// Normalized resting value and signed emphasis contribution.
    pub base: f64,
    pub boost: f64,
}
impl ControlResponse {
    pub fn value(&self, amount: f64) -> f64 {
        (self.base + self.boost * amount).clamp(0.0, 1.0)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlOutput {
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
    if responses.len() > MAX_CONTROLS {
        return Err(format!(
            "accent profiles support at most {MAX_CONTROLS} controls"
        ));
    }
    if !responses.keys().eq(outputs.keys()) {
        return Err("profile.controls and output.controls must contain the same names".into());
    }
    for (name, response) in responses {
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
        let output = &outputs[name];
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
