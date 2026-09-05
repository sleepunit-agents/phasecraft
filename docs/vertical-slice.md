# Phasecraft: first vertical slice

Build one procedural hat Part that sends live MIDI. Ableton supplies sound;
the engine owns rhythm. No harmony, arrangement, controller integration, or DSP.

## Contracts to prove

- Musical positions use integer ticks (960 PPQN), with a sixteenth-note grid.
- A four-bar working phrase is a boundary, not the length of every process.
- Trigger expressions recursively compose Euclidean leaves with OR, AND, XOR,
  A_NOT_B, and B_NOT_A. Each leaf owns its cycle length and phrase-reset policy.
- Positive rotation delays a pattern. Patterns have a documented canonical
  orientation so changing implementations cannot silently change a composition.
- Trigger and accent admissions use separate deterministic decision addresses:
  seed, stable Part ID, lane ID, step identity, decision ID. No shared RNG stream.
- Probability identity and structural phase are separate: phrase-locked rolls
  may repeat while a continuing five-step process moves across phrase boundaries.
- Accent annotates a fired event with active/amount. A velocity profile translates
  emphasis into MIDI velocity; accent itself never creates a note.
- Resolution produces generic timed events, independent of rhythm generator and
  MIDI destination. Provenance includes unsuccessful decisions, not just notes.
- Realtime scheduling uses a monotonic transport origin and bounded lookahead.
  Event generation and debug formatting stay outside the timed MIDI dispatch path.
- Stop/error cleanup sends note-offs for active notes. Late playback must avoid
  bursting through a backlog of stale notes.

## First acceptance checks

Known Euclidean patterns and rotations; all Boolean truth tables; different-length
operands; 5/7/16 phase relationships across 560 steps; phrase reset versus continued
phase; deterministic replay; trigger/accent RNG isolation; probability endpoints;
accent without trigger; note-off ordering and stop cleanup; inspectable dry runs.

Actual timing and sound require a connected MIDI host. A passing simulated clock
test is evidence for scheduler logic, not evidence of audible jitter performance.

## Deferred layers

Reusable behavior libraries, cross-Part references, stateful accent profiles,
harmony, E16 mappings, and arrangement follow evidence from this slice. Keep
parameters independent of input devices, without building those future systems.
