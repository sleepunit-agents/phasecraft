# Your Phasecraft project

This folder can hold one track, an album, or a live set. Compositions are playable
systems; their list is not an arrangement or playback order.

From this folder (put Phasecraft on PATH, or use the full executable path):

```sh
phasecraft validate .
phasecraft play --dry-run --bars 4
phasecraft ports
# Edit config/midi.toml to select your MIDI destination.
phasecraft play --watch
phasecraft play compositions/dnb.toml --watch
```

Drop Ableton's **909 Core Kit** on a MIDI track and enable monitoring for your
loopback input. Techno is 132 BPM; DnB is 172 BPM. Garage is a swung 132 BPM system. All three use channel 10.

- `phasecraft.toml`: default composition, all compositions to validate, shared libraries.
- `compositions/`: the choices specific to each piece. Add files to the manifest list.
- `patterns/drums.toml`: reusable drum behaviors and trigger patterns.
- `patterns/accents.toml`: emphasis patterns and response profiles.
- `patterns/grooves.toml`: shared swing feel for the garage Parts.
- `kits/909.toml`: reusable musical pad mappings.
- `config/midi.toml`: this machine's MIDI destination and scheduler lookahead.

Libraries listed in the manifest are available to every composition in this
project. Paths in the manifest are relative to it; composition `imports` are
relative to the composition. Move the whole folder anywhere. Files outside a
project remain standalone.

A common Part is just `[parts.closed_hat]` with `use = "techno.closed_hat"`.
Override its trigger admission using `[parts.closed_hat.trigger]` and
`probability = 0.75`. Give it a different rhythm using
`rhythm = { steps = 11, pulses = 4 }` in that same trigger table.
For independent Boolean cycles, use
`rhythm = { op = "xor", a = { steps = 16, pulses = 7 }, b = { steps = 5, pulses = 2 } }`.
Explicit `type` tags and the original `[[parts]]` format also work.

`phasecraft expand .` shows exactly what inheritance resolved;
`phasecraft inspect . --human` explains the notes and rests.
`phasecraft validate . --json` provides machine-readable validation.

`--watch` applies valid musical changes at phrase boundaries. Invalid edits keep
the last good system playing. Change MIDI configuration while stopped.
