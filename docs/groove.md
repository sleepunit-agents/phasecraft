# Groove and the 909 garage example

Groove interprets already-admitted hits. It never creates, removes, or re-rolls
trigger/accent decisions. Part references continue to mean structural eligibility
or admitted hits on the source grid, even when the audible hit is delayed.

Compose reusable behavior components:

```toml
[parts.closed_hat]
compose = ["techno.closed_hat", "groove.ukg", "groove.ramp_up", "groove.ghosts"]
trigger.rhythm = { steps = 16, pulses = 13 }
groove.swing = 0.58
```

Definitions live in `library/grooves/drums.toml`; personal groove components can
use the same ordinary `[library.behaviors."my.name".groove]` structure.

| Parameter | Meaning |
| --- | --- |
| `swing` | First sixteenth's share of an eighth-note pair, 0.5–0.75. 0.5 is straight; 0.58 delays odd sixteenths by 38 ticks. |
| `delay_ticks` | Additional laid-back offset, 0–60 musical ticks. 960 ticks = one quarter note. |
| `run` | `none`, `ramp_up`, or `low_high_low`. Applies to runs of at least three adjacent admitted hits. |
| `ghost_probability` | Independent probability of softening a fired, unaccented hit; default zero. |
| `ghost_gain` | Multiplier for a ghost's base velocity, default 0.45. |
| `ghost_mode` | `phrase_locked` or `continuous`, with its own stable keyed decision. |

`ramp_up` uses base-velocity factors 0.75 / 0.875 / 1.0; `low_high_low` uses
0.8 / 1.0 / 0.8. Longer runs hold the third factor, rather than starting a new
ramp every three hits. Isolated hits and two-hit runs keep their normal factor.
Run context considers two neighbors on each side, using actual admissions and the
current configuration. It spans phrase boundaries and does not invent pre-roll
hits before transport step zero. At a live edit boundary, neighborhood evaluation
uses that event's configuration, not a recorded history of previous revisions.

Contour and ghost factors multiply the profile's base velocity; semantic accent
boost is applied afterward. Accented hits are never ghosted. MIDI velocity remains
1–127, so a gain of zero still emits a minimum-velocity note. To omit notes, use
trigger admission. Ghosting adds quiet articulation to eligible hits; extra snare
chatter in the garage example is explicitly supplied by its trigger expression.

This first timing layer only delays hits. It keeps every note-on and note-off
inside the source sixteenth, shortening the gate when needed. The inspector shows
the requested and resulting gates. This preserves scheduler ordering and clean
phrase reloads; early hits, offsets across steps, triplets and arbitrary
subdivisions remain future work. MIDI clock itself always stays straight.

## Listening

- `examples/studies/groove-straight.toml` and `groove-swung.toml`: the same seeded
  hits and accents, with different timing and touch. Listen to the hats against
  the unchanged kick.
- `examples/quickstart/garage.toml`: a 132 BPM, 2-step-inspired six-Part system for
  Ableton's **909 Core Kit**, channel 10. Snare/clap backbeats anchor the loop,
  non-backbeat snare hits become ghosts, dense hats receive contours, and rim
  percussion avoids admitted kicks. Independent accent cycles keep it moving.

New projects include `compositions/garage.toml` and `patterns/grooves.toml`,
where one shared swing component controls the garage Parts. Existing projects are never
rewritten: copy the example into `compositions/`, then add its relative path to
the `compositions` list in `phasecraft.toml`, or create a new project from the player.

JSON inspection includes onset, gate, ghost roll, run context and velocity factor
on each grooved event. Human inspection includes actual onset ticks and resolved
velocity. The player's Groove detail panel exposes these decisions, and its hit
flash waits until the delayed onset. Rings continue to show the source cycles.
