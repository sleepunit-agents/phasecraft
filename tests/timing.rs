use phasecraft::music::{
    Composition, STEP_TICKS,
    resolve::{Compiled, MidiEvent, realize, resolve_step},
    time::NoteValue,
};
fn song(fields: &str) -> Composition {
    Composition::parse(&format!("tempo=132\nseed=123\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.rhythm={{steps=1,pulses=1}}\naccent.rhythm={{steps=3,pulses=1}}\n{fields}")).unwrap()
}
fn render(c: &Composition, steps: u64) -> Vec<MidiEvent> {
    let mut compiled = Compiled::new(c);
    (0..steps)
        .flat_map(|s| compiled.resolve_step(s).1)
        .collect()
}
fn onsets(events: &[MidiEvent]) -> Vec<u64> {
    events
        .iter()
        .filter(|m| m.bytes[0] & 0xf0 == 0x90)
        .map(|m| m.tick)
        .collect()
}
fn balanced(events: &[MidiEvent]) {
    let mut active = std::collections::BTreeSet::new();
    assert!(events.windows(2).all(|w| w[0].tick <= w[1].tick));
    for e in events {
        let key = (e.bytes[0] & 15, e.bytes[1]);
        match e.bytes[0] & 0xf0 {
            0x90 => assert!(active.insert(key), "overlapping note at {}", e.tick),
            0x80 => assert!(active.remove(&key), "orphan release at {}", e.tick),
            _ => (),
        }
    }
    assert!(active.is_empty(), "unfinished notes: {active:?}");
}
#[test]
fn triplets_and_dots_use_exact_ticks_without_drift() {
    for (value, ticks) in [
        ("1/8T", 320),
        ("1/16T", 160),
        ("1/16.", 360),
        ("1/8.", 720),
        ("1/64T", 40),
    ] {
        assert_eq!(NoteValue::parse(value).unwrap().0, ticks);
        let c = song(&format!("subdivision='{value}'"));
        let events = render(&c, 16 * 35);
        assert_eq!(
            onsets(&events),
            (0..16 * 35 * STEP_TICKS)
                .step_by(ticks as usize)
                .collect::<Vec<_>>()
        );
        balanced(&events);
        assert_eq!(
            realize(&c, &c.parts[0], 17, 31)
                .events
                .iter()
                .map(|e| e.tick)
                .collect::<Vec<_>>(),
            onsets(&events)
                .into_iter()
                .filter(|&t| (17 * 240..48 * 240).contains(&t))
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn mixed_grid_references_mean_exact_source_coincidence() {
    let c = Composition::parse("tempo=132\nseed=1\n[parts.kick]\nuse='techno.kick'\ntrigger.rhythm={steps=1,pulses=1}\n[parts.hat]\nuse='techno.closed_hat'\nsubdivision='1/8T'\ntrigger.rhythm={part='kick',mode='hits'}").unwrap();
    let events = render(&c, 16);
    assert_eq!(
        onsets(
            &events
                .into_iter()
                .filter(|m| m.bytes[1] == 42)
                .collect::<Vec<_>>()
        ),
        vec![0, 960, 1920, 2880]
    );
}
#[test]
fn ornaments_have_independent_admissions_and_complete_releases() {
    let base = song("");
    let c = song(
        "ornaments.ratchet={count=3,probability=0.65}\nornaments.flam={spacing='1/64T',probability=0.5,gain=0.4}",
    );
    for s in 0..64 {
        let a = resolve_step(&base, s).0;
        let b = resolve_step(&c, s).0;
        assert_eq!(
            serde_json::to_value(&a[0].trigger).unwrap(),
            serde_json::to_value(&b[0].trigger).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&a[0].accent).unwrap(),
            serde_json::to_value(&b[0].accent).unwrap()
        );
    }
    let events = render(&c, 64);
    balanced(&events);
    assert!(onsets(&events).len() > 64);
    let mut muted = c.clone();
    muted.parts[0].trigger.probability = 0.;
    assert!(onsets(&render(&muted, 64)).is_empty());
    let trace = resolve_step(&song("ornaments.ratchet={count=3}"), 0)
        .0
        .remove(0);
    assert_eq!(
        trace
            .extra_events
            .iter()
            .map(|e| e.tick)
            .collect::<Vec<_>>(),
        vec![80, 160]
    );
}
#[test]
fn anticipation_and_flam_are_dispatched_in_the_previous_window_but_not_previous_bar() {
    let c = song("groove.delay_ticks=-30\nornaments.flam={spacing='1/64T'}");
    let events = render(&c, 32);
    assert!(onsets(&events).contains(&170)); // grace before source tick 240's early main at 210
    assert!(onsets(&events).contains(&210));
    assert!(!onsets(&events).contains(&(3840 - 30)));
    assert!(onsets(&events).contains(&3840));
    balanced(&events);
    assert_eq!(
        resolve_step(&c, 1).0[0]
            .event
            .as_ref()
            .unwrap()
            .groove
            .as_ref()
            .unwrap()
            .advance_ticks,
        30
    );
}
#[test]
fn compiled_random_access_matches_fresh_snapshots_and_edits_invalidate_decisions() {
    let c = song(
        "subdivision='1/16T'\ngroove.run='ramp_up'\ntrigger.probability=0.7\nornaments.ratchet={count=3,probability=0.4}",
    );
    let mut compiled = Compiled::new(&c);
    for s in [0, 17, 12, 17, 560, 2, 1024, 17] {
        let (a, midi) = compiled.resolve_step(s);
        let (b, fresh) = resolve_step(&c, s);
        assert_eq!(
            serde_json::to_value(a).unwrap(),
            serde_json::to_value(b).unwrap()
        );
        assert_eq!(midi, fresh);
    }
    let mut changed = c.clone();
    changed.parts[0].trigger.probability = 0.;
    assert!(onsets(&render(&changed, 16)).is_empty());
}
#[test]
fn musical_gates_and_invalid_timing_are_validated() {
    let c = song("subdivision='1/4.'\noutput.gate='1/8T'");
    assert_eq!(c.parts[0].output.gate_ticks, 320);
    assert_eq!(
        resolve_step(&c, 0).0[0]
            .event
            .as_ref()
            .unwrap()
            .duration_ticks,
        320
    );
    for bad in [
        "subdivision='1/7'",
        "subdivision='16t'",
        "ornaments.ratchet={count=9}",
        "ornaments.flam={spacing='1/4'}",
        "output.gate='1/8'\noutput.gate_ticks=2",
        "ornaments.ratchet={count=3,probability=nan}",
    ] {
        assert!(
            Composition::parse(&format!(
                "tempo=132\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\n{bad}"
            ))
            .is_err(),
            "{bad}"
        );
    }
}
#[test]
fn dense_swing_ornaments_and_early_timing_never_overlap_notes() {
    for division in ["1/16", "1/8T", "1/16T"] {
        for swing in [0.5, 0.66, 0.75] {
            for delay in [-60, 0, 60] {
                let c = song(&format!(
                    "subdivision='{division}'\ngroove.swing={swing}\ngroove.delay_ticks={delay}\ngroove.humanize={{timing_ticks=30}}\nornaments.ratchet={{count=8}}\nornaments.flam={{spacing='1/64T'}}"
                ));
                balanced(&render(&c, 64));
            }
        }
    }
}

#[test]
fn shared_accent_and_phrase_reset_use_musical_position() {
    let c = Composition::parse("tempo=132\nseed=1\nphrase_bars=1\n[accents.all]\nrhythm={steps=1,pulses=1}\n[parts.hat]\nuse='techno.closed_hat'\nsubdivision='1/8T'\ntrigger.rhythm={steps=1,pulses=1}\naccent.rhythm={steps=7,pulses=3,reset_on_phrase=true}\naccent.sources=['all']").unwrap();
    let traces: Vec<_> = (0..32)
        .flat_map(|s| resolve_step(&c, s).0)
        .filter(|t| t.event.is_some())
        .collect();
    for t in &traces {
        assert_eq!(
            t.shared_accents[0].decision.admitted,
            t.tick.is_multiple_of(960)
        );
        let phase = serde_json::to_value(&t.accent.rhythm).unwrap()["phase"]
            .as_u64()
            .unwrap();
        assert_eq!(phase, (t.tick % 3840) / 320 % 7);
    }
    assert_eq!(traces[0].trigger.roll, traces[12].trigger.roll);
    let pattern = realize(&c, &c.parts[0], 0, 16);
    assert_eq!(pattern.cycles[0].phase_alignment_ticks, Some(3840));
}
#[test]
fn ornament_controls_and_automation_match_every_actual_gate() {
    let c = song(
        "ornaments.ratchet={count=3}\nornaments.flam={spacing='1/64T'}\noutput.controls.cutoff={cc=74,default=1.0}\nprofile.controls.cutoff={base=0.2,boost=0.6}\nparameters.cutoff={value=0.2,ramp={to=0.8,over_bars=2}}",
    );
    let events = render(&c, 32);
    balanced(&events);
    for note in events.iter().filter(|m| m.bytes[0] & 0xf0 == 0x90) {
        assert!(
            events
                .iter()
                .any(|m| m.tick == note.tick && m.bytes[0] & 0xf0 == 0xb0)
        );
    }
    let mut muted = c.clone();
    muted.parts[0].trigger.probability = 0.;
    assert!(
        render(&muted, 32)
            .iter()
            .any(|m| m.parameter && m.tick > 3840)
    );
}
#[test]
fn sections_and_snapshot_boundaries_finish_ornament_ownership() {
    let mut c = song(
        "subdivision='1/8T'\nornaments.ratchet={count=3}\nornaments.flam={spacing='1/64T'}\ngroove.delay_ticks=-30",
    );
    let mut quiet = c.clone();
    quiet.parts[0].trigger.probability = 0.;
    let mut spliced = render(&c, 16);
    let mut next = Compiled::new(&quiet);
    spliced.extend((16..32).flat_map(|s| next.resolve_step(s).1));
    balanced(&spliced);
    use phasecraft::music::arrangement::{Arrangement, PhasePolicy, Section};
    for phase in [PhasePolicy::Restart, PhasePolicy::Continue] {
        c.arrangement = Some(Arrangement {
            repeat: false,
            sections: vec![
                Section {
                    phrase: "A".into(),
                    bars: 1,
                    phase,
                    composition: Box::new(song(
                        "subdivision='1/16.'\nornaments.ratchet={count=3}\ngroove.delay_ticks=-30",
                    )),
                },
                Section {
                    phrase: "B".into(),
                    bars: 1,
                    phase,
                    composition: Box::new(quiet.clone()),
                },
            ],
        });
        c.validate().unwrap();
        let events = render(&c, 32);
        balanced(&events);
        assert!(onsets(&events).iter().all(|&t| t < 3840));
    }
}
#[test]
fn timing_starter_compositions_resolve_with_prepared_909_routes() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/project/compositions");
    for name in ["triplet-techno", "ratchet-breaks", "dotted-garage"] {
        let c = Composition::read(&root.join(format!("{name}.toml"))).unwrap();
        assert_eq!(c.parts[0].id, "kick");
        assert!(c.parts.iter().all(|p| p.output.channel == 10));
        let events = render(&c, 16 * 35);
        balanced(&events);
        assert!(
            events
                .iter()
                .any(|m| m.tick % 240 != 0 && m.bytes[0] & 0xf0 == 0x90)
        );
    }
}
