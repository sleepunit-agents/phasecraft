# Current percussion coverage

Updated 2026-09-06 after the unattended percussion checklist. This supersedes the
status tables in `spec-audit.md`; that file remains a historical engineering record.
The original 49-section handoff included future possibilities, not a mandate to
build every named feature immediately.

| Requirement or later request | Current implementation | Listening / validation |
| --- | --- | --- |
| Deterministic local probability (§14–18) | SHA-256 decision addresses isolate Parts, lanes and decisions; phrase-locked/continuous modes | Probability studies, original 35-bar golden traces, RNG-isolation tests |
| Independent rhythms and reset (§9–13, 19, 25, 32) | Recursive five-operator Boolean algebra; signed leaf rotation; independent Euclidean lengths; per-leaf reset | Original hat, algebra/phase studies, cycle tests |
| Generator-independent realization (§12) | Semantic events in a requested window; cycle spans describe structural phase alignment without allocating an LCM-sized pattern | `resolve::realize`, `tests/cycles.rs`; same resolution path as playback |
| Part relationships (§11, 28–29) | Explicit structural versus admitted-hit references, acyclic dependency validation | Reference studies, garage rim avoids actual kicks |
| Musical reuse (§2–5, 26–28, 38–39) | Built-in/personal namespaces, `use`, `compose`, nested overrides and expanded TOML | Project `patterns/`, `kits/`, project/import tests |
| Semantic emphasis (§20–23) | Independent accent lane; reusable velocity and named multi-CC profiles; named shared sources; finite-memory accumulation and decay | accent-punch, accent-memory, shared/control tests |
| Procedural groove (§24) | Swing, delay, gate, ghosts, three-hit contours, offbeat gain, first-hit-after-gap gain, independently seeded touch | garage versus garage-touch; context/RNG/timing tests |
| Parameter control outside notes (later request) | Held values, single ramps, segmented linear/smooth/hold motion, fractional durations, delayed start, repeats; emphasis combines with current base | intro, movement, breathing, automation/envelope tests |
| Musical time and live sequencing (§30–31, 35–37) | Integer 960 PPQN, fixed BPM, bounded lookahead queue, independent deadline dispatch, optional MIDI clock/Start/Stop, owned note/control cleanup | Clock, stall, error and Stop-default tests; user Live recordings |
| Procedural identity and arranging (§6, 18, 33; later scope extension) | A/A2 inheritance; finite or repeated sections; explicit restart/continue clock; default resets; watched musical edits; structural edits require restart | sections, three genre journeys, finite clock and watched-reload tests |
| Explainability (§45) | JSON traces and readable inspection; resolved MIDI/control values; groove causes; parameter/envelope progress; shared decisions; audible section display | CLI and Player tests; `expand` for fully resolved defaults |
| Controller/host separation (§26, 28, 37, 41) | MIDI bindings separate from semantic parameters; arbitrary note/channel per Part; CC can use another channel; one output port | Kit routing tests; fixed note on each hardware voice's channel is supported |
| Friendly project workflow (later request) | `phasecraft new`, project discovery, separate musical libraries/kit/config; folder-opening Player and visual rings | Portable scaffold, 15 starter compositions, browser/native smoke |
| Distribution (later request) | CLI and native Player packages for Windows x64, macOS Intel/Apple Silicon, Linux x64; signed Player update feed; explicit click to install | Full platform CI plus published-feed and Windows checksum verification |
| Prepared 909 adapter (later request) | Stock note mapping and 64 mapped controls: cutoff, level, pan, stock decay/tail across 16 pads; compact channels 15/16; defaults restored | Existing user-validated Prepared Set; fixture-scoped XML tests |

## Cycle metadata contract

`RhythmPattern` contains a requested time window and semantic events. Its `cycles`
list splits that window at arrangement boundaries, omitting intervals where the
requested Part is absent or finite playback has ended. Each span gives global
bounds, a musical phase origin, phrase length and `phase_alignment_steps`.

The alignment is a common multiple of the trigger/accent structural processes,
including referenced trigger processes and consumed shared accents. A process that
resets on phrase contributes the phrase length. It is an alignment period, not
necessarily the smallest repeating rhythm. It deliberately makes no promise that
probability decisions, groove, automation or accent memory repeat after that period.
For example, 16/5/7 produces 560 even when continuously addressed probability keeps
changing the realized notes. An unrepresentable common multiple is `None`; no huge
allocation or silent overflow occurs. Future nonperiodic generators can also report
an unknown alignment. Per-step traces still expose individual leaf phases.

## Deliberately deferred

- Harmony, scales, chords, voice leading, melody and pitched 303 sequencing.
- Tie, slide, overlapping note ownership and synth-specific articulation.
- DSP, synthesis, plugin hosting, audio recording and DAW project manipulation
  beyond the narrowly scoped prepared-kit adapter.
- Full controller layouts/macros, AI/MCP, in-app composition editing and a custom DSL.
  The [E16 kick preview](../tools/controllers/README.md) now has temporary bar-boundary
  controls and feedback; physical testing is pending.
- Whole-expression Rotate or nested Probability nodes: leaf rotation and explicit
  lane decisions cover current examples. New algebra nodes need a concrete use case.
- Universal density/variation knobs, automatic style generation and phrase mutation:
  their semantics remain intentionally unearned. Probability is not density.
- Tempo/meter changes, cross-bar pickups, arbitrary tuplets beyond triplets, transport
  seek/resume, clock following, multiple simultaneous output ports, and recording
  the edit history of a live performance.
- A general configurable synthesizer template generator or mapped tune/drive/space/
  separate accent-gain stage. Current stock fixture evidence supports four controls.

“Shared” accent means a named process with explicit consumers: using it on all drums
makes it global to that set, and using it on a subset makes it a group. Maximum
amount combines sources. The memory profile is a bounded MIDI control response,
not a claim of authentic analog 303 circuitry. Level remains independent of velocity.

## What still needs ears or hardware

The original beats, prepared cutoff/level/pan/decay and Stop resets were confirmed by
Jonathan on Windows and Live. The overnight automation, touch, shared memory and
arrangements have automated musical, scheduler and UI coverage here. They still
need the same Windows/Live listening pass. Silent MIDI tests cannot prove audio
latency or that a particular plugin's MIDI mapping is enabled. No new Set or manual
mapping is required for the new prepared-kit examples.

## Rhythmic time extension

[Timing](timing.md) now covers per-Part straight/triplet/dotted clocks, musical gate syntax, independently admitted ratchets/flams, anticipation, tick-based cycle metadata and mixed-grid reference semantics. The compiled resolver caches dependencies and neighboring decisions; file reloads load and compile off the producer thread. The three timing examples await the Windows/Live listening pass.
