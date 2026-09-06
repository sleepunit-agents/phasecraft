# 909 listening guide

Load **909 Core Kit** on one monitored MIDI track receiving channel 10 from
Phasecraft. All examples use the existing kit pads; no extra mappings or samples.
Run one at a time and stop with Ctrl-C. Set Live's tempo to match if recording.

```powershell
.\phasecraft.exe play examples/showcases/showcase.toml --port "Phasecraft" --watch
```

## Start here

| File | BPM | Listen for |
| --- | ---: | --- |
| `quickstart/techno.toml` / `quickstart/techno-reuse.toml` | 132 | The same validated groove, explicit versus reusable authoring. No audible difference is intended. |
| `quickstart/dnb.toml` / `quickstart/dnb-reuse.toml` | 172 | The same validated two-step DnB groove, with both authoring forms. |
| `showcases/showcase.toml` | 132 | A steady kick/clap anchors shifting closed hats, rolling rim and sparse ride. Open hats exclude coincident closed hats; rim avoids admitted kicks. |
| `quickstart/hat.toml` | 132 | Original XOR trigger with 16/5-step cycles and a seven-step accent. |

`showcases/showcase.toml` imports `showcases/library/personal.toml`. Keep the examples directory intact
when copying it. Change the personal library while playing with `--watch`: the
resolved change applies at the next phrase planning boundary, just like a song edit.
An invalid or missing library keeps the complete previous composition running.

## Controlled comparisons

Use the same filename substitution in the play command. Give each comparison at
least three four-bar phrases. Restart each file from the beginning for aligned
comparisons; restarting resets the transport. These files are demonstrations, not
an arrangement or automatic transitions between examples.

| Files | What differs | What stays fixed |
| --- | --- | --- |
| `studies/phase-continue.toml` / `studies/phase-reset.toml` | Five-step hat and seven-step accent either carry on across the phrase boundary or restart there. | Kick, seed, pulses, rotations, and all admission probabilities (1). |
| `studies/probability-locked.toml` / `studies/probability-continuous.toml` | Trigger and accent rolls either repeat every four bars or evolve with absolute position. | Seed, IDs, rhythmic eligibility, probabilities, and profiles. |
| `studies/probability-locked.toml` / `studies/probability-new-seed.toml` | Seed changes from 909 to 910. | IDs, rhythms, probabilities, and profiles. The deterministic kick remains unchanged. |
| `studies/probability-locked.toml` / `studies/probability-accent-only.toml` | Accent admission falls from 0.75 to 0.20. | Every trigger roll and hit position. |
| `studies/emphasis-subtle.toml` / `studies/emphasis-punch.toml` | Named velocity profile changes normal/accented intensity. | Every musical event, its semantic accent, and its duration. |
| `studies/interlock-hits.toml` / `studies/interlock-structural.toml` | Rim avoids admitted kick hits versus every potential kick position. | Kick probability (0.5), its seed/ID/pattern, rim candidate pattern. |

In the interlock pair, the kick deliberately drops some eligible notes. `hits`
allows rim material into those gaps; `structural` reserves those spaces even when
no kick fires. References concern the engine's musical admission, not whether a
MIDI driver delivered a late packet or a sample was audible.

`studies/algebra.toml` is the operator gallery. Kick stays on the quarters. Rim uses OR,
snare AND, clap a nested XOR/AND, closed hat A_NOT_B, and ride B_NOT_A. The source
cycles are 16 and 7 steps, with negative rotation on the second source. It is a
busy listening exercise; the complete musical showcase is `showcases/showcase.toml`.

## Understand or change an example

```powershell
# See defaults, imported definitions and overrides resolved into ordinary TOML.
.\phasecraft.exe expand examples/showcases/showcase.toml

# Read a bar of Part decisions and resulting note/velocity/gate values.
.\phasecraft.exe inspect examples/showcases/showcase.toml --steps 16 --human

# Full recursive decision provenance (JSONL), including references and rests.
.\phasecraft.exe inspect examples/showcases/showcase.toml --steps 64
```

A subtle/punch profile is still a **velocity-only** response. These examples do
not claim to implement 303 filter/envelope behavior, MIDI CC mapping, shared accent,
stateful accent memory, swing, or contextual groove. UK garage becomes a useful
next example when that groove layer exists.

## Showcase coverage

| Part | Mechanisms |
| --- | --- |
| Kick | Reusable four-on-the-floor behavior; stable anchor. |
| Clap | Composition of backbeat, no-accent, and 909 output components. |
| Open hat | Reusable behavior and independent accent. |
| Closed hat | Nested OR/subtraction, independent 16/11-step cycles, signed rotation, actual-hit reference, local probability override, named profile. |
| Rim | Imported personal behavior/profile, 15/7-step cycles, actual-hit exclusion, continuous trigger probability and phrase-locked accent probability. |
| Ride | AND, a five-step process reset at phrase boundaries, continuous probability, quiet profile. |

The comparison files supply B_NOT_A/XOR coverage, alternate reference semantics,
and isolated demonstrations without forcing every possible mechanism into this
one groove.

## Groove and garage

Compare `studies/groove-straight.toml` with `studies/groove-swung.toml`, then try
`quickstart/garage.toml` with the 909 Core Kit. See [the groove guide](../docs/groove.md).

### Accent controls

`quickstart/accent-punch.toml` demonstrates velocity plus filter/envelope emphasis
on 909 percussion. `studies/learn-filter.toml` and `learn-envelope.toml` isolate its
CCs for MIDI learning. See [the accent guide](../docs/accents.md) for host mapping
and reset semantics.

### Parameter timelines

`quickstart/intro.toml` opens hats and rim over eight bars while its four-bar phrase
repeats. It targets the generated compact-v1 cutoff Set. See
[parameter lanes](../docs/parameters.md) for base values, ramps and accent interaction.

`quickstart/movement.toml` requires **Phasecraft 909 Prepared.als**. It combines
cutoff, level, pan and decay; Stop restores each touched parameter's kit default.
The original `intro` still works with the compact cutoff Set.

`quickstart/breathing.toml`: smooth six-bar opening, two-bar hold, six-bar closing,
two-bar hold, repeating independently of the four-bar rhythm. Prepared 909 Set.

`quickstart/garage-touch.toml`: compare with garage for identical source hits and
accents, interpreted with offbeat/gap emphasis and bounded timing/velocity touch.

`quickstart/accent-memory.toml`: shared semantic accent source and two finite-memory
cutoff responses. Prepared 909 Set; watch/listen for accumulation and decay on rests.

## Prepared-kit feature tour

These newer examples use **Phasecraft 909 Prepared.als**, with cutoff, level, pan
and stock decay/tail mapped. The original stock-kit examples above remain unchanged.
No new mappings are required if you already used `movement` successfully.

| File in `quickstart/` | Listen for |
| --- | --- |
| `breathing.toml` | Sixteen-bar smooth open/hold/close/hold motion with independent pan |
| `garage-touch.toml` | Compare against garage: unchanged admissions, procedural meter/gap/touch |
| `accent-memory.toml` | Shared seven-step emphasis accumulating into hat/rim cutoff through rests |
| `sections.toml` | A → A2 → A2 → B; inheritance, transitions and section display |
| `techno-journey.toml` | 132 BPM, 32 bars: opening, evolving main groove, kickless break, A2 and closing |
| `dnb-journey.toml` | 172 BPM, same structural tour with the proven DnB core |
| `garage-journey.toml` | 132 BPM, swung ghosts, actual-kick avoidance and shared control motion |

The three journeys stop automatically; set arrangement `repeat = true` to loop.
The main A section retains its original genre's trigger decisions. New control
behavior and groove interpretation do not scramble the note-admission random keys.
Create a fresh project to get the compact versions plus shared libraries. Standalone
journey files include their own definitions so copying one file is sufficient.

See [the current coverage map](../docs/current-coverage.md) and
[the arrangement contract](../docs/arrangement.md) for exact scope and clock behavior.
