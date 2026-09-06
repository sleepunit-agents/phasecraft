# Phasecraft Player

The desktop player opens the same projects as the CLI. TOML is still the score;
the window provides project navigation, transport, MIDI routing and visualization.
There is no embedded composition editor, mixer or audio engine.

## Install and open

Download the player for your platform from the [dev release](https://github.com/sleepunit-agents/phasecraft/releases/tag/dev):

- **Windows x64:** run `phasecraft-player-windows-x64-setup.exe`. The per-user
  installer adds **Phasecraft Player** to Start. No PATH setup or admin installation
  is required. The installer can bootstrap Microsoft's WebView2 runtime if needed.
- **macOS:** use the DMG matching Apple Silicon (`arm64`) or Intel (`x64`), then
  drag the application into Applications.
- **Linux x64:** install the `.deb` with your package manager, for example
  `sudo apt install ./phasecraft-player-linux-x64.deb`. It uses the system's GTK3
  and WebKitGTK 4.1 libraries; the package declares its shared-library dependencies.

These are development builds, without purchased Windows/macOS signing identities.
The GUI checks the rolling `dev` release on launch and every five minutes. When
its full Git commit differs, an **Update & restart** chip appears. Click it to stop
MIDI cleanly, verify and install the signed package, and restart. Projects and
settings stay intact; playback never starts automatically after updating.
Click the build label at bottom left to check immediately. Failed checks or
installs are retryable and do not prevent using the player.

Install this updater-enabled build once manually. Windows and macOS support
in-app updates. On Linux, use `phasecraft-player-linux-x64.AppImage` (mark it
executable with `chmod +x`). `.deb` installations remain package-manager-owned:
`sudo apt install --reinstall ./phasecraft-player-linux-x64.deb`.
Source/debug builds do not self-update. `phasecraft update` updates only the CLI.

Choose **Open project** and select a folder containing `phasecraft.toml`, or use
**New project** to choose a new folder name. Existing destinations are refused.
Recent folders appear on the welcome screen and in **Projects**. Once a project is
open, the sidebar is devoted to compositions; its list scrolls independently of
Projects and the build label. **Projects → Close current project** stops playback,
releases notes/restores controls, and returns home. There is no composition Save
button because the Player does not edit compositions. The list comes from the
manifest; it does not sequence the files automatically.

## Play

Open **Settings**, select your MIDI destination, click **Save settings**, then press
**Play**. On Windows, create a loopMIDI port
and enable that input's Track setting in Ableton; load **909 Core Kit** on a
monitored track. Mac/Linux can also create a virtual MIDI source. **Refresh**
rescans ports; a missing port is an error, never a silent fallback.

**Silent preview** runs the same musical clock and resolver without sending MIDI.
The player does not produce audio. Space toggles Play/Stop when focus is outside
buttons and selectors. Stop, composition changes, project changes and window close
release owned notes. Restarting begins at step zero with reproducible decisions.
Stop also resets the visible position to **1.1.1** and clears history, including at
a finite arrangement's end. Play starts from the beginning.

Settings contains destination, port refresh and **Send tempo & transport**. Changes
are disabled during playback. Save remembers these preferences per canonical
project folder on this computer, in the application config's `player-settings.json`.
Opening a different project uses its own preference. A project without an override
starts with `config/midi.toml`; moving the folder to a new location uses that default
again. Dismiss/Escape cancels unsaved settings edits. CLI playback continues to use
its MIDI config; these local Player preferences do not rewrite musical files or
`config/midi.toml`. To discard stored overrides entirely, close the Player and remove
`player-settings.json` from its application config directory.

During playback, composition and imported-library edits are checked at phrase
planning boundaries. A valid change takes effect as the boundary plays. Invalid
edits show an error while the last good system continues. Fixing the file clears
the error at a later boundary. Tempo and phrase-length changes require Stop/Play.
Additions to the manifest composition list appear when reopening the project.

## Read the system

Each Part card shows its MIDI route, rhythm cycles and the last 16 observed
sixteenth-step resolutions. Mint means a resolved hit; amber means an accented
hit; a slash marks an eligible hit omitted by probability. Other dark cells are
rests or empty history. History begins when playback starts.

The rings show Euclidean eligibility, each with its own phase and rotation. A
white cursor travels smoothly around the rim, even for 1/1 and 0/1 cycles. Eligible
steps briefly highlight as the cursor passes. A hollow highlight marks an active
input without a resolved note; mint/amber card flashes still follow resolved note
onsets, including groove delay. The cursor itself is position, not a note indicator. Trigger A/B and
accent cycles remain independent; a five-step rhythm is never stretched into
sixteen steps. A Part reference is labeled with its target and `hits`/`structural`
mode. Direct sibling inputs show their Boolean operator between them, and the full
trigger expression appears on every card. Up to eight lanes appear across rows;
**Inspect** shows
the complete nested expression and every leaf's phase. Cycles over 128 steps are
sampled for drawing, with their exact lengths and sampling noted in the detail.

Select a card to inspect it; re-click the same card, use ×, or press Escape to close
the inspector. It shows trigger and accent admission probabilities, actual rolls,
Boolean expressions, references and resolved emphasis. A stopped, not-yet-played
project previews the starting position; it does not claim a note has been sent.
The seed display preserves the full unsigned integer value.

Visuals follow deadline-stamped musical resolutions, not planning time. They show
engine intent, not MIDI-driver acknowledgements or audio output. Rendering and IPC
are outside MIDI dispatch. A stalled UI can skip visual frames without delaying
the sequencer; the display catches up when it resumes. Its update rate is about
25 snapshots/second. A requestAnimationFrame loop interpolates within the latest
known sixteenth using tempo; it freezes at that step's end if telemetry stalls,
never inventing future hits. Reduced-motion preferences disable the extra
interpolation. Visuals are an inspection tool, not a timing measurement.

## Build and test

The root crate remains a standalone CLI/library and needs no GUI libraries.
`desktop/` is a separate Tauri crate with a small HTML/CSS/Canvas frontend.

```sh
cd desktop
npm ci
npm run tauri -- dev
# Or create your platform's installer:
npm run tauri -- build
```

Use the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for native
build dependencies. Rust is pinned by the repository and Node 22 is used in CI.

`cargo test --locked` at the root tests the engine and reusable player controller.
`npm test` checks the ring projection. `npx playwright test` exercises browser
interaction against recorded engine output through a mock IPC bridge. Linux CI
also drives the actual packaged Tauri binary through WebKitWebDriver, opening a
real project, playing, stopping/restarting, handling invalid/valid watched edits,
switching compositions and closing during playback. MIDI hardware acceptance
remains a check on the music machine.


## Publishing desktop updates

CI signs Windows NSIS, macOS app archives, and Linux AppImage payloads with the
repository secret `TAURI_SIGNING_PRIVATE_KEY`. The public verification key is in
`desktop/tauri.conf.json`; keep the private key outside the repository and retain
a secure backup. Local builds do not require this key. Signed builds pass
`--config '{"bundle":{"createUpdaterArtifacts":true}}'` to Tauri.

`player-update.json` is published after the packages, with signed targets and
`0.1.0+<full-commit>` metadata. The Player compares commits rather than SemVer
precedence. A publication race can fail signature verification; retry once the
release finishes. The CLI continues to use its separate `update.json` feed.


## Let Ableton follow tempo and transport

Enable **Send tempo & transport** before pressing Play. Phasecraft sends standard
MIDI Clock (24 pulses per quarter note), Start, and Stop on the selected MIDI
output. This works over an existing loopMIDI connection too. Clock continues
through rests and is independent of every Part and random decision.

In Live → Settings → Link, Tempo & MIDI, enable **Track** and **Sync** for the
input receiving Phasecraft, then enable **EXT** in Live's transport. Use Phasecraft
Play/Stop. Avoid enabling Sync for the return output back into Phasecraft or for
multiple competing clock inputs. Live can take a moment to settle to the incoming
tempo. BPM is conveyed by pulse timing, not a literal “132 BPM” MIDI message.

Every Play begins from the start; Pause/Continue and seeking are not implemented.
Tempo edits still require stopping and restarting. Sync is off by default and can
be saved in `config/midi.toml` as `send_clock = true`, or enabled for a CLI run with
`phasecraft play --send-clock`. A player checkbox overrides the file for that run.
Silent preview sends no MIDI, including no clock. If dispatch falls a full clock
pulse behind, playback stops rather than emitting a burst of stale clock messages.

See [Ableton's external MIDI sync instructions](https://help.ableton.com/hc/en-us/articles/209071149-Synchronizing-Live-via-MIDI).

## Windows MIDI routing

Use your existing loopMIDI port: select it in Phasecraft and enable the same input
in Ableton. The experimental Microsoft MIDI Tools setup has been removed; no SDK
or MIDI service setup is required by Phasecraft. Previously created ports are left
alone and can still be selected if available. macOS/Linux retain virtual MIDI output.

A selected composition can now contain its own procedural arrangement. The
composition list is still a project browser; it does not sequence files. Within an
arrangement, the Player shows phrase, section, local bar and cycle at the audible
playhead. A finite arrangement stops automatically, and Play starts it again from
its first section. See [arrangement authoring](arrangement.md).


## Window frame

Windows and Linux use a matching dark custom frame with drag, double-click
maximize/restore, minimize and close controls. macOS keeps native window controls
in an overlaid title bar. Custom close follows the same MIDI cleanup as OS Close/Quit.
The implementation follows [Tauri's window customization support](https://v2.tauri.app/learn/window-customization/).
Physical Windows resizing/window-management behavior still warrants a user pass;
CI builds Windows, while native interaction automation currently runs on Linux.

## E16 kick preview

Settings now includes separate controller input/feedback ports and **Reset live edits**.
These connections are chosen per app launch and do not change the Ableton destination.
The [setup guide](../tools/controllers/README.md) provides the script, encoder assignments,
one-kick project and limits. Temporary edits apply at the next bar and are not saved to
TOML; Stop, composition switch or a changed valid file reload clears them.

The [dynamic E16 Kit](../tools/controllers/KIT.md) follows authored Part order and
keeps edits for multiple Parts. Cards and inspectors show desired values immediately
with a pending marker until their audible bar boundary; rhythm graphics stay audible.
