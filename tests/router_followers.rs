//! Shared sources and routing must agree at the same absolute transport instant.
use phasecraft::music::{Composition, STEP_TICKS, resolve::Compiled, router, shared::Cache};

const BAR: u64 = 16 * STEP_TICKS;

fn piece(lane: &str, rows: &str) -> String {
    format!(
        r#"
tempo = 126
seed = 91827
start = "a"
[lanes.motion]
{lane}
[parts.hat]
use = "techno.closed_hat"
trigger.rhythm = {{ steps = 16, pulses = 4 }}
[scenes.a]
[scenes.b]
seed = 4471
parts.hat.trigger.probability = 0.25
[returns.bar]
align = ["voices.hat.trigger.cycle"]
[router]
every = {{ returns = "bar" }}
[router.routes]
{rows}
"#
    )
}

const COMPLEMENTARY: &str = r#"
a = { a = { follows = "motion", low = 1.0, high = 0.0 }, b = { follows = "motion", low = 0.0, high = 1.0 } }
b = { a = { follows = "motion", low = 1.0, high = 0.0 }, b = { follows = "motion", low = 0.0, high = 1.0 } }
"#;
const WALK: &str = "start=0\nbounds=[0,2]\ncarry='always'\nevery={slots=16}\nstep=[2]";

#[test]
fn walk_update_at_a_return_precedes_the_door_and_stays_do_not_reset_it() {
    let c = Composition::parse(&piece(WALK, COMPLEMENTARY)).unwrap();
    let mut compiled = Compiled::new(&c);
    assert!(compiled.moves_through(BAR - 1).is_empty());
    assert_eq!(
        compiled.resolve_step(15).0[0].scene.as_ref().unwrap().scene,
        "a"
    );
    let moves = compiled.moves_through(3 * BAR).to_vec();
    assert_eq!(
        moves.iter().map(|m| m.to.as_str()).collect::<Vec<_>>(),
        ["b", "b", "b"]
    );
    assert!(moves[0].moved);
    assert!(!moves[1].moved);
    for step in [16, 32, 48, 15, 16] {
        let expected = if step < 16 { "a" } else { "b" };
        assert_eq!(
            compiled.resolve_step(step).0[0]
                .scene
                .as_ref()
                .unwrap()
                .scene,
            expected
        );
        assert_eq!(
            c.at_step(step).parts[0].trigger.probability,
            if step < 16 { 1.0 } else { 0.25 }
        );
    }
}

#[test]
fn target_ramp_is_sampled_at_the_return_without_restarting_on_scene_entry() {
    let source =
        "start=0\nrange=[0,2]\ncarry='always'\ntarget={every={bars=2},delta=[2],ramp={bars=1}}";
    let rows = "a={a='rest',b={follows='motion',low=0,high=1}}\nb={a='rest',b={follows='motion',low=0,high=1}}";
    let c = Composition::parse(&piece(source, rows)).unwrap();
    let mut compiled = Compiled::new(&c);
    let moves = compiled.moves_through(5 * BAR);
    assert_eq!(
        moves.iter().map(|m| m.to.as_str()).collect::<Vec<_>>(),
        ["a", "a", "b", "b", "b"]
    );
    assert_eq!(
        moves.iter().map(|m| m.moved).collect::<Vec<_>>(),
        [false, false, true, false, false]
    );
}

#[test]
fn probabilistic_doors_agree_with_source_values_and_cold_out_of_order_playback() {
    let source = "start=1\nbounds=[0,2]\ncarry='always'\nevery={slots=1}\nstep=[-1,0,1]";
    let text = piece(source, COMPLEMENTARY).replace(
        "use = \"techno.closed_hat\"",
        "use = \"techno.closed_hat\"\nvelocity={pattern='1 0.8 0.4',per='event',carry='always'}",
    );
    let c = Composition::parse(&text).unwrap();
    let r = c.router.as_ref().unwrap();
    let mut compiled = Compiled::new(&c);
    let mut source_cache = Cache::default();
    let mut previous = "a";
    let mut stayed = false;
    let mut moved = false;
    // More than 4096 source updates exercises checkpoint eviction during the warm walk.
    for index in 1..=270 {
        let tick = index * BAR;
        let value = c.lanes["motion"]
            .sample("motion", c.seed, tick, &mut source_cache)
            .value;
        let u = router::roll(c.seed, index);
        let expected = if u < 1.0 - value / 2.0 { "a" } else { "b" };
        let record = compiled.moves_through(tick).last().unwrap();
        assert_eq!((record.index, record.tick, record.roll), (index, tick, u));
        assert_eq!(
            (record.from.as_str(), record.to.as_str()),
            (previous, expected)
        );
        stayed |= !record.moved;
        moved |= record.moved;
        previous = expected;
    }
    assert!(stayed && moved);
    for step in [4320, 0, 17, 4095, 4096, 16, 4319] {
        let warm = compiled.resolve_step(step);
        let cold = Compiled::new(&c).resolve_step(step);
        assert_eq!(
            serde_json::to_value(&warm.0).unwrap(),
            serde_json::to_value(&cold.0).unwrap()
        );
        assert_eq!(warm.1, cold.1);
        let visit = r.visit_at(&c, BAR, step * STEP_TICKS);
        assert_eq!(warm.0[0].scene.as_ref().unwrap(), &visit);
        assert_eq!(
            c.at_step(step).parts[0].trigger.probability,
            r.scene(&visit.scene).unwrap().composition.parts[0]
                .trigger
                .probability
        );
    }
}

#[test]
fn load_checks_source_identity_and_the_whole_range_not_only_the_start() {
    let missing =
        Composition::parse(&piece(WALK, &COMPLEMENTARY.replace("motion", "missing"))).unwrap_err();
    assert!(missing.contains("follows \"missing\""), "{missing}");
    // At motion=0 the fixed 1 and the follower sum to one, but at motion=2 they sum to two.
    let invalid = "a={a=1,b={follows='motion',low=0,high=1}}\nb={b=1}";
    let error = Composition::parse(&piece(WALK, invalid)).unwrap_err();
    assert!(
        error.contains("router.routes.a") && error.contains("not 1"),
        "{error}"
    );
}
