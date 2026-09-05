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
| `src/music/resolve.rs` | Keyed probability, semantic events, resolution and MIDI translation |
| `src/playback/mod.rs` | Clock conversion, MIDI sinks, deadline dispatch and cleanup |
| `src/playback/transport.rs` | Lookahead producer, looping and phrase-boundary reload |
| `src/cli.rs` | Commands, output selection and terminal presentation |
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

This split deliberately keeps related code together. Add groove definitions when
there is working groove behavior to put in them. No plugin discovery, arrangement
engine, schema code generation, or controller abstraction is introduced here.
