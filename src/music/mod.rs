pub mod groove;
pub mod resolve;
pub mod rhythm;
use rhythm::Expression;
use serde::{Deserialize, Serialize};

pub const MAX_PARTS: usize = 32;

pub const PPQN: u64 = 960;
pub const STEP_TICKS: u64 = PPQN / 4;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "CompositionFile")]
pub struct Composition {
    pub tempo: f64,
    pub seed: u64,
    #[serde(default = "four")]
    pub phrase_bars: u32,
    pub parts: Vec<Part>,
}
// Accept the original single-Part file without changing its musical identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompositionFile {
    tempo: f64,
    seed: u64,
    #[serde(default = "four")]
    phrase_bars: u32,
    part: Option<Part>,
    parts: Option<Vec<Part>>,
}
impl TryFrom<CompositionFile> for Composition {
    type Error = String;
    fn try_from(file: CompositionFile) -> Result<Self, String> {
        let parts = match (file.part, file.parts) {
            (Some(part), None) => vec![part],
            (None, Some(parts)) => parts,
            _ => {
                return Err(
                    "Use either [part] or parts ([parts.NAME] / [[parts]]), not both; at least one Part is required"
                        .into(),
                );
            }
        };
        let c = Self {
            tempo: file.tempo,
            seed: file.seed,
            phrase_bars: file.phrase_bars,
            parts,
        };
        c.validate()?;
        Ok(c)
    }
}
fn four() -> u32 {
    4
}
fn one() -> f64 {
    1.0
}
fn channel() -> u8 {
    10
}
fn gate() -> u64 {
    120
}
fn velocity() -> u8 {
    80
}
fn boost() -> u8 {
    35
}
fn amount() -> f64 {
    0.8
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub id: String,
    pub trigger: Lane,
    pub accent: AccentLane,
    pub output: Output,
    #[serde(default)]
    pub profile: VelocityProfile,
    #[serde(default, skip_serializing_if = "groove::Groove::is_default")]
    pub groove: groove::Groove,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub rhythm: Expression,
    #[serde(default = "one")]
    pub probability: f64,
    #[serde(default)]
    pub probability_mode: ProbabilityMode,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilityMode {
    #[default]
    PhraseLocked,
    Continuous,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccentLane {
    pub rhythm: Expression,
    #[serde(default = "one")]
    pub probability: f64,
    #[serde(default)]
    pub probability_mode: ProbabilityMode,
    #[serde(default = "amount")]
    pub amount: f64,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    #[serde(default = "channel")]
    pub channel: u8,
    pub note: u8,
    #[serde(default = "gate")]
    pub gate_ticks: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct VelocityProfile {
    pub base: u8,
    pub boost: u8,
}
impl Default for VelocityProfile {
    fn default() -> Self {
        Self {
            base: velocity(),
            boost: boost(),
        }
    }
}
impl Composition {
    pub fn parse(text: &str) -> Result<Self, String> {
        crate::library::expand(text, None)?
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())
    }
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        crate::authoring::project::load(path).map(|loaded| loaded.composition)
    }
    pub fn phrase_steps(&self) -> u64 {
        u64::from(self.phrase_bars) * 16
    }
    pub fn evaluation_order(&self) -> Result<Vec<&Part>, String> {
        fn visit<'a>(
            part: &'a Part,
            parts: &std::collections::BTreeMap<&str, &'a Part>,
            active: &mut Vec<String>,
            done: &mut std::collections::HashSet<String>,
            result: &mut Vec<&'a Part>,
        ) -> Result<(), String> {
            if done.contains(&part.id) {
                return Ok(());
            }
            if active.contains(&part.id) {
                return Err(format!(
                    "Part dependency cycle: {} -> {}",
                    active.join(" -> "),
                    part.id
                ));
            }
            active.push(part.id.clone());
            for id in part
                .trigger
                .rhythm
                .references()
                .into_iter()
                .chain(part.accent.rhythm.references())
            {
                let target = parts
                    .get(id)
                    .ok_or_else(|| format!("Part {:?} references missing Part {id:?}", part.id))?;
                visit(target, parts, active, done, result)?;
            }
            active.pop();
            done.insert(part.id.clone());
            result.push(part);
            Ok(())
        }
        let parts: std::collections::BTreeMap<_, _> =
            self.parts.iter().map(|p| (p.id.as_str(), p)).collect();
        let mut result = vec![];
        let mut active = vec![];
        let mut done = std::collections::HashSet::new();
        for part in parts.values() {
            visit(part, &parts, &mut active, &mut done, &mut result)?;
        }
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.tempo.is_finite() || !(20.0..=400.0).contains(&self.tempo) {
            return Err("tempo must be finite and within 20..400 BPM".into());
        }
        if !(1..=1024).contains(&self.phrase_bars) {
            return Err("phrase_bars must be 1..1024 (4/4)".into());
        }
        if self.parts.is_empty() || self.parts.len() > MAX_PARTS {
            return Err(format!("composition requires 1..{MAX_PARTS} Parts"));
        }
        let mut ids = std::collections::HashSet::new();
        let mut routes = std::collections::HashSet::new();
        for part in &self.parts {
            part.validate()
                .map_err(|e| format!("Part {:?}: {e}", part.id))?;
            if !ids.insert(&part.id) {
                return Err(format!("duplicate Part ID {:?}", part.id));
            }
            if !routes.insert((part.output.channel, part.output.note)) {
                return Err(format!(
                    "Parts must use distinct MIDI channel/note pairs; duplicate route on {:?}",
                    part.id
                ));
            }
        }
        self.evaluation_order()?;
        Ok(())
    }
}
impl Part {
    fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("part.id cannot be empty".into());
        }
        for (name, value) in [
            ("trigger.probability", self.trigger.probability),
            ("accent.probability", self.accent.probability),
            ("accent.amount", self.accent.amount),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(format!("{name} must be finite and within 0..1"));
            }
        }
        self.trigger
            .rhythm
            .validate(0)
            .map_err(|e| format!("trigger.rhythm: {e}"))?;
        self.accent
            .rhythm
            .validate(0)
            .map_err(|e| format!("accent.rhythm: {e}"))?;
        self.groove.validate()?;
        let o = &self.output;
        if !(1..=16).contains(&o.channel) || o.note > 127 {
            return Err("MIDI channel must be 1..16 and note 0..127".into());
        }
        if o.gate_ticks == 0 || o.gate_ticks >= STEP_TICKS {
            return Err("gate_ticks must be 1..239 for this drum slice".into());
        }
        if !(1..=127).contains(&self.profile.base) || self.profile.boost > 127 {
            return Err("velocity base must be 1..127 and boost 0..127".into());
        }
        Ok(())
    }
}
