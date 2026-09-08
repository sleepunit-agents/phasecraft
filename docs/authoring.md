# Authoring conventions

Use a project to share musical material across compositions. Use a standalone TOML
for a small experiment. Both expand to the same validated musical model.

## Three levels, one format

Common:

```toml
tempo = 132
seed = 91827
[parts.hat]
use = "techno.closed_hat"
```

Tuned: append `[parts.hat.trigger]` with `probability = 0.75`.

Primitive: put this rhythm in that trigger table:

```toml
rhythm = { op = "xor", a = { steps = 16, pulses = 7 }, b = { steps = 5, pulses = 2 } }
```

A rhythm with `steps` implies `type = "euclidean"`; one with `op` implies
`type = "binary"`. `{ part = "kick", mode = "hits" }` is shorthand for
`{ type = "part", id = "kick", mode = "hits" }`. The reference mode remains
required. An explicit `type` wins; contradictory fields are rejected. A partial
override such as `{ pulses = 4 }` inherits its existing type and other fields.
For written rhythms, `trigger.pattern = "x ~ ~ [~ x]"` supplies a literal trigger.
Its explicit form is `rhythm = { type = "literal", pattern = "x ~ ~ [~ x]" }`;
`pattern` also infers the literal type. The closed grammar, probability markers,
cycle length and rotation are described in [notation.md](notation.md). A literal
currently occupies the trigger root; it cannot be nested in Boolean expressions or
used as an accent source.

`[parts.hat]` supplies ID `hat`; writing `id` there is an error. Existing
`[[parts]]` entries with explicit `id`, and single `[part]` files, remain valid.
Do not rename IDs casually: they address random decisions and Part references.
Keyed table order carries no musical meaning.

## Pins

Every random decision is a pure function of the seed and an address, so one draw can
be forced without moving any other. A pin names the **dice**, never the outcome:

```toml
[[pins]]
at = { roll = "fire", voice = "hat", bar = 2, slot = 3 }
u  = 0.99
```

`roll` says which dice; `voice` (a Part) or `accent` (a shared accent lane) says whose;
`bar` and `slot` say when, both 1-based, the slot counted on the owner's own grid (a
`1/16T` Part has 24 slots per bar; a shared accent lane has 16). `u` is the number the
draw returns instead of the hash, `0 <= u < 1`; the composition still decides what
`u` means at that address, so a pinned `0.5` can admit under one probability and refuse
under another. A voice rolls `fire` (trigger admission), `accent`, `burst` (ratchet),
`flam`, `ghost`, `timing` and `velocity`; a shared accent lane rolls `accent`. A pin on a
dice the owner never rolls — `burst` without a ratchet (written `*n` for literal
triggers), `ghost` without a groove — is an
error, as is a pin on a Part or lane that does not exist. A composition carries at most
256 pins.

**The slot is counted inside the bar it names, not by multiplying a per-bar count.** The
grid is continuous — step *n* is at tick *n·cell* — and a dotted cell does not divide the
3840-tick bar, so bars hold different numbers of onsets and only the first starts on a bar
line. A `1/8.` Part (720 ticks) has six slots in bar 1, at ticks 0…3600, and five in bar 2,
the first at 4320; `bar = 2, slot = 1` is that 4320. The error for a slot past the end names
the count for the bar it was asked about, because there is no single count. That count can
be zero: `1/1.` is 5760 ticks, longer than a bar, so its onsets fall at 0, 5760, 11520, …
and every third bar holds none at all. No slot addresses such a bar, and a pin that names
one is refused for the empty bar, not for its slot number.

`velocity` is drawn wherever the touch closure runs, which any one of `groove.offbeat_gain`,
`groove.after_gap` or `groove.humanize` opens — not `humanize` alone. `timing` is drawn on
every admitted event and again for the next onset's reservation, so it needs no groove at
all and is never refused. Both are the same die whether or not the amount is audible: with
no `humanize` the jitter is zero ticks and the velocity factor is 1.0. **A pin names the
die, not its effect** — pinning `timing` under a groove with no `humanize` forces a draw
that moves the onset by exactly zero. With a [note or pitch lane](pitch.md), a timing
pin that moves the attack across a value onset also changes its sounding note: those lanes
sample at the final attack tick. Bar ownership still clips movement at bar boundaries.

The address is the engine's, so the pin obeys the lane's `probability_mode`: under
`phrase_locked` the dice at bar 2 slot 3 is rolled again every phrase, and the pin lands
every time; under `continuous` the address is the step index itself, and the pin lands once
per pass of that step **on the clock the section runs**. That clock is the qualification: a
section with `phase = "restart"` restarts the step index, so a continuous pin at bar 1 slot 1
is consulted again at the head of every restarting section; under `phase = "continue"` the
clock is transport-relative and the step passes once. With no arrangement there is one clock
and a continuous pin lands exactly once. Two pins that resolve to one dice are an error that
names both shapes. `inspect` marks a forced draw with `pinned = true` on the **decision** it
reached — trigger, accent and shared-accent admissions carry the flag, and so do the ornament
gates (`ornaments.ratchet.pinned`, `ornaments.flam.pinned`), where a refused gate's
`suppression_reason = "probability"` otherwise reads as chance when the roll was authored; the
touch traces do not yet, so a forced `timing` or `velocity` is read from its rolled value in
the trace rather than from a flag. Pins live with the composition, not a phrase:
every phrase and section inherits them, and the seed is not part of the address, so changing
the seed rerolls everything except the pinned draws — which is what pins are for.

## Sharing musical knowledge

Group related definitions into files such as `patterns/drums.toml` and
`patterns/accents.toml`. A behavior can be a whole drum voice or just one lane:

```toml
[library.behaviors."my.backbeat".trigger]
rhythm = { steps = 16, pulses = 2, rotation = 4 }

[library.behaviors."my.shifting_accent".accent]
rhythm = { steps = 7, pulses = 3 }
probability = 0.75

[library.profiles."my.soft_punch"]
use = "accent.punch"
base = 50
boost = 20
```

A composition can then use:

```toml
[parts.clap]
compose = ["my.backbeat", "my.shifting_accent", "kit.909.clap"]
[parts.clap.profile]
use = "my.soft_punch"
```

A voice names its instrument with `kit = "closed_hat"`, resolved through the
project's `[library.kit]` table — see [output bindings](output-bindings.md#the-kit-instruments-read-by-name).

Components merge left to right; local fields win. Tables merge recursively.
A change of rhythm kind replaces the old expression completely, including when
kind is inferred from shorthand. Arrays replace, not append. `use` selects a
fresh definition for that subtree. Unknown fields, missing names, duplicate
definitions, cycles and incomplete final Parts are errors. Partial library
components need not be independently playable; validation checks reachable
compositions, not every unused fragment.

## Project manifest

```toml
name = "My album"
default = "compositions/techno.toml"
compositions = ["compositions/techno.toml", "compositions/dnb.toml"]
libraries = ["patterns/drums.toml", "patterns/accents.toml", "kits/909.toml"]
midi = "config/midi.toml"
```

The list is for validation and organization; it is not an arrangement. Paths are
relative to the manifest and cannot contain `..` or be absolute. A file loaded
inside a project uses the nearest ancestor `phasecraft.toml`, including when the
file is not yet in `compositions`. Add it to the list for whole-project validation.
A directory argument requires its own manifest. For `play`, `expand`, and `inspect`,
a directory or manifest selects `default`.

Libraries load explicitly from the manifest, followed by composition `imports`
(relative to the importing file), then inline definitions. Duplicate names are
errors across all sources; there is no implicit shadowing or folder scan. Repeated
imports of the same canonical file load once. Move the whole project to preserve
relative references. `expand` writes a standalone musical snapshot, excluding
project connection settings.

## MIDI configuration and edits

`config/midi.toml` contains `port = "Phasecraft"` and `lookahead_ms = 100` by
default. Alternatively omit `port` and set `virtual_port = true` on macOS/Linux.
Lookahead must be 10–1000ms. Command-line destination flags override the configured
destination; `--lookahead-ms` overrides configured lookahead. The config must be
valid even when overriding it. No MIDI device is opened by `new`, `validate`,
`expand`, `inspect`, or `play --dry-run`.

Playback retains its connection and lookahead until restart. `--watch` reloads
the selected project/composition and its libraries at phrase planning boundaries;
valid music changes apply atomically. Invalid edits retain the last good music.
Changing a project's default while playing the project selects the new composition
at a boundary, subject to existing tempo/phrase-length restrictions. Playing an
explicit composition keeps that selection. Tempo or phrase-length changes still
require restart. This does not implement live-set arrangement.

For agent authors: run `validate PATH --json` after editing, use `expand PATH` to
check inheritance, and `inspect PATH` to inspect decision provenance. Errors include
source paths and Part context where available. Standard TOML errors may refer to
the expanded structure rather than the original source line.

## Updating Phasecraft

`phasecraft --version` reports version, full source commit and native platform.
`phasecraft version --json` provides those fields for scripts.

`phasecraft update --check` compares the installed commit with the published `dev`
manifest. `phasecraft update` downloads the platform executable, checks its size
and SHA-256, verifies its reported commit/platform, and replaces the running
executable. The next command uses the update. `--force` reinstalls the same commit.
A different commit means a different build; this follows `dev`, not semantic-version
ordering or Git ancestry. A locally built executable reports its checkout HEAD
(or `unknown` without Git); uncommitted edits are not a separate release identity.

The repository is public; updates work without authentication. An existing GitHub
CLI login or `GH_TOKEN` / `GITHUB_TOKEN` can optionally authenticate requests.
Tokens are neither saved by Phasecraft nor accepted in TOML; HTTPS requests strip
authentication on cross-host redirects. See [GitHub CLI authentication](https://cli.github.com/manual/gh_auth_login).
This updater manages the CLI. Install the latest desktop package to update the player.

Stop playback before updating. Updating touches only the executable; projects,
config, edited examples and adjacent files are preserved. Embedded starter
files and built-in behaviors update with the executable. Download the full package
when you want refreshed standalone examples/docs. `play` never checks the network
or silently installs anything. The first updater-enabled executable still needs
one manual download.

Publication uploads native assets first and `update.json` last. Concurrent
publication can temporarily return a missing asset or checksum mismatch; retry
shortly. Download, checksum or candidate validation failures leave the installed
executable unchanged. Replacement requires a writable installation directory.
The updater uses [self-replace](https://docs.rs/self-replace/latest/self_replace/)
for native Windows/Unix replacement and cleanup, with a subprocess replacement
test on every supported native CI platform. Checksums detect mixed/corrupted
assets; release authenticity relies on GitHub HTTPS and repository permissions,
not a separate signing infrastructure.

Part order in keyed `[parts.name]` tables is preserved as written, just like explicit
`[[parts]]` arrays. Player cards and the E16 Kit use this order. It does not alter
seeded decision identities or dependency evaluation order.


Numeric velocity sequences are available as `[parts.NAME.velocity]`: a flat `pattern`,
`per = "step"` or `"event"`, optional event `clock = "main"` or `"attacks"`,
`cycle` in sixteenth slots, and event `carry = "always"`. See [event values](event-values.md)
for admission, boundary spending, scene compatibility, and seek/reload limits.
