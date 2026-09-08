# Pitch, step 1: the note as a held value source

Status: implemented (TO-PHASECRAFT D9 milestone 1, row M1.8). Written 2026-09-08 (Art)
from `until-stop/voices/sub.toml`, `voices/metal.toml` and the two scenes that override
them. This is the first consumer of value patterns; the parser and its grammar are
[notation.md](notation.md), and the sampling model is [generative systems](generative-systems.md)
§ "Clocks and sampling". Step 2 — `key = { root, mode }`, degrees, chords, note ownership
across bars — is designed in TO-PHASECRAFT § E and not built.

## Two lanes on a Part

```toml
[parts.sub]
kit = "sub"                          # output.note = 48 from the kit
[parts.sub.trigger]
rhythm = { steps = 16, pulses = 2 }
[parts.sub.note]
pattern = "<c1 c1 eb1 bb0>"          # one note per bar, four-bar cycle

[parts.metal.pitch]
pattern = "<0 0 -5 7>"               # semitones from the kit note, one per bar
```

| lane | tokens | replaces / offsets | initialisation |
|---|---|---|---|
| `note.pattern` | note names (`c1`, `eb1`, `f#-1`; Live's octaves, `c3` = 60) or integers 0..=127 | replaces `output.note` | the kit note |
| `pitch.pattern` | integer semitone offsets, −127..=127 | offsets the note | 0 |

Both accept `cycle_bars` (default 1, at most 1024): the pattern string is one cycle of that
many bars. `sounding note = note + pitch`, both sampled at the attack's tick, and every value
that arithmetic can produce is proven a MIDI note **at load**: the check runs over every
token the lanes can ever produce (each `< >` and `{ }` element, not a sampled cycle) plus
the kit note, which a note lane holds before its first value. A pattern that can leave
0..=127 refuses with the offset and the note that leave it.

## Sampling: a held source, per attack

A value lane is a **held value source**: sampling it at tick *t* reads the value of the last
event at or before *t*. Before the lane's first value ever, it reads its initialisation. A
cycle with no event at or before the position reads the last event of the nearest earlier
cycle that has one — the value is *held across the cycle line*, which is what makes
`~ eb1` on a voice that hits on beat 1 read the kit note in bar 1 and `eb1` from bar 2 on.
The walk back is bounded by the pattern's period (a full silent period proves the pattern
silent everywhere) and by 4096 cycles when the period is longer or unrepresentable.

Every **sounding attack** samples at its own tick: the main hit, each ratchet tail, a flam
grace. A grace that lands before a value boundary sounds the earlier value and its main hit
the later one. The lanes are sampled where every attack has its final tick — after groove
offsets and ornament expansion — and the result is stamped on the event as `note`, which is
what `to_midi` sends and what `inspect` prints. A Part with neither lane carries no `note`
field on its events, so an unpitched piece's trace and MIDI are byte-identical to before.

Under an arrangement or a router the tick is the section's or scene's musical tick, the
same clock every other decision in the Part reads. A scene overrides a lane the way it
overrides anything else: `[scenes.hollow.parts.sub.note] pattern = "c1"` replaces the
string and keeps `cycle_bars`.

The Part's identity is still its kit note: the duplicate-route check
(`Parts must use distinct MIDI channel/note pairs`) reads `output.note`, and the desktop
kit display shows it. Playback tracks sounding notes by the bytes it sent, so a pitched
Part's note-offs and Stop cleanup follow the sounding note, not the kit note.

## Closed, on purpose

The lane refuses at load, naming the string and the token:

- a hit (`x`), a name, a non-integer, a note outside 0..=127, a note name in `pitch`;
- a **stack** (`[c1,eb1]`): a chord, and a Part sounds one note per attack until D9 step 2;
- `*n` or `?`: ratchets and draws belong to the trigger pattern — a value pattern says
  *what*, never *when*;
- any field but `pattern` and `cycle_bars` — in particular `per = "event"`, the
  read-then-advance clock, which is M1.3's contract (`music/process.rs`) and will subsume
  this positional read rather than be bolted onto it here.

Euclid (`c1(3,8)`), `< >`, `{ }%n`, `@w` and `[ ]` are all accepted: they only place values.
