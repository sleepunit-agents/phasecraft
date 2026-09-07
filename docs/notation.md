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
| `x(p,s[,r])` | euclid: the step becomes `s` equal sub-steps with `p` pulses by the engine's balanced-modular rule (`(i·p) mod s < p`, first pulse at 0), rotated right by `r` (default 0). Same convention as `Expression::Euclidean` |
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
computed in integer arithmetic. **Rounding rule: floor.** Every position reachable by `[ ]`
subdivision of a bar into 2, 3, 4, 5, 6, 8, 12, 16, 24, 32, 48, 64 parts and their products is
exact at 3840 ticks; the rule only matters for 7-, 9-, 11-tuplets, and it is stated so nobody
has to guess.

A step's **span** is the interval it occupies. Ratchet tail attack *i* (1-based, `i < n`) is at
`onset + floor(span_ticks · i / n)` — integer division from the origin, no accumulated rounding,
the same rule as `Ornaments::expand`.

## The leaf in the engine

`Expression::Literal { pattern, cycle_bars, rotate }` joins Euclidean, Binary and Part as a
rhythm leaf. At a Part subdivision step whose tick is `t`:

- the leaf is **active** iff an event's main onset is exactly `t`;
- the step's **ratchet** is the event's `*n` (count *n* over the event's span, not over the
  Part cell), replacing `ornaments.ratchet.count` for that step;
- the step's **draw** flag says whether `?` was written, and whether it gates the hit or the
  tail; the resolver rolls at `step/<part>/<bar>/<slot>` (hit) or `<part>/burst/<bar>/<slot>`
  (tail) — the addresses `until-stop` already names;
- `rotate = r` reads the pattern `r` Part steps later, like signed Euclidean rotation. It moves
  the playhead, not the cycle origin, so a return group containing the leaf keeps its phase.

A main onset that is not a multiple of the Part's subdivision cell is a **validation error**
naming the pattern, the onset, and the fix ("set `subdivision` finer, or write `*n`"). Tail
attacks may fall off the grid: they are ornaments and the ornament path already owns sub-cell
ticks. Two stacked events with equal onset in a *trigger* pattern is an error (a drum cannot hit
itself twice at once); in a value pattern it is a chord and legal.

Value patterns are the same parser producing tokens instead of hits. How a consumer walks them
(`per = "event"` read-then-advance, per-step position, per-bar `< >`) is the consumer's contract
and is documented with the consumer, not here.

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
| `<c1 c1 eb1 bb0>` | one note per cycle: c1, c1, eb1, bb0, repeating |
| `[1,3,5,7,9]` | five simultaneous values at 0 |
| `1 2 3 ~ 5 ~ 7 8 1 ~ 3 4 ~ 6 7 ~` | sixteen slots of slice indices, rests at 3, 5, 9, 12, 15 |
| `~ 12*6` | one event at 1/2, token 12, ratchet 6 over span 1/2 |
