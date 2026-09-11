//! Transport-clocked bounded target/walk lanes and explicit readers.
use super::{Composition, PPQN, resolve::decision_roll};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
const BAR: u64 = 4 * PPQN;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetLane {
    pub start: f64,
    pub range: [f64; 2],
    pub carry: Carry,
    pub target: Target,
}
/// The two wire shapes are disjoint; each refuses unknown or mixed fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Lane {
    Target(TargetLane),
    Walk(WalkLane),
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WalkLane {
    pub start: f64,
    pub bounds: [f64; 2],
    pub carry: Carry,
    pub every: Slots,
    pub step: Vec<f64>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Slots {
    pub slots: u32,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Carry {
    Always,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bars {
    pub bars: u32,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub every: Bars,
    pub delta: Vec<f64>,
    pub ramp: Bars,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Follower {
    pub follows: String,
    pub op: Operation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unclipped: Option<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Direct,
    Range,
    ScaleDefault,
}
#[derive(Clone, Debug, Serialize)]
pub struct Sample {
    pub name: String,
    pub tick: u64,
    pub value: f64,
    pub occurrence: u64,
    pub origin: f64,
    pub target: f64,
    pub progress: f64,
    pub roll: Option<f64>,
}
/// Bounded checkpoints. Evicted history is replayed exactly, with no silent expiry.
#[derive(Default)]
pub struct Cache {
    entries: BTreeMap<(String, u64), f64>,
    bindings: BTreeMap<String, (u64, Lane)>,
}
impl Lane {
    pub fn range(&self) -> [f64; 2] {
        match self {
            Self::Target(lane) => lane.range,
            Self::Walk(lane) => lane.bounds,
        }
    }
    fn start(&self) -> f64 {
        match self {
            Self::Target(lane) => lane.start,
            Self::Walk(lane) => lane.start,
        }
    }
    pub(crate) fn every_ticks(&self) -> u64 {
        match self {
            Self::Target(lane) => u64::from(lane.target.every.bars) * BAR,
            Self::Walk(lane) => u64::from(lane.every.slots) * super::STEP_TICKS,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let [lo, hi] = self.range();
        if !lo.is_finite()
            || !hi.is_finite()
            || lo >= hi
            || !(hi - lo).is_finite()
            || !self.start().is_finite()
            || !(lo..=hi).contains(&self.start())
        {
            return Err("lane requires finite increasing range/bounds and start within it".into());
        }
        let choices = match self {
            Self::Target(lane) => {
                let target = &lane.target;
                if !(1..=65536).contains(&target.every.bars)
                    || target.ramp.bars == 0
                    || target.ramp.bars >= target.every.bars
                {
                    return Err(
                        "target.every.bars must be 2..65536 and ramp.bars must be positive and shorter"
                            .into(),
                    );
                }
                &target.delta
            }
            Self::Walk(lane) => {
                if !(1..=65536).contains(&lane.every.slots) {
                    return Err("walk every.slots must be 1..65536".into());
                }
                &lane.step
            }
        };
        if choices.is_empty()
            || choices.len() > 64
            || choices
                .iter()
                .any(|d| !d.is_finite() || !(lo + d).is_finite() || !(hi + d).is_finite())
        {
            return Err("lane delta/step requires 1..64 finite, non-overflowing choices".into());
        }
        Ok(())
    }
    pub fn sample(&self, name: &str, seed: u64, tick: u64, cache: &mut Cache) -> Sample {
        if !cache
            .bindings
            .get(name)
            .is_some_and(|(bound_seed, lane)| *bound_seed == seed && lane == self)
        {
            cache.entries.clear();
            if cache.bindings.len() >= 16 {
                cache.bindings.clear();
            }
            cache.bindings.insert(name.to_owned(), (seed, self.clone()));
        }
        let every = self.every_ticks();
        let occurrence = tick / every;
        let key = name.to_owned();
        let (mut k, mut origin) = cache
            .entries
            .range((key.clone(), 0)..=(key.clone(), occurrence.saturating_sub(1)))
            .next_back()
            .map(|((_, k), v)| (*k, *v))
            .unwrap_or((0, self.start()));
        while k < occurrence.saturating_sub(1) {
            k += 1;
            origin = self.next(name, seed, k, origin).0;
            if cache.entries.len() >= 4096 {
                cache.entries.pop_first();
            }
            cache.entries.insert((key.clone(), k), origin);
        }
        let (target, roll, progress) = if occurrence == 0 {
            (self.start(), None, 0.0)
        } else {
            let (target, roll) = self.next(name, seed, occurrence, origin);
            (
                target,
                Some(roll),
                match self {
                    Self::Target(lane) => {
                        let bars = (tick % every) / BAR;
                        bars.min(u64::from(lane.target.ramp.bars)) as f64
                            / f64::from(lane.target.ramp.bars)
                    }
                    Self::Walk(_) => 1.0,
                },
            )
        };
        Sample {
            name: key,
            tick,
            // A convex sum preserves a clamped small target beside a very large
            // origin; origin + (target-origin) can cancel the target at progress=1.
            value: (origin * (1.0 - progress) + target * progress)
                .clamp(self.range()[0], self.range()[1]),
            occurrence,
            origin,
            target,
            progress,
            roll,
        }
    }
    fn next(&self, name: &str, seed: u64, k: u64, from: f64) -> (f64, f64) {
        let (roll, choices) = match self {
            Self::Target(lane) => (
                decision_roll(seed, name, "target", k, "delta"),
                &lane.target.delta,
            ),
            // The walk address names the absolute transport tick of the step.
            Self::Walk(lane) => (
                decision_roll(seed, name, "step", k * self.every_ticks(), "delta"),
                &lane.step,
            ),
        };
        let delta = choices[(roll * choices.len() as f64) as usize];
        ((from + delta).clamp(self.range()[0], self.range()[1]), roll)
    }
}
impl Follower {
    pub fn mapped(&self, lane: &Lane, value: f64, default: f64) -> f64 {
        match self.op {
            Operation::Direct => value,
            Operation::Range | Operation::ScaleDefault => {
                let low = self.low.expect("validated mapping span");
                let high = self.high.expect("validated mapping span");
                let fraction = (value - lane.range()[0]) / (lane.range()[1] - lane.range()[0]);
                let mapped = low + (high - low) * fraction;
                if self.op == Operation::ScaleDefault {
                    mapped * default
                } else {
                    mapped
                }
            }
        }
    }
    pub fn validate(
        &self,
        lanes: &BTreeMap<String, Lane>,
        default: Option<f64>,
        gate: bool,
    ) -> Result<(), String> {
        let lane = lanes
            .get(&self.follows)
            .ok_or_else(|| format!("unknown shared lane {:?}", self.follows))?;
        if self.op == Operation::Direct {
            if self.low.is_some() || self.high.is_some() || self.unclipped.is_some() {
                return Err(
                    "direct takes the lane value unchanged; omit low, high and unclipped".into(),
                );
            }
        } else {
            let (Some(low), Some(high)) = (self.low, self.high) else {
                return Err("mapping follower requires low and high".into());
            };
            if !low.is_finite() || !high.is_finite() || !(high - low).is_finite() {
                return Err("follower span must be finite".into());
            }
        }
        if gate && self.op != Operation::Range {
            return Err("ratchet gate requires op = range".into());
        }
        if self.op == Operation::ScaleDefault && default.is_none() {
            return Err("scale-default requires an explicit output control default".into());
        }
        for edge in lane.range() {
            let value = self.mapped(lane, edge, default.unwrap_or(1.0));
            if !value.is_finite() {
                return Err(format!(
                    "follower on lane {:?} (range {}..{}) produces non-finite value {value} at {edge}; check mapping span and output control default",
                    self.follows,
                    lane.range()[0],
                    lane.range()[1],
                ));
            }
            if (gate || self.op == Operation::Direct || self.unclipped.unwrap_or(false))
                && !(0.0..=1.0).contains(&value)
            {
                let promise = if self.op == Operation::Direct {
                    "direct takes the lane value unchanged, so the lane's own range must fit 0..1; use op = range to map it"
                } else if gate {
                    "a ratchet gate is a probability and must fit 0..1"
                } else {
                    "unclipped = true promises the mapped control fits 0..1"
                };
                return Err(format!(
                    "follower on lane {:?} (range {}..{}) reaches {value} outside 0..1: {promise}",
                    self.follows,
                    lane.range()[0],
                    lane.range()[1],
                ));
            }
        }
        Ok(())
    }
}
pub fn validate(c: &Composition) -> Result<(), String> {
    if c.lanes.len() > 16 {
        return Err("at most 16 shared lanes".into());
    }
    for (name, lane) in &c.lanes {
        if name.trim().is_empty() {
            return Err("shared lane requires a name".into());
        }
        lane.validate().map_err(|e| format!("lane {name:?}: {e}"))?;
    }
    let mut addresses = BTreeMap::new();
    for part in &c.parts {
        for (name, param) in &part.parameters {
            let output = &part.output.controls[name];
            let address = (output.channel.unwrap_or(part.output.channel), output.cc);
            if let Some(previous_followed) = addresses.insert(address, param.follow.is_some())
                && (previous_followed || param.follow.is_some())
            {
                return Err("followed controls require distinct active channel/CC targets".into());
            }
        }
        for name in part
            .profile
            .controls
            .keys()
            .filter(|name| !part.parameters.contains_key(*name))
        {
            let output = &part.output.controls[name];
            let address = (output.channel.unwrap_or(part.output.channel), output.cc);
            if addresses.insert(address, false) == Some(true) {
                return Err("followed controls require distinct active channel/CC targets".into());
            }
        }
    }
    for part in &c.parts {
        if let Some(gate) = &part.ornaments.gate {
            gate.validate(&c.lanes, None, true)?;
            if part.ornaments.ratchet.is_none()
                && !part
                    .trigger
                    .rhythm
                    .literal_schedule()
                    .is_some_and(|p| p.schedule().has_ratchet())
            {
                return Err("followed ratchet gate requires a written or configured burst".into());
            }
        }
        for (name, param) in &part.parameters {
            if let Some(f) = &param.follow {
                f.validate(&c.lanes, part.output.controls[name].default, false)?;
                if part.profile.controls.contains_key(name) {
                    return Err("followed control cannot also have an accent response".into());
                }
            }
        }
    }
    for child in c
        .arrangement
        .iter()
        .flat_map(|a| a.sections.iter().map(|s| &s.composition))
        .chain(
            c.router
                .iter()
                .flat_map(|r| r.scenes.iter().map(|s| &s.composition)),
        )
    {
        if c.lanes != child.lanes {
            return Err(
                "shared lanes belong to the root transport and must match every scene/section"
                    .into(),
            );
        }
    }
    if !c.lanes.is_empty()
        && let Some(r) = &c.router
        && !r.period_ticks(c)?.is_multiple_of(BAR)
    {
        return Err("shared barwise controls require router boundaries on barlines".into());
    }
    Ok(())
}

/// Reach and candidate-publication count, separate from errors and missing kit routes.
pub fn report(c: &Composition) -> Vec<String> {
    fn scope(c: &Composition, label: &str, rows: &mut Vec<String>) {
        let mut count = 0;
        for p in &c.parts {
            for (name, param) in &p.parameters {
                let Some(f) = &param.follow else {
                    continue;
                };
                count += 1;
                let lane = &c.lanes[&f.follows];
                let default = p.output.controls[name].default.unwrap_or(1.0);
                let a = f.mapped(lane, lane.range()[0], default);
                let b = f.mapped(lane, lane.range()[1], default);
                let (lo, hi) = (a.min(b), a.max(b));
                rows.push(format!(
                    "{label} {}.{name} follows {}: raw {lo:.6}..{hi:.6}, control {:.6}..{:.6}, {}",
                    p.id,
                    f.follows,
                    lo.clamp(0.0, 1.0),
                    hi.clamp(0.0, 1.0),
                    if f.op == Operation::Direct {
                        "direct"
                    } else if lo < 0.0 || hi > 1.0 {
                        "saturates"
                    } else {
                        "unclipped"
                    }
                ));
            }
            if let Some(f) = &p.ornaments.gate {
                let a = f.low.expect("validated gate span");
                let b = f.high.expect("validated gate span");
                rows.push(format!(
                    "{label} {} burst gate follows {}: {:.6}..{:.6}; main unaffected",
                    p.id,
                    f.follows,
                    a.min(b),
                    a.max(b)
                ));
            }
        }
        if count > 0 {
            rows.push(format!("{label} at most {count} followed CC candidates/bar before dedup (outgoing resets excluded)"));
        }
    }
    let mut rows = Vec::new();
    scope(c, "root", &mut rows);
    if let Some(a) = &c.arrangement {
        for s in &a.sections {
            scope(&s.composition, "section", &mut rows);
        }
    }
    if let Some(r) = &c.router {
        for s in &r.scenes {
            scope(&s.composition, &format!("scene {}", s.name), &mut rows);
        }
    }
    rows
}
