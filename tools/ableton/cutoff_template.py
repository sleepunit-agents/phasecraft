#!/usr/bin/env python3
"""Build a Live 12.4.3 cutoff-only test Set from the supplied 909 fixtures.

Maintainer tool, standard library only. Does not modify inputs or existing outputs.
The generated Set still requires an opening/listening check in Ableton Live.
"""
import argparse
import copy
import gzip
import hashlib
import json
import tomllib
from pathlib import Path
import xml.etree.ElementTree as ET

# MIDI note -> (stable role, external control channel). All note channels remain 10.
PADS = {
    36: ("kick", 1), 37: ("rim", 16), 38: ("snare", 2), 39: ("clap", 3),
    40: ("kick_alt", 8), 41: ("snare_alt", 9), 42: ("closed_hat", 4),
    43: ("snare_alt_2", 10), 44: ("low_tom", 6), 45: ("mid_tom", 11),
    46: ("open_hat", 5), 47: ("high_tom", 12), 48: ("crash", 13),
    49: ("crash_alt", 14), 50: ("ride_alt", 15), 51: ("ride", 7),
}
TARGETS = {"AutomationTarget", "ModulationTarget"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load(path):
    data = gzip.decompress(Path(path).read_bytes())
    root = ET.fromstring(data)
    require(root.tag == "Ableton" and root.get("Creator") == "Ableton Live 12.4.3"
            and root.get("MinorVersion") == "12.0_12402", "Expected the verified Live 12.4.3 structure")
    return root


def value(parent, path):
    item = parent.find(path)
    require(item is not None and "Value" in item.attrib, f"Missing {path}")
    return item.get("Value")


def voices(root):
    tracks = root.findall("./LiveSet/Tracks/MidiTrack")
    require(len(tracks) == 1, "Expected one MIDI track")
    racks = tracks[0].findall(".//DrumGroupDevice")
    require(len(racks) == 1, "Expected one Drum Rack")
    result = {}
    for branch in racks[0].findall("./Branches/DrumBranch"):
        # This serialization is reverse-indexed; the verified rim stores 91 -> note 37.
        note = 128 - int(value(branch, "./BranchInfo/ReceivingNote"))
        instruments = branch.findall(".//OriginalSimpler")
        require(len(instruments) == 1 and note not in result, "Expected one Simpler per unique pad")
        result[note] = instruments[0]
    require(set(result) == set(PADS), "Expected exactly the supplied 16 pads, notes 36..51")
    return result


def external_mappings(root):
    return [m for m in root.iter("KeyMidi") if 0 <= int(value(m, "Channel")) < 16]


def signature(node):
    """Ignore formatting only; preserve every element, attribute and meaningful text."""
    return (node.tag, tuple(sorted(node.attrib.items())), (node.text or "").strip(),
            tuple(signature(child) for child in node))


def build(base, mapped):
    original = copy.deepcopy(base)
    source_voices = voices(mapped)
    template = source_voices[37].find("./Filter/Slot/Value/SimplerFilter")
    require(template is not None, "Mapped fixture needs the rim filter")
    key = template.find("./Freq/KeyMidi")
    require(key is not None and value(key, "Channel") == "15"
            and value(key, "NoteOrController") == "20"
            and value(key, "IsNote") == "false"
            and value(key, "ControllerMapMode") == "0", "Expected rim channel 16 / CC20 mapping")
    require(not external_mappings(base), "Base Set must not contain external mappings")
    require(len(external_mappings(mapped)) == 1, "Mapped fixture must have just the one external mapping")
    require(all(e.tag in TARGETS or e.tag == "SimplerFilter" for e in template.iter() if "Id" in e.attrib),
            "Unexpected ID-bearing element in filter; review before cloning")
    require(all(set(e.attrib) <= {"Id", "Value", "LomId"} for e in template.iter()),
            "Unexpected filter attributes; review references before cloning")
    next_id = base.find("./LiveSet/NextPointeeId")
    require(next_id is not None, "Missing global ID allocator")
    allocated = int(next_id.get("Value"))
    known_ids = {int(e.get("Id")) for e in base.iter()
                 if e.tag in TARGETS | {"Pointee"} and e.get("Id") is not None}
    require(allocated > max(known_ids), "NextPointeeId must exceed existing target IDs")
    report = []
    for note, instrument in sorted(voices(base).items()):
        role, channel = PADS[note]
        filter_node = instrument.find("./Filter")
        require(filter_node is not None, f"Missing filter container on {role}")
        slot = filter_node.find("./Slot/Value")
        require(slot is not None, f"Missing filter slot on {role}")
        existing = slot.find("SimplerFilter")
        added = existing is None
        if added:
            require(len(slot) == 0, f"Unknown filter type on {role}")
            existing = copy.deepcopy(template)
            for e in existing.iter():
                if e.tag in TARGETS:
                    e.set("Id", str(allocated))
                    allocated += 1
            slot.append(existing)
        freq = existing.find("Freq")
        require(freq is not None, f"Missing filter frequency on {role}")
        old_key = freq.find("KeyMidi")
        require(added or old_key is None, f"Existing frequency binding on {role}; do not overwrite")
        if old_key is not None:
            freq.remove(old_key)
        binding = copy.deepcopy(key)
        binding.find("Channel").set("Value", str(channel - 1))
        # Follow the working fixture's parameter child order: LomId, KeyMidi, ...
        freq.insert(1, binding)
        filter_node.find("./IsOn/Manual").set("Value", "true")
        limits = freq.find("MidiControllerRange")
        require(limits is not None and value(limits, "Min") == "30"
                and value(limits, "Max") == "22000", f"Unexpected cutoff range on {role}")
        report.append({"role": role, "note": note, "note_channel": 10,
                       "control_channel": channel, "cc": 20, "filter_created": added,
                       "cutoff_min_hz": 30, "cutoff_max_hz": 22000})
    next_id.set("Value", str(allocated))
    require(len(external_mappings(base)) == 16, "Expected sixteen external assignments")
    targets = [e.get("Id") for e in base.iter() if e.tag in TARGETS]
    require(len(targets) == len(set(targets)), "Duplicate automation/modulation target ID")
    # Prove no sample, routing, macro, mixer, timing or other device data changed.
    stripped = copy.deepcopy(base)
    stripped.find("./LiveSet/NextPointeeId").set("Value", value(original, "./LiveSet/NextPointeeId"))
    before = voices(original)
    for note, instrument in voices(stripped).items():
        instrument.remove(instrument.find("Filter"))
        old = before[note]
        instrument.insert(list(old).index(old.find("Filter")), copy.deepcopy(old.find("Filter")))
    require(signature(stripped) == signature(original), "Changes escaped the intended filter/ID scope")
    return base, report


def write_bundle(base_path, mapped_path, output):
    base, report = build(load(base_path), load(mapped_path))
    xml = ET.tostring(base, encoding="utf-8", xml_declaration=True)
    require(signature(ET.fromstring(xml)) == signature(base), "XML round-trip failed")
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False)
    als = output / "Phasecraft 909 Cutoff Test.als"
    als.write_bytes(gzip.compress(xml, mtime=0))
    project = output / "phasecraft-cutoff-check"
    (project / "compositions").mkdir(parents=True)
    names = []
    bindings = []
    for pad in report:
        role, note, channel = pad["role"], pad["note"], pad["control_channel"]
        name = f"compositions/{note:02}-{role}.toml"
        names.append(name)
        bindings.append(f'''[library.behaviors."kit.target.{role}".output]
note = {note}
channel = 10
controls.cutoff = {{ cc = 20, channel = {channel} }}
''')
        (project / name).write_text(f'''# Alternating dark/bright quarter notes. CC resets open at note-off.
tempo = 100
seed = 1
phrase_bars = 1
[parts.{role}]
compose = ["std.no_accent", "kit.target.{role}"]
trigger.rhythm = {{ steps = 4, pulses = 1 }}
accent.rhythm = {{ steps = 8, pulses = 1 }}
accent.probability = 1.0
accent.amount = 1.0
output.gate_ticks = 239
profile.base = 90
profile.boost = 0
profile.controls.cutoff = {{ base = 1.0, boost = -0.85 }}
''')
    (project / "kit.toml").write_text("\n".join(bindings))
    (project / "phasecraft.toml").write_text(
        'name = "909 cutoff check"\ndefault = '+json.dumps(names[0])+'\ncompositions = '+json.dumps(names)+'\nlibraries = ["kit.toml"]\nmidi = "midi.toml"\n')
    (project / "midi.toml").write_text('port = "Phasecraft"\nsend_clock = false\n')
    for config in project.rglob("*.toml"):
        tomllib.loads(config.read_text())
    manifest = {"status": "Awaiting Live 12.4.3 opening/listening validation",
                "creator": base.get("Creator"), "base_sha256": hashlib.sha256(Path(base_path).read_bytes()).hexdigest(),
                "mapped_sha256": hashlib.sha256(Path(mapped_path).read_bytes()).hexdigest(),
                "output_sha256": hashlib.sha256(als.read_bytes()).hexdigest(), "pads": report}
    (output / "mapping-report.json").write_text(json.dumps(manifest, indent=2)+'\n')
    (output / "README.txt").write_text('''PHASECRAFT 909 CUTOFF TEST — generated, awaiting a real Live opening check

1. Open Phasecraft 909 Cutoff Test.als in Live 12.4.3.
2. Select the loopMIDI input on the drum track, enable monitoring/arm as usual.
   Track and Remote must be ON for that INPUT in Live MIDI settings.
3. In Phasecraft Player, Open project -> phasecraft-cutoff-check, select your
   loopMIDI destination, and play the first composition. No app update needed.
4. Inspect the kick's Simpler filter frequency. It should alternate dark/bright
   with the quarter-note hits, returning fully open at note-off. Only this pad's
   cutoff should move. Stop; select the next composition to check another pad.
5. Live MIDI Map mode should show 16 external CC20 assignments on channels 1..16.
   The original internal rack macros remain in place. Do not move a stock macro
   during this isolation test; it may control related instrument settings.
6. Save As a fresh Set, close/reopen and retest. Report any missing samples,
   load errors, wrong-pad movement or failed mappings before adopting this kit.

This test maps each existing Simpler's actual Filter Frequency directly. It does
not add a new visible rack macro, change samples, or convert to Drum Sampler.
Pads lacking filters get a copy of the working rim filter with fresh target IDs.
Existing filters/settings are preserved and all are enabled. Range 30–22000 Hz
matches the known working rim mapping. Track mix/mute state comes from the original
unmapped Set (not the muted mapped test Set).

The original factory sample references are preserved for your installation.
Long tails may brighten at note-off because current Phasecraft CCs are momentary.
This tests mapping generation, not held parameter behavior or the final kit sound.
See mapping-report.json for the complete pad/channel/CC list.
All note/channel/CC assignments live in phasecraft-cutoff-check/kit.toml.
The compositions contain rhythm and emphasis, not Ableton addresses.
''')
    print(json.dumps({"output": str(output), "pads": len(report),
                      "filters_created": sum(p["filter_created"] for p in report)}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", type=Path)
    parser.add_argument("mapped", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    write_bundle(args.base, args.mapped, args.output)


if __name__ == "__main__":
    main()
