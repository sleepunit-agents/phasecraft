use phasecraft::music::{Composition, resolve::resolve_step};
use std::path::Path;
#[test]
fn prepared_journeys_preserve_the_main_groove_and_finish_after_32_bars() {
    for (genre, base, tempo) in [
        ("techno", "techno", 132.0),
        ("dnb", "dnb", 172.0),
        ("garage", "garage-touch", 132.0),
    ] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/quickstart");
        let c = Composition::read(&root.join(format!("{genre}-journey.toml"))).unwrap();
        let original = Composition::read(&root.join(format!("{base}.toml"))).unwrap();
        assert_eq!(c.tempo, tempo);
        assert_eq!(c.end_step(), Some(512));
        for step in 128..256 {
            let (traces, _) = resolve_step(&c, step);
            let (old, _) = resolve_step(&original, step);
            for trace in traces {
                let old = old.iter().find(|t| t.part == trace.part).unwrap();
                assert_eq!(
                    trace.trigger.admitted, old.trigger.admitted,
                    "{genre} {} {step}",
                    trace.part
                );
            }
        }
        for step in 256..320 {
            let (traces, _) = resolve_step(&c, step);
            assert!(
                traces
                    .iter()
                    .find(|t| t.part == "kick")
                    .unwrap()
                    .event
                    .is_none()
            );
        }
        assert!(resolve_step(&c, 512).1.is_empty());
        for section in &c.arrangement.as_ref().unwrap().sections {
            for p in &section.composition.parts {
                assert_eq!(p.output.channel, 10);
                assert!((36..=51).contains(&p.output.note));
                for output in p.output.controls.values() {
                    assert!(matches!(output.channel, Some(15 | 16)));
                    assert!(output.default.is_some());
                }
            }
        }
    }
}
