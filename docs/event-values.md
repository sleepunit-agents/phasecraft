# Numeric velocity patterns

A voice can read a written velocity shape by time, or advance through it on admitted events.
`examples/quickstart/event-values.toml` uses until-stop's carried seven-value hat emphasis
and **exposed** snare. Its hat trigger is a static stand-in for retained hat-memory.

```toml
[parts.hat.velocity]
pattern = "1.0 0.62 0.78 0.55 0.9 0.6 0.7"
per = "event"
clock = "attacks"
carry = "always"
```

`pattern` currently accepts a flat list of 1..4096 finite numbers in 0..1. The common
notation parser checks its syntax and input bounds; this consumer refuses nested notation,
rests, names, chords, ratchets, and draws. This is the numeric velocity slice of M1.3.
Event-indexed pitch, nudge (M1.10), shared lanes (M1.4), and retained trigger mutation
(M1.5) remain separate work.

Each value multiplies the existing semantic velocity, including accent and groove, before
MIDI's existing round-and-clamp to 1..127. Zero is a minimum-velocity note, not a rest.
Gate admission remains upstream: these values cannot determine the events that advance them.

| field | reading |
|---|---|
| `per = "step"` (default) | Read the value at each child's written position in the time cycle, before timing displacement. Four values across a 16-slot cycle each cover a quarter of the bar. |
| `per = "event", clock = "attacks"` | Read then advance on each admitted structural child. Main first, ratchet tails in order, then an admitted flam grace, regardless of sounding order. This is the default event clock and until-stop's explicit choice. |
| `per = "event", clock = "main"` | Read then advance once per admitted main. Its tails and grace inherit that sample. |
| `cycle = 16` (default) | Sixteenth-note slots. The time-read pattern spans this duration; an event index without carry resets at these cycle lines. It is independent of the Part subdivision. |
| `carry = "always"` | The event index does not reset at a cycle or scene boundary. Absence freezes it. Only valid with `per = "event"`. |

`clock` also requires `per = "event"`. Without carry, entry into a new section/scene
initializes the event index, and subsequent transport cycle lines reset it. An arrangement's
restart/continue policy still determines its musical phase. A router stay is not entry.
The native value-reset field is `velocity.cycle`; the example repository declares the snare's
reset against `trigger.cycle`. A native port writes the corresponding value duration here.
This does not add a `trigger.cycle` alias or change literal trigger duration.

A refused main or burst gate spends nothing for its refused structural attacks. Later
ownership suppression never refunds: an admitted tail cut at a barline has already read a
value. Coincident structural attacks each read before their sounding winner is selected.
The exposed snare's `~ x ~ [x x*3?]`, with velocity `0.95 0.6`, therefore reads
`.95 .6 .95 .6 .95` when the triple opens and `.95 .6 .95` when its gate refuses the tails.
Its next reset cycle starts at `.95` in either case.

`StepTrace.values` records every structural child's zero-based wrapped index and sampled value,
including suppressed children. Each emitted `MusicalEvent.velocity` carries its matching read;
`velocity_gain` includes that multiplier. Existing ornament traces distinguish admission counts,
emission counts and suppression reasons. A coincident merge can remove an event from the final
stream while its owner's structural value reads remain in the trace.

Carried state is keyed by voice ID across arrangement sections and router scenes. Every active
occurrence of that ID must declare carry with the same clock and value count. The gains may
change while keeping that index space. An incompatible or missing velocity declaration on an
active occurrence is a load error; a wholly absent voice freezes its index. Seeking reconstructs
the same history as forward evaluation, including restart sections and repeated arrangements.

The evaluator stores bounded residue checkpoints (4096 per compiled snapshot) and boundary
checkpoints (4096). Eviction causes replay from an earlier checkpoint or the start; it never
expires history or substitutes zero. Memory bounds do **not** bound cold-seek work: replay is
linear in the uncached source opportunities. This is not a transport deadline guarantee; t-494
remains open. Snapshot reload reconstructs history under the new immutable composition; it does
not transfer a live process journal from the previous snapshot. Cross-reload state migration is
not established by this slice.

`voices.<id>.velocity.cycle` can join a return group for time-read values. Event-read velocity
is reported as an event clock and refused as a fixed alignment member, even with periodic reset.
Phase metadata includes a time-read value period and reports unknown common alignment for an
event-driven value clock.

The source contract was written first in until-stop PR #16, including its finite witness
`until-stop/EVENT-VALUES.md`. That witness explicitly specifies admission outcomes; it does not
pretend to be seed-91827 playback. Engine tests additionally cover suppression, coincident
merges, graces, carried absence, restart/continue, random seeks, cache eviction and serialization.
