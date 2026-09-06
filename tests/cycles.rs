use phasecraft::music::{Composition, resolve::realize};
#[test]
fn tiny_window_carries_long_cycle_without_expanding_it() {
    let c = Composition::parse(
        r#"
        tempo=132
        seed=1
        [parts.hat]
        use="techno.closed_hat"
        trigger.rhythm={op="xor",a={steps=16,pulses=7},b={steps=5,pulses=2}}
        accent.rhythm={steps=7,pulses=3}
    "#,
    )
    .unwrap();
    let p = realize(&c, &c.parts[0], 0, 4);
    assert!(p.events.len() <= 4);
    assert_eq!(p.cycles.len(), 1);
    assert_eq!(p.cycles[0].phase_alignment_steps, Some(560));
    assert_eq!(p.cycles[0].phrase_steps, 64);
    assert_eq!(p.cycles[0].end_tick, 960);
}
#[test]
fn references_shared_lanes_and_reset_policies_contribute_phase_alignment() {
    let c = Composition::parse(
        r#"
        tempo=132
        seed=1
        phrase_bars=1
        [accents.group]
        rhythm={steps=7,pulses=3,reset_on_phrase=true}
        [parts.kick]
        use="techno.kick"
        trigger.rhythm={steps=5,pulses=2}
        [parts.hat]
        use="techno.closed_hat"
        trigger.rhythm={part="kick",mode="hits"}
        accent.rhythm={steps=3,pulses=1}
        accent.sources=["group"]
        [phrases.A]
        [arrangement]
        sections=[{phrase="A",bars=1},{phrase="A",bars=1}]
    "#,
    )
    .unwrap();
    let hat = c.parts.iter().find(|p| p.id == "hat").unwrap();
    let p = realize(&c, hat, 8, 40);
    assert_eq!(p.cycles.len(), 2);
    assert_eq!(p.cycles[0].phase_alignment_steps, Some(240));
    assert_eq!(p.cycles[0].phase_origin_tick, 0);
    assert_eq!(p.cycles[1].phase_origin_tick, 3840);
    assert_eq!(p.cycles[1].end_tick, 7680);
}
#[test]
fn enormous_common_period_is_unknown_without_overflow_or_allocation() {
    let mut expression = "{steps=65521,pulses=1}".to_string();
    for n in [65519, 65497, 65479, 65449] {
        expression = format!("{{op='or',a={expression},b={{steps={n},pulses=1}}}}");
    }
    let c = Composition::parse(&format!(
        "tempo=132\nseed=1\n[parts.hat]\nuse='techno.closed_hat'\ntrigger.rhythm={expression}"
    ))
    .unwrap();
    let p = realize(&c, &c.parts[0], 0, 1);
    assert_eq!(p.cycles[0].phase_alignment_steps, None);
}
