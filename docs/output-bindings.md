# Musical Parts and replaceable output bindings

A Part is a musical role, not an Ableton pad. The rhythm and emphasis can stay the
same while a kit definition binds that role to different hardware or software.
The prepared Ableton rack and its CC assignments are one default adapter.

Two common target arrangements:

| Target arrangement | Voice selection | Note to send |
| --- | --- | --- |
| Drum Rack / note-selected kit | A pad's configured note on a shared channel | The pad's note number |
| Multitimbral / channel-selected instrument | A dedicated receive channel for the voice | An explicit configurable trigger/root note; it may affect pitch |

Both are supported by today's per-Part `output.note` and `output.channel`. The
engine does not require note channel 10. The same note on two different channels
is valid. There is no wire-level “any note” MIDI Note On: use a documented default
for the target, with an override. A future binding may describe that note's role
as pad selection versus pitch/default trigger without changing the scheduler.

For example, these are alternative definitions of the same role:

```toml
# A prepared Ableton rack binding.
[library.behaviors."kit.target.kick".output]
channel = 10
note = 36
controls.cutoff = { cc = 20, channel = 1 }
```

```toml
# A schematic channel-selected voice binding. Select real target CCs/settings
# before using it; this is not a complete tested Syntakt preset.
[library.behaviors."kit.target.kick".output]
channel = 1
note = 60
controls.cutoff = { cc = 74 } # omitted CC channel follows this Part's note channel
```

The composition references `kit.target.kick` and supplies rhythm/accent behavior.
Switch the project's kit library and MIDI destination; don't translate drum note
numbers throughout every composition. The cutoff test bundle now follows this
layout: `kit.toml` owns all note/channel/CC assignments. Its generated ALS is only
the Ableton side of those bindings.

Syntakt is a motivating real target, not an assumption that note pitch is ignored:
Elektron's manual documents pitch variations from incoming notes and configurable
track MIDI channels. A dedicated adapter should choose the trigger pitch, receive
channels, supported parameter maps and lifetimes deliberately. See the official
[Syntakt manual](https://www.elektron.se/wp-content/uploads/2025/01/Syntakt-User-Manual_ENG_OS1.30B_250129.pdf).

## Current limits to preserve in the architecture

- One selected MIDI output port per running composition today. Different channels
  on that port work; simultaneous multiple physical output ports are future work.
- CC number and optional channel are configurable per named response. They are not
  hardcoded to the Ableton layout. Current CC range validation is deliberately
  narrower than 0–127; a full hardware adapter may need target-aware validation.
- NRPN, pitch bend and target-specific value curves are
  not implemented. Do not equate “can send drum notes to hardware” with a complete
  hardware control profile.
- Accent-only responses reset at note-off. [Parameter lanes](parameters.md) now
  provide held values and musical-time ramps, including on rests. A hardware
  adapter must choose the right lifetime for tuning, levels and envelopes.
- Different instruments give velocity, gate and emphasis different responses.
  Reusing musical intent preserves the system, not an identical sound.
