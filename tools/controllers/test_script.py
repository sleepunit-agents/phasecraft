"""Development-only Lua API simulation; does not establish firmware compatibility."""
from pathlib import Path
import unittest
from lupa import LuaRuntime

SCRIPT = Path(__file__).with_name("kick.e16script")

class AdapterTest(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute('''
          current_page=1;sent={};labels={};rings={};titles={};writes={}
          controller={getPage=function() return current_page end,
            set=function(id,k,v) writes[#writes+1]={id,k,v} end}
          page={resetTitle=function() end,setTitle=function(t) titles[#titles+1]=t end}
          slots={reset=function(i) labels[i]=nil end, update=function(i,s) labels[i]=s end}
          leds={reset=function(i) rings[i]=nil end,updateByIndex=function(b) rings=b end}
          midi={sendSysex=function(out,b) assert(out==0);sent[#sent+1]=b end}
          system={setUpdateRate=function(ms) assert(ms==40) end}
        ''')
        self.lua.execute(SCRIPT.read_text())
        self.g = self.lua.globals()
        self.g.page.onInit()
    def feedback(self, page=1, slot=0, enabled=True, generation=132, text="99% "):
        body=[240,125,80,67,1,4,generation//128,generation%128,page,slot,int(enabled),64,0,*b"Kick",*text.encode(),247]
        self.g.controller.onSysex(self.lua.table_from(body))
    def turn(self, page=1, index=1, id=13, delta=-1):
        self.g.controller.onEncoderTurn(self.lua.table_from(dict(page=page,index=index,id=id,increment=delta)))
    def test_budget(self):
        self.assertLess(len(SCRIPT.read_bytes()),4000)
        self.assertLessEqual(len(SCRIPT.name),16)
    def test_handshake_turn_display_and_feedback_is_not_input(self):
        self.assertEqual(list(self.g.sent[1].values()),[240,125,80,67,1,1,1,247])
        self.turn();self.assertEqual(len(self.g.sent),1) # no host yet
        self.feedback();self.assertEqual(len(self.g.sent),1)
        self.turn();self.assertEqual(list(self.g.sent[2].values()),[240,125,80,67,1,2,1,4,1,0,63,247])
        self.g.system.update();self.assertEqual(self.g.labels[1],"99% ")
        self.assertEqual(self.g.writes[len(self.g.writes)][3],8192)
        for _ in range(25):self.g.system.update()
        self.assertEqual(self.g.labels[1],"Kick")
        self.turn(delta=0);self.assertEqual(len(self.g.sent),3) # heartbeat only
    def test_wrong_assignment_unavailable_and_off_page_are_ignored(self):
        self.feedback();self.turn(id=1);self.assertEqual(len(self.g.sent),1)
        self.feedback(enabled=False);self.turn();self.assertEqual(len(self.g.sent),1)
        self.g.current_page=3;self.g.page.onPageChange(1,3)
        for _ in range(100):self.g.system.update()
        self.assertEqual(len(self.g.sent),1);self.assertIsNone(self.g.labels[1])
    def test_page_transition_requires_fresh_feedback_and_disconnect_times_out(self):
        self.feedback();self.g.current_page=2;self.g.page.onPageChange(1,2)
        self.turn(page=2,id=1);self.assertEqual(len(self.g.sent),2)
        self.feedback(page=1);self.turn(page=2,id=1);self.assertEqual(len(self.g.sent),2)
        self.feedback(page=2);self.turn(page=2,id=1);self.assertEqual(len(self.g.sent),3)
        for _ in range(77):self.g.system.update()
        before=len(self.g.sent);self.turn(page=2,id=1)
        self.assertEqual(len(self.g.sent),before)
        self.assertEqual(self.g.titles[len(self.g.titles)],"Connect player")

if __name__ == "__main__":unittest.main()
