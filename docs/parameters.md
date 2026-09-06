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
can affect already-sounding tails. Stateful accent envelopes remain future work.

## Transport and editing semantics

- Play starts from bar 1 and initializes parameters before the first note attack.
- Ramps use absolute transport time, not phrase-local phase. A four-bar phrase does
  not restart an eight-bar opening. After its end a ramp holds its target.
- Live file edits apply at the existing phrase boundary. They evaluate at the
  current absolute position; editing a ramp does not restart its clock. For a new
  eight-bar rise beginning at bar 17, set `start_bar = 17` and `over_bars = 8`.
- Stop retains held values. If temporary emphasis is outstanding, cleanup restores
  its most recently sampled base. A later Play reinitializes from bar 1.
- Removing a held binding leaves the host at its last value; Phasecraft cannot read
  and restore the host's earlier state. Configure another value before removing it
  if that matters. Existing device settings remain authoritative outside our lanes.
- Finite playback remains end-exclusive. To observe a ramp's endpoint at bar 9,
  play into bar 9, rather than stopping immediately before it.

Controls are sampled at 24 points per quarter note plus actual note-on/off
boundaries. CC values remain 7-bit; unchanged values are suppressed at dispatch,
so a constant held lane sends once until it changes. A long ramp only sends when
its rounded MIDI value changes. This is sufficient for slow builds; smoothing and
sound response belong to the host. It is not audio-rate modulation.

Stale timeline samples are skipped instead of replayed as a burst; an outstanding
emphasis still gets a reset. A stopped emphasis restores the base from the latest
processed sample, within the control sampling resolution during healthy playback.
The player and JSON inspection expose sampled base, emphasis and final MIDI values,
including on rests. Human CLI inspection includes the parameter trace too.

## Listening

New projects include **intro**, using the compact-v1 cutoff Set already generated
for the 909 kit. Kick and clap anchor the groove while hats and rim open over eight
bars. The four-bar phrase continues underneath; accent nudges sit on the moving
base. Edit `patterns/parameters.toml` for the shared opening and
`kits/909-cutoff.toml` for target bindings. The one-off `learn-filter` helper uses
older addresses and is not needed for this prepared Set.

This first timeline implements constants and one scheduled linear ramp per control.
It does not yet implement multiple segments, curves, cycling automation, runtime
“ramp from here” commands, group mixing, or whole-song arrangement. The lane model
keeps these separate from note generation so those can be added without changing
rhythmic identity.
