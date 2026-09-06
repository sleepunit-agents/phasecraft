# Source layout

The source separates editing files, resolving musical decisions, and delivering
MIDI. Musical definitions are grouped by what a musician wants to find.

| Location | Responsibility |
| --- | --- |
| `src/authoring/project.rs` | Project discovery, MIDI settings, validation reports, safe scaffolding |
| `src/authoring/library.rs` | Imports, reusable components, profiles, override expansion |
| `src/authoring/syntax.rs` | Keyed Parts and rhythm shorthand |
| `src/music/mod.rs` | Typed composition, Parts, lanes, profiles and validation |
| `src/music/rhythm.rs` | Rhythm expressions and structural provenance |
| `src/music/groove.rs` | Swing, bounded timing offsets, run contours and ghost interpretation |
| `src/music/resolve.rs` | Keyed probability, semantic events, resolution and MIDI translation |
| `src/playback/mod.rs` | Clock conversion, MIDI sinks, deadline dispatch and cleanup |
| `src/playback/transport.rs` | Lookahead producer, looping and phrase-boundary reload |
| `src/cli.rs` | Commands, output selection and terminal presentation |
| `src/music/parameter.rs` | Held values and musical-time ramps independent of note admission |
| `src/music/accent.rs` | Named emphasis responses, MIDI control bindings and validation |
| `library/drums/` | Common, techno and DnB behaviors |
| `library/accents/` | Reusable emphasis responses |
| `library/kits/` | Output pad components |
| `templates/project/` | Files embedded by `phasecraft new` |
| `examples/quickstart/` | Proven playable beats and their compact equivalents |
| `examples/studies/` | Controlled comparisons of individual mechanisms |
| `examples/showcases/` | Combined musical systems and their personal libraries |

The `config`, `engine`, and `library` Rust exports remain compatibility facades.
The model's existing parse/read convenience methods delegate to authoring; event
resolution and dispatch do not interpret project files. The producer expands and
validates a complete composition before applying it to the transport.

This split deliberately keeps related code together. Reusable groove components
now live in `library/grooves/`; project-specific groove overrides live in `patterns/grooves.toml`. No plugin discovery, arrangement
engine, schema code generation, or controller abstraction is introduced here.

## Desktop player

`src/player.rs` owns project selection and a stoppable session independent of any
window toolkit. `run_controlled` in transport accepts cancellation and an optional
bounded snapshot sender; Ctrl-C registration is confined to the CLI wrapper.
Telemetry is best-effort and never blocks the producer. Frames carry their musical
deadlines and the composition revision that produced them; Player exposes only
frames due at the current monotonic time. MIDI dispatch remains unchanged.

`desktop/src/main.rs` is a Tauri command adapter and local recent-project storage.
`desktop/ui/` renders the project browser, transport, Canvas rings and detail view.
No JavaScript code schedules MIDI or writes composition parameters. The desktop
crate has its own lockfile and platform dependencies; the CLI builds separately.
