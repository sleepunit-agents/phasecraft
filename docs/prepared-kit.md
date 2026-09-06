# Prepared 909: four controls per pad

The generated **Phasecraft 909 Prepared.als** expands the working compact cutoff
Set to 64 external mappings: cutoff, level, pan and decay for each of 16 pads.
Notes remain 36–51 on channel 10. Controls stay on channels 15–16, using the same
reserved eight-slot layout. All previous compact cutoff addresses are unchanged.

| Name | Actual target | Stop default |
|---|---|---|
| cutoff | Simpler filter frequency, 30–22000 Hz | Fully open |
| level | Existing per-pad Level macro → chain mixer volume | Saved stock macro position |
| pan | Simpler panorama, left to right | Approximately centered (7-bit MIDI) |
| decay | Stock envelope decay/release, through existing macro where bound | Saved stock value, within MIDI rounding |

Level's normalized maximum maps to the saved stock Level macro position, and its
minimum maps to that macro's existing minimum. This preserves the stock mix at 1.0
without adding the macro's extra gain headroom. It is independent of note velocity
and does not reset at note-off. A separate accent-gain stage remains future work.

The stock closed hat uses actual amplitude **DecayTime**. The other fifteen pads
use **ReleaseTime**, controlling the tail after note-off. Existing macro mappings
and ranges are preserved, including snare macros named Tone that only target
ReleaseTime. Unmapped release controls get a 1 ms–saved-value range. This does not
convert the kit to Drum Sampler or change envelope modes. A short sample still
cannot be extended beyond its recorded tail. The generated report lists exact
per-pad targets, ranges and normalized defaults.

This distinction follows Simpler's ADSR behavior: decay approaches sustain while
release controls the fade after note-off. It is not the One-Shot Fade Out control.
[Ableton's instrument reference](https://www.ableton.com/en/manual/live-instrument-reference/)

## Listening check

1. Update Phasecraft and open **Phasecraft 909 Prepared.als** in Live 12.4.3.
2. Enable Track and Remote on the loopMIDI **input**, and monitor the drum track.
3. Create a new Phasecraft project and play **movement**. It opens hats/rim with
   cutoff and level, moves hats in opposite stereo directions and changes hat tails.
4. Stop midway. Filters should open, levels return to stock, pans center and tails
   return to their declared defaults. Play **techno** to check the clean transition.
5. The supplied `phasecraft-prepared-check` project has 64 isolated checks, one for
   every pad/control. Save As and reopen the Set to verify mappings survive.

XML structure, unique mappings, macro preservation and sample references are
checked automatically. New mappings still need an opening/listening check in Live.
The already-working compact cutoff Set remains usable with **intro**, but it lacks
level/pan/decay assignments for **movement**.

Existing projects are not rewritten by app updates. Add `default = 1.0` to old
intro cutoff bindings or use a new project. See [parameter lanes](parameters.md).

## Maintainer generation

```sh
python3 tools/ableton/prepared_template.py \
  '/path/to/909 Core Kit.als' '/path/to/909 Core Kit One Mapped.als' \
  /tmp/phasecraft-909-prepared
python3 -m unittest discover -s tools/ableton -p 'test_*.py'
```

The tool accepts only the verified fixture schema, refuses existing output folders,
keeps all original sample references and internal macro mappings, and verifies that
changes stay within the filter/allocator and 48 additional parameter nodes. The
private fixtures and generated stock Set are not committed to the public repository.
