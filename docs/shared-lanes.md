# Shared weather lanes

`examples/quickstart/weather.toml` implements until-stop's weather and three readers:
hat decay, snare decay, and the gate on written snare bursts. The hat rhythm in this
study is static; retained hat-memory, drift, and the full scene port remain later work.

```toml
[lanes.weather]
start = 4
range = [0, 8]
carry = "always"
target = { every = { bars = 11 }, delta = [-2, -1, 0, 1, 2], ramp = { bars = 4 } }
```

Decision 1 is at bar 12. It selects uniformly from `delta`, adds to the last settled
value, and clamps the target to `range`. Bar 12 keeps the old value; bars 13–16
publish four equal increments; then the lane holds through the next decision.
Duplicate delta entries provide proportional weight. The source uses the root seed
and the existing versioned decision hash with owner `weather`, lane `target`,
occurrence 1, decision `delta`. Target pins are not supported in this slice.

The source belongs to transport time, including when nobody reads it. Scene seeds
and section restarts do not restart its clock or reseed it. Expanded child snapshots
must carry the same lane declarations as the root. Values and the latest target,
origin, progress, occurrence and roll appear in inspect JSON under `lanes`.
Those sample ticks are absolute transport ticks even inside restarted sections.

Readers name the operation and own their spans:

```toml
[parts.hat.parameters.decay]
follows = "weather"
op = "scale-default"
low = 0.70
high = 1.30
unclipped = true

[parts.snare.ornaments.gate]
follows = "weather"
op = "range"
low = 0.02
high = 0.25
```

For until-stop's source spelling, `decay` becomes native `parameters.decay`, and
`ratchet.gate` becomes native `ornaments.gate`. The lane and follower declarations
otherwise retain their meanings. `range` linearly maps the source range onto the
reader's span; `scale-default` also multiplies the control's declared output default.
Descending spans are permitted. Continuous results clamp to 0..1 before MIDI rounding;
gate spans must already fit 0..1. `unclipped` rejects a mapping whose endpoints clip.

A continuous control can instead take a normalized lane value unchanged:

```toml
[lanes.level]
start = 0.5
range = [0.2, 0.8]
carry = "always"
target = { every = { bars = 2 }, delta = [0.3], ramp = { bars = 1 } }

[parts.hat.parameters.decay]
follows = "level"
op = "direct"
```

`direct` requires the lane's entire declared range to fit 0..1. For example, a
lane with range `[0.2, 0.8]` publishes 0.2/0.5/0.8 as CC 25/64/102, regardless of
the kit default. It neither normalizes the lane range nor multiplies a default.
The original weather study's 0..8 lane therefore requires a mapping operation;
using `direct` with it is a load error. Omit `low`, `high`, and `unclipped` for
`direct`; mapping operations still require both endpoints. Burst gates retain
`op = "range"`. Direct controls use the same barline publication, quantization,
deduplication, scene reset, and stop lifecycle as mapped controls. Load reports
show their declared reach. This is the existing companion ENGINES follower contract
and `check.py` direct/no-span rule; it adds no source or timing-walk support.

A followed control cannot also have a fixed/ramped/automated value or accent response.
Active followed controls must have distinct channel/CC targets.

The prepared 909 hat default is 89/127, giving CC 62/89/116 at weather 0/4/8. The
snare default is 1.0, giving 89/127/127. Its saturation above weather 4 is intentional.
These numbers do not establish the instrument's decay curve or a hardware audition.
`phasecraft validate examples/quickstart/weather.toml` reports each reader's reach,
saturation, and at most two followed CC candidates per bar before deduplication.
Outgoing boundary resets are additional traffic, excluded from that count.

Continuous followers produce candidates only on barlines, after lane updates and
before note events. The existing dispatcher suppresses unchanged quantized controls.
On a scene change, outgoing resets precede incoming publications; absent readers
publish nothing. Router boundaries must be bar-aligned when these lanes are present.
Stop restores declared kit defaults through the existing dispatcher lifecycle.

A gate reads weather at the written main onset, before groove offsets. It replaces
the opening probability for both plain `*n` and questioned `*n?` bursts, keeping the
source's count and spacing. The main hit remains; refused tails spend no velocity
values. Admitted tails spend even if later suppressed, as in the event-values contract.
The draw is continuous rather than phrase-locked; `burst` pins use that identity.
Its native hash address remains the ornament's `ratchet` / `admission` address, not
a new random family. A configured ratchet and a followed gate may coexist; the gate
supplies probability. The literal source still supplies count when present.

Limits: at most 16 lanes, 1–64 finite delta choices, every 2–65536 bars, and a positive
ramp strictly shorter than the decision interval. Checkpoints retain at most 4096
settled targets per compiled snapshot. Missing history is replayed exactly; a distant
cold seek costs linear work in elapsed decisions and has no constant-time deadline
guarantee. New compiled snapshots reconstruct under the new score; no live retained
state journal migrates across edits. Event-clocked shared sources, walks, lane-following
doors are outside this slice.

The example-first contract is until-stop `WEATHER.md` and its default-checked
`trace_weather.py`. That document uses the existing hypothetical TRACE delta deck;
the engine study uses actual seed hashing and does not claim to reproduce that deck.
