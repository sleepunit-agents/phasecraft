# Cutoff-only Live fixture experiment

This is an offline maintainer tool, not a runtime dependency or general ALS editor.
It accepts the two user-provided Live 12.4.3 909 Core Kit fixtures (not committed):

```sh
python3 tools/ableton/cutoff_template.py \
  'recordings/ableton-kit-fixtures/909 Core Kit.als' \
  'recordings/ableton-kit-fixtures/909 Core Kit One Mapped.als' \
  /tmp/phasecraft-909-cutoff-test
phasecraft validate /tmp/phasecraft-909-cutoff-test/phasecraft-cutoff-check
```

Python 3.11+ standard library only. Output must be a new directory. The generated
bundle contains a Set, a portable Phasecraft project with 16 isolated pad checks,
separate `kit.toml` output bindings, a mapping report and listening instructions.
Users only need the generated bundle, not Python or a rebuilt Phasecraft executable.

Existing filters remain; empty slots receive the real mapped rim filter with fresh
global automation/modulation target IDs. Each cutoff gets CC20 on its assigned
channel, preserving the eight-voice draft contract and adding channels for the
other eight pads. All 16 channels are used in this experiment. Internal channel-16
XML KeyMidi entries are preserved; they are not treated as ordinary external MIDI
channels (which serialize as 0..15). No other parameter/device data is changed.

Validation includes expected fixture version/shape, refusal of preexisting external
bindings in the base, unique output addresses, allocator bounds, unique target IDs,
XML round-trip, and a structural comparison proving that edits are restricted to
filters and the global ID allocator. Generated TOML is parsed and must also pass
Phasecraft's validator. The local fixture run is deterministic and passed negative
checks for missing pads, invalid ID allocation and unexpected existing mappings.

The original fixtures remain unchanged and outside Git. Generated Sets still need
to be opened, played and saved/reopened in Live before claiming compatibility.

Pass `--compact` to generate the new two-control-channel layout. It reserves eight
CC slots per pad across channels15/16 while mapping only cutoff. Both layouts use
the same source fixtures and validation; consumers must use the generated matching
kit.toml. Compact validation additionally checked all 128 reserved `(channel, CC)`
slots are unique and that the 16 serialized external mappings match the report.
Jonathan has confirmed the original per-pad-channel Set works perfectly in Live;
the compact revision awaits the same user check.
