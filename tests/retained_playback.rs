use phasecraft::music::{Composition, STEP_TICKS, resolve::Compiled};
const HAT: &str = include_str!("fixtures/hat-memory.toml");
fn base() -> String {
    let hat = HAT.replace("[change", "[patterns.memory.change");
    format!(
        r#"
tempo=126
seed=91827
phrase_bars=1
[patterns.memory]
{hat}
[parts.hat]
use='techno.closed_hat'
trigger.pattern={{pattern='memory'}}
trigger.probability=1.0
accent.probability=0.0
[parts.other]
use='techno.closed_hat'
output.note=44
trigger.pattern={{pattern='memory'}}
trigger.probability=1.0
accent.probability=0.0
"#
    )
}
fn arranged() -> Composition {
    Composition::parse(&format!(
        r#"{}
[phrases.crowded]
[phrases.hollow]
parts.hat.trigger.pattern='~'
parts.other.trigger.pattern='~'
[phrases.exposed]
seed=999
[arrangement]
repeat=true
sections=[{{phrase='crowded',bars=1}},{{phrase='hollow',bars=1}},{{phrase='exposed',bars=1}}]
"#,
        base()
            .replace("crowded = 0.23", "crowded = 0.23, hollow = 1.0")
            .replace(
                "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~\"",
                "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~ ~\""
            )
    ))
    .unwrap()
}
fn expected(c: &Composition, count: u64) -> Vec<bool> {
    let mut source = c.patterns["memory"]
        .bind("memory", &["crowded", "hollow", "exposed"])
        .unwrap();
    (0..count)
        .map(|slot| {
            let scene = if let Some(a) = &c.arrangement {
                a.locate(slot).unwrap().section.phrase.clone()
            } else {
                let r = c.router.as_ref().unwrap();
                r.visit_at(c, r.period_ticks(c).unwrap(), slot * STEP_TICKS)
                    .scene
            };
            source.advance(&scene, c.dice()).unwrap();
            let m = source.state().material();
            m[(slot % m.len() as u64) as usize]
        })
        .collect()
}
fn assert_slot(compiled: &mut Compiled, c: &Composition, slot: u64, active: bool) -> String {
    let (traces, midi) = compiled.resolve_step(slot);
    let silent = c
        .arrangement
        .as_ref()
        .is_some_and(|a| a.locate(slot).unwrap().section.phrase == "hollow");
    for t in &traces {
        assert_eq!(
            t.trigger.rhythm.active(),
            active && !silent,
            "slot {slot} part {}",
            t.part
        );
        assert_eq!(t.trigger.admitted, active && !silent);
    }
    let notes: Vec<_> = midi
        .iter()
        .filter(|e| e.bytes[0] & 0xf0 == 0x90 && e.bytes[2] > 0)
        .collect();
    assert_eq!(
        notes.len(),
        if active && !silent { 2 } else { 0 },
        "MIDI slot {slot}"
    );
    format!("{} {midi:?}", serde_json::to_string(&traces).unwrap())
}
#[test]
fn readers_share_transport_through_silent_scenes_restarts_and_random_seeks() {
    let c = arranged();
    let expected = expected(&c, 5000);
    let mut compiled = Compiled::new(&c);
    let mut snapshots = Vec::new();
    for slot in 0..5000 {
        snapshots.push(assert_slot(
            &mut compiled,
            &c,
            slot,
            expected[slot as usize],
        ));
    }
    for slot in [4999, 0, 17, 80, 96, 3, 4321, 32, 4998, 48, 260] {
        assert_eq!(
            assert_slot(&mut compiled, &c, slot, expected[slot as usize]),
            snapshots[slot as usize]
        );
    }
    let roundtrip = Composition::parse(&toml::to_string(&c).unwrap()).unwrap();
    let mut fresh = Compiled::new(&roundtrip);
    for slot in [260, 16, 48, 80, 4999] {
        assert_eq!(
            assert_slot(&mut fresh, &roundtrip, slot, expected[slot as usize]),
            snapshots[slot as usize]
        );
    }
}
#[test]
fn routed_material_uses_incoming_scene_and_root_seed() {
    let c = Composition::parse(&format!(
        r#"{}
[parts.clock]
use='techno.kick'
trigger.rhythm={{steps=16,pulses=0}}
[scenes.crowded]
[scenes.hollow]
[scenes.exposed]
seed=7
[returns.bar]
align=['voices.clock.trigger.cycle']
[router]
start='crowded'
every={{returns='bar'}}
[router.routes]
crowded={{hollow=1.0}}
hollow={{exposed=1.0}}
exposed={{crowded=0.5,exposed=0.5}}
"#,
        base()
    ))
    .unwrap();
    let expected = expected(&c, 350);
    let mut compiled = Compiled::new(&c);
    for slot in 0..350 {
        let (traces, midi) = compiled.resolve_step(slot);
        for t in traces.iter().filter(|t| t.part != "clock") {
            assert_eq!(t.trigger.admitted, expected[slot as usize], "slot {slot}");
        }
        assert_eq!(
            midi.iter()
                .filter(|e| e.bytes[0] & 0xf0 == 0x90 && e.bytes[2] > 0)
                .count(),
            if expected[slot as usize] { 2 } else { 0 }
        );
    }
}
#[test]
fn load_rejects_invalid_readers_vocabulary_and_child_sources() {
    for (text, needle) in [
        (
            base().replace("pattern='memory'", "pattern='missing'"),
            "unknown retained pattern",
        ),
        (
            base().replace(
                "use='techno.closed_hat'",
                "use='techno.closed_hat'\nsubdivision='1/8'",
            ),
            "subdivision",
        ),
        (
            base().replace(
                "trigger.pattern={pattern='memory'}",
                "accent.rhythm={type='retained',pattern='memory'}",
            ),
            "trigger source",
        ),
        (
            format!(
                "{}\n[phrases.crowded]\n[arrangement]\nsections=[{{phrase='crowded',bars=1}}]",
                base()
            ),
            "unknown chance scene",
        ),
    ] {
        let e = Composition::parse(&text).unwrap_err();
        assert!(e.contains(needle), "{e}");
    }
    let error = Composition::parse(&format!(
        "{}\n[returns.retained]\nalign=['voices.hat.trigger.cycle']",
        base()
    ))
    .unwrap_err();
    assert!(
        error.contains("no fixed repeating trigger period"),
        "{error}"
    );
    let mut c = arranged();
    c.arrangement.as_mut().unwrap().sections[0]
        .composition
        .patterns
        .clear();
    assert!(c.validate().unwrap_err().contains("must match"));
}
#[test]
fn standalone_default_scene_and_probability_zero_keep_structural_material() {
    let text = base()
        .replace("crowded = 0.23", "default = 1.0")
        .replace("exposed = 0.05", "");
    let c = Composition::parse(&text).unwrap();
    let mut source = c.patterns["memory"].bind("memory", &["default"]).unwrap();
    let mut compiled = Compiled::new(&c);
    for slot in 0..130 {
        source.advance("default", c.dice()).unwrap();
        let active = source.state().material()[slot as usize % 16];
        assert_slot(&mut compiled, &c, slot, active);
    }
    let mut muted = c.clone();
    for p in &mut muted.parts {
        p.trigger.probability = 0.0;
    }
    let mut compiled = Compiled::new(&muted);
    for slot in 0..130 {
        let (t, m) = compiled.resolve_step(slot);
        assert!(m.is_empty());
        assert!(t.iter().all(|t| !t.trigger.admitted));
    }
}

#[test]
fn neighbor_groove_and_part_references_preserve_random_lookup_results() {
    let text = base()
        .replace("crowded = 0.23", "default = 0.75")
        .replace("exposed = 0.05", "");
    let mut c = Composition::parse(&text).unwrap();
    c.parts[0].groove.run = phasecraft::music::groove::RunContour::RampUp;
    c.parts[1].trigger.rhythm = phasecraft::music::rhythm::Expression::Part {
        id: c.parts[0].id.clone(),
        mode: phasecraft::music::rhythm::ReferenceMode::Hits,
    };
    c.validate().unwrap();
    let mut compiled = Compiled::new(&c);
    let snapshots: Vec<_> = (0..300)
        .map(|slot| {
            let (traces, midi) = compiled.resolve_step(slot);
            assert_eq!(traces[0].trigger.admitted, traces[1].trigger.admitted);
            format!("{} {midi:?}", serde_json::to_string(&traces).unwrap())
        })
        .collect();
    let mut seek = Compiled::new(&c);
    for slot in [80, 0, 16, 95, 299, 260, 15, 96, 31] {
        let (traces, midi) = seek.resolve_step(slot);
        assert_eq!(
            format!("{} {midi:?}", serde_json::to_string(&traces).unwrap()),
            snapshots[slot as usize]
        );
    }
    assert!(
        phasecraft::music::cycle::trigger_period_ticks(&c, &c.parts[0])
            .unwrap()
            .is_none()
    );
}
