# Generative systems: design proposal

Status: proposed, not implemented. Baseline: `770dfbb`. Written 2026-09-06 for
Jonathan and Art to review. TOML below is a syntax experiment, not a playable file
or a promise of the final schema. Existing compositions remain supported.

## Direction

Compose interacting processes that remain recognizable while developing over long
periods. Standard techno, DnB and garage remain useful regression tests, but are no
longer the primary demonstration of generative capability. A fixed drum note is
enough to prove independent rhythm, attribute, memory and modulation processes.

Near-term priorities: independent parameter lanes; typed reusable modifiers;
structural-return boundaries; deterministic state; explainable long-running studies.
Richer repeat processes follow on the same foundations.

Icebox: live tempo, external clock following, unusual meter, controller work,
melody/harmony, live recording and multi-device performance infrastructure. Transport
remains fixed-tempo 4/4. No need to solve these to make much richer percussion.

## Musical model

| Concept | Meaning |
| --- | --- |
| Part | Enduring role plus output binding; e.g. a fixed-note rim voice |
| Process | Produces triggers, values, or state transitions |
| Clock | Defines when a process advances, independently of the value it produces |
| Source | Constant, sequence, curve, oscillator, held sample, or bounded random source |
| Modifier | Transforms a typed value or makes an explicit admission decision |
| Mutation | Deliberately changes retained state for subsequent evaluations |
| Boundary | Named musical occasion at which a transition may take effect |
| Profile | Interprets semantic emphasis and other annotations for an output |

Rhythm decides when something is eligible to happen. A separate value process
supplies what that event uses. For drums that can be emphasis, gate, velocity or
repeat count; a later pitch source can use the same contract without changing it.
An inactive trigger does not imply every other process stops advancing.

Not everything needs a named top-level process. Constants and local rules remain
local. Promote a source to a name when sharing it, referring to its clock, or giving
its retained state an enduring identity helps explain the music.

## Clocks and sampling

Support two explicitly different advancement modes:

1. **Musical time:** advance on a chosen straight/triplet/dotted interval, with
   independent length and phase. Trigger A, B, accent and value processes may each
   use their own interval. A Part subdivision remains their convenient default.
2. **Events:** advance when a named upstream event stream admits an event. For the
   first version this means admitted main triggers, before ornaments and timing
   displacement. Sounding attacks can become a separate, explicit stream later.

Define event-driven sequence behavior as read-current-then-advance: the first
eligible event receives element zero. Rejected triggers consume no element. Two
upstream events at the same tick consume two elements in stable event-ID order.
There is no recursive same-tick advancement through a cycle of dependencies.

Time-clocked sources update before consumers at the same tick. Sampling a source
between its updates reads its last held value. Source initialization supplies a
value before the first consumer. These semantics let a seven-step value lane run
against a five-step trigger without requiring their steps to coincide.

Source advancement, consumer sampling and MIDI transmission are different events.
Changing an output sampling rate must not change random draws or musical state.
Continuous controls update through rests; event-scoped controls need an explicit
hold/restore lifetime and cannot silently become continuous automation.

## Modifiers: broad reach, typed operations

“Mutator” is a reasonable musical umbrella. Internally distinguish three operations:

- **Value transformation:** change the current value; do not rewrite its base.
- **Admission:** accept or reject an eligible event, repeat, accent or mutation.
- **Retained mutation:** update a pattern/value/state that future evaluations use.

Probability is an admission operation. A random velocity offset is a value
transformation. A random walk is retained mutation. Keeping this distinction avoids
accidentally accumulating what was intended as gentle humanization.

Targets are a registry of musical capabilities, not arbitrary Rust/TOML reflection.
Each declares type, units, bounds, evaluation stage, legal update boundaries and
state/reset behavior. Expose musical parameters broadly; file paths, output routes,
IDs and transport configuration are not modulation targets.

| Target family | Examples | Required interpretation |
| --- | --- | --- |
| Admission | trigger/accent/repeat probability | Bounded 0..1, applied to a named decision |
| Semantic | accent amount | Normalized emphasis before profile interpretation |
| Dynamics | resolved velocity | MIDI 1..127; distinct from level and emphasis |
| Timing | onset offset, gate | Musical duration; signed offset, positive gate |
| Controls | cutoff, level, pan, decay | Normalized musical binding, not raw CC number |
| Rhythm rules | pulses, rotation, operator, repeat count | Integer/enum-aware; structural commits |

Initial operators: add, scale, clamp, sequence, bounded random offset, sample-and-hold
and explicit admission. Then curves/oscillators and bounded random walk. Discrete
choice handles enums: never average XOR and AND or silently float-round an operator.
No arbitrary expression evaluator or embedded scripting language in this slice.

Every random modifier specifies a distribution and update domain. Start with uniform
offsets; expose asymmetric min/max bounds. A random value is addressed by seed,
Part/source ID, modifier ID, occurrence and decision ID. Modifier array indices are
never random identities. A `[[pins]]` entry forces one address's draw before the hash
is consulted (`authoring.md`); the address is the engine's, so a pin follows the
lane's probability mode and never moves another draw. Shared randomness requires an explicit shared source;
unrelated modifiers remain isolated.

Examples of intended semantics:

- Velocity: resolved base 80 plus uniform integer offset -6..4 on each admitted hit.
  Every hit starts from its current base, not the previous randomized velocity.
- Decay: multiply the current authored/automated base by a held factor 0.9..1.1,
  resampling every bar; clamp to the binding's legal range. This is relative variation,
  not an absolute random decay value and not an accumulating walk.
- Timing: a bounded signed musical offset per hit, independently addressed from
  velocity. Convert it once to integer ticks with a documented rounding rule.
- Probability: a slow source supplies trigger admission chance; admission still has
  its own random identity, independent of how that probability was generated.
- Pulse mutation: change pulses only at a declared process-cycle boundary; validate
  `0 <= pulses <= steps` atomically with any simultaneous steps change.

Modifier order is meaningful and visible. Named reusable bundles expand to an ordered
list; local arrays replace inherited arrays, preserving current merge conventions.
Require stable IDs within each list, including expanded bundles; duplicate IDs are
errors. Do not invent implicit array concatenation. Reordering retains each random
draw identity even when changed arithmetic order changes the result.

Evaluation has typed stages, not one unconstrained universal chain:
clock/source updates -> rhythm structure -> trigger admission -> event attribute
sampling -> accent interpretation -> value modifiers -> ornament expansion ->
per-attack modifiers -> timing/gate ownership -> output binding.
Modifiers at different stages cannot be reordered across causal dependencies.
Main-event versus per-attack random variation is explicit; default event attributes
are inherited by ornaments. Current CC accent responses continue to combine with
the moving control baseline under their existing documented rules.

## Structural returns and change boundaries

Distinguish bars, phrase repetitions, arrangement repetitions and structural returns.
“The composition comes around” is ambiguous once sources retain state. A return
means a selected set of structural clocks returns to its starting phase vector;
it does not mean the same MIDI events or accumulated envelope state recur.

Declare the members of a named return group. An explicit whole-composition group
can expand all finite structural clocks, but adding an unrelated slow process must
not silently change an existing named group's period. Show expanded membership.

For fixed time-clocked members with periods `p_i`, the return interval is the checked
LCM in ticks. Existing offsets are part of the starting phase vector; return does
not require every member to be at step zero. Tick zero is the origin, not return #1.
For 16/5/7 sixteenth cycles: 560 steps = 35 bars. Two returns mean 70 bars, not two
four-bar working phrases. A return may occur between bar boundaries.

Include reset policies and subdivision intervals when calculating periods. Existing
cycle metadata is conservative structural alignment, not necessarily the smallest
audible period. Exclude continuous randomness, envelopes and stateful/event-clocked
sources from automatic periodicity claims. A nonperiodic or overflowing group is a
validation error for a return-based schedule; never silently substitute a phrase or
search forever. The author can select a finite structural subset or use bars.

First implementation supports boundaries in bars, phrase repetitions and named
structural returns, and counts positive completed intervals from section entry.
Return-based section duration requires restart-aligned entry initially. Advanced
continue-phase entry will need an explicit inherited origin; reject that combination
until implemented rather than pretend absolute phase and section entry are identical.

At entry, freeze the return group's structural signature and interval for that visit.
Pulses/rotations may change without changing that interval. A period-changing mutation
of a member is rejected while the schedule depends on it. A new section may establish
a new signature. A future adaptive-return policy must be named and separately tested;
otherwise moving lengths can perpetually postpone a transition.

Boundary occurrence and application quantization are separate: exact return is the
default; explicit next-bar alignment may defer the transition. Inspection reports
both requested and actual ticks. Several updates due at one tick commit as a single
validated transaction. No incidental action ordering through file table order.

## State and event lifetimes

Keep pure time-addressable sources pure. Add retained state only for processes that
need it: event-driven counters, held event samples, walks and mutable patterns.
The current resolver reconstructs bounded history from a selected definition. That
does not provide actual cross-section history for a generative state machine.

Compile a dependency graph and typed transition plan outside MIDI dispatch. Reject
instantaneous cycles; later feedback must contain an explicit one-event/one-tick
delay and initialized state. Distinguish this from permissible internal recurrence
inside a bounded random-walk process.

Advance retained state once in chronological planning order. Cache immutable resolved
events for neighboring-window queries and inspection; asking twice cannot advance a
counter twice. Inspect/replay uses an isolated evaluator. Chunk size, lookahead,
telemetry polling and late MIDI drops do not change the logical event history.

State has stable process IDs and definition versions. On a transition, preserve state
only where the process explicitly requests carry and its state schema remains
compatible; otherwise reset from its authored initial state. Incompatible carry is an
error. Stop/Play starts a fresh deterministic run. Existing compositions retain their
current restart/continue semantics; new retained processes must declare their policy.

Checkpoints plus deterministic replay are an implementation path, not a user-facing
seek feature. Budget state size, history, advances per planning window, pending events
and recursive expansion. Long structural cycles must never allocate a full score.
Edits invalidate only the dependent future plan; already dispatched events remain
historical facts. Bound replay work rather than blocking the deadline dispatcher.

Separate musical lifetime from edit transactions. Unchanged playback should eventually
allow cross-bar grace notes, gates and repeat tails. An emitted early attack cannot
be recalled. Updates must choose a commit horizon beyond already committed attacks;
old note-offs retain ownership until completion or an explicit transition truncates
them. Carrying a tail across a transition is distinct from carrying process state.
Keep existing compositions' trimming behavior until an explicit migration/new policy
is available; do not silently change previously validated beats.

## Authoring experiment

Keep familiar keyed Parts and the existing project/library layout. This sketch tests
whether a human can explain a system by reading one Part and its named dependencies.
It intentionally uses small value lists rather than entering every emitted event.

```toml
# PROPOSED SYNTAX — not accepted by today's player.
tempo = 132
seed = 91827
phrase_bars = 4

[parts.kick]
use = "techno.kick"

[parts.hat]
compose = ["techno.closed_hat", "kit.prepared.closed_hat"]
trigger.rhythm = { steps = 16, pulses = 7 }
accent.rhythm = { steps = 5, pulses = 2 }

[parts.hat.processes.touch]
values = [0.6, 0.85, 0.7, 1.0, 0.75, 0.65, 0.9]
clock = { every = "1/16" }
reset = "section"

[[parts.hat.modifiers]]
id = "touch"
target = "velocity"
operation = "scale"
source = { process = "touch" }

[[parts.hat.modifiers]]
id = "velocity_variation"
target = "velocity"
operation = "add"
source = { random = "uniform", min = -4, max = 4, sample = "hit" }

[[parts.hat.modifiers]]
id = "tail_variation"
target = "parameters.decay"
operation = "scale"
source = { random = "uniform", min = 0.9, max = 1.1, sample = { bars = 1 } }

[returns.hat_system]
members = ["parts.hat.trigger", "parts.hat.accent", "parts.hat.processes.touch"]

[phrases.A]
[phrases.B]
parts.hat.trigger.rhythm.pulses = 9

[arrangement]
repeat = true
sections = [
  { phrase = "A", after = { returns = "hat_system", count = 2 } },
  { phrase = "B", bars = 8 },
]
```

This study leaves A after 70 bars. It promises the selected 16/5/7 clocks have returned
twice, not identical touch randomness. The scoped `touch` reference resolves only in
this Part; cross-Part references must use explicit qualified names. A value source
can later use `clock = { on = "parts.hat.trigger.hits" }` with the read-then-advance
contract, but must then be removed from this automatically periodic return group.

For normal use, put these modifiers/processes in a reusable behavior and compose it:

```toml
# PROPOSED reusable behavior; name is illustrative, not shipped.
[parts.hat]
compose = ["techno.closed_hat", "kit.prepared.closed_hat", "my.slowly_shifting_touch"]
```

Do not require every composition to contain the expanded advanced sketch. Avoid
mixing kit MIDI mappings into it. A human should see the baseline, the variation
bounds, when they change, and what causes the next section without tracing raw CCs.

## Structure review and concrete changes

The existing top-level separation is sound: authoring -> musical resolution ->
playback, with host/controller adapters outside the engine. Keep it. Specific seams:

| Current location | Change when the relevant slice lands |
| --- | --- |
| `music/mod.rs` | Move new target/source types into cohesive modules; do not keep expanding Part with one-off attributes |
| `music/rhythm.rs`, `time.rs`, `cycle.rs` | Introduce independent process clocks and named return contracts alongside current rhythm expressions |
| `music/parameter.rs`, `groove.rs`, `accent.rs` | Share typed source/modifier infrastructure, retain musical convenience rules and semantic accent profiles |
| `music/resolve/compiled.rs` | Separate pure compiled plan from chronological evaluator state and immutable event cache |
| `music/arrangement.rs` | Resolve typed boundary specifications, not only pre-expanded whole-bar durations |
| `authoring/syntax.rs`, `library.rs` | Normalize concise syntax and preserve stable IDs/order; validate target paths against the registry |
| `playback/transport.rs`, `reload.rs` | Explicit commit horizon and state transition policy; no file parsing in deadline dispatch |

Likely cohesive new modules are `music/process.rs`, `music/modifier.rs` and
`music/boundary.rs`. Add them as implementations earn them, not an empty framework
or a file per modifier. Keep source definitions together until they become unwieldy.

Project folders stay musical: `patterns/drums.toml`, `patterns/grooves.toml`,
`patterns/accents.toml`, `patterns/parameters.toml`, plus an optional
`patterns/processes.toml` for shared systems. Keep explicit imports. No mandatory
new folder tree, per-note files or automatic discovery. Existing arrays replace,
tables merge, named behaviors compose left-to-right; explain advanced list replacement
clearly and let `expand` expose the result.

Current readability friction is mostly long inline automation/expression tables and
indirection through inherited values. Prefer multiline tables, reusable named musical
behavior, and an effective-parameter report over inventing a DSL. Retain authored Part
order for display without using it as a random or dependency identity.

Inspection additions: target base -> source sample -> ordered transforms -> clamp ->
effective value; random address/roll; advance cause and source position; retained
state before/after; boundary members, interval, count and next commit; carried/reset
state. Errors should report authored file/key and inherited origin, not just an
expanded TOML location. UI should display authored, effective and pending separately.

## Delivery slices and acceptance

1. **Typed modifiers without retained mutation.** Start with velocity offsets and
   per-bar held decay variation, independent IDs and full provenance. Include one
   reusable behavior and an expanded readable equivalent. Preserve legacy traces.
2. **Independent processes.** Time-clocked attribute sequences, independent accent
   rate and event-clocked read-then-advance. Introduce chronological evaluator state
   here, before any random walk. Compare time-driven and hit-driven versions.
3. **Return boundaries.** Named finite groups, counted returns and section changes;
   demonstrate 16/5/7 and a return that is not bar-aligned. No full-cycle allocation.
4. **Musical-parameter modulation.** Curves/held sources for probability, then atomic
   cycle-boundary pulses/rotation/repeat-count updates. Add shared sources explicitly.
5. **Retained development.** Bounded walk, sparse mutation of a retained rhythm,
   freeze/resume and state-carry policies. Keep density semantics behavior-specific.
6. **Event lifetimes and richer repeats.** Cross-boundary ownership, accelerating
   rolls, repeat envelopes and independent repeat duration. Preserve prior policies.

Tests required across slices: identical seeds/history give identical events; unrelated
modifiers and display order do not scramble decisions; changing window/lookahead or
inspecting twice does not advance state; rejected hits do not advance hit-clocked
lanes; shared samples correlate only named consumers; ranges/units/rounding are
checked; cyclic dependencies fail clearly; nonperiodic returns fail without fallback;
return #1 occurs after a full interval; simultaneous transitions are atomic; Stop
cleans up controls/notes; carried tails retain their release; legacy files round-trip
and retain golden events. Measure bounded memory and planning cost in long runs.

New listening studies should expose mechanisms rather than claim automatic styles:

- **Orbit:** straight kick anchoring 16/5/7 trigger/emphasis/touch; change after two returns.
- **Counting rain:** sparse hits advance gate/dynamics; missed triggers preserve the next value.
- **Slow weather:** held decay drift and gradual probability movement with fixed rhythmic anchors.
- **Remembering metal:** a retained percussion pattern mutates sparsely, freezes, then resumes.
- **Returning roll:** a later study of long accelerating repeats and safe section transitions.

Use the prepared 909 kit throughout. Supply small comparison traces plus long renders
of event statistics, and listen across at least several structural returns. Diversity
counts are diagnostics, not a substitute for musical coherence or Jonathan's ears.

## Review questions for Art and Jonathan

- Does the source/modifier/boundary vocabulary explain the system without requiring
  someone to think like the engine? Are `processes` and `modifiers` the right author terms?
- Is explicit return membership plus a frozen per-visit interval the least surprising
  rule when structure changes? Should exact non-bar returns be exposed immediately?
- Does the advanced sketch remain reviewable once inheritance is expanded? Is an
  ordered modifier list with stable IDs sufficient before introducing named chains?
- What state should authors expect to carry between derived phrases? The proposal
  requires explicit compatible carry rather than inferring it from a similar name.

These are review points, not blockers to the design. Do not implement the entire
sketch before validating the first two slices with real compositions.
