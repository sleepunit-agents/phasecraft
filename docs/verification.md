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
