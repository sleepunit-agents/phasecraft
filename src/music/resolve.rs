use super::rhythm::*;
use super::{Composition, Part, ProbabilityMode, STEP_TICKS};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Versioned, length-framed address: reproducible across platforms and builds.
/// Top 53 bits map to [0,1), with no mutable random state.
pub fn decision_roll(seed: u64, part: &str, lane: &str, event: u64, decision: &str) -> f64 {
    let mut hash = Sha256::new();
    hash.update(b"phasecraft/decision/v1\0");
    hash.update(seed.to_le_bytes());
    for value in [part, lane, decision] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(event.to_le_bytes());
    let bytes = hash.finalize();
    let bits = u64::from_le_bytes(bytes[..8].try_into().unwrap()) >> 11;
    bits as f64 / (1u64 << 53) as f64
}

/// The exact address one draw hashes: what a pin has to name to force it.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address {
    pub part: String,
    pub lane: String,
    pub event: u64,
    pub decision: String,
}
/// Where a pin lands, in the author's words: which dice (`roll`), whose (`voice`, or a shared
/// `accent` lane), and when (`bar`, `slot`, both 1-based; the slot counts the owner's own grid).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinAt {
    pub roll: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    pub bar: u64,
    pub slot: u64,
}
/// A forced draw: the dice at `at` returns `u` instead of the hash. The outcome is still the
/// composition's to decide from `u`; a pin never names a result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub at: PinAt,
    pub u: f64,
    /// Resolved at load from `at` against the Part it names; never authored, never printed.
    #[serde(skip)]
    pub address: Address,
}
/// The dice a piece rolls with: its seed, and the pins consulted before the hash.
#[derive(Clone, Copy)]
pub struct Dice<'a> {
    pub seed: u64,
    pub pins: &'a [Pin],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw {
    pub u: f64,
    pub pinned: bool,
}
impl Dice<'_> {
    pub fn roll(&self, part: &str, lane: &str, event: u64, decision: &str) -> Draw {
        let pinned = self.pins.iter().find(|pin| {
            let a = &pin.address;
            a.event == event && a.part == part && a.lane == lane && a.decision == decision
        });
        match pinned {
            Some(pin) => Draw {
                u: pin.u,
                pinned: true,
            },
            None => Draw {
                u: decision_roll(self.seed, part, lane, event, decision),
                pinned: false,
            },
        }
    }
}
/// One bar of 4/4 in ticks; the slot grid a pin's `bar` is counted against.
const BAR_TICKS: u64 = 16 * STEP_TICKS;
/// The most pins one composition may carry, enforced on the authored list at load.
pub const MAX_PINS: usize = 256;
const VOICE_DICE: [&str; 7] = [
    "fire", "accent", "burst", "flam", "ghost", "timing", "velocity",
];
/// Turn every authored pin into the address its draw hashes. Errors name the pin by its shape.
pub fn resolve_pins(c: &Composition, pins: &[Pin]) -> Result<Vec<Pin>, String> {
    // The authored length, before this allocates for it and before the quadratic duplicate check
    // walks it. `Composition::validate` runs with `pins` still empty, so its own copy of this
    // bound never sees the file's pins; checking here is what makes the limit real.
    if pins.len() > MAX_PINS {
        return Err(format!("at most {MAX_PINS} pins are supported"));
    }
    let mut resolved: Vec<Pin> = Vec::with_capacity(pins.len());
    for pin in pins {
        let at = &pin.at;
        let shape = || {
            let owner = match (&at.voice, &at.accent) {
                (Some(v), _) => format!("voice {v:?}"),
                (None, Some(a)) => format!("accent {a:?}"),
                (None, None) => "no owner".into(),
            };
            format!(
                "pin {{ roll = {:?}, {owner}, bar = {}, slot = {} }}",
                at.roll, at.bar, at.slot
            )
        };
        if !pin.u.is_finite() || !(0.0..1.0).contains(&pin.u) {
            return Err(format!(
                "{}: u must be finite and within 0 <= u < 1",
                shape()
            ));
        }
        if at.bar == 0 || at.bar > 10_000_000 {
            return Err(format!("{}: bar is 1-based and at most 10000000", shape()));
        }
        let address = match (&at.voice, &at.accent) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(format!(
                    "{}: name exactly one owner, voice = \"<part>\" or accent = \"<shared lane>\"",
                    shape()
                ));
            }
            (Some(voice), None) => {
                let part = c
                    .parts
                    .iter()
                    .find(|p| &p.id == voice)
                    .ok_or_else(|| format!("{}: no Part {voice:?}", shape()))?;
                let cell = part.subdivision.0;
                // The grid is continuous across bars: step n is at tick n*cell, and a cell that
                // does not divide a bar (`1/8.`, `1/4.`) neither starts on the bar line nor holds
                // the same number of onsets in every bar. So the slot is counted inside this bar's
                // own tick interval. Multiplying a truncated per-bar count instead walks the
                // address into an earlier bar and rejects a slot that exists (Mark, #9 review).
                let bar_start = (at.bar - 1) * BAR_TICKS;
                let first = bar_start.div_ceil(cell);
                // Both ends rounded up, exclusive end minus start: the count is the number of
                // grid onsets in [bar_start, bar_start + BAR_TICKS), and it is never negative.
                // Counting down from the last inclusive index instead underflows on a bar the
                // grid steps clean over — `1/1.` is 5760 ticks, so bar 3 spans [7680, 11520) and
                // holds no onset at all — and the panic lands before the error that would have
                // named the empty bar (Mark, #9 review).
                let slots = (bar_start + BAR_TICKS).div_ceil(cell) - first;
                if slots == 0 {
                    return Err(format!(
                        "{}: {voice:?} has no onset in bar {}; its subdivision is longer than a \
                         bar and steps over that one entirely",
                        shape(),
                        at.bar
                    ));
                }
                if at.slot == 0 || at.slot > slots {
                    return Err(format!(
                        "{}: slot is 1-based and {voice:?} has {slots} slots in bar {}",
                        shape(),
                        at.bar
                    ));
                }
                let step = first + (at.slot - 1);
                let g = &part.groove;
                let (lane, decision, mode) = match at.roll.as_str() {
                    "fire" => ("trigger", "admission", part.trigger.probability_mode),
                    "accent" => ("accent", "admission", part.accent.probability_mode),
                    "burst" => match &part.ornaments.ratchet {
                        Some(r) => ("ratchet", "admission", r.probability_mode),
                        None => {
                            return Err(format!(
                                "{}: {voice:?} has no ratchet to draw for",
                                shape()
                            ));
                        }
                    },
                    "flam" => match &part.ornaments.flam {
                        Some(f) => ("flam", "admission", f.probability_mode),
                        None => {
                            return Err(format!("{}: {voice:?} has no flam to draw for", shape()));
                        }
                    },
                    "ghost" if !g.is_default() => ("groove", "ghost", g.ghost_mode),
                    // `humanize_timing` needs no eligibility guard: `compiled::offset` draws it on
                    // every admitted event and again for the next-onset reservation, outside the
                    // touch closure and with no groove required. `touch_mode` is the mode that
                    // call site hashes under.
                    "timing" => ("groove", "humanize_timing", g.touch_mode()),
                    // `humanize_velocity` is drawn exactly when the touch closure runs, which any
                    // one of offbeat gain, gap response or humanize opens.
                    "velocity" if g.draws_touch() => {
                        ("groove", "humanize_velocity", g.touch_mode())
                    }
                    "ghost" => {
                        return Err(format!(
                            "{}: {voice:?} has no groove that draws \"ghost\"",
                            shape()
                        ));
                    }
                    "velocity" => {
                        return Err(format!(
                            "{}: {voice:?} has no groove that draws \"velocity\"; one of                              groove.offbeat_gain, groove.after_gap or groove.humanize is what                              draws it",
                            shape()
                        ));
                    }
                    other => {
                        return Err(format!(
                            "{}: unknown dice {other:?}; a voice rolls one of {}",
                            shape(),
                            VOICE_DICE.join(", ")
                        ));
                    }
                };
                Address {
                    part: voice.clone(),
                    lane: lane.into(),
                    event: decision_identity(c, cell, step, mode),
                    decision: decision.into(),
                }
            }
            (None, Some(accent)) => {
                if at.roll != "accent" {
                    return Err(format!(
                        "{}: a shared accent lane rolls only \"accent\"",
                        shape()
                    ));
                }
                let lane = c
                    .accents
                    .get(accent)
                    .ok_or_else(|| format!("{}: no shared accent {accent:?}", shape()))?;
                if at.slot == 0 || at.slot > 16 {
                    return Err(format!(
                        "{}: slot is 1-based and a shared accent has 16 slots per bar",
                        shape()
                    ));
                }
                let sixteenth = (at.bar - 1) * 16 + (at.slot - 1);
                Address {
                    part: accent.clone(),
                    lane: "shared_accent".into(),
                    event: match lane.probability_mode {
                        ProbabilityMode::PhraseLocked => sixteenth % c.phrase_steps(),
                        ProbabilityMode::Continuous => sixteenth,
                    },
                    decision: "shared_admission".into(),
                }
            }
        };
        if let Some(twin) = resolved.iter().find(|p| p.address == address) {
            return Err(format!(
                "{} names the same dice as pin {{ roll = {:?}, bar = {}, slot = {} }}; one pin per draw",
                shape(),
                twin.at.roll,
                twin.at.bar,
                twin.at.slot
            ));
        }
        resolved.push(Pin {
            at: at.clone(),
            u: pin.u,
            address,
        });
    }
    Ok(resolved)
}
fn is_false(v: &bool) -> bool {
    !*v
}
#[derive(Clone, Debug, Serialize)]
pub struct DecisionTrace {
    pub rhythm: RhythmTrace,
    pub event_identity: u64,
    pub probability: f64,
    pub roll: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
    pub admitted: bool,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Accent {
    pub active: bool,
    pub amount: f64,
}
fn is_default_cell(v: &u64) -> bool {
    *v == STEP_TICKS
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
#[derive(Clone, Debug, Serialize)]
pub struct MusicalEvent {
    #[serde(skip_serializing_if = "is_one")]
    pub velocity_gain: f64,
    pub tick: u64,
    pub duration_ticks: u64,
    pub accent: Accent,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<super::accent::ResolvedControl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groove: Option<super::groove::GrooveTrace>,
}
/// A realized window is generic; source cycles need not fit in the window.
#[derive(Clone, Debug, Serialize)]
pub struct RhythmPattern {
    pub cycles: Vec<super::cycle::CycleSpan>,
    pub start_tick: u64,
    pub end_tick: u64,
    pub events: Vec<MusicalEvent>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SharedAccentTrace {
    pub name: String,
    pub decision: DecisionTrace,
    pub amount: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct StepTrace {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sounding: Option<Vec<MusicalEvent>>,
    #[serde(skip_serializing_if = "is_default_cell")]
    pub cell_ticks: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_events: Vec<MusicalEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ornaments: Option<super::ornament::OrnamentTrace>,
    pub step: u64,
    pub tick: u64,
    pub position: String,
    pub part: String,
    pub trigger: DecisionTrace,
    pub accent: DecisionTrace,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shared_accents: Vec<SharedAccentTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<super::arrangement::SectionPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<super::router::Visit>,
    pub event: Option<MusicalEvent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<super::parameter::ParameterTrace>,
}
fn admission(
    c: &Composition,
    cell: u64,
    part_id: &str,
    step: u64,
    name: &str,
    lane: (&Expression, f64, ProbabilityMode),
    reference: &dyn Fn(&str, ReferenceMode) -> bool,
) -> DecisionTrace {
    let (expression, probability, mode) = lane;
    let event_identity = match mode {
        ProbabilityMode::PhraseLocked => {
            decision_identity(c, cell, step, ProbabilityMode::PhraseLocked)
        }
        ProbabilityMode::Continuous => step,
    };
    let rhythm = expression.evaluate_position(
        step,
        (step * cell % (c.phrase_steps() * STEP_TICKS)) / cell,
        reference,
    );
    let draw = c.dice().roll(part_id, name, event_identity, "admission");
    let admitted = rhythm.active() && draw.u < probability;
    DecisionTrace {
        rhythm,
        event_identity,
        probability,
        roll: draw.u,
        pinned: draw.pinned,
        admitted,
    }
}
fn resolve_part(
    c: &Composition,
    part: &Part,
    step: u64,
    reference: &dyn Fn(&str, ReferenceMode) -> bool,
) -> StepTrace {
    let trigger = admission(
        c,
        part.subdivision.0,
        &part.id,
        step,
        "trigger",
        (
            &part.trigger.rhythm,
            part.trigger.probability,
            part.trigger.probability_mode,
        ),
        reference,
    );
    let accent = admission(
        c,
        part.subdivision.0,
        &part.id,
        step,
        "accent",
        (
            &part.accent.rhythm,
            part.accent.probability,
            part.accent.probability_mode,
        ),
        reference,
    );
    let shared_accents: Vec<_> = part
        .accent
        .sources
        .iter()
        .map(|name| {
            let lane = &c.accents[name];
            let event_identity = match lane.probability_mode {
                ProbabilityMode::PhraseLocked => {
                    step * part.subdivision.0 / STEP_TICKS % c.phrase_steps()
                }
                ProbabilityMode::Continuous => step * part.subdivision.0 / STEP_TICKS,
            };
            let shared_step = step * part.subdivision.0 / STEP_TICKS;
            let rhythm = lane
                .rhythm
                .evaluate(shared_step, c.phrase_steps(), &|_, _| false);
            let draw = c
                .dice()
                .roll(name, "shared_accent", event_identity, "shared_admission");
            let admitted = (step * part.subdivision.0).is_multiple_of(STEP_TICKS)
                && rhythm.active()
                && draw.u < lane.probability;
            SharedAccentTrace {
                name: name.clone(),
                amount: if admitted { lane.amount } else { 0.0 },
                decision: DecisionTrace {
                    rhythm,
                    event_identity,
                    probability: lane.probability,
                    roll: draw.u,
                    pinned: draw.pinned,
                    admitted,
                },
            }
        })
        .collect();
    let combined_active = accent.admitted || shared_accents.iter().any(|s| s.decision.admitted);
    let combined_amount = shared_accents.iter().map(|s| s.amount).fold(
        if accent.admitted {
            part.accent.amount
        } else {
            0.0
        },
        f64::max,
    );
    let event = trigger.admitted.then(|| MusicalEvent {
        velocity_gain: 1.0,
        tick: step * part.subdivision.0,
        duration_ticks: part.output.gate_ticks,
        groove: None,
        controls: part
            .profile
            .controls
            .iter()
            .filter(|(name, response)| {
                !part.parameters.contains_key(*name) && response.envelope.is_none()
            })
            .map(|(name, response)| {
                let output = &part.output.controls[name];
                let amount = response.value(combined_amount);
                super::accent::ResolvedControl {
                    name: name.clone(),
                    amount,
                    channel: output.channel.unwrap_or(part.output.channel),
                    cc: output.cc,
                    value: super::accent::midi_value(amount),
                    reset: super::accent::midi_value(response.base),
                }
            })
            .collect(),
        accent: Accent {
            active: combined_active,
            amount: combined_amount,
        },
    });
    StepTrace {
        sounding: None,
        cell_ticks: part.subdivision.0,
        extra_events: Vec::new(),
        ornaments: None,
        step,
        tick: step * part.subdivision.0,
        position: format!("{}.{}.{}", step / 16 + 1, step / 4 % 4 + 1, step % 4 + 1),
        part: part.id.clone(),
        parameters: Vec::new(),
        shared_accents,
        section: None,
        scene: None,
        trigger,
        accent,
        event,
    }
}
pub fn realize(c: &Composition, part: &Part, start_step: u64, steps: u64) -> RhythmPattern {
    let mut compiled = Compiled::new(c);
    let start_tick = start_step * STEP_TICKS;
    let end_tick = (start_step + steps) * STEP_TICKS;
    let mut events: Vec<_> = (start_step.saturating_sub(25)..start_step + steps + 2)
        .flat_map(|s| compiled.resolve_step(s).0)
        .filter(|t| t.part == part.id)
        .flat_map(|t| t.event.into_iter().chain(t.extra_events))
        .filter(|e| e.tick >= start_tick && e.tick < end_tick)
        .collect();
    events.sort_by_key(|e| e.tick);
    RhythmPattern {
        cycles: super::cycle::spans(c, &part.id, start_step, start_step + steps),
        start_tick,
        end_tick,
        events,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    pub tick: u64,
    pub bytes: [u8; 3],
    /// CC onset registers a resting value for stop/error cleanup.
    pub reset_value: Option<u8>,
    /// Neutral target restored on Stop, independently of per-note emphasis resets.
    pub stop_value: Option<u8>,
    /// Timeline sample: deduplicate values and skip obsolete samples on dispatch.
    pub parameter: bool,
    /// Section transition: reset before the incoming section initializes controls.
    pub boundary_reset: bool,
}
fn midi_velocity(part: &Part, event: &MusicalEvent) -> u8 {
    ((f64::from(part.profile.base) * event.groove.as_ref().map_or(1.0, |g| g.velocity_factor)
        + if event.accent.active {
            event.accent.amount * f64::from(part.profile.boost)
        } else {
            0.0
        })
        * event.velocity_gain)
        .round()
        .clamp(1.0, 127.0) as u8
}
pub fn to_midi(part: &Part, event: &MusicalEvent) -> Vec<MidiEvent> {
    let output = &part.output;
    let velocity = midi_velocity(part, event);
    let mut events = vec![
        MidiEvent {
            tick: event.tick,
            bytes: [0x90 | (output.channel - 1), output.note, velocity],
            stop_value: None,
            reset_value: None,
            boundary_reset: false,
            parameter: false,
        },
        MidiEvent {
            tick: event.tick + event.duration_ticks,
            bytes: [0x80 | (output.channel - 1), output.note, 0],
            stop_value: None,
            reset_value: None,
            boundary_reset: false,
            parameter: false,
        },
    ];
    for control in &event.controls {
        events.push(MidiEvent {
            tick: event.tick,
            bytes: [0xb0 | (control.channel - 1), control.cc, control.value],
            stop_value: output.controls[&control.name]
                .default
                .map(super::accent::midi_value),
            reset_value: Some(control.reset),
            boundary_reset: false,
            parameter: false,
        });
        events.push(MidiEvent {
            tick: event.tick + event.duration_ticks,
            bytes: [0xb0 | (control.channel - 1), control.cc, control.reset],
            stop_value: None,
            reset_value: None,
            boundary_reset: false,
            parameter: false,
        });
    }
    events
}

fn decision_identity(c: &Composition, cell: u64, step: u64, mode: ProbabilityMode) -> u64 {
    match mode {
        ProbabilityMode::Continuous => step,
        ProbabilityMode::PhraseLocked if cell == STEP_TICKS => step % c.phrase_steps(),
        ProbabilityMode::PhraseLocked => (step * cell) % (c.phrase_steps() * STEP_TICKS),
    }
}

mod compiled;
pub use compiled::Compiled;

pub fn resolve_step(c: &Composition, step: u64) -> (Vec<StepTrace>, Vec<MidiEvent>) {
    Compiled::new(c).resolve_step(step)
}

/// Resolve one member Part with the same dependency graph used by live playback.
pub fn resolve(c: &Composition, part: &Part, step: u64) -> StepTrace {
    resolve_step(c, step)
        .0
        .into_iter()
        .find(|trace| trace.part == part.id)
        .expect("resolve requires a member of the validated composition")
}

pub fn midi_order(event: &MidiEvent) -> (u64, u8, [u8; 3]) {
    let priority = if event.bytes[0] & 0xf0 == 0x80 {
        0
    } else if event.boundary_reset {
        1
    } else if event.bytes[0] & 0xf0 == 0xb0 {
        if event.reset_value.is_none() { 2 } else { 3 }
    } else {
        4
    };
    (event.tick, priority, event.bytes)
}
