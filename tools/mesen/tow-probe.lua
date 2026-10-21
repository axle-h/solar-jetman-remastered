-- Black-box probe of the tractor beam's effect on the pod in the original: a recorded input
-- sequence (SJ_REPLAY_IN, as tools/mesen/replay-capture.lua reads it) is played from the start
-- of level 0 until the beam holds an item; from then on, before each logic step, the pod's
-- speeds are set to 0 and the beam's state (rope angle, angular speed, sense, load class) to
-- the next probe's; one step runs with no buttons held and the pod's speeds and the beam's
-- state after it are logged. The effects the specs do not give (player.md 8.3) are fitted
-- from this log.
-- Environment: SJ_REPLAY_IN, SJ_PROBE_OUT (the log path), SJ_PROBE_MASS (the load class,
-- default 0), SJ_PROBE_X and SJ_PROBE_Y (where the pod is parked, in open sky: use x 635, y 120),
-- SJ_PROBE_SPEED (whole part of both speeds, default 0), SJ_PROBE_ANGLES (a list
-- of angles to repeat).
local finished = false
local function guard(fn)
  return function(...)
    if finished then return end
    local ok, err = pcall(fn, ...)
    if not ok then
      print("tow-probe: " .. tostring(err))
      emu.stop(1)
    end
  end
end

local RAM = emu.memType.nesInternalRam
local function r(a) return emu.read(a, RAM) end
local function w(a, v) emu.write(a, v & 0xFF, RAM) end
local function w16(hi, lo) return r(hi) * 256 + r(lo) end

local out = assert(io.open(assert(os.getenv("SJ_PROBE_OUT")), "w"))
local mass = tonumber(os.getenv("SJ_PROBE_MASS") or "0")

local BUTTONS = { A = "a", B = "b", SEL = "select", START = "start",
                  U = "up", D = "down", L = "left", R = "right" }
local steps = {}
for line in assert(io.open(assert(os.getenv("SJ_REPLAY_IN")), "r")):lines() do
  line = line:gsub("#.*", "")
  local tokens = {}
  for t in line:gmatch("%S+") do tokens[#tokens + 1] = t end
  if #tokens > 0 and tonumber(tokens[1]) then
    local held = {}
    for i = 2, #tokens do
      local name = BUTTONS[tokens[i]]
      if name then held[name] = true end
    end
    for _ = 1, tonumber(tokens[1]) do steps[#steps + 1] = held end
  end
end
local probing = false
local sample = 0

-- The probes: angle, angular speed (24 bits), sense, horizontal direction, vertical direction.
local probes = {}
local only = os.getenv("SJ_PROBE_ANGLES")   -- "8,4" repeats these angles 40 times each
if only then
  for a in only:gmatch("%d+") do
    for dy = 0, 15 do
      for dx = 0, 15 do
        probes[#probes + 1] = { angle = tonumber(a), w = 0, sense = 0, hl = 0, vd = 1, dx = dx, dy = dy }
      end
    end
  end
else
  for _, wspeed in ipairs({ 0, 0x2000, 0x4000, 0x8000, 0xF000 }) do
    for sense = 0, 1 do
      for hl = 0, 1 do
        for vd = 0, 1 do
          for i = 0, 127 do
            local angle = os.getenv("SJ_PROBE_REVERSE") and (127 - i) or i
            probes[#probes + 1] = { angle = angle, w = wspeed, sense = sense, hl = hl, vd = vd }
          end
        end
      end
    end
  end
end

local current = 0
local frame = 0
local passes = 0
local pass_pending = nil
local started = false
local pending = nil

local base = nil
local function setup(p)
  if not base then
    base = { x = w16(0x21F, 0x200), y = w16(0x27C, 0x25D), xf = r(0x197), yf = r(0x23E) }
  end
  local jitter = os.getenv("SJ_PROBE_JITTER") and (current % 40) or 0
  local pdx, pdy = p.dx or 0, p.dy or 0
  if os.getenv("SJ_PROBE_Y") then base.y = tonumber(os.getenv("SJ_PROBE_Y")) end
  if os.getenv("SJ_PROBE_X") then base.x = tonumber(os.getenv("SJ_PROBE_X")) end
  w(0x200, base.x + jitter % 5 + pdx); w(0x21F, (base.x + jitter % 5 + pdx) >> 8); w(0x197, (base.xf + 7 * jitter) & 0xFF)
  w(0x25D, base.y + jitter // 5 + pdy); w(0x27C, (base.y + jitter // 5 + pdy) >> 8); w(0x23E, (base.yf + 13 * jitter) & 0xFF)
  local speed = tonumber(os.getenv("SJ_PROBE_SPEED") or "0")
  w(0x2BA, speed); w(0x29B, 0); w(0x336, p.hl)
  w(0x2F8, speed); w(0x2D9, 0); w(0x355, p.vd)
  w(0xC4, 0); w(0xC5, 0)
  w(0x67, mass)
  w(0x68, p.frac or 0); w(0x69, p.angle)
  w(0x6A, p.w & 0xFF); w(0x6B, (p.w >> 8) & 0xFF); w(0x6C, p.w >> 16)
  w(0x6D, p.sense)
  w(0x99, 0); w(0x97, 0)
end

emu.addEventCallback(guard(function()
  local start = (frame >= 1300 and frame < 1305) or (frame >= 1500 and frame < 1505)
  local held = {}
  if r(0x50A) + 256 * r(0x50B) == 0xB0B6 then
    if not probing then held = steps[passes // 2 + 1] or {} end
  elseif start then
    held = { start = true }
  end
  emu.setInput(held, 0)
end), emu.eventType.inputPolled)


emu.addMemoryCallback(guard(function(address)
  local where = emu.convertAddress(address, emu.memType.nesMemory)
  if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= 0x30B6 then return end
  passes = passes + 1
  pass_pending = r(0x5D) & 1
end), emu.callbackType.exec, 0xB0B6)

emu.addMemoryCallback(guard(function(address)
  if pass_pending == nil then return end
  local where = emu.convertAddress(address, emu.memType.nesMemory)
  if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= 0x1570 then return end
  local parity = pass_pending
  pass_pending = nil
  if parity == 1 and not probing then
    sample = sample + 1
    if sample > tonumber(os.getenv("SJ_PROBE_AFTER") or "20") and r(0x66) ~= 0 then
      probing = true
      current = 1
      setup(probes[current])
    end
  elseif parity == 1 and current > 0 then
    local p = probes[current]
    out:write(string.format(
      "a=%d w=%X s=%d hl=%d vd=%d | hw=%02X hf=%02X hl=%d ach=%02X vw=%02X vf=%02X vd=%d acv=%02X | " ..
      "t66=%02X t68=%02X t69=%02X t6a=%02X t6b=%02X t6c=%02X t6d=%02X y=%04X yf=%02X x=%04X xf=%02X\n",
      p.angle, p.w, p.sense, p.hl, p.vd,
      r(0x2BA), r(0x29B), r(0x336) & 1, r(0xC4), r(0x2F8), r(0x2D9), r(0x355) & 1, r(0xC5),
      r(0x66), r(0x68), r(0x69), r(0x6A), r(0x6B), r(0x6C), r(0x6D),
      w16(0x27C, 0x25D), r(0x23E), w16(0x21F, 0x200), r(0x197)))
    current = current + 1
    if current > #probes then
      out:close()
      finished = true
      emu.stop(0)
    else
      setup(probes[current])
    end
  end
end), emu.callbackType.exec, 0x9570)

emu.addEventCallback(guard(function() frame = frame + 1 end), emu.eventType.endFrame)
