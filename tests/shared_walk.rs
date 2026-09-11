use phasecraft::music::{
    Composition, STEP_TICKS,
    resolve::{Compiled, decision_roll},
    shared::{Cache, Lane},
};
const STUDY: &str = include_str!("../examples/quickstart/walk.toml");
const INTERVAL: u64 = 7 * STEP_TICKS;
fn lane(step: &str) -> Lane {
    toml::from_str(&format!(
        "start=0\nbounds=[-30,30]\ncarry='always'\nevery={{slots=7}}\nstep={step}"
    ))
    .unwrap()
}

#[test]
fn holds_start_then_steps_at_exact_boundaries_and_leans_at_both_edges() {
    for (choices, sign) in [("[20]", 1.0), ("[-20]", -1.0)] {
        let lane = lane(choices);
        lane.validate().unwrap();
        let mut cache = Cache::default();
        for (tick, expected) in [
            (0, 0.0),
            (INTERVAL - 1, 0.0),
            (INTERVAL, 20.0),
            (2 * INTERVAL - 1, 20.0),
            (2 * INTERVAL, 30.0),
            (3 * INTERVAL, 30.0),
            (4 * INTERVAL, 30.0),
        ] {
            let sample = lane.sample("drift", 91827, tick, &mut cache);
            assert_eq!(sample.value, expected * sign);
            assert_eq!(sample.occurrence, tick / INTERVAL);
            assert_eq!(sample.progress, if tick < INTERVAL { 0.0 } else { 1.0 });
            assert_eq!(sample.roll.is_some(), tick >= INTERVAL);
            assert_eq!(sample.target, sample.value);
        }
    }
}

#[test]
fn seeded_reference_matches_sparse_backward_evicted_and_rebound_reads() {
    let lane = lane("[-10,0,10,10]");
    // An independent forward accumulator: no Lane::sample or cache creates expected values.
    let reference = |name: &str, seed| {
        let mut values = vec![0.0_f64];
        for k in 1..=4200 {
            let roll = decision_roll(seed, name, "step", k * INTERVAL, "delta");
            let delta = [-10.0, 0.0, 10.0, 10.0][(roll * 4.0) as usize];
            values.push((values.last().unwrap() + delta).clamp(-30.0, 30.0));
        }
        values
    };
    let mut cache = Cache::default();
    for (name, seed) in [
        ("drift", 91827),
        ("other", 91827),
        ("drift", 2),
        ("drift", 91827),
    ] {
        let values = reference(name, seed);
        for k in [4200, 1, 4100, 0, 4000, 4199, 2, 7] {
            for offset in [0, 1, INTERVAL - 1] {
                let sample = lane.sample(name, seed, k * INTERVAL + offset, &mut cache);
                assert_eq!(sample.value, values[k as usize]);
                if k > 0 {
                    assert_eq!(sample.origin, values[k as usize - 1]);
                    assert_eq!(
                        sample.roll,
                        Some(decision_roll(seed, name, "step", k * INTERVAL, "delta"))
                    );
                }
            }
        }
    }
    let changed = self::lane("[20]");
    assert_eq!(
        changed.sample("drift", 91827, INTERVAL, &mut cache).value,
        20.0
    );
}

#[test]
fn load_refuses_bad_or_mixed_sources_and_preserves_both_wire_shapes() {
    for (old, new) in [
        ("slots = 7", "slots = 0"),
        ("slots = 7", "slots = 65537"),
        ("slots = 7", "bars = 7"),
        ("slots = 7", "slots = 7, bars = 1"),
        ("start = 0", "start = 31"),
        ("start = 0", "start = nan"),
        ("[-30, 30]", "[30, -30]"),
        ("[-30, 30]", "[0, 0]"),
        ("[-30, 30]", "[-1e308, 1e308]"),
        ("[-10, 0, 10]", "[]"),
        ("[-10, 0, 10]", "[inf]"),
        ("carry = \"always\"", "carry = \"scene\""),
        ("bounds =", "range ="),
        (
            "every = { slots = 7 }",
            "every = { slots = 7 }\ntarget = { every = { bars = 11 }, delta = [1], ramp = { bars = 4 } }",
        ),
        ("follows = \"drift\"", "follows = \"missing\""),
    ] {
        assert!(
            Composition::parse(&STUDY.replace(old, new)).is_err(),
            "accepted {new}"
        );
    }
    let choices = vec!["0"; 65].join(",");
    assert!(Composition::parse(&STUDY.replace("[-10, 0, 10]", &format!("[{choices}]"))).is_err());
    assert!(
        Composition::parse(
            &STUDY
                .replace("[-30, 30]", "[0, 1e308]")
                .replace("[-10, 0, 10]", "[1e308]")
        )
        .is_err()
    );
    let source = format!(
        "{STUDY}\n[lanes.weather]\nstart=4\nrange=[0,8]\ncarry='always'\ntarget={{every={{bars=11}},delta=[-2,0,2],ramp={{bars=4}}}}"
    );
    let c = Composition::parse(&source).unwrap();
    let roundtrip = Composition::parse(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(c.lanes, roundtrip.lanes);
    let mut a = Compiled::new(&c);
    let mut b = Compiled::new(&roundtrip);
    for step in [0, 7, 16, 207, 192, 3] {
        let (traces, messages) = a.resolve_step(step);
        let (other_traces, other_messages) = b.resolve_step(step);
        assert_eq!(messages, other_messages);
        assert_eq!(
            serde_json::to_value(traces).unwrap(),
            serde_json::to_value(other_traces).unwrap()
        );
    }
}

#[test]
fn burst_reads_subbar_updates_while_control_keeps_barline_publication() {
    let c = Composition::parse(
        &STUDY
            .replace("start = 0", "start = -30")
            .replace("[-10, 0, 10]", "[30]"),
    )
    .unwrap();
    let mut compiled = Compiled::new(&c);
    for step in 0..32 {
        let (traces, messages) = compiled.resolve_step(step);
        let value = match step {
            0..7 => -30.0,
            7..14 => 0.0,
            _ => 30.0,
        };
        assert_eq!(traces[0].lanes[0].value, value);
        let snare = traces.iter().find(|t| t.part == "snare").unwrap();
        let probability = snare
            .ornaments
            .as_ref()
            .unwrap()
            .ratchet
            .as_ref()
            .unwrap()
            .probability;
        assert_eq!(probability, (value + 30.0) / 60.0);
        let notes = messages
            .iter()
            .filter(|m| m.bytes[0] == 0x99 && m.bytes[1] == 38)
            .count();
        if step < 7 {
            assert_eq!(notes, 1);
        }
        if step >= 14 {
            assert_eq!(notes, 2);
        }
        let cc: Vec<_> = messages.iter().filter(|m| m.parameter).collect();
        if step % 16 == 0 {
            assert_eq!(cc.len(), 1);
            assert_eq!(cc[0].bytes, [0xbe, 76, if step == 0 { 25 } else { 102 }]);
            assert!(
                messages.iter().position(|m| m.parameter)
                    < messages.iter().position(|m| m.bytes[0] == 0x99)
            );
        } else {
            assert!(cc.is_empty());
        }
    }
}

#[test]
fn absent_readers_and_restarted_sections_keep_root_seed_and_clock() {
    let source = format!(
        "{STUDY}\n[scenes.A]\n[scenes.B]\nseed=3\n[arrangement]\nrepeat=true\nsections=[{{phrase='A',bars=1}},{{phrase='B',bars=1}}]"
    );
    let mut c = Composition::parse(&source).unwrap();
    c.arrangement.as_mut().unwrap().sections[1]
        .composition
        .parts
        .retain(|p| p.id != "snare");
    c.validate().unwrap();
    let mut forward = Compiled::new(&c);
    let mut baseline = Compiled::new(&Composition::parse(STUDY).unwrap());
    let mut expected = Vec::new();
    for step in 0..100 {
        let (traces, messages) = forward.resolve_step(step);
        assert_eq!(
            serde_json::to_value(&traces[0].lanes).unwrap(),
            serde_json::to_value(&baseline.resolve_step(step).0[0].lanes).unwrap()
        );
        if (step / 16) % 2 == 1 {
            assert!(traces.iter().all(|t| t.part != "snare"));
        }
        expected.push((serde_json::to_value(traces).unwrap(), messages));
    }
    let mut seek = Compiled::new(&c);
    for step in [99, 16, 32, 0, 31, 33, 7] {
        let (traces, messages) = seek.resolve_step(step);
        assert_eq!(
            (serde_json::to_value(traces).unwrap(), messages),
            expected[step as usize]
        );
    }
}

#[test]
fn return_group_uses_walk_clock_not_value_repetition_and_router_keeps_it_phased() {
    use phasecraft::music::router::{Clock, member_clock};
    let source = format!("{STUDY}\n[returns.hat_system]\nalign=['voices.hat.trigger.cycle','lanes.drift.every']\n[scenes.A]\n[scenes.B]\nseed=3\n[router]\nevery={{returns='hat_system'}}\n[router.routes]\nA={{B=1.0}}\nB={{A=1.0}}").replacen("tempo = 126", "start='A'\ntempo = 126", 1);
    let c = Composition::parse(&source).unwrap();
    assert_eq!(
        member_clock(&c, "lanes.drift.every").unwrap(),
        Clock::Fixed(INTERVAL)
    );
    assert_eq!(
        c.router.as_ref().unwrap().period_ticks(&c).unwrap(),
        112 * STEP_TICKS
    );
    for key in [
        "lanes.missing.every",
        "lanes.drift.cycle",
        "lanes.drift.step",
    ] {
        assert!(member_clock(&c, key).unwrap_err().contains(key));
    }
    let weather = Composition::parse(include_str!("../examples/quickstart/weather.toml")).unwrap();
    assert!(
        member_clock(&weather, "lanes.weather.every")
            .unwrap_err()
            .contains("target lanes")
    );
    let mut routed = Compiled::new(&c);
    let mut plain = Compiled::new(&Composition::parse(STUDY).unwrap());
    for step in [111, 112, 113, 224, 7, 225] {
        let actual = routed.resolve_step(step).0;
        let expected = plain.resolve_step(step).0;
        assert_eq!(
            serde_json::to_value(&actual[0].lanes).unwrap(),
            serde_json::to_value(&expected[0].lanes).unwrap()
        );
    }
    let moves = routed.moves_through(224 * STEP_TICKS);
    assert_eq!(moves.len(), 2);
}
