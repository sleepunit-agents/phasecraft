# Held parameters and musical-time ramps

A parameter lane exists independently of notes. It initializes at transport start,
updates through rests, and holds its value after note-off. A ramp can cross any
number of phrase repeats:

```toml
[parts.hat.parameters.cutoff]
value = 0.15
ramp = { to = 0.9, over_bars = 8, start_bar = 1 }
```

This starts at 0.15 on bar 1, reaches 0.9 at bar 9, and holds there. Bars are 4/4;
`start_bar` is one-based and defaults to 1. The ramp is linear in normalized control
space, not necessarily linear in Hz or perceived brightness. All values are 0–1.
`over_bars` and `start_bar` are integers 1–65536. A downward ramp works the same way.
Omit `ramp` for a constant held value. This is an implemented format, not pseudocode.

The receiving control is declared separately, for example:

```toml
[parts.hat.output.controls.cutoff]
channel = 15
cc = 75
default = 1.0 # fully open when transport stops
```

These particular addresses match the closed hat in our compact-v1 prepared Ableton
kit. Hardware can provide another channel/CC binding without changing the ramp.
A kit may declare unused output mappings; only active parameter lanes and accent
responses emit MIDI. Each active name must have a binding, with at most eight
bindings per Part and one owner per channel/CC across the composition.

## Shared intent and independent emphasis

Put a reusable behavior in a project's library file:

```toml
[library.behaviors."my.opening".parameters.cutoff]
value = 0.15
ramp = { to = 0.9, over_bars = 8 }
```

Compose `my.opening` into the hats and percussion that should rise together. Each
Part evaluates the same absolute musical timeline, even when it has different
trigger, accent and phrase-reset policies. No notes are needed for it to advance.

An accent profile may add emphasis on top:

```toml
[parts.hat.profile.controls.cutoff]
boost = 0.1
```

The resolver owns the complete value: `clamp(current parameter base + boost ×
accent amount, 0, 1)`. At note-off it restores the **current** base, including any
ramp movement since the attack. Groove delays the emphasis with the actual note;
the base timeline stays on the clock. If a profile supplies `base` too, the active
parameter lane takes precedence. Profiles without a parameter lane keep their
original momentary response and configured reset base. Missing profile base now
defaults to zero; existing explicit bases behave as before.

This is still per-Part/channel control, not per-note expression. Changing a filter
can affect already-sounding tails. [Stateful accent envelopes](https://github.com/sleepunit-agents/phasecraft/blob/main/docs/accents.md#control-emphasis-with-memory)
can now add a decaying response instead of a gate-length emphasis impulse.

## Transport and editing semantics

- Play starts from bar 1 and initializes parameters before the first note attack.
- Ramps use absolute transport time, not phrase-local phase. A four-bar phrase does
  not restart an eight-bar opening. After its end a ramp holds its target.
- Live file edits apply at the existing phrase boundary. They evaluate at the
  current absolute position; editing a ramp does not restart its clock. For a new
  eight-bar rise beginning at bar 17, set `start_bar = 17` and `over_bars = 8`.
- Stop, finite completion, and playback error restore each touched control to its
  declared `output.controls.<name>.default`. This is a normalized kit value, separate
  from the composition's starting value. Without an explicit default, held lanes
  restore their configured `value`. A later Play reinitializes from bar 1.
- Note-off still restores the **current moving baseline**. Stop's kit default takes
  precedence over any outstanding accent. Only controls actually used are reset;
  unused declared mappings are not sent. Selecting another composition after Stop
  therefore does not inherit the previous ramp. This is a declared default, not
  readback of arbitrary knob positions in Live.
- Removing a held binding during watched playback leaves the last value until Stop,
  when its remembered default is restored. Phasecraft cannot read the host's earlier
  state. Configure another value before removing it if an immediate change matters.
- Finite playback remains end-exclusive. To observe a ramp's endpoint at bar 9,
  play into bar 9, rather than stopping immediately before it.

Controls are sampled at 24 points per quarter note plus actual note-on/off
boundaries. CC values remain 7-bit; unchanged values are suppressed at dispatch,
so a constant held lane sends once until it changes. A long ramp only sends when
its rounded MIDI value changes. This is sufficient for slow builds; smoothing and
sound response belong to the host. It is not audio-rate modulation.

Stale timeline samples are skipped instead of replayed as a burst; an outstanding
emphasis still gets a reset. At Stop the declared kit default is restored, independently of the latest ramp
position or any active emphasis.
The player and JSON inspection expose sampled base, emphasis and final MIDI values,
including on rests. Human CLI inspection includes the parameter trace too.

## Listening

New projects include **intro**, using the compact-v1 cutoff Set already generated
for the 909 kit. Kick and clap anchor the groove while hats and rim open over eight
bars. The four-bar phrase continues underneath; accent nudges sit on the moving
base. Edit `patterns/parameters.toml` for the shared opening and
`kits/909-cutoff.toml` for target bindings. The one-off `learn-filter` helper uses
older addresses and is not needed for this prepared Set.

Runtime “ramp from here” commands and group mixing remain outside this timeline.

## Prepared kit and existing projects

**movement** uses the expanded `Phasecraft 909 Prepared.als` with cutoff, level,
pan and decay mappings. Its kit defaults are in `kits/909-prepared.toml`. Level
is separate from velocity; decay follows each stock voice's envelope mechanism.
See [prepared kit](https://github.com/sleepunit-agents/phasecraft/blob/main/docs/prepared-kit.md) for target-specific details.

Updating the executable does not rewrite existing projects. For an older intro,
add `default = 1.0` to each of the three `controls.cutoff` mappings in
`kits/909-cutoff.toml`. Otherwise the fallback reset is its starting value (0.15),
which is still dark. New projects include these defaults and both examples.

## Segments, curves and repeating motion

Use `automation` instead of `ramp` for a sequence of destinations:

```toml
[parts.hat.parameters.cutoff]
value = 0.15
automation.repeat = true
automation.segments = [
  { to = 0.9, over_bars = 6, curve = "smooth" },
  { to = 0.9, over_bars = 2, curve = "hold" },
  { to = 0.15, over_bars = 6, curve = "smooth" },
  { to = 0.15, over_bars = 2, curve = "hold" },
]
```

Each segment begins at the previous destination; the first begins at `value`.
`linear` is the default. `smooth` uses cubic smoothstep (gentle start and finish).
`hold` retains the starting value until the segment ends, then changes to `to`;
use equal start/destination values for a plain hold. This is normalized control
space, not a promise of linear Hz/dB or host smoothing.

`over_bars` accepts integer or fractional bars in sixteenth-note increments
(0.0625 bars minimum). A lane accepts 1–64 segments, at most 65536 bars total.
Optional `automation.start_bar` is one-based (default 1). Before that bar, the lane
holds `value`. With `repeat = false` (default), the last destination holds forever.
With `repeat = true`, the segment sequence repeats on its own total duration,
independent of the rhythm phrase; the exact cycle boundary begins at `value`.
For seamless cycles, end the final segment at `value`; unequal endpoints make an
intentional jump. Stop defaults are unaffected.

A local `automation` override replaces an inherited `ramp`, and vice versa. Explicit
`ramp` and `automation` in the same lane are rejected. Existing single ramps remain
compatible. Trace samples show segment number, zero-based cycle, curve and progress;
the player presents one-based cycle numbers. `breathing` is the prepared-909 example.
