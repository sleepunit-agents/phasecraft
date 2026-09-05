# Local verification — 2026-09-04

Environment: Linux x86_64 container; Rust 1.98.1. No ALSA sequencer device
(`/dev/snd/seq`) is exposed. These results do not establish MIDI driver or audio
latency, or native Windows/macOS runtime behavior.

- `cargo test --locked`: 18 tests passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- `cargo build --release --locked`: passed.
- `cargo +stable check --locked --target x86_64-pc-windows-gnu`: passed.
  This checks Windows source compatibility; it does not produce or run an EXE.
- 35-bar realtime dry run of `examples/hat.toml` at 132 BPM: 492 messages sent
  to the silent sink, zero late notes dropped, maximum dispatch lateness 0.320ms.
- Live watch smoke at 400 BPM with one-bar phrases: changed seed after bar 1
  began; new seed applied at bar 2. Invalid TOML retained that configuration at
  bar 3. A valid new seed applied at bar 4. All 64 step traces were present.
- MIDI port enumeration: correctly failed initialization because the container
  has no ALSA sequencer device. Real MIDI reception and audible groove remain
  unverified.

Native packaging CI is prepared for Linux x64, Windows x64, macOS ARM64 and
macOS x64. It has not run remotely. The local Linux package uses this container's
system libc/ALSA; the CI Linux package builds against Ubuntu 22.04 for a broader
compatibility baseline.

## Music-machine acceptance

1. Build or unpack the native executable and route its MIDI to a host.
2. Load a hat at note 42 and listen to the example for at least 35 bars.
3. Record output, restart with the same seed, and compare note/velocity content.
4. Edit accent probability while watching: trigger decisions should stay fixed.
5. Edit seed: hear the change at the next unplanned phrase boundary.
6. Stop during a note and confirm note-off delivery. Check driver/audio timing
   separately from the CLI's dispatch-lateness measurements.

## Multi-Part extension — 2026-09-05

- 24 tests pass, including conventional example anchors, distinct kit routes,
  Part identity isolation, stable output under reordering, simultaneous note
  ordering with unequal gates, legacy TOML equivalence, and multi-note cleanup.
- Clippy and release build pass. The old hat's 4,480-step inspection output is
  byte-for-byte identical to the output captured before the refactor.
- Four-bar realtime silent runs: techno 132 BPM, 124 messages, zero late drops;
  DnB 172 BPM, 170 messages, zero late drops.
- Live reload smoke: removed the rim Part at bar 2, re-added under a new ID at
  bar 3; exactly 224 expected provenance records, with correct Part membership.
- Capacity smoke: 32 Parts firing every sixteenth at 400 BPM with maximum 1s
  lookahead, 1,024 messages over one bar, zero late drops or queue overflow.
- Kit notes were checked against the 909 Core Kit in Ableton's published
  [DJ Gigola Live Set](https://www.ableton.com/en/blog/download-the-live-set-of-dj-gigolas-new-track-unfolding-practice-ii/).
  Its stored ReceivingNote values use the inverse mapping (128 minus MIDI note),
  consistent with [Paketti's documented rack-format findings](https://esaruoho.github.io/paketti/CHANGESLOG.html).
  Only note-assignment facts were used; no Live Set, samples, or musical content
  from that project is included here.

The new full-kit grooves still need listening/recording in the user's Ableton.

## Reusable authoring and interlocking rhythms — 2026-09-05

- 38 tests cover the prior engine plus named-behavior equivalence, deep overrides,
  expression replacement, component precedence, profile resolution, personal
  imports, missing definitions, duplicate names, import cycles, Part reference
  cycles, structural/admitted-hit semantics, and ordering independence.
- All 17 playable examples validate and produce complete ordered MIDI note pairs.
  Controlled pairs assert reset/probability/profile/seed differences and accent
  edits that preserve trigger decisions.
- Original hat/techno/DnB JSONL output matches the previous release byte-for-byte
  over 560 steps each. Compact reusable genre examples match their explicit forms.
- Import-watch integration changes library admission at bar 2, rejects malformed
  library content at bar 3 while preserving the previous model, and recovers at
  bar 4. The main file is untouched throughout.
- Clippy, formatting, and optimized Linux build pass.
- The six-Part showcase ran eight bars with import-aware watch: 330 MIDI messages
  to the silent sink, zero late drops, maximum dispatch lateness 0.084ms.
- Expanded standalone showcase TOML reproduces its imported source's 128-step
  JSONL decision stream exactly.

Native packaging now includes the examples directory recursively, including the
listening guide and imported personal library. These new grooves need the same
real-host listening evaluation as previous milestones; dry-run timing is not
an audio measurement.

## Project authoring and structural refactor

- 48 integration tests pass locally, including ten project/authoring tests.
- Saved SHA-256 fingerprints of 560-step JSONL traces from pre-refactor release
  `951df512f4641bed4a56afc05a7419410f6978eb`; the original hat, techno, DnB and
  showcase traces remain byte-identical. Fixtures are in
  `tests/fixtures/pre-project-provenance.json` and run on every native platform.
- Generated projects validate both compositions, retain identical genre traces,
  survive moving the project directory, and run the default composition through
  realtime dry-run playback without opening MIDI.
- Project library edits apply at phrase boundaries; invalid edits retain the
  previous music and subsequent valid edits recover. Shared-kit edits resolve
  for both compositions. Existing standalone import-watch tests still pass.
- New-directory refusal preserves existing files. Validation aggregates errors
  from listed compositions and emits parseable JSON with nonzero failure status.
- Keyed Parts, inferred rhythm kinds, explicit syntax, references, partial
  overrides and expression-kind changes are covered by equivalence/error tests.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and
  `cargo build --release --locked` pass locally. Native CI additionally runs the
  complete test suite on Windows, Linux and both Mac architectures.
- No new physical MIDI recording was made for this authoring refactor. Playback
  acceptance recordings from the prior milestone remain the listening evidence.

Example paths moved to `examples/quickstart`, `examples/studies` and
`examples/showcases`; older paths above describe the historical test runs.
