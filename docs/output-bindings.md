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

## The kit: instruments read by name

A composition binds a voice to an instrument by name, and the kit says what that
name is:

```toml
# kits/until-stop.toml — the piece's own kit file
[library.kit]
kick       = "kit.prepared.kick"        # an alias: this behavior's output, read as-is
closed_hat = "kit.prepared.closed_hat"
metal      = "kit.prepared.ride"        # the piece's word for the ride

[library.kit.sub]                       # an instrument phasecraft has not met: declared inline
note = 48
channel = 2
```

```toml
[parts.hat]
kit = "closed_hat"
[parts.metal]
kit = "metal"
[parts.metal.output]
gate = "1/16"                            # the voice's own output fields overlay the kit's
```

`[library.kit.<name>]` is a third library section beside `behaviors` and
`profiles`. An entry is either an output table or the name of a behavior, whose
`output` it takes verbatim, so an imported kit such as `kits/909-prepared.toml`
is read and never re-typed. On a Part, `kit = "<name>"` **replaces** any output a
composed behavior brought and is then overlaid by the Part's own `output` fields.
An unknown name is an error that lists the instruments the kit does declare; a
`kit` on a profile is an error; duplicate instrument names across kit files are
errors like any library name.

**Every kit entry is valid where it is written, whether or not a voice uses it.**
An output table is checked as the Part would check it — shape *and* range: channel
1..16, note 0..127, gate within the bar — the moment its file is read, so
`note = 200` in an instrument no Part binds to is refused, and a Part that does
bind to it cannot rescue it by overlaying its own `note`. An alias is resolved once
every library has loaded (built-ins, the project's `libraries`, imports, then the
composition's own table, in that order), so it may name a behavior a later file
declares and is still refused if nothing ever does — and what it resolves *to* is
then held to that same shape-and-range check, so an alias to a behavior whose
`output` carries `note = 200` is refused exactly as the inline table would be, and one
whose `output` is not a table at all — a bare string, a number — is refused as a shape.
(A behavior name is a legal way to *write* an entry, not a legal thing for one to
resolve to: at that point the value is the output.) The error names the entry, the
bad value or the unresolved target, **and the file the entry was written in** — an
imported kit's fault is reported against the kit file, not against the composition
that imported it.

**What an instrument declares, it is held to; what it does not declare is a gap.**
A kit entry that lists a `controls` table declares what the instrument can hear:
a parameter lane or profile response on a name it does not list refuses to load,
as it always has. A kit entry with **no `controls` table at all** has declared
nothing. A Part bound to it that still addresses controls is a *gap*: the
controls are listed by name, set aside, and the draft goes on without them, so
`validate` reports it (valid, with the gap) and `inspect` and `expand` print it
and proceed. Every playback door refuses a composition with a gap — `play`, the
Player and `--watch` reloads all load through the same refusal — because a piece
whose live control lands nowhere is not a piece that can sound, and a draft
inspection must never become the authorisation to play one. Declare the controls
in the kit (or on the Part's own `output.controls`) to close the gap; an explicit
empty `controls = {}` is a declaration, and refuses like any other.

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
