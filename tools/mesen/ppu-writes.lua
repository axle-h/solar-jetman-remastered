-- Logs the original's writes to video memory as runs, for working out which loads a scene makes.
-- Run through tools/mesen/run.sh; parameters from the environment:
--   SJ_SCENE_OUT    absolute path of a directory (must exist); writes "ppu-writes.txt"
--   SJ_SCENE_FROM   first frame to log;  SJ_SCENE_TO  last frame (also where the run stops)
--   SJ_SCENE_INPUT  as in title-capture.lua
-- A line per run of consecutive $2007 writes: frame, first PPU address, byte count, increment, and
-- the first byte written (hex).
local finished = false
local out = os.getenv("SJ_SCENE_OUT")
local from = tonumber(os.getenv("SJ_SCENE_FROM") or "0")
local to = tonumber(os.getenv("SJ_SCENE_TO") or "1600")
local inputs = {}
for f, b in string.gmatch(os.getenv("SJ_SCENE_INPUT") or "", "(%d+):([%a+]+)") do
  inputs[tonumber(f)] = b
end
local log = assert(io.open(out .. "/ppu-writes.txt", "w"))
local frame = 0
local latch, addr, inc = false, 0, 1
local run = nil
local function flush()
  if run then
    log:write(string.format("%d %04X %d %d %02X\n", run.frame, run.start, run.n, run.inc, run.first))
    run = nil
  end
end
emu.addMemoryCallback(function(_, value)
  if value ~= nil then inc = (value & 4) ~= 0 and 32 or 1 end
end, emu.callbackType.write, 0x2000)
emu.addMemoryCallback(function(_, value)
  if not latch then addr = (value & 0x3F) << 8 else addr = (addr & 0xFF00) | value end
  latch = not latch
  flush()
end, emu.callbackType.write, 0x2006)
emu.addMemoryCallback(function(_, value)
  if frame >= from then
    if run and run.next == addr and run.inc == inc then
      run.n = run.n + 1
      run.next = addr + inc
    else
      flush()
      run = { frame = frame, start = addr, n = 1, inc = inc, first = value, next = addr + inc }
    end
  end
  addr = (addr + inc) & 0x3FFF
end, emu.callbackType.write, 0x2007)
emu.addMemoryCallback(function() latch = false end, emu.callbackType.read, 0x2002)
emu.addEventCallback(function()
  local coming = frame + 1
  local now = {}
  for f, buttons in pairs(inputs) do
    if coming >= f and coming < f + 2 then
      for b in string.gmatch(buttons, "[%a]+") do now[b] = true end
    end
  end
  emu.setInput(now, 0)
end, emu.eventType.inputPolled)
emu.addEventCallback(function()
  frame = frame + 1
  if frame >= to and not finished then
    flush()
    log:close()
    finished = true
    emu.stop(0)
  end
end, emu.eventType.endFrame)
