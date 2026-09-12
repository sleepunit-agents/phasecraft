# Return groups and the router

Status: specification for `[returns.<name>]` and `[router]` — the return group and the
returns-clocked router. Written 2026-09-07 (Art) from `until-stop/piece.toml`, `seeds.toml`,
`ENGINES.md` and `TO-PHASECRAFT.md` § E (rows B13, B14, B28) in sleepunit-agents/until-stop.
This is M1.6 of that plan. `decide` and the promise, the router's events stream and pins are
not here; the last section says what they will attach to.

## The words

| the author writes | the engine calls it | note |
|---|---|---|
| `[scenes.<name>]` | phrase | one table under two names; `[phrases]` still loads. A scene is a diff over the voices, resolved from the piece's base — never from the scene before it |
| `start = "<scene>"` | the router's first scene | at the root, outside `[scenes]`, so a scene called `start` cannot collide with it |
| `[returns.<name>] align = [...]` | `members` | `align` is the word in the file; `members` is accepted and never written back |
| `[router]` | the stochastic arrangement | a piece has an `[arrangement]` list **or** a router, never both: load error |

Every visit to a scene continues the transport's phase. There is no restart, no shift and no
per-visit clock: a scene evaluated at step *s* makes the decisions the plain composition makes
at *s*. `phrase_bars` is declared, fixed and independent of the router (B28): it keys every
phrase-locked decision in the piece, so the router's cadence must not touch it.

## Return groups

```toml
[returns.hat_system]
align = ["voices.hat.trigger.cycle", "voices.memory.trigger.cycle", "voices.drift.trigger.cycle"]
```

A return group names *what has to line up*, never a number of bars. Each key names a fixed
transport period elsewhere in the piece; the group returns when every member is back at its
starting phase together, and its period is the **LCM in ticks**, computed by the engine and
checked at load. Sixteen, five and seven slots line up every 560 slots — 35 bars — and the
engine works that out rather than trusting a comment that says so. Tick zero is the origin,
not return #1; the first return is at `period · 1`.

What a key resolves to today:

| key | resolves to |
|---|---|
| `voices.<id>.trigger.cycle` (`parts.` is the same word) | the structural period of that voice's trigger rhythm, in ticks: steps × the voice's subdivision, through Boolean and Part-reference expressions |
| `voices.<id>.accent.cycle` | the same for its own accent stream |
| `patterns.<name>.change.every` | **not yet** — patterns are M1.5. Load error naming the key; nothing is guessed |
| `lanes.<name>.every` | a shared walk's fixed step interval (`every.slots` × 240 ticks); its values need not repeat. Unknown lanes and target lanes without this top-level clock are refused |
| anything else | load error naming the key and the shapes above |

Refusals, all at load and all naming the member:

- an **event-clocked** member (`Clock::Event`): `align` accepts fixed transport periods and
  nothing else. No Source in this engine is event-clocked yet; the refusal is built and
  tested against the resolved clock, so the first one to arrive is refused without new code;
- a member whose voice the piece does not have;
- a period of zero, or an LCM that does not fit in `u64` ticks (the error names the member
  that overflowed);
- a group with no members, an empty or duplicate member, more than 16 groups.

Two rules about scenes. A scene **may not change a member's period** — the group's period is
the piece's, in every scene, and a scene that rewrites a member's rhythm to a different length
is refused with both numbers. A scene **may drop the voice**: the clock is the transport's,
and a voice leaving the room does not stop time. Return groups belong to the piece; a scene
carrying its own `[returns]` is refused.

One limitation, stated rather than hidden: the group's period must lie on the sixteenth grid
(a multiple of 240 ticks), because the router changes scene on that grid. A period that does
not — a lone 1/16T voice, say — is refused with the number. A return that is not
*bar*-aligned is fine.

Every group is checked whether or not a router uses it.

## The router

```toml
start = "crowded"

[router]
every = { returns = "hat_system" }

[router.routes]
crowded = { crowded = 0.70, exposed = 0.25, hollow = 0.05 }
exposed = { crowded = 0.30, exposed = 0.55, hollow = 0.15 }
hollow  = { crowded = 0.30, exposed = 0.45, hollow = 0.25 }
```

`every = { returns = "<group>" }` is the only clock: the router asks "move?" at every return
of that group. `every = { bars = N }` is M2 (it needs `decide` and the promise) and is refused
by name.

`[router.routes]` has **one row per scene**, keyed by the scene it leaves. A room with no row
has no door — not even to itself — and is refused. A row is keyed by destination:

| weight | meaning |
|---|---|
| a number in 0..1 | that chance |
| `"rest"` | whatever the named entries leave. At most once in a row; `rest` is a weight word and a scene cannot be called that |
| `{ follows = "<lane>", low, high }` | the weight runs linearly from `low` at the bottom of the lane's range to `high` at the top, read at the roll |

A destination a row does not name is a **wall**: weight zero. A destination that is not a scene
is a load error. A stay is a route to yourself.

**Rows sum to 1.** A row of numbers must sum to 1 (to 1e-9; the error names the row and the
sum). A row with `"rest"` must have its named entries sum to at most 1. A row that follows a
lane must satisfy that **at every value of the lane's range**: every follower is linear in its
lane, so the sum over the range is extremal at the corners. Readers of the **same named lane**
share one value at each corner: weights `low = 0, high = 1` and `low = 1, high = 0` following
that lane sum to one throughout its range. Different named lanes vary independently, even
when their ranges match. The engine checks every combination of their endpoints and names
the lane values where the sum fails. At most 16 followed weights are allowed in a row.

The loader checks followed rows against the root composition's named shared-source ranges
(walk `bounds` or target `range`). An unknown source is a load error. At each return, the
router samples each distinct source used by the outgoing row at the absolute return tick,
after any source update due at that tick. The row then reads those frozen values. A target
ramp contributes its value at that instant, not its future target. Stays and scene changes
do not restart a source; all doors use the root transport seed, like other shared readers.

Compiled playback retains the move log and a bounded source cache. Cold routing queries
replay from the start with the same source sampler, so inspection, scene lookup and playback
agree. Replay work grows with elapsed returns and source decisions; no constant-time cold
seek or realtime latency bound is claimed.

## The roll

At every return — absolute ticks `period · r`, r ≥ 1, **never counted from scene entry**
(B13) — the router rolls the current scene's row once. The draw is the engine's usual one:

```
moves/<r>   ≡   decision_roll(seed, "moves", "door", r, "u")
```

with the piece's seed. The router is a composition-level owner, not a Part; `"moves"` is the
address family, `"door"` the dice, `r` the occurrence, `"u"` the decision. The pin
`{ roll = "door", return = R }` in `seeds.toml` names this draw (M1.8 will force it here).

The row is read as **cumulative intervals in the fixed order of the `[scenes]` table** —
the order the author wrote it, which the expanded form keeps — so the same `u` means different
rooms from different rows. `seeds.toml`'s own example: `u = 0.50` is *crowded* from crowded
(its row gives crowded up to 0.70), *exposed* from exposed (0.30 to 0.85) and *exposed* from
hollow (0.30 to 0.75); `u = 0.97` is hollow from every row. Intervals are half-open on the
right, and an edge sits where `f64` addition puts it: 0.30 + 0.55 is a hair above 0.85, so a
draw of exactly 0.85 is still exposed. Decimal weights can sum a hair under 1; a draw past the
last interval lands in the last named room.

Without `decide`, the roll and the landing are the same tick. That is not the promise
contract in `ENGINES.md`; it is the returns-clocked case that needs no promise.

## A stay is not a move

A **move** is a change of scene. At a move, the outgoing scene's declared controls send their
defaults and the incoming scene initializes its own, exactly as at a section boundary
(`arrangement.md`, "Control ownership and transitions"), and the new scene's run begins:
`entered_tick` is the return's tick and `from` is the scene left behind.

A **stay** — a route to yourself — does nothing. No resets, no restart, and the run continues:
a scene-gated history that begins at `entered_tick` keeps counting through a stay (B14). That is
the property `{ on = "enter" }` and `in`-gated lanes will depend on when M2 subscribes to the
router.

The evaluation window for a step opens at the run's `entered_tick` and closes at the next
return. A window closing at every return costs nothing audible — drum gates end inside their
sixteenth and bars own their attacks — and it is what keeps one scene's cells from being
planned into the next scene's time.

## The record

The router leaves one record per return, in order:

```rust
Move { index: r, tick: period · r, from, to, roll: u, moved: from != to }
```

and every step trace carries where the piece is:

```json
"scene": { "scene": "crowded", "index": 1, "start_tick": 134400, "entered_tick": 0, "next_tick": 268800 }
```

`inspect` prints the JSON field; `inspect --human` prefixes each line with
`[<scene> return <r> bar <n> from <scene>]`. `Compiled::moves_through(tick)` returns the log up
to the return `tick` falls in; `Router::visit_at` is the pure form (it walks the returns from
the start), and `Composition::at_step` uses it. The log is a pure function of the seed and the
rows: extending it twice is extending it once, and it is bounded by the number of returns
elapsed, never by a score.

## Watched edits

A route weight, `start`, `every`, a scene's name or order, a scene's Part layout, the piece's
seed, or anything that changes the group's period is structural: changing one re-rolls the log,
so it needs Stop and Play, like an arrangement's layout. A musical edit inside a scene reloads
as a phrase's would.

## The expanded form

`expand` writes the router with its scenes in order, each carrying its resolved composition,
and `start` inside `[router]`:

```toml
[returns.hat_system]
align = ["voices.hat.trigger.cycle", "voices.memory.trigger.cycle", "voices.drift.trigger.cycle"]

[router]
start = "crowded"
[router.every]
returns = "hat_system"
[[router.scenes]]
name = "crowded"
[router.scenes.composition]
# ...
```

That form loads back to the same piece and the same events (tested across two returns). A
scene's composition never carries `[returns]`; the groups are the piece's.

## Not here, and where it attaches

- **`decide` and the promise** (M2, B14): the roll moves earlier than the landing; `Move`
  gains the tick it was rolled at and `every = { bars }` becomes legal.
- **The events stream** (M2, B15): `{ before = "leave" }`, `{ after = "enter" }`,
  `{ on = "enter" }` subscribe to `Move`; `to` filters on the promised destination.
- **Pins** (M1.8, B25): `{ roll = "door", return = R }` forces `roll` at `moves/<R>`; the row
  is still read as intervals, which is why a pin is a number and not a scene.
- **Patterns' and lanes' clocks as members** (M1.4, M1.5): two arms in `member_clock`, where
  the errors that name them are today.
