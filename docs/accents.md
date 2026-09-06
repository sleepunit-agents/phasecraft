# Accent beyond velocity

Accent remains semantic emphasis (`active`, `amount`), with its own rhythm and
probability. A profile now interprets that emphasis as velocity plus up to eight
named, normalized control responses. MIDI assignments belong to the output:

```toml
[parts.rim]
use = "techno.rim"
profile.use = "accent.filter_punch"
output.controls.filter = { cc = 20, channel = 16 }
output.controls.envelope = { cc = 21, channel = 16 }
```

The built-in profile lives in `library/accents/controls.toml`. Personal profiles use
the same library mechanism:

```toml
[library.profiles."my.punch"]
base = 72                         # base velocity
boost = 30                        # emphasis velocity contribution
controls.filter = { base = 0.20, boost = 0.65 }
controls.envelope = { base = 0.15, boost = 0.55 }
```

Each response computes `clamp(base + boost × accent.amount, 0, 1)`, then the MIDI
adapter rounds that normalized value to 0–127. An unaccented hit uses zero emphasis.
Negative boosts are allowed, for example to shorten decay on accented hits.
Groove still shapes base velocity; it does not change the control response or
accent decisions. Existing velocity-only profiles and their event streams remain
unchanged.

Controls go out before the note-on at its actual, possibly swung, onset. They
return to the profile's base at note-off. Stop and dispatch errors also attempt
to restore any outstanding controls after releasing notes. This is a momentary
response lasting the note's musical gate, not a simulated filter/envelope. It
restores the configured base, not a value read from the host. A rest emits no
controls. Late control attacks are skipped; resets are still delivered.

Each `(channel, CC)` has one owner across the composition. Every named profile
control must have an output mapping. Kits may declare additional unused mappings;
these emit nothing until a response or parameter lane uses them. The supported CC ranges are 1–31, 33–63 and 70–95, excluding pedal,
bank-selection, parameter-selection and channel-mode commands. A mapping's channel
defaults to the Part's note channel. MIDI CC is channel-wide; a Part name does not
make it specific to one Drum Rack pad. Configure that association in the host.

## Hear it with the 909 Core Kit

New projects include **accent-punch**, a 132 BPM percussion system, and two
**learn-** helpers. The drums play immediately; the extra controls require explicit
Ableton mappings. The kit does not automatically interpret CC 20/21 as these
parameters.

1. On the Phasecraft MIDI input in Live, enable **Track** for notes and **Remote**
   for mappings. Keep Sync enabled only if using Phasecraft's clock.
2. In the 909 rack's rim chain, choose a filter-frequency parameter and an envelope
   decay parameter, or map those to two rack macros. Set sensible ranges using the
   macro or MIDI Mapping Browser's minimum/maximum values.
3. Select **learn-filter** in Phasecraft. Enter Live's MIDI Map mode, select the
   filter parameter/macro, then briefly Play and Stop Phasecraft. The helper sends
   one changing CC; verify that Live learned **channel 16, CC 20**, rather than its
   quiet channel-10 rim note. Exit Map mode.
4. Repeat with **learn-envelope**, verifying **channel 16, CC 21**, for decay.
5. Select **accent-punch** and Play. The rim's independent seven-step accent now
   changes its velocity and both mapped controls. Save the Live Set to preserve
   these mappings. Use absolute CC mapping and Takeover Mode **None** so each
   generated value applies directly.

The helpers are also in `examples/studies/` for CLI use. Existing projects are
not rewritten; a new project is the easiest way to get the examples and helpers.
The player inspector's **Accent controls** panel and JSON/human traces show each
normalized response, MIDI value and reset. The CLI reports note and control
message counts separately.

Ableton's [MIDI mapping guide](https://help.ableton.com/hc/en-us/articles/360000038859-Making-custom-MIDI-Mappings)
and [remote-control manual](https://www.ableton.com/en/manual/midi-and-key-remote-control/)
cover mapping and takeover settings; its [CC guide](https://help.ableton.com/hc/en-us/articles/360010389480-Using-MIDI-CC-in-Live)
explains why Live devices need explicit assignments.

This proves multi-control emphasis. Stateful accent accumulation/decay, shared
accents and pitched 303 articulation remain separate future work. Host smoothing
and parameter response determine the audible shape; Phasecraft performs no DSP.

The proposed [prepared-kit control contract](https://github.com/sleepunit-agents/phasecraft/blob/main/docs/kit-control-spec.md) separates
velocity, held mix level and temporary accent gain, and plans stable per-voice mappings.

[Held parameters and ramps](https://github.com/sleepunit-agents/phasecraft/blob/main/docs/parameters.md)
can now supply a moving base underneath these temporary accent responses.

## Shared semantic accent lanes

One named clocked lane can emphasize any explicitly selected Parts:

```toml
[accents.drums]
rhythm = { steps = 7, pulses = 3 }
probability = 0.85
amount = 0.8

[parts.hat]
use = "techno.closed_hat"
accent.sources = ["drums"]
accent.probability = 0.0 # disables only this Part's own accent decision
```

Consumers share the same admitted source impulse, including its seeded probability
roll. Their trigger decisions remain independent. A shared accent on a rest creates
no note. The resolver takes the **maximum** amount from the Part's admitted local
accent and its admitted shared sources; simultaneous sources do not add loudness
implicitly. Profiles interpret that resulting semantic amount as usual.

Up to 16 named shared lanes are allowed. Consumers explicitly list unique existing
names. Shared lanes have the same Euclidean/Boolean rhythms, reset policy, probability
and probability modes as local lanes, but cannot reference Parts or other shared
lanes. This avoids hidden dependency cycles. Shared decisions use a separate stable
RNG namespace. They are global or group accents by which Parts consume them, not by
special MIDI ownership. Traces and the player show each contributing source.

## Control emphasis with memory

A control response can retain recent accent impulses:

```toml
[parts.hat.profile.controls.cutoff]
base = 0.2
boost = 0.55
envelope = { decay_beats = 2, accumulation = 0.5 }
```

Each fired accented note contributes `accent.amount × accumulation`, decaying
linearly to zero over `decay_beats`. Overlapping contributions add, then clamp to
0–1; the profile multiplies that envelope level by `boost` and adds it to the
current parameter baseline (or profile `base` if no parameter lane is present).
This is a bounded finite-memory control envelope, **not DSP or a TB-303 emulation**.
`accent.memory_punch` is a reusable profile with this behavior.

Decay accepts 0.25–8 beats in sixteenth increments; accumulation is 0–1. Recent
history is bounded to 32 source steps, and impulses begin at actual grooved onsets.
Responses continue through rests and note-offs. An unaccented note does not reset
the envelope. Stop/error restores the binding's kit default, or the baseline's
configured starting value when no explicit default exists. The same control cannot
also have a competing momentary writer. Velocity emphasis remains per-hit.

This response is reconstructed from deterministic admitted history, so inspection
in any order yields the same result. At a watched edit, history is re-evaluated
under the new configuration; it is not a recording of earlier revisions or MIDI
notes physically heard by the host. The envelope decays in musical time, follows
parameter sampling resolution, and is not tied to phrase repeats. The inspector
shows envelope level and the number of recent contributing impulses.

**accent-memory** drives closed-hat and rim cutoff from one shared seven-step
accent process, using different decay lengths. Use the existing Prepared 909 Set.
Listen for clustered accents to build emphasis and sparse passages to relax.
