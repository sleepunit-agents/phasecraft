//! Deterministic control timelines evaluated in musical time, including rests.
use super::{
    PPQN, Part, STEP_TICKS,
    accent::midi_value,
    resolve::{MidiEvent, MusicalEvent},
};
use serde::{Deserialize, Serialize};

/// 24 samples per quarter note. MIDI quantization/dedup reduces actual traffic.
pub const CONTROL_TICKS: u64 = PPQN / 24;
pub const MAX_SAMPLES_PER_STEP: usize = (STEP_TICKS / CONTROL_TICKS) as usize + 2;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterLane {
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp: Option<Ramp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation: Option<Automation>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Ramp {
    pub to: f64,
    pub over_bars: u32,
    #[serde(default = "first_bar")]
    pub start_bar: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Automation {
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default = "first_bar")]
    pub start_bar: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub to: f64,
    pub over_bars: f64,
    #[serde(default)]
    pub curve: Curve,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    #[default]
    Linear,
    Smooth,
    Hold,
}
impl Curve {
    fn shape(self, t: f64) -> f64 {
        match self {
            Self::Linear => t,
            Self::Smooth => t * t * (3.0 - 2.0 * t),
            Self::Hold => {
                if t < 1.0 {
                    0.0
                } else {
                    1.0
                }
            }
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct AutomationPosition {
    /// One-based segment and zero-based cycle. None before the scheduled start.
    pub segment: usize,
    pub cycle: u64,
    pub progress: f64,
    pub curve: Curve,
}
impl Segment {
    fn ticks(&self) -> u64 {
        (self.over_bars * 4.0 * PPQN as f64).round() as u64
    }
}
impl Automation {
    pub fn duration_ticks(&self) -> u64 {
        self.segments.iter().map(Segment::ticks).sum()
    }
    fn validate(&self) -> Result<(), String> {
        if self.segments.is_empty()
            || self.segments.len() > 64
            || !(1..=65536).contains(&self.start_bar)
        {
            return Err("automation requires 1..64 segments and start_bar within 1..65536".into());
        }
        for segment in &self.segments {
            let steps = segment.over_bars * 16.0;
            if !normalized(segment.to)
                || !steps.is_finite()
                || !(1.0..=1048576.0).contains(&steps)
                || (steps - steps.round()).abs() > 1e-9
            {
                return Err("segment.to must be within 0..1; over_bars must be 1/16..65536 in sixteenth-note increments".into());
            }
        }
        if self.duration_ticks() > 65536 * 4 * PPQN {
            return Err("automation total duration exceeds 65536 bars".into());
        }
        Ok(())
    }
    fn evaluate(&self, initial: f64, tick: u64) -> (f64, Option<AutomationPosition>) {
        let start = u64::from(self.start_bar - 1) * 4 * PPQN;
        if tick < start {
            return (initial, None);
        }
        let elapsed = tick - start;
        let duration = self.duration_ticks();
        let cycle = if self.repeat { elapsed / duration } else { 0 };
        let mut position = if self.repeat {
            elapsed % duration
        } else {
            elapsed.min(duration)
        };
        let mut from = initial;
        for (index, segment) in self.segments.iter().enumerate() {
            let length = segment.ticks();
            if position < length || index + 1 == self.segments.len() {
                let progress = (position as f64 / length as f64).min(1.0);
                return (
                    from + (segment.to - from) * segment.curve.shape(progress),
                    Some(AutomationPosition {
                        segment: index + 1,
                        cycle,
                        progress,
                        curve: segment.curve,
                    }),
                );
            }
            position -= length;
            from = segment.to;
        }
        unreachable!("validated automation has segments")
    }
}
fn first_bar() -> u32 {
    1
}
fn normalized(v: f64) -> bool {
    v.is_finite() && (0.0..=1.0).contains(&v)
}
impl ParameterLane {
    pub fn validate(&self) -> Result<(), String> {
        if !normalized(self.value) {
            return Err("parameter.value must be finite and within 0..1".into());
        }
        if let Some(automation) = &self.automation {
            if self.ramp.is_some() {
                return Err("choose ramp or automation, not both".into());
            }
            automation.validate()?;
        }
        if let Some(ramp) = &self.ramp
            && (!normalized(ramp.to)
                || !(1..=65536).contains(&ramp.over_bars)
                || !(1..=65536).contains(&ramp.start_bar))
        {
            return Err(
                "ramp.to must be finite within 0..1; over_bars/start_bar must be 1..65536".into(),
            );
        }
        Ok(())
    }
    pub fn at(&self, tick: u64) -> f64 {
        if let Some(automation) = &self.automation {
            return automation.evaluate(self.value, tick).0;
        }
        let Some(ramp) = &self.ramp else {
            return self.value;
        };
        let start = u64::from(ramp.start_bar - 1) * 4 * PPQN;
        let length = u64::from(ramp.over_bars) * 4 * PPQN;
        let fraction = tick.saturating_sub(start).min(length) as f64 / length as f64;
        self.value + (ramp.to - self.value) * fraction
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ParameterSample {
    pub tick: u64,
    pub base: f64,
    pub emphasis: f64,
    pub amount: f64,
    pub value: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<AutomationPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope: Option<super::accent::EnvelopeTrace>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ParameterTrace {
    pub name: String,
    pub channel: u8,
    pub cc: u8,
    pub samples: Vec<ParameterSample>,
}

pub fn resolve(
    part: &Part,
    step: u64,
    event: Option<&MusicalEvent>,
    history: &[(u64, f64)],
) -> (Vec<ParameterTrace>, Vec<MidiEvent>) {
    resolve_window(
        part,
        step * STEP_TICKS,
        (step + 1) * STEP_TICKS,
        &event.into_iter().cloned().collect::<Vec<_>>(),
        history,
    )
}
pub fn resolve_window(
    part: &Part,
    start: u64,
    end: u64,
    events: &[MusicalEvent],
    history: &[(u64, f64)],
) -> (Vec<ParameterTrace>, Vec<MidiEvent>) {
    let names: std::collections::BTreeSet<_> = part
        .parameters
        .keys()
        .chain(
            part.profile
                .controls
                .iter()
                .filter(|(_, r)| r.envelope.is_some())
                .map(|(name, _)| name),
        )
        .collect();
    if names.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut ticks: Vec<_> = (0..end - start)
        .step_by(CONTROL_TICKS as usize)
        .map(|t| start + t)
        .collect();
    for event in events {
        ticks.extend([event.tick, event.tick + event.duration_ticks]);
    }
    ticks.retain(|&t| t >= start && t < end);
    ticks.sort_unstable();
    ticks.dedup();
    let mut traces = Vec::new();
    let mut midi = Vec::new();
    for name in names {
        let lane = part.parameters.get(name);
        let response = part.profile.controls.get(name);
        let initial = lane.map_or_else(|| response.map_or(0.0, |r| r.base), |l| l.value);
        let output = &part.output.controls[name];
        let channel = output.channel.unwrap_or(part.output.channel);
        let boost = part.profile.controls.get(name).map_or(0.0, |r| r.boost);
        let samples = ticks
            .iter()
            .map(|&tick| {
                let base = lane.map_or(initial, |l| l.at(tick));
                let envelope = response
                    .and_then(|r| r.envelope.as_ref())
                    .map(|e| e.evaluate(tick, history));
                let momentary = events
                    .iter()
                    .find(|e| tick >= e.tick && tick < e.tick + e.duration_ticks && e.accent.active)
                    .map_or(0.0, |e| e.accent.amount * boost);
                let emphasis = envelope.as_ref().map_or(momentary, |e| e.level * boost);
                let amount = (base + emphasis).clamp(0.0, 1.0);
                let value = midi_value(amount);
                midi.push(MidiEvent {
                    tick,
                    bytes: [0xb0 | (channel - 1), output.cc, value],
                    reset_value: (emphasis != 0.0).then(|| midi_value(base)),
                    boundary_reset: false,
                    parameter: true,
                    stop_value: Some(midi_value(output.default.unwrap_or(initial))),
                });
                ParameterSample {
                    tick,
                    base,
                    emphasis,
                    envelope,
                    automation: lane
                        .and_then(|l| l.automation.as_ref())
                        .and_then(|a| a.evaluate(initial, tick).1),
                    amount,
                    value,
                }
            })
            .collect();
        traces.push(ParameterTrace {
            name: name.clone(),
            channel,
            cc: output.cc,
            samples,
        });
    }
    (traces, midi)
}
