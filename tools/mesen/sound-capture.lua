-- Logs the original's sound driver, update by update, for `sj-tools sound-compare`.
-- Run through tools/mesen/run.sh; parameters come from the environment:
--   SJ_SOUND_OUT     absolute path of the log to write
--   SJ_SOUND_FRAMES  frames to run from power-on
--   SJ_SOUND_SONG    optional "frame tempo t0 t1 t2 t3" (decimal frame and tempo, hex track
--                    addresses): at that frame's driver update, put the driver's RAM in the
--                    state a song start leaves it in (docs/spec/sound.md 5), as a black-box
--                    way to start any song
--   SJ_SOUND_REQUESTS optional "frame slot channel id;..." (decimal frame, slot A or B, hex
--                    channel code $15/$18/$1B/$1E and id): effect requests put in the request
--                    slots of sound.md 8.2 at the end of that frame
--   SJ_SOUND_EFFECTS optional "frame channel id address;..." (decimal frame and channel 0-3,
--                    hex id and bank 4 address of the effect, or 06E8 for the RAM effect):
--                    at that frame's driver update, start the effect as sound.md 7.1 says,
--                    in RAM, unless that id is already playing on the channel
--   SJ_SOUND_RAM_EFFECT optional frame: at that frame's driver update, copy the RAM effect's
--                    6-byte template (bank 0, PRG ROM 0x07273) to RAM $06E8, as the game
--                    does at game start (sound.md 4.3)
--
-- Log lines:
--   u FRAME                 a driver update starts (bank 4 entry $800F, rom-map.md 1); it
--                           ends when the driver returns
--   x ADDR=VAL ...          driver RAM the game changed since the last update ended
--   s HEX                   then the whole driver RAM: $F8, $F9, then $0637-$06ED
--   w REG VAL               an APU write during the update
--   g FRAME REG VAL         an APU write outside driver updates
--   end FRAME               the run is over
-- Errors inside callbacks would otherwise hang the run silently.
local finished = false
local function guard(fn)
  return function(...)
    if finished then return end
    local ok, err = pcall(fn, ...)
    if not ok then
      print("sound-capture: " .. tostring(err))
      emu.stop(1)
    end
  end
end

local function main()
  local RAM = emu.memType.nesInternalRam
  local out = assert(io.open(os.getenv("SJ_SOUND_OUT"), "w"))
  local total = tonumber(os.getenv("SJ_SOUND_FRAMES")) or 3000

  local song = nil
  local spec = os.getenv("SJ_SOUND_SONG")
  if spec and spec ~= "" then
    local f, tempo, a, b, c, d = spec:match("(%d+) (%d+) (%x+) (%x+) (%x+) (%x+)")
    song = { frame = tonumber(f), tempo = tonumber(tempo),
             tracks = { tonumber(a, 16), tonumber(b, 16), tonumber(c, 16), tonumber(d, 16) } }
  end
  local requests = {}
  for f, slot, channel, id in (os.getenv("SJ_SOUND_REQUESTS") or ""):gmatch("(%d+) (%a) (%x+) (%x+)") do
    local list = requests[tonumber(f)] or {}
    list[#list + 1] = { slot = slot, channel = tonumber(channel, 16), id = tonumber(id, 16) }
    requests[tonumber(f)] = list
  end

  local effects = {}
  for f, k, id, address in (os.getenv("SJ_SOUND_EFFECTS") or ""):gmatch("(%d+) (%d) (%x+) (%x+)") do
    local list = effects[tonumber(f)] or {}
    list[#list + 1] = { channel = tonumber(k), id = tonumber(id, 16), address = tonumber(address, 16) }
    effects[tonumber(f)] = list
  end
  local ram_effect_frame = tonumber(os.getenv("SJ_SOUND_RAM_EFFECT") or "")

  local function start_effect(effect)
    local base = 4 * effect.channel
    if emu.read(0x0648 + base, RAM) == effect.id then
      return
    end
    local header = {}
    for i = 0, 2 do
      if effect.address < 0x8000 then
        header[i] = emu.read(effect.address + i, RAM)
      else
        header[i] = emu.read(0x20000 + effect.address - 0x8000 + i, emu.memType.nesPrgRom)
      end
    end
    local start = effect.address + 3
    emu.write(0x0648 + base, effect.id, RAM)
    emu.write(0x064A + base, start & 0xFF, RAM)
    emu.write(0x064B + base, start >> 8, RAM)
    emu.write(0x0649 + base, header[0] & 0x0F, RAM)
    emu.write(0x0668 + base, header[0] & 0xF0, RAM)
    emu.write(0x0669 + base, header[1], RAM)
    emu.write(0x066B + base, header[2], RAM)
    emu.write(0x0698 + base, 0, RAM)
  end

  local addresses = { 0xF8, 0xF9 }
  for a = 0x0637, 0x06ED do addresses[#addresses + 1] = a end
  local function snapshot()
    local t = {}
    for i, a in ipairs(addresses) do t[i] = emu.read(a, RAM) end
    return t
  end

  local frame = 0
  local in_update = false
  local last = nil
  local watch_return -- defined below

  emu.addMemoryCallback(guard(function(address)
    local where = emu.convertAddress(address, emu.memType.nesMemory)
    if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= 0x2000F then
      return
    end
    if song and frame == song.frame then
      for k = 0, 3 do
        local base = 4 * k
        emu.write(0x0638 + base, 1, RAM)
        emu.write(0x0639 + base, 1, RAM)
        emu.write(0x063A + base, song.tracks[k + 1] & 0xFF, RAM)
        emu.write(0x063B + base, song.tracks[k + 1] >> 8, RAM)
        emu.write(0x0688 + base, 0, RAM)
        emu.write(0x068B + base, 0, RAM)
        emu.write(0x0698 + base, 0, RAM)
        emu.write(0x06D8 + base, 0, RAM)
      end
      emu.write(0xF8, 0, RAM)
      emu.write(0xF9, song.tempo, RAM)
      song = nil
    end
    if ram_effect_frame == frame then
      for i = 0, 5 do
        emu.write(0x06E8 + i, emu.read(0x07273 + i, emu.memType.nesPrgRom), RAM)
      end
    end
    for _, effect in ipairs(effects[frame] or {}) do
      start_effect(effect)
    end
    out:write(string.format("u %d\n", frame))
    local now = snapshot()
    local changed = {}
    for i, value in ipairs(now) do
      if last == nil or last[i] ~= value then
        changed[#changed + 1] = string.format("%04X=%02X", addresses[i], value)
      end
    end
    if #changed > 0 then
      out:write("x " .. table.concat(changed, " ") .. "\n")
      local hex = {}
      for i, value in ipairs(now) do hex[i] = string.format("%02X", value) end
      out:write("s " .. table.concat(hex) .. "\n")
    end
    in_update = true
    watch_return()
  end), emu.callbackType.exec, 0x800F)

  -- An update ends when the driver returns to its caller: the return address is read from
  -- the stack at entry, and watched from then on.
  local return_to = nil
  local watched = {}
  watch_return = function()
    local sp = emu.getState()["cpu.sp"]
    local low = emu.read(0x100 + ((sp + 1) & 0xFF), emu.memType.nesMemory)
    local high = emu.read(0x100 + ((sp + 2) & 0xFF), emu.memType.nesMemory)
    return_to = (high * 256 + low + 1) & 0xFFFF
    if not watched[return_to] then
      watched[return_to] = true
      emu.addMemoryCallback(guard(function(address)
        if in_update and address == return_to then
          in_update = false
          last = snapshot()
        end
      end), emu.callbackType.exec, return_to)
    end
  end

  emu.addMemoryCallback(guard(function(address, value)
    if in_update then
      out:write(string.format("w %04X %02X\n", address, value))
    else
      out:write(string.format("g %d %04X %02X\n", frame, address, value))
    end
  end), emu.callbackType.write, 0x4000, 0x4017)

  emu.addEventCallback(guard(function()
    for _, request in ipairs(requests[frame] or {}) do
      local base = request.slot == "A" and 0xF3 or 0xF5
      emu.write(base, request.channel, RAM)
      emu.write(base + 1, request.id, RAM)
    end
    frame = frame + 1
    if frame >= total then
      out:write(string.format("end %d\n", frame))
      out:close()
      finished = true
      emu.stop(0)
    end
  end), emu.eventType.endFrame)
end

local ok, err = pcall(main)
if not ok then
  print("sound-capture: " .. tostring(err))
  emu.stop(1)
end
