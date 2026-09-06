"""Local fixture checks. User Ableton Sets stay outside the public repository."""
import copy
from pathlib import Path
import unittest
import prepared_template as prepared
import cutoff_template as kit

FIXTURES = Path(__file__).resolve().parents[2] / 'recordings/ableton-kit-fixtures'

@unittest.skipUnless((FIXTURES / '909 Core Kit.als').exists(), 'Private Ableton fixtures not installed')
class PreparedKitTests(unittest.TestCase):
    def setUp(self):
        self.base = kit.load(FIXTURES / '909 Core Kit.als')
        self.mapped = kit.load(FIXTURES / '909 Core Kit One Mapped.als')

    def test_mapping_scope_addresses_samples_and_defaults(self):
        original = copy.deepcopy(self.base)
        result, report = prepared.build(self.base, self.mapped)
        self.assertEqual(len(kit.external_mappings(result)), 64)
        self.assertEqual([kit.signature(x) for x in original.iter('SampleRef')],
                         [kit.signature(x) for x in result.iter('SampleRef')])
        # Internal macro links survive unchanged, including decay and mixer level.
        internal = lambda root: [kit.signature(x) for x in root.iter('KeyMidi') if kit.value(x, 'Channel') == '16']
        self.assertEqual(internal(original), internal(result))
        self.assertEqual({p['control_channel'] for p in report}, {15,16})
        for p in report:
            self.assertEqual(set(p['controls']), {'cutoff','level','pan','decay'})
            self.assertEqual(p['controls']['cutoff']['cc'], p['cc'])
            self.assertEqual(p['controls']['level']['default'], 1.0)
            self.assertEqual(p['controls']['pan']['default'], 0.5)
            self.assertTrue(0 <= p['controls']['decay']['default'] <= 1)
        expected = (Path(__file__).resolve().parents[2] / 'templates/project/kits/909-prepared.toml').read_text()
        self.assertEqual(prepared.bindings(report), expected)

    def test_refuses_preexisting_pan_assignment(self):
        voice = kit.voices(self.base)[42]
        import xml.etree.ElementTree as ET
        key = ET.SubElement(voice.find('VolumeAndPan/Panorama'), 'KeyMidi')
        for name,value in [('Channel','16'),('NoteOrController','3')]:
            ET.SubElement(key, name, Value=value)
        with self.assertRaisesRegex(ValueError, 'overwrite'):
            prepared.build(self.base, self.mapped)

    def test_refuses_tail_macro_with_extra_target(self):
        voice = kit.voices(self.base)[42]
        tail_key = voice.find('VolumeAndPan/Envelope/DecayTime/KeyMidi')
        voice.find('VolumeAndPan/Volume').insert(1, copy.deepcopy(tail_key))
        with self.assertRaisesRegex(ValueError, 'another parameter'):
            prepared.build(self.base, self.mapped)

if __name__ == '__main__':
    unittest.main()
