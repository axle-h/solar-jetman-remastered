-- Plays a recorded input sequence in the original from the start of a planet (level 0 unless the
-- replay says "level P") and logs the pod's
-- state once per logic step, for `sj-tools replay-compare`.
-- Run through tools/mesen/run.sh; parameters come from the environment:
--   SJ_REPLAY_IN   absolute path of the replay (tools/mesen/replays/*.replay)
--   SJ_REPLAY_OUT  absolute path of the log to write
--   SJ_REPLAY_SHOTS absolute path of a directory (optional, must exist): the picture after every
--                  step, "step-N.png", with "step-N.txt" (palette memory, then sprite memory,
--                  as title-capture.lua dumps them). It is the frame whose NMI starts the next
--                  step's B pass: it shows the sprite table the step's B pass built and the
--                  scroll at the camera that pass left; the frame before still shows the camera
--                  without the B pass's half-step (seen in Mesen; game-flow.md 1.2).
--
-- Replay format: one line per run, "COUNT TOKEN...", meaning COUNT logic steps with those
-- buttons held (A B SEL START U D L R; "-" for none). "dc HEX" sets RAM $DC (the pod's
-- equipment, game-flow.md 9) at the start of play. "flag P" sets the warpship flag of planet P
-- (0-11) then. "weapon HEX" sets RAM $D1 (the fitted special
-- weapon, economy.md 6) at the start, a test setup; "device HEX" sets RAM $D3 (the fitted pod
-- device), "bullet HEX" RAM $050E (bullet upgrade), "gauge HEX" (four digits) the weapon gauge
-- RAM $C9:$C8, all at the start. "lives HEX" sets Jetman's lives RAM $3E at the start (1 makes
-- the next loss the last). "fuel HEX" sets the fuel RAM $C7:$C6 before
-- the next step (a test setup: it empties the tank to destroy the pod or Jetman). "#" starts a
-- comment. "demo N" instead of input: the Nth attract demo of a power-on run (no button is
-- ever pressed; the log starts at that demo's first pass and has no step limit); the log then
-- also holds the autopilot's memory and bytes (wp, tm, ih, ip) and the thrust carry-in byte (cc).
--
-- "warp X Y CX CY" (decimal) places the pod at rest at (X, Y) and the camera at (CX, CY)
-- before the next step, like "fuel" a test setup that lets a replay meet objects far from the
-- start. After step 1 the game then redraws the whole background at that camera (levels.md,
-- Appendix A): 10 or 20 frames with the screen blank and no NMI, the game state untouched.
-- "level P" (0-12, default 0) plays planet P: RAM $3B is written at frame 1301.
-- "level L" with L 13-25 plays the second level L (levels.md, Appendix A): planet L - 13 is
-- started, and 150 gameplay frames in, transition 3 is requested; the log starts where the
-- new level's gameplay does. "level 26" is the final level: from planet 0, 150 frames in, RAM
-- $DB = 26, $9A = $80 and transition 5; at the new level's first pass the player state
-- ($0317) = 6, $9A = $80 and the camera = (0, 928) are written (the route recipe), so the
-- camera follows its route by itself. "level L" with L 29-41 plays the warp level L: planet
-- L - 29 is started, its second level is entered as above, and 20 gameplay frames into that, the
-- planet's flag is set (RAM $DE / $DF), the guard RAM $41 is cleared and transition 3 is
-- requested again (Appendix A); the log starts where the warp level's gameplay does.
-- On planets 9-11 the idle pod would fall back into the bay before the transition 150 frames in,
-- so until then its position is pinned and its speeds and fuel reset each frame.
-- "obj SLOT STATE X Y FRAME INDEX" (hex STATE FRAME INDEX, decimal SLOT X Y) writes object slot
-- SLOT as a fresh object (state, position, frame, list index; the rest cleared) before the next step.
-- "podsync" is read by sj-tools only (the pod's fields are taken from the log after every step).
-- "keepfuel" sets the fuel to $E000 before every following step.
-- "spin HEX" writes the tractor beam's swing count RAM $6E before the next step.
-- "item N KIND X Y" (hex KIND, decimal X Y) writes entry N of the level's items list (RAM
-- $0513, $052D, $0561, $0547, $057B) before the next step, a test setup that puts any item
-- kind beside the pod.
-- Menus: Start at frame 1300 and 1500 start the level (levels.md, Appendix A). Gameplay starts
-- at the first entry of the gameplay NMI handler (bank 0 $B0B6) and every step is two passes.
-- Input for step k is held for passes 2k and 2k+1 (the game reads it before the A pass).
--
-- Log (pod fields, score, then the object slots in use; names in src/game's Gameplay::snapshot,
-- object_fields and attached_fields): "init k=HEX ..." before the first pass, then "s N cut k=HEX ..." for each step N
-- (1-based: the number of A passes so far) at the first arrival in the idle loop (bank 0 $9570)
-- after a B pass. cut=1 when the step's A pass was cut short by the next frame (game-flow.md
-- 1.4). A step that ends in a scene change has no "s" line. "away N" says gameplay was left
-- after N steps; "reload N k=HEX ..." that it came back (a respawn), with the state at the
-- first pass; "left gameplay N" ends the log when it does not come back within 900 frames.
local finished = false
local function guard(fn)
  return function(...)
    if finished then return end
    local ok, err = pcall(fn, ...)
    if not ok then
      print("replay-capture: " .. tostring(err))
      emu.stop(1)
    end
  end
end

local BUTTONS = { A = "a", B = "b", SEL = "select", START = "start",
                  U = "up", D = "down", L = "left", R = "right" }

local keepfuel = nil
local weapon, device, bullet, gauge, lives = nil, nil, nil, nil, nil
local flags = {}
local function parse_replay(path)
  local steps, dc, pokes, demo, level, warps, items, spins, objs = {}, nil, {}, nil, 0, {}, {}, {}, {}
  for line in assert(io.open(path, "r")):lines() do
    line = line:gsub("#.*", "")
    local tokens = {}
    for t in line:gmatch("%S+") do tokens[#tokens + 1] = t end
    if tokens[1] == "level" then
      level = tonumber(tokens[2])
    elseif tokens[1] == "demo" then
      demo = tonumber(tokens[2])
    elseif tokens[1] == "dc" then
      dc = tonumber(tokens[2], 16)
    elseif tokens[1] == "weapon" then
      weapon = tonumber(tokens[2], 16)
    elseif tokens[1] == "device" then
      device = tonumber(tokens[2], 16)
    elseif tokens[1] == "bullet" then
      bullet = tonumber(tokens[2], 16)
    elseif tokens[1] == "flag" then
      flags[#flags + 1] = tonumber(tokens[2])
    elseif tokens[1] == "lives" then
      lives = tonumber(tokens[2], 16)
    elseif tokens[1] == "gauge" then
      gauge = tonumber(tokens[2], 16)
    elseif tokens[1] == "warp" then
      warps[#steps + 1] = { tonumber(tokens[2]), tonumber(tokens[3]),
                            tonumber(tokens[4]), tonumber(tokens[5]) }
    elseif tokens[1] == "obj" then
      local at = #steps + 1
      objs[at] = objs[at] or {}
      table.insert(objs[at], { tonumber(tokens[2]), tonumber(tokens[3], 16), tonumber(tokens[4]),
                               tonumber(tokens[5]), tonumber(tokens[6], 16), tonumber(tokens[7], 16) })
    elseif tokens[1] == "podsync" then
      -- only for sj-tools: the pod's fields are taken from this log after every step
    elseif tokens[1] == "keepfuel" then
      keepfuel = #steps + 1
    elseif tokens[1] == "spin" then
      spins[#steps + 1] = tonumber(tokens[2], 16)
    elseif tokens[1] == "item" then
      local at = #steps + 1
      items[at] = items[at] or {}
      table.insert(items[at], { tonumber(tokens[2]), tonumber(tokens[3], 16),
                                tonumber(tokens[4]), tonumber(tokens[5]) })
    elseif tokens[1] == "fuel" then
      pokes[#steps + 1] = tonumber(tokens[2], 16)
    elseif #tokens > 0 then
      local held = {}
      for i = 2, #tokens do
        local name = BUTTONS[tokens[i]]
        if name then held[name] = true end
      end
      for _ = 1, tonumber(tokens[1]) do steps[#steps + 1] = held end
    end
  end
  return steps, dc, pokes, demo, level, warps, items, spins, objs
end

local function main()
  local RAM = emu.memType.nesInternalRam
  local function r(a) return emu.read(a, RAM) end
  local function w16(hi, lo) return r(hi) * 256 + r(lo) end
  local steps, dc, pokes, demo, level, warps, items, spins, objs = parse_replay(assert(os.getenv("SJ_REPLAY_IN")))
  -- Levels above 12 are entered from a planet by a transition 150 gameplay frames in.
  local base = level
  local swap = nil
  local second_swap = false
  if level >= 13 and level <= 25 then base = level - 13; swap = "second"
  elseif level >= 29 and level <= 41 then base = level - 29; swap = "second"; second_swap = true
  elseif level == 26 then base = 0; swap = "final" end
  local swap_done = false
  -- Planets 9-11 pull hard enough that an idle pod crashes before the 150 frames are over (and
  -- the capture then never sees the second level): the pod is pinned until the transition.
  local pinned = nil
  local hover = swap and base >= 9 and base <= 11
  local entries, swapped, left_after_swap = 0, false, false
  local demos_seen, last_entry = 0, -100
  local out = assert(io.open(assert(os.getenv("SJ_REPLAY_OUT")), "w"))

  local frame = 0     -- frames since power-on
  local a_passes = 0  -- A passes started so far: the number of the step in progress
  local started = false
  local away = nil    -- frame at which gameplay was left, while it is left
  local a_open = false   -- an A pass has started and not reached the idle loop yet
  local cut = false
  local pending = nil    -- parity of $5D at the last NMI, until the pass reaches the idle loop
  local done = 0         -- the last step whose sample was written
  local first5 = {}
  local in_loop = false  -- between the objects' loop entry and the end of the pass
  local shots = os.getenv("SJ_REPLAY_SHOTS")
  local shot = nil       -- the step whose picture this frame shows

  -- The object slots 3-30 in use, as "oS<slot>=state oX oY oF (frame) oH (hit points) oI (list
  -- index)"; slots 1-2 (the pod's flame) are not compared.
  local function objects()
    local t = {}
    for s = 1, 30 do
      local st = r(0x317 + s)
      -- The flags Y and frame bytes of every slot, in use or not: the tow's lift sign reads
      -- bit 0 of one of them (player.md 8.3 b), whichever slot it is.
      if demo then
        t[#t + 1] = string.format("zy%d=%X zi%d=%X", s, (r(0x355 + s) << 8) | r(0x374 + s), s, r(0x393 + s))
      end
      if st ~= 0 and s <= 2 then
        -- The slots attached to the pod (the flame, the beam or shield ring, explosions):
        -- "aS<slot>" state, x, y, frame and the direction flags X/Y, compared apart.
        t[#t + 1] = string.format("as%d=%X ax%d=%X ay%d=%X af%d=%X ad%d=%X",
          s, st, s, w16(0x21F + s, 0x200 + s), s, w16(0x27C + s, 0x25D + s),
          s, r(0x374 + s), s, (r(0x336 + s) << 8) | r(0x355 + s))
      end
      if st ~= 0 and s >= 3 then
        t[#t + 1] = string.format("os%d=%X ox%d=%X oy%d=%X of%d=%X oh%d=%X oi%d=%X qx%d=%X qy%d=%X",
          s, st, s, w16(0x21F + s, 0x200 + s), s, w16(0x27C + s, 0x25D + s),
          s, r(0x374 + s), s, r(0x441 + s), s, r(0x393 + s), s, r(0x197 + s), s, r(0x23E + s))
        -- The slot's hidden bytes (speeds, directions, work bytes), which lockstep comparisons
        -- (sj-tools adopt_objects) take from the original; not compared themselves.
        t[#t + 1] = string.format("zv%d=%X zd%d=%X zk%d=%X zw%d=%X zz%d=%X",
          s, (r(0x2BA + s) << 24) | (r(0x29B + s) << 16) | (r(0x2F8 + s) << 8) | r(0x2D9 + s),
          s, (r(0x336 + s) << 8) | r(0x355 + s), s, r(0x3B2 + s), s, r(0x403 + s), s, r(0x422 + s))
      end
    end
    return table.concat(t, " ")
  end

  -- The bytes of the object slots 3-30 at the start, in use or not ("l" + a letter + slot):
  -- the previous occupants' leftovers, which a new object keeps where its creator does not clear
  -- them (behaviour.md 2.5, Q17). Demos only; sj-tools seeds the free slots with them.
  local function leftovers()
    local t = {}
    for s = 3, 30 do
      t[#t + 1] = string.format(
        "lx%d=%X ly%d=%X lv%d=%X ld%d=%X lk%d=%X lw%d=%X lz%d=%X li%d=%X lh%d=%X",
        s, r(0x197 + s), s, r(0x23E + s),
        s, (r(0x2BA + s) << 24) | (r(0x29B + s) << 16) | (r(0x2F8 + s) << 8) | r(0x2D9 + s),
        s, (r(0x336 + s) << 8) | r(0x355 + s), s, r(0x3B2 + s), s, r(0x403 + s), s, r(0x422 + s),
        s, r(0x393 + s), s, r(0x441 + s))
      -- Position and picture too (warp-level replays seed them).
      t[#t + 1] = string.format("lp%d=%X lf%d=%X", s, (w16(0x21F + s, 0x200 + s) << 16) | w16(0x27C + s, 0x25D + s),
        s, r(0x374 + s))
    end
    -- The parallax stars (graphics.md 5.7.3): the mode RAM $D5 and the dead mask $04D7, then x, y,
    -- x fraction, y fraction.
    t[#t + 1] = string.format("km=%X kd=%X", r(0xD5), r(0x4D7))
    for i = 0, 7 do
      t[#t + 1] = string.format("kx%d=%X ky%d=%X kf%d=%X kg%d=%X",
        i, r(0x4B7 + i), i, r(0x4BF + i), i, r(0x4C7 + i), i, r(0x4CF + i))
    end
    return table.concat(t, " ")
  end

  -- $68-$6D keep their values between tows (player.md 8.3, "What persists").
  local function rope(a) return r(a) end

  -- 1 when the 16 bytes RAM $A8-$B7 hold 0, 1, ..., 15 in order (the demo's shield timer checks).
  local function palette_table_ready()
    for i = 0, 15 do
      if r(0xA8 + i) ~= i then return 0 end
    end
    return 1
  end

  local function state()
    return string.format(
      "x=%04X xf=%02X y=%04X yf=%02X hw=%02X hf=%02X hl=%02X vw=%02X vf=%02X vd=%02X " ..
      "face=%02X st=%02X fuel=%04X cx=%04X cy=%04X ang=%02X hp=%02X iv=%02X sh=%02X " ..
      "ach=%02X acv=%02X dd=%02X f1=%02X f2=%02X rng=%02X%02X%02X ctr=%02X%02X%02X",
      w16(0x21F, 0x200), r(0x197), w16(0x27C, 0x25D), r(0x23E),
      r(0x2BA), r(0x29B), r(0x336) & 1, r(0x2F8), r(0x2D9), r(0x355) & 1,
      r(0x374), r(0x317), w16(0xC7, 0xC6), w16(0x4D, 0x4C), w16(0x4F, 0x4E),
      r(0x9C), r(0x99), r(0x97), r(0x65) & 0x80,
      r(0xC4), r(0xC5), r(0xDD), r(0xF1), r(0xF2),
      r(0x5A), r(0x5B), r(0x5C), r(0x5D), r(0x5E), r(0x5F)) ..
      string.format(" score=%X%X%X%X%X%X bb=%02X ba=%02X fs=%02X ", r(0xCB), r(0xCC), r(0xCD), r(0xCE), r(0xCF), r(0xD0), r(0xBB), r(0xBA), r(0x5CE)) ..
      (demo and string.format("wp=%02X tm=%02X ih=%02X ip=%02X cc=%02X pt=%X ", r(0x510), r(0x511), r(0x18), r(0x19), r(0x0D), palette_table_ready()) or "") ..
      -- The tractor beam (the rope's bytes always), the player's work bytes, lives and the economy.
      string.format("t66=%02X t67=%02X t68=%02X t69=%02X t6a=%02X t6b=%02X t6c=%02X t6d=%02X t6e=%02X " ..
        "ptm=%02X pcn=%02X fl=%02X lives=%02X pods=%02X e0=%02X c9=%02X bay=%02X grd=%02X fc=%02X d2=%02X c8=%02X ag=%02X sf=%02X ",
        r(0x66), r(0x67), rope(0x68), rope(0x69), rope(0x6A), rope(0x6B), rope(0x6C), rope(0x6D),
        r(0x6E),
        r(0x393), r(0x3B2), r(0x336), r(0x3E), r(0x3D), r(0xE0), r(0xC9), r(0x98) & 0x20, r(0x41),
        r(0x513), r(0xD2) & 0x7F, r(0xC8), r(0xD2) >> 7, r(0x636)) ..
      -- The warp levels' mesh helpers (objects.md 10.6): the position bytes of slots 23 and 24.
      (r(0x3B) >= 29 and string.format("wx=%02X wy=%02X vx=%02X vy=%02X ",
        r(0x200 + 23), r(0x25D + 23), r(0x200 + 24), r(0x25D + 24)) or "") ..
      objects()
  end

  -- The full redraw after a warp (levels.md, Appendix A, "Redrawing the whole background after a
  -- warp"): the game's own full-screen draw, called from the idle loop at the camera rounded down
  -- to 32 px. That fills 32 columns and 29 rows, so more calls supply the rows below (32 px
  -- down) and, with an unaligned camera, the columns on the right (32 px right, and both).
  -- Rendering and the NMI are off meanwhile (5 frames per call); no step is lost.
  -- Name table page 0 itself: PPU $2000 shows whichever page the mapper has selected, and the
  -- idle loop has the status panel's selected.
  local NT = emu.memType.nesNametableRam
  local redraw = nil
  local last_held = {}
  local function draw_call(cx, cy)
    emu.write(0x4C, cx & 0xFF, RAM); emu.write(0x4D, cx >> 8, RAM)
    emu.write(0x4E, cy & 0xFF, RAM); emu.write(0x4F, cy >> 8, RAM)
    emu.write(0x73, 0x80, RAM)
    local page = r(0x75)
    emu.write(0x75, page & 0xEF, RAM)
    emu.write(0x00, r(0x00) & 0x7F, RAM)
    -- The far call's frame: the idle loop's return address, the page byte, the helper's tail.
    local sp = redraw.cpu["cpu.sp"]
    for _, b in ipairs({ 0x95, 0x6F, page, 0xFF, 0xBF }) do
      emu.write(0x100 + sp, b, RAM)
      sp = sp - 1
    end
    local state = emu.getState()
    state["cpu.pc"], state["cpu.a"], state["cpu.y"], state["cpu.sp"] = 0xFFAC, 3, 0, sp
    emu.setState(state)
  end

  local function start_redraw(cx, cy)
    local zp = {}
    for a = 0, 0xFF do zp[a] = r(a) end
    local cpu = {}
    for k, v in pairs(emu.getState()) do
      if k:match("^cpu%.[apsxy]+$") then cpu[k] = v end
    end
    local extra, k = ((cx & 31) + 255 >> 3) - 31, (cy & 31) >> 3
    local x, y = cx & ~31, cy & ~31
    -- B: the columns on the right; D: their rows below; C: the rows below; A: the picture, last.
    local queue = {}
    if extra > 0 then queue[#queue + 1] = { "B", x + 32, y } end
    if extra > 0 then queue[#queue + 1] = { "D", x + 32, y + 32 } end
    queue[#queue + 1] = { "C", x, y + 32 }
    queue[#queue + 1] = { "A", x, y }
    redraw = { cx = cx, cy = cy, zp = zp, cpu = cpu, extra = extra, k = k, queue = queue, kept = {} }
    draw_call(queue[1][2], queue[1][3])
  end

  -- What a finished extra draw leaves for the patches after the main draw (byte row * 32 + slot).
  local function keep(name)
    local extra, k, kept = redraw.extra, redraw.k, { tiles = {}, attributes = {} }
    if name == "B" then
      for row = 0, 29 do
        kept.tiles[row] = {}
        for i = 0, extra - 1 do kept.tiles[row][i] = emu.read(row * 32 + 28 + i, NT) end
      end
      for row = 0, 7 do kept.attributes[row] = emu.read(0x3C7 + row * 8, NT) end
    elseif name == "D" then
      for i = 0, k do
        kept.tiles[i] = {}
        for j = 0, extra - 1 do kept.tiles[i][j] = emu.read((25 + i) * 32 + 28 + j, NT) end
      end
      kept.attributes[0] = emu.read(0x3C0 + 6 * 8 + 7, NT)
    else
      for i = 0, k do
        kept.tiles[i] = {}
        for slot = 0, 31 do kept.tiles[i][slot] = emu.read((25 + i) * 32 + slot, NT) end
      end
      for c = 0, 7 do kept.attributes[c] = emu.read(0x3C0 + 6 * 8 + c, NT) end
    end
    redraw.kept[name] = kept
  end

  -- The main draw's picture completed with the extra draws' columns and rows.
  local function patch()
    local extra, k, kept = redraw.extra, redraw.k, redraw.kept
    local below = {}
    for i = 0, k do
      local row = (29 + i) % 30
      below[row] = i
      for slot = 0, 31 do emu.write(row * 32 + slot, kept.C.tiles[i][slot], NT) end
    end
    if k >= 2 then
      -- Ring rows 0-1 now hold rows below the picture: their quadrants are C's lower ones.
      for c = 0, 7 do
        local a = 0x3C0 + c
        emu.write(a, (emu.read(a, NT) & 0xF0) | (kept.C.attributes[c] >> 4), NT)
      end
    end
    if extra > 0 then
      for row = 0, 29 do
        local tiles = below[row] and kept.D.tiles[below[row]] or kept.B.tiles[row]
        for i = 0, extra - 1 do emu.write(row * 32 + i, tiles[i], NT) end
      end
      -- The quadrants of the patched columns: slots 0-1 are bits 0-1 and 4-5, slots 2-3 the rest.
      local m = extra > 2 and 0xFF or 0x33
      for row = 0, 7 do
        local source = kept.B.attributes[row]
        if row == 0 and k >= 2 then source = (source & 0xF0) | (kept.D.attributes[0] >> 4) end
        local a = 0x3C0 + row * 8
        emu.write(a, (emu.read(a, NT) & ~m & 0xFF) | (source & m), NT)
      end
    end
  end

  -- Called at the idle loop: true while a redraw is running (a far call has returned when the
  -- stack is back where it was).
  local function redrawing()
    if not redraw then return false end
    if emu.getState()["cpu.sp"] ~= redraw.cpu["cpu.sp"] then return true end
    if not redraw.patched then
      local done = table.remove(redraw.queue, 1)
      if done[1] ~= "A" then
        keep(done[1])
        draw_call(redraw.queue[1][2], redraw.queue[1][3])
        return true
      end
      patch()
      redraw.patched = true
    end
    -- Turning the NMI on inside a vertical blank would start a pass at once, early: the idle
    -- loop waits for the picture's lines.
    local line = emu.getState()["ppu.scanline"]
    if line < 0 or line >= 240 then return true end
    for a = 0, 0xFF do emu.write(a, redraw.zp[a], RAM) end
    emu.write(0x2001, r(0x01), emu.memType.nesMemory)
    emu.write(0x2000, r(0x00), emu.memType.nesMemory)
    local cx, cy = redraw.cx, redraw.cy
    emu.write(0x4C, cx & 0xFF, RAM); emu.write(0x4D, cx >> 8, RAM)
    emu.write(0x4E, cy & 0xFF, RAM); emu.write(0x4F, cy >> 8, RAM)
    emu.write(0x02, cx & 31, RAM); emu.write(0x03, cy & 31, RAM)
    -- The uploads the last pass left pending are for the old camera.
    for _, a in ipairs({ 0xEE, 0x49, 0x50, 0x51, 0x52, 0x53, 0x54 }) do emu.write(a, 0, RAM) end
    local state = emu.getState()
    for k, v in pairs(redraw.cpu) do state[k] = v end
    emu.setState(state)
    redraw = nil
    return true
  end

  local function poke(step)
    local warp = warps[step]
    if warp then
      -- Test setup: the pod stands still at (x, y) and the camera at (cx, cy).
      local x, y, cx, cy = warp[1], warp[2], warp[3], warp[4]
      emu.write(0x200, x & 0xFF, RAM); emu.write(0x21F, x >> 8, RAM)
      emu.write(0x25D, y & 0xFF, RAM); emu.write(0x27C, y >> 8, RAM)
      for _, a in ipairs({ 0x197, 0x23E, 0x29B, 0x2BA, 0x2D9, 0x2F8 }) do emu.write(a, 0, RAM) end
      emu.write(0x4C, cx & 0xFF, RAM); emu.write(0x4D, cx >> 8, RAM)
      emu.write(0x4E, cy & 0xFF, RAM); emu.write(0x4F, cy >> 8, RAM)
    end
    if step == 1 and swap == "final" then
      emu.write(0x9A, 0x80, RAM); emu.write(0x317, 6, RAM)
      emu.write(0x4C, 0, RAM); emu.write(0x4D, 0, RAM)
      emu.write(0x4E, 928 & 0xFF, RAM); emu.write(0x4F, 928 >> 8, RAM)
    end
    for _, it in ipairs(items[step] or {}) do
      local n, kind, x, y = it[1], it[2], it[3], it[4]
      emu.write(0x513 + n, kind, RAM)
      emu.write(0x52D + n, x & 0xFF, RAM); emu.write(0x561 + n, x >> 8, RAM)
      emu.write(0x547 + n, y & 0xFF, RAM); emu.write(0x57B + n, y >> 8, RAM)
    end
    for _, o in ipairs(objs[step] or {}) do
      local sl, st, x, y, fr, idx = o[1], o[2], o[3], o[4], o[5], o[6]
      emu.write(0x317 + sl, st, RAM)
      emu.write(0x200 + sl, x & 0xFF, RAM); emu.write(0x21F + sl, x >> 8, RAM)
      emu.write(0x25D + sl, y & 0xFF, RAM); emu.write(0x27C + sl, y >> 8, RAM)
      emu.write(0x374 + sl, fr, RAM); emu.write(0x393 + sl, idx, RAM)
      for _, a in ipairs({ 0x197, 0x23E, 0x29B, 0x2BA, 0x2D9, 0x2F8, 0x355, 0x336, 0x3B2, 0x403, 0x422 }) do
        emu.write(a + sl, 0, RAM)
      end
    end
    if spins[step] then emu.write(0x6E, spins[step], RAM) end
    if keepfuel and step >= keepfuel then
      emu.write(0xC6, 0, RAM); emu.write(0xC7, 0xE0, RAM)
    end
    if pokes[step] then
      emu.write(0xC6, pokes[step] & 0xFF, RAM)
      emu.write(0xC7, pokes[step] >> 8, RAM)
    end
    -- Last, from the idle loop only (a warp before step 1 comes before the level's own draw).
    if warp and step > 1 then start_redraw(warp[3], warp[4]) end
  end

  local function gameplay()
    return r(0x50A) + 256 * r(0x50B) == 0xB0B6
  end

  emu.addEventCallback(guard(function()
    local start = not demo and ((frame >= 1300 and frame < 1305) or (frame >= 1500 and frame < 1505))
    local held = {}
    -- The poll comes before the frame's NMI handler runs: the upcoming pass is an A pass when
    -- $5D is even, and belongs to step a_passes + 1; a B pass belongs to step a_passes.
    if gameplay() then
      if hover and not started and not swapped and not swap_done then
        -- Gravity here would drop the pod back into the bay (and the capture would never see
        -- the second level): its position is pinned and its speeds and fuel reset each frame.
        pinned = pinned or {}
        for _, a in ipairs({ 0x200, 0x21F, 0x25D, 0x27C, 0x197, 0x23E }) do
          pinned[a] = pinned[a] or r(a)
          emu.write(a, pinned[a], RAM)
        end
        for _, a in ipairs({ 0x2BA, 0x29B, 0x2F8, 0x2D9 }) do emu.write(a, 0, RAM) end
        emu.write(0xC6, 0, RAM); emu.write(0xC7, 0xE0, RAM)
      end
      local upcoming = a_passes + (r(0x5D) & 1 == 0 and 1 or 0)
      held = (swap and not started) and {} or steps[upcoming] or {}
    elseif start then
      held = { start = true }
    end
    -- The full draw of a warp's redraw reads the controller too: it gets the buttons of the
    -- last poll before, so the game's memory of the buttons held stays as it was.
    if redraw then held = last_held else last_held = held end
    emu.setInput(held, 0)
  end), emu.eventType.inputPolled)

  -- The gameplay NMI handler is entered: $5D still holds the value the pass starts with.
  emu.addMemoryCallback(guard(function(address)
    local where = emu.convertAddress(address, emu.memType.nesMemory)
    if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= 0x30B6 then
      return
    end
    if demo then
      if r(0x50F) == 0x80 and frame - last_entry > 5 then demos_seen = demos_seen + 1 end
      last_entry = frame
      if demos_seen ~= demo then return end
    end
    if swap and not started then
      -- Entering a level above 12: the first level's gameplay runs 150 frames, then the
      -- transition is requested; the log starts at the next gameplay.
      entries = entries + 1
      if not swapped then
        if entries >= (second_swap and swap_done and 20 or 150) then
          swapped = true
          if swap == "second" and second_swap and swap_done then
            -- The warp level: the planet's flag, the guard and transition 3 again.
            if base < 8 then
              emu.write(0xDE, r(0xDE) | (1 << (7 - base)), RAM)
            else
              local bit = base == 12 and 10 or base
              emu.write(0xDF, r(0xDF) | (1 << (7 - (bit - 8))), RAM)
            end
            emu.write(0x41, 0, RAM)
            emu.write(0x3F, 3, RAM)
          elseif swap == "second" then
            emu.write(0x3F, 3, RAM)
          else
            emu.write(0xDB, 26, RAM); emu.write(0x9A, 0x80, RAM); emu.write(0x3F, 5, RAM)
          end
        end
        return
      end
      if not left_after_swap then return end
      if second_swap and not swap_done then
        -- The second level is up: count its frames before the second request.
        swap_done = true
        swapped, left_after_swap, entries = false, false, 0
        return
      end
    end
    if not started then
      started = true
      if dc then emu.write(0xDC, dc, RAM) end
      if weapon then emu.write(0xD1, weapon, RAM) end
      if device then emu.write(0xD3, device, RAM) end
      if bullet then emu.write(0x50E, bullet, RAM) end
      if lives then emu.write(0x3E, lives, RAM) end
      for _, p in ipairs(flags) do
        -- The planet's warpship flag: RAM $DE bit 7 - P (planets 0-7), $DF bit 7 - (P - 8).
        if p < 8 then emu.write(0xDE, r(0xDE) | (1 << (7 - p)), RAM)
        else emu.write(0xDF, r(0xDF) | (1 << (7 - (p - 8))), RAM) end
      end
      if gauge then emu.write(0xC8, gauge & 0xFF, RAM); emu.write(0xC9, gauge >> 8, RAM) end
      poke(1)
      out:write("init " .. state() .. string.format(" ic=%02X", r(0xBB)) .. ((demo or level >= 13) and (" " .. leftovers()) or "") .. "\n")
    elseif away then
      -- Back from a scene change: the level was reloaded (a respawn).
      away = nil
      a_passes = math.max(a_passes, done)
      out:write(string.format("reload %d %s ic=%02X%s\n", a_passes, state(), r(0xBB),
        r(0x3B) >= 29 and (" " .. leftovers()) or ""))
    end
    if a_open then cut = true end
    pending = r(0x5D) & 1
    if shots and pending == 1 and done == a_passes - 1 and done > 0 then shot = done end
    if pending == 0 then
      a_open = true
      a_passes = a_passes + 1
    end
  end), emu.callbackType.exec, 0xB0B6)

  emu.addMemoryCallback(guard(function(address)
    -- The idle loop spins here: the cheap test first.
    if pending == nil and not redraw then return end
    local where = emu.convertAddress(address, emu.memType.nesMemory)
    if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= 0x1570 then
      return
    end
    if redrawing() or pending == nil then return end
    local parity = pending
    pending = nil
    in_loop = false
    if parity == 0 then
      a_open = false
    else
      done = a_passes
      out:write(string.format("s %d %d %s", a_passes, cut and 1 or 0, state()))
      if demo then
        -- RAM $05 as the objects found it (the distance routine's scratch byte, Q24).
        for slot = 15, 30 do
          if first5[slot] then out:write(string.format(" sr%d=%02X", slot, first5[slot])) end
        end
      end
      out:write("\n")
      cut = false
      if not demo and a_passes >= #steps then
        out:write("end\n")
        out:close()
        finished = true
        emu.stop(0)
      else
        poke(a_passes + 1)
      end
    end
  end), emu.callbackType.exec, 0x9570)

  -- The objects' loop (bank 7 $800F) is entered: RAM $05 now is what the drones and hoppers read
  -- when the distance routine stops short (behaviour.md 2.4). Logged in demos; sj-tools feeds
  -- it to the validation, as it does the shield flag.
  emu.addMemoryCallback(guard(function(address)
    local where = emu.convertAddress(address, emu.memType.nesMemory)
    if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= 0x3800F then
      return
    end
    first5 = {}
    in_loop = started and demo ~= nil
  end), emu.callbackType.exec, 0x800F)

  -- The first access to RAM $05 with X (the objects' loop index) in 15..30 after the loop is
  -- entered: when it is a read, its value is what the object in that slot found there ("sr<slot>").
  local function first_access(is_read)
    return guard(function(address, value)
      if not in_loop then return end
      local x = emu.getState()["cpu.x"]
      if x >= 15 and x <= 30 and first5[x] == nil then
        first5[x] = is_read and value or false
      end
    end)
  end
  emu.addMemoryCallback(first_access(true), emu.callbackType.read, 0x05)
  emu.addMemoryCallback(first_access(false), emu.callbackType.write, 0x05)

  -- The sound requests (sound.md 7.1, 8.1): every effect the driver starts is a write of its id to
-- the channel's effect byte (RAM $0648, $064C, $0650, $0654), and every song request is a call
-- of the two song entries (bank 0 $D480 "if different", $D488 "always"; the song is in A).
-- They are logged as "e PASS.CHANNEL.ID" and "m PASS.KIND.SONG" lines ahead of the step line
-- they belong to (PASS 0 = the A pass, 1 = the B pass; KIND S or A).
local function log_effect(address, value)
  if not started or value == 0 then return end
  -- PASS "x": started outside a pass (a scene is loading or showing); not compared.
  out:write(string.format("e %s.%d.%02X\n", pending == nil and "x" or tostring(pending), (address - 0x648) // 4, value))
end
for _, address in ipairs({ 0x648, 0x64C, 0x650, 0x654 }) do
  emu.addMemoryCallback(guard(function(a, value) log_effect(a, value) end),
    emu.callbackType.write, address)
end
local function log_song(kind, offset)
  emu.addMemoryCallback(guard(function(address)
    local where = emu.convertAddress(address, emu.memType.nesMemory)
    if not where or where.memType ~= emu.memType.nesPrgRom or where.address ~= offset then
      return
    end
    if not started then return end
    out:write(string.format("m %s.%s.%d\n", pending == nil and "x" or tostring(pending), kind, emu.getState()["cpu.a"]))
  end), emu.callbackType.exec, offset + 0x8000)
end
log_song("S", 0x5480)
log_song("A", 0x5488)

-- Leaving gameplay (docking in the mothership, a transition) is logged; if gameplay does not
  -- come back within 900 frames the log ends.
  local function dump(path, memtype, size)
    local f = assert(io.open(path, "a"))
    local row = {}
    for i = 0, size - 1 do row[#row + 1] = string.format("%02X", emu.read(i, memtype)) end
    f:write(table.concat(row, " "), "\n")
    f:close()
  end

  emu.addEventCallback(guard(function()
    frame = frame + 1
    if shot then
      local png = assert(io.open(string.format("%s/step-%d.png", shots, shot), "wb"))
      png:write(emu.takeScreenshot())
      png:close()
      local path = string.format("%s/step-%d.txt", shots, shot)
      os.remove(path)
      dump(path, emu.memType.nesPaletteRam, 32)
      dump(path, emu.memType.nesSpriteRam, 256)
      shot = nil
    end
    if started and os.getenv("SJ_PROBE") then
      -- A debugging aid: the mesh helpers of a warp level every frame (slots 23 and 24: x, y,
      -- fractions, speeds, direction flags, then the pod's low bytes and the counter).
      local function slot(s)
        return string.format("%02X%02X.%02X,%02X%02X.%02X,%02X%02X,%02X%02X,%02X,%02X",
          r(0x21F + s), r(0x200 + s), r(0x197 + s), r(0x27C + s), r(0x25D + s), r(0x23E + s),
          r(0x2BA + s), r(0x29B + s), r(0x2F8 + s), r(0x2D9 + s), r(0x336 + s), r(0x355 + s))
      end
      out:write(string.format("h %d %s %s pod=%02X,%02X cam=%02X,%02X c=%02X\n", frame, slot(23), slot(24),
        r(0x200), r(0x25D), r(0x4C), r(0x4E), r(0x5D)))
    end
    -- The planet to land on (levels.md, Appendix A): written after the title's Start.
    if frame == 1301 and not demo then emu.write(0x3B, base, RAM) end
    if swapped and not started and not gameplay() then left_after_swap = true end
    if started and not gameplay() then
      if not away then
        away = frame
        a_open = false
        pending = nil
        out:write(string.format("away %d\n", a_passes))
      elseif frame - away > (demo and 60 or 900) then
        out:write(string.format("left gameplay %d\nend\n", a_passes))
        out:close()
        finished = true
        emu.stop(0)
      end
    end
  end), emu.eventType.endFrame)
end

local ok, err = pcall(main)
if not ok then
  print("replay-capture: " .. tostring(err))
  emu.stop(1)
end
