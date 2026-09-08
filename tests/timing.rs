use phasecraft::music::{
    Composition, STEP_TICKS,
    resolve::{Compiled, Dice, MidiEvent, realize, resolve_step},
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

#[test]
fn ornament_trace_separates_gate_refusal_from_boundary_suppression() {
    use phasecraft::music::ornament::SuppressionReason::{LowerBound, Probability, UpperBound};

    let c = song("ornaments.ratchet={count=3}\nornaments.flam={spacing='1/64T'}");
    let mut event = resolve_step(&song(""), 0).0.remove(0).event.unwrap();
    event.tick = 120;
    // Ratchet requests 120/200/280; 280 cannot close before exclusive upper 281.
    // Flam requests 80, before lower 100. Both gates admitted independently.
    let (hits, trace) = c.parts[0].ornaments.expand(
        Dice {
            seed: 123,
            pins: &[],
        },
        "hat",
        |_, _| 0,
        &event,
        240,
        100..281,
    );
    assert_eq!(hits.iter().map(|e| e.tick).collect::<Vec<_>>(), [120, 200]);
    let r = trace.ratchet.unwrap();
    let f = trace.flam.unwrap();
    assert_eq!(
        (r.probability, r.admitted_count, r.emitted_count),
        (1.0, 3, 2)
    );
    assert_eq!(r.suppression_reason, Some(UpperBound));
    assert_eq!(
        (f.probability, f.admitted_count, f.emitted_count),
        (1.0, 1, 0)
    );
    assert_eq!(f.suppression_reason, Some(LowerBound));
    assert_eq!(trace.ratchet_count, 3);
    assert!(!trace.flam_active);

    // One more tick admits the last tail; equality with lower admits the grace.
    let (hits, trace) = c.parts[0].ornaments.expand(
        Dice {
            seed: 123,
            pins: &[],
        },
        "hat",
        |_, _| 0,
        &event,
        240,
        80..282,
    );
    assert_eq!(
        hits.iter().map(|e| e.tick).collect::<Vec<_>>(),
        [80, 120, 200, 280]
    );
    assert_eq!(trace.ratchet.as_ref().unwrap().emitted_count, 3);
    assert_eq!(trace.flam.as_ref().unwrap().emitted_count, 1);
    assert!(trace.ratchet.unwrap().suppression_reason.is_none());
    assert!(trace.flam.unwrap().suppression_reason.is_none());

    let refused = song(
        "ornaments.ratchet={count=3,probability=0}\nornaments.flam={spacing='1/64T',probability=0}",
    );
    let (hits, trace) = refused.parts[0].ornaments.expand(
        Dice {
            seed: 123,
            pins: &[],
        },
        "hat",
        |_, _| 0,
        &event,
        240,
        100..281,
    );
    assert_eq!(hits.len(), 1); // the source still sounds, outside either refused expansion
    assert_eq!(trace.ratchet_count, 1);
    assert!(!trace.flam_active);
    for expansion in [trace.ratchet.unwrap(), trace.flam.unwrap()] {
        assert_eq!(
            (
                expansion.probability,
                expansion.admitted_count,
                expansion.emitted_count
            ),
            (0.0, 0, 0)
        );
        assert_eq!(expansion.suppression_reason, Some(Probability));
    }
}

#[test]
fn downbeat_flam_trace_records_spent_attack_every_bar() {
    use phasecraft::music::ornament::SuppressionReason::LowerBound;
    let c = song("ornaments.flam={spacing='1/64T'}");
    for step in [0, 16, 32, 48] {
        let trace = resolve_step(&c, step).0.remove(0);
        let ornaments = trace.ornaments.unwrap();
        assert!(ornaments.ratchet.is_none());
        let flam = ornaments.flam.unwrap();
        assert_eq!((flam.admitted_count, flam.emitted_count), (1, 0));
        assert_eq!(flam.suppression_reason, Some(LowerBound));
        assert!(!ornaments.flam_active);
        assert!(trace.extra_events.is_empty());
        assert_eq!(trace.event.unwrap().tick, step * STEP_TICKS);
    }
    assert!(resolve_step(&song(""), 0).0[0].ornaments.is_none());
    let muted = song("trigger.probability=0\nornaments.flam={spacing='1/64T'}");
    assert!(resolve_step(&muted, 0).0[0].ornaments.is_none());
}

#[test]
fn ornament_trace_samples_match_rolls_and_expansion_output() {
    for probability in [0.0, 0.37, 1.0] {
        let c = song(&format!(
            "ornaments.ratchet={{count=3,probability={probability}}}\nornaments.flam={{spacing='1/64T',probability={probability}}}"
        ));
        for step in 0..64 {
            let trace = resolve_step(&c, step).0.remove(0);
            let ornaments = trace.ornaments.as_ref().unwrap();
            let r = ornaments.ratchet.as_ref().unwrap();
            let f = ornaments.flam.as_ref().unwrap();
            assert_eq!(r.probability, probability);
            assert_eq!(f.probability, probability);
            assert_eq!(
                r.admitted_count,
                if ornaments.ratchet_roll.unwrap() < probability {
                    3
                } else {
                    0
                }
            );
            assert_eq!(
                f.admitted_count,
                u8::from(ornaments.flam_roll.unwrap() < probability)
            );
            // This straight-grid fixture has no coincident attacks to merge.
            assert_eq!(
                trace.extra_events.len() + usize::from(trace.event.is_some()),
                usize::from(r.emitted_count.max(1) + f.emitted_count)
            );
            let json = serde_json::to_value(&trace).unwrap();
            assert_eq!(json["ornaments"]["ratchet"]["probability"], probability);
            if step == 0 && probability == 1.0 {
                assert_eq!(
                    json["ornaments"]["flam"]["suppression_reason"],
                    "lower_bound"
                );
            }
        }
        balanced(&render(&c, 64));
    }
}

#[test]
fn ornament_trace_exposes_compiled_bar_and_next_onset_bounds() {
    use phasecraft::music::ornament::SuppressionReason::UpperBound;
    let late = song("groove.swing=0.75\ngroove.delay_ticks=60\nornaments.ratchet={count=3}");
    let (traces, midi) = resolve_step(&late, 15);
    let trace = &traces[0];
    let ratchet = trace.ornaments.as_ref().unwrap().ratchet.as_ref().unwrap();
    assert_eq!(trace.event.as_ref().unwrap().tick, 3780);
    assert_eq!((ratchet.admitted_count, ratchet.emitted_count), (3, 1));
    assert_eq!(ratchet.suppression_reason, Some(UpperBound));
    assert_eq!(onsets(&midi), [3780]);

    // The next source reserves its grace at 200, cutting the tail at 210 even
    // though this source is nowhere near the bar end.
    let dense = song("ornaments.ratchet={count=8}\nornaments.flam={spacing='1/64T'}");
    let (traces, midi) = resolve_step(&dense, 0);
    let ratchet = traces[0]
        .ornaments
        .as_ref()
        .unwrap()
        .ratchet
        .as_ref()
        .unwrap();
    assert_eq!((ratchet.admitted_count, ratchet.emitted_count), (8, 7));
    assert_eq!(ratchet.suppression_reason, Some(UpperBound));
    assert_eq!(onsets(&midi), [0, 30, 60, 90, 120, 150, 180, 200]);
}
