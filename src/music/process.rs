//! Numeric velocity sequences. Structural admission owns the event clock; rendering does not.
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Per {
    #[default]
    Step,
    Event,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    Main,
    #[default]
    Attacks,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Carry {
    Always,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "File", into = "File")]
pub struct ValuePattern {
    file: File,
    values: Arc<[f64]>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct File {
    pattern: String,
    #[serde(default)]
    per: Per,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clock: Option<Clock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    carry: Option<Carry>,
    #[serde(default = "cycle")]
    cycle: u32,
}
fn cycle() -> u32 {
    16
}
impl TryFrom<File> for ValuePattern {
    type Error = String;
    fn try_from(file: File) -> Result<Self, String> {
        if !(1..=65536).contains(&file.cycle) {
            return Err("velocity.cycle must be 1..65536 sixteenth-note slots".into());
        }
        if file.per != Per::Event && (file.clock.is_some() || file.carry.is_some()) {
            return Err("velocity clock/carry require per = \"event\"".into());
        }
        super::notation::Pattern::parse(&file.pattern)
            .map_err(|e| format!("velocity.pattern: {e}"))?;
        let mut values = Vec::new();
        for word in file.pattern.split_whitespace() {
            if values.len() == 4096 {
                return Err("velocity.pattern supports at most 4096 values".into());
            }
            let value: f64 = word.parse().map_err(|_| "velocity.pattern currently requires a flat list of numbers; nested notation, rests and draws are not supported")?;
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err("velocity.pattern values must be finite and within 0..1".into());
            }
            values.push(value);
        }
        if values.is_empty() {
            return Err("velocity.pattern must contain a value".into());
        }
        Ok(Self {
            file,
            values: values.into(),
        })
    }
}
impl From<ValuePattern> for File {
    fn from(p: ValuePattern) -> Self {
        p.file
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ValueRead {
    /// Main is 0; tails follow; an admitted flam is last, regardless of sounding time.
    pub child: u8,
    pub index: usize,
    pub value: f64,
}
impl ValuePattern {
    pub fn per(&self) -> Per {
        self.file.per
    }
    pub fn clock(&self) -> Clock {
        self.file.clock.unwrap_or_default()
    }
    pub fn carries(&self) -> bool {
        self.file.carry.is_some()
    }
    pub fn cycle_ticks(&self) -> u64 {
        u64::from(self.file.cycle) * super::STEP_TICKS
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn read(&self, child: u8, position: u64) -> ValueRead {
        let index = if self.per() == Per::Event {
            (position % self.len() as u64) as usize
        } else {
            ((u128::from(position % self.cycle_ticks()) * self.len() as u128)
                / u128::from(self.cycle_ticks())) as usize
        };
        ValueRead {
            child,
            index,
            value: self.values[index],
        }
    }
}

/// A carried index belongs to the voice ID. Every active occurrence must retain a
/// compatible schema; absence freezes it. Values may change without changing the index space.
pub fn validate_carry(c: &super::Composition) -> Result<(), String> {
    let snapshots: Vec<_> = if let Some(a) = &c.arrangement {
        a.sections.iter().map(|s| s.composition.as_ref()).collect()
    } else if let Some(r) = &c.router {
        r.scenes.iter().map(|s| s.composition.as_ref()).collect()
    } else {
        return Ok(());
    };
    for owner in snapshots.iter().flat_map(|s| &s.parts) {
        let Some(value) = owner.velocity.as_ref().filter(|v| v.carries()) else {
            continue;
        };
        for other in snapshots
            .iter()
            .filter_map(|s| s.parts.iter().find(|p| p.id == owner.id))
        {
            if other.velocity.as_ref().is_none_or(|v| {
                !v.carries() || v.len() != value.len() || v.clock() != value.clock()
            }) {
                return Err(format!(
                    "Part {:?}: carried velocity requires the same value count and clock in every active scene/section; absent Parts freeze",
                    owner.id
                ));
            }
        }
    }
    Ok(())
}
