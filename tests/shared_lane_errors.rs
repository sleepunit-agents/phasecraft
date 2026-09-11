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
