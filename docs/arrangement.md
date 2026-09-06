# Procedural phrases and sections

A composition still loops forever unless it declares an arrangement. Parts at the
root are the shared starting point. Named phrases contain only their differences;
`use` derives one phrase from another. They remain rhythmic systems with stable
Part IDs and seeds, not recorded MIDI clips.

```toml
tempo = 132
seed = 91827
phrase_bars = 4

[parts.kick]
use = "techno.kick"
[parts.closed_hat]
use = "techno.closed_hat"

[phrases.A]
[phrases.A2]
use = "A"
parts.closed_hat.trigger.probability = 0.75
[phrases.B]
parts.kick.trigger.probability = 0.0

[arrangement]
repeat = true
sections = [
  { phrase = "A", bars = 8 },
  { phrase = "A2", bars = 4, repeat = 2 },
  { phrase = "B", bars = 4 },
]
```

This is a twenty-bar cycle. Each entry's `repeat` creates separate visits to the
phrase. Omitting `bars` uses that phrase's `phrase_bars`. Omitting arrangement
`repeat` plays the sequence once and stops at its end, including trailing rests.
CLI `--bars` may end it earlier. Play starts from the beginning again.

## Phase is explicit

A section defaults to `phase = "restart"`: its musical clock starts at zero. All
its cycles, probability occurrence IDs, groove context, accent memory and automation
use that clock. Repeating A with the same seed makes the same performance. Independent
five- and seven-step processes still move freely within an eight-bar section unless
their own `reset_on_phrase` requests a shorter reset.

`phase = "continue"` evaluates the incoming phrase at the absolute transport
position. This applies to **all** its musical processes: continuously cycling
rhythms, continuous probability, touch, accent history and automation. Phrase-locked
probability still takes the position modulo that phrase's length. A ramp whose end
is already past stays at its final value. This mode does not mean “resume where this
particular phrase last stopped.” Section names are labels, not extra RNG keys.
Give A2 its own `seed` when you want new probability decisions.

History and nearby-note rules are reconstructed from the selected phrase's rules,
just as after a watched edit. Continue does not replay the previous section's actual
accent impulses; it reconstructs the incoming definition's bounded history at the
current time. Restart has no history before its beginning. Run-based groove may
look ahead within the selected definition, even at its final section step.

## Control ownership and transitions

At a section boundary, outgoing active parameter/profile bindings send their
explicit kit `default`, or their configured initial value if no default exists.
Incoming held controls then initialize at the same tick, before incoming notes.
Only those declared active bindings are reset, including a profile that happened
not to fire. Merely listing an unused output binding does not claim it. Drum gates
end within their source sixteenth, so notes cannot hang across the boundary.
The MIDI clock continues without extra Start/Stop messages between sections.
Normal Stop still restores the defaults of controls actually sent during playback.

Use the same kit defaults throughout a project. The prepared 909 library already
declares them. Different phrases can use different Part sets via the explicit
`[[phrases.B.parts]]` array form; keyed tables normally merge with the base.
A Part `use` selects a fresh behavior; ordinary nested overrides merge. Tempo stays
fixed across the arrangement. Phrases may override seed, phrase_bars, Parts and shared
accents; they cannot introduce nested arrangements or per-phrase imports.

## Editing and inspecting

The Player displays the audible section, bar within section, cycle and phase policy.
Each inspect trace carries the same section identity and global musical position.
`phasecraft expand` shows fully resolved procedural sections and can round-trip them.

Watched musical edits apply at the root `phrase_bars` boundaries. Changing section
order, length, repeat policy, phase policy, phrase length, Part IDs or output routing
requires Stop and Play. Invalid edits keep the last valid version running.

Limits: 64 named phrases, 16 inheritance levels, 64 expanded sections and 65,536 bars
per arrangement cycle. No LCM-sized score is materialized.

Try `sections.toml` in a newly created project with **Phasecraft 909 Prepared.als**.
It opens A's hats, derives a busier A2 with moving rim pan, then strips the kick and
clap for B. No new Ableton mappings are needed. Existing project folders are never
rewritten by updating the application.

Subdivisions, ornaments and anticipation obey the [timing boundary rules](timing.md): source clocks can continue independently, while each bar owns and finishes its audible attacks. Subdivision changes require restarting playback.
