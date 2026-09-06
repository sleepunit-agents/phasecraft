//! A bounded sequence of procedural snapshots. Nothing here reads files or stores MIDI scores.
use super::{Composition, STEP_TICKS};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Arrangement {
    #[serde(default)]
    pub repeat: bool,
    pub sections: Vec<Section>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub phrase: String,
    pub bars: u32,
    #[serde(default)]
    pub phase: PhasePolicy,
    pub composition: Box<Composition>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhasePolicy {
    #[default]
    Restart,
    Continue,
}
#[derive(Clone, Debug, Serialize)]
pub struct SectionPosition {
    pub phrase: String,
    pub index: usize,
    pub count: usize,
    pub cycle: u64,
    pub bar: u64,
    pub bars: u32,
    pub phase: PhasePolicy,
}
pub struct Located<'a> {
    pub section: &'a Section,
    pub start_step: u64,
    pub local_step: u64,
    pub musical_step: u64,
    pub position: SectionPosition,
}
impl Arrangement {
    pub fn steps(&self) -> u64 {
        self.sections.iter().map(|s| u64::from(s.bars) * 16).sum()
    }
    pub fn validate(&self, tempo: f64) -> Result<(), String> {
        if self.sections.is_empty() || self.sections.len() > 64 {
            return Err("arrangement requires 1..64 sections".into());
        }
        for s in &self.sections {
            if s.phrase.trim().is_empty() || !(1..=65536).contains(&s.bars) {
                return Err("section requires a phrase name and bars 1..65536".into());
            }
            if s.composition.arrangement.is_some() {
                return Err("nested arrangements are not supported".into());
            }
            if s.composition.tempo != tempo {
                return Err("all arrangement phrases must use the same tempo".into());
            }
            s.composition
                .validate()
                .map_err(|e| format!("phrase {:?}: {e}", s.phrase))?;
        }
        if self.steps() > 65536 * 16 {
            return Err("arrangement exceeds 65536 bars".into());
        }
        Ok(())
    }
    pub fn locate(&self, step: u64) -> Option<Located<'_>> {
        let length = self.steps();
        if !self.repeat && step >= length {
            return None;
        }
        let cycle = step / length;
        let mut local = step % length;
        for (index, s) in self.sections.iter().enumerate() {
            let steps = u64::from(s.bars) * 16;
            if local < steps {
                return Some(Located {
                    section: s,
                    start_step: step - local,
                    local_step: local,
                    musical_step: if s.phase == PhasePolicy::Restart {
                        local
                    } else {
                        step
                    },
                    position: SectionPosition {
                        phrase: s.phrase.clone(),
                        index: index + 1,
                        count: self.sections.len(),
                        cycle,
                        bar: local / 16 + 1,
                        bars: s.bars,
                        phase: s.phase,
                    },
                });
            }
            local -= steps;
        }
        None
    }
    /// Structural/routing edits need a restart; musical edits may reload at a phrase boundary.
    pub fn same_layout(&self, other: &Self) -> bool {
        self.repeat == other.repeat
            && self.sections.len() == other.sections.len()
            && self.sections.iter().zip(&other.sections).all(|(a, b)| {
                a.phrase == b.phrase
                    && a.bars == b.bars
                    && a.phase == b.phase
                    && a.composition.phrase_bars == b.composition.phrase_bars
                    && a.composition.parts.len() == b.composition.parts.len()
                    && a.composition
                        .parts
                        .iter()
                        .zip(&b.composition.parts)
                        .all(|(a, b)| a.id == b.id && a.output == b.output)
            })
    }
}
impl Composition {
    pub fn end_step(&self) -> Option<u64> {
        self.arrangement
            .as_ref()
            .filter(|a| !a.repeat)
            .map(Arrangement::steps)
    }
    pub fn at_step(&self, step: u64) -> &Composition {
        self.arrangement
            .as_ref()
            .and_then(|a| a.locate(step))
            .map_or(self, |s| s.section.composition.as_ref())
    }
    pub fn same_arrangement_layout(&self, other: &Self) -> bool {
        match (&self.arrangement, &other.arrangement) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same_layout(b),
            _ => false,
        }
    }
}
/// Reset declared active outputs before the next section initializes its values.
/// All drum note gates end before the section boundary.
pub fn resets(c: &Composition, tick: u64) -> Vec<super::resolve::MidiEvent> {
    let mut result = vec![];
    for p in &c.parts {
        let names: std::collections::BTreeSet<_> = p
            .parameters
            .keys()
            .chain(p.profile.controls.keys())
            .collect();
        for name in names {
            let output = &p.output.controls[name];
            let baseline = p
                .parameters
                .get(name)
                .map(|l| l.value)
                .or_else(|| p.profile.controls.get(name).map(|r| r.base))
                .unwrap();
            result.push(super::resolve::MidiEvent {
                tick,
                bytes: [
                    0xb0 | (output.channel.unwrap_or(p.output.channel) - 1),
                    output.cc,
                    super::accent::midi_value(output.default.unwrap_or(baseline)),
                ],
                reset_value: None,
                stop_value: None,
                parameter: false,
                boundary_reset: true,
            });
        }
    }
    result
}
pub fn resolve(
    c: &Composition,
    step: u64,
) -> (
    Vec<super::resolve::StepTrace>,
    Vec<super::resolve::MidiEvent>,
) {
    let a = c.arrangement.as_ref().unwrap();
    let Some(located) = a.locate(step) else {
        return (vec![], vec![]);
    };
    let (mut traces, mut events) =
        super::resolve::resolve_step(&located.section.composition, located.musical_step);
    let shift = (step - located.musical_step) * STEP_TICKS;
    for e in &mut events {
        e.tick += shift;
    }
    for t in &mut traces {
        t.step = step;
        t.tick = step * STEP_TICKS;
        t.position = format!("{}.{}.{}", step / 16 + 1, step / 4 % 4 + 1, step % 4 + 1);
        t.section = Some(located.position.clone());
        if let Some(e) = &mut t.event {
            e.tick += shift;
        }
        for p in &mut t.parameters {
            for s in &mut p.samples {
                s.tick += shift;
            }
        }
    }
    if located.local_step == 0
        && step > 0
        && let Some(previous) = a.locate(step - 1)
    {
        events.extend(resets(&previous.section.composition, step * STEP_TICKS));
    }
    events.sort_by_key(super::resolve::midi_order);
    (traces, events)
}
