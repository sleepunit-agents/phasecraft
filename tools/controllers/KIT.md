# Dynamic E16 Kit

Jonathan confirmed the original kick adapter, navigation and encoder assignments work
on his E16. This extends that working adapter to all Parts in a loop composition.
The new multi-Part flow still needs its own physical test.

## Upgrade from kick

Import **[kit.e16script](kit.e16script)** and select it for the scene. Firmware 1.1.0 /
Lua API 1.2.0 remains the target. Keep the exact filename (13 characters).

**Keep Rhythm on native page 2 with its existing turn IDs 1–16.** The musical layout
is unchanged: A / trigger probability, B / combination, accent / accent probability,
then level / cutoff / amount / decay. These controls now follow the selected Part.

On native page 1, Kit, assign for each encoder position 1–16:

| Action | Assignment |
| --- | --- |
| Turn, destination 1 | Part 01 Level through Part 16 Level, IDs **17–32** |
| Push, type Script | Select Part 01 through Select Part 16, IDs **33–48** |
| Turn destination 2 / special functions | Off |

For example, top-left is turn ID **17**, push ID **33**. Replace its previous
Navigate push with Script. The script's assignment panel supplies these names;
importing it alone does not wire the controls. Check **Push Type**, not a turn
resolution or the push's press/release mode. AT is aftertouch, not navigation.

Press a Kit encoder to select its Part, then **O → page 2** to open Rhythm.
O → page 1 returns to Kit. Selection stays stable across page changes. The title
identifies the selected Part. A push selects without changing level. The native
Navigate action and script selection cannot both occupy the push assignment; this
version uses the explicit two-action path rather than guessing a Lua page setter.

For more than 16 Parts, native page **3** is Kit 2. Copy Kit's assignments there:
the same turn IDs 17–32 and push IDs 33–48 now address Parts 17–32. Add page 3 to
O's menu if needed. There are no extra assignments for this bank.

## Connect and play

Update Player, select the E16 input and feedback output in **Settings → Controller ·
E16**, and leave the musical output pointing at your loopMIDI port. Keep the E16's
direct Ableton routing disabled for this setup. The controller connection is selected
each app launch, independently of musical output. Output 0 in the script broadcasts
its private SysEx; avoid MIDI Thru loops. No clock or notes come from the Lua script.

The supplied [kit project](../../examples/controllers/kit) contains techno, DnB and
garage with all Part level/cutoff/pan/decay bindings declared for your existing
**Phasecraft 909 Prepared.als**. The grooves match the existing examples. No new Set
or Ableton mapping is needed. Open this project folder in Player.

Existing projects are not rewritten by updates. A Part without a declared level
mapping is still named/selectable on Kit and its rhythmic controls work, but turning
its Kit level knob does nothing. Add the appropriate `kit.prepared.*` behavior to
that Part or use the supplied project. We do not substitute note velocity for volume.

## Dynamic names and order

Both Player and Kit use the order of Parts in the composition. Keyed TOML tables now
preserve source order instead of being alphabetized. Our examples put kick first;
there is no rule forcing everyone else's kick to the front. Legacy `[[parts]]` arrays
retain their explicit order too. Musical dependency evaluation and seeded decisions
remain independent of display order.

Labels come from the loaded Part IDs. Common labels include Kick, Snar, CHat, OHat;
other IDs are shortened to four ASCII characters. Collisions or unrepresentable names
receive distinct P01/P02-style position labels. The selected Part's ID appears in the
15-character title; Player retains the full name. Empty slots are inactive. Reloading
or switching compositions refreshes the labels and invalidates old controller commands.

## Immediate values, quantized playback

Player shows desired values immediately: changed cards show **old → new · NEXT BAR**,
and their inspectors show desired values with the currently playing values alongside.
The running rhythm graphics continue to represent audible events. Pending markers clear
at the scheduled audible deadline, including when another edit was made after that bar
was already planned. Stopped edits show directly without a pending marker.

E16 displays returned values immediately. Pending controls keep their numeric readout
and a different ring color; the Rhythm title gains `*`. Once active, the marker clears
and the normal label returns after the short value-display interval. Kit rings indicate
selection and pending Part changes. Physical color appearance needs confirmation.

All edits, including sound knobs, still apply at the next planned bar beyond lookahead.
They are temporary: Stop, Reset, composition switch or a changed valid file reload
clears them. Reset during playback can itself be pending until the next bar. Multiple
Parts can hold independent edits. Disconnect keeps edits until Stop/Reset. Sound knobs
start from the authored lane/base/default value, latch that lane, and may still receive
accent emphasis. Reset resumes authored automation at its current musical position.

## Limits and checks

Loop compositions only; arrangement controls remain disabled. Literal A/B editing
requires direct Euclidean leaves. For example, garage's kick-referencing rim keeps its
unsupported structural knobs disabled while its probability and sound controls work.
No performance recording, saved edits, live tempo/seed, or controller port persistence.

The new script has 48 assignment declarations but about **3.3 KB of code before
minification once comments are removed**. Assignments are stripped by the App and do
not consume the documented 8000-byte upload allowance. See the [budget research](README.md#script-budget-and-evidence).

Rust tests cover source order, multi-Part edit isolation, stale selection, both banks,
label collisions and audible pending deadlines. Browser tests cover immediate desired
values without pretending the playing pattern changed. Lua simulations cover selection,
labels, pending feedback, second-bank routing and inactive pages. The original script
and its tests remain as a compatible fallback.
