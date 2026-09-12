//! The return group and the returns-clocked router. `docs/router.md` is the contract.
//!
//! A return group names fixed transport periods elsewhere in the piece and returns when they all
//! line up: its period is the checked LCM in ticks. The router asks "move?" at every return —
//! absolute ticks `period · r`, r ≥ 1, never counted from scene entry — and rolls the current
//! scene's row once, at `moves/<r>`. A stay is a route to yourself and is not a move.
//!
//! Nothing here decides early (`decide` is M2), publishes an events stream (M2) or honours a pin
//! (M1.8). The seam for all three is [`Move`]: one record per return, in order.
use super::{Composition, STEP_TICKS, resolve::decision_roll};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_GROUPS: usize = 16;
pub const MAX_SCENES: usize = 64;
/// The weight word that absorbs whatever a row's named entries leave.
pub const REST: &str = "rest";
/// Row weights are decimal fractions in a file; a sum gets this much slack for it.
const SUM_TOLERANCE: f64 = 1e-9;

// ---------------------------------------------------------------- return groups

/// `[returns.<name>] align = [...]`: the author writes `align`; the engine calls them members.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReturnGroup {
    #[serde(rename = "align", alias = "members")]
    pub members: Vec<String>,
}
/// What one `align` key resolves to in one composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// A fixed transport period, in ticks.
    Fixed(u64),
    /// Advances on events rather than on the transport. Never a member.
    Event,
    /// The key's owner is not in this composition — a scene that dropped the voice.
    Absent,
}
/// Resolve one member key. The shapes this engine can read today are
/// `voices.<id>.trigger.cycle`, `voices.<id>.accent.cycle`, and
/// `voices.<id>.velocity.cycle` (`parts.` is the same word). Velocity per=event
/// is an event clock even when it has a periodic reset; per=step has a transport period.
/// `lanes.<name>.every` names a walk's step clock, not a repetition of its values.
/// `patterns.*` (M1.5) remains an error that names the key. Nothing is stubbed.
pub fn member_clock(c: &Composition, key: &str) -> Result<Clock, String> {
    let segments: Vec<&str> = key.split('.').collect();
    match segments.as_slice() {
        ["voices" | "parts", id, "velocity", "cycle"] => {
            let Some(part) = c.parts.iter().find(|p| p.id == *id) else {
                return Ok(Clock::Absent);
            };
            let value = part
                .velocity
                .as_ref()
                .ok_or_else(|| format!("{key}: no velocity value pattern"))?;
            Ok(if value.per() == super::process::Per::Event {
                Clock::Event
            } else {
                Clock::Fixed(value.cycle_ticks())
            })
        }

        [
            "voices" | "parts",
            id,
            stream @ ("trigger" | "accent"),
            "cycle",
        ] => {
            let Some(part) = c.parts.iter().find(|p| p.id == *id) else {
                return Ok(Clock::Absent);
            };
            let period = if *stream == "trigger" {
                super::cycle::trigger_period_ticks(c, part)?
            } else {
                super::cycle::accent_period_ticks(c, part)?
            };
            period
                .map(Clock::Fixed)
                .ok_or_else(|| format!("{key}: the {stream} period does not fit in u64 ticks"))
        }
        ["voices" | "parts", ..] => Err(format!(
            "{key}: a voice's clocks are trigger.cycle, accent.cycle and velocity.cycle; velocity per=event has no fixed transport period"
        )),
        ["patterns", ..] => Err(format!(
            "{key}: patterns are not in this engine yet (M1.5), so the key cannot be resolved; it is not guessed"
        )),
        ["lanes", name, "every"] => {
            let lane = c
                .lanes
                .get(*name)
                .ok_or_else(|| format!("{key}: unknown shared lane"))?;
            match lane {
                super::shared::Lane::Walk(_) => Ok(Clock::Fixed(lane.every_ticks())),
                super::shared::Lane::Target(_) => Err(format!(
                    "{key}: target lanes have no member clock in this engine, under this or any other spelling, so the key cannot be resolved; it is not guessed"
                )),
            }
        }
        ["lanes", ..] => Err(format!(
            "{key}: not a lane member clock; only a walk lane has one, spelled lanes.<name>.every"
        )),
        _ => Err(format!(
            "{key}: not a clock this engine can name; a member is voices.<id>.trigger.cycle, voices.<id>.accent.cycle, patterns.<name>.change.every or lanes.<name>.every"
        )),
    }
}
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
/// The checked LCM of named periods: an overflow names the member that caused it.
pub fn lcm_ticks(group: &str, periods: &[(&str, u64)]) -> Result<u64, String> {
    let mut period = 1u64;
    for (key, ticks) in periods {
        if *ticks == 0 {
            return Err(format!("returns.{group}: {key} has a period of zero ticks"));
        }
        period = (period / gcd(period, *ticks))
            .checked_mul(*ticks)
            .ok_or_else(|| {
                format!(
                    "returns.{group}: the period of {key} does not fit in u64 ticks together with the members before it"
                )
            })?;
    }
    Ok(period)
}
impl ReturnGroup {
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("returns: a group needs a name".into());
        }
        if self.members.is_empty() {
            return Err(format!("returns.{name}: align needs at least one member"));
        }
        let mut seen = std::collections::HashSet::new();
        for key in &self.members {
            if key.trim().is_empty() || !seen.insert(key) {
                return Err(format!("returns.{name}: empty or duplicate member {key:?}"));
            }
        }
        Ok(())
    }
    /// Every member's clock in one composition, in authored order.
    pub fn resolve<'a>(
        &'a self,
        c: &Composition,
        name: &str,
    ) -> Result<Vec<(&'a str, Clock)>, String> {
        self.members
            .iter()
            .map(|key| {
                member_clock(c, key)
                    .map(|clock| (key.as_str(), clock))
                    .map_err(|e| format!("returns.{name}: {e}"))
            })
            .collect()
    }
    /// The period from resolved clocks. An event-clocked member is refused; an absent one is
    /// refused too unless `absent_ok`, which a scene gets because the clock is the transport's
    /// and the voice merely left the room.
    pub fn period_from(
        name: &str,
        clocks: &[(&str, Clock)],
        absent_ok: bool,
    ) -> Result<u64, String> {
        let mut fixed = Vec::new();
        for (key, clock) in clocks {
            match clock {
                Clock::Fixed(ticks) => fixed.push((*key, *ticks)),
                Clock::Event => {
                    return Err(format!(
                        "returns.{name}: {key} is event-clocked; align accepts members with a fixed transport period and nothing else"
                    ));
                }
                Clock::Absent if absent_ok => {}
                Clock::Absent => {
                    return Err(format!(
                        "returns.{name}: {key} names a voice the piece does not have"
                    ));
                }
            }
        }
        lcm_ticks(name, &fixed)
    }
    /// The group's period against the piece: the checked LCM of every member, all present.
    pub fn period_ticks(&self, c: &Composition, name: &str) -> Result<u64, String> {
        Self::period_from(name, &self.resolve(c, name)?, false)
    }
}

// ---------------------------------------------------------------- the router

/// `[router]`: the stochastic arrangement. Scenes are phrases; `scenes` keeps the authored order,
/// which is the order every row is read in.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Router {
    pub every: Every,
    pub start: String,
    pub scenes: Vec<Scene>,
    pub routes: BTreeMap<String, Row>,
}
/// `every = { returns = "<group>" }`. `bars` is named so its refusal can say why.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Every {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bars: Option<u64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub name: String,
    pub composition: Box<Composition>,
}
/// One scene's routes out, keyed by destination. A destination the row does not name is a wall.
pub type Row = BTreeMap<String, Weight>;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Weight {
    Fixed(f64),
    Follows(Follows),
    /// Only `"rest"`: whatever the named entries leave.
    Word(String),
}
/// `{ follows = "<lane>", low, high }`: the weight runs linearly from `low` at the bottom of the
/// lane's range to `high` at the top, read at the roll.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Follows {
    pub follows: String,
    pub low: f64,
    pub high: f64,
}
/// A lane's reachable interval, which is all the load-time check needs to know about a lane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub low: f64,
    pub high: f64,
}
impl Follows {
    /// The weight at lane value `v` within `range`; a flat range gives `low`.
    pub fn at(&self, v: f64, range: Range) -> f64 {
        if range.high == range.low {
            return self.low;
        }
        let t = ((v - range.low) / (range.high - range.low)).clamp(0.0, 1.0);
        self.low + (self.high - self.low) * t
    }
}
/// A sum for an error message: the decimal an author wrote, not its f64 residue.
fn shown(sum: f64) -> f64 {
    (sum * 1e9).round() / 1e9
}
fn unit(v: f64) -> bool {
    v.is_finite() && (0.0..=1.0).contains(&v)
}
/// Validate one row and its sum. `ranges` answers what a followed lane can reach; a lane it does
/// not know is a load error naming the lane, never a guess.
pub fn check_row(
    name: &str,
    row: &Row,
    scenes: &[String],
    ranges: &dyn Fn(&str) -> Option<Range>,
) -> Result<(), String> {
    let mut fixed = 0.0;
    let mut rest = 0;
    let mut follows: Vec<(&Follows, Range)> = Vec::new();
    for (to, weight) in row {
        if !scenes.iter().any(|s| s == to) {
            return Err(format!(
                "router.routes.{name}: {to:?} is not a scene; a destination is a scene name"
            ));
        }
        match weight {
            Weight::Fixed(w) if unit(*w) => fixed += w,
            Weight::Fixed(w) => {
                return Err(format!(
                    "router.routes.{name}.{to}: weight {w} is not within 0..1"
                ));
            }
            Weight::Word(word) if word == REST => rest += 1,
            Weight::Word(word) => {
                return Err(format!(
                    "router.routes.{name}.{to}: {word:?} is not a weight; write a number, \"rest\", or {{ follows = ..., low, high }}"
                ));
            }
            Weight::Follows(f) => {
                if !unit(f.low) || !unit(f.high) {
                    return Err(format!(
                        "router.routes.{name}.{to}: low and high must be within 0..1"
                    ));
                }
                let range = ranges(&f.follows).ok_or_else(|| {
                    format!(
                        "router.routes.{name}.{to} follows {:?}, which is not a lane this engine has (M1.4); the row cannot be checked",
                        f.follows
                    )
                })?;
                follows.push((f, range));
            }
        }
    }
    if rest > 1 {
        return Err(format!(
            "router.routes.{name}: \"rest\" appears {rest} times; it is the remainder and there is one"
        ));
    }
    if follows.len() > 16 {
        return Err(format!(
            "router.routes.{name}: at most 16 followed weights in one row"
        ));
    }
    // One axis per named source: two readers cannot see opposite ends of the
    // same lane at the same roll. Different sources remain independent axes.
    let mut lanes = BTreeMap::new();
    for (f, range) in &follows {
        let index = lanes.len();
        lanes.entry(f.follows.as_str()).or_insert((index, *range));
    }
    // Every follower is linear in its lane, so a sum over the range is extremal at the corners.
    for corner in 0u32..(1 << lanes.len()) {
        let sum = fixed
            + follows
                .iter()
                .map(|(f, range)| {
                    let i = lanes[f.follows.as_str()].0;
                    f.at(
                        if corner & (1 << i) == 0 {
                            range.low
                        } else {
                            range.high
                        },
                        *range,
                    )
                })
                .sum::<f64>();
        let at = || {
            lanes
                .iter()
                .map(|(name, (i, range))| {
                    format!(
                        " at {} = {}",
                        name,
                        if corner & (1 << i) == 0 {
                            range.low
                        } else {
                            range.high
                        }
                    )
                })
                .collect::<String>()
        };
        if rest == 1 && sum > 1.0 + SUM_TOLERANCE {
            return Err(format!(
                "router.routes.{name}: the named entries sum to {}{}, leaving nothing for \"rest\"",
                shown(sum),
                at()
            ));
        }
        if rest == 0 && (sum - 1.0).abs() > SUM_TOLERANCE {
            return Err(format!(
                "router.routes.{name} sums to {}{}, not 1",
                shown(sum),
                at()
            ));
        }
    }
    Ok(())
}
/// Read a row as cumulative intervals in scene order and land the draw `u` in one. The same `u`
/// means different rooms from different rows; that is the property pins depend on.
pub fn pick(
    row: &Row,
    scenes: &[String],
    u: f64,
    lane: &dyn Fn(&str) -> Option<(f64, Range)>,
) -> Result<String, String> {
    let weight = |to: &str| -> Result<f64, String> {
        Ok(match row.get(to) {
            None => 0.0,
            Some(Weight::Fixed(w)) => *w,
            Some(Weight::Follows(f)) => {
                let (v, range) = lane(&f.follows)
                    .ok_or_else(|| format!("lane {:?} has no value at the roll", f.follows))?;
                f.at(v, range)
            }
            Some(Weight::Word(_)) => 0.0,
        })
    };
    let mut named = 0.0;
    for (to, w) in row {
        if !matches!(w, Weight::Word(_)) {
            named += weight(to)?;
        }
    }
    let rest = (1.0 - named).max(0.0);
    let mut cumulative = 0.0;
    let mut last = None;
    for scene in scenes {
        let w = match row.get(scene.as_str()) {
            Some(Weight::Word(_)) => rest,
            _ => weight(scene)?,
        };
        if w <= 0.0 {
            continue;
        }
        cumulative += w;
        last = Some(scene.clone());
        if u < cumulative {
            return Ok(scene.clone());
        }
    }
    // Decimal weights can sum a hair under 1; a draw past the last interval lands in it.
    last.ok_or_else(|| "a row with no weight has no door".into())
}

/// One return, in order: the record M2's events stream subscribes to and `inspect` prints.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Move {
    /// Return index r ≥ 1: the roll's occurrence, and a pin's `return`.
    pub index: u64,
    /// The landing tick, `period · r`. Without `decide` the roll is made here too.
    pub tick: u64,
    pub from: String,
    pub to: String,
    /// The draw at `moves/<r>`.
    pub roll: f64,
    /// A change of scene. A stay is a route to yourself, and it is not a move.
    pub moved: bool,
}
/// Where the piece is at one tick, for a trace.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Visit {
    pub scene: String,
    /// Returns elapsed: 0 before the first return.
    pub index: u64,
    /// `period · index`: the return that opened this visit, or 0.
    pub start_tick: u64,
    /// The tick of the move that opened this run of the scene, or 0: where a scene-gated history
    /// begins. A stay keeps it.
    pub entered_tick: u64,
    /// The next return, where the router asks again.
    pub next_tick: u64,
    /// The scene the run was entered from; none at the start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}
/// The draw at `moves/<r>`: the router is a composition-level owner, not a Part.
pub fn roll(seed: u64, index: u64) -> f64 {
    decision_roll(seed, "moves", "door", index, "u")
}
impl Router {
    pub fn scene_names(&self) -> Vec<String> {
        self.scenes.iter().map(|s| s.name.clone()).collect()
    }
    pub fn scene(&self, name: &str) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.name == name)
    }
    /// The group this router is clocked by, and its period against the piece.
    pub fn period_ticks(&self, c: &Composition) -> Result<u64, String> {
        let name = self.every.returns.as_deref().ok_or(
            "router.every needs { returns = \"<group>\" }; a router clocked in bars is M2",
        )?;
        let group = c
            .returns
            .get(name)
            .ok_or_else(|| format!("router.every.returns = {name:?}: no [returns.{name}]"))?;
        group.period_ticks(c, name)
    }
    pub fn validate(&self, c: &Composition) -> Result<(), String> {
        if self.every.bars.is_some() {
            return Err(
                "router.every = { bars } is M2 (decide and the promise); this router is clocked by a return group"
                    .into(),
            );
        }
        let period = self.period_ticks(c)?;
        if !period.is_multiple_of(STEP_TICKS) {
            return Err(format!(
                "router: the return group's period is {period} ticks, which is not on the sixteenth grid; the router changes scene on that grid"
            ));
        }
        if self.scenes.is_empty() || self.scenes.len() > MAX_SCENES {
            return Err(format!("router requires 1..{MAX_SCENES} scenes"));
        }
        let names = self.scene_names();
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            if name.trim().is_empty() || name == REST || !seen.insert(name) {
                return Err(format!(
                    "router: scene names must be nonempty and distinct, and {REST:?} is a weight word, not a scene: {name:?}"
                ));
            }
        }
        if !seen.contains(&self.start) {
            return Err(format!(
                "start = {:?} is not a scene; the router has {}",
                self.start,
                names.join(", ")
            ));
        }
        let group_name = self.every.returns.as_deref().unwrap();
        let group = &c.returns[group_name];
        let base = group.resolve(c, group_name)?;
        for scene in &self.scenes {
            let s = scene.composition.as_ref();
            if s.arrangement.is_some() || s.router.is_some() {
                return Err(format!(
                    "scene {:?}: a scene cannot carry an arrangement or a router",
                    scene.name
                ));
            }
            if s.tempo != c.tempo {
                return Err(format!(
                    "scene {:?}: every scene plays at the piece's tempo",
                    scene.name
                ));
            }
            if !s.returns.is_empty() {
                return Err(format!(
                    "scene {:?}: return groups belong to the piece, not to a scene",
                    scene.name
                ));
            }
            s.validate()
                .map_err(|e| format!("scene {:?}: {e}", scene.name))?;
            for ((key, root), (_, here)) in base.iter().zip(group.resolve(s, group_name)?) {
                match (root, here) {
                    (Clock::Fixed(a), Clock::Fixed(b)) if *a != b => {
                        return Err(format!(
                            "scene {:?} changes the period of returns.{group_name} member {key} from {a} to {b} ticks; a return group's period is the piece's, in every scene",
                            scene.name
                        ));
                    }
                    (_, Clock::Event) => {
                        return Err(format!(
                            "scene {:?}: returns.{group_name} member {key} is event-clocked there",
                            scene.name
                        ));
                    }
                    _ => {}
                }
            }
            ReturnGroup::period_from(group_name, &group.resolve(s, group_name)?, true)?;
        }
        for row in self.routes.keys() {
            if !seen.contains(row) {
                return Err(format!(
                    "router.routes.{row}: not a scene; a row is keyed by the scene it leaves"
                ));
            }
        }
        for name in &names {
            let row = self.routes.get(name).ok_or_else(|| {
                format!(
                    "router.routes has no row for scene {name:?}; a room with no row has no door, not even to itself"
                )
            })?;
            check_row(name, row, &names, &|name| {
                c.lanes.get(name).map(|lane| {
                    let [low, high] = lane.range();
                    Range { low, high }
                })
            })?;
        }
        Ok(())
    }
    /// Extend a move log through return `through`, rolling each return once in order.
    pub fn extend_moves(
        &self,
        c: &Composition,
        period: u64,
        log: &mut Vec<Move>,
        through: u64,
        cache: &mut super::shared::Cache,
    ) {
        let names = self.scene_names();
        while (log.len() as u64) < through {
            let index = log.len() as u64 + 1;
            let from = log.last().map_or(self.start.clone(), |m| m.to.clone());
            let tick = index.saturating_mul(period);
            let row = &self.routes[&from];
            // Freeze each distinct source once at this absolute return tick. `pick` may
            // consult a follower more than once; all those reads see the same value.
            let mut samples = BTreeMap::new();
            for weight in row.values() {
                if let Weight::Follows(f) = weight {
                    samples.entry(f.follows.as_str()).or_insert_with(|| {
                        let lane = &c.lanes[&f.follows];
                        let [low, high] = lane.range();
                        (
                            lane.sample(&f.follows, c.seed, tick, cache).value,
                            Range { low, high },
                        )
                    });
                }
            }
            let u = roll(c.seed, index);
            let to = pick(row, &names, u, &|name| samples.get(name).copied())
                .expect("validated router row");
            log.push(Move {
                index,
                tick,
                moved: to != from,
                from,
                to,
                roll: u,
            });
        }
    }
    /// Where the piece is at `tick`, given a log that reaches that far.
    pub fn locate(&self, period: u64, tick: u64, log: &[Move]) -> Visit {
        let index = tick / period;
        let n = usize::try_from(index).expect("return index fits usize");
        assert!(log.len() >= n, "move log must reach return {index}");
        let scene = if n == 0 {
            self.start.clone()
        } else {
            log[n - 1].to.clone()
        };
        let last_move = log[..n].iter().rev().find(|m| m.moved);
        Visit {
            scene,
            index,
            start_tick: index * period,
            entered_tick: last_move.map_or(0, |m| m.tick),
            next_tick: (index + 1).saturating_mul(period),
            from: last_move.map(|m| m.from.clone()),
        }
    }
    /// The pure form: walk the returns from the start. `Compiled` keeps the log instead.
    pub fn visit_at(&self, c: &Composition, period: u64, tick: u64) -> Visit {
        let mut log = Vec::new();
        self.extend_moves(c, period, &mut log, tick / period, &mut Default::default());
        self.locate(period, tick, &log)
    }
    /// Structural/routing edits need a restart; a scene's musical edits may reload.
    pub fn same_layout(&self, other: &Self) -> bool {
        self.every == other.every
            && self.start == other.start
            && self.routes == other.routes
            && self.scenes.len() == other.scenes.len()
            && self.scenes.iter().zip(&other.scenes).all(|(a, b)| {
                a.name == b.name
                    && a.composition.phrase_bars == b.composition.phrase_bars
                    && a.composition.parts.len() == b.composition.parts.len()
                    && a.composition
                        .parts
                        .iter()
                        .zip(&b.composition.parts)
                        .all(|(a, b)| {
                            a.id == b.id && a.output == b.output && a.subdivision == b.subdivision
                        })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SLOT: u64 = STEP_TICKS;

    // ---- the return group
    #[test]
    fn sixteen_five_and_seven_slots_line_up_every_thirty_five_bars() {
        let period = lcm_ticks(
            "hat_system",
            &[
                ("hat", 16 * SLOT),
                ("memory", 5 * SLOT),
                ("drift", 7 * SLOT),
            ],
        )
        .unwrap();
        assert_eq!(period, 560 * SLOT);
        assert_eq!(period / (16 * SLOT), 35);
    }
    #[test]
    fn the_lcm_is_checked_and_names_the_member_that_overflowed() {
        let e = lcm_ticks("g", &[("a", (1 << 40) + 1), ("b", (1 << 40) + 3)]).unwrap_err();
        assert!(
            e.contains("returns.g") && e.contains("b does not fit"),
            "{e}"
        );
        let e = lcm_ticks("g", &[("a", 0)]).unwrap_err();
        assert!(e.contains("a has a period of zero"), "{e}");
        assert_eq!(lcm_ticks("g", &[("a", 6), ("b", 4), ("c", 6)]), Ok(12));
    }
    #[test]
    fn an_event_clocked_member_is_refused() {
        let clocks = [
            ("voices.hat.trigger.cycle", Clock::Fixed(16 * SLOT)),
            ("voices.hat.emphasis", Clock::Event),
        ];
        let e = ReturnGroup::period_from("g", &clocks, false).unwrap_err();
        assert!(e.contains("voices.hat.emphasis is event-clocked"), "{e}");
        assert!(ReturnGroup::period_from("g", &clocks, true).is_err());
    }
    #[test]
    fn an_absent_member_is_refused_by_the_piece_and_allowed_in_a_scene() {
        let clocks = [
            ("voices.hat.trigger.cycle", Clock::Fixed(16 * SLOT)),
            ("voices.metal.trigger.cycle", Clock::Absent),
        ];
        let e = ReturnGroup::period_from("g", &clocks, false).unwrap_err();
        assert!(
            e.contains("voices.metal.trigger.cycle names a voice"),
            "{e}"
        );
        assert_eq!(ReturnGroup::period_from("g", &clocks, true), Ok(16 * SLOT));
    }
    fn piece(extra: &str) -> Composition {
        Composition::parse(&format!(
            "tempo=126\nseed=91827\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.rhythm={{steps=16,pulses=6}}\naccent.rhythm={{steps=5,pulses=2}}\n{extra}"
        ))
        .unwrap()
    }
    #[test]
    fn member_keys_resolve_to_stream_periods_or_name_what_this_engine_lacks() {
        let c = piece("");
        assert_eq!(
            member_clock(&c, "voices.hat.trigger.cycle"),
            Ok(Clock::Fixed(16 * SLOT))
        );
        assert_eq!(
            member_clock(&c, "parts.hat.trigger.cycle"),
            Ok(Clock::Fixed(16 * SLOT))
        );
        assert_eq!(
            member_clock(&c, "voices.hat.accent.cycle"),
            Ok(Clock::Fixed(5 * SLOT))
        );
        assert_eq!(
            member_clock(&c, "voices.metal.trigger.cycle"),
            Ok(Clock::Absent)
        );
        for (key, says) in [
            ("patterns.hat-memory.change.every", "M1.5"),
            ("lanes.drift.every", "unknown shared lane"),
            ("voices.hat.velocity", "velocity.cycle"),
            ("weather", "not a clock"),
        ] {
            let e = member_clock(&c, key).unwrap_err();
            assert!(e.starts_with(key) && e.contains(says), "{e}");
        }
    }
    #[test]
    fn a_return_group_is_checked_at_load_whether_or_not_a_router_uses_it() {
        let e = Composition::parse(
            "tempo=126\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\n[returns.g]\nalign=['voices.hat.trigger.cycle','lanes.drift.every']",
        )
        .unwrap_err();
        assert!(e.contains("returns.g: lanes.drift.every"), "{e}");
        let c =
            piece("[returns.g]\nmembers=['voices.hat.trigger.cycle','voices.hat.accent.cycle']");
        assert_eq!(c.returns["g"].period_ticks(&c, "g"), Ok(80 * SLOT));
        let e = Composition::parse(
            "tempo=126\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\n[returns.g2]\nalign=[]",
        )
        .unwrap_err();
        assert!(
            e.contains("returns.g2: align needs at least one member"),
            "{e}"
        );
    }

    // ---- rows
    fn scenes() -> Vec<String> {
        ["crowded", "exposed", "hollow"].map(String::from).to_vec()
    }
    fn row(text: &str) -> Row {
        toml::from_str(text).unwrap()
    }
    #[test]
    fn rows_sum_to_one_and_the_error_names_the_row_and_the_sum() {
        check_row(
            "crowded",
            &row("crowded=0.70\nexposed=0.25\nhollow=0.05"),
            &scenes(),
            &|_| None,
        )
        .unwrap();
        check_row("hollow", &row("hollow=1"), &scenes(), &|_| None).unwrap();
        let e = check_row(
            "crowded",
            &row("crowded=0.70\nexposed=0.25"),
            &scenes(),
            &|_| None,
        )
        .unwrap_err();
        assert_eq!(e, "router.routes.crowded sums to 0.95, not 1");
        let e = check_row(
            "crowded",
            &row("crowded=0.70\nfoyer=0.30"),
            &scenes(),
            &|_| None,
        )
        .unwrap_err();
        assert!(e.contains("\"foyer\" is not a scene"), "{e}");
        let e = check_row(
            "crowded",
            &row("crowded='rest'\nexposed='rest'"),
            &scenes(),
            &|_| None,
        )
        .unwrap_err();
        assert!(e.contains("\"rest\" appears 2 times"), "{e}");
        let e = check_row("crowded", &row("crowded='most'"), &scenes(), &|_| None).unwrap_err();
        assert!(e.contains("\"most\" is not a weight"), "{e}");
        let e = check_row("crowded", &row("crowded=1.5"), &scenes(), &|_| None).unwrap_err();
        assert!(e.contains("weight 1.5 is not within 0..1"), "{e}");
    }
    #[test]
    fn a_row_that_follows_a_lane_must_sum_to_one_at_every_value_of_its_range() {
        // run-the-line's firefight row, checked against a fake pressure range: no Source is built.
        let pressure = |lane: &str| {
            (lane == "pressure").then_some(Range {
                low: 0.0,
                high: 1.0,
            })
        };
        let rooms = ["pursuit", "firefight", "getaway", "aftermath"]
            .map(String::from)
            .to_vec();
        let firefight = row(
            "pursuit=0.05\nfirefight='rest'\ngetaway=0.40\naftermath={follows='pressure',low=0.05,high=0.45}",
        );
        check_row("firefight", &firefight, &rooms, &pressure).unwrap();
        let e = check_row("firefight", &firefight, &rooms, &|_| None).unwrap_err();
        assert!(
            e.contains("follows \"pressure\", which is not a lane this engine has"),
            "{e}"
        );
        let e = check_row(
            "firefight",
            &row("pursuit=0.55\ngetaway={follows='pressure',low=0.05,high=0.45}"),
            &rooms,
            &pressure,
        )
        .unwrap_err();
        assert_eq!(
            e,
            "router.routes.firefight sums to 0.6 at pressure = 0, not 1"
        );
        let e = check_row(
            "firefight",
            &row("pursuit=0.60\nfirefight='rest'\ngetaway={follows='pressure',low=0.05,high=0.45}"),
            &rooms,
            &pressure,
        )
        .unwrap_err();
        assert!(
            e.contains("sum to 1.05 at pressure = 1, leaving nothing"),
            "{e}"
        );
        // A single follower alongside fixed weights must be flat without a remainder.
        check_row(
            "firefight",
            &row("pursuit=0.5\ngetaway={follows='pressure',low=0.5,high=0.5}"),
            &rooms,
            &pressure,
        )
        .unwrap();
    }
    #[test]
    fn complementary_weights_share_one_lane_value() {
        let rooms = vec!["a".into(), "b".into(), "c".into()];
        let range = Range {
            low: -30.0,
            high: 30.0,
        };
        let ranges = |name: &str| (name == "drift").then_some(range);
        let complementary =
            row("a={follows='drift',low=0.0,high=1.0}\nb={follows='drift',low=1.0,high=0.0}");
        check_row("a", &complementary, &rooms, &ranges).unwrap();
        // The accepted row and the runtime reader agree at both edges and inside.
        for (value, expected) in [(-30.0, "b"), (0.0, "a"), (30.0, "a")] {
            assert_eq!(
                pick(&complementary, &rooms, 0.25, &|name| {
                    ranges(name).map(|span| (value, span))
                })
                .unwrap(),
                expected
            );
        }
        let remainder = row(
            "a={follows='drift',low=0.0,high=0.75}\nb={follows='drift',low=0.75,high=0.0}\nc='rest'",
        );
        check_row("a", &remainder, &rooms, &ranges).unwrap();
        for value in [-30.0, 0.0, 30.0] {
            assert_eq!(
                pick(&remainder, &rooms, 0.9, &|name| {
                    ranges(name).map(|span| (value, span))
                })
                .unwrap(),
                "c"
            );
        }
    }

    #[test]
    fn independent_lanes_still_check_mixed_corners() {
        let rooms = vec!["a".into(), "b".into(), "c".into()];
        let ranges = |_: &str| {
            Some(Range {
                low: 0.0,
                high: 1.0,
            })
        };
        // Both all-low and all-high sum to one. A mixed corner exceeds one.
        let independent =
            row("a={follows='p',low=0.0,high=1.0}\nb={follows='q',low=1.0,high=0.0}\nc='rest'");
        let e = check_row("a", &independent, &rooms, &ranges).unwrap_err();
        assert!(e.contains("sum to 2"), "{e}");
        assert!(e.contains("at p = 1") && e.contains("at q = 0"), "{e}");
        // Sharing a source must not excuse a genuinely overweight endpoint.
        let invalid =
            row("a={follows='p',low=0.0,high=0.8}\nb={follows='p',low=0.5,high=0.3}\nc='rest'");
        let e = check_row("a", &invalid, &rooms, &ranges).unwrap_err();
        assert!(e.contains("sum to 1.1 at p = 1"), "{e}");
    }

    #[test]
    fn the_same_draw_means_different_rooms_from_different_rows() {
        // seeds.toml's own example: pin u = 0.50 and the room depends on where the piece got to.
        let routes: BTreeMap<String, Row> = toml::from_str(
            "crowded={crowded=0.70,exposed=0.25,hollow=0.05}\nexposed={crowded=0.30,exposed=0.55,hollow=0.15}\nhollow={crowded=0.30,exposed=0.45,hollow=0.25}",
        )
        .unwrap();
        let at = |from: &str, u: f64| pick(&routes[from], &scenes(), u, &|_| None).unwrap();
        assert_eq!(at("crowded", 0.50), "crowded");
        assert_eq!(at("exposed", 0.50), "exposed");
        assert_eq!(at("hollow", 0.50), "exposed");
        for from in ["crowded", "exposed", "hollow"] {
            assert_eq!(at(from, 0.97), "hollow", "{from}");
        }
        // Intervals are half-open on the right, and an edge sits where f64 addition puts it:
        // 0.30 + 0.55 is a hair above 0.85, so 0.85 is still exposed and 0.86 is hollow.
        assert_eq!(at("crowded", 0.70), "exposed");
        assert_eq!(at("crowded", 0.0), "crowded");
        assert_eq!(at("exposed", 0.85), "exposed");
        assert_eq!(at("exposed", 0.86), "hollow");
    }
    #[test]
    fn rest_absorbs_the_remainder_and_a_short_sum_still_lands_in_the_last_room() {
        let rooms = ["a", "b", "c"].map(String::from).to_vec();
        let r = row("a=0.25\nb='rest'\nc={follows='p',low=0.0,high=0.5}");
        let lane = |v: f64| {
            move |lane: &str| {
                (lane == "p").then_some((
                    v,
                    Range {
                        low: 0.0,
                        high: 1.0,
                    },
                ))
            }
        };
        // p = 1: c takes 0.5, rest is 0.25; cumulative a .25, b .5, c 1.
        assert_eq!(pick(&r, &rooms, 0.3, &lane(1.0)).unwrap(), "b");
        assert_eq!(pick(&r, &rooms, 0.6, &lane(1.0)).unwrap(), "c");
        // p = 0: c takes nothing and b absorbs 0.75; a draw in c's old interval is b's now.
        assert_eq!(pick(&r, &rooms, 0.6, &lane(0.0)).unwrap(), "b");
        let e = pick(&r, &rooms, 0.6, &|_| None).unwrap_err();
        assert!(e.contains("\"p\" has no value at the roll"), "{e}");
        let short = row("a=0.1\nb=0.2");
        assert_eq!(pick(&short, &rooms, 0.999, &|_| None).unwrap(), "b");
    }

    // ---- the walk
    fn routed(routes: &str, seed: u64) -> Composition {
        Composition::parse(&format!(
            "tempo=126\nseed={seed}\nstart='crowded'\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.rhythm={{steps=16,pulses=6}}\n[scenes.crowded]\n[scenes.exposed]\nparts.hat.trigger.probability=0.5\n[scenes.hollow]\nparts.hat.trigger.probability=0.2\n[router]\nevery={{returns='hat_system'}}\n[router.routes]\n{routes}\n[returns.hat_system]\nalign=['voices.hat.trigger.cycle']"
        ))
        .unwrap()
    }
    const PIECE_ROWS: &str = "crowded={crowded=0.70,exposed=0.25,hollow=0.05}\nexposed={crowded=0.30,exposed=0.55,hollow=0.15}\nhollow={crowded=0.30,exposed=0.45,hollow=0.25}";
    #[test]
    fn returns_are_absolute_and_the_first_roll_is_at_the_first_return_not_at_zero() {
        let c = routed(PIECE_ROWS, 91827);
        let r = c.router.as_ref().unwrap();
        let period = r.period_ticks(&c).unwrap();
        assert_eq!(period, 16 * SLOT);
        let mut log = Vec::new();
        r.extend_moves(&c, period, &mut log, 3, &mut Default::default());
        assert_eq!(
            log.iter().map(|m| (m.index, m.tick)).collect::<Vec<_>>(),
            vec![(1, period), (2, 2 * period), (3, 3 * period)]
        );
        assert_eq!(log[0].from, "crowded");
        for m in &log {
            assert_eq!(m.roll, roll(91827, m.index));
            assert_eq!(m.roll, decision_roll(91827, "moves", "door", m.index, "u"));
            assert_eq!(m.moved, m.from != m.to);
        }
        let before = r.locate(period, period - 1, &log);
        assert_eq!(
            (
                before.scene.as_str(),
                before.index,
                before.start_tick,
                before.next_tick
            ),
            ("crowded", 0, 0, period)
        );
        let after = r.locate(period, period, &log);
        assert_eq!(
            (after.scene.as_str(), after.index, after.start_tick),
            (log[0].to.as_str(), 1, period)
        );
        // The log is a pure function of the seed: extending it twice is extending it once.
        let mut again = Vec::new();
        r.extend_moves(&c, period, &mut again, 2, &mut Default::default());
        r.extend_moves(&c, period, &mut again, 3, &mut Default::default());
        assert_eq!(again, log);
        assert_eq!(
            r.visit_at(&c, period, 3 * period + 5),
            r.locate(period, 3 * period + 5, &log)
        );
    }
    #[test]
    fn a_stay_is_not_a_move_and_keeps_the_run_it_is_in() {
        let c = routed(
            "crowded={crowded=1.0}\nexposed={exposed=1.0}\nhollow={hollow=1.0}",
            7,
        );
        let r = c.router.as_ref().unwrap();
        let period = r.period_ticks(&c).unwrap();
        let mut log = Vec::new();
        r.extend_moves(&c, period, &mut log, 40, &mut Default::default());
        assert!(log.iter().all(|m| !m.moved && m.to == "crowded"));
        let visit = r.locate(period, 40 * period, &log);
        assert_eq!((visit.index, visit.entered_tick, visit.from), (40, 0, None));
        // With the piece's rows the log mixes stays and moves, and a move opens a new run.
        let c = routed(PIECE_ROWS, 91827);
        let r = c.router.as_ref().unwrap();
        let mut log = Vec::new();
        r.extend_moves(&c, period, &mut log, 200, &mut Default::default());
        assert!(log.iter().any(|m| m.moved) && log.iter().any(|m| !m.moved));
        let first = log.iter().find(|m| m.moved).unwrap();
        let visit = r.locate(period, first.tick + period / 2, &log);
        assert_eq!(
            (
                visit.scene.as_str(),
                visit.entered_tick,
                visit.from.as_deref()
            ),
            (first.to.as_str(), first.tick, Some(first.from.as_str()))
        );
        let other = routed(PIECE_ROWS, 4471);
        let mut log2 = Vec::new();
        other.router.as_ref().unwrap().extend_moves(
            &other,
            period,
            &mut log2,
            200,
            &mut Default::default(),
        );
        assert_ne!(
            log.iter().map(|m| &m.to).collect::<Vec<_>>(),
            log2.iter().map(|m| &m.to).collect::<Vec<_>>()
        );
    }

    // ---- what the loader refuses
    fn refuses(text: &str, says: &str) {
        let e = Composition::parse(text).unwrap_err();
        assert!(e.contains(says), "wanted {says:?} in: {e}");
    }
    /// A two-scene piece; `scene_b` and `tail` are appended where a case needs more.
    fn text(start: &str, scene_b: &str, router: &str, tail: &str) -> String {
        format!(
            "tempo=126\nseed=1\nstart='{start}'\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.rhythm={{steps=16,pulses=6}}\n[scenes.a]\n[scenes.b]\n{scene_b}\n{router}\n[returns.g]\nalign=['voices.hat.trigger.cycle']\n{tail}"
        )
    }
    const ROUTER: &str = "[router]\nevery={returns='g'}\n[router.routes]\na={a=1}\nb={b=1}";
    #[test]
    fn the_loader_refuses_what_the_contract_refuses() {
        Composition::parse(&text("a", "", ROUTER, "")).unwrap();
        refuses(
            &text("a", "", ROUTER, "[arrangement]\nsections=[{phrase='a'}]"),
            "either an arrangement or a router",
        );
        refuses(
            &text("a", "", &ROUTER.replace("{returns='g'}", "{bars=16}"), ""),
            "router.every = { bars } is M2",
        );
        refuses(
            &text("a", "", &ROUTER.replace("'g'", "'nope'"), ""),
            "no [returns.nope]",
        );
        refuses(
            &text("a", "", &ROUTER.replace("\nb={b=1}", ""), ""),
            "no row for scene \"b\"",
        );
        refuses(
            &text("a", "", &format!("{ROUTER}\nc={{c=1}}"), ""),
            "router.routes.c: not a scene",
        );
        refuses(&text("z", "", ROUTER, ""), "start = \"z\" is not a scene");
        refuses(
            "tempo=126\nseed=1\nstart='a'\n[parts.hat]\nuse='techno.closed_hat'\n[scenes.a]",
            "there is no [router]",
        );
        refuses(
            "tempo=126\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\n[scenes.a]\n[phrases.a]",
            "not both",
        );
        refuses(
            &text(
                "a",
                "[scenes.rest]",
                &format!("{ROUTER}\nrest={{rest=1}}"),
                "",
            ),
            "\"rest\" is a weight word, not a scene",
        );
        // A scene may not change a member's period: the group's period is the piece's.
        refuses(
            &text(
                "a",
                "parts.hat.trigger.rhythm={steps=12,pulses=5}",
                ROUTER,
                "",
            ),
            "scene \"b\" changes the period of returns.g member voices.hat.trigger.cycle from 3840 to 2880",
        );
        // A scene may drop the voice: the clock is the transport's.
        let c = Composition::parse(&text(
            "a",
            "[[scenes.b.parts]]\nid='kick'\nuse='techno.kick'",
            ROUTER,
            "",
        ))
        .unwrap();
        let b = &c.router.as_ref().unwrap().scene("b").unwrap().composition;
        assert!(b.parts.iter().all(|p| p.id == "kick") && b.returns.is_empty());
        // A scene cannot carry its own return groups.
        refuses(
            &text(
                "a",
                "[scenes.b.returns.h]\nalign=['voices.hat.trigger.cycle']",
                ROUTER,
                "",
            ),
            "only accepts use, seed, phrase_bars, parts and accents",
        );
        // A period off the sixteenth grid is refused, with the number.
        refuses(
            "tempo=126\nseed=1\nstart='a'\n[parts.hat]\nuse='techno.closed_hat'\nsubdivision='1/16T'\ntrigger.rhythm={steps=1,pulses=1}\n[scenes.a]\n[router]\nevery={returns='g'}\n[router.routes]\na={a=1}\n[returns.g]\nalign=['voices.hat.trigger.cycle']",
            "period is 160 ticks, which is not on the sixteenth grid",
        );
    }
}
