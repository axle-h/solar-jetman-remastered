-- Measures the RGB colour Mesen shows for each of the 64 NES colours, for comparing the remake's
-- pictures (NES colour numbers) with Mesen's screenshots exactly (`sj-tools replay-compare`).
-- Run through tools/mesen/run.sh; SJ_PALETTE_OUT is the absolute path of the table to write: 64
-- lines "INDEX RRGGBB" (hex).
-- Each frame writes one colour to palette entry 0 and reads it back from the next frame's
-- picture, at a pixel of the backdrop. A frame in which the game itself wrote the entry is
-- skipped and that colour measured again.
local finished = false
local out = assert(os.getenv("SJ_PALETTE_OUT"))
local PAL = emu.memType.nesPaletteRam
local measured, frame, wrote = {}, 0, nil

emu.addEventCallback(function()
  if finished then return end
  local ok, err = pcall(function()
    frame = frame + 1
    if wrote and emu.read(0, PAL) == wrote then
      -- The centre of the licence screen and of the blank frames before it is backdrop.
      measured[wrote] = emu.getScreenBuffer()[120 * 256 + 128 + 1] & 0xFFFFFF
    end
    wrote = nil
    for i = 0, 63 do
      if not measured[i] then wrote = i break end
    end
    if not wrote or frame > 400 then
      local f = assert(io.open(out, "w"))
      for i = 0, 63 do f:write(string.format("%02X %06X\n", i, measured[i] or 0)) end
      f:close()
      finished = true
      emu.stop(wrote and 1 or 0)
      return
    end
    if frame >= 2 then emu.write(0, wrote, PAL) else wrote = nil end
  end)
  if not ok then
    print("palette-capture: " .. tostring(err))
    emu.stop(1)
  end
end, emu.eventType.endFrame)
