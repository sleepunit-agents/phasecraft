# Phasecraft

A tiny deterministic realtime MIDI sequencer. Describe a rhythmic system in TOML,
loop it, and inspect every decision. Parts share a musical clock and MIDI port while keeping independent rhythms,
accents, profiles, and stable random identities. Start with one polymetric hat,
a 132 BPM techno beat, or a 172 BPM drum-and-bass beat.
Ableton (or any MIDI host) supplies the sound.

## Download

Grab a native package from the [rolling dev release](https://github.com/sleepunit-agents/phasecraft/releases/tag/dev).
The repository and downloads are public; no GitHub login is needed.

**Desktop player:** on Windows, run `phasecraft-player-windows-x64-setup.exe`.
It installs for your account and adds a Start menu shortcut. Launch **Phasecraft
Player**, choose **Open project**, and point it at your project folder. Select a
composition and MIDI destination, then press **Play**. **New project** seeds the
909 techno, DnB, garage and accent-control examples. **Silent preview** runs the same transport without MIDI.
Mac builds ship as DMGs; Linux x64 ships as an AppImage and Debian package. See the
[desktop guide](docs/player.md) for installation, playback and visualization details.

**CLI:** on Windows, download `phasecraft-windows-x64.zip` and extract it. Open
PowerShell in that folder. No Rust installation is required to run either package.

The `dev` prerelease follows `main` after all four native builds and tests pass.
It replaces the same downloads in place and includes the source commit and SHA-256
checksums. Pull requests do not publish releases. Maintainers can also rerun the
build using Actions → Test and package → Run workflow on `main`.

The player can send MIDI tempo/transport sync through your MIDI connection. See [routing and sync](docs/player.md#let-ableton-follow-tempo-and-transport).

## Update in place

Download this updater-enabled build once, then:

```powershell
.\phasecraft.exe --version
.\phasecraft.exe update --check
.\phasecraft.exe update
```

Updates compare Git commits against the rolling `dev` release, verify the download,
and replace only the executable. Stop playback first; your next command uses the
new build. Your projects and configuration stay intact. `play` makes no update
requests. Use `update --force` to reinstall the same commit.

Public updates require no credentials. If you supply a GitHub CLI login or
`GH_TOKEN` / `GITHUB_TOKEN`, it is used for authenticated requests. No token belongs
in your TOML. This updater manages the CLI executable; update the desktop player
using its **Update & restart** chip. See [update details](docs/authoring.md#updating-phasecraft).

## Start a project

The project directory can hold a track, an album, or a live set. It is a collection
of playable compositions and shared musical definitions; it does not arrange them.

From the extracted Windows package:

```powershell
.\phasecraft.exe new my-set
.\phasecraft.exe validate my-set
.\phasecraft.exe play my-set --dry-run --bars 4
.\phasecraft.exe ports
# Set the destination name in my-set/config/midi.toml.
.\phasecraft.exe play my-set --watch
.\phasecraft.exe play my-set/compositions/dnb.toml --watch
```

`new` seeds **132 BPM techno and garage**, **172 BPM DnB**, and an accent-control study for
Ableton's **909 Core Kit**. It refuses existing destinations. Templates and built-in
behaviors are embedded in the executable; no source checkout or template download
is required.

```text
my-set/
  phasecraft.toml          # default composition and explicit shared library list
  compositions/           # techno.toml, dnb.toml; add more pieces here
  patterns/
    drums.toml            # reusable drum behaviors and trigger patterns
    accents.toml          # emphasis timing and response profiles
  kits/909.toml           # musical MIDI pad mappings
  config/midi.toml        # this machine's connection and lookahead
  README.md               # local editing and playback guide
```

With Phasecraft on PATH, `phasecraft play` inside that folder plays its default.
An explicit composition inside a project also receives the project's libraries
and MIDI settings. `--port`, `--virtual-port`, and `--dry-run` override the configured
destination; `--lookahead-ms` overrides configured lookahead. MIDI settings take
effect when playback starts. Musical changes—including shared libraries—apply at
phrase boundaries with `--watch`.

`phasecraft validate my-set --json` checks every listed composition without opening
MIDI and returns file paths, errors, and a `valid` boolean (nonzero exit on failure).
`expand my-set` and `inspect my-set --human` use its default composition. Standalone
TOML files still work. See [authoring conventions](docs/authoring.md) for path and
merge rules, and [source layout](docs/architecture.md) for the internal boundaries.

## Run

From a native package, use `./phasecraft` (Windows: `.\phasecraft.exe`). No Rust
installation is required to run a packaged executable. From source, install Rust
and run `cargo build --release --locked`; the executable is in `target/release`.
Linux source builds need `pkg-config` and ALSA development headers
(`sudo apt-get install pkg-config libasound2-dev` on Debian/Ubuntu).

```sh
# Inspect one four-bar phrase, including rests, as JSONL.
phasecraft inspect examples/quickstart/hat.toml --steps 64

# Exercise realtime scheduling for 35 bars without opening a MIDI device.
phasecraft play examples/quickstart/hat.toml --dry-run --bars 35

# Find and connect to an existing MIDI destination.
phasecraft ports
phasecraft play examples/quickstart/hat.toml --port "Your MIDI Port" --watch

# macOS/Linux: publish a virtual MIDI source for the host to receive.
phasecraft play examples/quickstart/hat.toml --virtual-port --watch
```

Ctrl-C stops and releases active notes. `--trace` prints the same decision records
as `inspect`, at planning time (normally 100ms ahead). Normal playback prints only
startup, reload, and shutdown messages. Redirect inspection to a file to compare
runs: `phasecraft inspect examples/quickstart/hat.toml --steps 560 > cycle.jsonl`.

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

## Play a full kit

Load **909 Core Kit** on a single monitored MIDI track and run either example:

```powershell
.\phasecraft.exe play examples/quickstart/techno.toml --port "Phasecraft" --watch
.\phasecraft.exe play examples/quickstart/dnb.toml --port "Phasecraft" --watch
```

Run one at a time; Ctrl-C stops. Set Live to the matching tempo if you want its
recording grid to line up (Phasecraft still owns its clock).

- `techno.toml`: 132 BPM, quarter-note kick, claps on 2 and 4, alternating closed
  and open eighth-note hats, plus quiet probabilistic rim clicks. A seven-step
  closed-hat accent and fifteen-step rim cycle add movement around fixed anchors.
- `dnb.toml`: 172 BPM, kick on 1 and the & of 3, snare on 2 and 4, alternating
  emphasis on sixteenth hats, an open hat on the & of 4, and quiet optional rim
  clicks. A simple two-step drum groove; the 909 supplies all sounds.
- `hat.toml`: the original one-Part polymetric experiment, unchanged.

Both kit examples use channel 10 and the following rack pads:

| Role | MIDI note | Live note name |
| --- | ---: | --- |
| Kick | 36 | C1 |
| Rim shot | 37 | C#1 |
| Snare | 38 | D1 |
| Clap | 39 | D#1 |
| Closed hat | 42 | F#1 |
| Open hat | 46 | A#1 |

Assignments were cross-checked against the 909 Core Kit rack in
[Ableton's published DJ Gigola Live Set](https://www.ableton.com/en/blog/download-the-live-set-of-dj-gigolas-new-track-unfolding-practice-ii/).
The examples avoid simultaneous closed/open hat triggers; the kit retains
ownership of its hat choke behavior and audio envelopes.

New compositions can use `[parts.kick]`, with `[parts.kick.trigger]` and similar
sections beneath it. The table name supplies the stable ID. The original `[part]`
and `[[parts]]` forms remain supported with explicit `id`; do not mix forms. Each
Part needs a unique ID and MIDI channel/note pair. Up to 32 Parts share the selected output port.
Keeping the same ID preserves random decisions when Parts are reordered or added.
Use one Part per drum voice; combining multiple Parts on the same MIDI route is
rejected to prevent conflicting note-offs.

Inspection emits one JSONL record per Part per step, ordered by stable Part ID.
Playback merges all Parts' events by musical tick before dispatching them.
Configuration reloads apply the entire composition at a phrase boundary, including
added/removed Parts. Gates end before that boundary, so outgoing notes are released
before new routing takes effect.

## Reusable authoring

The [909 listening guide](examples/README.md) covers the focused examples and
`showcase.toml`. The example directories are now `quickstart/`, `studies/`, and `showcases/`.
The original hat/techno/DnB musical definitions remain unchanged; their compact
`techno-reuse.toml` and `dnb-reuse.toml` counterparts produce identical events.

```toml
tempo = 132
seed = 91827

[parts.kick]
use = "techno.kick"

[parts.closed_hat]
use = "techno.closed_hat"
[parts.closed_hat.trigger]
probability = 0.8
[parts.closed_hat.accent.rhythm]
steps = 11
pulses = 4
[parts.closed_hat.profile]
use = "accent.subtle"
```

Built-ins include `techno.{kick,clap,closed_hat,open_hat,rim}` and
`dnb.{kick,snare,closed_hat,open_hat,rim}`; smaller components include
`std.backbeat`, `std.no_accent`, and `kit.909.{kick,rim,snare,clap,closed_hat,open_hat,low_tom,ride}`.
Velocity profiles are `accent.velocity_only` (80 + 35 × amount), `accent.subtle`
(70 + 12 × amount), and `accent.punch` (72 + 40 × amount).

`use` selects one definition. `compose = ["std.backbeat", "std.no_accent",
"kit.909.clap"]` combines definitions from left to right. Local fields override
the result. Do not specify both `use` and `compose` on one table. Overrides merge
nested fields; changing a rhythm's `type` replaces that expression, and selecting
a different profile with `use` replaces the previous profile's settings. Arrays
replace rather than append. Every Part instance supplies its own stable identity, through its table name or `id`.

Define personal behaviors under `[library.behaviors."my.name"]` and profiles
under `[library.profiles."my.name"]`, either in the composition or in a file loaded
with `imports = ["library/personal.toml"]`. Imports are relative to the file that
names them. A library file contains only `imports` and `library`. Named definitions
may compose other definitions; duplicate names, unknown references, import cycles,
and dependency cycles are errors. Reusing the same imported file is idempotent.
Reachable definitions are expanded and the result is validated before playback;
unreferenced behavior bodies are not independently complete Parts.

Use `phasecraft expand FILE` to inspect the fully resolved configuration, or save
its output as a standalone TOML with no library dependencies. `inspect FILE
--human` shows readable admissions and resulting MIDI values; default inspection
retains full JSONL rhythm trees. Library expansion happens in the producer, outside
MIDI dispatch. `--watch` reloads imports too, preserving the whole previous model
if the new composition cannot be resolved or validated.

## Rhythmic relationships

An expression can reference another Part's trigger lane:

```toml
rhythm = { op = "a_not_b", a = { steps = 16, pulses = 7 }, b = { part = "kick", mode = "hits" } }
```

`mode = "hits"` uses that Part's admitted trigger after probability. `mode =
"structural"` uses its trigger expression before its own lane admission. The mode
is required. Both modes refer to the same absolute grid position. Neither creates
an extra random decision; Part ordering cannot affect the result. Both trigger
and accent expressions can contain references, but all Part dependencies must form
an acyclic graph. Cycles (including self-references) and missing targets reject the
configuration before any of it is applied.

## Tune the system

The smallest starting composition is in `examples/quickstart/hat.toml`. It uses one stable
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
560-step phase behavior, multi-Part ordering and isolation, example backbeats,
independent reset/probability semantics, deterministic
replay and RNG isolation, semantic accent, MIDI translation, and dispatch cleanup
including output failure. `cargo clippy --locked --all-targets -- -D warnings`
checks the implementation. A dry run validates transport execution, not actual
MIDI/audio timing. Listening and recording the real MIDI output remain the final
acceptance test on each music machine.

The desktop GUI is a player and visualizer; TOML remains the authoring surface.
No harmony, arranger, E16 integration, AI/MCP, or DSP is included.

Reusable swing, laid-back timing, run contours and ghost articulation are available;
see [groove and the 909 garage example](docs/groove.md).
The first design contracts are recorded in `docs/vertical-slice.md`.

Reusable [multi-control accent profiles](docs/accents.md) can drive velocity plus
named MIDI controls. New projects include a 909 `accent-punch` example and MIDI-learn
helpers for its two Ableton mappings.
