use phasecraft::music::{Composition, resolve::Compiled};

fn parse(pattern: &str, value: &str, extra: &str) -> Result<Composition, String> {
    Composition::parse(&format!(
        r#"
tempo=126
seed=91827
[parts.hat]
trigger.pattern={pattern:?}
accent.rhythm={{steps=16,pulses=0}}
profile.base=100
output.note=42
output.channel=10
velocity={{ {value} }}
{extra}
"#
    ))
}
fn render(c: &Composition, steps: u64) -> Vec<(u64, u8)> {
    let mut compiled = Compiled::new(c);
    (0..steps)
        .flat_map(|s| compiled.resolve_step(s).1)
        .filter(|m| m.bytes[0] & 0xf0 == 0x90)
        .map(|m| (m.tick, m.bytes[2]))
        .collect()
}
#[test]
fn exposed_snare_spends_on_each_structural_attack_and_resets() {
    for (roll, expected) in [(0.0, vec![95, 60, 95, 60, 95]), (0.99, vec![95, 60, 95])] {
        let c = parse("~ x ~ [x x*3?]", "pattern='0.95 0.6',per='event',clock='attacks'", &format!("[[pins]]\nat={{roll='burst',voice='hat',bar=1,slot=15}}\nu={roll}\n[[pins]]\nat={{roll='burst',voice='hat',bar=2,slot=15}}\nu={roll}")).unwrap();
        assert_eq!(
            render(&c, 32).iter().map(|x| x.1).collect::<Vec<_>>(),
            expected.repeat(2)
        );
    }
}
#[test]
fn main_clock_children_inherit_but_time_clock_reads_written_position() {
    let c = parse(
        "~ x ~ [x x*3]",
        "pattern='0.95 0.6',per='event',clock='main'",
        "",
    )
    .unwrap();
    assert_eq!(
        render(&c, 16).iter().map(|x| x.1).collect::<Vec<_>>(),
        [95, 60, 95, 95, 95]
    );
    let c = parse("x*4", "pattern='1 0.8 0.6 0.4'", "groove.delay_ticks=24").unwrap();
    assert_eq!(
        render(&c, 16),
        [(24, 100), (984, 80), (1944, 60), (2904, 40)]
    );
}
#[test]
fn later_suppression_does_not_refund_a_carried_value() {
    let c = parse(
        "~ ~ ~ [~ ~ ~ x*3]",
        "pattern='1 0.9 0.8 0.7 0.6 0.5 0.4',per='event',carry='always'",
        "groove.delay_ticks=60\ngroove.swing=0.75",
    )
    .unwrap();
    let mut compiled = Compiled::new(&c);
    let t = compiled.resolve_step(15).0.remove(0);
    assert_eq!(
        t.values.iter().map(|v| v.index).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(
        t.ornaments
            .as_ref()
            .unwrap()
            .ratchet
            .as_ref()
            .unwrap()
            .emitted_count
            < 3
    );
    let t = compiled.resolve_step(31).0.remove(0);
    assert_eq!(
        t.values.iter().map(|v| v.index).collect::<Vec<_>>(),
        [3, 4, 5]
    );
}
#[test]
fn admitted_grace_spends_last_even_when_it_sounds_first_or_is_suppressed() {
    let c = parse(
        "x ~",
        "pattern='1 0.8 0.6 0.4 0.2',per='event',carry='always'",
        "ornaments.flam={spacing='1/32',probability=1.0,gain=1.0}",
    )
    .unwrap();
    let mut compiled = Compiled::new(&c);
    let t = compiled.resolve_step(0).0.remove(0);
    assert_eq!(t.values.iter().map(|v| v.index).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(t.extra_events.len(), 0); // Lower-bound suppression still spent child 1.
    let t = compiled.resolve_step(16).0.remove(0);
    assert_eq!(t.values.iter().map(|v| v.index).collect::<Vec<_>>(), [2, 3]);
    let c = parse(
        "~ x",
        "pattern='1 0.8 0.6 0.4 0.2',per='event',carry='always'",
        "ornaments.flam={spacing='1/32',probability=1.0,gain=1.0}",
    )
    .unwrap();
    assert_eq!(render(&c, 16), [(1800, 80), (1920, 100)]);
    let combined = parse(
        "~ x*3",
        "pattern='1 0.8 0.6 0.4 0.2',per='event',carry='always'",
        "ornaments.flam={spacing='1/32',probability=1.0,gain=1.0}",
    )
    .unwrap();
    assert_eq!(
        render(&combined, 16),
        [(1800, 40), (1920, 100), (2560, 80), (3200, 60)]
    );
    let mut compiled = Compiled::new(&combined);
    assert_eq!(
        compiled.resolve_step(8).0[0]
            .values
            .iter()
            .map(|v| v.index)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
}
#[test]
fn seeks_and_evicted_source_cells_keep_the_carried_residue() {
    let c = parse(
        "x? ~ x ~",
        "pattern='1 0.62 0.78 0.55 0.9 0.6 0.7',per='event',carry='always'",
        "trigger.probability=0.35\ntrigger.probability_mode='continuous'",
    )
    .unwrap();
    let mut forward = Compiled::new(&c);
    let mut expected = std::collections::BTreeMap::new();
    for step in 0..=5000 {
        let value = forward.resolve_step(step);
        if [0, 8, 16, 4992, 5000].contains(&step) {
            expected.insert(step, (serde_json::to_value(value.0).unwrap(), value.1));
        }
    }
    let mut seek = Compiled::new(&c);
    for step in [5000, 16, 4992, 0, 8, 5000] {
        let value = seek.resolve_step(step);
        assert_eq!(
            (serde_json::to_value(value.0).unwrap(), value.1),
            expected[&step],
            "step {step}"
        );
    }
    // Independent count of admitted SOURCE opportunities; plain x always admits.
    let mut plain = c.clone();
    plain.parts[0].velocity = None;
    let mut structural = Compiled::new(&plain);
    let count: usize = (0..5000)
        .map(|s| {
            structural
                .resolve_step(s)
                .0
                .iter()
                .filter(|t| t.trigger.admitted)
                .count()
        })
        .sum();
    let values = seek.resolve_step(5000).0.remove(0).values;
    if !values.is_empty() {
        assert_eq!(values[0].index, count % 7);
    }
}
#[test]
fn carried_values_cross_restart_continue_and_absent_sections() {
    let mut c = parse(
        "x x x ~",
        "pattern='1 0.62 0.78 0.55 0.9 0.6 0.7',per='event',carry='always'",
        "",
    )
    .unwrap();
    use phasecraft::music::arrangement::{Arrangement, PhasePolicy, Section};
    let active = c.clone();
    let mut absent = parse("~", "pattern='1'", "").unwrap();
    absent.parts[0].id = "other".into(); // The hat is absent; another silent Part remains.
    c.arrangement = Some(Arrangement {
        repeat: true,
        sections: vec![
            Section {
                phrase: "A".into(),
                bars: 1,
                phase: PhasePolicy::Restart,
                composition: Box::new(active.clone()),
            },
            Section {
                phrase: "absent".into(),
                bars: 1,
                phase: PhasePolicy::Continue,
                composition: Box::new(absent),
            },
            Section {
                phrase: "B".into(),
                bars: 1,
                phase: PhasePolicy::Continue,
                composition: Box::new(active),
            },
        ],
    });
    c.validate().unwrap();
    let expected = [100, 62, 78, 55, 90, 60, 70, 100, 62, 78, 55, 90];
    assert_eq!(
        render(&c, 96).iter().map(|x| x.1).collect::<Vec<_>>(),
        expected
    );
    let mut seek = Compiled::new(&c);
    for (step, index) in [(32, 3), (80, 2), (0, 0), (48, 6), (32, 3)] {
        let t = seek.resolve_step(step).0.remove(0);
        assert_eq!(t.values[0].index, index);
    }
    c.arrangement.as_mut().unwrap().sections[2]
        .composition
        .parts[0]
        .velocity = None;
    assert!(c.validate().unwrap_err().contains("carried velocity"));
}
#[test]
fn router_moves_and_stays_keep_one_history_per_voice() {
    let c = Composition::parse(
        r#"
tempo=126
seed=91827
start='A'
[parts.hat]
use='techno.closed_hat'
trigger.pattern='x x x ~'
velocity={pattern='1 0.62 0.78 0.55 0.9 0.6 0.7',per='event',carry='always'}
[scenes]
A={}
B={}
[router]
every={returns='bar'}
[router.routes]
A={B=1.0}
B={A=1.0}
[returns.bar]
align=['voices.hat.trigger.cycle']
"#,
    )
    .unwrap();
    let mut forward = Compiled::new(&c);
    let expected: Vec<_> = (0..64)
        .map(|s| {
            let (t, m) = forward.resolve_step(s);
            (serde_json::to_value(t).unwrap(), m)
        })
        .collect();
    let mut seek = Compiled::new(&c);
    for s in [48, 32, 0, 16, 48] {
        let (t, m) = seek.resolve_step(s);
        assert_eq!(t[0].values[0].index, (s as usize / 16 * 3) % 7);
        assert_eq!((serde_json::to_value(t).unwrap(), m), expected[s as usize]);
    }
    let mut stays = c.clone();
    stays.router.as_mut().unwrap().routes.insert(
        "B".into(),
        [("B".into(), phasecraft::music::router::Weight::Fixed(1.0))].into(),
    );
    stays.validate().unwrap();
    let mut stayed = Compiled::new(&stays);
    assert_eq!(stayed.resolve_step(48).0[0].values[0].index, 2);
    let mut absent = c.clone();
    let b = &mut absent.router.as_mut().unwrap().scenes[1].composition.parts[0];
    b.id = "other".into();
    b.velocity = None;
    absent.validate().unwrap();
    let mut returned = Compiled::new(&absent);
    assert_eq!(returned.resolve_step(32).0[0].values[0].index, 3);
}
#[test]
fn loader_refuses_unsupported_shapes_and_conflicting_clocks() {
    for value in [
        "pattern=''",
        "pattern='NaN'",
        "pattern='1 1.1'",
        "pattern='1 ~'",
        "pattern='[1 0.5]'",
        "pattern='1',cycle=0",
        "pattern='1',clock='attacks'",
        "pattern='1',carry='always'",
        "pattern='1',per='event',clock='emitted'",
    ] {
        assert!(parse("x", value, "").is_err(), "{value}");
    }
    let c = parse("x ~", "pattern='1 0.6',per='event',cycle=32", "").unwrap();
    assert_eq!(c.parts[0].velocity.as_ref().unwrap().cycle_ticks(), 7680);
    let copy = Composition::parse(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(render(&c, 64), render(&copy, 64));
    assert!(
        parse("x", "pattern='1',cycle=16", "trigger.cycle=16")
            .unwrap_err()
            .contains("unknown field")
    );
}

#[test]
fn coincident_attacks_spend_before_the_strongest_is_chosen() {
    // The second source's anticipated grace coincides with the first main at tick zero.
    // The first source's own grace is suppressed, but all four structural attacks spend.
    let c = parse(
        &vec!["x"; 32].join(" "),
        "pattern='0.2 0.9 0.4 0.6 0.8',per='event',carry='always'",
        "subdivision='1/32'\ngroove.delay_ticks=-30\nornaments.flam={spacing='1/64.',gain=1.0}",
    )
    .unwrap();
    let mut compiled = Compiled::new(&c);
    let (traces, midi) = compiled.resolve_step(0);
    assert_eq!(
        midi.iter()
            .find(|m| m.tick == 0 && m.bytes[0] & 0xf0 == 0x90)
            .unwrap()
            .bytes[2],
        60
    );
    assert_eq!(
        traces
            .iter()
            .find(|t| t.tick == 120)
            .unwrap()
            .values
            .iter()
            .map(|v| v.index)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(compiled.resolve_step(1).0[0].values[0].index, 4);
}
#[test]
fn dense_carried_prefix_eviction_and_nondefault_cycles_are_exact() {
    let c = parse(
        &vec!["x"; 16].join(" "),
        "pattern='1 0.9 0.8 0.7 0.6 0.5 0.4',per='event',carry='always'",
        "",
    )
    .unwrap();
    let mut compiled = Compiled::new(&c);
    for s in 0..5000 {
        compiled.resolve_step(s);
    }
    for s in [0, 4900, 16, 5000] {
        assert_eq!(
            compiled.resolve_step(s).0[0].values[0].index,
            s as usize % 7
        );
    }
    let c = parse(
        &vec!["x"; 16].join(" "),
        "pattern='1 0.9 0.8 0.7 0.6',per='event',cycle=7",
        "",
    )
    .unwrap();
    let mut compiled = Compiled::new(&c);
    for s in [0, 6, 7, 8, 14, 15] {
        assert_eq!(
            compiled.resolve_step(s).0[0].values[0].index,
            (s as usize % 7) % 5
        );
    }
}
#[test]
fn value_clock_periods_distinguish_time_from_events() {
    use phasecraft::music::router::{Clock, member_clock};
    let timed = parse("x", "pattern='1 0.5',cycle=7", "").unwrap();
    assert!(matches!(
        member_clock(&timed, "voices.hat.velocity.cycle").unwrap(),
        Clock::Fixed(1680)
    ));
    let event = parse("x", "pattern='1 0.5',per='event',cycle=7", "").unwrap();
    assert!(matches!(
        member_clock(&event, "voices.hat.velocity.cycle").unwrap(),
        Clock::Event
    ));
}
