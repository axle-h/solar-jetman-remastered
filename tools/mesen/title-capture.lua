-- Logs the title, password, flight and launch screens of the original frame by frame, for
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
      print("title-capture: " .. tostring(err))
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
local RAM = emu.memType.nesInternalRam
local log = assert(io.open(out .. "/scroll.txt", "w"))
local frame = 0

local function dump(path, memtype, size)
  local f = assert(io.open(path, "a"))
  local row = {}
  for i = 0, size - 1 do row[#row + 1] = string.format("%02X", emu.read(i, memtype)) end
  f:write(table.concat(row, " "), "\n")
  f:close()
end

-- Also "sound.txt": every sound-effect start (the driver writes the effect's id to RAM $0648,
-- $064C, $0650 or $0654 for its channel: "FRAME e CHANNEL ID") and every call of the song
-- entries ("FRAME m S|A SONG", bank 0 $D480 if different, $D488 always).
local slog = assert(io.open(out .. "/sound.txt", "w"))
for _, address in ipairs({ 0x648, 0x64C, 0x650, 0x654 }) do
  emu.addMemoryCallback(guard(function(a, value)
    if value ~= 0 then slog:write(string.format("%d e %d %02X\n", frame, (a - 0x648) // 4, value)) end
  end), emu.callbackType.write, address)
end
for _, entry in ipairs({ { "S", 0x5480 }, { "A", 0x5488 } }) do
  emu.addMemoryCallback(guard(function(address)
    local where = emu.convertAddress(address, emu.memType.nesMemory)
    if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= entry[2] then
      return
    end
    slog:write(string.format("%d m %s %d\n", frame, entry[1], emu.getState()["cpu.a"]))
  end), emu.callbackType.exec, entry[2] + 0x8000)
end

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
  local st = emu.getState()
  local t = st["ppu.tmpVideoRamAddr"]
  local sx = (t & 31) * 8 + st["ppu.xScroll"]
  local sy = ((t >> 5) & 31) * 8 + ((t >> 12) & 7)
  log:write(string.format("%d %02X %02X %02X %02X %02X %02X %02X %d %d %02X %02X\n", frame,
    r(0x0C), r(0x0D), r(0x5D), r(0x5E), r(0x5F), r(0x60), r(0x5A), sx, sy, r(0x5B), r(0x5C)))
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
    slog:close()
    finished = true
    emu.stop(0)
  end
end), emu.eventType.endFrame)
