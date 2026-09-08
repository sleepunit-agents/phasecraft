use phasecraft::music::{
    notation::{Draw, Pattern},
    rhythm::literal::Schedule,
    time::NoteValue,
};

const BAR: u64 = 3840;
fn prepare(text: &str) -> Schedule {
    Schedule::prepare(text, 1, NoteValue(240), 0).unwrap()
}
fn onsets(s: &Schedule, end: u64) -> Vec<u64> {
    (0..end).filter(|&tick| s.at(tick).is_some()).collect()
}

#[test]
fn exact_onsets_keep_the_leaf_span_and_separate_hit_and_tail_draws() {
    let s = prepare("~ x ~ [x x*3?]");
    assert_eq!(onsets(&s, BAR), [960, 2880, 3360]);
    let attack = s.at(3360).unwrap();
    assert_eq!(attack.span_ticks, 480);
    assert_eq!(attack.ratchet, 3);
    assert_eq!(attack.draw, Draw::Tail);
    assert_eq!(attack.child_offset(0), Some(0));
    assert_eq!(attack.child_offset(1), Some(160));
    assert_eq!(attack.child_offset(2), Some(320));
    assert_eq!(attack.child_offset(3), None);
    assert!(s.at(3520).is_none(), "a tail is not a main onset");
    let hit = prepare("x?").at(0).unwrap();
    assert_eq!((hit.draw, hit.ratchet, hit.span_ticks), (Draw::Hit, 1, BAR));
    assert_eq!(prepare("x").at(0).unwrap().draw, Draw::None);
}

#[test]
fn rotation_delays_or_advances_the_whole_period_in_part_cells() {
    // The two cycles differ; wrapping just one cycle would pick the wrong branch.
    let text = "<x ~> ~ ~ x";
    let base = prepare(text);
    assert_eq!(base.period_ticks(), 2 * BAR);
    for rotate in [-33, -17, -1, 0, 1, 17, 33, i32::MIN, i32::MAX] {
        let shifted = Schedule::prepare(text, 1, NoteValue(240), rotate).unwrap();
        assert_eq!(shifted.period_ticks(), base.period_ticks());
        for tick in (0..3 * BAR).chain([u64::MAX - 1, u64::MAX]) {
            let source = (i128::from(tick) - i128::from(rotate) * 240)
                .rem_euclid(i128::from(base.period_ticks())) as u64;
            assert_eq!(shifted.at(tick), base.at(source), "r={rotate}, t={tick}");
        }
    }
    assert_eq!(
        onsets(
            &Schedule::prepare("x ~ ~ ~", 1, NoteValue(240), 1).unwrap(),
            BAR
        ),
        [240]
    );
}

#[test]
fn prepared_lookup_matches_notation_across_cycles_and_random_seeks() {
    for (text, bars, cell) in [
        ("x x x [x x?]", 1, 240),
        ("x ~ [~ x] [~ x?]", 1, 240),
        ("~ x ~ <x [x x*2]>", 1, 240),
        ("~ x ~ [x x*3?]", 1, 240),
        ("x@3 [~ x]", 1, 240),
        ("{x ~ x x ~ x ~}%16", 1, 240),
        ("x(5,8)", 1, 240),
        ("<x ~ x> <~ x> ~ x", 2, 120),
        ("x(8,8)", 3, 360),
        ("x*7", 1, 240),
        ("<~ <x ~>>", 1, 240),
    ] {
        let pattern = Pattern::parse(text).unwrap();
        let schedule = Schedule::prepare(text, bars, NoteValue(cell), 0).unwrap();
        let ticks = u64::from(bars) * BAR;
        let period = pattern.period_cycles().unwrap();
        assert_eq!(schedule.period_ticks(), period * ticks);
        for k in (0..period * 2).chain([999, 31, 0, 12345]) {
            let events = pattern.ticks(k, ticks).unwrap();
            for tick in k * ticks..(k + 1) * ticks {
                let expected = events.iter().find(|e| e.tick == tick);
                let found = schedule.at(tick);
                assert_eq!(found.is_some(), expected.is_some(), "{text}: {tick}");
                if let (Some(actual), Some(expected)) = (found, expected) {
                    assert_eq!(actual.span_ticks, expected.span_ticks);
                    assert_eq!(actual.ratchet, expected.ratchet);
                    assert_eq!(actual.draw, expected.draw);
                    assert_eq!(
                        (1..actual.ratchet)
                            .map(|i| tick + actual.child_offset(i).unwrap())
                            .collect::<Vec<_>>(),
                        expected.tails,
                    );
                }
            }
        }
    }
}

#[test]
fn validates_later_branches_and_period_repetition_not_just_cycle_zero() {
    for (text, cell, message) in [
        ("<x [x x x x x x x]>", 240, "off subdivision"),
        ("<x [x,x]>", 240, "duplicate trigger onset"),
        ("<x c1>", 240, "only hits"),
        ("<x 1>", 240, "only hits"),
        ("<x kick>", 240, "only hits"),
        // Tick zero is valid; 3840, in the second period, is not a dotted grid cell.
        ("x", 360, "next period"),
        // floor(3840/10001) is tick zero, but the exact rational onset is not zero.
        ("~@1 x@10000", 240, "off subdivision"),
        ("x@0.0001 ~", 240, "two ticks"),
    ] {
        let error = Schedule::prepare(text, 1, NoteValue(cell), 0).unwrap_err();
        assert!(error.contains(message), "{text}: {error}");
        assert!(
            error.contains(text),
            "error should identify the pattern: {error}"
        );
    }
    assert!(Schedule::prepare("x", 0, NoteValue(240), 0).is_err());
    assert!(Schedule::prepare("x", 1, NoteValue(0), 0).is_err());
    assert!(Schedule::prepare("x", 1, NoteValue(7), 0).is_err());
}

#[test]
fn preparation_refuses_large_period_work_and_storage_before_rendering() {
    let alt = |n| format!("<{}>", vec!["~"; n].join(" "));
    let long = format!("{} {} {}", alt(17), alt(19), alt(23));
    assert!(
        Pattern::parse(&long).is_ok(),
        "standalone grammar still accepts it"
    );
    assert!(
        Schedule::prepare(&long, 1, NoteValue(240), 0)
            .unwrap_err()
            .contains("period")
    );
    let expensive = "[~(1024,1024)](340,340), <~ ~>";
    assert!(Pattern::parse(expensive).is_ok());
    assert!(
        Schedule::prepare(expensive, 1, NoteValue(240), 0)
            .unwrap_err()
            .contains("work bound")
    );
    let large = format!("[x(64,64)](64,64), {}", alt(17));
    assert!(Pattern::parse(&large).is_ok());
    assert!(
        Schedule::prepare(&large, 1, NoteValue(240), 0)
            .unwrap_err()
            .contains("event bound")
    );
}

#[test]
fn sparse_long_cycles_and_silence_do_not_allocate_per_tick() {
    let s = Schedule::prepare("x", u32::MAX, NoteValue(240), 0).unwrap();
    assert_eq!(s.period_ticks(), u64::from(u32::MAX) * BAR);
    assert_eq!(s.at(0).unwrap().span_ticks, s.period_ticks());
    assert!(s.at(u64::MAX).is_none());
    let silent = Schedule::prepare("~", 1, NoteValue(360), i32::MIN).unwrap();
    assert!(onsets(&silent, 2 * BAR).is_empty());
    assert!(silent.at(u64::MAX).is_none());
}

#[test]
fn preparation_accepts_the_work_and_event_limits_inclusively() {
    // This silent traversal is exactly the standalone parser's 2^20 work bound.
    // It is prepared once; querying the resulting empty schedule never traverses it.
    let text = "[~(1024,1024)](341,341)";
    assert_eq!(Pattern::parse(text).unwrap().render_bounds(), (1 << 20, 0));
    let schedule = prepare(text);
    assert!(schedule.at(0).is_none());
    assert!(schedule.at(u64::MAX).is_none());

    // 4096 events per cycle * 16 cycles: exactly 65,536 entries, on a 1/64 grid.
    let text = format!("[x(64,64)](64,64), <{}>", vec!["~"; 16].join(" "));
    let schedule = Schedule::prepare(&text, 64, NoteValue(60), 0).unwrap();
    assert_eq!(schedule.period_ticks(), 64 * BAR * 16);
    for index in 0..65536 {
        assert_eq!(schedule.at(index * 60).unwrap().span_ticks, 60);
        assert!(schedule.at(index * 60 + 1).is_none());
    }
}
