use phasecraft::music::{
    Composition,
    resolve::Compiled,
    shared::{Cache, Lane},
};
const BAR: u64 = 3840;
const STUDY: &str = include_str!("../examples/quickstart/weather.toml");
fn lane(delta: &str) -> Lane {
    toml::from_str(&format!("start=4\nrange=[0,8]\ncarry='always'\ntarget={{every={{bars=11}},delta={delta},ramp={{bars=4}}}}" )).unwrap()
}
#[test]
fn decision_bar_holds_four_next_bar_increments_then_settles_and_clamps() {
    let lane = lane("[2]");
    lane.validate().unwrap();
    let mut cache = Cache::default();
    for (bar, w) in [
        (1, 4.0),
        (12, 4.0),
        (13, 4.5),
        (14, 5.0),
        (15, 5.5),
        (16, 6.0),
        (23, 6.0),
        (27, 8.0),
        (34, 8.0),
        (38, 8.0),
    ] {
        for offset in [0, 1, 3839] {
            assert_eq!(
                lane.sample("weather", 91827, (bar - 1) * BAR + offset, &mut cache)
                    .value,
                w
            );
        }
    }
}
#[test]
fn all_readers_use_one_value_and_controls_only_publish_before_barline_notes() {
    for (w, hat, snare, p) in [(0, 62, 89, 0.02), (4, 89, 127, 0.135), (8, 116, 127, 0.25)] {
        let c = Composition::parse(&STUDY.replace("start = 4", &format!("start = {w}"))).unwrap();
        let mut compiled = Compiled::new(&c);
        let (t, m) = compiled.resolve_step(0);
        assert_eq!(t[0].lanes[0].value, w as f64);
        let cc: Vec<_> = m
            .iter()
            .filter(|m| m.bytes[0] & 0xf0 == 0xb0)
            .map(|m| m.bytes)
            .collect();
        assert_eq!(cc, vec![[0xbe, 38, snare], [0xbe, 76, hat]]);
        assert!(
            m.iter().position(|m| m.bytes[0] & 0xf0 == 0xb0)
                < m.iter().position(|m| m.bytes[0] & 0xf0 == 0x90)
        );
        for s in 1..16 {
            assert!(
                compiled
                    .resolve_step(s)
                    .1
                    .iter()
                    .all(|m| m.bytes[0] & 0xf0 != 0xb0)
            );
        }
        let trace = compiled
            .resolve_step(14)
            .0
            .into_iter()
            .find(|t| t.part == "snare")
            .unwrap();
        assert!((trace.ornaments.unwrap().ratchet.unwrap().probability - p).abs() < 1e-12);
    }
}
#[test]
fn explicit_gate_controls_plain_and_question_bursts_without_removing_main() {
    for notation in ["x*3", "x*3?"] {
        for (probability, notes) in [(0, 1), (1, 3)] {
            let source = STUDY.replace("~ x ~ [x x*3?]", notation).replace(
                "low = 0.02, high = 0.25",
                &format!("low = {probability}, high = {probability}"),
            );
            let c = Composition::parse(&source).unwrap();
            let mut compiled = Compiled::new(&c);
            let events: Vec<_> = (0..16)
                .flat_map(|s| compiled.resolve_step(s).1)
                .filter(|m| m.bytes[0] & 0xf0 == 0x90 && m.bytes[1] == 38)
                .collect();
            assert_eq!(events.len(), notes);
            let t = compiled
                .resolve_step(0)
                .0
                .into_iter()
                .find(|t| t.part == "snare")
                .unwrap();
            assert!(t.event.is_some());
            assert_eq!(t.values.len(), notes);
        }
    }
}
#[test]
fn invalid_sources_and_readers_fail_load_and_roundtrip_preserves_followers() {
    let c = Composition::parse(STUDY).unwrap();
    let copy = Composition::parse(&toml::to_string(&c).unwrap()).unwrap();
    let mut a = Compiled::new(&c);
    let mut b = Compiled::new(&copy);
    for step in [0, 192, 206, 416, 0] {
        assert_eq!(a.resolve_step(step).1, b.resolve_step(step).1);
    }
    for (old, new) in [
        ("range = [0, 8]", "range = [8, 0]"),
        ("bars = 4", "bars = 11"),
        ("delta = [-2, -1, 0, 1, 2]", "delta = []"),
        ("follows = \"weather\"", "follows = \"missing\""),
        ("high = 0.25", "high = 1.1"),
        ("high = 1.30, unclipped", "high = 1.50, unclipped"),
        ("default = 0.7007874015748031", "bogus = 1"),
        (
            "parameters.decay = { follows",
            "parameters.decay = { value = 0.2, follows",
        ),
    ] {
        assert!(
            Composition::parse(&STUDY.replace(old, new)).is_err(),
            "accepted {new}"
        );
    }
}
#[test]
fn distant_and_backwards_reads_replay_evicted_history_exactly() {
    let lane = lane("[-2,-1,0,1,2]");
    let mut cache = Cache::default();
    let mut expected = Vec::new();
    for k in 0..=4200 {
        expected.push(
            lane.sample("weather", 91827, (k * 11 + 4) * BAR, &mut cache)
                .value,
        );
    }
    for k in [4200, 1, 4100, 0, 4000, 4199] {
        let tick = (k * 11 + 4) * BAR;
        assert_eq!(
            lane.sample("weather", 91827, tick, &mut cache).value,
            expected[k as usize]
        );
        assert_eq!(
            lane.sample("weather", 91827, tick, &mut Cache::default())
                .value,
            expected[k as usize]
        );
    }
}
#[test]
fn scene_absence_and_section_restarts_keep_transport_weather() {
    let source = format!(
        "{STUDY}\n[scenes.A]\n[scenes.B]\nseed=3\n[arrangement]\nrepeat=true\nsections=[{{phrase='A',bars=13}},{{phrase='B',bars=13}}]"
    );
    let mut c = Composition::parse(&source).unwrap();
    // Runtime expanded B omits snare. The root lane still advances independently.
    c.arrangement.as_mut().unwrap().sections[1]
        .composition
        .parts
        .retain(|p| p.id != "snare");
    c.validate().unwrap();
    let mut forward = Compiled::new(&c);
    let mut baseline = Compiled::new(&Composition::parse(STUDY).unwrap());
    let mut expected = Vec::new();
    for s in 0..900 {
        let (t, m) = forward.resolve_step(s);
        assert_eq!(
            t[0].lanes[0].value,
            baseline.resolve_step(s).0[0].lanes[0].value
        );
        if (208..416).contains(&s) {
            assert!(m.iter().all(|m| !(m.parameter && m.bytes[1] == 38)));
        }
        expected.push((serde_json::to_value(t).unwrap(), m));
    }
    let mut seek = Compiled::new(&c);
    for s in [416, 208, 0, 899, 207, 416] {
        let (t, m) = seek.resolve_step(s);
        assert_eq!((serde_json::to_value(t).unwrap(), m), expected[s as usize]);
    }
}

#[test]
fn continuous_burst_draw_uses_transport_onset_even_when_section_restarts() {
    let source = format!(
        "{STUDY}\n[scenes.A]\n[arrangement]\nrepeat=true\nsections=[{{phrase='A',bars=1}}]"
    );
    let c = Composition::parse(&source).unwrap();
    let mut routed = Compiled::new(&c);
    let mut plain = Compiled::new(&Composition::parse(STUDY).unwrap());
    for step in (14..800).step_by(16) {
        let a = routed
            .resolve_step(step)
            .0
            .into_iter()
            .find(|t| t.part == "snare")
            .unwrap();
        let b = plain
            .resolve_step(step)
            .0
            .into_iter()
            .find(|t| t.part == "snare")
            .unwrap();
        assert_eq!(
            serde_json::to_value(a.ornaments).unwrap(),
            serde_json::to_value(b.ornaments).unwrap()
        );
        assert_eq!(a.values.len(), b.values.len());
    }
}

#[test]
fn report_names_saturation_and_shared_target_collision_is_rejected() {
    let c = Composition::parse(STUDY).unwrap();
    let rows = phasecraft::music::shared::report(&c);
    assert!(
        rows.iter()
            .any(|s| s.contains("snare.decay") && s.contains("saturates"))
    );
    assert!(
        rows.iter()
            .any(|s| s.contains("hat.decay") && s.contains("unclipped"))
    );
    assert!(
        rows.iter()
            .any(|s| s.contains("2 followed CC candidates/bar"))
    );
    assert!(
        Composition::parse(&STUDY.replace("cc = 38, channel = 15", "cc = 76, channel = 15"))
            .is_err()
    );
}

#[test]
fn transport_router_keeps_weather_through_absence_and_same_tick_reset_precedes_incoming() {
    let source = format!(
        "{STUDY}\n[scenes.A]\n[scenes.B]\n[router]\nevery={{returns='bar'}}\n[router.routes]\nA={{B=1.0}}\nB={{A=1.0}}\n[returns.bar]\nalign=['voices.hat.trigger.cycle']\n"
    );
    let source = source.replacen("tempo = 126", "start='A'\ntempo = 126", 1);
    let mut c = Composition::parse(&source).unwrap();
    c.router.as_mut().unwrap().scenes[1]
        .composition
        .parts
        .retain(|p| p.id != "snare");
    c.validate().unwrap();
    let mut forward = Compiled::new(&c);
    let mut expected = Vec::new();
    for step in 0..450 {
        let (t, m) = forward.resolve_step(step);
        if step == 192 {
            // A re-entry at bar 13: first ramp increment under the actual hash.
            let target: Vec<_> = m
                .iter()
                .filter(|m| m.bytes[0] == 0xbe && m.bytes[1] == 76)
                .collect();
            assert_eq!(target.len(), 2);
            assert!(target[0].boundary_reset);
            assert!(target[1].parameter);
        }
        expected.push((serde_json::to_value(t).unwrap(), m));
    }
    let mut seek = Compiled::new(&c);
    for step in [448, 192, 16, 0, 208, 192] {
        let (t, m) = seek.resolve_step(step);
        assert_eq!(
            (serde_json::to_value(t).unwrap(), m),
            expected[step as usize]
        );
    }
}

#[test]
fn wire_deduplicates_held_bars_and_stop_restores_both_kit_defaults() {
    use phasecraft::playback::{EventDispatcher, MidiOutput};
    use std::time::Duration;
    #[derive(Default)]
    struct Recording(Vec<Vec<u8>>);
    impl MidiOutput for Recording {
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.0.push(bytes.to_vec());
            Ok(())
        }
    }
    let c =
        Composition::parse(&STUDY.replace("delta = [-2, -1, 0, 1, 2]", "delta = [-2]")).unwrap();
    let mut compiled = Compiled::new(&c);
    let mut dispatcher = EventDispatcher::new(Recording::default());
    for step in 0..272 {
        for event in compiled.resolve_step(step).1 {
            dispatcher
                .dispatch(&event, Duration::ZERO, Duration::ZERO)
                .unwrap();
        }
    }
    for (cc, expected) in [
        (76, vec![89, 86, 82, 79, 76]),
        (38, vec![127, 122, 117, 113, 108]),
    ] {
        let values: Vec<_> = dispatcher
            .sink
            .0
            .iter()
            .filter(|b| b[0] == 0xbe && b[1] == cc)
            .map(|b| b[2])
            .collect();
        assert_eq!(values, expected);
    }
    dispatcher.cleanup().unwrap();
    for (cc, value) in [(76, 89), (38, 127)] {
        assert_eq!(
            dispatcher
                .sink
                .0
                .iter()
                .rev()
                .find(|b| b[0] == 0xbe && b[1] == cc)
                .unwrap()[2],
            value
        );
    }
}
