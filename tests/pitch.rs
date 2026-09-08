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
    let ons = notes(&c, 1);
    assert_eq!(
        ons,
        vec![
            (0, 36),
            (8 * STEP_TICKS - STEP_TICKS / 2, 36),
            (8 * STEP_TICKS, 39)
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
