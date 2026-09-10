//! The pitch sliver (docs/pitch.md): note and pitch lanes as held value sources, sampled per attack.
use phasecraft::music::{
    Composition, STEP_TICKS,
    pitch::{BAR_TICKS, ValueLane, sounding_note},
    resolve::resolve_step,
};

/// A `sub` voice on beats 1 and 3, kit note 48 on channel 2, plus whatever `extra` writes.
fn piece(extra: &str) -> Composition {
    Composition::parse(&format!(
        r#"
tempo = 120
seed = 91827
phrase_bars = 4
[parts.sub]
trigger.rhythm = {{ steps = 16, pulses = 2 }}
trigger.probability = 1.0
accent.rhythm = {{ steps = 16, pulses = 1 }}
accent.probability = 0.0
output.channel = 2
output.note = 48
{extra}
"#
    ))
    .unwrap()
}
fn refuses(extra: &str) -> String {
    Composition::parse(&format!(
        r#"
tempo = 120
seed = 91827
[parts.sub]
trigger.rhythm = {{ steps = 16, pulses = 2 }}
accent.rhythm = {{ steps = 16, pulses = 1 }}
output.note = 48
{extra}
"#
    ))
    .expect_err(extra)
}
/// (tick, note) of every note-on in `bars` bars, and every note-off checked against its on.
fn notes(c: &Composition, bars: u64) -> Vec<(u64, u8)> {
    let mut ons = vec![];
    let mut offs = vec![];
    for step in 0..bars * 16 {
        for m in resolve_step(c, step).1 {
            match m.bytes[0] & 0xf0 {
                0x90 => ons.push((m.tick, m.bytes[1])),
                0x80 => offs.push((m.tick, m.bytes[1])),
                _ => {}
            }
        }
    }
    for (tick, note) in &ons {
        assert!(
            offs.iter().any(|(t, n)| t > tick && n == note),
            "note-on {note} at {tick} has no note-off carrying the same note"
        );
    }
    ons
}
fn per_bar(c: &Composition, bars: u64) -> Vec<Vec<u8>> {
    let ons = notes(c, bars);
    (0..bars)
        .map(|bar| {
            ons.iter()
                .filter(|(t, _)| t / BAR_TICKS == bar)
                .map(|(_, n)| *n)
                .collect()
        })
        .collect()
}

#[test]
fn a_note_pattern_replaces_the_kit_note_one_bar_per_alternation_element() {
    let c = piece("[parts.sub.note]\npattern = \"<c1 c1 eb1 bb0>\"");
    assert_eq!(
        per_bar(&c, 5),
        vec![
            vec![36, 36],
            vec![36, 36],
            vec![39, 39],
            vec![34, 34],
            vec![36, 36]
        ]
    );
}
#[test]
fn a_pitch_pattern_offsets_the_kit_note_in_semitones() {
    let c = piece("[parts.sub.pitch]\npattern = \"<0 0 -5 7>\"");
    assert_eq!(
        per_bar(&c, 4),
        vec![vec![48, 48], vec![48, 48], vec![43, 43], vec![55, 55]]
    );
}
#[test]
fn note_and_pitch_compose_by_addition() {
    let c = piece(
        "[parts.sub.note]\npattern = \"<c1 eb1>\"\n[parts.sub.pitch]\npattern = \"<0 0 -5 7>\"",
    );
    assert_eq!(
        per_bar(&c, 4),
        vec![vec![36, 36], vec![39, 39], vec![31, 31], vec![46, 46]]
    );
}
#[test]
fn a_rest_holds_the_last_value_across_the_cycle_line_and_the_kit_note_before_the_first() {
    // eb1 lands on beat 3; beat 1 of bar 1 is before the first value ever.
    let c = piece("[parts.sub.note]\npattern = \"~ eb1\"");
    assert_eq!(
        per_bar(&c, 3),
        vec![vec![48, 39], vec![39, 39], vec![39, 39]]
    );
    let lane = ValueLane::parse("<~ ~ ~ c1>", 1).unwrap();
    assert_eq!(lane.sample(0), None);
    assert_eq!(
        lane.sample(3 * BAR_TICKS).map(|t| format!("{t:?}")),
        Some("Note { name: \"c1\", midi: 36 }".into())
    );
    // Bar 4 (cycle 4) is silent; the value is held from cycle 3, one full period back.
    assert!(lane.sample(4 * BAR_TICKS).is_some());
    assert!(lane.sample(1000 * BAR_TICKS).is_some());
}
#[test]
fn cycle_bars_stretches_one_string_over_several_bars() {
    let c = piece("[parts.sub.note]\npattern = \"c1 eb1\"\ncycle_bars = 2");
    assert_eq!(
        per_bar(&c, 4),
        vec![vec![36, 36], vec![39, 39], vec![36, 36], vec![39, 39]]
    );
}
#[test]
fn a_constant_and_an_integer_are_notes_too() {
    assert_eq!(
        per_bar(&piece("[parts.sub.note]\npattern = \"c1\""), 2),
        vec![vec![36, 36]; 2]
    );
    assert_eq!(
        per_bar(&piece("[parts.sub.note]\npattern = \"60\""), 2),
        vec![vec![60, 60]; 2]
    );
    assert_eq!(
        per_bar(&piece("[parts.sub.note]\npattern = \"f#-1\""), 1),
        vec![vec![18, 18]]
    );
}
#[test]
fn every_attack_samples_at_its_own_tick_including_a_flam_grace() {
    // Beat 3's grace lands a thirty-second early, inside the first half of the bar: c1; its main
    // hit is on the second half: eb1. The value boundary falls between the two attacks.
    let c = piece(
        "[parts.sub.note]\npattern = \"c1 eb1\"\n[parts.sub.ornaments.flam]\nspacing = \"1/32\"\nprobability = 1.0",
    );
    // The next bar's downbeat is itself flammed: its grace lands a thirty-second before the
    // barline, inside this bar's last step, and so samples eb1 there — while its main hit
    // opens the next bar on c1. Before t-492 that grace was dropped, because the attack it
    // belongs to sits at the bar start.
    let ons = notes(&c, 1);
    assert_eq!(
        ons,
        vec![
            (0, 36),
            (8 * STEP_TICKS - STEP_TICKS / 2, 36),
            (8 * STEP_TICKS, 39),
            (16 * STEP_TICKS - STEP_TICKS / 2, 39)
        ]
    );
}
#[test]
fn a_part_without_lanes_is_unchanged_and_carries_no_note_field() {
    let c = Composition::parse(include_str!("../examples/quickstart/hat.toml")).unwrap();
    assert_eq!(sounding_note(&c.parts[0], 0), None);
    for step in 0..64 {
        let (traces, midi) = resolve_step(&c, step);
        let json = serde_json::to_string(&traces).unwrap();
        assert!(!json.contains("\"note\""), "{json}");
        for m in midi.iter().filter(|m| m.bytes[0] & 0xe0 == 0x80) {
            assert_eq!(m.bytes[1], 42);
        }
    }
    let pitched = piece("[parts.sub.note]\npattern = \"c1\"");
    let json = serde_json::to_string(&resolve_step(&pitched, 0).0).unwrap();
    assert!(json.contains("\"note\":36"), "{json}");
}
#[test]
fn expand_round_trips_both_lanes() {
    let c = piece(
        "[parts.sub.note]\npattern = \"<c1 eb1>\"\ncycle_bars = 2\n[parts.sub.pitch]\npattern = \"<0 7>\"",
    );
    let text = toml::to_string(&c).unwrap();
    assert!(text.contains("pattern = \"<c1 eb1>\""), "{text}");
    assert!(text.contains("cycle_bars = 2"), "{text}");
    let copy = Composition::parse(&text).unwrap();
    assert_eq!(notes(&c, 8), notes(&copy, 8));
}
#[test]
fn a_scene_overrides_the_string_and_keeps_the_cycle() {
    let c = piece(
        "[parts.sub.note]\npattern = \"<c1 eb1>\"\ncycle_bars = 2\n[phrases.A]\n[phrases.B]\nuse = \"A\"\nparts.sub.note.pattern = \"bb0\"\n[arrangement]\nsections = [{ phrase = \"A\", bars = 4 }, { phrase = \"B\", bars = 2 }]",
    );
    let b = &c.arrangement.as_ref().unwrap().sections[1]
        .composition
        .parts[0];
    let lane = b.note.as_ref().unwrap();
    assert_eq!((lane.pattern.as_str(), lane.cycle_bars), ("bb0", 2));
    assert_eq!(
        per_bar(&c, 6),
        vec![
            vec![36, 36],
            vec![36, 36],
            vec![39, 39],
            vec![39, 39],
            vec![34, 34],
            vec![34, 34]
        ]
    );
}
#[test]
fn the_lane_refuses_what_it_does_not_walk_naming_the_string_and_the_token() {
    for (extra, expect) in [
        (
            "[parts.sub.note]\npattern = \"x ~ x\"",
            "`x` is a hit, not a note",
        ),
        (
            "[parts.sub.note]\npattern = \"rifle\"",
            "\"rifle\" is a name, not a note",
        ),
        (
            "[parts.sub.note]\npattern = \"0.5\"",
            "0.5 is not a MIDI note",
        ),
        (
            "[parts.sub.note]\npattern = \"128\"",
            "128 is not a MIDI note",
        ),
        (
            "[parts.sub.note]\npattern = \"[c1,eb1]\"",
            "a stack is a chord",
        ),
        (
            "[parts.sub.note]\npattern = \"c1, eb1\"",
            "a stack is a chord",
        ),
        (
            "[parts.sub.note]\npattern = \"c1*2\"",
            "`*n` and `?` belong to the trigger",
        ),
        (
            "[parts.sub.note]\npattern = \"c1?\"",
            "`*n` and `?` belong to the trigger",
        ),
        ("[parts.sub.note]\npattern = \"~\"", "holds no value"),
        (
            "[parts.sub.note]\npattern = \"c1\"\nper = \"event\"",
            "unknown field `per`",
        ),
        (
            "[parts.sub.note]\npattern = \"c1\"\ncycle_bars = 0",
            "cycle_bars must be 1..1024",
        ),
        ("[parts.sub.note]\npattern = \"c1 [\"", "pattern \"c1 [\""),
        (
            "[parts.sub.pitch]\npattern = \"c1\"",
            "c1 is a note name; pitch.pattern holds semitone offsets",
        ),
        (
            "[parts.sub.pitch]\npattern = \"0.5\"",
            "0.5 is not a semitone offset",
        ),
        (
            "[parts.sub.pitch]\npattern = \"<0 -60>\"",
            "offset -60 from note 48 leaves MIDI 0..=127",
        ),
        (
            "[parts.sub.pitch]\npattern = \"80\"",
            "offset 80 from note 48 leaves MIDI 0..=127",
        ),
        // The kit note is a value the note lane holds before its first one, so it is checked too.
        (
            "[parts.sub.note]\npattern = \"c1\"\n[parts.sub.pitch]\npattern = \"80\"",
            "offset 80 from note 48",
        ),
        (
            "[parts.sub.note]\npattern = \"<c1 c5>\"\n[parts.sub.pitch]\npattern = \"<0 50>\"",
            "offset 50 from note 84",
        ),
    ] {
        let error = refuses(extra);
        assert!(
            error.contains(expect),
            "{extra}\n  expected {expect:?}\n  got {error}"
        );
    }
    // Placement constructs are fine: they only say where a value sits.
    for extra in [
        "[parts.sub.note]\npattern = \"c1(3,8)\"",
        "[parts.sub.note]\npattern = \"{c1 eb1 bb0}%4\"",
        "[parts.sub.note]\npattern = \"c1@3 [~ eb1]\"",
        "[parts.sub.pitch]\npattern = \"<0 0 -5 7>\"\ncycle_bars = 4",
    ] {
        piece(extra);
    }
}

#[test]
fn value_preparation_refuses_periods_it_cannot_hold_exactly() {
    // Only cycle 0 of 65*67 sounds. The former 4096-cycle search forgot c1 at cycle 4097.
    let inner = format!("<c1 {}>", vec!["~"; 66].join(" "));
    let pattern = format!("<{inner} {}>", vec!["~"; 64].join(" "));
    let error = ValueLane::parse(&pattern, 1).unwrap_err();
    assert!(
        error.contains("repeats every 4355 cycles") && error.contains("limited to 4096"),
        "{error}"
    );
    let unrepresentable = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53]
        .map(|n| format!("<c1{}>", " ~".repeat(n - 1)))
        .join(" ");
    let error = ValueLane::parse(&unrepresentable, 1).unwrap_err();
    assert!(error.contains("cycle period exceeds u64"), "{error}");
    // A supported sparse period holds indefinitely, including the very last silent cycle.
    let pattern = format!("<c1 {}>", vec!["~"; 4095].join(" "));
    let lane = ValueLane::parse(&pattern, 1).unwrap();
    assert_eq!(lane.sample(0), lane.sample(4095 * BAR_TICKS));
    assert_eq!(lane.sample(0), lane.sample(8191 * BAR_TICKS));
}

#[test]
fn prepared_values_match_a_forward_held_source() {
    use phasecraft::music::notation::Pattern;
    for text in [
        "~ eb1",
        "<~ ~ c1> ~",
        "{c1 ~ eb1 ~ ~}%3",
        "<c1 <~ eb1 ~>>",
        "c1@3 [~ eb1]",
        "<~ <c1 ~>>",
    ] {
        let pattern = Pattern::parse(text).unwrap();
        let lane = ValueLane::parse(text, 1).unwrap();
        let mut held = None;
        for cycle in 0..18 {
            let events = pattern.cycle(cycle);
            let mut next = 0;
            for position in 0..BAR_TICKS {
                while next < events.len() && events[next].onset.ticks(BAR_TICKS) <= position {
                    held = Some(events[next].token.clone());
                    next += 1;
                }
                assert_eq!(
                    lane.sample(cycle * BAR_TICKS + position),
                    held,
                    "{text} cycle {cycle} tick {position}"
                );
            }
        }
    }
}

#[test]
fn value_preparation_bounds_total_work_and_events_before_rendering() {
    let error = ValueLane::parse("<c1 [~(1024,1024)](256,256)>", 1).unwrap_err();
    assert!(error.contains("preparation work bound"), "{error}");
    let values = vec!["c1"; 17].join(" ");
    let error = ValueLane::parse(&format!("[<{values}>(64,64)](64,64)"), 1).unwrap_err();
    assert!(error.contains("event bound"), "{error}");
}

fn timing_piece(pattern: &str, bar: u64, slot: u64, roll: f64) -> Composition {
    piece(&format!(
        "[parts.sub.note]\npattern = {pattern:?}\n[parts.sub.groove.humanize]\ntiming_ticks = 24\nmode = 'continuous'\n[[pins]]\nat = {{roll='timing',voice='sub',bar={bar},slot={slot}}}\nu = {roll}"
    ))
}

#[test]
fn a_timing_pin_crosses_a_value_onset_and_trace_matches_the_wire() {
    for (roll, tick, note) in [(0.0, 1896, 36), (0.5, 1920, 39), (0.99, 1944, 39)] {
        let c = timing_piece("c1 eb1", 1, 9, roll);
        let ons = notes(&c, 1);
        assert!(ons.contains(&(tick, note)), "{ons:?}");
        let traces = resolve_step(&c, 8).0;
        let event = traces[0].event.as_ref().unwrap();
        assert_eq!((event.tick, event.note), (tick, Some(note)));
        assert_eq!(
            event
                .groove
                .as_ref()
                .unwrap()
                .touch
                .as_ref()
                .unwrap()
                .timing_roll,
            roll
        );
    }
}

#[test]
fn a_timing_pin_at_a_value_cycle_line_obeys_bar_ownership() {
    // A cycle line is also a bar line. An early requested onset is clipped to that line,
    // so it must sample eb1 in cycle 1, never reach back to c1 in cycle 0.
    for (roll, tick) in [(0.0, BAR_TICKS), (0.5, BAR_TICKS), (0.99, BAR_TICKS + 24)] {
        let c = timing_piece("<c1 eb1>", 2, 1, roll);
        let ons = notes(&c, 2);
        assert!(ons.contains(&(tick, 39)), "{ons:?}");
        let traces = resolve_step(&c, 16).0;
        let event = traces[0].event.as_ref().unwrap();
        assert_eq!((event.tick, event.note), (tick, Some(39)));
        assert_eq!(
            event
                .groove
                .as_ref()
                .unwrap()
                .touch
                .as_ref()
                .unwrap()
                .timing_roll,
            roll
        );
    }
}
