# Rhythmic time, ratchets and flams

Each Part has its own `subdivision`, defaulting to `"1/16"`. The transport remains
fixed-tempo 4/4 at 960 ticks per quarter. A sixteenth is a **planning window**, not
a restriction on when notes may sound. No cumulative floating-point timing is used.

```toml
[parts.closed_hat]
use = "techno.closed_hat"
subdivision = "1/8T"
trigger.rhythm = { steps = 12, pulses = 10 }
accent.rhythm = { steps = 7, pulses = 3 }
ornaments.ratchet = { count = 3, probability = 0.2 }

[parts.snare]
use = "dnb.snare"
ornaments.flam = { spacing = "1/64T", gain = 0.4, probability = 0.5 }
groove.delay_ticks = -18
```

A note value is `1/1`, `1/2`, `1/4`, `1/8`, `1/16`, `1/32`, or `1/64`, optionally
followed by `T` (triplet, two thirds of the straight value) or `.` (dotted, one and
a half). For example, `1/8T` = 320 ticks; `1/16T` = 160; `1/16.` = 360.
`output.gate = "1/16T"` is musical syntax for `gate_ticks = 160`; choose one spelling.
Gates can request up to 5760 ticks and are shortened to fit their cell, the next
attack, and the bar. This is still a percussion engine, with no ties or note overlap.

A and B and the local accent advance once per Part subdivision; their independent
step counts still produce polymeter. Shared accent lanes retain their own straight
sixteenth clock. Shared accents affect a note only on exact source-grid coincidence.
Part references likewise inspect the referenced Part **at the exact source tick**:
`hits` reads trigger admission, `structural` reads the Boolean rhythm. Neither sees
ratchets, flams, swing or anticipation. A straight sixteenth Part and an eighth-triplet
Part coincide every quarter note, not on their nearest neighboring hits.

The subdivision clock runs continuously. `reset_on_phrase` resets a Euclidean
leaf's phase, not the subdivision clock: at an eligible onset its phase is the
number of subdivisions since the latest phrase boundary. Dotted values can straddle
a phrase boundary; no extra onset is inserted there. Continuous probability uses the
absolute local occurrence; phrase-locked probability uses its musical position within
the phrase. Identical positions get identical decisions. When the grid does not divide
the phrase, eligible positions themselves move between phrase repetitions. Section
`phase = "restart"` starts all clocks again; `continue` keeps absolute musical time.

## Hit expansion

A ratchet divides the source cell into `count` equal intervals (2–8), including the
original hit. Integer division locates each attack from the same origin, without
accumulated rounding. A flam adds one quieter grace hit **before** the main hit.
Its `spacing` must be shorter than the Part subdivision and no longer than `1/16`;
`gain` scales the resulting grace-note velocity, default 0.5. Both inherit the source
hit's accent and control interpretation. Trigger rejection means no main hit or ornaments.

Both ornaments default to probability 1 and `probability_mode = "phrase_locked"`.
Use `continuous` for evolving choices. They have independent seeded addresses:
changing flam probability cannot change ratchet, trigger or accent admission.
Ratchets and flams can coexist. Neighboring expanded attacks are resolved together:
coincident attacks merge into the stronger one; the next attack terminates an older
gate. The admitted ratchet count describes the requested expansion; bar limits can
shorten it. Traces list the main event, `extra_events` and both probability rolls.
`realize()` returns every actual attack in its requested tick window, including grace
notes whose source is in a neighboring planning window.

## Groove and boundaries

`groove.delay_ticks` accepts -60 through 60. Negative values anticipate; the effective
advance is limited to a quarter of the Part's subdivision. Positive timing keeps its
previous behavior, including clipping humanization at the source onset. A negative
delay allows jitter on either side of that earlier onset. Swing acts on pairs of the
Part's subdivisions; after-gap and run-contour lengths count local source positions.
Offbeat emphasis and parameter automation remain tied to absolute musical time.

Bars are clean ownership boundaries. Main hits cannot anticipate into a preceding
bar; an opening grace note that would cross the bar is omitted. Repeats and note-offs
finish inside their owning bar. Thus a source edit or temporary performance change
can take over on a bar without an old anticipated hit or gate leaking into it. This
also applies to the beginning and end of a section. Cross-bar pickups are not yet
supported. Automation continues independently across these bar boundaries.

MIDI is dispatched by event time, including early hits in the preceding planning
window. Controls are sampled through rests and at all ornament onsets/releases.
Note-offs precede replacement note-ons; control releases precede new emphasis at
the same tick. Stop restores the existing kit defaults.

## Examples and inspection

New projects include:

- **triplet-techno**, 132 BPM: straight kick, triplet hats, dotted rim, occasional
  three-hit ratchets, snare flams and a slow cutoff curve.
- **ratchet-breaks**, 172 BPM: early flammed backbeat, probabilistic hat rolls and
  continuously varying triplet rim doubles.
- **dotted-garage**, 132 BPM: swung anchors, dotted rim, triplet open hat and panning.

All use the existing prepared 909 mappings. No new Set or mapping step is needed.
With a plain 909 Core Kit the notes still work; prepared control mappings add the
cutoff/pan changes. Existing projects are never rewritten by an update: create a
new project or copy the compositions and their referenced kit/groove libraries.

`phasecraft inspect PATH --steps 64` shows source ticks, local `cell_ticks`, decisions,
ornament rolls and actual attacks. Several source traces can occupy one planning
window. `sounding` carries that window's audible events for the player; the display
selects reached source positions and interpolates on the Part's clock.
Cycle metadata includes `phase_alignment_ticks`; `phase_alignment_steps` is available
only when that period is an exact number of global sixteenths.

## Execution limits

Playback compiles dependency order and Part indices once per snapshot. Bounded caches
reuse raw decisions and interpreted neighboring cells; edits replace those caches.
`--watch` reads, expands, validates and compiles off the MIDI producer thread, keeping
a single latest candidate. A completed candidate swaps at a planned phrase boundary;
an unfinished load waits for a later boundary. Invalid files keep the last good music.
Changing subdivision, Part topology, routing, tempo or phrase layout requires a restart.

There is still **one musical output port per process**, fixed tempo and 4/4 meter.
Different notes/channels/CC channels can share that port. This release adds no E16
controls, melodic parts, clock following, multiport routing or performance recording.
