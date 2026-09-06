# E16 navigation probe

This is a hardware experiment, not a controller integration. It uses the documented Lua API and has not yet been run on an E16. Jonathan's unit is HW v5, upgrading from firmware 1.0.1; the official latest release checked on 2026-09-06 is 1.1.0. Update completion is not yet confirmed.

In the OXI App, create a spare test scene and a script containing [e16-navigation-probe.lua](e16-navigation-probe.lua). Select that script for the scene. Assign its **Probe Turn** and **Probe Push** parameters to encoder 1's turn and push actions, on native pages 1 and 2. Upload the scene using the app. Keep the standard Alt-menu page destinations for this test.

The script sends no MIDI and writes no persistent variables. Its top four screen labels show:

- `P`: current native page.
- `E`: number of page-change callbacks since the script loaded.
- `T`: number of turn callbacks.
- `B`: number of press callbacks.

1. Open page 1. Confirm the title reads **Nav probe** and the labels show counters.
2. Turn and quickly press encoder 1. Confirm T and B increment respectively. Avoid long presses, which can invoke firmware functions.
3. Note E. Tap O to open the native page/Alt menu. Note whether the menu keeps its normal appearance. Tap O again to return. Report whether E changed and whether the probe labels survived.
4. Tap O, select native page 2, and confirm P becomes 2 and E increments. Return to page 1 through the menu and check again.

If the title never appears or counters reset unexpectedly, report that before interpreting the results. No change in E on an O round trip would establish only that this firmware does not expose that action through the page-change callback; it would not prove there are no undocumented APIs.

## Supported design versus open questions

The manual documents one script per scene, dynamic control properties via `controller.set`/`setByIndex`/`setControls`, control labels via `slots.update`, a title via `page.setTitle`, and actual page-change notifications. It does not document a Lua setter for Alt-menu entries or an O/menu-enter callback. Native Alt-menu entries can be configured manually as page destinations, among other built-in commands. See the [official manual](https://drive.google.com/file/d/1yZn1i96nRkosn2o6eDlj5wzuErPQEe9N/view), printed pp. 68, 86, 90–102, and [1.1.0 release notes](https://github.com/OXI-Instruments/OXI-E16-Releases/releases/tag/1.1.0).

The initial layout should therefore keep fixed native destinations in the Alt menu: Kit, Rhythm, Sound, Perform. Reuse those control pages for the selected Part. The twelve documented native pages are view types, not a twelve-Part limit. Some narrative manual passages incorrectly say sixteen pages; the Lua page range and native page-menu description both specify twelve. Verify the installed unit if this differs.

This probe deliberately does not guess undocumented callback names or try to modify firmware-owned menu state. Arbitrary custom navigation remains unconfirmed. HW v5-specific compatibility is also not independently established by the release notes; the on-device test is the relevant next check.
