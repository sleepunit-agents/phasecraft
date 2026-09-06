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
| `swing` | First subdivision's share of a pair, 0.5–0.75. 0.5 is straight; 0.58 delays odd sixteenths by 38 ticks. |
| `delay_ticks` | Signed offset, -60–60 musical ticks. Negative values anticipate. 960 ticks = one quarter note. |
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

Timing can delay or anticipate hits across source steps. Bar boundaries keep
ownership clean for configuration swaps. Independent subdivisions, ratchets,
flams, gate clipping and exact-grid references are specified in [timing](timing.md).
MIDI clock itself always stays straight.

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

## Meter, space and human touch

The optional next layer adds three composable interpretations:

```toml
[parts.rim.groove]
offbeat_gain = 1.15
after_gap = { steps = 4, gain = 1.25 }
delay_ticks = 12
humanize = { timing_ticks = 8, velocity = 0.08, mode = "phrase_locked" }
```

`offbeat_gain` (0–2, default 1) applies to the eighth-note offbeat, sixteenth
positions 2, 6, 10 and 14 in each 4/4 bar. `after_gap` applies its gain (0–2) when
all of the preceding `steps` (1–32) have no admitted hit in this Part. It crosses
phrase boundaries and does not invent silence before transport zero. Both multiply
the base-velocity contour before the semantic accent boost. Gap context, like run
context, is recomputed under the current configuration after watched edits.

`humanize.timing_ticks` requests symmetric jitter up to 30 ticks around the grooved
onset. `humanize.velocity` requests symmetric base-velocity variation up to 0.5
(0.08 means ±8%). Their keyed rolls are independent of one another, ghosting,
trigger probability and accents. `mode` selects phrase-locked or continuous
occurrences. Changing unrelated Parts does not scramble the rolls.

Timing remains inside the source step. A negative offset is clipped to zero;
choose a small `delay_ticks` if you want room on both sides of a straight onset.
The existing swing/delay bounds plus jitter leave a positive gate, which may be
shortened to end before the next step. These are musical ticks, not fixed ms.
The inspector reports the requested jitter, actual offset, meter/gap factors and
velocity-touch factor. Setting a velocity factor to zero still keeps the admitted
note at MIDI velocity 1; probability controls omission.

Reusable components: `groove.offbeats`, `groove.after_space`, and
`groove.human_touch`. **garage-touch** has exactly the original garage's source
hits/accents with additional interpretation. Compare the two using the same 909
Set. No new mappings are needed.
