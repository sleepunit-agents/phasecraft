# Run the Line — first driving-drums audition

Open this folder's `phasecraft.toml` in Phasecraft Player. It selects a looping,
170 BPM kick/snare reference on your existing prepared 909 route. The folder is
self-contained: keep the compositions, config and kits directories together.

The actual Run the Line kick is on 1 and the & of 3; the snare is on 2 and 4.
This audition deliberately disables the full scene's snare groove/nudge. It is a
straight routing, transport and balance check, not the complete driving scene.
The bass and other musical layers remain in the original Ableton set.

1. Use the existing loopMIDI port `Phasecraft`. If yours has a different name,
   edit `config/midi.toml` or select it in Player.
2. In the existing prepared set, route channel 10 to the 909: kick note 36,
   snare note 38. Mute the original kick/snare material to avoid doubling.
3. Use Phasecraft's **Send tempo & transport** and Play/Stop. This project's
   `send_clock = true` enables clock output; Live's receiving port needs its
   existing Track/Sync setup and external sync enabled. Start at the beginning.
4. Listen against the remaining tracks. Confirm the two-step placement, backbeat,
   balance and common downbeat. This score sends no CC automation.
5. Record eight aligned bars of incoming MIDI for comparison: 16 kicks, 16 snares,
   all with matching note-offs. A kick is velocity 127 on 1 and 114 on the & of 3;
   both snares are velocity 121. Each gate is one sixteenth (about 88.24 ms).

CLI, from the parent directory of this folder:

```powershell
phasecraft validate run-the-line-driving-drums
phasecraft play run-the-line-driving-drums --dry-run --bars 8
phasecraft play run-the-line-driving-drums --bars 8
```

The dry run uses a silent output: its counters may include scheduled clock pulses,
but nothing goes to the MIDI port. Omit `--bars 8` for an indefinite live loop.
Every Play starts from the beginning; this is not an Ableton-slaved seek/resume
transport. Stop before changing tempo or restarting the comparison.

The source contract is [until-stop Run the Line DRIVING-DRUMS.md](https://github.com/sleepunit-agents/until-stop/blob/mark/driving-drums-example-20260908/run-the-line/DRIVING-DRUMS.md).
Its separate audition scene leaves the authored driving scene unchanged. No live
capture or listening result is claimed by this project. Next: restore written
feel in a checked comparison and implement the bass's real four-bar note holds.
