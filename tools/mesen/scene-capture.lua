-- Like force-capture.lua, plus SJ_RAM (hex addresses appended to each log line). Forced RAM writes, then the title, password, flight and launch screens frame by frame, for
-- comparing them with the remake (`sj-tools scene --compare`; the screenshots are "shot-FRAME.png",
-- the sprite memory of the frame before is in "shot-FRAME-1.txt"). Run through tools/mesen/run.sh; parameters come from
-- the environment:
--   SJ_SCENE_OUT     absolute path of a directory (must exist) for the log and screenshots
--   SJ_SCENE_FRAMES  comma list of frames at which to save a screenshot, a palette and sprite
--                    memory dump ("shot-FRAME.png", "shot-FRAME.txt")
--   SJ_SCENE_LAST    last frame to run (default 1600)
--   SJ_SCENE_INPUT   comma list "FRAME:BUTTONS" (BUTTONS joined by +: start, select, a, b, up,
--                    down, left, right): held for the 2 frames FRAME and FRAME + 1 (the frames
--                    whose pass reads them, numbered as in the log)
-- Also at each screenshot frame: "shot-FRAME.chr" (12 KB: pattern memory, then nametable page
-- 0 and its attributes), for working out which tiles a scene loads.
-- Log "scroll.txt": one line per frame: frame, RAM $0C $0D (title sub-state and timer), $5D $5E
-- $5F $60 (counters), $5A (random byte) in hex, then the scroll x and y in decimal (from the
-- PPU: the position the frame's picture was drawn at), then $5B and $5C (the rest of the random
-- generator's state) in hex.
local finished = false
local function guard(fn)
  return function(...)
    if finished then return end
    local ok, err = pcall(fn, ...)
    if not ok then
      print("force-capture: " .. tostring(err))
      emu.stop(1)
    end
  end
end

local out = os.getenv("SJ_SCENE_OUT")
local last = tonumber(os.getenv("SJ_SCENE_LAST") or "1600")
local shots, inputs = {}, {}
for f in string.gmatch(os.getenv("SJ_SCENE_FRAMES") or "", "%d+") do shots[tonumber(f)] = true end
for f, b in string.gmatch(os.getenv("SJ_SCENE_INPUT") or "", "(%d+):([%a+]+)") do
  inputs[tonumber(f)] = b
end
--   SJ_WRITES        comma list "FRAME:ADDR=VALUE" (hex ADDR and VALUE): at the end of frame FRAME
--                    the byte of internal RAM is set (forces a level, a delivery, a state)
-- The log's lines also end with $050A (scene handler), $3B (level), $A4 (surface mode), $A1 $A5
-- (menu layout and cursor), $DB $DA (message kind and timer) in hex.
local writes = {}
for f, a, v in string.gmatch(os.getenv("SJ_WRITES") or "", "(%d+):(%x+)=(%x+)") do
  writes[#writes + 1] = { tonumber(f), tonumber(a, 16), tonumber(v, 16) }
end
-- SJ_RAM: comma list of hex RAM addresses whose bytes are appended to each log line (hex).
local watch = {}
for a in string.gmatch(os.getenv("SJ_RAM") or "", "(%x+)") do watch[#watch + 1] = tonumber(a, 16) end
local RAM = emu.memType.nesInternalRam
local log = assert(io.open(out .. "/scroll.txt", "w"))
local frame = 0

-- Log "sound.txt": the sound requests, as replay-capture.lua logs them (sound.md 7.1, 8.1): one
-- line "FRAME e CHANNEL.ID" for every effect the driver starts (a write of its id to the
-- channel's effect byte RAM $0648, $064C, $0650, $0654) and "FRAME m KIND.SONG" for every call of
-- the two song entries (bank 0 $D480 "if different" S, $D488 "always" A; the song is in A).
-- FRAME is the frame whose pass made the request, numbered as in scroll.txt. `sj-tools scene
-- --compare` compares them with the remake's.
local sound = assert(io.open(out .. "/sound.txt", "w"))
for _, address in ipairs({ 0x648, 0x64C, 0x650, 0x654 }) do
  emu.addMemoryCallback(guard(function(a, value)
    if value ~= 0 then sound:write(string.format("%d e %d.%02X\n", frame + 1, (a - 0x648) // 4, value)) end
  end), emu.callbackType.write, address)
end
local function log_song(kind, offset)
  emu.addMemoryCallback(guard(function(address)
    local where = emu.convertAddress(address, emu.memType.nesMemory)
    if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= offset then
      return
    end
    sound:write(string.format("%d m %s.%d\n", frame + 1, kind, emu.getState()["cpu.a"]))
  end), emu.callbackType.exec, offset + 0x8000)
end
log_song("S", 0x5480)
log_song("A", 0x5488)

local function dump(path, memtype, size)
  local f = assert(io.open(path, "a"))
  local row = {}
  for i = 0, size - 1 do row[#row + 1] = string.format("%02X", emu.read(i, memtype)) end
  f:write(table.concat(row, " "), "\n")
  f:close()
end

-- "shot-FRAME.p0" (1 KB): nametable page 0 read at the start of the frame (the end-of-frame
-- dump of the ".chr" file shows page 1 while a status panel is selected).
emu.addEventCallback(guard(function()
  if shots[frame + 1] then
    local p = assert(io.open(string.format("%s/shot-%d.p0", out, frame + 1), "wb"))
    for i = 0, 0x3FF do p:write(string.char(emu.read(0x2000 + i, emu.memType.nesPpuMemory))) end
    p:close()
  end
end), emu.eventType.startFrame)

-- The controller is read in the frame's own interrupt: the buttons set before frame F run are
-- the ones its pass sees.
emu.addEventCallback(guard(function()
  local coming = frame + 1
  local now = {}
  for from, buttons in pairs(inputs) do
    if coming >= from and coming < from + 2 then
      for b in string.gmatch(buttons, "[%a]+") do now[b] = true end
    end
  end
  emu.setInput(now, 0)
end), emu.eventType.inputPolled)

emu.addEventCallback(guard(function()
  frame = frame + 1
  local function r(a) return emu.read(a, RAM) end
  for _, w in ipairs(writes) do
    if w[1] == frame then emu.write(w[2], w[3], RAM) end
  end
  local st = emu.getState()
  local t = st["ppu.tmpVideoRamAddr"]
  local sx = (t & 31) * 8 + st["ppu.xScroll"]
  local sy = ((t >> 5) & 31) * 8 + ((t >> 12) & 7)
  log:write(string.format("%d %02X %02X %02X %02X %02X %02X %02X %d %d %02X %02X %02X %02X %02X %02X %02X %02X %02X", frame,
    r(0x0C), r(0x0D), r(0x5D), r(0x5E), r(0x5F), r(0x60), r(0x5A), sx, sy, r(0x5B), r(0x5C),
    r(0x050A), r(0x3B), r(0xA4), r(0xA1), r(0xA5), r(0xDB), r(0xDA)))
  for _, a in ipairs(watch) do log:write(string.format(" %02X", r(a))) end
  log:write("\n")
  if shots[frame] then
    local png = assert(io.open(string.format("%s/shot-%d.png", out, frame), "wb"))
    png:write(emu.takeScreenshot())
    png:close()
    local path = string.format("%s/shot-%d.txt", out, frame)
    os.remove(path)
    dump(path, emu.memType.nesPaletteRam, 32)
    dump(path, emu.memType.nesSpriteRam, 256)
    local chr = assert(io.open(string.format("%s/shot-%d.chr", out, frame), "wb"))
    for i = 0, 0x23FF do chr:write(string.char(emu.read(i, emu.memType.nesPpuMemory))) end
    chr:close()
  end
  if frame >= last then
    log:close()
    sound:close()
    finished = true
    emu.stop(0)
  end
end), emu.eventType.endFrame)
