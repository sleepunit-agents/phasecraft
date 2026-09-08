//! A fully prepared literal trigger schedule. Preparation belongs off the transport
//! path; lookup never parses, renders a cycle, allocates, or fills a cache.
//!
//! The literal Expression owns this prepared material. Admission, timing displacement
//! and boundary suppression remain downstream.
use crate::music::{
    PPQN,
    notation::{Draw, Pattern},
    time::NoteValue,
};

/// Consumer policy limits, independent of the standalone notation grammar.
const MAX_CYCLES: u64 = 4096;
const MAX_PREPARE_WORK: u64 = 1 << 20;
const MAX_PREPARE_EVENTS: u64 = 65536;

/// Metadata of one main structural onset, before probability or timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Attack {
    /// The event's own span, not the Part's subdivision cell.
    pub span_ticks: u64,
    /// Includes the main hit; a denied tail gate leaves the main hit intact.
    pub ratchet: u8,
    pub draw: Draw,
}

/// Authored configuration and prepared data travel together, so validation catches stale
/// material after a Rust caller changes an Expression or the Part's subdivision.
#[derive(Clone, Debug)]
pub struct Prepared {
    pattern: std::sync::Arc<str>,
    cycle_bars: u32,
    rotate: i32,
    cell: NoteValue,
    schedule: Schedule,
}
impl Prepared {
    pub fn new(
        pattern: &str,
        cycle_bars: u32,
        cell: NoteValue,
        rotate: i32,
    ) -> Result<Self, String> {
        if !(1..=1024).contains(&cycle_bars) {
            return Err("literal cycle_bars must be 1..1024".into());
        }
        Ok(Self {
            pattern: pattern.into(),
            cycle_bars,
            rotate,
            cell,
            schedule: Schedule::prepare(pattern, cycle_bars, cell, rotate)?,
        })
    }
    pub fn matches(&self, pattern: &str, cycle_bars: u32, rotate: i32) -> bool {
        self.pattern.as_ref() == pattern && self.cycle_bars == cycle_bars && self.rotate == rotate
    }
    pub fn cell(&self) -> NoteValue {
        self.cell
    }
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }
    /// Phase lookup alone is not absolute-event admission: the complete structural span
    /// must fit on the tick grid before any caller can add its offsets.
    pub fn at_step(&self, step: u64) -> Option<Attack> {
        let tick = step.checked_mul(self.cell.0)?;
        let attack = self.schedule.at(tick)?;
        tick.checked_add(attack.span_ticks)?;
        Some(attack)
    }
}
impl Attack {
    /// Structural child offset, using the notation renderer's floor rule. Child 0
    /// is the main hit. This describes admission candidates, not emitted notes.
    pub fn child_offset(&self, child: u8) -> Option<u64> {
        (child < self.ratchet).then(|| {
            let (n, i) = (u64::from(self.ratchet), u64::from(child));
            self.span_ticks / n * i + self.span_ticks % n * i / n
        })
    }
}

#[derive(Clone, Debug)]
struct Entry {
    tick: u64,
    attack: Attack,
}

/// Complete finite structural period, stored sparsely and shared across clones.
/// A positive rotation delays material by that many Part cells, matching Euclidean
/// rotation; it never changes the period or phase origin. Rotation wraps over the
/// complete period, including alternations and polymeters, not each individual bar.
#[derive(Clone, Debug)]
pub struct Schedule {
    period_ticks: u64,
    rotation_ticks: u64,
    entries: std::sync::Arc<[Entry]>,
}
impl Schedule {
    pub fn prepare(
        text: &str,
        cycle_bars: u32,
        subdivision: NoteValue,
        rotate: i32,
    ) -> Result<Self, String> {
        Self::build(text, cycle_bars, subdivision, rotate)
            .map_err(|why| format!("literal pattern {text:?}: {why}"))
    }

    fn build(
        text: &str,
        cycle_bars: u32,
        subdivision: NoteValue,
        rotate: i32,
    ) -> Result<Self, String> {
        if cycle_bars == 0 {
            return Err("cycle_bars must be positive".into());
        }
        if !subdivision.valid() {
            return Err("subdivision must be a supported note value".into());
        }
        let pattern = Pattern::parse(text)?;
        let kinds = pattern.kinds();
        if kinds.numbers || kinds.notes || kinds.names {
            return Err("a trigger schedule accepts only hits (x) and rests (~)".into());
        }
        let cycles = pattern
            .period_cycles()
            .filter(|&n| n <= MAX_CYCLES)
            .ok_or_else(|| format!("prepared period exceeds {MAX_CYCLES} cycles"))?;
        // Check before rendering anything, including expensive silent branches.
        // The bounds are conservative: they do not claim each cycle costs its maximum.
        let (work, events) = pattern.render_bounds();
        if work
            .checked_mul(cycles)
            .is_none_or(|n| n > MAX_PREPARE_WORK)
        {
            return Err(format!(
                "preparation work bound exceeds {MAX_PREPARE_WORK} renderer visits; simplify the pattern or shorten its period"
            ));
        }
        if events
            .checked_mul(cycles)
            .is_none_or(|n| n > MAX_PREPARE_EVENTS)
        {
            return Err(format!(
                "prepared event bound exceeds {MAX_PREPARE_EVENTS}; simplify the pattern or shorten its period"
            ));
        }
        let cycle_ticks = u64::from(cycle_bars) * 4 * PPQN;
        let period_ticks = cycle_ticks
            .checked_mul(cycles)
            .ok_or("prepared period exceeds the u64 tick grid")?;
        let cell = subdivision.0;
        let mut entries: Vec<Entry> = Vec::new();
        for k in 0..cycles {
            let base = k * cycle_ticks;
            for e in pattern.cycle(k) {
                // Check the rational onset, BEFORE the renderer floors to ticks. A
                // fractional onset that happens to floor to a cell is still off-grid.
                let numerator = u128::from(base) * u128::from(e.onset.den)
                    + u128::from(cycle_ticks) * u128::from(e.onset.num);
                let grid = u128::from(e.onset.den) * u128::from(cell);
                let tick = base + e.onset.ticks(cycle_ticks);
                if numerator % grid != 0 {
                    return Err(format!(
                        "onset {numerator}/{} ticks (cycle {k}, floored tick {tick}) is off subdivision {}; set subdivision finer, or write *n",
                        e.onset.den,
                        subdivision.label(),
                    ));
                }
                if entries.last().is_some_and(|previous| previous.tick == tick) {
                    return Err(format!(
                        "duplicate trigger onset at tick {tick} (cycle {k}); a trigger cannot stack simultaneous hits"
                    ));
                }
                let span_ticks = (e.onset + e.span).ticks(cycle_ticks) - e.onset.ticks(cycle_ticks);
                if span_ticks / u64::from(e.ratchet) < 2 {
                    return Err(format!(
                        "onset at tick {tick} has span {span_ticks} with ratchet {}; at least two ticks per attack are required",
                        e.ratchet,
                    ));
                }
                entries.push(Entry {
                    tick,
                    attack: Attack {
                        span_ticks,
                        ratchet: e.ratchet,
                        draw: e.draw,
                    },
                });
            }
        }
        // Checking one complete material period proves its repeats only if the
        // period also returns to the Part grid (notably for dotted subdivisions).
        if let Some(first) = entries.first()
            && !period_ticks.is_multiple_of(cell)
        {
            return Err(format!(
                "onset at tick {} in the next period is off subdivision {}; set subdivision finer, or change cycle_bars",
                u128::from(period_ticks) + u128::from(first.tick),
                subdivision.label(),
            ));
        }
        let rotation_ticks =
            (i128::from(rotate) * i128::from(cell)).rem_euclid(i128::from(period_ticks)) as u64;
        Ok(Self {
            period_ticks,
            rotation_ticks,
            entries: entries.into(),
        })
    }

    pub fn period_ticks(&self) -> u64 {
        self.period_ticks
    }
    pub fn has_ratchet(&self) -> bool {
        self.entries.iter().any(|e| e.attack.ratchet > 1)
    }

    /// Next possible main onset, independent of probability, for gate/tail reservation.
    pub fn next_after(&self, tick: u64) -> Option<u64> {
        if self.entries.is_empty() {
            return None;
        }
        let phase = tick % self.period_ticks;
        let source = (u128::from(phase) + u128::from(self.period_ticks)
            - u128::from(self.rotation_ticks))
            % u128::from(self.period_ticks);
        let source = source as u64;
        let next = self.entries.partition_point(|e| e.tick <= source);
        let distance = match self.entries.get(next) {
            Some(e) => e.tick - source,
            None => self.period_ticks - source + self.entries[0].tick,
        };
        tick.checked_add(distance)
    }

    /// Exact main onset lookup at an absolute transport tick. Tails are metadata
    /// of their source attack and do not become new main onsets in this schedule.
    /// Modular arithmetic stays representable even at the end of the u64 grid.
    pub fn at(&self, tick: u64) -> Option<Attack> {
        let phase = tick % self.period_ticks;
        let source = if phase >= self.rotation_ticks {
            phase - self.rotation_ticks
        } else {
            self.period_ticks - (self.rotation_ticks - phase)
        };
        self.entries
            .binary_search_by_key(&source, |e| e.tick)
            .ok()
            .map(|i| self.entries[i].attack)
    }
}
