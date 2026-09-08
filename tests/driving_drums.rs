use phasecraft::{authoring::project, music::resolve::Compiled};
use std::path::Path;

#[test]
fn driving_drums_project_matches_the_eight_bar_audition_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/run-the-line-driving-drums");
    let loaded = project::load(&root).unwrap();
    assert!(loaded.gaps.is_empty());
    assert_eq!(loaded.midi.port.as_deref(), Some("Phasecraft"));
    assert!(loaded.midi.send_clock);
    assert_eq!(loaded.composition.tempo, 170.0);
    assert_eq!(loaded.composition.seed, 4471);
    let mut compiled = Compiled::new(&loaded.composition);
    let actual: Vec<_> = (0..128)
        .flat_map(|step| compiled.resolve_step(step).1)
        .map(|event| (event.tick, event.bytes))
        .collect();
    let mut expected = Vec::new();
    for bar in 0..8 {
        for (offset, note, velocity) in [
            (0, 36, 127),
            (960, 38, 121),
            (2400, 36, 114),
            (2880, 38, 121),
        ] {
            let tick = bar * 3840 + offset;
            expected.push((tick, [0x99, note, velocity]));
            expected.push((tick + 240, [0x89, note, 0]));
        }
    }
    assert_eq!(actual, expected); // Includes every note-off and rejects unexpected CCs/hits.
    let from_manifest = project::load(&root.join("phasecraft.toml")).unwrap();
    assert_eq!(from_manifest.composition.tempo, 170.0);
}
