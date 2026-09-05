# Phasecraft

A tiny deterministic realtime MIDI sequencer. Describe a rhythmic system in TOML,
loop it, and inspect every decision. This first slice plays one hat Part:
`XOR(E(7,16), E(2,5))` triggers with an independent `E(3,7)` accent lane.
Ableton (or any MIDI host) supplies the sound.

## Run

From a native package, use `./phasecraft` (Windows: `.\phasecraft.exe`). No Rust
installation is required to run a packaged executable. From source, install Rust
and run `cargo build --release --locked`; the executable is in `target/release`.
Linux source builds need `pkg-config` and ALSA development headers
(`sudo apt-get install pkg-config libasound2-dev` on Debian/Ubuntu).

```sh
# Inspect one four-bar phrase, including rests, as JSONL.
phasecraft inspect examples/hat.toml --steps 64

# Exercise realtime scheduling for 35 bars without opening a MIDI device.
phasecraft play examples/hat.toml --dry-run --bars 35

# Find and connect to an existing MIDI destination.
phasecraft ports
phasecraft play examples/hat.toml --port "Your MIDI Port" --watch

# macOS/Linux: publish a virtual MIDI source for the host to receive.
phasecraft play examples/hat.toml --virtual-port --watch
```

Ctrl-C stops and releases active notes. `--trace` prints the same decision records
as `inspect`, at planning time (normally 100ms ahead). Normal playback prints only
startup, reload, and shutdown messages. Redirect inspection to a file to compare
runs: `phasecraft inspect examples/hat.toml --steps 560 > cycle.jsonl`.

## Hear the hat

On macOS, start with `--virtual-port`, or enable an IAC bus and select it with
`--port`. On Windows, create a loopback destination with a utility such as loopMIDI,
then select it with `--port`. MIDI routing is an OS/driver concern;
[midir virtual source creation is available on Unix](https://docs.rs/midir/0.11.0/midir/struct.MidiOutput.html).
See [Ableton's virtual MIDI bus setup](https://help.ableton.com/hc/en-us/articles/209774225-Setting-up-a-virtual-MIDI-bus)
and [Apple's IAC instructions](https://support.apple.com/en-gb/guide/audio-midi-setup/ams1013/mac).

In Live, enable Track input for that port, choose it on a MIDI track, set monitoring
to In, and load a Drum Rack with a hat at MIDI note **42**. The example sends on
channel **10**. Adjust `part.output.note` for your rack. Phasecraft owns its clock:
this slice does not send MIDI clock, follow Live transport, or provide tempo sync.
Live can monitor and record the incoming notes without clips driving playback.

The Linux executable links to system ALSA and libc. Windows builds request a
static C runtime; macOS uses system frameworks. Packages are native CLI builds,
not signed installers. Cross-platform CI is defined in `.github/workflows/build.yml`;
adding that file does not mean its remote builds have run.

## Tune the system

The complete starting composition is in `examples/hat.toml`. It uses one stable
Part ID, two trigger generators, and one accent generator. Supported Boolean
operators are `or`, `and`, `xor`, `a_not_b`, and `b_not_a`. Binary nodes can nest;
the engine does not special-case a two-Euclidean composite.

Each Euclidean leaf accepts `steps`, `pulses`, `rotation` (default 0), and
`reset_on_phrase` (default false). The chosen Euclidean convention is a balanced
modular distribution with its first pulse at zero: E(3,8) is `x..x..x.`.
Positive rotation delays the pattern; negative rotation advances it. This
orientation is part of the contract, not a promise of identical knob positions
on another sequencer.

Time is 960 ticks per quarter; this prototype evaluates sixteenths (240 ticks)
in 4/4. A phrase defaults to four bars. Continuing cycles of 16, 5, and 7 steps
realign after 560 steps, or 35 bars, irrespective of phrase boundaries. We evaluate
at an absolute position rather than allocating an LCM-sized event array.

`trigger.probability` and `accent.probability` admit eligible decisions separately.
Their default `probability_mode = "phrase_locked"` repeats the random rolls at the
phrase boundary. `"continuous"` keys rolls to absolute steps. Neither mode resets
a generator's phase. The complete repeating output can therefore have a longer
period than 35 bars when phrase-locked probability is included.

Randomness is SHA-256 over a versioned, length-framed address containing seed,
Part ID, lane, decision, and step identity. The upper 53 bits of the first
little-endian 64-bit digest word map to [0,1). No process consumes shared RNG
state. Keeping IDs and the same configuration history preserves repeatability.

Accent is semantic: an event carries `active` and `amount`, independently of
MIDI. The initial velocity profile defaults to base 80 and boost 35; amount 0.8
produces velocity 108. Override it with `[part.profile]`, `base`, and `boost`.
The profile clamps to valid nonzero MIDI velocity. No trigger means no note,
even when the independent accent decision was admitted.

Output defaults to channel 10 and a 120-tick gate. The drum slice restricts gates
to 1..239 ticks to avoid overlapping instances of the same MIDI note. Musical
notes remain generator-independent; the output adapter creates note-on/off pairs.

With `--watch`, edits apply when the next phrase boundary enters lookahead.
An edit after that planning point waits until the following phrase. Invalid edits
keep the previous configuration running. Tempo and phrase-length changes require
a restart; seed, rhythms, probability, profiles, and routing fields can reload.
Continuing phase retains absolute transport position across edits. For an exact
replay of an edited performance, repeat its configuration changes at the same
boundaries; this slice does not record an edit history.

## Timing and verification

A producer resolves events into a bounded queue ahead of playback. A separate
thread sends MIDI at absolute monotonic deadlines; parsing and provenance output
are outside that thread. The initial lookahead is 100ms (`--lookahead-ms` adjusts
it). This is OS-thread scheduling, not hardware timestamp scheduling or a
hard-realtime guarantee. Notes more than 20ms late are dropped to prevent a burst
after a stall. Active note-offs still run. Shutdown reports dispatch lateness and
late drops; those measurements do not include driver or audio-host latency.

`cargo test --locked` checks balanced Euclidean rhythms, all Boolean operations,
560-step phase behavior, independent reset/probability semantics, deterministic
replay and RNG isolation, semantic accent, MIDI translation, and dispatch cleanup
including output failure. `cargo clippy --locked --all-targets -- -D warnings`
checks the implementation. A dry run validates transport execution, not actual
MIDI/audio timing. Listening and recording the real MIDI output remain the final
acceptance test on each music machine.

No harmony, arranger, groove engine, E16 integration, AI/MCP, DSP, or GUI is included.
The first design contracts are recorded in `docs/vertical-slice.md`.
