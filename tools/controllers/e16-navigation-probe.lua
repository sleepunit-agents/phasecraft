-- Phasecraft navigation probe. Display only: no MIDI output or persistent writes.
-- Assign Turn and Push to encoder 1 on native pages 1 and 2.
--@assign id=1 abbr="TURN" name="Probe Turn" l=0 h=127 manual=true
--@assign id=2 abbr="PUSH" name="Probe Push" p=true

local changes, turns, presses = 0, 0, 0

local function draw()
  page.setTitle("Nav probe")
  slots.update(1, "P" .. controller.getPage())
  slots.update(2, "E" .. (changes % 1000))
  slots.update(3, "T" .. (turns % 1000))
  slots.update(4, "B" .. (presses % 1000))
end

function page.onInit()
  draw()
end

function page.onPageChange(previous_page, new_page)
  changes = changes + 1
  for i = 1, 16 do slots.reset(i) end
  draw()
end

function controller.onEncoderTurn(enc)
  turns = turns + 1
  draw()
end

function controller.onEncoderPress(enc)
  presses = presses + 1
  draw()
end
