# Retained rhythm evaluator (M1.5 prerequisite)

The companion's `until-stop/patterns/hat-memory.toml` starts with six hits
in sixteen slots. Every five sixteenth-note slots, its current scene can admit
one swap of a rest with a hit. Several edits accumulate in a pending copy; the
pattern being heard changes only when that copy commits at a cycle boundary.
The Rust `music::process::RetainedSwap` evaluator implements that state transition.

This prerequisite exposes a Rust API only. Composition loading, pattern references,
scene probability lookup, mutation hashing and pins, return-group clocks, inspect
output, and playback integration remain to be wired. `phasecraft` cannot yet play
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
cycle commit. This API does not resolve authored pins or add pin provenance to
inspect; that remains part of the authoring/playback integration.

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
