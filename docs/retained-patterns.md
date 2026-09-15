# Retained rhythm sources (M1.5 prerequisite)

The companion's `until-stop/patterns/hat-memory.toml` starts with six hits
in sixteen slots. Every five sixteenth-note slots, its current scene can admit
one swap of a rest with a hit. Several edits accumulate in a pending copy; the
pattern being heard changes only when that copy commits at a cycle boundary.
The Rust `music::process::RetainedSwap` evaluator implements that state transition.

This prerequisite exposes Rust APIs only. `process::retained::RetainedPattern` now
prepares the source file, validates its scene probabilities and drives addressed
draws through the existing resolved `Dice` interface. Composition loading, pattern
references, return-group clocks, CLI inspect output, and
playback integration remain to be wired. `phasecraft` cannot yet play
an authored `hat-memory` pattern. Existing examples retain their static stand-ins.

## Clock and boundary contract

Call `advance` once for every absolute transport sixteenth, beginning at slot zero,
including slots in scenes with no reader. The caller supplies the incoming scene's
probability and addressable draws. Each call publishes any pending copy at its
application boundary, then considers that slot's mutation opportunity:

- `Apply::Cycle` commits at multiples of the initial material's length.
- `Apply::Bar` commits at multiples of sixteen slots (native 4/4).
- Mutation opportunities occur at slots `0, every, 2 * every, ...`. Their
  zero-based occurrence numbers include suspended and refused opportunities.
- `None` for probability suspends without drawing. `Some(0.0)` draws admission
  and refuses. A pending copy still commits on entry to a suspended scene.
- Admission uses `roll < chance`. Refusal draws no selection values. Admission
  draws `RestIndex`, then `HitIndex`; each uniform roll selects an index in the
  corresponding ascending list over pending material, or committed material if
  there is no pending copy. Swapping preserves the initial hit count.

For a five-slot mutation clock and a sixteen-slot cycle, the first cycle's
opportunities are at 0, 5, 10 and 15. Their edits become audible at slot 16.
At slot 80 both clocks coincide: the old pending copy commits first, then
opportunity 16 edits the next pending copy. That edit becomes audible at slot 96.
Two admitted swaps can undo each other; a commit does not necessarily change the
set of sounding slots.

`material()` returns only the committed slots. `pending()` exposes the pending copy
for diagnostics. `MutationStep` reports the transport slot, optional occurrence,
commit flag, outcome, and the actual selected material slots and rolls on a swap.
The swap's `rest_index` and `hit_index` are zero-based positions in the ascending
lists **before** that edit, distinct from `rest_slot` and `hit_slot` in the material.
An absent pending copy differs from a pending copy equal to the audible pattern.

## Forced selection indices

The companion's mutation pins name list indices, while `advance` accepts rolls.
The Rust helper `mutation_index_roll(index, count)` bridges those representations:
pass the eligible rest count for `RestIndex`, or hit count for `HitIndex`. For
hat-memory these counts stay ten and six because swaps conserve the hit count.
The helper accepts counts 1–4096 and indices below the count; invalid inputs return
an error. It chooses `(index + 0.5) / count`, the bucket midpoint. Using `index / count`
can round below the intended bucket (for example index 15 of 22 selects 14).

All valid index/count pairs through 4096 are checked for exact recovery by the
evaluator's floor-of-roll-times-count selection. A 44-slot regression also checks
the reported indices and actual pending slots across accumulating edits and a
cycle commit. `RetainedPattern::bind_pinned` below resolves authored mutation pins for the source
API. Composition and inspect integration remain later work.

## Bounds and integration obligations

The evaluator accepts 1–4096 initial slots with at least one hit and one rest, and
an opportunity interval of 1–65536 slots. It retains at most two material vectors,
not a history. Non-opportunity calls allocate no material; a swap scans at most
4096 slots. The first pending edit after each commit copies the current pattern.
Probabilities must be finite in 0..1; draws must be finite in [0,1).
Invalid input leaves the evaluator unchanged, including a coincident commit.
Caller-side draw effects cannot be rolled back, so integration must use stable
addresses rather than advance a sequential random generator.

The caller owns scene traversal and transport order. Random seeks require replay
from the initial state or a valid cloned checkpoint, with the same scene and draw
history. No seek cache, realtime replay bound, or state migration on score edits is
provided here. Shift mutation, flat empty/full patterns, and exported counts are
outside this swap prerequisite.

`tests/retained_swap.rs` checks the six-hit material, pending accumulation,
commit-before-mutation, bar versus cycle application, suspension versus zero
probability, ascending selections, rollback on invalid input, and hit conservation
through 10,000 slots with scene changes. These are evaluator results, without MIDI,
hardware, or listening evidence.

## Prepared source and addressed draws

`music::process::retained::RetainedPattern` deserializes the companion's closed
hat-memory shape directly:

```toml
initial = "x ~ x ~ ~ x ~ x ~ ~ x ~ x ~ ~ ~"
elsewhere = "hold"
carry = "always"
[change]
every = { slots = 5 }
chance = { crowded = 0.23, exposed = 0.05 }
moves = ["swap"]
apply = { on = "cycle" }
```

This is a source-file API, **not a Composition table or a playable project**.
Initial material accepts only whitespace-separated `x`/`~` sixteenth slots with
both a hit and a rest; nested notation, probabilities, repetitions and ornaments
are refused. All fields above are required, all unknown fields are errors, and the
only policies are `hold`, `always`, exactly one `swap`, and bar/cycle application.
The evaluator bounds above apply. `chance` may be empty and has at most 64 named
scenes with finite probabilities in 0..1.

After deserialization, `pattern.bind("hat-memory", &["crowded", "hollow", "exposed"])`
validates the complete scene vocabulary (1–64 unique, nonempty names) and returns
an independent `RetainedSource` starting at transport slot zero. A chance entry
naming an undeclared scene is an error. Call `source.advance(scene, dice)` exactly
once per transport sixteenth, including silent scenes; readers use
`source.state().material()` after that advance. Multiple readers must share that
one advance. A misspelled runtime scene is an error, while a declared scene absent
from `chance` is suspended. Binding anew starts anew; cloning a source preserves
its exact checkpoint. The source never resets itself on a scene change.

The native versioned hash receives the tuple
`(seed, pattern name, "mutate", transport opportunity, decision)`, where decision
is `admit`, `rest_index` or `hit_index`. Scene names and file paths are not random
addresses. The pattern name and seed/pins must be preserved during replay.
`RetainedSample` records the name, scene, effective optional probability, mutation
step, and only the draws actually requested in admission/rest/hit order. Each draw
retains its `Dice` pin-list index. The sample holds at most three draws; the source
stores no sample history. Invalid scenes/draws leave state and pending commits
unchanged.

The existing `Dice` API can supply **manually resolved** mutation addresses to this
source. Numeric admission pins still use `u < chance`: even pinned `u = 0` cannot
admit at chance zero. Selection pins use `mutation_index_roll` with the conserved
rest/hit counts. The Composition `[[pins]]` resolver still does not accept mutation
addresses; the mutation-only source API below prepares them separately.

`tests/retained_source.rs` uses a verbatim hat-memory fixture from until-stop
`0a72b1480e6a244078bf162e93e9778e0a0c64f2`. It covers source round trips, a replayed
scene journey, preserved addresses across absence versus zero probability, manually
resolved pin 52 under different seeds, pin provenance, boundary rollback and closed
shape/scene validation. This is native hash/evaluator evidence, not agreement with
the companion's historical `TRACE.md` or MIDI/listening evidence.


## Authored mutation pins (source API)

Deserialize `process::retained::pins::MutationPin` from the companion shape:

```toml
at = { roll = "mutate", pattern = "hat-memory", tick = 52 }
admit = true
rest_index = 2
hit_index = 0
```

`pattern.bind_pinned("hat-memory", scenes, &pins)` prepares a `PinnedSource`.
It accepts a mutation-only slice of at most 256 entries for this one pattern;
loading the companion's mixed `seeds.toml` or Composition pins remains later work.
The address and at least one draw field are required. Unknown fields, another
pattern, an index outside the conserved rest/hit counts, an unreachable transport
slot and duplicate draw addresses are errors. Distinct fields at one tick may be
split across entries. A tick is a zero-based transport opportunity, not a slot or
count of successful mutations. Source shape and pins are bound together at load.

Boolean admission is numeric sugar: `true` resolves to `u = 0.0`, `false` to the
largest representable `f64` below one. The scene still decides `u < chance`.
Thus true refuses at chance zero, false admits at chance one, and suspension
consults neither. Selection indices resolve to the checked bucket midpoints above.
A false admission together with selection fields produces a `SelectionUnderFalse`
lint, including when fields are split across entries; it remains legal because
those selections can land at chance one. Selection-only pins are legal too.

Call `source.advance(scene, seed)` on every transport sixteenth with a stable seed.
Its `PinnedSample.sample` contains the normal scene, chance, mutation and draws.
Here `Draw.pinned` indexes the **authored mutation slice**, so three fields in one
entry all carry the same index. `landings` lists each pinned field at this
opportunity with `drawn = false` when suspension or admission refusal skips it.
A consulted boolean whose spelling disagrees with the result sets
`endpoint_mismatch`; the enclosing sample names the actual scene and chance.
Not-due steps have no landings. No pin history is stored: consumers must aggregate
landings across their inspection window, including untouched future/past pins.
This is the data seam for t-508; CLI zero-landings reporting is not wired here.
Cloning preserves a replay checkpoint. Unknown scenes leave it unchanged.

`tests/retained_pins.rs` covers pin 52 through multiple histories, exact selection
and authored provenance, all six admission endpoint/interior cases, skipped
fields, split-entry lints/duplicates, scene rollback, replay and load validation.
These are source API results, not MIDI or listening evidence.
