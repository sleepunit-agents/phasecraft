# Percussion completion plan

Started 2026-09-06 from `3031717`, under Jonathan's explicit unattended goal.
This is a finite implementation checklist. The original v0.1 exclusions are not
all new requirements; the scope below records the later authorized extensions.

## Current audit

Working and user-validated: deterministic multi-Part Euclidean/Boolean rhythm,
independent trigger/accent probability and phase, reusable libraries and overrides,
Part references, TOML projects, live watched playback, desktop player/inspection,
MIDI clock/transport, rolling signed updates, basic procedural groove, multi-control
accents, held values and single ramps, explicit Stop defaults, and the prepared
909 cutoff/level/pan/stock-tail adapter. All original genre anchors remain regression
fixtures. The earlier `spec-audit.md` tables describe historical states.

Missing within the newly authorized scope: multi-stage/curved/cycling control
movement; groove rules based on meter/gaps with isolated deterministic humanization;
shared semantic accent sources and history-dependent control response; reusable
procedural phrase variants and explicit arrangement. Generic realized pattern cycle
metadata and example coverage need an updated audit, not an LCM-sized event buffer.

## Checkpoints

- [x] **1. Control automation:** backward-compatible segments, explicit musical
  durations, linear/smooth/hold curves, optional finite-length cycle. Preserve
  existing single ramps and neutral Stop behavior. Add a prepared-909 breathing
  example, validation, endpoint/reset/tempo/determinism tests and readable traces.
- [x] **2. Procedural groove:** meter/offbeat emphasis, first-note-after-gap response,
  independently keyed timing/velocity variation, reusable profiles and a garage
  comparison. Preserve note admission and bounded scheduling; explain each effect.
- [x] **3. Shared and stateful emphasis:** named shared accent lanes with explicit
  consumers; bounded musical-time accumulation/decay for control emphasis, including
  rests. Keep semantic accents separate from MIDI and RNG addresses local. Use
  existing cutoff mapping for a 303-inspired percussion response; no pitched 303.
- [x] **4. Procedural phrases and arrangement:** reusable A/A2/B definitions with
  overrides, explicit section lengths/repeats and loop policy, quantized transitions,
  phase/probability/automation identity contracts, control cleanup at changes, and
  player section visibility. Reuse procedural Parts, never freeze a giant MIDI score.
- [ ] **5. Coverage and hardening:** current spec-to-code coverage, cycle metadata
  where useful, integrated prepared-909 techno/DnB/garage examples, migration docs,
  input validation and meaningful transport regressions. Keep confirmed examples
  unchanged and do not invent vague density/variation knobs.
- [ ] **6. Delivery:** publish passing cross-platform milestones, verify updater
  artifacts, and write `docs/morning-handoff.md` with listening order, changes,
  any remaining physical checks, and exact release/fixture locations.

Each implementation checkpoint includes its own tests/docs and runnable example.
A failure of one checkpoint is repaired before publication. Later checkpoints may
reuse earlier machinery; the checklist is not permission to add endless features.

## Boundaries and user-dependent checks

No harmony, melody, DSP, E16, AI/MCP, plugin hosting, in-app composition editor,
new controller drivers, arbitrary tempo/meter changes, or general DAW construction.
The existing stock Simpler adapter remains; a new device/gain stage can require a
Live fixture and will be documented rather than guessed. Physical Windows/Live
listening is handed off explicitly; automated event/scheduling/UI validation can
continue here. No existing user projects are rewritten by updates.
