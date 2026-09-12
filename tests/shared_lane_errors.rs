use phasecraft::music::Composition;
const STUDY: &str = include_str!("../examples/quickstart/walk.toml");

fn refusal(body: &str) -> String {
    Composition::parse(&format!("{STUDY}\n[lanes.probe]\n{body}\n"))
        .expect_err("accepted a bad lane")
        .to_string()
}
fn assert_says(body: &str, wants: &[&str]) {
    let said = refusal(body);
    for want in wants {
        assert!(
            said.contains(want),
            "refusal lost {want}\n  lane: {body}\n  said: {said}"
        );
    }
}

/// Witness one: a misspelled key is named, as it was before the lane became an enum.
#[test]
fn an_unknown_lane_key_is_named() {
    assert_says(
        "start=4\nrnge=[0,8]\ncarry='always'\ntarget={every={bars=11},delta=[-2,0,2],ramp={bars=4}}",
        &["unknown field `rnge`", "lanes.probe"],
    );
}

/// Witness two: a fault inside `target` keeps its full path, not just the lane's.
#[test]
fn a_fault_inside_target_keeps_its_path() {
    assert_says(
        "start=4\nrange=[0,8]\ncarry='always'\ntarget={every={bars=11},delta=[-2,0,2],ramp={bars='four'}}",
        &["expected u32", "lanes.probe.target.ramp.bars"],
    );
    assert_says(
        "start=4\nrange=[0,8]\ncarry='always'\ntarget={every={bars=11},delta=[-2,0,2],rmp={bars=4}}",
        &["unknown field `rmp`", "lanes.probe.target"],
    );
}

/// Shape selection: overlapping fields and each malformed partial shape say which is which.
#[test]
fn shape_selection_names_the_mix_and_the_missing_fields() {
    assert_says(
        "start=4\nrange=[0,8]\nbounds=[0,8]\ncarry='always'\ntarget={every={bars=11},delta=[-2,0,2],ramp={bars=4}}\nevery={slots=7}\nstep=[1]",
        &["mixes", "`range`", "`bounds`"],
    );
    assert_says(
        "start=4\nrange=[0,8]\ncarry='always'\ntarget={every={bars=11},delta=[-2,0,2],ramp={bars=4}}\nstep=[1]",
        &["mixes", "`step`"],
    );
    assert_says(
        "start=4\nrange=[0,8]\ncarry='always'",
        &["missing", "`target`"],
    );
    assert_says(
        "start=4\ncarry='always'\nevery={slots=7}\nstep=[1]",
        &["missing", "`bounds`"],
    );
    assert_says("start=4\ncarry='always'", &["`range`", "`bounds`"]);
}

/// Walk diagnostics retain the same key and nested-value witnesses as weather.
#[test]
fn walk_faults_name_the_key_and_nested_path() {
    assert_says(
        "start=4\nboonds=[0,8]\ncarry='always'\nevery={slots=7}\nstep=[1]",
        &["unknown field `boonds`", "lanes.probe"],
    );
    assert_says(
        "start=4\nbounds=[0,8]\ncarry='always'\nevery={slots='seven'}\nstep=[1]",
        &["expected u32", "lanes.probe.every.slots"],
    );
}

/// Witness five: a target lane's refusal must close the search, not correct a spelling.
///
/// `lanes.weather.every` used to say "target lanes have no top-level every", which reads
/// as *you spelled it wrong*. The spelling a reader tries next, `lanes.weather.target.every`,
/// answered "a walk's transport clock is lanes.<name>.every" — the key they started from.
/// Both hops contain their own key, so both passed the `contains(key)` witness above while
/// routing a reader in a circle past the fact that a target lane has no member clock at all.
#[test]
fn a_target_lane_refusal_closes_the_search_instead_of_redirecting() {
    use phasecraft::music::router::member_clock;
    let weather = Composition::parse(include_str!("../examples/quickstart/weather.toml")).unwrap();
    let said = member_clock(&weather, "lanes.weather.every").unwrap_err();
    assert!(said.contains("target lanes"), "{said}");
    assert!(
        said.contains("any other spelling"),
        "still reads as a spelling correction: {said}"
    );
    assert!(
        !said.contains("lanes.<name>.every"),
        "offers a spelling no target lane answers: {said}"
    );
    // The keys a reader reaches for next must not hand back the key they came from.
    for next in [
        "lanes.weather.target.every",
        "lanes.weather.target.every.bars",
    ] {
        let said = member_clock(&weather, next).unwrap_err();
        assert!(said.contains(next), "refusal lost its key: {said}");
        assert!(
            said.contains("only a walk lane"),
            "does not say which lane kind answers, so the reader loops: {said}"
        );
    }
}
