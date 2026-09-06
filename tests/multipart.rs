use phasecraft::{
    music::{Composition, MAX_PARTS},
    music::{
        resolve::{resolve, resolve_step},
        rhythm::Expression,
    },
};

fn techno() -> Composition {
    Composition::parse(include_str!("../examples/quickstart/techno.toml")).unwrap()
}
fn dnb() -> Composition {
    Composition::parse(include_str!("../examples/quickstart/dnb.toml")).unwrap()
}
fn positions(c: &Composition, id: &str) -> Vec<u64> {
    let part = c.parts.iter().find(|p| p.id == id).unwrap();
    (0..16)
        .filter(|step| resolve(c, part, *step).event.is_some())
        .collect()
}
#[test]
fn examples_have_conventional_anchors_and_kit_notes() {
    let t = techno();
    let d = dnb();
    assert_eq!(t.tempo, 132.0);
    assert_eq!(d.tempo, 172.0);
    assert_eq!(positions(&t, "kick"), vec![0, 4, 8, 12]);
    assert_eq!(positions(&t, "clap"), vec![4, 12]);
    assert_eq!(positions(&t, "open_hat"), vec![2, 6, 10, 14]);
    assert_eq!(positions(&d, "kick"), vec![0, 10]);
    assert_eq!(positions(&d, "snare"), vec![4, 12]);
    for c in [&t, &d] {
        for part in &c.parts {
            let note = match part.id.as_str() {
                "kick" => 36,
                "rim" => 37,
                "snare" => 38,
                "clap" => 39,
                "closed_hat" => 42,
                "open_hat" => 46,
                _ => panic!("unexpected kit role"),
            };
            assert_eq!(part.output.note, note);
            assert_eq!(part.output.channel, 10);
        }
        for step in 0..560 {
            let closed = c.parts.iter().find(|p| p.id == "closed_hat").unwrap();
            let open = c.parts.iter().find(|p| p.id == "open_hat").unwrap();
            assert!(
                !(resolve(c, closed, step).event.is_some()
                    && resolve(c, open, step).event.is_some())
            );
        }
    }
}
#[test]
fn reordering_adding_and_editing_other_parts_preserves_identity() {
    let c = techno();
    let mut reordered = c.clone();
    reordered.parts.reverse();
    let mut extended = c.clone();
    let mut extra = c.parts[0].clone();
    extra.id = "extra".into();
    extra.output.note = 51;
    extra.trigger.probability = 0.37;
    extended.parts.push(extra);
    extended.parts[1].accent.probability = 0.03;
    extended.validate().unwrap();
    for step in 0..560 {
        let (traces, midi) = resolve_step(&c, step);
        let (again, midi_again) = resolve_step(&reordered, step);
        assert_eq!(
            serde_json::to_string(&traces).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
        assert_eq!(midi, midi_again);
        let a = resolve(&c, &c.parts[0], step);
        let b = resolve(&extended, &extended.parts[0], step);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}
#[test]
fn simultaneous_hits_are_merged_before_any_note_off() {
    let mut c = techno();
    for (i, part) in c.parts.iter_mut().enumerate() {
        part.trigger.rhythm = Expression::Euclidean {
            steps: 1,
            pulses: 1,
            rotation: 0,
            reset_on_phrase: false,
        };
        part.trigger.probability = 1.0;
        part.output.gate_ticks = 30 + i as u64 * 20;
    }
    let (_, midi) = resolve_step(&c, 4);
    assert_eq!(midi.len(), 10);
    assert!(
        midi[..5]
            .iter()
            .all(|e| e.tick == 960 && e.bytes[0] & 0xf0 == 0x90)
    );
    assert!(
        midi[5..]
            .iter()
            .all(|e| e.tick > 960 && e.bytes[0] & 0xf0 == 0x80)
    );
    assert!(midi.windows(2).all(|pair| pair[0].tick <= pair[1].tick));
}
#[test]
fn invalid_part_collections_are_rejected() {
    let mut c = techno();
    c.parts.clear();
    assert!(c.validate().is_err());
    let mut c = techno();
    c.parts[1].id = c.parts[0].id.clone();
    assert!(c.validate().is_err());
    let mut c = techno();
    c.parts[1].output = c.parts[0].output.clone();
    assert!(c.validate().is_err());
    c.parts[1].output.channel = 11;
    assert!(c.validate().is_ok());
    let mut c = techno();
    c.parts = vec![c.parts[0].clone(); MAX_PARTS + 1];
    assert!(c.validate().is_err());
    let mixed = format!(
        "{}\n{}",
        include_str!("../examples/quickstart/hat.toml"),
        include_str!("../examples/quickstart/techno.toml")
            .split("[[parts]]")
            .skip(1)
            .map(|s| format!("[[parts]]{s}"))
            .collect::<String>()
    );
    assert!(Composition::parse(&mixed).is_err());
    let source = include_str!("../examples/quickstart/techno.toml");
    assert!(Composition::parse(&source.replace("id = \"kick\"", "id = \"\"")).is_err());
}
#[test]
fn legacy_hat_and_new_parts_syntax_produce_identical_streams() {
    let source = include_str!("../examples/quickstart/hat.toml");
    let old = Composition::parse(source).unwrap();
    let new = Composition::parse(
        &source
            .replace("[part]", "[[parts]]")
            .replace("[part.", "[parts."),
    )
    .unwrap();
    for step in 0..560 {
        assert_eq!(
            serde_json::to_string(&resolve_step(&old, step).0).unwrap(),
            serde_json::to_string(&resolve_step(&new, step).0).unwrap()
        );
        assert_eq!(resolve_step(&old, step).1, resolve_step(&new, step).1);
    }
}
