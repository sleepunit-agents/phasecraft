# Phasecraft overnight handoff

Work in progress. See [the finite checklist](overnight-plan.md).

Starting release: `3031717`. Jonathan confirmed the prepared kit and Stop-default
fix work in Live. Existing Set: `group-booth/phasecraft-909-prepared-v1.zip` on drop.

Completed checkpoints and listening instructions will be recorded here as shipped.

## 1. Breathing automation

Implemented segments with linear/smooth/hold curves, fractional musical durations,
delayed starts and independent repeating cycles. Existing ramps and Stop defaults
remain compatible. New project example: **breathing**, using the existing Prepared
Set. Listen for sixteen bars to hear the complete open/hold/close/hold cycle.
Engine: 80 tests pass, including exact boundaries, repeat equality, inherited ramp
replacement, invalid durations and unchanged original 35-bar provenance.

## 2. Meter, space and touch

Implemented offbeat emphasis, first-hit-after-gap gain and isolated deterministic
timing/velocity humanization. Compare **garage-touch** with **garage**; source
trigger and accent decisions are identical. No kit changes needed. Timing stays
inside the source step and never affects MIDI clock. 83 Rust tests pass, including
RNG isolation, phase boundaries, context, bounds and the paired-example comparison.

Automation release `3d1864a`: all native jobs passed; published updater commit, four
signed targets and Windows installer checksum verified. Groove release `fdda262`
also passed all platforms; published feed and Windows checksum verified.

## 3. Shared emphasis with memory

**accent-memory** uses one named seven-step accent lane across hat and rim. Each
Part's cutoff profile accumulates recent accented hits and decays through rests;
Stop returns to the kit default. It uses the existing Prepared 909 Set. Source
accents combine by maximum amount, and never create notes. This is a control
envelope inspired by the accent-memory requirement, not a pitched 303 emulation.
New tests cover shared decisions, no-note behavior, finite decay/accumulation,
onset timing, query-order independence, RNG locality and maximum bounded load.

Accent-memory validation: 88 Rust tests, eight browser checks, root/desktop clippy,
and native Linux player smoke passed. Four bars: 124 note messages, 202 CCs,
384 clock pulses, zero dropped notes; maximum measured lateness 2.092 ms locally.
