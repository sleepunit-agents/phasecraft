# Phasecraft prepared drum kit — control contract draft v1

Status: proposed design, 2026-09-05. This specifies the next kit and engine work;
it does not claim a prepared Ableton rack or new parameter engine already exists.
The initial sound palette is 909. The enduring object is the voice structure and
control contract, so replacing a sample does not require rewriting the composition.

## 1. What Phasecraft controls today

Verified against `src/music/{mod,accent,resolve}.rs` and `src/playback/` at d7e612b.

| Capability | Current behavior | Consequence for this kit |
| --- | --- | --- |
| Drum selection | Fixed MIDI note and channel per Part | Keep existing kit notes; changing note selects another pad, not its tuning |
| Velocity | Note-on velocity from base, accent contribution and groove | Describes strike strength; host may interpret it as loudness and timbre |
| Gate | Musical note duration, currently inside one sixteenth | Whether note-off shortens the sound depends on the host instrument |
| Accent | Independent emphasis amount/probability | A musical annotation interpreted by a profile |
| Named controls | Up to eight CC responses per Part, normalized base + emphasis contribution | Can address mapped gain, filter, tuning, etc.; current response is momentary |
| CC lifetime | Send before a fired note; reset to configured base at note-off and outstanding-control cleanup on Stop/error | No independent control events on rests, no general held parameter/automation lane yet |
| Sync | MIDI Clock, Start and Stop | Optional host tempo/transport following |
| Pitch bend | **Not implemented**: no pitch-bend lane or 0xE0 output path | Reserve native bend for later articulation; do not build the kit around it |
| Other expression | No pressure, MPE, per-note CC or general modulation lanes | CCs affect their mapped voice chain, including any overlapping tails |

A zero-boost CC response can set a fixed base value on each hit today. That is not
an independently scheduled mixer/automation parameter: it cannot apply a change on
a rest or initialize the whole kit before the first note.

## 2. Separate strike, mix and emphasis

- **Velocity**: how the drum is struck. Preserve its musical contour when mixing.
- **Level**: sustained gain for the voice, downstream of timbre shaping. Turning
  down the hat must not rewrite its velocities or change its velocity layers.
- **Accent Gain**: optional short emphasis gain, on a separate gain stage. It must
  not overwrite Level. A profile may use velocity, Accent Gain, tone or several.
- **Drive**: nonlinear sound shaping, with separate compensation. Do not use Level
  as the drive input or use velocity as a substitute for a mixer fader.

Do not assume velocity is amplitude-only in a stock kit. Audit existing mappings
before adapting it. Default prepared sample voices should use a documented velocity
response; additional velocity-to-filter behavior must be deliberate and visible.

## 3. Eight controls per voice

Macro order below matches CC order. Names/addresses form the stable contract;
physical ranges are initial proposals to tune on the prepared voices, not universal
claims about every sample. Labels shown to humans may use units; wire values are
absolute 7-bit CC. Tone and decay curves need useful resolution near their low end.

| Macro / CC | Semantic name | Meaning and intended host destination | Initial range / neutral | Intended lifetime |
| --- | --- | --- | --- | --- |
| 1 / 20 | `tone` | Brightness via a low-pass cutoff, fixed modest resonance | 200 Hz–20 kHz; neutral fully open; constrain low end per voice after listening | Held base + optional emphasis |
| 2 / 21 | `decay` | Audible tail duration, via a true amplitude decay where available | Voice-specific ranges below | Held base; per-hit variation only after testing envelope behavior |
| 3 / 22 | `level` | Dedicated post-processing gain | -48 to 0 dB, neutral 0 dB; actual mute is a separate future command | Held; never reset on note-off |
| 4 / 23 | `tune` | Sample transpose relative to the voice's manually chosen root tuning | -12 to +12 semitones; neutral exact 0; semitone steps | Held; no native pitch-bend dependence |
| 5 / 24 | `drive` | Saturation amount with paired output compensation | 0–12 dB drive, neutral zero; calibrate compensation by ear | Held base + optional emphasis |
| 6 / 25 | `pan` | Voice position using a post-instrument pan/balance stage | Full left–right; exact center default | Held |
| 7 / 26 | `space` | Wet contribution of a per-voice parallel ambience branch | Dry through a restrained wet maximum; neutral dry | Held base; a proper timed throw is later work |
| 8 / 27 | `accent_gain` | Dedicated second gain stage for emphasis, separate from Level | 0 to +6 dB, neutral 0 dB | Momentary; current gate/Stop reset is suitable |

`level` is attenuation-only initially to preserve headroom. Set sample calibration
and kit master gain locally before composing. Centered controls need a tested CC64
neutral; do not assume normalized 0.5 or a rack macro midpoint lands at exact zero.
Record actual neutral bytes and range curves in the eventual kit definition.

Provisional decay ranges: kick 30–1500 ms; snare/clap 20–1500 ms; closed hat 5–300 ms;
open hat 30–2500 ms; rim 5–600 ms; low tom 30–2000 ms; ride 50–4000 ms. A short sample
cannot be made longer merely by raising decay. Preserve the control and its meaning,
but retune usable limits after a sample swap when necessary.

Compatibility: current `accent.filter_punch` calls CC20 `filter` and CC21 `envelope`.
Those names remain valid in existing compositions. They bind to the proposed Tone
and Decay controls; do not silently rename the existing profile. New semantic names
will need explicit kit bindings rather than two competing owners of the same CC.

## 4. MIDI address plan

All drum **notes remain on channel 10**, matching the current `kit.909` library.
Each voice gets a dedicated **remote-control channel**, reusing CC20–27. These are
explicit Ableton remote mappings, not channel routing to individual Drum Rack pads.
The receiving track accepts the note input; Remote handles mapped controls.

| Voice | Note number | Note channel | Control channel | Controls |
| --- | ---: | ---: | ---: | --- |
| Kick | 36 | 10 | 1 | CC20–27 |
| Snare | 38 | 10 | 2 | CC20–27 |
| Clap | 39 | 10 | 3 | CC20–27 |
| Closed hat | 42 | 10 | 4 | CC20–27 |
| Open hat | 46 | 10 | 5 | CC20–27 |
| Low tom | 44 | 10 | 6 | CC20–27 |
| Ride | 51 | 10 | 7 | CC20–27 |
| Rim | 37 | 10 | **16** | CC20–27 |

Rim channel 16 / CC20–21 preserves the current accent-punch learning setup.
Use MIDI numbers in documentation; octave labels differ between applications.
Channels 8–15 have no new per-voice assignments in this draft. Do not allocate kit
master controls or a second kit implicitly. A second independently controlled kit
needs a distinct control input/address plan; duplicating mapped racks in a Set is
not proof of independent addressing. Native channel-volume CC7 is not a per-pad
volume control and is not used here. Native pitch bend is also channel-wide and
is not a substitute for the Tune macro.

## 5. Persistent voice structure

One Drum Rack with eight prepared pads. Each pad contains a persistent Instrument
Rack exposing the eight macros above. Suggested signal path:

`sample player → compensated drive → tone filter → accent gain → level/pan → dry + parallel ambience`

The sample player's own amplitude envelope supplies Decay; Tune addresses its
transpose. The parallel ambience branch belongs to that voice so its macro can be
contained in the nested rack. This first version does not depend on mapping a
nested macro to an outer Drum Rack return send. A later kit may substitute shared
returns while preserving the musical `space` intent and documenting its response.

Keep a clearly named sample player, the wrappers, pad receive/play notes, macros,
and MIDI mappings in place. Replace the sample **inside that existing instrument**.
Do not replace the entire pad or drag a new instrument over its title bar. Device
replacement is an adapter change requiring remapping and revalidation. Sample gain,
start/end, warp, root tune and useful decay bounds are local sound-design settings;
structural compatibility does not guarantee equal loudness or identical timbre.

Proposed closed/open hat choke group: one shared group, other voices unchoked.
Verify its musical behavior with long open hats and interleaved closed hats.
Choking, note-off response and overlapping tails are host behavior, not implicit
promises of Phasecraft's current note scheduler.

### Sample player: Live 12.4 Suite target

Jonathan confirmed **Live 12.4 Suite**. Target that environment for the first kit;
compatibility with older releases or other editions is not yet promised. Specify
**Drum Sampler in Trigger mode** for the first voice prototype, with Hold at its
minimum and Decay mapped to the named tail control. Ableton documents its
trigger-mode Hold/Decay and sample-only hot swap. A future Simpler adapter must
document its playback mode: One-Shot Fade Out
is anchored to the sample end and is not equivalent to amplitude decay from onset.
Do not label it a universal Decay implementation without a listening check.
[Instrument reference](https://www.ableton.com/en/manual/live-instrument-reference/).

Prototype one rim voice before converting the complete 909 kit. In particular,
test whether changing/resetting Decay during a sounding sample modifies its current
tail. Do not assume the host latches the value at note-on. Phasecraft's short note
gates must not accidentally truncate the intended drum envelope.

## 6. Packaging and host setup

Deliver a **starter Live Set/template** containing the complete remote assignments,
and a reusable **rack preset** containing the voice devices and internal macros.
The template is the supported mapped entry point; a rack preset alone is not our
promise of a portable external MIDI map. Ableton explicitly documents MIDI mappings
in template Sets. Verify preset-only import separately during construction.
[Template Sets](https://www.ableton.com/en/manual/managing-files-and-sets/).

Track on for the incoming drum notes, Remote on for the mapped control input,
optional Sync for Phasecraft clock. Use absolute CC mappings and a takeover setting
that applies generated values directly. Give mappings voice-qualified names such
as `Rim / Level`; nested macro labels can stay concise. Save and reopen to test all
addresses. Kit devices require the declared Live version/edition; do not assume
that a rack using arbitrary stock devices runs in every Live edition.
[Remote mappings](https://help.ableton.com/hc/en-us/articles/360000038859-Making-custom-MIDI-Mappings).

Macros may address multiple contained device parameters and have constrained ranges,
which supports the proposed Drive compensation and reusable voice controls.
[Rack macro controls](https://www.ableton.com/en/manual/instrument-drum-and-effect-racks/).

## 7. Engine work this exposes — not implemented by this spec

1. **Kit binding metadata independent of active responses.** Declare all available
   controls, units, curves, neutral values and addresses once. Today's engine
   requires exactly matching profile/output control keys, so it cannot simply load
   all eight mappings while using a two-control accent profile.
2. **Held parameters.** Initialize before the first attack; apply quantized edits
   even on rests; hold through note-offs. Stop policy must distinguish mixer state
   from temporary emphasis. Do not reset a fader merely because a note ended.
3. **Base plus modulation resolution.** One owner computes the final output when
   an accent modifies a held tone/drive base; releasing the accent restores the
   current base. Do not allow two independent CC writers to race.
4. **Control lifetimes beyond the gate.** Envelope values, short gain emphasis and
   reverb throws have different timing requirements. Earn those policies from the
   one-voice prototype; never pretend current gate-reset behavior fits all three.
5. **Control-check/learning compositions.** Exercise one voice/control at a time,
   expose the transmitted channel/CC/value, then test the entire mapped kit.

Pitch bend is deliberately not a prerequisite. If implemented later, define its
range, target scope and center/Stop behavior separately from static drum tuning.
No pitched sequencing or MPE implementation is part of this kit task.

## 8. Acceptance checks before calling the kit ready

- Existing techno, DnB and garage trigger the same named pads at the same notes.
- Fixed velocity + changing Level changes mix gain without changing strike data;
  fixed Level + changing velocity retains the chosen instrument response.
- Accent Gain returns to neutral while Level stays at its held setting; test Stop
  mid-emphasis and configuration edits across rests.
- Every CC affects only its named voice/control. Test two voices simultaneously.
- Tune affects that sample, never changes its drum identity, and returns exactly
  to the neutral pitch when requested. Pan center is actually centered.
- Decay behaves usefully at 132 and 172 BPM, with short gates, long samples, choke
  groups, accents and overlapping tails. Document any channel-wide tail effects.
- Swap samples inside rim, kick and hat instruments; all controls and assignments
  remain attached. Retune sample gain/ranges without changing their MIDI addresses.
- Save/reopen the Set; save/import the rack separately and record precisely which
  bindings survive. Provide an honest minimum Live/device requirement.

First construction milestone: rim + Tone, Decay, Level and Accent Gain, preserving
channel16 CC20/21. Verify separation and envelope lifetime, then extend to all eight
controls and voices. No runtime or Ableton files are modified by this draft.
