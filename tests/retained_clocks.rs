use phasecraft::music::{
    Composition, STEP_TICKS, cycle,
    resolve::Compiled,
    router::{Clock, member_clock},
};

const PIECE: &str = include_str!("../examples/studies/retained-returns.toml");

#[test]
fn real_source_clocks_drive_returns_without_claiming_repeated_material() {
    let c = Composition::parse(PIECE).unwrap();
    for (key, slots) in [
        ("voices.hat.trigger.cycle", 16),
        ("parts.hat.trigger.cycle", 16),
        ("patterns.hat-memory.change.every", 5),
        ("lanes.drift.every", 7),
    ] {
        assert_eq!(
            member_clock(&c, key).unwrap(),
            Clock::Fixed(slots * STEP_TICKS)
        );
    }
    assert_eq!(
        c.router.as_ref().unwrap().period_ticks(&c).unwrap(),
        560 * STEP_TICKS
    );
    assert_eq!(cycle::trigger_period_ticks(&c, &c.parts[0]).unwrap(), None);
    assert!(
        cycle::spans(&c, "hat", 0, 1121)
            .iter()
            .all(|s| s.phase_alignment_ticks.is_none())
    );
    let mut compiled = Compiled::new(&c);
    for slot in [559, 560, 1119, 1120, 1679, 1680] {
        let traces = compiled.resolve_step(slot).0;
        let visit = traces[0].scene.as_ref().unwrap();
        assert_eq!(visit.index, slot / 560);
        assert_eq!(visit.start_tick, slot / 560 * 560 * STEP_TICKS);
    }
    // An unwatched mutation clock still exists. No voice is needed to supply it.
    let mut c = c.clone();
    c.parts.clear();
    assert_eq!(
        member_clock(&c, "patterns.hat-memory.change.every").unwrap(),
        Clock::Fixed(5 * STEP_TICKS)
    );
    for (key, needle) in [
        ("patterns.missing.change.every", "unknown retained pattern"),
        ("patterns.hat-memory.cycle", "not a pattern member clock"),
        ("patterns.hat-memory.change", "not a pattern member clock"),
    ] {
        let error = member_clock(&c, key).unwrap_err();
        assert!(error.contains(key) && error.contains(needle), "{error}");
    }
}

#[test]
fn simultaneous_return_commit_and_opportunity_use_the_incoming_room() {
    let text = PIECE
        .replace(
            "crowded = 0.23, exposed = 0.05",
            "crowded = 1.0, exposed = 1.0",
        )
        .replace(
            "crowded = { crowded = 0.70, exposed = 0.25, hollow = 0.05 }",
            "crowded = { hollow = 1.0 }",
        )
        .replace(
            "exposed = { crowded = 0.30, exposed = 0.55, hollow = 0.15 }",
            "exposed = { crowded = 1.0 }",
        )
        .replace(
            "hollow = { crowded = 0.30, exposed = 0.45, hollow = 0.25 }",
            "hollow = { exposed = 1.0 }",
        );
    let c = Composition::parse(&text).unwrap();
    let mut source = c.patterns["hat-memory"]
        .bind("hat-memory", &["crowded", "hollow", "exposed"])
        .unwrap();
    let mut compiled = Compiled::new(&c);
    let mut snapshots = Vec::new();
    for slot in 0..1700 {
        let scene = ["crowded", "hollow", "exposed"][(slot / 560 % 3) as usize];
        let sample = source.advance(scene, c.dice()).unwrap();
        assert_eq!(
            source.state().material().len(),
            16,
            "swap conserves slot count"
        );
        if slot == 560 {
            assert!(
                sample.step.committed,
                "the prior pending edit was published"
            );
            assert!(
                sample.draws.is_empty(),
                "entering hollow suspends the simultaneous opportunity"
            );
            assert!(
                source.state().pending().is_none(),
                "the prior pending edit committed first"
            );
        }
        if slot == 1120 {
            assert_eq!(
                sample.draws.len(),
                3,
                "leaving hollow resumes at that same boundary"
            );
            assert!(source.state().pending().is_some());
        }
        let active = source.state().material()[(slot % 16) as usize];
        let (traces, midi) = compiled.resolve_step(slot);
        assert_eq!(traces[0].scene.as_ref().unwrap().scene, scene);
        assert_eq!(traces[0].trigger.rhythm.active(), active, "slot {slot}");
        assert_eq!(traces[0].trigger.admitted, active && scene != "hollow");
        assert_eq!(
            midi.iter()
                .filter(|e| e.bytes[0] & 0xf0 == 0x90 && e.bytes[2] > 0)
                .count(),
            usize::from(active && scene != "hollow")
        );
        snapshots.push(format!(
            "{} {midi:?}",
            serde_json::to_string(&traces).unwrap()
        ));
    }
    let roundtrip = Composition::parse(&toml::to_string(&c).unwrap()).unwrap();
    let mut seek = Compiled::new(&roundtrip);
    for slot in [1120, 559, 560, 0, 1680, 1119] {
        let (traces, midi) = seek.resolve_step(slot);
        assert_eq!(
            format!("{} {midi:?}", serde_json::to_string(&traces).unwrap()),
            snapshots[slot as usize]
        );
    }
}

#[test]
fn clocks_derive_from_material_and_cadence_and_scene_period_changes_reject() {
    let text = PIECE
        .replace(
            "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~",
            "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~ ~",
        )
        .replace("slots = 5", "slots = 11")
        .replace("slots = 7", "slots = 16");
    let c = Composition::parse(&text).unwrap();
    assert_eq!(
        c.router.as_ref().unwrap().period_ticks(&c).unwrap(),
        17 * 11 * 16 * STEP_TICKS
    );
    let text = PIECE.replace(
        "[scenes.exposed]",
        "[scenes.exposed]\n[[scenes.exposed.parts]]\nid = 'hat'\nuse = 'techno.closed_hat'\ntrigger.rhythm = { steps = 8, pulses = 3 }",
    );
    let error = Composition::parse(&text).unwrap_err();
    assert!(
        error.contains("changes the period") && error.contains("voices.hat.trigger.cycle"),
        "{error}"
    );
    // A reader can leave a scene while its fixed clock keeps running.
    let text = PIECE.replace(
        "parts.hat.trigger.probability = 0.0",
        "[[scenes.hollow.parts]]\nid = 'kick'\nuse = 'techno.kick'",
    );
    let c = Composition::parse(&text).unwrap();
    assert_eq!(
        c.router.as_ref().unwrap().period_ticks(&c).unwrap(),
        560 * STEP_TICKS
    );
}

#[test]
fn retained_followers_refuse_return_clocks_with_the_absent_period_cause() {
    for rhythm in [
        "{ part = 'hat', mode = 'hits' }",
        "{ part = 'hat', mode = 'structural' }",
        "{ op = 'or', a = { part = 'hat', mode = 'hits' }, b = { steps = 3, pulses = 1 } }",
        "{ op = 'and', a = { steps = 3, pulses = 1 }, b = { part = 'hat', mode = 'structural' } }",
    ] {
        for stream in ["trigger", "accent"] {
            let text = format!(
                "{PIECE}\n[parts.ref]\nuse = 'techno.closed_hat'\noutput.note = 44\n{stream}.rhythm = {rhythm}\n"
            );
            // A follower is supported when it is not asked to supply a return clock.
            let c = Composition::parse(&text).unwrap();
            let follower = c.parts.iter().find(|p| p.id == "ref").unwrap();
            let period = if stream == "trigger" {
                cycle::trigger_period_ticks(&c, follower)
            } else {
                cycle::accent_period_ticks(&c, follower)
            };
            assert_eq!(period.unwrap(), None);
            let key = format!("voices.ref.{stream}.cycle");
            let error = member_clock(&c, &key).unwrap_err();
            assert!(
                error.contains(&key)
                    && error.contains("no fixed structural repetition period")
                    && error.contains("retained material")
                    && !error.contains("u64"),
                "{error}"
            );
            let error =
                Composition::parse(&text.replace("voices.hat.trigger.cycle", &key)).unwrap_err();
            assert!(
                error.contains("returns.hat_system")
                    && error.contains(&key)
                    && error.contains("no fixed structural repetition period"),
                "{error}"
            );
        }
    }
}

#[test]
fn oversized_fixed_followers_keep_the_overflow_cause() {
    let mut rhythm = "{ steps = 65521, pulses = 1 }".to_string();
    for steps in [65519, 65497, 65479, 65449] {
        rhythm = format!("{{ op = 'or', a = {rhythm}, b = {{ steps = {steps}, pulses = 1 }} }}");
    }
    let text = format!(
        "tempo = 126\nseed = 1\n[parts.large]\nuse = 'techno.closed_hat'\ntrigger.rhythm = {rhythm}\n[parts.ref]\nuse = 'techno.closed_hat'\noutput.note = 44\ntrigger.rhythm = {{ part = 'large', mode = 'structural' }}"
    );
    let c = Composition::parse(&text).unwrap();
    for id in ["large", "ref"] {
        let key = format!("voices.{id}.trigger.cycle");
        let error = member_clock(&c, &key).unwrap_err();
        assert!(
            error.contains(&key)
                && error.contains("does not fit in u64 ticks")
                && !error.contains("retained"),
            "{error}"
        );
    }
}
