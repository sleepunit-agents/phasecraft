"""Simulates actual kit Lua; hardware display colors still need a listening pass."""
from pathlib import Path
import unittest
import test_script

class KitTest(unittest.TestCase):
    def setUp(self):
        stub=test_script.AdapterTest();stub.setUp();self.lua=stub.lua;self.g=stub.g
        self.lua.execute('sent={};labels={};rings={};titles={};current_page=1')
        self.lua.execute(Path(__file__).with_name('kit.e16script').read_text())
        self.g.page.onInit()
    def feedback(self,page=1,slot=0,flags=9,number='75% ',label='CHat'):
        b=[240,125,80,67,1,4,1,4,page,slot,flags,64,0,*label.encode(),*number.encode(),247]
        self.g.controller.onSysex(self.lua.table_from(b))
    def test_budget_and_dynamic_labels_pending_then_applied(self):
        source=Path(__file__).with_name('kit.e16script').read_text()
        code='\n'.join(l for l in source.splitlines() if not l.lstrip().startswith('--'))
        self.assertLess(len(code.encode()),4000)
        self.assertEqual(self.g.sent[1][6],6)
        self.feedback(flags=11)
        for _ in range(30):self.g.system.update()
        self.assertEqual(self.g.labels[1],'75% ')
        self.assertEqual(self.g.rings[1][3],20)
        self.feedback(flags=13)
        self.g.system.update();self.assertEqual(self.g.labels[1],'CHat');self.assertEqual(self.g.rings[1][3],0)
    def test_select_unmapped_part_and_second_bank(self):
        self.feedback(flags=8,slot=5,label='Snar')
        self.g.controller.onEncoderPress(self.lua.table_from(dict(page=1,index=6,id=38)))
        self.assertEqual(list(self.g.sent[len(self.g.sent)].values()),[240,125,80,67,1,5,1,4,1,5,247])
        self.g.current_page=3;self.g.page.onPageChange(1,3)
        self.feedback(page=3,slot=15)
        self.g.controller.onEncoderTurn(self.lua.table_from(dict(page=3,index=16,id=32,increment=-1)))
        self.assertEqual(list(self.g.sent[len(self.g.sent)].values()),[240,125,80,67,1,2,1,4,3,15,63,247])
        self.g.current_page=4;self.g.page.onPageChange(3,4)
        before=len(self.g.sent)
        self.g.controller.onEncoderPress(self.lua.table_from(dict(page=4,index=1,id=33)))
        self.g.system.update();self.assertEqual(len(self.g.sent),before)
    def test_host_title_and_empty_slot_are_safe(self):
        self.feedback(flags=0)
        before=len(self.g.sent)
        self.g.controller.onEncoderPress(self.lua.table_from(dict(page=1,index=1,id=33)))
        self.assertEqual(len(self.g.sent),before)
        self.g.controller.onSysex(self.lua.table_from([240,125,80,67,1,7,1,4,1,*b'Kit 1 / CHat   ',247]))
        self.g.system.update();self.g.system.update()
        self.assertEqual(self.g.titles[len(self.g.titles)],'Kit 1 / CHat   ')

if __name__=='__main__':unittest.main()
