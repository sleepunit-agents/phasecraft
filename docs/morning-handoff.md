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
