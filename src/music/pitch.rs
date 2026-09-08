//! The pitch sliver (TO-PHASECRAFT D9, step 1): a Part's sounding note as a held value source.
//!
//! Two optional lanes on a Part read the step-notation parser (`notation.rs`) for values
//! instead of hits:
//!
//! - `[parts.sub.note] pattern = "<c1 ~ eb1 bb0>"` — note names, or integers 0..=127, that
//!   replace the kit note (`output.note`);
//! - `[parts.metal.pitch] pattern = "<0 0 -5 7>"` — integer semitone offsets from that note.
//!
//! Both are *held value sources* in the sense of `docs/generative-systems.md` § "Clocks and
//! sampling": sampling the source at a tick reads the value of the last event at or before
//! that tick, and before the first value ever it reads its initialisation (the kit note;
//! offset 0). The consumer is every sounding attack of the Part — main hit, ratchet tail,
//! flam grace — each sampled at its own tick, so the sounding note is
//! `note.sample(tick) + pitch.sample(tick)`. `docs/pitch.md` is the contract, including
//! what is deliberately not here (chords, `per = "event"`, key and degree resolution).
use super::notation::{Pattern, Token};
use super::{Part, STEP_TICKS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const BAR_TICKS: u64 = STEP_TICKS * 16;
/// Consumer preparation limits, checked before rendering any cycles.
pub const MAX_HELD_CYCLES: u64 = 4096;
const MAX_PREPARE_WORK: u64 = 1 << 20;
const MAX_PREPARE_EVENTS: u64 = 65536;

#[derive(Clone, Debug)]
struct ValueEvent {
    tick: u64,
    token: Token,
}

/// A value pattern on a Part: the authored string, parsed once, over `cycle_bars` bars.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "ValueLaneFile", into = "ValueLaneFile")]
pub struct ValueLane {
    pub pattern: String,
    pub cycle_bars: u32,
    parsed: Arc<Pattern>,
    period_ticks: u64,
    events: Arc<[ValueEvent]>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ValueLaneFile {
    pattern: String,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    cycle_bars: u32,
}
fn one() -> u32 {
    1
}
fn is_one(v: &u32) -> bool {
    *v == 1
}
impl TryFrom<ValueLaneFile> for ValueLane {
    type Error = String;
    fn try_from(file: ValueLaneFile) -> Result<Self, String> {
        if !(1..=1024).contains(&file.cycle_bars) {
            return Err("cycle_bars must be 1..1024".into());
        }
        let parsed = Pattern::parse(&file.pattern)
            .map_err(|e| format!("pattern {:?}: {e}", file.pattern))?;
        let refuse = |why: String| format!("pattern {:?}: {why}", file.pattern);
        let cycles = match parsed.period_cycles() {
            None => {
                return Err(refuse(
                    "the pattern's cycle period exceeds u64; its value period cannot be prepared"
                        .into(),
                ));
            }
            Some(n) if n > MAX_HELD_CYCLES => {
                return Err(refuse(format!(
                    "the pattern repeats every {n} cycles; prepared value periods are limited to {MAX_HELD_CYCLES}"
                )));
            }
            Some(n) => n,
        };
        let (work, events) = parsed.render_bounds();
        if work
            .checked_mul(cycles)
            .is_none_or(|n| n > MAX_PREPARE_WORK)
        {
            return Err(refuse(format!(
                "value preparation work bound exceeds {MAX_PREPARE_WORK} renderer visits"
            )));
        }
        if events
            .checked_mul(cycles)
            .is_none_or(|n| n > MAX_PREPARE_EVENTS)
        {
            return Err(refuse(format!(
                "prepared value event bound exceeds {MAX_PREPARE_EVENTS}"
            )));
        }
        let cycle_ticks = u64::from(file.cycle_bars) * BAR_TICKS;
        let period_ticks = cycle_ticks
            .checked_mul(cycles)
            .ok_or_else(|| refuse("prepared value period exceeds the u64 tick grid".into()))?;
        let mut events = Vec::new();
        for k in 0..cycles {
            for event in parsed.cycle(k) {
                events.push(ValueEvent {
                    tick: k * cycle_ticks + event.onset.ticks(cycle_ticks),
                    token: event.token,
                });
            }
        }
        Ok(Self {
            pattern: file.pattern,
            cycle_bars: file.cycle_bars,
            parsed: Arc::new(parsed),
            period_ticks,
            events: events.into(),
        })
    }
}
impl From<ValueLane> for ValueLaneFile {
    fn from(lane: ValueLane) -> Self {
        Self {
            pattern: lane.pattern,
            cycle_bars: lane.cycle_bars,
        }
    }
}
impl ValueLane {
    pub fn parse(pattern: &str, cycle_bars: u32) -> Result<Self, String> {
        ValueLaneFile {
            pattern: pattern.into(),
            cycle_bars,
        }
        .try_into()
    }
    pub fn cycle_ticks(&self) -> u64 {
        u64::from(self.cycle_bars) * BAR_TICKS
    }
    /// Read the last value at or before `tick` from the complete prepared period. Before the
    /// first value ever, return `None`; later periods hold the previous period's final value.
    /// No cycle is rendered here, and silence never expires an already established value.
    pub fn sample(&self, tick: u64) -> Option<Token> {
        let position = tick % self.period_ticks;
        let end = self.events.partition_point(|e| e.tick <= position);
        end.checked_sub(1)
            .and_then(|i| self.events.get(i))
            .or_else(|| {
                (tick >= self.period_ticks)
                    .then(|| self.events.last())
                    .flatten()
            })
            .map(|e| e.token.clone())
    }
}

/// A note token as a MIDI note number: a note name, or an integer 0..=127.
fn note_number(token: &Token) -> Result<u8, String> {
    match token {
        Token::Note { midi, .. } => Ok(*midi),
        Token::Number(n) if n.fract() == 0.0 && (0.0..=127.0).contains(n) => Ok(*n as u8),
        Token::Number(n) => Err(format!("{n} is not a MIDI note (an integer 0..=127)")),
        Token::Hit => Err("`x` is a hit, not a note; a note pattern says what, not when".into()),
        Token::Name(name) => Err(format!("{name:?} is a name, not a note")),
    }
}
/// A pitch token as a semitone offset: an integer within -127..=127.
fn semitones(token: &Token) -> Result<i32, String> {
    match token {
        Token::Number(n) if n.fract() == 0.0 && (-127.0..=127.0).contains(n) => Ok(*n as i32),
        Token::Number(n) => Err(format!("{n} is not a semitone offset (an integer)")),
        Token::Note { name, .. } => Err(format!(
            "{name} is a note name; pitch.pattern holds semitone offsets from the note"
        )),
        Token::Hit => {
            Err("`x` is a hit, not an offset; a pitch pattern says what, not when".into())
        }
        Token::Name(name) => Err(format!("{name:?} is a name, not an offset")),
    }
}
/// The sliver is closed: one value per attack, and `*n` / `?` stay with the trigger.
fn closed(lane: &ValueLane, name: &str) -> Result<Vec<Token>, String> {
    let shape = lane.parsed.shape();
    let refuse = |why: &str| Err(format!("{name}.pattern {:?}: {why}", lane.pattern));
    if shape.tokens.is_empty() {
        return refuse("holds no value; a source needs at least one");
    }
    if shape.stacked {
        return refuse(
            "a stack is a chord, and a Part sounds one note per attack (chords are D9 step 2)",
        );
    }
    if shape.ratchet || shape.draw {
        return refuse(
            "`*n` and `?` belong to the trigger pattern; a value pattern says what, not when",
        );
    }
    Ok(shape.tokens)
}
/// Every note this Part can sound is a MIDI note: checked here, at load, over every token the
/// lanes can produce — including the kit note, which a note lane holds before its first value.
pub fn validate(part: &Part) -> Result<(), String> {
    let mut notes = vec![part.output.note];
    if let Some(lane) = &part.note {
        for token in closed(lane, "note")? {
            notes.push(
                note_number(&token).map_err(|e| format!("note.pattern {:?}: {e}", lane.pattern))?,
            );
        }
    }
    let mut offsets = vec![0];
    if let Some(lane) = &part.pitch {
        for token in closed(lane, "pitch")? {
            offsets.push(
                semitones(&token).map_err(|e| format!("pitch.pattern {:?}: {e}", lane.pattern))?,
            );
        }
        for &offset in &offsets {
            for &note in &notes {
                if !(0..=127).contains(&(i32::from(note) + offset)) {
                    return Err(format!(
                        "pitch.pattern {:?}: offset {offset} from note {note} leaves MIDI 0..=127",
                        lane.pattern
                    ));
                }
            }
        }
    }
    Ok(())
}
/// The note a Part sounds at `tick`: `Some` only when it has a note or pitch lane, so a Part
/// without either changes nothing — its events carry no note and the kit note sounds.
///
/// Tokens were proven notes and offsets by `validate`; a Part assembled in Rust without it
/// reads the kit note rather than panicking the player.
pub fn sounding_note(part: &Part, tick: u64) -> Option<u8> {
    if part.note.is_none() && part.pitch.is_none() {
        return None;
    }
    let base = part
        .note
        .as_ref()
        .and_then(|lane| lane.sample(tick))
        .and_then(|token| note_number(&token).ok())
        .unwrap_or(part.output.note);
    let offset = part
        .pitch
        .as_ref()
        .and_then(|lane| lane.sample(tick))
        .and_then(|token| semitones(&token).ok())
        .unwrap_or(0);
    Some((i32::from(base) + offset).clamp(0, 127) as u8)
}
