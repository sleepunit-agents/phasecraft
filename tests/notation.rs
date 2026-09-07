//! Every step-notation string used by the two pieces the literal leaf is for, parsed for real.
use phasecraft::music::notation::{Pattern, TokenKinds};

/// Every pattern string in sleepunit-agents/until-stop's two pieces.
const PIECE_PATTERNS: &[&str] = &[
    "x x x [x x?]",
    "x ~ [~ x] [~ x?]",
    "~ x ~ <x [x x*2]>",
    "~ x ~ [x x*3?]",
    "x@3 [~ x]",
    "x@4",
    "<x ~ ~> ~ ~ ~",
    "{x ~ x x ~ x ~}%16",
    "x ~ ~ [~ x]",
    "x ~ [~ x] ~",
    "~ x ~ x",
    "<x ~>",
    "x ~ ~ ~",
    "x(5,8)",
    "1 0.9 0.95 0.9",
    "0.95 0.6",
    "0 10 -5 12.5 2.5",
    "0 -5 -10",
    "<0 0 -5 7>",
    "1.0 0.62 0.78 0.55 0.9 0.6 0.7",
    "<c1 eb1>",
    "<c1 c1 eb1 bb0>",
    "c1",
    "<1 1 2 1>",
    "[1,3,5,7,9]",
    "1",
    "<[1 1 1 8 1 1 7 1 1 5 1 1 6 1 3 1] [1 1 1 8 1 1 7 1 1 5 1 1 6 1 3 1] [1 1 1 8 1 1 7 1 1 5 1 1 6 1 3 1] [1 1 1 8 1 1 5 1 1 3 1 1 6 1 7 1]>",
    "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~",
    "1 2 3 ~ 5 ~ 7 8 1 ~ 3 4 ~ 6 7 ~",
    "rifle ~ pistol pistol",
    "rifle ~ rifle",
    "~ pistol*2",
    "3 ~ 3*4",
    "9 9 9 [9 9]",
    "[5 5] 6*4",
    "~ 12*6",
];

#[test]
fn every_pattern_the_pieces_use_parses() {
    for text in PIECE_PATTERNS {
        Pattern::parse(text).unwrap_or_else(|e| panic!("{text:?} failed to parse: {e}"));
    }
}

#[test]
fn every_pattern_renders_a_finite_cycle_and_a_tick_grid() {
    for text in PIECE_PATTERNS {
        let p = Pattern::parse(text).expect("parsed above");
        let period = p.period_cycles().expect("a piece pattern has a period");
        assert!(period >= 1, "{text:?}");
        for k in 0..period.min(8) {
            let events = p.cycle(k);
            let ticks = p.ticks(k, 3840).unwrap();
            assert_eq!(events.len(), ticks.len(), "{text:?} cycle {k}");
            // onsets are sorted, inside the cycle, and land on the grid the spec floors to
            let mut previous = None;
            for (event, tick) in events.iter().zip(&ticks) {
                assert!(
                    event.onset < phasecraft::music::notation::Ratio::ONE,
                    "{text:?}"
                );
                assert!(Some(event.onset) >= previous, "{text:?} is unsorted");
                previous = Some(event.onset);
                assert_eq!(tick.tick, k * 3840 + event.onset.ticks(3840), "{text:?}");
                assert_eq!(tick.tails.len(), usize::from(event.ratchet) - 1, "{text:?}");
            }
        }
    }
}

#[test]
fn kinds_let_a_consumer_refuse_a_mismatch() {
    let only = |text: &str| Pattern::parse(text).expect("parses").kinds();
    assert_eq!(
        only("rifle ~ rifle"),
        TokenKinds {
            names: true,
            ..TokenKinds::default()
        }
    );
    assert_eq!(
        only("<c1 eb1>"),
        TokenKinds {
            notes: true,
            ..TokenKinds::default()
        }
    );
    assert_eq!(
        only("x x x [x x?]"),
        TokenKinds {
            hits: true,
            ..TokenKinds::default()
        }
    );
    assert_eq!(
        only("0 10 -5 12.5 2.5"),
        TokenKinds {
            numbers: true,
            ..TokenKinds::default()
        }
    );
    assert_eq!(
        only("~ pistol*2"),
        TokenKinds {
            names: true,
            ..TokenKinds::default()
        }
    );
}

#[test]
fn the_sixteen_slot_slice_row_places_every_index() {
    let p = Pattern::parse("1 2 3 ~ 5 ~ 7 8 1 ~ 3 4 ~ 6 7 ~").expect("parses");
    let ticks: Vec<u64> = p
        .ticks(0, 3840)
        .unwrap()
        .into_iter()
        .map(|e| e.tick)
        .collect();
    assert_eq!(
        ticks,
        vec![0, 240, 480, 960, 1440, 1680, 1920, 2400, 2640, 3120, 3360]
    );
}
