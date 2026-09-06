# Phasecraft overnight handoff

The finite percussion checklist is implemented. Update the Player, choose **New
project** with a fresh folder, and load the existing **Phasecraft 909 Prepared.als**.
The new starter project has fifteen compositions. Your existing projects are not
rewritten by updates, and no new Ableton mappings are needed for these examples.

## Listen in this order

1. **techno-journey** — 132 BPM, 32 bars (about 58 seconds). Opening → A → breakdown
   → A2 → closing; it stops automatically. Start here for the combined feature tour.
2. **dnb-journey** — 172 BPM, 32 bars (about 45 seconds), with the proven DnB core.
3. **garage-journey** — 132 BPM, 32 bars, with swing, ghosts and percussion that
   avoids actual kick hits. All three use cutoff, level, pan and stock decay/tail.
4. **breathing** — give it sixteen bars for its complete smooth open/hold/close/hold
   motion. **accent-memory** isolates shared seven-step emphasis and accumulating
   cutoff response. Both loop continuously.
5. Compare **garage-touch** with **garage** for the same admissions with different
   meter/gap/timing/velocity interpretation. **sections** isolates A/A2/B derivation.

The original techno, DnB and garage anchors remain unchanged. Set arrangement
`repeat = true` in a journey to turn it into a performance loop. MIDI clock/transport
is still an explicit checkbox; match Live's tempo manually when it is off.

## Where to change things

- `compositions/`: the musical differences, named phrases and section sequence.
- `patterns/performances.toml`: shared motion and accent-memory components.
- Other `patterns/` files: common drums, groove, accents and opening behavior.
- `kits/909-prepared.toml`: note/CC bindings and neutral defaults. Level is separate
  from note velocity; decay is the stock kit's supported decay/release mapping.
- `config/midi.toml`: machine-specific destination and transport preferences.

`ARRANGEMENT.md`, `PARAMETERS.md` and `ACCENTS.md` are included in a new project.
Use `phasecraft expand` to inspect inheritance or `phasecraft inspect --human` to
explain events. Musical edits can reload while playing; section layout, routing,
tempo and phrase length changes require stopping and restarting.

For an existing personal project, create a fresh sibling project and copy only the
new files you want, then add their paths to its manifest. The journeys also need
`patterns/performances.toml` in the manifest's libraries list and the prepared kit
bindings. Keep your own MIDI configuration and personal library edits. Alternatively,
the release's `examples/quickstart/*-journey.toml` files are standalone and include
their definitions. Do not place those standalone copies inside a project that also
imports the same library names; use the compact project versions there.

## Distribution and remaining listening checks

- [Rolling dev release](https://github.com/sleepunit-agents/phasecraft/releases/tag/dev)
- [Windows Player installer](https://github.com/sleepunit-agents/phasecraft/releases/download/dev/phasecraft-player-windows-x64-setup.exe)
- [Current coverage and deliberate deferrals](current-coverage.md)
- Existing prepared Set: `group-booth/phasecraft-909-prepared-v1.zip` on drop.
- Ready-to-open fifteen-composition project: `group-booth/phasecraft-percussion-examples-1ea8fee.zip`
  on drop. Unzip, then Open project → `PhasecraftPercussion`. ZIP SHA-256:
  `2879f21cd286c5afe3798ade26769fa7338fba3e8bdfa9c9ecfa097bf64ee9e8`.

The implementation checkpoint is `1ea8fee`; release verification is recorded below.
The updater displays the full build identity from the published feed, including
subsequent handoff/documentation commits.

Windows/Live listening is the remaining physical check: audition the three journeys,
Stop during a sweep, switch back to an original beat, and confirm the kit returns to
its normal mix/tone. Original beats and the prepared kit/reset behavior were already
confirmed by Jonathan. New overnight features have event, timing and UI coverage here;
silent MIDI tests do not measure audio latency or replace listening judgment.

No harmony, melody, DSP, E16, AI/MCP or in-app editor was added. Shared finite-memory
emphasis is a MIDI control behavior, not a pitched or analog-modelled 303.

## Implementation and validation record

## 1. Breathing automation

Implemented segments with linear/smooth/hold curves, fractional musical durations,
delayed starts and independent repeating cycles. Existing ramps and Stop defaults
remain compatible. New project example: **breathing**, using the existing Prepared
Set. Listen for sixteen bars to hear the complete open/hold/close/hold cycle.
Engine: 80 tests pass, including exact boundaries, repeat equality, inherited ramp
replacement, invalid durations and unchanged original 35-bar provenance.

## 2. Meter, space and touch

Implemented offbeat emphasis, first-hit-after-gap gain and isolated deterministic
timing/velocity humanization. Compare **garage-touch** with **garage**; source
trigger and accent decisions are identical. No kit changes needed. Timing stays
inside the source step and never affects MIDI clock. 83 Rust tests pass, including
RNG isolation, phase boundaries, context, bounds and the paired-example comparison.

Automation release `3d1864a`: all native jobs passed; published updater commit, four
signed targets and Windows installer checksum verified. Groove release `fdda262`
also passed all platforms; published feed and Windows checksum verified.

## 3. Shared emphasis with memory

**accent-memory** uses one named seven-step accent lane across hat and rim. Each
Part's cutoff profile accumulates recent accented hits and decays through rests;
Stop returns to the kit default. It uses the existing Prepared 909 Set. Source
accents combine by maximum amount, and never create notes. This is a control
envelope inspired by the accent-memory requirement, not a pitched 303 emulation.
New tests cover shared decisions, no-note behavior, finite decay/accumulation,
onset timing, query-order independence, RNG locality and maximum bounded load.

Accent-memory validation: 88 Rust tests, eight browser checks, root/desktop clippy,
and native Linux player smoke passed. Four bars: 124 note messages, 202 CCs,
384 clock pulses, zero dropped notes; maximum measured lateness 2.092 ms locally.

Shared-accent release `e8c38d9`: all platform jobs passed after retrying Intel Mac
DMG packaging. Published commit, four signed targets and Windows checksum verified.

## 4. Procedural sections

**sections** plays A → A2 → A2 → B over sixteen bars. A2 inherits A, including its
opening cutoff; B removes kick/clap. Section changes restore outgoing control
defaults before initializing the next phrase. The Player shows the audible section.
See `arrangement.md` for finite endings, repeats and explicit restart/continue clocks.
The sixteen-bar dry run sent 436 note messages, 965 controls and 1,536 clocks with
zero drops, maximum measured lateness 1.022 ms. These are local scheduling checks;
listening on Windows/Live is still yours.

## 5. Combined examples and coverage

New projects contain fifteen compositions. **techno-journey**, **dnb-journey** and
**garage-journey** each play 32 procedural bars and stop, combining shared control
motion, accent memory, groove and named sections. Their main A sections preserve
the original genre's trigger decisions. See `current-coverage.md` for the updated
spec map and explicit deferred work. Cycle spans now describe a realized window's
phase alignment without materializing long common periods.

102 Rust tests pass, including complete-project/standalone equivalence, original
35-bar golden traces, arrangement watch validation and absent-Part inspection.
Root and desktop clippy pass; three scoped Ableton fixture tests pass. Full DnB
journey at 172 BPM: 1,272 note messages, 2,927 controls, 3,072 clocks, no dropped
notes, maximum measured lateness 5.828 ms locally.

The initial section CI exposed a test-design problem: a 400 BPM exact-clock test
required shared Mac runners to meet real-time deadlines. The engine correctly
stopped when they did not. Exact counts now use simulated deadlines; a separate
real-time check covers finite transport duration and cleanup. Runtime clock safety
was not weakened. That unsuccessful intermediate commit was not published.

Full garage journey at 132 BPM: 1,840 note messages, 3,827 controls, 3,072 clocks,
no dropped notes, maximum measured lateness 6.020 ms locally.

Full techno journey at 132 BPM: 928 note messages, 2,331 controls, 3,072 clocks,
no dropped notes, maximum measured lateness 6.926 ms locally. All three full journeys
ended cleanly. Native Linux smoke also passed automatic finite ending, section
visibility, restart from Intro, and close-during-playback cleanup. Eight browser
checks and two ring unit tests passed at the section checkpoint; the later coverage
checkpoint changed examples, metadata, docs and native tests, not browser behavior.

## Delivery verification

Implementation release `1ea8feee0540e8c79305816e5f3d7b8696b35242` passed all eight
platform jobs and publication in [CI run 34010876143](https://github.com/sleepunit-agents/phasecraft/actions/runs/34010876143).
The downloaded updater feed names that exact commit and four signed platform
targets. The Windows installer checksum matches SHA256SUMS, and its Minisign
signature verifies cryptographically against the public key embedded in the Player.
Windows installer SHA-256 for this checkpoint:
`ae9165fb605f2d973d35042305be6c55f5390388c9fc9e78a9945b249a25cd26`.
Subsequent documentation builds carry their own commit/checksum; use their published
manifest rather than comparing them to this checkpoint's digest.

The example ZIP was created from a generated project, validated as fifteen
compositions and checked for ZIP integrity. No private recordings or Ableton source
fixtures were added to Git. No existing user project was modified.

The finite checklist is complete. The only remaining validation is the explicitly
handed-off Windows/Live listening pass; the longer-term exclusions in
`current-coverage.md` remain future decisions.
