# Controller research: OXI E16 and Neuzeit Drop

Research date: 2026-09-06. **Update:** the kick adapter is hardware-confirmed. The [dynamic Kit upgrade](../tools/controllers/KIT.md) adds multi-Part selection, dynamic labels and audible pending feedback; this extension awaits physical testing. The broader integration below remains a proposal. Both devices are already available to the user; price is not the deciding factor.

**Start with the E16 as Phasecraft's musical control surface. Keep a generic MIDI adapter so Drop can use the same parameter layer, with a layout aimed at performing eight Parts at once.** Drop is also a viable first controller if simultaneous faders and track controls matter more than contextual labels.

## Verified capabilities

| Capability | OXI E16 | Neuzeit Drop |
| --- | --- | --- |
| Physical controls | 16 push encoders; 12 pages, 16 scenes | 32 push encoders, eight faders, eight mute buttons; eight layers |
| Values and feedback | Lua can receive SysEx and set control values, ranges, labels and rings | MIDI feedback for mapped controls, including relative encoders; NRPN feedback unsupported |
| Relative turns | Rel1: 1 increment / 127 decrement; Rel2: 63 increment / 65 decrement | Four documented relative CC formats |
| Displays | Dynamic labels, limited to four characters per encoder slot; page title available | Rings and MIDI-driven color modes; dynamic text-label protocol not established by this research |
| Performance features | Snapshots, morphing, groups, gesture recording | Snapshots, quantized changes and fades; fader pickup with direction indicators |
| Physical routing | USB MIDI, TRS and Bluetooth | Two USB host/device connections, four TRS inputs/outputs, CV |

E16 evidence: [official manual 1.0.1a](https://drive.google.com/file/d/1yZn1i96nRkosn2o6eDlj5wzuErPQEe9N/view), printed pp. 23 and 85–98; [product page](https://oxiinstruments.com/oxi-e16). Drop evidence: [official firmware 2.01 manual](https://www.neuzeit-instruments.com/mediafiles/Manuals/Drop/Drop_Manual_v2_01.pdf), pp. 4, 25–32, 36, 42–44. PDFs were downloaded directly from the manufacturers and text-extracted locally when the browser could not read them.

Drop feedback must correspond to a configured output slot with an input port, and linear mapping is recommended. Its merger does not forward SysEx. Connect a future E16 SysEx integration directly to the computer. [Drop manual](https://www.neuzeit-instruments.com/mediafiles/Manuals/Drop/Drop_Manual_v2_01.pdf), pp. 19, 28.

OXI's support landing page is stale relative to its official releases. [Firmware 1.1.0, August 11, 2026](https://github.com/OXI-Instruments/OXI-E16-Releases/releases/tag/1.1.0), adds push-toggle feedback and Lua changes/fixes. The manual contains inconsistent script-size limits (4000 versus 8000 bytes) and differs from the release notes in some LED API naming. Keep the adapter small and verify against the installed firmware before writing a substantial script. Do not infer working integration from the marketing page alone.

## Why E16 first

Our central interaction is selecting a Part and changing its rhythmic rules. A contextual page is useful here: selecting hats can bind the same encoders to that Part's trigger and accent parameters, with Phasecraft sending back current values. Lua's manual mode lets Phasecraft own the accepted value instead of letting the controller silently run ahead.

Drop has a different practical advantage: eight physical columns can remain dedicated to eight Parts, with less paging during a performance. Proposed column: level fader, mute button, and four knobs for cutoff, trigger probability, accent amount and decay. Further layers can expose pan and structural rhythm controls. Mapping availability should follow the Part's actual capabilities, rather than inventing cutoff or decay for an unmapped destination.

Both are useful. We should choose the first implementation by workflow, not make either device an engine dependency. No need to use Drop's Ableton remote script when it is controlling Phasecraft; Phasecraft would be the MIDI-feedback peer.

## Shared integration design

The current code has MIDI output in `src/playback/ports.rs` and phrase-boundary file replacement in `src/playback/transport.rs`. `src/control.rs` now provides temporary one-Part edits; `src/playback/controller.rs` supplies the initial E16 adapter. Existing parameter automation is musical output behavior, not that missing input API.

```text
Controller USB input → device adapter → typed parameter/action commands
                                          ↓
                               Phasecraft transport / state
                                          ↓
Controller USB feedback ← accepted values / pending changes / UI state

Phasecraft musical MIDI output → loopMIDI → Ableton
```

Use separate controller input/feedback ports and musical output. Phasecraft owns the controller ports; disable their direct Ableton control-surface/Remote routing for this setup. Keep Phasecraft as clock master initially. The controller connection does not remove Windows' existing loopMIDI requirement for the separate Ableton connection.

1. **Parameter identity and validation.** Stable addresses such as `parts.hat.trigger.probability`, with type, range, enum choices, step size and application boundary. Resolve bindings after inheritance/overrides; validate pulses against steps and disable controls that do not apply to an expression.
2. **Commands, not file rewrites.** Controllers send Set/Adjust/Trigger actions into a bounded queue. No TOML parsing, disk writes, UI work or MIDI feedback inside the MIDI callback. Stop must remain reliable under a burst of knob traffic.
3. **Explicit timing.** Structural edits and seed changes commit atomically at the next safe bar/phrase boundary, beyond already planned events. Continuous controls use the next safe control update with bounded smoothing. Do not mutate already scheduled notes. Current tempo changes require restart; live tempo needs separate clock work.
4. **Authoritative feedback.** Return accepted/clamped values, selected Part and pending/applied status. Relative commands must not be mistaken for absolute feedback; deduplicate unchanged output, rate-limit refreshes and prevent MIDI-thru loops. Resynchronize after reconnect or composition change; reject stale commands for the previous composition.
5. **Automation ownership.** Initially use explicit manual latch: touching a parameter overrides its automation until a Resume automation action. Show the override. Stop/switch clears overrides and restores the existing kit defaults. Later, trim modes can be added deliberately. Do not let hardware snapshots and Phasecraft automation continuously overwrite each other.
6. **Repeatability and persistence.** Same seed alone cannot reproduce an unrecorded performance. Record accepted edits with musical positions and boundary decisions for replay. Live state should be visibly temporary until an explicit Save variation action exists; do not silently rewrite the source composition. Controller hardware preferences belong in machine-local Settings; reusable semantic layouts can live with the project/library.

## First E16 page proposal

| Row | Four controls |
| --- | --- |
| 1 | Selected Part, trigger probability, accent probability, accent amount |
| 2 | Trigger A steps, pulses, rotation, Boolean operator |
| 3 | Trigger B steps, pulses, rotation, mute |
| 4 | Accent steps, pulses, rotation, seed variation selection |

This is a layout proposal, not a frozen protocol. The literal A/B page applies to compatible expressions; complex trees should expose selected nodes or use a project-authored page. Seed selection should stage a new seed for explicit/boundary application, not send a new full seed at every intermediate encoder tick. A second performance page can expose mapped cutoff/level/pan/decay and automation resume.

## Smallest useful proof

1. Add a MIDI diagnostics view: input/feedback port selection, incoming message display and deliberate feedback test. Verify both physical devices, firmware versions, port names, relative encodings, button edges and absence of feedback loops.
2. Bind one hat's trigger probability and one mapped cutoff; have the GUI show accepted state and the hardware follow changes from Phasecraft. Test rests, automation takeover/resume, Stop and reconnect.
3. Prove a quantized pulse-count change and staged seed change; replay the recorded command history to check deterministic results. Verify saturation does not disturb MIDI timing.
4. Add the full E16 contextual page through a small Lua/SysEx adapter, including initialization/resynchronization and four-character labels. Test actual firmware API names and off-page updates before expanding it.
5. Add Drop's eight-column CC layout through the same commands, testing fader pickup and feedback. Treat snapshot interpolation as a later explicit owner of continuous controls; discrete rhythm topology should change atomically, not sweep through intermediate operators/seeds.

The one-kick preview implements a subset of this proposal. Hardware tests remain necessary before calling the E16 profile supported; Drop has no adapter yet.
