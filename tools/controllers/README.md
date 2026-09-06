# E16 kick preview

**Hardware confirmed:** Jonathan found Navigate in the correct editor, mapped the
controls, and reports the kick adapter works. The [dynamic Kit upgrade](KIT.md) adds
all Parts, dynamic labels and pending values; this page documents the original adapter.
Target: firmware **1.1.0**, Lua API **1.2.0**, Jonathan's HW v5 unit.

## Setup

1. Update Player. Import **[kick.e16script](kick.e16script)** into a spare scene
   in the OXI App and select it as the scene's script. Keep that exact filename
   (14 characters including the required extension).
2. Name native page 1 **Kit**, page 2 **Rhythm**. On Kit, assign **Kick Level
   (script ID 13)** to the top-left turn destination 1. Set its push type to
   **Navigate**, destination **page 2**. Leave the other Kit controls Off.
3. On Rhythm, drag the 16 script assignments onto turn destination 1 of encoders
   1–16 in the table order below. IDs match encoder numbers on this page.
   Leave destination 2 and push actions Off. Assignments request manual mode and
   `dis=0` (no firmware value overlay).
4. Keep O's native menu. Set its top-left destination to page 1 and next to page 2.
   O → top-left returns to Kit; O → second opens Rhythm; O → O cancels.
   Upload the scene. Importing a script does **not** assign its controls. IDs refer
   to declared assignments, not positions in the script list.
5. Connect E16 directly by USB. Close the OXI App if it holds MIDI ports.
   In Player **Settings → Controller · kick preview**, click **Refresh ports**,
   select the E16 input and E16 feedback output, then **Connect**. Select these
   each app launch. Keep the ordinary musical MIDI destination pointed at loopMIDI
   for Ableton. Disable direct E16 Track/Remote/control-surface routing in Live
   for this setup and avoid MIDI Thru loops. The script sends private SysEx on
   output 0 (all outputs), not notes or clock.
6. Open the [kick project](../../examples/controllers/kick) in Player with your
   existing **Phasecraft 909 Prepared.als** in Live. No new Set mapping needed.
   Play from Player. E16 should show **Phasecraft Kit** / **Kick / rhythm**.
   Missing feedback shows **Connect player** within about three seconds.

A verified scene export can eliminate manual assignments later; this preview does
not guess an unverified `.oxie16` scene schema.

## Rhythm page

| Row / IDs | First encoder | Second | Third | Fourth |
| --- | --- | --- | --- | --- |
| 1 / 1–4 | A steps | A pulses | A rotation | Trigger probability |
| 2 / 5–8 | B steps | B pulses | B rotation | Combination |
| 3 / 9–12 | Accent steps | Accent pulses | Accent rotation | Accent probability |
| 4 / 13–16 | Kick level | Filter cutoff | Accent amount | Decay |

Turning shows the accepted numeric value for about one second, then the label.
Phasecraft returns values and ring positions. Percentages are normalized parameters,
not velocity. Level/cutoff/decay require named output mappings and an authored
initial/default value; unavailable controls
show `----`. Counts above 9999 use a rounded `k` readout; Player shows the full number.
Step counts support the engine's 1–65536 range; rings scale against that maximum.

Combination advances through **A, OR, AND, XOR, A-B, B-A**. A-only discards B;
turning a B control then creates an empty 16-step B and selects OR. Reducing steps
clamps pulses and wraps rotation. Phase reset/probability modes stay as authored.

The test project starts at 132 BPM: A=4/16, B=0/5, OR, accent=3/7. Add one B pulse
and hear the five-step process interact with the kick. Change accent probability:
trigger positions should stay put. Move cutoff/level, then use **Reset live edits**
while playing: defaults should return at the next bar, including on rests.
Stop restores kit defaults and returns to 1.1.1.

## Deliberate limits

- Exact Part ID `kick`, loop compositions only. Other Parts continue untouched;
  arrangements disable controls. A/B require direct Euclidean leaves, and C needs
  a direct Euclidean accent. Complex expressions need future layouts.
- All edits, including sound controls, apply at the next planned bar beyond the
  lookahead. A late turn may wait another bar. Hardware shows desired values
  immediately; Player rhythm graphics follow audible snapshots.
- Sound knobs start from the authored lane/base/default value (not the current point
  of a running ramp) and latch their lane to a static value; accent responses may
  still add emphasis. Reset resumes authored automation at the current position.
  Stop, composition switch, or changed valid file reload clears edits. Invalid reload
  keeps playing. Starting retains stopped edits if the score is unchanged.
- Edits are temporary, visibly marked, and neither saved to TOML nor recorded for
  replay. Seed alone cannot reproduce an unrecorded knob performance. Disconnect
  leaves edits in place until Stop/Reset.
- No seed control, controller transport/tempo, Part selection, or saved controller
  port preferences in this first slice.

## Script budget and evidence

The documented **8000-byte limit is per minified uploaded script**, not a combined
allowance shared by all scripts stored on the device. Comments and assignment
 declarations are stripped. A scene selects one Lua program.
[API guide transcription: assignments](https://github.com/tsln-lab/oxi-e16-lua-api/blob/main/docs/src/content/docs/assignments.md),
[execution model](https://github.com/tsln-lab/oxi-e16-lua-api/blob/main/docs/src/content/docs/execution-model.md).
This is community-maintained documentation, not OXI's official API distribution.

The older official manual contradicts itself (4000 vs 8000). Jonathan's exported
step-sequencer demo reports failures around 6200 bytes despite the App allowing
8000, plus about 35 upvalues per function. Those are its author's observations,
not a ceiling we measured. Our adapter is under 4 KB **including comments and
assignments**. We have not measured total device script storage.
[Official manual](https://drive.google.com/file/d/1yZn1i96nRkosn2o6eDlj5wzuErPQEe9N/view),
[firmware 1.1.0](https://github.com/OXI-Instruments/OXI-E16-Releases/releases/tag/1.1.0).

The demo uses one native page with internal Note/Velocity/Gate/Global views. Our
preview uses **two fixed native pages** to retain O navigation. Rhythm can later
be rebound to another selected Part: this does not require one native page per Part.
The demo is a research reference, not redistributed source.

## Verification

`cargo test --test controllers` covers bar-boundary edits, reset MIDI during silence,
RNG isolation, and source reloads. Library tests cover packet validation and stale
commands. Player browser tests exercise controller connection and Reset.
`python3 tools/controllers/test_script.py` runs the actual adapter with stub Lua APIs
(requires optional development dependency `lupa`). Physical USB delivery, display,
manual-mode behavior and listening still require the E16.

[nav.e16script](nav.e16script) remains as a display-only diagnostic. Map Probe Turn
ID 1 and Probe Push ID 2 to encoder 1 on pages 1/2. P=page, E=page changes,
T=turn callbacks, B=encoder presses. Hardware tests confirmed O → O leaves P/E
unchanged, selecting another page updates P/E, and mapped push increments B.
