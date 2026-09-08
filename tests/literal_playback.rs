use phasecraft::music::{
    Composition, STEP_TICKS,
    resolve::{Compiled, resolve_step},
    rhythm::literal::Prepared,
    time::NoteValue,
};

fn parse(pattern: &str, extra: &str) -> Result<Composition, String> {
    Composition::parse(&format!(
        r#"
tempo = 126
seed = 91827
phrase_bars = 4
[parts.sub]
trigger.pattern = {pattern:?}
trigger.probability = 0.5
accent.rhythm = {{steps=16,pulses=0}}
output.note = 48
output.channel = 2
{extra}
"#
    ))
}
fn ons(c: &Composition, steps: u64) -> Vec<(u64, u8)> {
    let mut compiled = Compiled::new(c);
    (0..steps)
        .flat_map(|s| compiled.resolve_step(s).1)
        .filter(|m| m.bytes[0] & 0xf0 == 0x90)
        .map(|m| (m.tick, m.bytes[1]))
        .collect()
}
#[test]
fn until_stop_sub_uses_written_onsets_and_holds_the_example_rest() {
    let c = parse("x ~ ~ [~ x]", "note.pattern='<c1 ~ eb1 bb0>'").unwrap();
    let expected = [36, 36, 39, 34, 36]
        .into_iter()
        .enumerate()
        .flat_map(|(bar, n)| [(bar as u64 * 3840, n), (bar as u64 * 3840 + 3360, n)])
        .collect::<Vec<_>>();
    assert_eq!(ons(&c, 80), expected);
}
#[test]
fn snare_ratchets_use_the_written_span_and_each_tail_samples_pitch() {
    let c = parse(
        "~ x ~ [x x*3?]",
        "note.pattern='c1@15 eb1'\n[[pins]]\nat={roll='burst',voice='sub',bar=1,slot=15}\nu=0.0",
    )
    .unwrap();
    assert_eq!(
        ons(&c, 16),
        vec![(960, 36), (2880, 36), (3360, 36), (3520, 36), (3680, 39)]
    );
    let traces = resolve_step(&c, 14).0;
    let ornament = traces[0].ornaments.as_ref().unwrap();
    assert_eq!(ornament.ratchet_count, 3);
    assert_eq!(ornament.ratchet.as_ref().unwrap().emitted_count, 3);
    assert!(ornament.ratchet.as_ref().unwrap().pinned);
}
#[test]
fn refusing_a_tail_keeps_the_main_and_refusing_a_question_hit_drops_it() {
    let tail = parse(
        "~ x ~ [x x*3?]",
        "[[pins]]\nat={roll='burst',voice='sub',bar=1,slot=15}\nu=0.99",
    )
    .unwrap();
    assert_eq!(ons(&tail, 16), vec![(960, 48), (2880, 48), (3360, 48)]);
    for (roll, expected) in [(0.0, vec![(0, 48), (3360, 48)]), (0.99, vec![(0, 48)])] {
        let c = parse(
            "x ~ ~ [~ x?]",
            &format!("[[pins]]\nat={{roll='fire',voice='sub',bar=1,slot=15}}\nu={roll}"),
        )
        .unwrap();
        assert_eq!(ons(&c, 16), expected);
    }
}
#[test]
fn timing_moves_the_source_once_and_bar_ownership_suppresses_late_tails() {
    for delay in [-24, 24] {
        let c = parse("~ x ~ [x x*3]", &format!("groove.delay_ticks={delay}")).unwrap();
        let expected = [960i64, 2880, 3360, 3520, 3680].map(|t| ((t + delay) as u64, 48));
        assert_eq!(ons(&c, 16), expected);
    }
    let c = parse("~ ~ ~ x*3", "groove.delay_ticks=60").unwrap();
    assert_eq!(ons(&c, 16), vec![(2940, 48), (3260, 48), (3580, 48)]);
    // The last main has only 240 ticks of written span; its shifted tails cross the bar.
    let c = parse("~@15 x*3", "groove.delay_ticks=60\ngroove.swing=0.75").unwrap();
    assert_eq!(ons(&c, 16), vec![(3780, 48)]);
    let trace = resolve_step(&c, 15).0;
    assert_eq!(
        trace[0]
            .ornaments
            .as_ref()
            .unwrap()
            .ratchet
            .as_ref()
            .unwrap()
            .emitted_count,
        1
    );
}
#[test]
fn random_seeks_find_tails_from_earlier_source_cells() {
    let c = parse("x*3 ~", "output.gate_ticks=500").unwrap();
    assert_eq!(ons(&c, 16), vec![(0, 48), (640, 48), (1280, 48)]);
    let mut sequential = Compiled::new(&c);
    let expected: Vec<_> = (0..32)
        .map(|s| {
            let (trace, midi) = sequential.resolve_step(s);
            (serde_json::to_value(trace).unwrap(), midi)
        })
        .collect();
    for s in [15, 5, 2, 31, 0, 7, 16, 21, 3] {
        let (trace, midi) = resolve_step(&c, s);
        assert_eq!(
            (serde_json::to_value(trace).unwrap(), midi),
            expected[s as usize]
        );
    }
}
#[test]
fn polymeter_and_rotation_drive_cycle_alignment_and_round_trip() {
    let c = parse("{x ~ x x ~ x ~}%16", "").unwrap();
    assert_eq!(
        ons(&c, 16)
            .into_iter()
            .map(|(t, _)| t / 240)
            .collect::<Vec<_>>(),
        [0, 2, 3, 5, 7, 9, 10, 12, 14]
    );
    assert_eq!(
        phasecraft::music::cycle::trigger_period_ticks(&c, &c.parts[0]).unwrap(),
        Some(7 * 3840)
    );
    let copy = Composition::parse(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(ons(&c, 112), ons(&copy, 112));
    let text = toml::to_string(&c).unwrap();
    assert!(!text.contains("prepared"));
    for (rotate, tick) in [(1, 240), (-1, 3600)] {
        let source = text
            .replace("{x ~ x x ~ x ~}%16", "x ~")
            .replace("rotate = 0", &format!("rotate = {rotate}"));
        let rotated = Composition::parse(&source).unwrap();
        assert_eq!(ons(&rotated, 16), [(tick, 48)]);
        assert_eq!(
            rotated.parts[0]
                .trigger
                .rhythm
                .literal_schedule()
                .unwrap()
                .schedule()
                .next_after(tick),
            Some(tick + 3840)
        );
    }
}
#[test]
fn restart_sections_repeat_literals_and_continue_sections_keep_phase() {
    for (phase, expected) in [
        ("restart", vec![(0, 48), (7680, 48)]),
        ("continue", vec![(0, 48)]),
    ] {
        let c = parse("<x ~ ~>",&format!("[phrases.A]\n[arrangement]\nsections=[{{phrase='A',bars=2,phase='{phase}'}},{{phrase='A',bars=1,phase='{phase}'}}]")).unwrap();
        assert_eq!(ons(&c, 48), expected);
    }
}
#[test]
fn references_see_main_structure_and_admission_not_ratchet_tails() {
    let c = parse("x*3 ~", "[parts.copy]\ntrigger.rhythm={part='sub',mode='hits'}\naccent.rhythm={steps=16,pulses=0}\noutput.note=50\noutput.channel=3").unwrap();
    let notes = ons(&c, 16);
    assert_eq!(
        notes
            .iter()
            .filter(|(_, n)| *n == 50)
            .copied()
            .collect::<Vec<_>>(),
        [(0, 50)]
    );
    assert_eq!(
        notes
            .iter()
            .filter(|(_, n)| *n == 48)
            .copied()
            .collect::<Vec<_>>(),
        [(0, 48), (640, 48), (1280, 48)]
    );
}
#[test]
fn loader_refuses_ambiguous_or_unprepared_material_before_playback() {
    for (pattern, why) in [
        ("x x x x x", "off subdivision"),
        ("[x,x]", "duplicate trigger"),
        ("c1", "only hits"),
        ("x*8@0.0001 x", "at least two ticks"),
    ] {
        let error = parse(pattern, "").unwrap_err();
        assert!(error.contains(why), "{pattern}: {error}");
    }
    let c = parse("x ~", "").unwrap();
    let text = toml::to_string(&c).unwrap();
    let error = Composition::parse(&text.replace("cycle_bars = 1", "cycle_bars = 0")).unwrap_err();
    assert!(error.contains("cycle_bars"));
    let mut changed = c;
    changed.parts[0].subdivision = NoteValue::parse("1/8").unwrap();
    assert!(
        changed
            .validate()
            .unwrap_err()
            .contains("different subdivision")
    );
    changed.prepare_patterns().unwrap();
    changed.validate().unwrap();
}
#[test]
fn absolute_admission_refuses_a_span_past_the_tick_grid() {
    let p = Prepared::new("x", 1, NoteValue(STEP_TICKS), 0).unwrap();
    let tick = u64::MAX / 3840 * 3840;
    assert!(p.schedule().at(tick).is_some()); // Phase identity alone is valid.
    assert!(p.at_step(tick / STEP_TICKS).is_none()); // The absolute span cannot end.
}

#[test]
fn inherited_rhythm_kind_changes_and_literal_context_refusals_are_explicit() {
    let source = "tempo=126\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.pattern='x ~'";
    let c = Composition::parse(source).unwrap();
    assert_eq!(ons(&c, 16), [(0, 42)]);
    for (extra, why) in [
        (
            "trigger.rhythm={steps=16,pulses=1}",
            "choose trigger.pattern",
        ),
        (
            "ornaments.ratchet={count=3,probability=1.0}\n[[pins]]\nat={roll='burst',voice='sub',bar=1,slot=1}\nu=0.0",
            "no literal ratchet",
        ),
    ] {
        let error = parse("x ~", extra).unwrap_err();
        assert!(error.contains(why), "{error}");
    }
    let error = Composition::parse(&format!(
        "{source}\naccent.rhythm={{type='literal',pattern='x'}}"
    ))
    .unwrap_err();
    assert!(error.contains("literal material"), "{error}");
    let error=Composition::parse("tempo=126\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.rhythm={op='or',a={type='literal',pattern='x'},b={steps=16,pulses=0}}").unwrap_err();
    assert!(error.contains("trigger root"), "{error}");
}
