# Step notation: the literal leaf

Status: specification for the `literal` rhythm/value leaf. Written 2026-09-07 (Art) from
`until-stop/ENGINES.md` § "the subset is closed" and every pattern string in both pieces of
sleepunit-agents/until-stop. This grammar is **closed**: an engine handed an open one builds
Tidal. A construct not listed here is a parse error, not a feature request.

A pattern string denotes **one cycle** of material. A trigger pattern says *when*; a value
pattern says *what* (a number, a note name, a slice index, a foley name) at those moments. Both
use one parser. The parser is pure and deterministic: given the string and a cycle index it
returns the same events every time. Random draws are not made here; the leaf only says *where a
draw belongs* and the resolver rolls it at the engine's usual address.

## Grammar

```
pattern   := stack
stack     := sequence ( ',' sequence )*          # ',' only meaningful inside [ ] or at top level
sequence  := step+                                # whitespace-separated
step      := atom modifier*
atom      := token | '~' | '[' stack ']' | '<' sequence '>' | '{' sequence '}' '%' INT
modifier  := '*' INT | '@' NUMBER | '?' | '(' INT ',' INT ( ',' INT )? ')'
token     := 'x' | NUMBER | NOTE | NAME
NUMBER    := '-'? DIGITS ( '.' DIGITS )?
NOTE      := [a-g] ( '#' | 'b' )? '-'? DIGIT          # c1, eb1, bb0, f#-1 — Live's octaves: c3 = 60, c1 = 36
NAME      := [a-z_] [a-z0-9_]*  (not 'x')             # rifle, pistol
INT       := DIGITS
```

| construct | meaning |
|---|---|
| sequence `"x ~ x x"` | the steps divide the cycle equally (or by `@` weight) |
| `~` | rest: a span with no event |
| `[ ]` | subdivide one step: the contents are a sequence over that step's span |
| `< >` | alternate: on cycle *k* (0-based) play element `k mod n`; the chosen element takes the whole step |
| `*n` | **ratchet**: the step is one main event plus `n−1` tail attacks, spaced `span / n` from the step's onset. `*` never means "n sequential steps". n in 2..=8 |
| `@w` | weight: the step takes `w` shares of the enclosing span instead of 1. w is a positive number |
| `?` | a **draw** at the step's own address. On a plain step: the event sounds only if the draw admits it. On a `*n` step: the main event always sounds and the draw gates the *tail* — one draw per main event. `?` is a marker; the chance it is rolled against belongs to the consumer (`trigger.admit`, `ratchet.gate`), never to the string |
| `x(p,s[,r])` | euclid: the step becomes `s` equal sub-steps with `p` pulses by the engine's balanced-modular rule (`(i·p) mod s < p`, first pulse at 0), rotated right by `r` (default 0). `1 ≤ p ≤ s ≤ 1024`; write `~` for silence, not `x(0,s)`. Same convention as `Expression::Euclidean` |
| `{ }%n` | polymeter: the braces hold a sequence of *m* steps played **n steps per cycle**. Step *j* of cycle *k* (0-based) plays element `(k·n + j) mod m`. The sequence does not restart at the cycle line; its own period is `lcm(m,n)/n` cycles |
| `,` | **stack**: simultaneity. `[1,3,5]` is a chord; `[1 3 5]` is an arpeggio. Each stacked sequence spans the same interval; events at equal onsets are simultaneous |

Weights, `*`, `?` and euclid attach to the step they follow, in any order, each at most once.
`?` may follow `*n` (`x*3?`). Nesting depth is limited to 8. Whitespace is insignificant except
as a step separator. The empty pattern is an error. A `< >` with one element is that element.

Note names use Live's octave numbering (MIDI = (octave + 2) · 12 + pitch class): `c3` = 60,
`c1` = 36, `eb1` = 39, `bb0` = 34. A word shaped like a note (`e5`, `d1`) is a note, never a
name; a note outside 0..=127 is an error. A modifier on a composite step (`[x x]*2`, `[x x]?`)
reaches every leaf inside it, each over its own span, and a leaf's own modifier wins; nothing in
either piece uses that and it exists only so the grammar has no undefined corner.

Tokens are checked by the **consumer**, not the parser: a trigger pattern accepts only `x` and
`~`; a value pattern accepts numbers; a note pattern accepts note names or integers; a slice or
foley pattern accepts integers or names. The parser reports which token kinds a pattern contains
so the consumer can refuse a mismatch at load with the string and the offending token.

## Time

Positions are exact rationals over the cycle (numerator/denominator in `u64`, reduced), never
floats. The cycle is `cycle_bars` bars, default 1 bar = 3840 ticks at 960 PPQN. An onset at
fraction `p/q` of a cycle that starts at tick `T0` lands at `T0 + floor(cycle_ticks · p / q)`,
computed in integer arithmetic. **Rounding rule: floor.**

**Which positions are exact.** An onset `p/q`, reduced, is exact when `q` divides `cycle_ticks`
and floored otherwise. There is no list of safe subdivisions: 3840 = 2⁸ · 3 · 5, so `q` is exact
iff it is a divisor of that — up to eight halvings (denominator 256), one factor of 3, one of 5. Products are *not*
closed under it. `[[x x]…]` nested to 64 × 64 needs `q` = 4096 and floors; so does anything
needing a second 3 (a 9-tuplet is 3 × 3), a second 5, or a 7 or 11. The exact/floored line is a
property of the reduced denominator, not of how the pattern was written.

A step's **span** is the interval it occupies, and on the grid `span_ticks = floor(end) −
floor(start)` — the difference of two floored positions, not the floor of the difference. That
is why seven floored steps still tile 3840 ticks exactly with no step lost or doubled.

Ratchet tail attack *i* (1-based, `i < n`) is at `onset + floor(span_ticks · i / n)` — integer
division from the origin, no accumulated rounding, the same rule as `Ornaments::expand`.

**Three limits, and no one of them implies another.** Nesting depth is 8 and any single `( )`
or `%` count is ≤ 1024, but neither bounds anything that matters: nesting is multiplicative, so
`[[x(1024,1024)](1024,1024)](1024,1024)` sits well inside every denominator and still asks for
2³⁰ events. The parser proves all three at parse time, before a consumer ever asks for a cycle.

1. **Denominator representability** — no reachable onset or span needs a denominator past
   `u64`, so rendering cannot panic. This bounds how *fine* a position can be.
2. **Output: at most 4096 events per cycle.** Stacks add, alternations take the widest element,
   polymeter multiplies by its `%n`, euclid by its pulse count. This bounds how much a cycle
   *emits*.
3. **Work: at most 2²⁰ render steps per cycle.** Every `render_step`, every `render_atom`, and
   every euclid slot the loop visits — pulse or no pulse. This bounds how much a cycle *does*,
   which is a different quantity, because **silence is not free**.
   `[[~(1024,1024)](1024,1024)](1024,1024)` emits zero events at every nesting and still walks
   2³⁰ slots to emit them; measured against a build with only limits 1 and 2, it parses clean
   and has not finished rendering after 20 seconds. A limit that counts output accepts it.

Limits 2 and 3 are **conservative upper bounds, not counts**. An alternation is charged its
widest element though only one plays per cycle, and a polymeter its widest element `%n` times
though its cells hold different elements — so `{x ~ x x ~ x ~}%16` is charged 16 events and
renders 9. A refusal therefore says a pattern *may* cost that much, never that it does; the
error messages say "may" for that reason.

Limit 3's unit is a **charged work unit, not a call**. A leaf is charged 1 when it is priced
and 1 more for each of the two visits that reach it, so a plain `x` costs 3 units against 2
named calls. The overcount is deliberate and safe in the only direction that matters — it can
refuse a pattern the renderer would have survived, never admit one it would not.

4096 is a provisional policy cap on output that a real piece may argue up; it carries no claim
about Part grids, since the leaf is not wired to a Part yet. 2²⁰ is a practicality cap with
headroom over the dense patterns a piece is likely to write: the event-saturating
`[x(64,64)](64,64)` costs about 12k steps, and the widest single construct the grammar allows
costs about 3k.

The work cap is **not** the most a pattern inside the event cap can cost — the two bounds are
independent in both directions. Silence is cheap in events and dear in work, so
`[~(1024,1024)](256,256)` emits nothing, costs about 787k, and is accepted;
`[~(1024,1024)](341,341)` is accepted at exactly 2²⁰. In the other direction
`[[x(1,1024)](64,64)](64,64)` is exactly 4096 events — inside the output cap — and refused for
its 4,194,304 innermost slot visits.

**The work cap bounds termination, not latency.** A cycle at the cap is not fast: rendering
`[~(1024,1024)](341,341)` measured ~34 ms per cycle, and the 12k dense baseline ~340 µs
(release build, the art LXC, 2026-09-07 — host-local observations, not a benchmark). Nothing
here promises sub-millisecond rendering. And that cost is not confined to patterns a consumer
could render once and keep: `[~(1024,1024)](340,340)` spends 0.3% of the budget to leave room
for nine stacked alternations, renders at the same ~35 ms, and repeats only every 223,092,870
cycles — with sixteen alternations of distinct prime lengths (2 through 53) the period has no
`u64` representation at all, so `period_cycles()` returns `None` while per-cycle work stays
under the cap. Measured cycles of that pattern cost ~35 ms wherever they are sampled: medians
34.3-35.3 ms at indices 0, 1, 7, 223,092,869, 10^12 and `u64::MAX`, which is what the structure
predicts — the expensive silent leaf is traversed identically on every cycle and the alternation
tail adds nothing measurable. Sampled indices are not all indices, but no index is cheap in the
sample. A consumer putting this leaf on a transport deadline therefore needs an answer for the
cycle it does *not* have rendered yet — cold start, seek, an edit invalidating prepared work, or
a producer falling behind. Caching and a tighter cap are two shapes that answer; a cheaper renderer is another.
That question is open as t-494 and is not settled by this bound.

A number literal that overflows `f64` to infinity is a parse error too, for the same family of
reason: a value consumer cannot tell an inherited infinity from a number it was given.

**The tick grid ends.** `ticks(k, cycle_ticks)` fails rather than saturating when cycle `k` is
not representable — when `(k + 1) · cycle_ticks` exceeds `u64`. Every event satisfies
`onset + span ≤ 1`, so that single check proves no tick, span or tail inside the cycle can
overflow. A clamped tick would be a mistimed event no consumer could distinguish from a real
one, which is the reason the contract is a refusal and not a clamp.

## The leaf in the engine

Literal trigger playback is implemented. Its preparation seam is
available as `music::rhythm::literal::Schedule::prepare(pattern, cycle_bars,
subdivision, rotate)`. A composition prepares each literal trigger before it becomes
a playback snapshot; cold seeks read the same immutable schedule.

Preparation parses once and renders **the complete material period**, checking all
reachable branches for hit-only tokens, duplicate main onsets and exact subdivision
alignment. Alignment is checked on rational onsets before flooring to ticks, and on
the period's return to the Part grid: `x` over one bar on a dotted-sixteenth grid is
refused because its second onset, tick 3840, is off that grid. Each attack retains its
own span, ratchet count and hit/tail draw flag. Ratchets require at least two ticks per
child; `Attack::child_offset` reproduces the notation renderer's floor rule.

This consumer accepts at most **4096 material cycles**, **2²⁰ total renderer visits**
and **65,536 main events** in the prepared period. Work and storage are checked using
the parser's conservative per-cycle bounds times the period, before any cycle is
rendered. A refusal says the bound exceeds the policy, not that every cycle actually
costs its worst case. These are consumer limits, not new grammar restrictions. They
allow a complete sparse schedule instead of a cache with an unprepared-cycle fallback;
long periods and large silent traversals are refused rather than deferred to playback.
Storage follows the event count, not the number of ticks, and clones share it.

`Schedule::at(tick)` returns only an exact main onset's metadata, with binary search,
no allocation and no parser or renderer work. It is a periodic phase query even at
`u64::MAX`; it does not construct an absolute event or authorize a note past the tick
grid. It does not draw probability, displace timing, truncate tails or emit MIDI.
The resolver uses the retained metadata downstream. Its absolute admission check also
requires the complete structural span to fit the tick grid; a valid phase lookup alone
does not establish that. Resource limits apply per source, and the work cap is not a
wall-clock or whole-composition bound. Live transport deadline work remains on t-494.

`Expression::Literal { pattern, cycle_bars, rotate }` joins Euclidean, Binary and Part as a
rhythm leaf. At a Part subdivision step whose tick is `t`:

- the leaf is **active** iff an event's main onset is exactly `t`;
- the step's **ratchet** is the event's `*n` (count *n* over the event's span, not over the
  Part cell), replacing `ornaments.ratchet.count` for that step;
- the step's **draw** flag says whether `?` was written, and whether it gates the hit or the
  tail; the resolver rolls at `step/<part>/<bar>/<slot>` (hit) or `<part>/burst/<bar>/<slot>`
  (tail) — the addresses `until-stop` already names;
- `rotate = r` delays material by `r` Part steps, like signed Euclidean rotation:
  the source position is `t - r * cell`, wrapped over the complete material period.
  Negative values advance it. This includes alternations and polymeters, rather than
  wrapping each bar separately. It moves the playhead, not the cycle origin, so a
  return group containing the leaf keeps its phase.

The authored convenience is `[parts.NAME.trigger] pattern = "x ~ ~ [~ x]"`.
It expands to `rhythm = { type = "literal", pattern = "x ~ ~ [~ x]", cycle_bars = 1,
rotate = 0 }`; the explicit rhythm table also supports cycle lengths of 1..1024 bars.
A literal currently occupies the **trigger root**. Nesting it inside a Boolean rhythm
or using it as an accent source is refused: those combinations do not yet define which
span and ratchet metadata to retain. A Part reference may read a literal Part's main
structure/admission; it does not treat the tails as new structural hits.

`x?` uses `trigger.probability` (set it explicitly; the existing lane default is 1).
Plain `x` and the main attack of `x*n?` are unconditional. For `*n`, the notation owns
count and span. An authored `ornaments.ratchet` supplies its probability/mode, even if
no `?` is written; its count is replaced by the notation's count. Without that gate,
`*n` opens fully and `*n?` uses a 0.5 phrase-locked tail gate. A plain literal hit gets
no tails from an external ratchet count. Existing `fire`/`burst` pin addresses are used;
this slice does not migrate the versioned hash to the example engine's address spelling.

Groove displacement is applied once to the source. Tails keep their offsets over its
written span. The next possible structural main onset reserves its earliest grace;
empty grid cells do not truncate literal tails. Bar/section ownership still clips attacks
and releases, and random seeks include earlier sources in the bar that can sound now.
Pitch is sampled separately at each final attack, including tails. See
`examples/quickstart/literal.toml` for the sub and snare figures adapted from until-stop.

A main onset that is not a multiple of the Part's subdivision cell is a **validation error**
naming the pattern, the onset, and the fix ("set `subdivision` finer, or write `*n`"). Tail
attacks may fall off the grid: they are ornaments and the ornament path already owns sub-cell
ticks. Two stacked events with equal onset in a *trigger* pattern is an error (a drum cannot hit
itself twice at once); in a value pattern it is a chord and legal.

Value patterns are the same parser producing tokens instead of hits. How a consumer walks them
(`per = "event"` read-then-advance, per-step position, per-bar `< >`) is the consumer's contract
and is documented with the consumer, not here.
The first consumer is the pitch sliver — `note.pattern` and `pitch.pattern` as held value
sources sampled per attack, [pitch.md](pitch.md).

## What is deliberately not here

`/` (slow), `!` (replicate), `.` (grouping), `_` (elongate), `|` (random choice), nested `?`
with an explicit probability (`x?0.3`), polymetric steps without `%`, and any operator on a whole
pattern (`.every`, `.sometimes`). None is used by either piece. Adding one is a change to this
file first, with a use in a piece that needs it.

## Worked examples (from the pieces)

| string | cycle 0 events (onset as fraction of the cycle) |
|---|---|
| `x x x [x x?]` | 0, 1/4, 2/4, 3/4, 7/8 (draw gates the last) |
| `x ~ [~ x] [~ x?]` | 0, 5/8, 7/8 (draw gates the last) |
| `~ x ~ <x [x x*2]>` | cycle 0: 1/4, 3/4 · cycle 1: 1/4, 3/4, 7/8 with ratchet 2 over span 1/8 (tail at 15/16) |
| `~ x ~ [x x*3?]` | 1/4, 3/4, 7/8 with ratchet 3 over span 1/8, draw gates the tail |
| `x@3 [~ x]` | 0 (span 3/4), 7/8 |
| `{x ~ x x ~ x ~}%16` | element `(16·k + j) mod 7` at slot *j*: cycle 0 slots 0 2 3 5 7 9 10 12 14 · cycle 1 slots 0 1 3 5 7 8 10 12 14 15 · period 7 cycles |
| `x(5,8)` | 0, 2/8, 4/8, 5/8, 7/8 … by `(i·5) mod 8 < 5`: i = 0,2,4,5,7 |
| `<c1 ~ eb1 bb0>` | c1, no new value, eb1, bb0; the pitch consumer holds c1 through the second cycle |
| `[1,3,5,7,9]` | five simultaneous values at 0 |
| `1 2 3 ~ 5 ~ 7 8 1 ~ 3 4 ~ 6 7 ~` | sixteen slots of slice indices, rests at 3, 5, 9, 12, 15 |
| `~ 12*6` | one event at 1/2, token 12, ratchet 6 over span 1/2 |


The [numeric velocity consumer](event-values.md) now reads flat number sequences by written
position or structural event count. Its event clock and carry are consumer fields; they do
not widen this grammar. Other notation forms are refused by this first velocity consumer.
