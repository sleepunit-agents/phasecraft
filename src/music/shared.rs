//! Transport-clocked bounded target lanes and explicit readers.
use super::{Composition, PPQN, resolve::decision_roll};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
const BAR: u64 = 4 * PPQN;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub start: f64,
    pub range: [f64; 2],
    pub carry: Carry,
    pub target: Target,
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
    pub low: f64,
    pub high: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unclipped: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
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
    pub fn validate(&self) -> Result<(), String> {
        let [lo, hi] = self.range;
        if !lo.is_finite()
            || !hi.is_finite()
            || lo >= hi
            || !(hi - lo).is_finite()
            || !self.start.is_finite()
            || !(lo..=hi).contains(&self.start)
        {
            return Err("lane requires finite increasing range and start within it".into());
        }
        if !(1..=65536).contains(&self.target.every.bars)
            || self.target.ramp.bars == 0
            || self.target.ramp.bars >= self.target.every.bars
        {
            return Err(
                "target.every.bars must be 2..65536 and ramp.bars must be positive and shorter"
                    .into(),
            );
        }
        if self.target.delta.is_empty()
            || self.target.delta.len() > 64
            || self
                .target
                .delta
                .iter()
                .any(|d| !d.is_finite() || !(lo + d).is_finite() || !(hi + d).is_finite())
        {
            return Err("target.delta requires 1..64 finite, non-overflowing choices".into());
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
        let bar = tick / BAR;
        let every = u64::from(self.target.every.bars);
        let occurrence = bar / every;
        let key = name.to_owned();
        let (mut k, mut origin) = cache
            .entries
            .range((key.clone(), 0)..=(key.clone(), occurrence.saturating_sub(1)))
            .next_back()
            .map(|((_, k), v)| (*k, *v))
            .unwrap_or((0, self.start));
        while k < occurrence.saturating_sub(1) {
            k += 1;
            origin = self.next(name, seed, k, origin).0;
            if cache.entries.len() >= 4096 {
                cache.entries.pop_first();
            }
            cache.entries.insert((key.clone(), k), origin);
        }
        let (target, roll, progress) = if occurrence == 0 {
            (self.start, None, 0.0)
        } else {
            let (target, roll) = self.next(name, seed, occurrence, origin);
            (
                target,
                Some(roll),
                (bar % every).min(u64::from(self.target.ramp.bars)) as f64
                    / f64::from(self.target.ramp.bars),
            )
        };
        Sample {
            name: key,
            tick,
            value: origin + (target - origin) * progress,
            occurrence,
            origin,
            target,
            progress,
            roll,
        }
    }
    fn next(&self, name: &str, seed: u64, k: u64, from: f64) -> (f64, f64) {
        let roll = decision_roll(seed, name, "target", k, "delta");
        let delta = self.target.delta[(roll * self.target.delta.len() as f64) as usize];
        ((from + delta).clamp(self.range[0], self.range[1]), roll)
    }
}
impl Follower {
    pub fn mapped(&self, lane: &Lane, value: f64, default: f64) -> f64 {
        let fraction = (value - lane.range[0]) / (lane.range[1] - lane.range[0]);
        let mapped = self.low + (self.high - self.low) * fraction;
        match self.op {
            Operation::Range => mapped,
            Operation::ScaleDefault => mapped * default,
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
        if !self.low.is_finite() || !self.high.is_finite() || !(self.high - self.low).is_finite() {
            return Err("follower span must be finite".into());
        }
        if gate && self.op != Operation::Range {
            return Err("ratchet gate requires op = range".into());
        }
        if self.op == Operation::ScaleDefault && default.is_none() {
            return Err("scale-default requires an explicit output control default".into());
        }
        for edge in lane.range {
            let value = self.mapped(lane, edge, default.unwrap_or(1.0));
            if !value.is_finite() || ((gate || self.unclipped) && !(0.0..=1.0).contains(&value)) {
                return Err("follower reaches outside 0..1 (gate or unclipped promise)".into());
            }
        }
        Ok(())
    }
}
pub fn validate(c: &Composition) -> Result<(), String> {
    if c.lanes.len() > 16 {
        return Err("at most 16 shared target lanes".into());
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
                let a = f.mapped(lane, lane.range[0], default);
                let b = f.mapped(lane, lane.range[1], default);
                let (lo, hi) = (a.min(b), a.max(b));
                rows.push(format!(
                    "{label} {}.{name} follows {}: raw {lo:.6}..{hi:.6}, control {:.6}..{:.6}, {}",
                    p.id,
                    f.follows,
                    lo.clamp(0.0, 1.0),
                    hi.clamp(0.0, 1.0),
                    if lo < 0.0 || hi > 1.0 {
                        "saturates"
                    } else {
                        "unclipped"
                    }
                ));
            }
            if let Some(f) = &p.ornaments.gate {
                rows.push(format!(
                    "{label} {} burst gate follows {}: {:.6}..{:.6}; main unaffected",
                    p.id,
                    f.follows,
                    f.low.min(f.high),
                    f.low.max(f.high)
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
