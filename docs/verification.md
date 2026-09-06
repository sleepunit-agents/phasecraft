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

## Commit-aware self-update

The updater adds manifest/schema/platform validation, corrupted/mixed asset
rejection, bounded downloads, HTTP failure and redirect credential tests. A native
subprocess replaces a temporary copy of its own test executable with Phasecraft,
then runs the replacement and checks adjacent project files remain unchanged.
This test runs on all four release platforms. `--version` and JSON version output
are checked, and the existing musical golden traces remain in the suite.

## Desktop player milestone

- The root suite has 57 tests, adding deadline-gated visual state, repeated
  stoppable sessions with note cleanup, and an unread/full telemetry channel
  that cannot block transport completion. Existing musical provenance hashes
  and CLI/import-watch tests remain unchanged.
- Two JavaScript tests check the ring projection and independent reference/cycle
  identities. Three browser tests cover open/new/recent projects, output selection,
  play/stop, inspection, watched-error display/recovery, and minimum window width.
  The UI fixture is recorded from the actual showcase composition and traces.
- Native Linux Tauri/WebKit automation opens a generated project, starts/stops
  repeatedly, observes playback progress, rejects and recovers from a broken
  imported library, switches to DnB and closes the window while playing.
- Native screenshots were inspected at 1440 pixels; minimum-width overflow is
  checked at 900 pixels. UI screenshots are test artifacts, not musical fixtures.
- MIDI tests in this environment use silent/fake sinks because no ALSA sequencer
  device is available. Actual Ableton playback remains a check on the music machine.

## Multi-control accent verification (2026-09-05)

- 70 Rust tests pass, including response scaling/clamping, CC-before-note ordering,
  gate and Stop/error resets, late control handling, 32-Part/eight-control load,
  routing validation, and unchanged original 35-bar provenance.
- Seven browser interaction tests pass, including existing-port MIDI sync and
  visible control values/reset information. Native Tauri/WebKit smoke passes
  repeated playback, reload rejection/recovery, garage and accent-punch playback.
- Four bars of accent-punch at 132 BPM through the silent realtime sink sent
  156 note messages, 40 CC messages and 384 clock pulses; zero late notes dropped,
  maximum observed lateness 1.283 ms on this Linux machine.
- Physical Windows MIDI and Ableton parameter response remain user-side listening
  checks. Automated recording sinks verify the bytes; they do not prove sound.

## Held parameters and ramps (2026-09-05)

- 76 Rust tests pass, including an eight-bar ramp crossing four-bar phrases,
  interpolation and holding endpoints, parameters on rests, deduplicated constant
  values, current-base accent restoration, swung note boundaries, stale samples,
  malformed timelines, maximum 32-Part/eight-parameter load, and watched value edits
  during an entirely silent phrase. Original 35-bar provenance remains unchanged.
- Eight browser tests pass, including the parameter inspector advancing on a rest
  without flashing a note. Root and desktop clippy pass with warnings denied.
- Native Linux Tauri/WebKit smoke passes repeated playback, watched error/recovery,
  composition selection, the intro's live parameter inspector, and closing during playback.
- Nine bars of the intro at 132 BPM through the silent realtime sink: 276 note
  messages, 327 control messages, 864 clock pulses, zero dropped notes, maximum
  measured dispatch lateness 2.713 ms on this machine. The intro's three output
  bindings were checked against the generated compact Set's mapping report.

## Stop defaults and prepared controls (2026-09-05)

- 78 Rust tests pass, including mid-ramp Stop during and after an accent, the
  transition to a composition without control lanes, explicit default validation,
  partial-send cleanup and finite completion after watched parameter changes.
- Eight browser checks and the native Linux player smoke pass, including selecting
  movement and inspecting cutoff/level/pan/decay before closing during playback.
  Root and desktop clippy pass with warnings denied.
- Three private-fixture generator tests verify all 64 mappings, unchanged sample
  references/internal macro links, kit-binding consistency and rejection of
  conflicting mappings. All 64 generated isolation compositions validate.
- New level/pan/decay response and round-trip Set opening still require Live on the
  music machine. Existing cutoff response was confirmed by Jonathan.
