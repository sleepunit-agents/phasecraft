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
control must have exactly one output mapping, and mappings without responses are
rejected. The supported CC ranges are 1–31, 33–63 and 70–95, excluding pedal,
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
