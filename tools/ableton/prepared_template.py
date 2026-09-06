#!/usr/bin/env python3
"""Generate a compact 909 Set with cutoff, level, pan and stock tail controls.

Uses only the verified user fixtures. Never overwrites a bundle or an input.
"""
import argparse
import copy
import gzip
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET
import cutoff_template as kit


def bind(parameter, key, channel, cc, low=None, high=None):
    kit.require(parameter is not None, 'Missing control parameter')
    kit.require(parameter.find('KeyMidi') is None, 'Refusing to overwrite an existing mapping')
    mapping = copy.deepcopy(key)
    mapping.find('Channel').set('Value', str(channel - 1))
    mapping.find('NoteOrController').set('Value', str(cc))
    parameter.insert(1, mapping)
    limits = parameter.find('MidiControllerRange')
    kit.require(limits is not None, 'Missing mapping limits')
    if low is not None:
        limits.find('Min').set('Value', str(low))
    if high is not None:
        limits.find('Max').set('Value', str(high))


def build(base, mapped):
    root, report = kit.build(base, mapped, compact=True)
    before = copy.deepcopy(root)
    key = mapped.find('.//OriginalSimpler/Filter/Slot/Value/SimplerFilter/Freq/KeyMidi')
    kit.require(key is not None, 'Missing verified CC mapping')
    parents = {child: parent for parent in root.iter() for child in parent}
    changed = []
    for pad in report:
        voice = kit.voices(root)[pad['note']]
        rack = parents[voice]
        while rack.tag != 'InstrumentGroupDevice':
            rack = parents[rack]
        channel, slots = pad['control_channel'], pad['reserved_ccs']
        controls = {'cutoff': {'cc': slots[0], 'default': 1.0, 'target': 'Simpler filter frequency', 'min': 30, 'max': 22000}}
        def assign(name, parameter, slot, neutral, target, low=None, high=None):
            # Snapshot exact allowed mutation scope for the structural comparison.
            changed.append((parameter, copy.deepcopy(parameter)))
            bind(parameter, key, channel, slots[slot], low, high)
            controls[name] = {'cc': slots[slot], 'default': neutral, 'target': target,
                              'min': float(kit.value(parameter, 'MidiControllerRange/Min')),
                              'max': float(kit.value(parameter, 'MidiControllerRange/Max'))}
        levels = [i for i in range(16) if kit.value(rack, f'MacroDisplayNames.{i}') == 'Level']
        kit.require(len(levels) == 1, 'Expected one stock Level macro')
        level = rack.find(f'MacroControls.{levels[0]}')
        # Full-scale returns exactly to the stock macro position, without +6dB headroom.
        assign('level', level, 2, 1.0, 'Stock Level macro (chain mixer)', 0, kit.value(level, 'Manual'))
        pan = voice.find('VolumeAndPan/Panorama')
        kit.require(kit.value(pan, 'Manual') == '0', 'Expected centered stock pan')
        assign('pan', pan, 5, 0.5, 'Simpler pan', -1, 1)
        # Closed hat is already shaped by actual ADSR decay. Other pads use release.
        tail_name = 'DecayTime' if pad['note'] == 42 else 'ReleaseTime'
        tail = voice.find(f'VolumeAndPan/Envelope/{tail_name}')
        existing = tail.find('KeyMidi')
        if existing is not None:
            kit.require(kit.value(existing, 'Channel') == '16', 'Expected an internal macro binding')
            index = int(kit.value(existing, 'NoteOrController'))
            targets = [p for p in rack.iter() if p.find('KeyMidi') is not None
                       and kit.value(p, 'KeyMidi/Channel') == '16'
                       and kit.value(p, 'KeyMidi/NoteOrController') == str(index)]
            kit.require(targets == [tail], 'Tail macro also controls another parameter; review required')
            macro = rack.find(f'MacroControls.{index}')
            neutral = float(kit.value(macro, 'Manual')) / 127
            assign('decay', macro, 1, neutral, f'Stock {kit.value(rack, f"MacroDisplayNames.{index}")} macro -> {tail_name}')
        else:
            neutral = 1.0
            assign('decay', tail, 1, neutral, f'Simpler {tail_name} (tail after note-off)', 1, kit.value(tail, 'Manual'))
        controls['decay']['envelope_parameter'] = tail_name
        controls['decay']['tail_min_ms'] = float(kit.value(tail, 'MidiControllerRange/Min'))
        controls['decay']['tail_max_ms'] = float(kit.value(tail, 'MidiControllerRange/Max'))
        pad['controls'] = controls
    kit.require(len(kit.external_mappings(root)) == 64, 'Expected 64 external mappings')
    addresses = [(kit.value(k, 'Channel'), kit.value(k, 'NoteOrController')) for k in kit.external_mappings(root)]
    kit.require(len(set(addresses)) == 64, 'Duplicate external address')
    # Restore only the 48 declared parameter nodes in a copy; everything else must match.
    stripped = copy.deepcopy(root)
    paths = {id(child): path for child, path in walk_paths(root)}
    for parameter, old in changed:
        path = paths[id(parameter)]
        parent = stripped
        for index in path[:-1]:
            parent = parent[index]
        parent.remove(parent[path[-1]])
        parent.insert(path[-1], old)
    kit.require(kit.signature(stripped) == kit.signature(before), 'Changes escaped declared parameter scope')
    return root, report


def walk_paths(node, path=()):
    yield node, path
    for i, child in enumerate(node):
        yield from walk_paths(child, path + (i,))


def bindings(report):
    lines = ['# compact-v1: notes on channel 10; controls on 15/16. Defaults restore on Stop.']
    for pad in report:
        lines.extend([f'[library.behaviors."kit.prepared.{pad["role"]}".output]',
                      f'note = {pad["note"]}', 'channel = 10'])
        for name, control in pad['controls'].items():
            lines.append(f'controls.{name} = {{ cc = {control["cc"]}, channel = {pad["control_channel"]}, default = {control["default"]} }}')
        lines.append('')
    return '\n'.join(lines)


def write_bundle(base_path, mapped_path, output):
    root, report = build(kit.load(base_path), kit.load(mapped_path))
    xml = ET.tostring(root, encoding='utf-8', xml_declaration=True)
    kit.require(kit.signature(ET.fromstring(xml)) == kit.signature(root), 'XML round trip failed')
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False)
    als = output / 'Phasecraft 909 Prepared.als'
    als.write_bytes(gzip.compress(xml, mtime=0))
    project = output / 'phasecraft-prepared-check'
    (project / 'compositions').mkdir(parents=True)
    (project / 'kit.toml').write_text(bindings(report))
    (project / 'midi.toml').write_text('port = "Phasecraft"\nsend_clock = false\n')
    names = []
    for pad in report:
        for name in ['cutoff', 'level', 'pan', 'decay']:
            file = f'compositions/{pad["note"]}-{pad["role"]}-{name}.toml'
            names.append(file)
            low, high = (0.1, 0.9) if name == 'pan' else (0.05, 1.0)
            (project / file).write_text(f'''# Isolate one control; Stop restores its kit default.
tempo = 100
seed = 1
phrase_bars = 4
[parts.{pad['role']}]
compose = ["std.no_accent", "kit.prepared.{pad['role']}"]
trigger.rhythm = {{ steps = 4, pulses = 1 }}
output.gate_ticks = 120
profile.base = 90
parameters.{name} = {{ value = {low}, ramp = {{ to = {high}, over_bars = 4 }} }}
''')
    (project / 'phasecraft.toml').write_text('name = "Prepared 909 checks"\ndefault = '+json.dumps(names[0])+'\ncompositions = '+json.dumps(names)+'\nlibraries = ["kit.toml"]\nmidi = "midi.toml"\n')
    manifest = {'layout': 'compact-v1', 'status': 'Awaiting Live opening/listening check',
                'creator': root.get('Creator'), 'pads': report,
                'output_sha256': hashlib.sha256(als.read_bytes()).hexdigest()}
    (output / 'mapping-report.json').write_text(json.dumps(manifest, indent=2)+'\n')
    (output / 'README.txt').write_text('''PHASECRAFT PREPARED 909 — cutoff / level / pan / decay

Open Phasecraft 909 Prepared.als in Live 12.4.3. Enable Track + Remote on the
loopMIDI INPUT; monitor the drum track. Update Phasecraft before testing.
Open phasecraft-prepared-check to isolate each of the 64 mappings, or use a fresh
Phasecraft project's movement example. Existing intro cutoff addresses still match.
Stop midway and check the parameter returns to its kit default. Save As, reopen,
and check mappings and sample references. No samples are bundled or changed.

Level controls the existing per-pad mixer Level macro, independently of velocity.
Its maximum is the saved stock macro position; minimum is the stock macro minimum.
Pan controls Simpler pan directly; 7-bit center is approximate.
Decay preserves the stock closed hat's envelope Decay; other pads use Release
(tail after note-off), via the existing macro when bound. Some stock macros are
named Tone. This is the stock Simpler adapter, not a universal onset-decay model.
Short samples cannot be extended beyond their recorded tail. The report gives
exact targets, ranges, defaults and addresses. Existing internal macros are kept.
Macro defaults may round by up to half a MIDI step on reset.

Cutoff opens fully on Stop. Level returns to stock level, pan to approximately
center, and decay to the saved stock setting. Only controls used during playback
are reset. This restores declared defaults, not arbitrary knob changes in Live.
A listening check in Live is still needed for new level/pan/decay mappings.
''')
    print(json.dumps({'output': str(output), 'mappings': 64}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('base', type=Path)
    parser.add_argument('mapped', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    write_bundle(args.base, args.mapped, args.output)
