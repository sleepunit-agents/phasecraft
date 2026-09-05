# Engineering handoff audit

Audited 2026-09-05 against commit `13e5cbc` and Jonathan's original 49-section
handoff. The handoff mixes immediate prototype requirements, architectural
principles, and explicitly deferred possibilities. Those are separate obligations.
This document is an audit and proposed sequence, not a declaration that every
future feature is now part of v0.1.

Jonathan reports that both the techno and DnB examples work flawlessly with the
909 Core Kit. Earlier, all 60 hits and velocities in his exported hat recording
matched the engine. That provides real-host musical validation in addition to
24 tests and successful builds on four native platforms. It does not establish
high-resolution MIDI/audio latency or every future routing arrangement.

## Follow-up milestone

The subsequent authoring milestone adds built-in/personal libraries, `use` and
component `compose`, nested overrides, named velocity profiles, explicit structural
and admitted-hit Part references, import-aware reload, expanded TOML output, and
readable MIDI explanations. See [the listening guide](../examples/README.md) for
coverage and controlled comparisons. The tables below preserve the original
`13e5cbc` audit rather than silently relabeling its historical findings.

Whole-expression rotation, nested probability nodes, cycle metadata, shared or
stateful accents, multi-control profiles, and advanced groove remain future work.

## Implemented

| Handoff sections | Capability | Evidence |
| --- | --- | --- |
| 1, 7–10, 42, 49 | Rhythm-only live MIDI with Euclidean A/B trigger and independent C accent | `engine::Expression`, `resolve`; `examples/hat.toml` |
| 9–13, 25, 31–32 | Independent cycle lengths, rotation, polymeter, cycle/phrase distinction | Absolute-step evaluation; independent leaf lengths; 560-step tests |
| 10–11 | All five Boolean operators and recursively nested expressions | `BooleanOp`, recursive `Expression::Binary`; tests |
| 14–16, 20, 29 | Independent trigger/accent admission; semantic emphasis; local randomness | Versioned, framed SHA-256 addresses; `Accent`; resolver |
| 18–19 | Phrase-locked and continuous probability; independent reset policy | `ProbabilityMode`; per-leaf `reset_on_phrase`; tests |
| 6, 33, 35 | Realtime looping, seed/config iteration, phrase-boundary changes | `play --watch`; whole-composition validation and replacement |
| 28 | Stable Parts with distinct output routing and velocity interpretation | `Part`, `Output`, `VelocityProfile`; multi-Part isolation tests |
| 30, 35–37 | Integer musical time, fixed-BPM monotonic transport, lookahead, MIDI adapter | 960 PPQN; separate producer/dispatcher; bounded queue and cleanup |
| 38 | Strongly typed model and ordinary declarative files | TOML, validation, legacy `[part]` and new `[[parts]]` |
| 44–45 | Determinism, RNG isolation, rhythm/phase tests, inspectable decisions | 24 tests; JSONL inspection includes rests and recursive rhythm traces |
| 42, 47 | One-Part model extended to a working kit after validation | Three 909 examples; user's Ableton listening feedback |

Some requirements are implemented as functions/data rather than a struct bearing
the exact suggested name. There is functioning transport and phase handling;
creating empty `Transport` or `CycleState` types would not complete a missing
musical feature. Stateless phase calculation is intentional.

## Partial or absent foundations

| Area | Actual state | Useful next work |
| --- | --- | --- |
| Reusable behaviors, namespaces, composition, progressive disclosure (§2–5, 28, 38–39) | No `use`, named library, local overrides, imports, or reusable component composition. Every example spells out its lanes. Only primitive defaults exist. | Add a small explicit library-resolution layer; built-in drum behaviors and a personal TOML library; resolve overrides into the existing typed model. Keep identity at the consuming Part. |
| Accent profiles (§20–23, 28) | Semantic accent is implemented, but the only interpretation is inline velocity base/boost. No named profiles, profile composition, MIDI CC response, state, group/global ownership. | Named velocity profiles are the smallest reuse step. Stateful and shared accents are later work, explicitly optional in the handoff. |
| Generic pattern representation (§12) | `RhythmPattern` has a realized time window and semantic events, but no cycle metadata. `realize` is a library helper; live playback calls `resolve_step` directly. Only Euclidean leaves produce rhythms. | Clarify window/cycle metadata and one shared realization path before adding further generator families. Do not flatten long cycles into giant LCM buffers. |
| Extensible rhythmic algebra (§11) | Recursive Boolean trees work. There are no `PartRhythm`, expression-wide `Rotate`, or nested `Probability` nodes. Leaf rotation and lane probability already work. | Add nodes when a musical example needs them. A Part reference must explicitly distinguish structural eligibility from actual admitted hits and validate dependency cycles. |
| Explainability (§45) | JSONL gives phase, structural decisions, rolls, admissions, semantic events, time, and Part ID. No friendly rendering, library-origin trace, profile/MIDI translation trace, or per-event dispatch outcome. | Add concise human-readable explanation while retaining JSON; show resolved defaults and profile effects. Keep planning and actual dispatch outcomes distinguishable. |
| Reusable phrase identity (§6, 18, 33) | Files and seeds can be saved manually. There is no named Phrase A/A2 derivation or parent/override relationship. | Reuse the library/override mechanism for procedural phrase variants later; no arranger required. |
| Tempo-aware semantics (§30–31) | Tick-to-time conversion works. Gate is specified in ticks; no higher-level musical gate intent or tempo-aware profile state. | Keep raw ticks available; add an intent parameter only when multiple behaviors establish useful semantics. |
| Controller-independent parameters (§41) | All parameters already exist in files without any controller coupling. No separate runtime parameter API or macro mappings. | Preserve that separation; E16 implementation remains deferred. |

## Current practical limits

- Fixed 4/4 and one global sixteenth grid. Independent cycle lengths do not mean
  independent clock subdivisions, triplets, or microtiming.
- One output port per running process; Parts can vary channel/note on that port.
- 1–32 Parts; unique `(channel, note)` routes; gates shorter than one step.
- No live tempo/phrase-length changes, pause/resume, seek, MIDI clock, or external
  transport sync. The handoff's fixed-tempo first slice and phrase reload target
  do not require those additions yet.
- Live edits are deterministic given the same edit history, but that history is
  not recorded. A seed alone cannot replay changes made during a performance.
- Stateful profiles and overlapping notes would require revisiting the current
  scheduling/reset contracts. Keeping a placeholder name would not solve that.

## Existing features that need better example coverage

The three shipped examples are useful grooves, not a complete feature tour.

| Capability | Current example coverage |
| --- | --- |
| XOR, OR, A_NOT_B | Present in hat/DnB |
| AND, B_NOT_A, deeply nested trees | Unit-tested; no dedicated example |
| Unequal cycles and positive rotation | Present |
| Negative rotation | Unit-tested; no dedicated example |
| Phrase reset versus continued phase | Unit-tested; examples use continued phase |
| Phrase-locked versus continuous probability | Unit-tested; examples use phrase-locked rolls |
| Trigger and accent probability isolation | Both in `hat.toml`; no paired listening examples demonstrating locality of change |
| Semantic accent amount and velocity profiles | Present; no controlled profile comparison |
| Phrase reload and Part addition/removal | Tested manually; no guided example exercise |
| Seed variation with stable Part IDs | Supported; no paired example set |

## Proposed 909 example suite

Keep the validated `hat`, `techno`, and `dnb` files as regression/listening anchors.
Add small focused examples with short listening notes and deterministic assertions,
then one complete groove that combines the demonstrated features.

1. **Rhythm algebra:** paired percussion lanes demonstrating all five operators,
   unequal cycles, nested expressions, and signed rotation over a fixed kick.
2. **Phase/reset pair:** same five/seven-step processes and four-bar phrase,
   differing only in reset policy; document what changes at each phrase boundary.
3. **Probability pair:** same seed and structural rhythm, phrase-locked versus
   continuous admissions. A companion accent-only edit must preserve trigger hits.
4. **Emphasis pair:** identical note positions, different semantic amount/profile;
   use named profiles once implemented.
5. **Reusable techno/DnB:** short `use`-based files plus meaningful overrides,
   equivalent to the existing explicit examples. A personal component adds a
   variation without copying the whole beat.
6. **Interlocking kit** (after Part references): percussion avoids actual kick
   hits, with explicit semantics for kick probability; prove dependency order
   does not change the result.
7. **Combined showcase:** a coherent 909 groove using the completed features,
   with a coverage table pointing to each participating Part/component.

Every example should have a clear audible purpose. Supported does not mean every
operator has to be forced into one crowded groove. Keep the original two genre
anchors simple.

## Deliberately deferred by the handoff

Harmony/scales/chords/voice leading, melodic or bass generation, arrangement,
AI/MCP, E16, GUI, DSP/hosting, Ableton project manipulation, tie/slide/legato,
advanced groove, full 303 behavior, automatic style generation, and phrase mutation
are explicitly outside the first build (§7–8, 24, 26–27, 33–34, 40–41, 46).
Group/global and stateful accents are architectural considerations, not mandatory
first-prototype features (§22–23). Density and universal variation are deliberately
unsettled (§17, 34); neither should be introduced as a synonym for probability.

## Recommended sequence

First close the example-coverage gaps using existing capabilities. Then introduce
reusable behaviors, overrides, and named velocity profiles: this is the largest
remaining gap between the working engine and the intended authoring experience.
Next earn cross-Part rhythm references with an interlocking 909 example, while
strengthening generic realization/provenance as needed. Shared/stateful accent,
phrase derivation, and broader timing should be separately scoped extensions.

### Project authoring follow-up

Project scaffolding (`new`), a neutral multi-composition manifest, separate MIDI
configuration, shared pattern/kit files, keyed Parts, rhythm shorthand, and
whole-project validation are implemented. Musical source is grouped into drums,
accents and kits; internal source separates authoring, music and playback.
These are organization/authoring changes, with golden pre-refactor traces retained.
They do not implement arrangement, groove, stateful accent or 303 control behavior.
See `authoring.md` and `architecture.md` for the current contracts and paths.
