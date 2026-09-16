//! Transport-owned retained material. Neighbor lookups share state and bounded samples.
use super::{BTreeMap, BTreeSet, RetainedSource};
use crate::music::{Composition, STEP_TICKS, rhythm::Expression, router, shared};

pub(crate) fn validate(c: &Composition) -> Result<(), String> {
    if c.patterns.len() > 16 || c.patterns.keys().any(|n| n.trim().is_empty()) {
        return Err("patterns requires at most 16 nonempty names".into());
    }
    for part in &c.parts {
        if let Expression::Retained { pattern } = &part.trigger.rhythm {
            if !c.patterns.contains_key(pattern) {
                return Err(format!(
                    "Part {:?}: unknown retained pattern {pattern:?}",
                    part.id
                ));
            }
            if part.subdivision.0 != STEP_TICKS {
                return Err("retained pattern readers require subdivision = 1/16".into());
            }
        }
        if matches!(part.accent.rhythm, Expression::Retained { .. }) {
            return Err("retained material is currently a trigger source".into());
        }
    }
    if c.accents
        .values()
        .any(|a| matches!(a.rhythm, Expression::Retained { .. }))
    {
        return Err("retained material is currently a trigger source".into());
    }
    if c.router.is_some() || c.arrangement.is_some() {
        validate_root(c)?;
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
        if !c.patterns.is_empty() && (child.router.is_some() || child.arrangement.is_some()) {
            return Err("retained transport requires flat scene/section snapshots".into());
        }
        if toml::Value::try_from(&c.patterns).map_err(|e| e.to_string())?
            != toml::Value::try_from(&child.patterns).map_err(|e| e.to_string())?
        {
            return Err(
                "retained patterns belong to the root transport and must match every scene/section"
                    .into(),
            );
        }
    }
    if !c.patterns.is_empty()
        && let Some(r) = &c.router
        && !r.period_ticks(c)?.is_multiple_of(STEP_TICKS)
    {
        return Err("retained patterns require router boundaries on sixteenth slots".into());
    }
    Ok(())
}
pub(crate) fn validate_root(c: &Composition) -> Result<(), String> {
    let names = scene_names(c);
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    for (name, pattern) in &c.patterns {
        pattern.bind(name, &names)?;
    }
    Ok(())
}
fn scene_names(c: &Composition) -> BTreeSet<String> {
    if let Some(r) = &c.router {
        r.scenes.iter().map(|s| s.name.clone()).collect()
    } else if let Some(a) = &c.arrangement {
        a.sections.iter().map(|s| s.phrase.clone()).collect()
    } else {
        BTreeSet::from(["default".into()])
    }
}

pub(crate) struct Transport {
    composition: Composition,
    initial: BTreeMap<String, RetainedSource>,
    sources: BTreeMap<String, RetainedSource>,
    next: u64,
    samples: BTreeMap<u64, BTreeMap<String, bool>>,
    moves: Vec<router::Move>,
    lanes: shared::Cache,
    period: u64,
}
impl Transport {
    pub(crate) fn new(c: &Composition) -> Self {
        let names = scene_names(c);
        let names: Vec<_> = names.iter().map(String::as_str).collect();
        let initial: BTreeMap<_, _> = c
            .patterns
            .iter()
            .map(|(name, p)| {
                (
                    name.clone(),
                    p.bind(name, &names).expect("validated retained vocabulary"),
                )
            })
            .collect();
        Self {
            composition: c.clone(),
            sources: initial.clone(),
            initial,
            next: 0,
            samples: BTreeMap::new(),
            moves: Vec::new(),
            lanes: Default::default(),
            period: c
                .router
                .as_ref()
                .map_or(0, |r| r.period_ticks(c).expect("validated router")),
        }
    }
    pub(crate) fn at(&mut self, slot: u64) -> BTreeMap<String, bool> {
        if let Some(sample) = self.samples.get(&slot) {
            return sample.clone();
        }
        if slot < self.next {
            self.sources = self.initial.clone();
            self.next = 0;
        }
        while self.next <= slot {
            let c = &self.composition;
            let scene = if let Some(r) = &c.router {
                let tick = self.next * STEP_TICKS;
                let index = tick / self.period;
                if (self.moves.len() as u64) < index {
                    r.extend_moves(c, self.period, &mut self.moves, index, &mut self.lanes);
                }
                // Only membership is needed here, not the most recent move's entry
                // boundary. Avoid scanning a long sequence of stays on every slot.
                if index == 0 {
                    r.start.clone()
                } else {
                    self.moves[index as usize - 1].to.clone()
                }
            } else if let Some(a) = &c.arrangement {
                a.locate(self.next)
                    .map(|s| s.section.phrase.clone())
                    .unwrap_or_else(|| a.sections.last().unwrap().phrase.clone())
            } else {
                "default".into()
            };
            let mut sample = BTreeMap::new();
            for (name, source) in &mut self.sources {
                source
                    .advance(&scene, c.dice())
                    .expect("validated retained transport");
                let material = source.state().material();
                sample.insert(
                    name.clone(),
                    material[(self.next % material.len() as u64) as usize],
                );
            }
            if self.samples.len() >= 4096 {
                self.samples.pop_first();
            }
            self.samples.insert(self.next, sample);
            self.next += 1;
        }
        self.samples[&slot].clone()
    }
}
