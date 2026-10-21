# Scenes: how the non-gameplay screens are composed

What each screen outside gameplay shows and how it is put together: the nametable contents and who writes them,
the sprites, scroll and splits, the animation and its timing, and how the scene ends. Graphics data (tiles,
palettes, ROM streams) is in `graphics.md`, strings and the text printer in `text.md`, the scene order and the
rules for leaving a scene in `game-flow.md` 7. This file links to those rather than repeating them.

Status: **[V]** seen in Mesen (nametable, sprite memory, scroll registers or screenshots recorded frame by frame),
**[C]** read from the program, **[H]** hypothesis.

## 0. Common rules

- **Screen model.** One nametable page is shown at a time (`graphics.md` 1); every scene in this file uses page 0,
  except the status panel of the approach screen (page 1). Rows and columns below are nametable rows (0-29) and
  columns (0-31) of page 0; the screen position follows from the scroll in `graphics.md` 1.1.
- **Text** is written with tile number = character code (`graphics.md` 2.4, `text.md` 1); "row r, column c" is
  where its first character goes.
- **Sprites** are given as screen position (x, y of the top-left pixel; y is the first screen line the sprite
  covers, one more than the value in sprite memory), tile number in the scene's sprite pattern table, palette (0-3), flips and priority ("behind" = behind
  non-transparent background pixels). Sprite order matters only for overlap.
- **Frames.** "Frame" = one video frame (60 per second). Scene timers count frames from the scene's first frame
  unless stated; the scenes in this file run once per frame (`game-flow.md` 1.3).
- **Split triggers.** A sprite placed only to trigger a mid-screen change (sprite 0 with a one-dot tile) is a
  hardware aid: the remake does not draw it (`graphics.md` section 3, split marker).

### 0.1 Entering and leaving a scene [V]

Every change of scene goes through **loading frames**: rendering is switched off (the screen is blank, the default
handler runs, `game-flow.md` 1.1) while the next scene's video data is written, then the scene's own handler starts.
Measured per frame in Mesen (rendering switched on or off, palette RAM, scene handler number in RAM $050A) over a
power-on run and forced runs of every transition below.

- **No fades.** The palette is written during the blank and the first visible frame already has its final colours.
  The only fades are inside scenes: the title logo (2.2) and the end of the flight (5.5). There is no fade-out
  before a blank either: the last frame of a scene is shown as it was, then the screen goes blank.
- **Blank frames** (count of frames with rendering off between the last frame of the old scene and the first
  visible frame of the new one):

  | From | To | Blank frames |
  |---|---|---|
  | power-on | licence screen | 6 (frames 1-6) |
  | licence screen | title | 14 |
  | title (timeout) | badge (2.4) | 3 |
  | badge | story (3) | 15 |
  | story | title | 14 |
  | badge | attract demo (3.1) | 28 |
  | demo (pod destroyed) | title | 17 |
  | title, START GAME | approach screen (7) | 6 |
  | title, CONTINUE | password entry (4) | 3 |
  | password entry, Start with the unchanged (all-A) password | approach screen | 7 |
  | approach screen, button | surface (8) | 23 (planet 0), 26 (planet 1) |
  | surface, descent over or skipped | launch scene (9) | 14 |
  | launch scene | gameplay (a level start) | 20 or 22 |
  | gameplay, change to another planet level (start level) | flight (5) | 17 |
  | flight | shop (6) | 10 (after the 4-step fade-out, which is part of the flight) |
  | shop, B | approach screen | 7 |
  | gameplay, dock (code 2) | surface menu (mode 3) | 14 or 15 |
  | surface menu, EXPLORE | launch scene | 14 |
| surface menu, EQUIP POD | equip list (6) | 11 |
| equip list, B | launch scene | 14 |
| gameplay, kind-6 message (warp-level gate) | warp-level screen (10.1) | 10 (9 in the 2026-10-17 run) |
| warp-level screen | gameplay (the warp level) | 13 (12 in the 2026-10-17 run) |
| gameplay, gate end on a planet (code 3, `player.md` 9.2.3) | gameplay, the second level | 27 |
| gameplay, gate end on a second level, flag clear (code 3) | gameplay, the planet | 28 |
| gameplay, warp-level countdown over (code 6) | badge intermission (11.2) | 3 |
| badge intermission after a warp level | gameplay, the planet | 28 |
| message screen, all twelve flags set (`game-flow.md` 7.4) | gameplay, the final level (no launch scene) | 29 |
| gameplay, Start | pause | **0**: no blank frame (see "Pause" below) |
| pause, Start | gameplay | **0** |
  | gameplay, delivery (code 1) | message screen (10) | 4 |
  | message screen | gameplay (reload) | 18 |
  | gameplay, badge intermission (code 6) | badge intermission (11.2) | 4 |
  | badge intermission | gameplay (reload) | 28 |
  | gameplay, backdrop interlude (code 7) | backdrop interlude (11.1) | 6 |
  | backdrop interlude | badge intermission | 3 |
  | badge intermission after the interlude | gameplay | 19 |
  | gameplay, Jetman lost for good | game over (12) | **0**: the animation starts on the playfield at once |
  | game over (240 frames) | last password (12) | 2 |
  | last password, button | title (warm reset) | 14 (a button on the first visible frame is accepted; the blank starts 3 frames later) |
  | pause, Select (planet map) | planet map (13) | 2 |
  | planet map, Start | gameplay (reload) | 18 |
  | gameplay, final level complete (14.1, forced) | final message (14.1) | 3 |
  | final message | ending (14.2) | 1 |
  | ending | credits (14.3) | 2 |
  | credits (2,274 visible frames) | title (warm reset) | 14 |

  The counts vary by a few frames with the amount of data the next scene needs (approach to surface 23 or 26,
  launch to gameplay 20 or 22: the same transition measured on two planets); the table gives what was seen. The
  remake has no loading time: it may start the new scene at once, and must then keep the **scene durations**
  stated in this file, which are counted from the first visible frame unless they say otherwise. The map (13), the ending and credits (14) were measured on
  2026-10-17 with the same method (rows below; the final message and the ending by forcing, 14). The equip list, the pause and the kind-6 screen were measured on
  2026-10-17 with the same method (rows above; counts are frames with rendering off, first visible frame
  already in its final colours, no music started or stopped by these three transitions) **[V]**.
- **Pause** (Start in gameplay, Start again) **[V]**, recorded as above: **no blank frame, no fade, no palette
  change, no picture change.** In the frame Start is seen the scene handler becomes the pause handler; that frame's
  sound step still runs and then the tone channel-enable register is written 0 (square 1, square 2, triangle and noise
  off); from then on no sound register is written, the picture, sprites, palette and the status-panel split stay as
  they were (`game-flow.md` 4). Leaving: in the frame Start is seen the gameplay handler returns and the channel-enable
  register is written $0F; the sound driver's song state is untouched (the current-song byte is the same), and the next
  frame it writes the channel registers again, so the music continues from where it stopped. The remake: freeze
  everything, silence all channels on entry, continue the same song on leaving, with no visual change either way. The
  first note after a resume may restart (switching the channels off clears their length counters), a hardware detail
  that need not be copied **[H]**.
- **Music at a level change (order of events) [V]** (2026-10-17). Song 2 starts on the frame the gate state begins (the level
  song stops); the gate lasts 385 frames, song 2 would last 389; the next level's loading starts at the gate's end (4 frames
  before song 2 would end) and the new level's own song (planet: `sound.md` 8.1; warp level: 5; final level: 33) starts on the
  last frame of the loading, one frame before the first gameplay frame, replacing song 2. The kind-6 screen
  has no music of its own (song 2, which started with the gate, has run out during it; the current-song byte stays 2); the warp
  level's song 5 starts on the last loading frame as above, and the badge screen after the warp level starts song 0
  (silence), after which the planet's song starts as above (checked on planet 0, level 13 and level 29). The remake: song 2 for the gate's 385 frames (cut at its end), then the next level's song at the first frame of the
  new level.
- **State carried in.** A scene starts with its own set-up and frame counter (`game-flow.md` 1.5); sprites of the
  previous scene do not survive (the sprite table is rewritten); the scroll and pattern-table settings are those
  given in each section.

## 1. Licence screen (power-on) [V]

- **Background:** cleared nametable; texts drawn by the printer (`text.md` 3): "TRADEWEST, INC. PRESENTS" row 3,
  "SOLAR JETMAN™" row 8, "HUNT FOR THE GOLDEN WARPSHIP" row 10, "© 1990 RARE LTD." row 13, "LICENSED TO
  TRADEWEST, INC." row 18, "BY RARE COIN-IT, INC." row 20, "LICENSED BY" row 23, "NINTENDO OF AMERICA INC." row
  25.
- **Scroll:** x = 0, y = 2. The scroll is set once, in the last loading frame (power-on frame 7: horizontal 0,
  vertical 0, then the video address set to the nametable base $2000, which leaves a fine vertical scroll of 2,
  `graphics.md` 1.1), and the licence handler never touches it again (it writes only the control register, every
  frame), so the offset holds for all 512 frames. Row r therefore sits at screen line 8r - 2, the top two lines of
  row 0 are cut off, and the last two screen lines (238-239) show lines 0-1 of row 0 again (blank here). **[V]** (video
  address registers read back at power-on frames 100 and 300: vertical fine scroll 2). The earlier "no scroll
  writes" was true of the steady frames only; the implementer's reading of 2 is right.
- **Left 8 pixels masked** by the display's left-column setting, a hardware option the game switches on in every
  picture scene (`graphics.md` 1.3). **[V]**
- **Sprites:** none visible.
- **Ends:** after 512 frames (`game-flow.md` 7, cold start only).

## 2. Title sequence

The title is one nametable: the logo stream (`graphics.md` 4.2, ROM 0x12B99) fills rows 0-13; the program later
adds the menu box and texts. The sequence's sub-states and their lengths are in `game-flow.md` 7.1.

### 2.1 Stars [V]

56 star sprites in sprite slots 8-63 (star `n` = slot 8 + `n`, `n` = 0-55), set up once when the title starts
(sub-state 0).

- **Start values.** `y` is the sprite-memory value (screen line = `y` + 1; `y` of $EF and above is off-screen), `x` the
  left edge, tile = $80 + `t` (star shapes, `graphics.md` 4.1, ROM 0x0FF2F, tiles $80-$83 of table 0), priority in
  front. Palette = `n` mod 4, with 3 replaced by 0 (so 0, 1, 2, 0, 0, 1, 2, 0, ...). **[V]** against sprite memory at
  the first title frames.

  | `n`: y x t | `n`: y x t | `n`: y x t | `n`: y x t |
  |---|---|---|---|
  | 0: 16 189 0 | 1: 133 17 1 | 2: 247 132 0 | 3: 16 24 1 |
  | 4: 1 136 0 | 5: 189 227 0 | 6: 86 169 3 | 7: 85 169 0 |
  | 8: 83 76 1 | 9: 230 12 3 | 10: 133 13 1 | 11: 133 201 1 |
  | 12: 125 164 3 | 13: 13 192 1 | 14: 41 169 3 | 15: 83 185 1 |
  | 16: 141 2 3 | 17: 3 1 1 | 18: 201 20 3 | 19: 201 23 2 |
  | 20: 202 134 0 | 21: 237 247 0 | 22: 1 133 1 | 23: 34 133 1 |
  | 24: 119 133 1 | 25: 169 35 1 | 26: 169 36 1 | 27: 160 26 1 |
  | 28: 185 50 0 | 29: 1 1 1 | 30: 247 136 0 | 31: 160 7 2 |
  | 32: 247 153 2 | 33: 136 16 1 | 34: 0 32 2 | 35: 169 64 3 |
  | 36: 2 169 1 | 37: 31 2 3 | 38: 2 141 1 | 39: 169 128 3 |
  | 40: 2 169 2 | 41: 23 3 3 | 42: 141 116 2 | 43: 119 133 0 |
  | 44: 240 3 1 | 45: 132 165 2 | 46: 165 13 2 | 47: 14 72 1 |
  | 48: 169 1 2 | 49: 32 172 1 | 50: 1 160 2 | 51: 248 32 0 |
  | 52: 104 133 1 | 53: 133 13 1 | 54: 12 162 0 | 55: 92 176 0 |

  **Where the numbers come from.** The original reads them from a run of bytes that is part of its own program code
  (`y` at ROM 0x002D1 + 4`n`, `x` at 0x002D2 + 4`n`, `t` = the low two bits of the byte at 0x000D3 + 4`n`), so they are
  not a data table and look arbitrary (several stars sit off-screen, in a column at x = 133, or in pairs). The remake
  uses the table above and does not read those addresses.
- **Flash.** From the start of sub-state 0 to the end of sub-state 5 (title sub-states, `game-flow.md` 7.1), after the
  frame counter `$5D` has been advanced for the frame: when `$5D` mod 16 = 0, one star is switched to palette 3. Its
  number is chosen from the random byte `$5A` (as left by that frame's generator step, `game-flow.md` 5): `v` = `$5A`,
  raised by 32 if below 32, rounded down to a multiple of 4, then `n` = `v` / 4 - 8 (so every `$5A` from 0 to 63 chooses
  one of stars 0-7, which are therefore twice as likely as the others). When `$5D` mod 16 = 6 the same star goes back to
  its own palette. So a star is in palette 3 for 6 of every 16 frames. **[C]** rule, **[V]** palettes seen changing.
  **Original bug, not reproduced:** the "go back" step also runs at the first tick of `$5D` mod 16 = 6, before any
  star was chosen, with a stale pointer to sprite slot 33; it rewrites the `y` byte of star 25 as its top three bits
  only (169 becomes 160). The remake keeps 169. **[V]**
- **Sun flicker.** Over the same frames the shadow palette entries 13 and 15 (background palette 3, colours 1 and 3,
  the sun and the glow of the logo; `graphics.md` 7.1) are replaced each frame by one of two pairs, switching every
  8 frames: for `$5D` mod 16 = 0-7 the first pair, for 8-15 the second. First pair: entry 13 = the title preset's own
  entry 13 (preset 5, ROM 0x07594, a pale yellow), entry 15 = the preset's own entry 15 (ROM 0x07596) lowered by
  $10, a dark brown. Second pair: entry 13 = white, entry 15 = the preset's own entry 15, a mid brown. The white is a colour number held as an operand in the program (ROM 0x0019F). **[V]** palette RAM sampled every 4 frames; **[C]** the pairs.
- **Slide** (sub-state 2). On every second frame (`$5D` odd) the `y` byte of all 56 stars is decreased by 1 (8-bit,
  wrapping 0 to 255), 24 times in all while the scroll goes from 176 to 224. At the end of the slide every star
  whose `y` byte is $90 or more is hidden (`y` = $F8 for good). Stars that start between 24 and 167 survive, now 24
  higher. **[V]** (sprite memory before and after: e.g. star 25 160 to 136; stars 0, 5 and 9 gone).
- **Behind the logo.** When sub-state 1 ends all 56 stars get the "behind background" priority, and keep it. **[C]**
- **Shared slots.** The walking Jetman's sprites (2.3) are written to slots 62 and 63 when sub-state 5 starts,
  replacing stars 54 and 55. The remake keeps its stars in their own sprites and so must remove stars 54 and 55 at
  that point. **[V]** (slots 62 and 63 change at about frame 1288 of a power-on run).

### 2.2 Sun and logo (sub-states 0-2) [V]

- **Background:** the title nametable, logo rows 0-13. The "O" of SOLAR is the sun: the camera starts close to
  it and pulls out.
- **Scroll:** the scroll is the position of a helper object (object slot 1) that homes on a fixed target (object
  slot 2); the swing past the target is that object's own motion. **[V]**: the model below reproduces the object's
  position and speeds, and so the scroll, for 400 frames with no difference.
  - **Set-up** (two frames before the first visible title frame, power-on frame 531): object 1 at x = 88, y = 236
    (sub-pixel fractions 0), horizontal speed 0, vertical speed 2.00 px per step (8.8 value $0200), direction bits
    "right" and "up". Target T = (4, 176). Speeds are magnitudes (`behaviour.md` 2.1).
  - **Each frame**, from the second frame after the set-up, for x and then for y (x is moved before y is looked
    at; positions are 16-bit signed, so x goes below 0 and the scroll shows it modulo 256):
    1. "toward" = the direction bit points from the object to T on this axis (T at the same coordinate counts as
       "right" and "down");
    2. toward: speed += 12/256, capped at $02FC; the object then moves by the new speed + 4/256;
    3. not toward: if the speed is 0, the direction bit flips and nothing else happens; if the speed is below
       20/256, it becomes 0, the direction bit flips and the object does **not** move; otherwise speed -= 20/256
       and the object moves by the new speed + 4/256 (also when that speed is exactly 0).
    The "+ 4/256" makes the capped step exactly 3.00 px. This is the homing of `behaviour.md` 2.3 with accel 12
    and a stronger brake; it is not the flight camera's accel (5.2).
  - **Leftover (`player.md` 7.4).** The helper's direction bytes (RAM $0337 = flags X, $0356 = flags Y) stay as they are when sub-state 0 ends and are not cleared by any later scene: at power-on frame 1041 they are X = 1 (left) and Y = 1 (down) **[V]**, and they are what slot 1 holds when play starts.
  - **Scroll output:** the scroll of frame f is the whole part of the object's position after frame f - 1 (x and y
    low bytes). Frame 533 is the first visible frame (scroll (88, 236)). With k = frame - 533:

    | k | 0 | 10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 100 | 120 | 150 | 200 | 250 | 300 | 350 |
    |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
    | scroll x | 88 | 85 | 78 | 67 | 50 | 29 | 4 | 236 | 220 | 212 | 226 | 20 | 9 | 5 | 255 | 5 |
    | scroll y | 236 | 213 | 185 | 158 | 137 | 125 | 120 | 122 | 129 | 156 | 196 | 209 | 156 | 189 | 176 | 177 |

    The lowest y is 120 (k = 58), the leftmost object x is -45 (k = 94, scroll x 211); from about k = 346 the
    scroll stays within one pixel of (4, 176), alternating 176/177 and 4/5.
- **Logo fade-in** (sub-state 1): palette steps from `graphics.md` 7.4, one every 8 frames.
- **Slide** (sub-state 2): the vertical scroll goes from 176 to 224, 1 pixel per frame (48 frames). [V]

### 2.3 Menu (sub-states 3-5) [V]

- **Scroll:** x = 4, y = 224 (`graphics.md` 1.1).
- **Menu box [V]** (recorded frame by frame: every nametable cell written, with its frame). "START GAME" (row 18,
  column 12) is written in the first frame of sub-state 3, "CONTINUE" (row 20, column 13) in the second, the third
  frame writes nothing. The box then **builds up over 26 frames** (sub-state 4, frames k = 1-26, one write per
  frame, each visible at once), and the copyright line (row 25, column 4, 26 cells, `text.md` 4.1) is written in
  one frame, k = 27. Tile codes (pattern table 1, `graphics.md` 1.1; the box tiles are part of the title's tile set):
  $10 top-left corner, $11 top-right, $12 bottom-left, $13 bottom-right, $14 horizontal bar, $15 vertical bar
  (frame art of the "planet menu box" family, but not the codes of `text.md` 4.6). Box: top edge row 16, bottom edge
  row 22, left side column 10, right side column 23, interior rows 17-21 columns 11-22 left blank (tile $00, the
  texts above sit in it). Draw order:

  | k | Cells written |
  |---|---|
  | 1 | top-left corner (16, 10) |
  | 2 | bottom-right corner (22, 23) |
  | 3, 5, 7, 9, 11, 13 | top edge, two $14 tiles each, left to right: columns 11-12, 13-14, 15-16, 17-18, 19-20, 21-22 of row 16 |
  | 4, 6, 8, 10, 12, 14 | bottom edge, two $14 tiles each, **right to left**: columns 21-22, 19-20, 17-18, 15-16, 13-14, 11-12 of row 22 |
  | 15 | top-right corner (16, 23) |
  | 16 | bottom-left corner (22, 10) |
  | 17-26 | side tiles, one per frame, alternating right and left: (17, 23), (21, 10), (18, 23), (20, 10), (19, 23), (19, 10), (20, 23), (18, 10), (21, 23), (17, 10) |

  (right side top to bottom, left side bottom to top, interleaved). The remake may draw the whole box at once; if it
  keeps the build-up, play it at one step per frame in this order and start sub-state 5 after k = 27
  (`game-flow.md` 7.1).
- **Cursor:** two sprites with tile $85 (palette 1), one on each side of the selected line, at x = 86 and x =
  173. START GAME: sprite-memory y = 161 (screen line 162); CONTINUE: y = 177 (16 lines lower). Select (newly
  pressed) moves them, taking effect within 2 frames. **[V]**
- **Jetman flying along the bottom** (it was called "walking": it jets). A small Jetman crosses the screen during
  sub-state 5. **[V]** Let `s` = frames since sub-state 5 started (power-on frame 1185 = s 0). The path is the same
  in every run: starting the same sub-state at another absolute frame (Start pressed in sub-state 0) and pressing
  Select during it gave 900 frames with no difference, so it depends on neither the random generator nor input.
  - **Parts.** A 2 × 2 group of sprites, palette 0, in front: bottom row tiles ($0C, $0D) in the *thrust* pose or
    ($0E, $0F) in the *fall* pose at (x, Y) and (x + 8, Y); top row tiles $01, $0B at (x, Y - 8) and (x + 8, Y - 8)
    (Y is the sprite-memory y of the bottom row). In the thrust pose (and for the first 3 frames of a fall) a
    flame sprite: tile $11 or $12 alternating every 2 frames, palette 1, at x - 1 and 1 or 2 lines below Y.
  - **x:** exactly 1 pixel every 4 frames: x = floor((s - 18) / 4) for s from 18. Sub-state 5 ends when x would
    reach 244 (s = 994; the figure was last seen at x = 243), which is the "about 990 frames". At s 2-17 only a
    pair of sprites is shown (tile $0B at x = 4-7, y = 124-127 above tile $0D, then $0F, at y = 132-135); the
    group appears in full at s = 18.
  - **Y:** alternating *thrust* (rising) and *fall* (sinking) stretches, each easing in and out. In s (the pose
    changes at the stretch's first and last sample of the table):

    | Pose | s | Y from - to |
    |---|---|---|
    | fall | 18-98 | 137 - 207 (the entry from the upper left) |
    | thrust, fall | 102-142, 146-186 | 207 - 186, then 186 - 207 (44 frames each; period about 88) |
    | thrust | 190-294 | 207 - 122 (one long climb, 108 frames) |
    | fall, thrust, fall, ... | 298-690 | alternately 122 - 143 and 143 - 122, 44 frames each (the first stretch is a fall from 122) |
    | thrust | 694-714 | 143 - 137 (cut short) |
    | fall | 718-806 | 137 - 207 (one long descent) |
    | thrust, fall, thrust, fall | 810-850, 854-890, 894-934, 938-974 | 207 - 186, 186 - 207, again (about 40-44 frames each) |
    | thrust | 978-994 | 207 - 202, where sub-state 5 ends |

    **The rule behind the stretches [V]** (a model run against the object's own position, speed and direction on
    every frame from s = 3 to 992, and against all 244 samples of the table below: no difference). The figure is
    object slot 0, moved on its vertical axis by the same homing scheme as the sun camera (2.2) but with different
    constants; horizontally it just moves 0.25 pixel per frame (the x rule above). State: position y (8.8, pixels),
    speed magnitude v (8.8), direction flag (down or up). At s = 3 (the first step): y = 128.00, v = 0, direction
    down. At each later s:

    1. toward = (direction down and T >= floor(y)) or (direction up and T < floor(y));
    2. toward: v = min(v + 12/256, 1.00), then y moves by v in the direction;
    3. not toward: if v = 0, flip the direction (no move); else if v <= 12/256, set v = 0 and flip (no move; note
       "<=", the sun camera uses "<"); else v = v - 12/256 and y moves by v.

    The target T is **192** except for s = 194 to 705 (512 steps), when it is **128** (any switch-on step from 168
    to 212 gives the same output; the switch-off step 706 is exact). This is a bang-bang swing about T: speed up
    toward T, brake once past it, turn at rest, so a swing between y = 182 and 204 (T = 192, 22 lines) takes 44
    frames and one between y = 118 and 140 (T = 128) the same. At s = 194 the figure is rising from a turn at y = 204,
    so the new target above it only lets it keep accelerating: the long climb (speed capped at 1 line per frame) up
    to y = 118, 11 lines past T. At s = 706 it is rising slowly (about 0.5 line per frame) from y = 140; the new
    target below makes it "not toward", so it brakes to rest at y = 133 (s = 717), turns and falls the long way to
    y = 204. **Pose:** thrust when the direction is up, fall when it is down. **Sprite y** (sprite memory y of the
    bottom row) = floor(y) + 4 (line 4 lower than the object). The target comes from the title logic, which picks
    it from its frame counter (RAM $5E:$5D): 128 while the counter is from $034E to $054D, 192 otherwise (the
    writes were seen to switch in frames where $5D = $4E with $5E = 3 and 5) **[V]** for the two switches, **[H]**
    that the condition is a plain range (only one window occurs in the 994 frames of the sub-state). Table (the
    recorded values, which the model reproduces): Y at every 4th frame from s = 18 (T = thrust pose, F = fall
    pose):

```
  137F 140F 144F 148F 152F 156F 160F 164F 168F 172F 176F 180F 184F 188F 192F 196F
  200F 203F 205F 207F 207F 207T 207T 205T 203T 200T 197T 193T 190T 188T 186T 186T
  186F 186F 188F 190F 193F 196F 200F 203F 205F 207F 207F 207T 207T 205T 203T 200T
  197T 193T 189T 185T 181T 177T 173T 169T 165T 161T 157T 153T 149T 145T 141T 137T
  133T 129T 126T 124T 122T 122T 122F 122F 124F 126F 129F 132F 136F 139F 141F 143F
  143F 143T 143T 141T 139T 136T 133T 129T 126T 124T 122T 122T 122F 122F 124F 126F
  129F 132F 136F 139F 141F 143F 143F 143T 143T 141T 139T 136T 133T 129T 126T 124T
  122T 122T 122F 122F 124F 126F 129F 132F 136F 139F 141F 143F 143F 143T 143T 141T
  139T 136T 133T 129T 126T 124T 122T 122T 122F 122F 124F 126F 129F 132F 136F 139F
  141F 143F 143F 143T 143T 141T 139T 136T 133T 129T 126T 124T 122T 122T 122F 122F
  124F 126F 129F 132F 136F 139F 141F 143F 143F 143T 143T 141T 139T 138T 137T 137F
  137F 139F 141F 144F 148F 152F 156F 160F 164F 168F 172F 176F 180F 184F 188F 192F
  196F 199F 202F 204F 206F 207F 207T 206T 205T 202T 200T 196T 192T 190T 188T 187T
  186T 186F 187F 189F 192F 195F 199F 202F 204F 206F 207F 207T 206T 205T 202T 200T
  196T 192T 190T 188T 187T 186T 186F 187F 189F 192F 195F 199F 202F 204F 206F 207F
  207T 206T 205T 202T
```
  - The cursor and the box (below) are unaffected by the figure.
- **Ends:** Start or a timeout (`game-flow.md` 7.1).

### 2.4 "Federation of Space Loonies" badge (sub-state 6, and other uses) [V]

- **Background:** the nametable is cleared (page 0 all zero).
- **Sprites:** 62 sprites in a block, tiles $C0-$FD of table 0 (`graphics.md` 4.5, 0x357C3 at $0C00), in
  order, row by row:

  | y | x of the first sprite | Tiles | Palettes |
  |---|---|---|---|
  | 69 | 109 | $C0-$C5 (6) | 0 |
  | 77 | 101 | $C6-$CD (8) | 0 |
  | 85 | 101 | $CE-$D5 | 1 |
  | 93 | 101 | $D6-$DD | 1 |
  | 101 | 101 | $DE-$E5 | 2 |
  | 109 | 101 | $E6-$ED | 1, 1, 3, 3, 3, 1, 1, 1 |
  | 117 | 101 | $EE-$F5 | 0, 0, 3, 3, 3, 3, 0, 0 |
  | 125 | 109 | $F6-$FB (6) | 3 |
  | 133 | 125 | $FC-$FD (2) | 3 |

  Sprites in a row are 8 pixels apart.
- **Palette:** preset 20 (`graphics.md` 7.2).
- **Ends:** after 256 frames (title), then the attract cycle (`game-flow.md` 7.1). The same badge is shown in the
  badge intermission (section 11.2, where it sits 48 lines higher) and during the credits (14.3: 8 pixels further left,
  behind the text). [V] title and credits, [C] others

## 3. Attract story: floating Jetman [V]

- **Background:** nametable page 0 holds the Jetman picture first (page 1 is filled from c = 324 on, see Animation): tiles 0x30973 / 0x31838 (`graphics.md` 4.5), the picture
  written into a 16-column × 11-row window at rows 0-10, columns 0-15 from the animation frames at ROM 0x3234A /
  0x32ADA (`graphics.md` 4.5). At the start all 11 rows of frame 0 are written at once.
- **Clock.** In this section `c` is RAM $5D + 256 × $5E read at the end of a frame: the counter is reset on the
  first loading frame, the picture is written when c = 3, the first visible frame has c = 4 (power-on frame 2450)
  and the last has c = 757; the screen goes blank at c = 758, so the scene shows 754 frames. The thresholds of
  `game-flow.md` 7.1 (240, 320, 600, 680) are c values tested after the increment; each shows 1-3 frames later (the
  list below gives the observed frames). **[V]** (recorded frame by frame: nametable changes, scroll, PPU control).
- **Animation (what is on screen).** The scene keeps **two copies of the picture's nametable** and flips between
  them: the mapper's single-screen page bit (bit 4 of the bank register, shadow RAM $75) chooses which of the two 1 KB
  pages the PPU shows. The next picture is written, one row per frame, into the page that is not shown, and the pages
  are swapped when it is complete. So the player sees a change at each swap, not at each write. (Reading only one of
  the two pages, as an earlier analysis here did, sees every second picture: the 22-frame cadence and "pictures 5, 7, 9"
  that this section carried from 2026-10-17 were that mistake. The nametable dumps of the implementer, taken from the
  PPU's own view of the displayed page, were right.) **[V]** Re-measured 2026-10-17: at the end of every frame from
  c = 3 to c = 766 the content of the PPU's nametable (which is the displayed page), page 0 and page 1 were each compared
  with the 11 ROM frames, with $75 and the PPU control byte.

  | Shown from c (end-of-frame reading) | Picture (ROM frame, `graphics.md` 4.5) | Shown for |
  |---|---|---|
  | 4 | 0 | to c = 330 |
  | 331 | 1 | 11 frames |
  | 342, 353, 364, ... (every 11) | 2, 3, 1, 2, 3, 1, ... | 11 frames each |
  | 595 (the last of the loop) | 1 | 16 frames (to c = 610) |
  | 611, 622, 633, 644, 655 | 5, 6, 7, 8, 9 | 11 frames each |
  | 666 | 10 | to the end of the scene (c = 757) |

  So the faces cycle **1, 2, 3, 1, 2, 3 ...** with a new picture every 11 frames, 25 pictures from c = 331 to 595 (the
  k-th, k = 0 to 24, is ROM frame k mod 3 + 1, shown from c = 331 + 11k), then the closing pictures 5 to 10, six of them,
  every 11 frames from c = 611, the last held. Frames 4 never appears (below). The swap is instantaneous: the
  pages are complete when swapped. **[V]**
  - **Writes into the hidden page** (not visible, listed for timing): the first picture (frame 1) is written whole, in 7
    or 8 frames before c = 331 (c = 324-330, a row or two per frame). After that only the face's rows 3, 4 and 5 differ
    between two faces and are written on three consecutive frames starting 7 frames before the swap (c = 335-337 for the
    swap at 342, 346-348 for 353, ...), one attribute byte (row 30) is written a few frames earlier, so the hidden
    page is ready 5 frames before it is shown. The closing pictures differ from the previous one in more rows and are written
    in 7, 8, 9, 10, 10 and 10 frames (c = 602-608 for frame 5, 612-619, 623-631, 634-643, 645-654, 656-665 for frames 6-10),
    finishing 1 to 3 frames before their swap. At c = 760-761 (blank screen) rows 0-10 are cleared and at 767 the title
    nametable is written.
  - **The program's own frame number** steps 1, 2, 3, 1, 2, 3, ... through the face loop (c = 320 to 599) and 5, 6, ..., 10
    through the closing run (c = 600 to 679), one step per 11 frames (11 rows are copied, one per frame), and **every step
    is shown**. (The earlier remark that half of the steps never reach the screen, with a hypothesis, was the page mistake
    above and is withdrawn.)
  - **Why frame 4 is skipped [C]:** the phase of the story (by c: 0-239, 240-319, 320-599, 600-679, 680 on) picks
    its starting frame from a five-entry table (ROM 0x00582: $FF $FF $01 $05 $FF = keep, keep, 1, 5, keep; the
    thresholds are the 16-bit words at ROM 0x0057A: 240, 320, 600, 680); the face loop wraps from 3 back to 1 and
    the closing run counts up from 5. Frame 4 is therefore never selected: it is spare data (its rows 1-7 differ
    from frame 3, as frame 5's do). There is no further rule.
- **PPU control.** Bit 4 of the control byte (background pattern table) is cleared at c = 644, the frame in which picture 8
  is first shown (control byte $90 to $80, the only change in the scene): pictures 8, 9 and 10 use the second tile set
  (0x31838, `graphics.md` 4.5), pictures 0-7 the first. **[V]**
- **Scroll x:** 0 at c = 4, then **-2 per frame** from c = 5 to c = 240, **-1 per frame** from c = 241 to c = 600,
  then it stops at **x = 192** (c = 601). (-2 gives 0, 254, 252, ...; the value shown is x mod 256.)
- **Scroll y** is a floating wave, given exactly by the frames at which it changes by one line. y = 184 at c = 4.
  - **c = 5-317:** y **rises** by 1 at c = 16, 21, 25, 28, 30, 33, 35, 37, 39, 41, 43, 45, 48, 50, 53, 57, 61, 70,
    152, 157, 161, 164, 166, 169, 171, 173, 175, 177, 179, 181, 184, 186, 189, 193, 197, 206, 259, 262, 263, 265,
    267, 268, 271, 303, 306, 307, 309, 311, 312, 315 and **falls** by 1 at c = 76, 85, 89, 93, 96, 98, 101, 103, 105,
    107, 109, 111, 113, 116, 118, 121, 125, 130, 212, 221, 225, 229, 232, 234, 237, 239, 241, 243, 244, 246, 249,
    281, 284, 285, 287, 289, 290, 293. So it climbs 184 to 202 (c 16-70), falls to 184 (c 76-130), rests on 184
    until c 151, climbs to 202 (c 152-206), falls to 189 (c 212-249), climbs to 196 (c 259-271), falls to 189
    (c 281-293) and climbs to 196 (c 303-315).
  - **c = 318-599:** a repeating cycle of **24 frames** (starting at c = 318, 342, 366, ...; the exact table holds
    through c = 599 except that c = 512 and 513 are one line off): y = 196, 196, 196, 196, 196, 195, 194, 193, 191,
    190, 189, 188, 188, 187, 187, 188, 188, 189, 190, 191, 193, 194, 195, 196 for the 24 frames starting at c = 342
    (the cycle starts at c = 318, the frame after the rest on 196 that ends the first phase; the 24-frame cycle and
    the 22-frame face cadence are independent). At c = 600 y becomes 190 and holds.
  - **c = 601-680:** the scroll is (192, 190) and does not move.
  - **c = 681-758:** x stays 192; y rises 7 lines (by 1 at c = 681, 682, 684, 685, 687, 688, 691) to 197, rests, then
    falls **faster and faster**: by 1 at c = 701, 704, 705, 707, 708, 710-714, 716, 717, 719, 720, 723; by 2 at
    c = 715, 718, 721, 722, 724-732, 734, 735, 738, 741; by 3 at c = 733, 736, 737, 739, 740 and every frame
    from 742 to 758 (y = 133 at c = 741, 85 at 757, 82 at 758). The fall speeds up from about 0.5 to 3 lines per
    frame (**[H]** an object speed with a cap of 3). Then the screen blanks and the title restarts from the
    sun (`game-flow.md` 7.1).
- **Sprites:** 56 star sprites, the same from the first to the last frame (they do not scroll with the picture):
  tiles $E0-$E5, behind the background, palette as listed; sprite-memory y (screen line = y + 1) and x:

| slot: y x tile palette | slot: y x tile palette | slot: y x tile palette | slot: y x tile palette |
|---|---|---|---|
| 0: 154 9 $E5 1 | 1: 32 7 $E0 1 | 2: 44 41 $E0 1 | 4: 16 9 $E0 0 |
| 5: 211 7 $E4 1 | 6: 0 232 $E1 1 | 7: 0 180 $E1 1 | 9: 16 216 $E5 0 |
| 10: 141 240 $E4 0 | 11: 15 88 $E1 0 | 12: 32 255 $E3 3 | 13: 128 255 $E1 1 |
| 14: 5 32 $E1 3 | 15: 217 93 $E0 0 | 16: 11 94 $E1 0 | 17: 48 0 $E4 2 |
| 18: 112 133 $E1 1 | 19: 185 7 $E5 0 | 22: 32 144 $E2 3 | 23: 24 197 $E5 1 |
| 24: 169 133 $E5 0 | 26: 32 10 $E5 0 | 27: 24 0 $E3 3 | 28: 32 128 $E1 2 |
| 29: 2 170 $E0 1 | 30: 141 23 $E3 0 | 31: 5 4 $E1 3 | 32: 142 32 $E0 0 |
| 33: 6 93 $E1 3 | 34: 32 198 $E4 3 | 35: 137 76 $E5 0 | 36: 215 1 $E5 1 |
| 37: 1 23 $E5 1 | 38: 169 16 $E1 1 | 39: 212 32 $E0 0 | 40: 32 93 $E0 0 |
| 41: 0 198 $E5 0 | 42: 32 134 $E2 0 | 43: 2 43 $E0 0 | 44: 169 255 $E4 0 |
| 45: 32 66 $E0 2 | 46: 2 211 $E5 1 | 47: 172 133 $E2 3 | 48: 160 208 $E0 0 |
| 50: 36 4 $E1 3 | 51: 32 1 $E3 0 | 52: 114 128 $E2 0 | 54: 141 201 $E0 0 |
| 55: 152 165 $E0 0 | 57: 160 240 $E1 1 | 58: 130 169 $E2 0 | 59: 185 169 $E0 2 |
| 60: 3 2 $E5 2 | 61: 128 162 $E3 1 | 62: 128 160 $E0 0 | 63: 169 188 $E0 0 |

  Slots 3, 8, 20, 21, 25, 49, 53 and 56 are unused (off-screen). **[V]**
- **Palette:** preset 19 (`graphics.md` 7.2); the backdrop colour is that preset's entry 16 (data byte 15, ROM
  0x07685), a medium blue. **[V]**

### 3.1 Attract demo (gameplay on levels 6, 4 and 10) [V]

- **Composition:** ordinary gameplay (`levels.md`, `player.md`), nothing added or removed: the full status panel
  (fuel and weapon gauges full, four jetpods, four lives, score 001000), no "DEMO" lettering, no start-up scene
  before it. The pod is shielded (probe shields) whenever the autopilot raises the shield, which shows as the
  shield ring. The shield works because the demo gives the pod the equipment byte $C0 (probe shields and
  boosters, `game-flow.md` 7.1): without it the autopilot's Down presses would do nothing.
- **Entry:** no fade. The screen is blank for 28 frames (table in 0.1: frames 5120-5147 of a power-on run, play
  from 5147), and **no music starts: the demo is silent**. **[V]** (the sound chip received no register write in
  400 frames of demo 1 and of demo 2, and the sound driver's current-song byte stayed at 1; the cause is **[H]**:
  music requests are dropped during a demo like effect requests, `game-flow.md` 1.1). The only music that could
  still be playing is the title song, which lasts 2,662 frames from the title's first frame and has ended long
  before the first demo.
- **Exit:** when the pod is destroyed the player state is 4, and 22 frames later 2; the screen blanks in that
  frame and the title's first visible frame follows 17 frames later (table in 0.1); a newly pressed button ends it
  at the next logic step. Timeline of the first demo of a power-on run: set-up 5120, play 5147, state 4 at 9584,
  state 2 and blank at 9606, title visible at 9623.
- **Input, set-up, steering and end:** `game-flow.md` 7.1.
- **Special weapons:** the demo pod fits homing missiles (demo 1), star bullets (demo 2) and multi warhead (demo
  3) with a full weapon gauge; the autopilot fires one pulse every 32 frames at a live enemy and the gauge goes down
  by 4, 32 or 16 per shot (`game-flow.md` 7.1, "Demo weapons"). Nothing on the screen announces them. **[V]**
- **What carries over between demos:** the pod's thrust accumulators $C4/$C5, the shield timer, the invulnerability
  timer (96 steps at the start of every demo but the first), the random generator and the parallax-star state are
  not reset; everything else listed in `game-flow.md` 7.1 is. The pod's horizontal thrust carry-in during a demo is
  the high byte of the current waypoint's x, not the star job's value (`game-flow.md` 7.1). **[V]**
- **Sound effects:** none (requests are dropped), nor any music.

## 4. Password entry (menu screen B) [V]

- **Background:** frame from the menu B stream (`graphics.md` 4.5) at rows 4-21; "ENTER PASSWORD" row 9 column 9,
  the 12 letters row 12 columns 10-21 (all "A" at the start), "OR PRESS START TO PLAY" row 15 column 5.
- **Scroll:** x = 0, y = 2 **[V]**. The load ends with the scroll written 0, 0 and then the video address set to
  $2000, which leaves a fine vertical scroll of 2 (`graphics.md` 1.1), as on the licence screen. Row r is at
  screen line 8r - 2 and the last two lines repeat lines 0-1 of row 0 (blank here). Never changed afterwards.
- **Left 8 pixels blank [V]:** this is the display's left-column mask, not part of the picture: the mask register
  holds $18 for the whole picture, so neither background nor sprites are drawn in screen columns 0-7
  (`graphics.md` 1.3). It is a hardware setting the original switches on in every picture scene. What lies under
  it here is the left edge of the frame art (nametable column 0, rows 4-21: tiles $25, $29, $2C, $15-$1E). The
  remake may keep the 8-pixel margin to look the same, or draw the full width and show that edge; nothing else
  is affected.
- **Cursor:** one sprite, tile $5C (the hyphen glyph, used as an underline), palette 1, at x = 80, y = 101 under
  the first letter; it moves 8 pixels per letter. [V] first position, [C] movement
- **Palette:** preset 24 then preset 9 (`graphics.md` 7.2): background palette entries 1-15 only.
- **What carries over from the title [V]** (video memory dumped in the last title frame and in the first password
  frame of a power-on run, then compared):
  - **Pattern table 0** (the title's sprite tiles): all 256 tiles unchanged. **Pattern table 1:** only tiles $01-$2C
    (44 tiles, the frame art of menu screen B, `graphics.md` 5.9) and $64-$65 are rewritten; every other tile is
    byte-identical to the title's, including the font (letters, digits, and the hyphen $5C that is the cursor).
    The title's logo and menu-box tiles stay in memory unused.
  - **Palette:** background entry 0 ($0F) is kept; entries 1-15 become, as palettes of four (entry 0 shared):
    background palette 0 = $30, $22, $02; 1 = $31, $21, $11; 2 = $3C, $2C, $0C; 3 = $3A, $2A, $0A (entries 4, 8
    and 12 are $00). The title's background colour flashing (entries 13-15) ends. **Sprite palettes (entries
    16-31) are not touched:** 0 = $34, $24, $14; 1 = $38, $28, $18 (the cursor's); 2 = $32, $22, $12; 3 unused
    (zeros), as the title left them.
  - **Nametable page 0** is cleared (every cell to $00) in the first loading frame and then rewritten with the
    password screen and its attributes (menu B stream and attributes, `graphics.md` 4.5); sprite memory is
    rewritten (the cursor is the only sprite).
  - **Display:** control register $98 (both pattern tables $1000, `graphics.md` 1.1), mask $18; the title's
    pattern-table setting ($90: sprites from $0000) is replaced.
- **Ends:** `game-flow.md` 7.2 and `password.md`.

## 5. Interplanetary flight [V]

The whole scene was reproduced by a model of the rules below and compared with the original frame by frame (sprite
memory, scroll, the generator state and the counters; 1,343 frames, 46,529 sprite records): no difference. The only
random input is the generator (`game-flow.md` 5), and it only shapes the small stars (5.3).

- **Entry** (START LEVEL with level index $3B not 0, `game-flow.md` 7.3). Music song 4 starts two frames before the
  first pass (section 15). Palette preset 1 once (`graphics.md` 7.2: shadow entries 1-16), preset 2 on every pass
  (entries 17-31); shadow entry 7 is written on every pass (5.1). The mothership is printer message 49 (`text.md`
  3: picture mode, rows 0-6, columns 1-14 of page 0, plus one attribute byte at row 30 column 10). Background
  pattern table $1000, sprite table $0000, nametable page 0, scroll as in 5.2. **There is no status panel, no
  sprite-0 split and no text** (the earlier statement of a panel at the bottom was wrong **[V]**); nothing fades in.
- **Load list** (in this order, before the first pass) **[V]** (code read, and the picture matched in Mesen): the title
  sprite tiles 0x0FF2F → $0800, 0x15546 → $0860 and 0x0FE52 → $0B80 (`graphics.md` 5.2, the same three loads as the
  title, without the title's common sprite set at $0010); then printer message 49 (the mothership's tile codes $00-$3E); then the stream 0x12FFD (RLE, 0x3F1 bytes
  → $1000), which is the **63 background tiles of the mothership picture** (it was listed as "screen of bank 2
  entry $8033, scene not identified"; it is this scene's tile set). No nametable or attributes are loaded other than
  by message 49. Left-column mask: $18 for the whole scene except the last 138 frames (`graphics.md` 1.3), so
  sprites with x < 8 are hidden here too **[V]**.
- **Counter and passes.** The scene runs one pass per frame. Let `c` be the frame counter RAM $5D and `b` the carry
  counter RAM $5E (b rises by 1 each time c wraps from 255 to 0); at the scene's start c = 1, b = 0. Every pass:
  1. A newly pressed: the scene ends at once and nothing else is done **[V]** (A at pass 100: the shop's loading
     began 13 frames later);
  2. c += 1 (the first pass has c = 2);
  3. one generator step (`game-flow.md` 5, it sees the incremented c): R = RAM $5A, R1 = RAM $5B afterwards;
  4. the stars (5.3, 5.4);
  5. the counters (5.2);
  6. palette entry 7 (5.1), then palette preset 2 again (entries 17-31) **[C]**: it comes after the stars of the same
     pass, which is why the implosion's preset 7 (5.4) survives only in entry 16;
  7. the camera (5.2);
  8. the fade and end test (5.5).
  The scene lasts **1,343 passes**, c = 2 to c = 1344 (b = 5, c mod 256 = 64). Frame k shows the state left by
  pass k (the original shows it one frame later; not observable).
- **Counters** (8-bit): `mask` (RAM $6A, starts 15), `speedbase` (RAM $68, starts 0), `K` (RAM $6B, starts 0),
  `radius` (RAM $66, starts 160).

### 5.1 Picture, sway and colours

- **The mothership is a fixed picture** (14 × 7 tiles at columns 1-14, rows 0-6); the scene moves it by
  scrolling. Its tile column 0, row 0 is at screen (256 − sx, 240 − sy), so the picture covers x from 264 − sx and
  y from 240 − sy, 112 × 56 pixels, cut off at x = 255. Measured: at scroll (119, 132) the visible picture spans
  x 147-251, y 113-163. **[V]**
- **Window colour.** Shadow palette entry 7 (the lit window) is set every pass from the 12-byte table at bank
  0:$8996 (ROM 0x00996; colour numbers, ripped): index i = (c AND 31) / 2 (integer), minus 12 when i is 12 or
  more. It runs 0-11 and then 0-3 again, a cycle of 32 passes, and changes every second pass. **[V]**
- **Sprite colours** are the sprite palettes of preset 2: palette 0 (entries 17-19), 1 (21-23), 2 (25-27),
  3 (29-31).
- **Backdrop.** Shadow entry 16 (universal background) is preset 1's last byte, black, until the flash (5.4).

### 5.2 Camera sway and the counters

The mothership's path is the scroll, which is the position of one **camera object** (pixels): it starts at
(112, 140) with both speeds 0 and direction bits "right" and "up", and on every pass homes on the point
(124, 126) by the rule of `behaviour.md` 2.3 (x first, then y) with **accel = (8 × K + 2) mod 256** (1/256 px per
pass) and **maximum M = $23** (in effect the 3.00 cap of 2.2). Scroll x = its x and scroll y = its y after the
pass. The picture therefore swings about (124, 126), along the line sx + sy = 251, with an amplitude that shrinks as
K grows (about 13 px each way at first, 1 px at the end). Verified points (scroll after the pass that has this c):

| c | 2-17 | 21 | 51 | 101 | 201 | 301 | 501 | 701 | 901 | 1101 | 1201 | 1301 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| (sx, sy) | (112,139) | (113,138) | (121,130) | (137,114) | (115,136) | (129,122) | (117,134) | (120,132) | (120,131) | (125,126) | (124,127) | (124,127) |

Counters (each tested after the stars of the same pass):

| Counter | Rule |
|---|---|
| mask | halved (15, 7, 3, 1, 0, 0) when c wraps to 0: c = 256, 512, 768, 1024, 1280 |
| speedbase | +1 when c AND 127 = 0 |
| K | +1 when b ≥ 2 and c AND 63 = 32 (c = 544, 608, ...), **with no upper limit** [C, V: the model with this rule matched all 1,343 passes]: K = 5 at c = 800 (5 is only the threshold at which the ring starts to shrink), 6 at 864, ..., 11 at 1184; set to 55 in pass 1205 (5.4), then +1 again at c = 1248 (56) and 1312 (57). K also sets the camera's accel (5.2), so it matters for the sway all the way |

### 5.3 Small stars (slots 1-22)

Each small star is one sprite: tile $81 + f (f = 0-4, five growing dots), palette 0, behind the background
(attribute bit 5), sprite-memory y = its y (screen line y + 1), x = its x.

- **Spawn**, on a pass with R AND mask = 0, b < 5 and a free slot (lowest free):
  x = 96 + (R1 AND 31) − 15; y = 96 + ((R1 / 4) AND 31) − 15 (R1 / 4 = shift right 2). Direction: left when
  (R1 AND 31) < 15, else right; up when ((R1 / 4) AND 31) < 15, else down (fixed for the star's life). Both speeds
  start at 0.125 px per pass (8.8 value $0020). Accel codes (8-bit): ax = (R1 AND 7) + speedbase,
  ay = ((R1 / 8) AND 7) + speedbase. Chance per pass: 1/16, 1/8, 1/4, 1/2, then every pass as mask falls. A typical
  run has about 3 stars at c = 257, 7 at 513, 14 at 769, 22 (all slots) at 1025.
- **Each pass, per star**, for each axis with code a (ax for x, ay for y) and build-up value g (8-bit, starts 0):
  - if a ≥ 4: g += a / 4 (integer); if a < 4: g += 1 on passes where (c AND m) = 0, m = 15, 7, 3, 1 for
    a = 0, 1, 2, 3, else g unchanged;
  - x speed (8.8) += (2 × g) AND 255 if the star moves right, += g / 4 if it moves left (the 8:1 difference is the
    original's); y speed += g; carries go into the whole part;
  - frame f: 4 if either speed's whole part is 4 or more (bits above bit 1), else f = 2 × (y speed whole) + top bit
    of its fraction, at most 4;
  - move: x ± x speed, y ± y speed on the 16.8 positions (+ for right/down);
  - **removed** when x or y has left 0-255 (any carry or borrow into the high byte).

### 5.4 The ring of eight big stars, the implosion and the flash

- **Appearance.** On every pass with c AND 63 = 0 (c = 64, 128, ..., 512; then all eight slots 23-30 are in use)
  a big star is created in the lowest free slot with angle a = 32 × (0-7, its order) and fade counter t = 15. Each
  pass a = (a + K) mod 256 (so the ring is still until c = 544). With r = radius, (dx, dy) from 5.4a, the star's
  centre is **(96 + dx − floor(dx / 4), 96 + dy)** (floor toward minus infinity, dx and dy signed: the ring is an
  ellipse 3/4 as wide as tall). It is four sprites forming 16 × 16: top-left sprite at (cx − 4, cy − 4), the
  right-hand column 8 pixels right and flipped horizontally; the top row is tile $B8, the bottom row $B9; attribute
  bit 5 set (behind the picture). Palette: while t > 0, 3 − floor(t / 4) with t counting down 15 to 1 (palette 0 for
  4 passes, 1 for 4, 2 for 4, 3 for 3 passes), then permanently 0 for the 1st, 3rd, 5th, 7th star and 1 for the
  others.
- **5.4a Rotation.** With S = byte i of the 128-byte table at bank 7:$8C74 (ROM 0x38C74, values $00-$7F rising to
  the top and falling again: 128 × sin over a half turn; ripped): fold i = a for a < 128, else 256 − a (a = 128
  gives index 128, read as 0). dx = floor(S × r / 256), negative when a ≥ 128. For dy: j = (i + 64) mod 256, index
  j for j < 128 else 256 − j (128 reads 0), dy = floor(S' × r / 256) negative when j ≥ 128. So (dx, dy) =
  (r / 2) × (sin, cos) of the angle (256 steps per turn), magnitudes truncated. **[V]**
- **Shrinking.** From the pass after K reaches 5 (c = 801) the radius (8.8: whole = radius, plus a fraction) falls
  each pass by a speed v (8.8, starts 0) that grows by 1/256 on every pass with c even; the ring spirals in.
- **Implosion** (**c = 1205** [V], not 1204: the radius is the 16-bit value whole part RAM $66 plus a fraction; each
  pass the speed is subtracted from it, and the implosion is the first pass in which that subtraction goes below 0.
  After pass 1204 the whole part is already 0 but the fraction still covers the speed (0.79 px/pass at that point), so
  the ring has not yet imploded; pass 1205 borrows). In pass 1205: radius := 0; background rendering is
  switched off for the rest of the scene (the mothership vanishes; sprites show over the backdrop colour); palette
  preset 7 is loaded into shadow entries 16-19 (`graphics.md` 7.2; entries 17-19 are rewritten by preset 2 on the
  next pass, so the effect is entry 16, a light colour: the screen goes **white**); K := 55 (the ring spins 55
  steps per pass; the ring's own rotation in pass 1205 has already used the old K = 11, so the 55-step spin first
  shows in pass 1206; the camera of pass 1205, which runs after, already uses K = 55); speedbase := speedbase × 4
  (8-bit; the +1 every 128 passes continues from there); the radius speed becomes 6 and the radius is in "grow"
  mode from the next pass.
- **Explosion.** The radius then grows by 6 per pass starting in pass 1206 (radius 6 after 1206, 12 after 1207, ...; none
  in 1205 itself), capped at 255 (reached at c = 1248); once it is 255 the big
  stars are no longer drawn (c = 1249 on). Small stars keep spawning (with the larger speedbase) while b < 5 and
  leave as usual.

### 5.5 Fade-out and the end

- While b = 5, on passes with c AND 7 = 0: shadow entry 16 := entry 16 − 16 (one luminance row darker); a negative
  result becomes the black value (preset 1's last byte). The white flash lasts c = 1205-1279, then c = 1280: one step
  darker, 1288: darker, 1296: darker, 1304: black. **[V]**
- Also while b = 5 and c AND 7 = 0: if entry 16 is already the black value and c AND 255 ≥ 64 the scene ends.
  First true at c = 1344. Odd $3B then goes to the shop, even $3B to the approach screen (`game-flow.md` 7.3).
- **Timeline** (c): ring stars appear every 64 from 64; spin starts 544 (K = 1); K = 5 at 800; implosion and flash
  1205; ring gone 1249; fade steps 1280, 1288, 1296, 1304; end 1344. **[V]**

## 6. Shop and equipment list [V]

The shop and the equip list are one scene (`game-flow.md` 7.3) with two modes; the shop is entered after the flight
to an odd planet index, the equip list from EQUIP POD on the surface. Prices, stock, buying and fitting are in
`economy.md` 6; this section is the screen and its controls. Everything below was recorded frame by frame in
Mesen (a five-item shop, a fourteen-item shop, an equip list with five items owned); the strip's tile contents are
the only [C] part.

- **Palette and tiles [V]:** the screen of `graphics.md` 5.10, two variants of one loader: the shop is the variant
  with RAM $63 bit 7 set (tiles 0x1579C → $1000, attributes 0x112EE, palette bytes $932E), the equip list the other
  one (tiles 0x15A02 → $1000, attributes 0x1130F, nametable 0x1134C, palette bytes $933D); both then preset 4. Neither
  uses menu screen D or the equipment-block loader of `graphics.md` 5.8 (checked 2026-10-17 by logging every
  loader call and comparing the pattern table byte for byte). Background pattern table $1000, sprites $0000. The
  scene uses nametable page 0 only, so the hardware strip repeats every 256 pixels. The sprite palettes are not
  loaded: they keep the previous scene's (shop: the flight's, equip list: the planet surface's, `graphics.md` 7.4).
- **Music:** none of its own: whatever played before continues (song 4 in the shop, section 15).
- **Layout (shop, rows and columns of page 0):**

  | Element | Where |
  |---|---|
  | logo (the "Interstellar Marketing Co" picture, shop only) | rows 2-5, columns 12-18, lines 16-47 |
  | strip of item frames (pipes), seven tiles wide, one per item, eight tiles (64 px) apart | rows 7-12, frame of cell k from column 4 + 8k to column 10 + 8k |
  | item picture inside a frame (3 × 3 tiles = 24 × 24 px) | rows 9-11, columns 6 + 8k to 8 + 8k |
  | item name, centred (shop **and equip list**) | row 15 (e.g. "HOMING MISSILES" at column 8) |
  | price, five digits | row 17, columns 13-17 (above the item box) |
  | item box (frame) | rows 19-24, columns 12-18 |
  | item picture in the box: nine sprites, 3 × 3 (rows 8 px apart, columns 8 px apart), tile numbers $80 + 9 n to $88 + 9 n for item n = 0-11 and $EC-$F4 / $F5-$FD for items 12 / 13 (the pictures of `graphics.md` 5.10 in table $0000), palette 0 | top-left at x = 112, **y = 166** (sprite-memory y; screen line 167; the rows are at 166, 174, 182); shown only while the count is 1 or more and no move is under way |
  | "YOU OWN" / count / "OF THEM" | row 20 column 3 / one large digit at row 21-22, column 6 / row 23 column 3 |
  | money: the score, six digits with leading zeros, large | rows 20-21, columns 21-26; "IN YOUR ACCOUNT" rows 22-23 column 21 |
  | "FIRE B TO EXIT" / "FIRE A TO SELECT" | rows 26-27 column 2 / column 23 |

  The equip list has the same strip, **the same item name** (row 15, centred, the same strings and the same redraw
  timing as the shop: cleared when a move starts, written again 1-2 frames after the move ends; the names are
  the table of `text.md` 4.5, items 0-11), "YOU OWN" / count / "OF THEM" and the two button captions, but
  purple frame art and a chain at the top instead of the logo, **no price and no money**, its item box moved to
  columns 16-22 (box picture sprites at x = 144, y = 166 instead of x = 112), and the fitted-item icons (below).
  **[V]** 2026-10-17 (nametable row 15 read back every frame; sprite memory read back; shop and equip list).
- **The strip.** A virtual row of cells, cell 0 an empty frame, then one frame per item of the list in list order
  (`economy.md` 6.1; the equip list: owned items 0-11 in item order). The picture of cell k is at world x
  48 + 64 k; the horizontal scroll of the strip is **64 × (index of the selected item)** pixels, so the selected
  item's picture is always at screen x = 112-135 (the middle), the previous item's at 48, the next at 176. The
  scroll applies to the strip only: from the split below the logo down to line 103 (rows 7-12); everything above
  and below is unscrolled. The split is a sprite-0 hit (the sprite sits at y = 44 in the shop and the strip's
  scroll is written about 3 lines later) - a hardware detail; the remake scrolls rows 7-12 (lines 56-103).
  The tile contents of a cell are rewritten as it scrolls into view (the hardware ring of four cells) **[C]**; the
  remake draws cell k from the item's picture directly.
- **Index, scroll and input (every frame, in this order).** Variables: `sel` (RAM $AA, 0 to `last`), `last` =
  list length − 1 (RAM $A9), `dir` (RAM $AC: 0, +2 or −2 pixels per frame) and the phase of a move (RAM $AB).
  1. If dir is not 0: the strip scrolls 2 pixels in that direction. A move is 64 pixels, **32 frames**: `sel` is
     lowered on the first step of a move to the left and raised on the last step of a move to the right; when the
     move completes dir = 0. The scroll is therefore always 64 × sel pixels when idle.
  2. Input: B (newly pressed) leaves at once, even in the middle of a move (shop: to the approach screen; equip
     list: to the launch scene). Otherwise, only when no move is under way: Left (newly pressed) with sel > 0 starts
     a move to the left; Right with sel < last starts a move to the right. A press is accepted in the very frame
     the previous move completes, so the fastest scroll is one item per 32 frames. There is **no auto-repeat**
     (`game-flow.md` 3.2: holding Right for 30 frames moved one item), no wrap, and a press at either end is
     ignored. A is handled by the purchase code (`economy.md` 6.3) and Up, Down, Start and Select do nothing.
     **[V]**
     **Timing of every button:** a button pressed in frame f is read at the end of the pass of frame f and acted
     on in pass f + 1 (the rule of 8.2, item 4), the same for B, Left, Right and A. In the equip list the fit
     (counts, RAM $D1/$D3/$050E, icon sprites) and the sound request all happen in pass f + 1; the old wording
     "takes effect in the frame A is pressed" was one pass early. **[V]** 2026-10-17 (A, B and Right each pressed for
     one frame, state read at the start of every frame).
  3. Redraws: when a move starts (2 frames after the press) the count digit and the box picture are cleared; the
     name (row 15, shop only), the count and the box picture of the new item appear 1-2 frames after the move
     ends. **[V]**
- **Cursor:** none. The selection is the item in the middle of the strip; there is no highlight sprite.
- **Count and money.** The count is a single digit 0-9. The money is the score as six decimal digits with leading
  zeros, in large digits (`text.md`); the price is five digits with leading zeros (e.g. 00400).
- **Fitted-item icons (equip list only).** One 2 × 2 block of sprites (palette 0) for each fitting slot that holds
  something, in the order engine/shield slot (RAM $050E) = slot 0, special weapon ($D1) = slot 1, third ($D3) =
  slot 2: tiles $13 + 4 × slot to $16 + 4 × slot (top-left, top-right, bottom-left, bottom-right), at
  sprite-memory y = 212 and 220, x = 100, 124, 148 for the first, second, third block in use (x and x + 8 for the
  two columns). Example: only the special weapon fitted: tiles $17-$1A at x = 100; with the engine slot also
  fitted: $13-$16 at x = 100 and $17-$1A at x = 124. Fitting takes effect in pass f + 1 for a press in frame f
  (see "Timing of every button"). **[V]**
- **Sound effects [V]** (every sound request, effect start and song start read back in Mesen, 2026-10-17):
  - **Cursor moves** (Left, Right, also refused ones at the ends) and **B**: no sound in either mode.
  - **Shop purchase:** effect $06 on both pulses (`sound.md` 8.3) at each payment step, i.e. once every 8 frames
    while the price is paid off, one step per digit unit (400 = 4 sounds, 1500 = 6: one for the thousand, five for
    the hundreds). No sound when the count is finally raised and none at the start of the purchase.
  - **Shop refusals** (score too low, 9 already owned, mapping device already owned): no sound, no payment.
  - **Equip list fit or un-fit** (A on an item with a count of 1 or more, fitted or not): effect $06 on both
    pulses, requested in pass f + 1 and started 1-2 frames later (the two request slots are served on alternate
    frames, `sound.md` 8.2). A on a listed item whose count is 0: nothing, no sound.
  - The music carries on; B in the shop then starts the approach screen's song (song 9 after the flight to planet 1,
    section 15).
- **Status panel in play:** the special weapon fitted here is drawn as a 3 × 3 icon in the gameplay panel
  (`graphics.md` 1.2).
- **Ends:** B (above). The shop leaves with the strip scroll reset to 0.

## 7. Approach screen [V]

- **Background, page 0:** the star-field stream (`graphics.md` 4.5, 0x35AD5) covers rows 0-23. Texts (printer
  message 51, `text.md`): "APPROACHING PLANET" row 2 column 7; planet number and name row 5, centred (e.g. "1
  PRELUDON" at column 11); "GRAVITY n" row 15 column 3 and "DIAMETER n" row 15 column 17; "PASSWORD" and the 12
  letters row 18 column 5; "PRESS START TO LAND" row 21 column 6.
- **Globe:** a 4 × 4 block of background tiles at rows 8-11, columns 14-17 (tiles from 0x35B99 at $1800, palette
  2), with 9 sprites over it (tiles $01-$09 of table 0 from 0x35F3C, palette 2, a 3 × 3 block at x = 112-143,
  y = 65-88: tiles $01-$03 in the top row at x 120, 128, 136; $04-$06 at x 112, 128, 136; $07-$09 at x 112, 128,
  136). The globe's colours are set per planet (`graphics.md` 7.3): the cells use background palette 2 (attribute bytes
  $88 and $22 at columns 14-17), entries 9-11. For planets 2, 4, 7 and 9 the program writes the planet's colours to the
  unused entries 21-23 instead, so those four globes show preset 0's colours ($22 $2A $19): quirk Q34. The sprites use
  sprite palette 2. It does not rotate: neither the cells nor the sprites change. [V]
- **Status panel, page 1** (shown from line 196, `graphics.md` 1.1; the gameplay panel is a different layout, with
  gauges and no system name: `graphics.md` 1.2): "SYSTEM n" row 4 column 1, the system name
  row 6 column 2 (`text.md` 4.2), remaining lives as pod icons at rows 4-6, columns 16-19, the score in large
  digits at rows 5-6, columns 21-26.
- **Ends:** any button (`game-flow.md` 7.3).

## 8. Planet surface [V]

- **Background:** the surface picture (`graphics.md` 4.4, 5.5), split at line 126 (`graphics.md` 1.1). A
  sprite-0 trigger (a hardware aid, not drawn by the remake) sits at y = 125 and an x that depends on the picture
  (8.1).
- **After docking:** the menu box (frame and options from `text.md` 4.6, layouts in `game-flow.md` 7.3) is drawn over
  the picture; its position and the cursor were verified for all four layouts by forcing the layout byte
  (RAM $A1) in Mesen and reading the nametable and sprite memory. **[V]**
  - **Box position:** nametable row 4 + r for the layout's row r (r = 0 is the top edge), columns 10-20 (11 tiles,
    88 px wide; the box's twelfth column, 21, stays blank). Row counts and where the text sits: layout 0: 9 rows
    (4-12; EXPLORE row 6, EQUIP row 8, POD row 9); layout 1: 12 rows (4-15; EXPLORE 6, EQUIP 8, POD 9, BLAST 11,
    OFF 12); layout 2: 6 rows (4-9; EXPLORE 6); layout 3: 9 rows (4-12; EXPLORE 6, BLAST 8, OFF 9). The last row of
    each is the blank row under the box. The height therefore **does** change with the option count. The box is
    redrawn one row per frame (cycling), so a forced layout change shows rows of both layouts for up to 12 frames.
  - **Text:** tile = character code + $90, frame tiles code + $BF (`text.md` 4.6).
  - **Colours (attributes):** the box is drawn in **background palette 3** (entries 13-15 of the surface picture's
    palette). Each row that is drawn also writes the four attribute bytes of attribute columns 2-5 (screen columns
    8-23) for its row pair, setting the quadrants that cover columns 10-21 to palette 3 and leaving the quadrants of
    columns 8-9 and 22-23 as the picture had them (the new byte is the old byte with those quadrants replaced). The
    quadrants are 2 × 2 cells, so a row pair is changed when either of its rows is a row of the box proper; the
    blank row under the box counts only when it shares its pair with a box row (layout 0 and 3: rows 4-11 changed,
    row 12 not; layout 1: rows 4-15; layout 2: rows 4-9). **[V]** (attribute table read back for all four layouts on
    planets 0 and 4; planet 0's picture has non-uniform attributes there and shows the merge: e.g. $A5 becomes
    $B7.) The remake draws the box in palette 3 with the same footprint; the merge is automatic.
  - **Wipe:** during blast-off mode 4 each wiped row also rewrites the same four attribute bytes (attribute columns
    2-5 of its row pair) from the picture's own data, so the picture's attributes are **restored** (planet 4:
    $55 $55 $55 $55 for rows 4-11, $55 $15 $02 $15 for rows 12-15) as its rows come back, one row per pass in step
    with the tile row. **[V]** (blast-off chosen by cursor and A on layout 1, writes logged with their addresses.
    Forcing mode 4 by poking RAM $A4 leaves the row counter stale and wipes a wrong page; the choice must go through
    the menu.)
  - **Cursor:** two sprites, tile $A7, palette 0, at x = 90 and x = 154; sprite-memory y = 49, 65, 89 for options
    1, 2, 3 (screen lines 50, 66, 90; the option's text row top is line 48, 64 or 88, so the cursor sits 2 lines
    below the row top). In layout 3 option 2 (BLAST OFF) uses y = 65. Up and Down (newly pressed) move it by one
    option within 2 frames; holding Down for 60 frames moved it exactly once: **no auto-repeat**. **[V]** The
    behaviour at the ends (wrap or stop) is in `text.md` 4.6 **[C]**.
  - First arrival (mode 2): layout 0, 9 rows, cursor on option 2 (y = 65). After docking: cursor on option 1
    (y = 49) and the layout of `game-flow.md` 7.3. **[V]** for the dock without an owned item (layout 2).
- **By mode** (the surface modes of `game-flow.md` 7.3, RAM $A4) [V]:
  - 0 (descent): the mothership flies in (8.2); the menu box is not drawn.
  - 2 (menu after the first arrival, only when something is owned or fitted): the picture with the mothership at
    its resting place, the menu box with EXPLORE and EQUIP POD, the cursor on EQUIP POD.
  - 3 (menu after docking): the same picture and box with the layout of `game-flow.md` 7.3, the cursor on the first
    option. The mothership is drawn at its resting place and does not move. Mode 3 is skipped entirely (no menu,
    mode 1 at once) when the planet's flag is set and nothing collectable is left on it (`game-flow.md` 7.3).
  - 4 (start of the blast-off): the picture is redrawn row by row, erasing the menu box (8.3), then mode 1.
  - 1 (leaving): the mothership flies off the top of the screen (8.3).
- **Music** (section 15): song 3 starts when mode 0 starts; modes 2 and 3 change nothing; song 5 starts when mode 1
  starts.
- **Ends:** `game-flow.md` 7.3.

### 8.1 The mothership's objects and the per-picture data

The scene draws three objects through the sprite layouts of table B (`objects.md` 6); object positions here are
screen pixels (there is no camera). **Tile numbers:** the scene sets the first frame range of table B to
[$80, $9F) with **tile base $A8** (RAM $05EF = $80, $05F0 = $9F, $05F7 = $05F8 = $A8, `objects.md` 5.2), so the layouts
$80-$9E (the ship, and the exhaust frames below $9F) show their layout tile + $A8; frames outside the range use
absolute tile numbers (the pod's frame $9F, tile $A6; the picture-6 ship $AB and its exhaust $AC-$AE, tiles $A8 and
$AB). The other ranges are left as the level before had them and catch no frame of this scene. **[V]** (OAM read back:
frame $85 gives tiles $C8-$CA and $C7, layout tiles $20, $21, $22 and $1F plus $A8). The ship's x speed points left, so
the ship is drawn mirrored (`objects.md` 6.3), at X - dx **+ 1** per sprite (`objects.md` 6.2: the +1 is
there for mirrored and unmirrored layouts alike).

- **Ship** (object 11): layout number (frame) F depends on the picture;
- **Exhaust** (object 12): at the ship's position, frame E (below), visible only while it is under power;
- **Pod** (object 13): frame $9F, a single sprite (tile $A6, palette 2).

The picture number P (RAM $05FF, 0-6) of each planet: **planet 0-12 = pictures 4, 6, 5, 0, 1, 2, 4, 1, 2, 5, 2, 0,
0** [V]. Per-picture constants are seven-byte tables in bank 0 (index P; picture 3 is empty and unused):

| Table (ROM) | Meaning | P = 0 | 1 | 2 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|---|
| $8AF9 (0x00AF9) | x of the sprite-0 trigger | 254 | 253 | 181 | 254 | 76 | 248 |
| $8B00 (0x00B00) | resting x (tx) | 83 | 87 | 153 | 23 | 162 | 51 |
| $8B07 (0x00B07) | resting y (ty) | 121 | 156 | 117 | 96 | 88 | 125 |
| $8B0E (0x00B0E) | start y | 57 | 0 | 61 | 48 | 0 | 0 |
| $8B15 (0x00B15) | ship layout F | $80 | $9A | $85 | $85 | $85 | $AB |

Picture 1 also has six fixed sprites, tiles $A8, $A9, $AA, $AB, $AC, $AC, palette 1, behind the background, at
(x, y) = (84, 136), (92, 136), (84, 144), (92, 152), (84, 160), (92, 160) (sprite-memory y), present in all modes.
The ship's sprites (all object sprites, from sprite-memory slot 32 up) get the "behind background" attribute
by a per-picture rule: picture 0: behind while the ship's x is not tx; picture 4: behind while x = tx or x ≥ 216;
other pictures: never. **[V]**

### 8.2 Descent (mode 0)

Pass n = 1, 2, ... counts from the first pass of the surface scene (the frame counter RAM $5D is 0 at the start and
rises by one per pass). The ship starts at (255, start y) with x speed 0.75 px per pass (8.8 value $00C0, moving
left), y speed 0 (picture 1: 0.5 px per pass, $0080, moving down). Positions and speeds are 16.8 / 8.8 numbers; the
y direction is down. Each pass:

1. If the ship's x is not yet tx: take d = (x − tx) as an 8-bit value, then x −= x speed. If d is below 37 (the
   last 37 px before the move): x speed −= 3/256 when the frame counter's low four bits are 0, else 2/256 (not
   below 0); if x speed is now 0, x := tx (the fraction is kept); then y speed += 1/256 (at most 3 px per pass) and
   y += y speed. In every downward move, if y ends below 16 it is set to 16 and the y speed to 0 **with the y fraction kept**
   (pictures 1, 5 and 6 start at y = 0, with only the bottom sprite row on screen, and first move here: picture 1's
   first move adds speed $0081 to y 0.00, so y becomes 16 with fraction $81, speed 0; the next pass's speed is 1/256
   and the fraction goes on from $81). **[V]** (y, fraction and speed logged on planet 4.) The initial y speed
   ($0080 of picture 1) is therefore never used.
2. Else (x = tx): if y = ty the ship has **arrived** (3); else, if y is 1 to 14 pixels above ty, y speed −= 1/256
   (stopping at 0); then y += y speed (the same line-16 rule).
3. Arrival: the ship stops. If nothing is owned or fitted (`game-flow.md` 7.3), the pod object is created at
   (x + 7, y) (x + 4 for picture 6) with y speed 0.25 px per pass (picture 0) or 0.125 (others), moving up, its
   own counter 0; it rises while its counter counts up each pass (0 in the arrival pass itself, 1 in the next); when the counter
   reaches 128 the descent ends: **128 passes after the arrival pass** (the earlier "127" was one short). If something
   is owned or fitted there is no pod and the descent ends **one pass before the arrival pass**: the end test is made
   after the mover in the same pass and sees y = ty in the pass whose move put it there, one pass before the mover's
   own arrival test fires. **[V]** (planets 0 and 4, with and without RAM $0600 = 1: ends at pass 595 and 466, 757 and
   628.) Picture 1: the ship object is switched off at arrival (it is hidden behind the scenery).
4. **A newly pressed in any pass** ends the descent at once: the ship is put at (tx, ty), the exhaust is switched
   off, and the scene continues as after a normal end (launch scene, or the planet menu in mode 2). **[V]** (A at
   100 and at 300 frames after the landing button: the launch scene started within a few frames.) Timing: the press
   made in frame f is read at the end of the pass of frame f and acted on in pass f + 1. With nothing owned the launch
   loading starts in that pass. With something owned the pass sets mode 2 and moves the ship; the cursor's two sprites
   appear in the next pass and the first row of the menu box (the top edge) in the one after, so the menu starts to
   show **3 passes after the press** (frames 1801, 1802 and 1803 for a press in frame 1800); the box is then built one
   row per pass as described above. **[V]**

Exhaust (object 12) is switched on in every pass of step 2 that does not arrive (the ship is over its x and moving
down) and off otherwise. In the descent it is placed at the ship's position **before this pass's move**, so it shows
the ship's position of the previous pass: its y lags the ship's by one pass's movement (x is constant while it is
on). **[V]** (e.g. ship y $23 and exhaust y $22 in the same frame). While leaving (8.3) it is placed after the move
and does not lag. **[V]** Quirk Q36. Its layout E, with b = bit 2 of the frame counter and F the ship's layout:

| Situation | E |
|---|---|
| y more than 14 px above ty | F + 1 + b |
| within 14 px (the braking part), F below $89 | F + 3 + b |
| within 14 px, F = $9A | F + b |
| within 14 px, F = $AB | $AE when b = 1, else $FF (invisible) |

Arrival pass (n), tx, ty and the totals (**[V]**: the model reproduces x and y of every pass for all thirteen
planets; the descent in `game-flow.md` 7.3 is arrival + 127 passes plus the 35-39 loading frames):

| Picture | Used by planets | Braking of x starts (pass) | x reaches tx | Arrival pass | Descent ends, nothing owned (pass) | Descent ends, something owned (pass) |
|---|---|---|---|---|---|---|
| 0 | 3, 11, 12 | 181 | 270 | 433 | 561 | 432 |
| 1 | 4, 7 | 175 | 268 | 629 | 757 | 628 |
| 2 | 5, 8, 10 | 87 | 180 | 305 | 433 | 304 |
| 4 | 0, 6 | 261 | 350 | 467 | 595 | 466 |
| 5 | 2, 9 | 75 | 168 | 341 | 469 | 340 |
| 6 | 1 | 223 | 316 | 592 | 720 | 591 |

(Ends: arrival + 128 and arrival - 1; planets 0 and 4 read in Mesen, the rest follow from the verified arrival
passes. The earlier column, arrival + 127, was one pass short.)

Sample positions (ship x, y after pass n; picture 4: n = 1: (254, 48); 100: (180, 48); 200: (105, 48); 300:
(35, 51); 400: (23, 81)); picture 0: 1: (254, 57); 100: (180, 57); 200: (106, 57); 300: (83, 83); 400: (83, 115).
With something owned the descent ends one pass before the arrival pass (picture 4: pass 466, [V]).

### 8.3 Blast-off (modes 4 and 1)

- **Mode 4 (15 passes).** Chosen BLAST OFF: the sprites other than the ship's (OAM 1-31) are hidden and a counter
  starts at $0080 and rises by $20 each pass; the counter is the nametable address (offset from $2000) of the
  next row to redraw. So the picture's rows 4, 5, ..., 18 are rewritten **one per pass, top to bottom**, from the
  picture's own nametable, which erases the menu box (rows 4-15, columns 10-21); when the counter reaches $0260
  (15 passes) mode 1 starts and **song 5** starts. The counter's 480 steps are address units, not frames
  (480 / 32 = 15 passes). **[V]** (rows changed one per frame, in order, in the NT record).
- **Mode 1 (leaving).** The ship is at (tx, ty) (reset there when the menu was entered, so with y speed 0 except
  picture 1, which has 0.5 px per pass), the y direction is up and the x speed is not used. Each pass: on those
  passes where the frame counter's low three bits are 0 (every 8th) the y speed rises by 1/256 (at most 3); then
  y −= y speed. The exhaust (object 12) is on all the time, layout F + 1 + bit 1 of the frame counter. The mode
  ends in the first pass in which y has gone below 0 (the high byte negative); then $3B + 1 (from 12: 10 + 1),
  START LEVEL (`game-flow.md` 7.3). **[V]**
  Passes in mode 1 from rest: picture 0: 704, picture 1 (0.5 px start): 275, picture 2: 692, picture 4: 627,
  picture 5: 600, picture 6: 715 (the model matched the position of every pass; the phase of "every 8th pass"
  moves a count by up to 7).
- **From choosing BLAST OFF to the start of loading** = 15 + (passes in mode 1) + 1, so by planet 0-12:
  643, 731, 616, 720, 291, 708, 643, 291, 708, 616, 708, 720, 720 (measured for planets 0, 1, 2, 3, 7, 8).
  The values in `game-flow.md` 7.3 that were measured by poking the menu state (468, 505, ...) left the ship its
  speed from the descent and are not the real path.

## 9. Launch scene [V]

The inside of the mothership: the pilot walks along a gantry to the pod, climbs in, the hatch closes and the pod
flies up and out through the top of the screen. Recorded frame by frame (planet 0, default pod; sprites, objects and
the pod's height in all 358 frames; the pod's motion below is a model that matched every frame). Re-checked against
the code and a fresh Mesen run 2026-10-17 (points marked below).

- **Background:** the launch picture (`graphics.md` 4.3, 5.6), scroll (0, 0), no split. Palette preset 27
  (`graphics.md` 7.2, [C]); sprite tiles from the launch sprite set (`graphics.md` 5.6: $0000-$08FF; the pod's
  tiles by pod type). Fuel and weapon energy are filled at the start (`economy.md` 3.2).
- **Music:** none of its own (section 15).
- **Left-column mask:** $18 all frame, so sprites with x < 8 are hidden in this scene too (`graphics.md` 1.3) **[V]**.
- **Objects.** Positions are screen pixels; each object is drawn with a layout of table B (`objects.md` 6, frames
  $C1-$D9, absolute tile numbers of the launch sprite set). Pass n = 0, 1, 2, ... is the n-th pass of the scene
  (one per frame; the frame counter RAM $5D, which keeps its value from the previous scene, rises by one per pass):

  | Object | Role | At the start |
  |---|---|---|
  | 14 | the pod's body, frame $C1 | (67, 50) in RAM; **drawn as if x = 68**: the leftmost sprite of the layout (dx = −12) is at screen x 56, not 55, so every sprite of the pod, hatch and flame sits 1 px right of x + dx **[V]**, OAM x = 56, 64, 72, 80 in all frames; camera is 0, the +1 comes from the drawing step |
  | 13 | the hatch / canopy, frames $C2-$C5, always at the pod's position | (67, 50), frame $C2 |
  | 12 | the pod's flame, frames $C6-$CA | **fixed at (67, 34) for the whole scene** (set up once and never moved, unlike the hatch; the pod's rise does not carry it, only its frame changes) **[V]** (RAM y = 34 at pass 200 while the pod's y is 45); switched on in pass 131, so first drawn in pass 132 |
  | 11 | the walking pilot, frames $CB-$D9 | not shown until pass 5 |

- **Phase 1, the pilot walks (passes 5-131).** A step counter Y = 0, 1, ..., 20 advances every **6 passes**. At
  pass 5 + 6 × Y the pilot takes the position and frame of step Y from three 21-byte tables in bank 0 (x at ROM
  0x052E2, y at 0x052F7, frame at 0x0530C), and keeps them for 6 passes:

  | Y | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
  | x | 143 | 147 | 143 | 139 | 135 | 131 | 127 | 123 | 119 | 115 | 111 | 107 | 103 | 98 | 68 |
  | y | 63 | 63 | 63 | 63 | 63 | 63 | 63 | 63 | 63 | 63 | 63 | 63 | 58 | 58 | 36 |
  | frame | $CB | $CC | $CD | $CE | $CF | $D0 | $D1 | $D2 | $CD | $CE | $CF | $D0 | $D1 | $D2 | $D3 |

  Y = 15-20: x = 64, y = 40, frames $D4, $D5, $D6, $D7, $D8, $D9. So he walks left along the gantry at 4 px per
  step (two short back-and-forth frames at the start), steps down at Y = 12-14, then stands at (64, 40) in front of
  the pod while the frames $D4-$D9 animate (climbing in). At Y = 18, 19, 20 (passes 113, 119, 125) the hatch
  advances $C2 → $C3 → $C4 → $C5 (closing; its sprite count falls from 22 to 14 at pass 114). At Y = 21 (pass 131)
  the pilot is removed and phase 2 starts.
- **Phase 2, the pod flies up (passes 131-357).** The pod's mover is first called in **pass 131**, the pass in which
  the pilot is removed **[C]**, but its y speed is still 0 there (the speed is raised only on passes with $5D odd,
  and $5D is even in pass 131), so the first change of y is in pass 132 and nothing differs from the description
  below. From pass 132 the pod body moves up (the hatch is moved to the
  same position every pass): on every second pass (those with $5D odd) its y speed (8.8) rises by 1/256, and every
  pass y −= y speed. So after pass n (n ≥ 132) the speed is (floor((n − 132) / 2) + 1) / 256 and y = 50 minus the
  sum of the speeds so far; y after pass 132 is 49 (49.996), after 225 it is 41, after 311 it is 18, after 345 it is 4. The flame object appears at pass 132 and shows $C6 + bit 2 of the frame counter (alternating every 4 frames)
  while the pod's y is above 9, and $C6 + (9 − y) / 2 (integer) once y ≤ 9. The code ends the scene in the **first pass in
  which the pod's y (16 bits plus fraction) goes below 0: pass 357, 358 passes in all** **[V]** (frames recorded).
  Rendering is switched off at once in that pass (about line 111 of its frame). **The picture stops changing earlier:**
  the whole part of y first reaches 0 after pass 355 and the sprites stay at OAM y = 0 for passes 356 and 357, so a
  remake that ends the scene when the whole part of y reaches 0 (355 passes of change) differs from the original
  only by two repeated frames, which is a legitimate equivalent **[V]**.
- **Button.** Any newly pressed button ends the scene: B pressed in pass 50 ended it within 1-2 frames. **[V]**
  Loading and the level start follow (`game-flow.md` 7.3).

## 10. Message screens [V]

Shown after an item is pulled into the mothership (transition 1, `game-flow.md` 7.4) and, as kind 6, when the
wormhole's gate ends in a warp-level introduction (10.1). **For the delivery messages (kinds 1-5) and the final message
(kind 7) the gameplay status panel stays and sprite 0 keeps its split trigger; the kind-6 screen has neither** (10.1,
screenshot 2026-10-17). The strings and their positions are in `text.md` 3, the item-kind-to-message map, timers and exits in
`game-flow.md` 7.4. This section gives what the screen is made of and the picture of each message. The rows and
columns are nametable rows and columns of page 0; sprites are as in section 0. The screen is not "cleared to black":
a frame picture is drawn first.

**Frame and colours.** Two frame screens (`graphics.md` 5.9), chosen by message **[V]** for the palette, **[C]** for
the screen:

| Frame | Screen | Palette | Messages |
|---|---|---|---|
| plain box, rows 4-21 | menu screen B, nametable at $2080 | preset 24 (`graphics.md` 7.2) | equipment (ids 6, 7, 8, 9), fuel (21, 22), crystals (25, 50) |
| box in a different colour | menu screen B, plus the warpship picture (below) | entries 8-15 as in preset 6's data (ROM 0x075A7) | warpship piece (47, 48) **[V]** |
| decorated frame, full page | menu screen C, nametable at $2000 | preset 25 | alien items (45), radioactives (46) |
| decorated frame, blue | menu screen D (the shop frame, `graphics.md` 5.9), not C | preset 26 (entries 13-15), entries 9-10 from a pair table by list, entry 11 = entry 26 (`graphics.md` 7.4) | analysis (27-43) **[V]** |

**Pictures.** Each message has one of three kinds of picture. The tile numbers below are nametable tile numbers; a
number of $81 and above is in the background pattern table at $1000.

1. **Tile pictures** (shields, boosters and navigators: item kinds 3, 4 and 6; printer ids 6/7, 8 and 9). The picture
   is part of the message data (`text.md` 3, "picture"): it is written at once, the text types afterwards. Item
   picture `p` (kind 3 uses 0, kind 4 uses 1, kind 6 uses 2: the map of `graphics.md` 5.7; ROM 0x2F514, 0x2F6A8,
   0x2F328) must be loaded into table 1 from tile $81 first (`graphics.md` 5.7: tile `k` of the stream, counting
   from 0, is tile $81 + `k`).
   Its colours are background palette 0 (the attribute bytes of these cells are 0). The held item itself is not
   drawn (it was removed). All cells not listed stay blank (tile 0). **[V]** (nametable read back for all three).

   | Cell | Shields (28 tiles) | Boosters (32 tiles) | Navigators (34 tiles) |
   |---|---|---|---|
   | row 8 | | col 15: $81-$82 | col 15: $81-$82 |
   | row 9 | col 14: $81-$84 | col 14: $83-$86 | col 14: $83-$86 |
   | row 10 | col 12: $85-$87; col 17: $88-$8A | col 14: $87-$8A | col 14: $87-$8A |
   | row 11 | col 12: $8B-$92 | col 14: $8B-$8E | col 12: $8B-$92 |
   | row 12 | col 13: $93-$98 | col 13: $8F-$94 | col 12: $93-$9A |
   | row 13 | col 14: $99-$9C | col 13: $95-$9A | col 14: $9B-$9E |
   | row 14 | | col 13: $9B-$A0 | col 15: $9F-$A0 |
   | row 15 | | | col 15: $A1-$A2 |

   (Each entry is the first column and the run of consecutive tile numbers written from there.) Text rows: shields
   15-17, boosters 15-17, navigators 16-17.

2. **The item's own sprites** (alien items, fuel, radioactives, crystals: item kinds 5, 10, 12, 15, 16). The delivered
   item's world sprite frame (`objects.md` 6.5) is drawn once at a fixed place; the item keeps its normal sprite
   palette. The sprites of each (x, y of the top-left pixel, tile of sprite table 0, attribute: palette, H = flipped
   horizontally, V = flipped vertically) **[V]**:

   | Item (frame) | Sprites |
   |---|---|
   | 5 alien items ($58) | (127, 33) $00 pal 0; (134, 33) $00 pal 0 H; (127, 41) $01 pal 1; (134, 41) $01 pal 1 H |
   | 12 radioactives ($61) | (127, 33) $D6 pal 1; (135, 33) $D6 pal 1 H; (127, 41) $D7 pal 1; (135, 41) $D7 pal 1 H |
   | 15 crystal ($65) | (119, 81) $DB pal 1; (127, 81) $DB pal 1 H; (119, 89) $DC pal 1; (127, 89) $DC pal 1 H |
   | 16 crystal ($66) | the same positions and tiles as kind 15, palette 0 |
   | 10 fuel ($5C) | all palette 3: (119, 69) $89; (127, 69) $89; (111, 73) $88; (135, 73) $88 H; (119, 77) $8A; (127, 77) $8B; (111, 81) $88 V; (135, 81) $88 H V; (119, 85) $89 V; (127, 85) $89 V |

   Where the picture sits relative to the text: the alien-items and radioactives pictures are above the text of
   messages 45 and 46 (lines at rows 8-17); the crystal picture is between "STORING CRYSTAL" (row 8) and "VALUE 500"
   (row 13) of message 25 (message 50, shown for the seventh crystal, was not watched; its picture is probably the same **[H]**); the fuel picture is above message 21 or 22. The sprite coordinates
   above are final screen positions; they need no further offset.

3. **The analysis container** (item kind 9, printer ids 27 and 28-43). Set-up (code read, **[V]** by Mesen): palette
   preset 26, menu screen D, the equipment-block loader with the analysis list k (`graphics.md` 5.8: the picture area
   of the frame, rows 3-7, columns 13-20, shows block k + 1), then the sprite tiles 0x35394 → $0800. The two palette
   entries 9 and 10 and the copy into entry 11 are in `graphics.md` 7.4. The container is three objects in sprite
   palette 2: slot 11 at screen (131, 18) and slots 12 and 13 (the two halves, slot 13 mirrored) at (100, 37) and
   (162, 37), drawn through table B (`objects.md` 6) with the +1 of `objects.md` 6.2 already counted in the sprite
   coordinates below. **Six stages**, not one: the objects start as frames $93 (slot 11) and $99 (slots 12 and 13);
   an internal counter rises by one every second pass from the second pass on, and from the stage change at
   $5D = 98 (the value after the pass's own increment; 96 passes after the first pass of the screen) all three
   frames fall by one every 8 passes: slot 11 $93, $92, $91, $90, $8F, $8E; slots 12 and 13 $99, $98, $97, $96,
   $95, $94, the last at $5D = 130, where they stay. The picture is the container opening: the layouts have 12, 12, 8, 8, 4, 8 sprites
   (slot 11, frames $93 to $8E) and 11, 10, 10, 7, 4, 7 (slots 12 and 13, frames $99 to $94); stage 0 has the two
   halves overlapping (tiles $C1-$CB from x = 109 and from x = 131 to 155) and the final stage has them at x = 93-101
   and 163-171 with eight sprites in the middle (tiles $80-$85 at y = 22, $80 and $81 at y = 14). Every stage's
   sprites come from the layouts of table B (`objects.md` 6) at the three positions above. The table below is
   **stage 1** (frame $92, $5D = 98 to 105), as recorded earlier. **[V]** (frames and counter read every pass; OAM of
   stages 0, 1 and 5.) Stage 1:

   | y | left half (x, tile) | centre (x, tile) | right half (x, tile, H) |
   |---|---|---|---|
   | 22 | | 120 $90; 128 $91; 136 $91 H; 144 $90 H | |
   | 30 | 108 $B7; 116 $B8 | 120 $92; 128 $93; 136 $93 H; 144 $92 H | 148 $B8; 156 $B7 |
   | 38 | 108 $B9; 116 $BA | 120 $94; 128 $95; 136 $95 H; 144 $94 H | 148 $BA; 156 $B9 |
   | 46 | 108 $BB; 116 $BC; 124 $BD | | 140 $BD; 148 $BC; 156 $BB |
   | 54 | 108 $BE; 116 $BF; 124 $C0 | | 140 $C0; 148 $BF; 156 $BE |

   The frame's text (ANALYSIS at row 2, VALUE at row 18, the three element names at rows 10, 12 and 14 and their
   amounts) is in `text.md` 4.4.

4. **Warpship pieces** (item kind 19, ids 47 and 48, kind 5) **[V]**. Menu screen B's box (preset 24 for its frame
   colours, then preset 6 over entries 8-15) and the picture of the **golden warpship being assembled**, made of
   background tiles $80-$A5 of table 1 (the stream 0x12DA5 → $1800 with its 22 attribute bytes
   0x12FE7 → $23D1-$23E6, `graphics.md` 4.5). The twelve pieces are 2 × 2 cells each; piece p (0-11) lies at the nametable cell (row,
   column) and has the tiles (top-left, top-right, bottom-left, bottom-right):

   | p | row, column | tiles | p | row, column | tiles |
   |---|---|---|---|---|---|
   | 0 | 8, 10 | $81 $82 $87 $88 | 6 | 10, 12 | $91 $92 $9B $9B |
   | 1 | 10, 14 | $93 $94 $9B $9B | 7 | 8, 12 | $83 $84 $89 $8A |
   | 2 | 8, 16 | $80 $80 $8D $8E | 8 | 10, 16 | $95 $96 $9C $9D |
   | 3 | 12, 12 | $A0 $A1 $80 $80 | 9 | 8, 14 | $85 $86 $8B $8C |
   | 4 | 10, 18 | $97 $98 $9E $9F | 10 | 10, 10 | $8F $90 $99 $9A |
   | 5 | 12, 16 | $A4 $A5 $80 $80 | 11 | 12, 14 | $A2 $A3 $80 $80 |

   (Table at bank 2 $AD69 for the cells' addresses and $AD75 for the tiles, ROM 0x12D69 and 0x12D75.) At the start the
   pieces already collected are drawn: piece p is collected when bit 7 - p of RAM $DE (p below 8) or bit 15 - p of RAM
   $DF (p 8-11) is set (the planet flags, `economy.md` 5.1). The piece just delivered (piece = level index $3B - 13, with 12 taken as 10) is **not**
   in the first picture: it appears when the typing ends. The program writes it as two cell pairs on two successive frames (the lower pair first,
   then the upper pair), i.e. the **four cells of that piece's 2 × 2 block** change at once to the visible eye. In the
   case watched the last letter of message 47 went into the nametable 5 frames before the lower pair (the printer's
   finished flag, RAM $0597 = $FF, is what triggers it), and the upper pair came one frame after the lower. For piece 4
   (level index 17) these are the cells at rows 10-11, columns 18-19, the right edge of the picture; for piece 1,
   columns 14-15 in the middle. The cells change only once. **[V]**

- **Ends:** `game-flow.md` 7.4 (timer `$DA` or a button).

### 10.1 Kind 6: the warp-level screen [V]

Shown when the wormhole's gate state ends on a level 13 or above with the planet's flag set (`game-flow.md` 7.4). It
is not a delivery and prints no text.

- **Background:** one full-screen tile picture on page 0, from the screen of bank 6 entry $8012 (`graphics.md`
  4.5: nametable at 0x34296, tiles at 0x344A4), palette preset 21 ($15) (`graphics.md` 7.2). The picture is a night
  sky with a bright comet fan above a blue mountain-shaped mass. There is no status panel and no sprite-0 split.
- **Lettering (sprites):** the words are **not** part of the background. They are 30 sprites, the layout of the
  object in slot 11 (frame $9E, at x = 128, y = 160 as a position), with the tiles $00-$1D of table 0 (stream 0x3512D,
  loaded at $0000: `graphics.md` 4.5). The title word **CYBERZONE** (white), then the lines "COLLECT", "10", "GEMS",
  "BEFORE TIME", "CATCHES UP" (yellow with a red shadow) **[V]** (screenshot, 2026-10-17). **The table below is the whole truth**: one row
  per word in that order (title first); the tile counts are the lengths of the tile strips, not of the words (the letters are
  narrow: about 1.5 letters per 8-pixel tile, "10" fits one tile). Sprites (x of the first sprite, y, tiles in order, 8 pixels apart;
  the attribute is the palette) **[V]**:

  | y | x | Tiles | Palette |
  |---|---|---|---|
  | 137 | 105 | $00-$06 (7) | 0 |
  | 153 | 113 | $07-$0B (5) | 1 |
  | 161 | 129 | $0C (1) | 1 |
  | 169 | 121 | $0D-$0F (3) | 1 |
  | 177 | 105 | $10-$16 (7) | 1 |
  | 185 | 105 | $17-$1D (7) | 1 |

  Sprite 0 (the split trigger of the other screens, tile $FF at x = 8, y = 192, behind) is also in sprite memory and
  is not drawn by the remake.
- **Motion:** the background's vertical scroll alternates between two values 2 pixels apart on every frame. The
  lettering does **not** move (its sprite positions are the same on every frame). **[V]**
- **Length in the remake:** always 256 frames (`docs/deviations.md`, "Warp-level intro length"); the rest of this bullet is the
  original's rule, which depends on the gameplay frame counter.
- **Ends:** after the timer, 256 frames from the first frame of the handler in the case watched (entry `$5D` = 166).
  The timer `$DA` starts at 192 and counts down by one on each frame after which `$5D` (advanced by the handler first)
  is 64 or more, so 192 counted frames and 64 idle ones (`$5D` from 0 to 63) make 256 frames in all when the screen
  starts with `$5D` of 64 or more; if it starts with `$5D` = `s` below 64, the screen lasts 255 - `s` frames. A newly
  pressed button ends it once `$DA` is below 64 **[C]**. Then the warp level starts (`game-flow.md` 7.4, code 3).
  **[V]** for the 256 frames (`$DA` 192 at the first frame, 103 after the first 90 frames, 0 at the last) and the
  decrement rule (`$DA` = 103 constant while `$5D` runs 0-63); the earlier figure of 254 frames was a measuring
  slip.

## 11. Backdrop interlude and badge intermission [V]

The two are chained: the interlude (transition code 7) ends by requesting the intermission (code 6 with `$DB` = 1),
which returns to the level (`game-flow.md` 7.4). The star marker that starts the interlude is in `economy.md` 5.2 and
5.4; the badge intermission is also entered alone from the warp levels (`economy.md` 5.4).

### 11.1 Backdrop interlude (code 7) [V] unless marked

A bonus flight: the pod climbs for about 34 seconds through a field of falling streaks, steered left and right,
collecting gems and dodging rocks (and, on two of the three backdrops, mines). It lasts 2,054 frames from the request
to the hand-over (about 6 load frames, then 2,048 steps of one frame), whatever the player does. **[V]** (levels 0, 4
and 8, so all three backdrops; the same count with steering, with a mine hit and with the pod's starting y changed).
All three backdrops were run through in Mesen on 2026-10-17: every spawn (33 per run) equals the ROM bytes at the
entry computed in 11.1.2, the palette is the one below, a mine killed the pod on backdrop 2 as on 1, and every drawn
cell of the backdrop rows (11.1.1) matched the table.

**Set-up** (about 6 frames, screen blanked, the default handler): the pod object (slot 0) is saved for the return
(`game-flow.md` 7.4) and then put in player state 10 (this scene's state), x = 120, vertical and horizontal speeds
0, direction "moving up", while its y is kept from the planet; the crystal count `$67` is cleared; the camera x is 0
and the camera y is the pod's y - 160; the vertical scroll starts at 0; the object limits are set to a margin of 128;
the backdrop screen of bank 7 (`graphics.md` 4.6: tiles at ROM 0x3A430 and 0x3A88D) is loaded, with palettes from
`graphics.md` 7.4: background palette 3 (entries 12-15) from the 4-byte group `k` of the table at ROM 0x3833A
(`k` below), sprite palettes 1-3 (entries 20-31) from ROM 0x3832E-0x38339. **Sprite palette 0 (entries 16-19) is not
loaded**: the table's first group at ROM 0x3832A (0F 30 21 02) is never used, and the entries keep what the planet
had (0F 30 22 12 in every case seen). Background palettes 0-2 (entries 0-11, which the status panel uses) are kept
too. **[V]** (Mesen: palette RAM read back on all three backdrops; shadow entries 16-19 poked to 0F 16 26 36 during
the set-up came out unchanged); song 31 starts (`sound.md`, loops) **[C]**; the playfield nametable is
empty (tile 0) and the gameplay status panel is shown with its split at line 192, unchanged (fuel, weapon, pods,
score as they were). The backdrop number `k` is the position of the level index in the list 0, 4, 8: `k` = 0, 1, 2 for
planets 0, 4, 8 (`graphics.md` 7.4: ROM 0x380F8). **[C]**

**Each frame, in this order** (one step of the scene per frame, all of `game-flow.md` 1.3 applies; the order was
read from a log of the entry of each part, 2026-10-17 **[V]**):

1. Advance the random generator (`game-flow.md` 5, at the start of the step).
2. **Move the pod up** by its vertical speed `v` (8.8 pixels per step, always upward), which sets the camera y = pod y
   - 160; the pod therefore stays at screen line 160, the world scrolls down. y is 16-bit and wraps. Scroll y = the
   previous scroll y minus the whole pixels moved (the fraction carries), modulo 240 (0 goes to 239). When the new
   scroll y and the old one are in different 8-line rows (scroll AND $F8), **one** new backdrop row is written
   (11.1.1); at speeds above 8 pixels per step two boundaries can be crossed in one step, and only one row is
   written (the skipped row keeps its old contents). **[V]** The backdrop generator (the bars' random draws) runs
   inside this part, so it comes **first**, right after the random step, before steering and before any object
   logic, and it sees the new scroll.
3. **Steer the pod** (11.1.3): the pod's x and `h` change now.
4. **Spawn, touch and move the gems and rocks** (11.1.2), with the pod where parts 2 and 3 left it. (An earlier
   draft had this part before the steering; that was wrong.)
5. Draw (pod and objects), then **change `v`**: while the climb is accelerating `v` grows by 4/256 pixel per step; when
   its whole part reaches 16 the direction of change reverses and `v` shrinks by 4/256 per step; when `v` reaches 0
   the scene is over. So `v` = 4 `n` / 256 at step `n` (`n` from 0), peaks at 16.0 after 1,024 steps, and is 0 after
   2,048. The total climb is 16,388 pixels; the pod ends at its starting y - 16,388 (wrapping). **[V]**

#### 11.1.1 The backdrop rows [V] for rule and data, [C] for details

The backdrop is not a stored picture. The playfield is built row by row as the scroll moves. It has 15 vertical
*bars* in columns 1-30 (columns 0 and 31 stay blank); each bar is a streak of 4 rows. Bar `b` covers `w` columns from
column `c`:

| `b` | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `c` | 1 | 2 | 3 | 4 | 6 | 8 | 11 | 14 | 18 | 21 | 24 | 26 | 28 | 29 | 30 |
| `w` | 1 | 1 | 1 | 2 | 2 | 3 | 3 | 4 | 3 | 3 | 2 | 2 | 1 | 1 | 1 |

(`w` and `c` are in ROM at bank 7 $8413 and $8422.) Each bar has a phase 0-3, 0 meaning idle. Each time a row is
written, for each bar in turn:

- if the phase is 0, draw the random test: `$5A` = (`$5A` + $37) AND $FF (no carry in), and if (`$5A` AND $7F) is
  below 23 (about 18 % of the draws) the bar starts, drawing phase 0 in this row; otherwise the bar's columns stay
  blank (tile 0) in this row;
- if the phase is 1, 2 or 3, the bar draws that phase in this row;
- after drawing phase `p` the bar's phase becomes `p` + 1, or 0 after phase 3.

Drawing phase `p` in a row writes, for each column `col` of the bar, the tile number (byte at ROM 0x3A80D +
32 `p` + `col`) + 1. The table is 4 phases of 32 bytes (bank 7 $A80D). The row written is the one above the row
now at the top of the screen: tile row = (scroll y / 8, rounded down) - 1, 29 when that is -1. As the scroll runs
upward through the nametable the streaks therefore build up from phase 0 at the bottom to phase 3 at the top.
The attributes are those loaded with the screen. **[V]** (nametable rows compared with the table).

#### 11.1.2 Gems, rocks and mines [V] unless marked

The objects live in slots 11-14. **Spawning:** the index `i` = ((0 - camera y high byte) AND $FF) / 2, rounded down (0 to 127), evaluated every step
with this step's camera y; whenever it differs from the last value (the first step always counts), entry
`Y` = 32 `k` + `i` of the lists below is created (x = the byte at ROM 0x38033 + `Y`, type = the byte at ROM
0x38094 + `Y`; the bytes after the lists are whatever the ROM holds): the first free slot of 11-14 (if none is free
the original writes into slot 15, where the object is never processed, so the entry is lost **[V]**; the remake
may skip the spawn), x and type as read, y = camera y - 32 (just above the top edge), moving upward, state and frame
by type. The index goes up one per 512 pixels climbed, and where it starts depends on the camera's high byte at the
start, that is on the pod's y when it touched the star marker:

- **Planets 0 and 8** (markers at y = 59 and 48; the camera y is negative, high byte $FF): `i` runs 0 to 32, entry 32
  being reached only in the last 215 or so frames. Entry 32 reads past the backdrop's list: for backdrops 0 and 1
  it is the first entry of the next backdrop's list, for backdrop 2 it reads the bytes that follow the lists
  (x = the byte at ROM 0x38093, type 3, a mine). **[V]** (all spawns of both runs compared with the ROM bytes.)
- **Planet 4** (marker at y = 1,505, camera y about $0540, high byte 5, whatever the exact touch): `i` runs 125, 126,
  127, then wraps to 0 and goes on to 29 (33 spawns). Entries 157, 158 and 159 are bytes beyond the lists: x = 3, 3,
  3 and types 5, 232, 224 (ROM 0x380D0-0x380D2 and 0x38131-0x38133). Types other than 0 to 3 look like rocks and
  act like mines on touch **[C]**, but these three objects are **never drawn** (no sprite in sprite memory) and can
  **never be touched** (x = 3, the pod's smallest x is 16); they still fill slots 11-13 for several hundred steps.
  So the gem of entry 33 (`i` = 1, x = 64) finds no free slot (slot 14 holds the rock of `i` = 0) and is lost: planet
  4 offers at most 9 of its 10 gems. **[V]** (the object table logged through the whole run.) Quirk Q40.
- For any other start the same formula reads the bytes of ROM 0x38033 + 32 `k` + `i` and 0x38094 + 32 `k` + `i`
  (largest `Y` = 191).

Types: 0 = nothing (the slot stays free), 1 = gem, 2 = rock, 3 = mine, 4 = explosion (made only by a mine hit),
5-255 = rock look, mine effect **[C]**.

| Type | Is | Frames (`$393` counts steps from the spawn) |
|---|---|---|
| 1 | gem | $65 (static) |
| 2 | rock | $A1 + (count / 8 mod 4) |
| 3 | mine | $A5 + (count / 8), the count restarting at 0 after 48 |

x and type lists (ROM bank 7 $8033 and $8094, ROM 0x38033 and 0x38094; 32 entries per backdrop):

| Backdrop (level) | x, entries 0-31 | Types, entries 0-31 |
|---|---|---|
| 0 (planet 0) | 240 16 240 16 240 16 240 16 240 16 62 47 151 22 135 131 224 52 40 22 48 120 146 22 85 49 152 67 103 201 163 69 | 1 2 2 1 1 1 2 2 1 2 2 1 2 2 1 2 1 2 1 2 2 2 1 2 2 2 2 2 2 2 2 2 |
| 1 (planet 4) | 128 64 192 64 192 72 147 98 51 120 152 146 250 21 135 51 216 164 51 120 69 150 148 34 83 180 226 23 99 168 148 136 | 2 1 2 1 2 2 1 1 2 2 1 2 1 3 3 1 3 1 1 1 3 3 3 3 3 3 3 3 3 3 3 3 |
| 2 (planet 8) | 69 131 148 240 148 40 87 136 179 195 36 25 103 152 86 52 137 149 118 69 120 86 52 144 179 40 86 35 136 114 69 195 | 3 3 1 3 1 2 3 1 1 1 3 3 2 1 3 2 2 2 2 1 3 3 1 2 1 3 1 3 3 3 3 3 |

Each backdrop has exactly 10 gems in entries 0-31 (planet 4 loses one, above).

**Slot leftovers.** A spawn writes x (and clears its high byte), y (whole pixels), type and frame, but not the
object's count (the step counter of the table above) nor its y fraction, which keep the previous occupant's values;
the scene's set-up does not clear them either. So a rock or mine starts its animation at the phase the slot's last
occupant had reached (the count is incremented before the first frame) and a new object's y carries the old fraction.
**[V]** (slot values poked before the set-up appeared in the first spawn: count $55 went on, fraction $77 was kept.)
The remake may start both at 0 (the difference is the animation phase and less than 1 pixel of y): Q41.

**Movement:** each object moves upward at 3/4 of the pod's
vertical speed (`(v >> 1) + (v >> 2)` in 8.8), so it drifts down the screen at 1/4 of the pod's speed; it is removed
by the usual margin rule (`objects.md` 3.5, margin 128). **[C]** removal; the speed is **[V]**.

**Touch.** An object touches the pod when, for both axes, pod position - object position is from -12 to +11
(pixels), using the pod's position after this step's climb and steering and the object's position **before** this
step's move (an object that spawns in this step is also tested at its spawn position and then moved). **[V]**
(planet 0's first gem was touched with a difference of 11 in y before the move, which would have been 14 after it, and
12 one step earlier.) It is only tested while the pod is visible. Effects:

- **Gem:** the crystal count `$67` goes up by 1, the object disappears, a pick-up sound plays **[C]**.
- **Rock:** the pod's horizontal speed (whole part) is set to 3 pixels per step, in its current direction; the rock
  stays. Contact lasts several steps, so one rock moves the pod roughly 35 pixels. **[V]**
- **Mine:** the pod is destroyed (state 0, invisible); the mine becomes an explosion that follows the pod's y: frames
  9 to 15, 4 steps each, then it vanishes. The pod can no longer collect or steer; the scene runs on unchanged to its
  end. **[V]** (level 4, pod put in a mine's column).

#### 11.1.3 Steering [V]

The pod's horizontal control in this scene reads the held direction bits (`game-flow.md` 3.1). The horizontal speed `h`
(8.8, magnitude) has a direction bit "moving left"; **frame** values are the pod layouts of `objects.md` 6.4 (0 =
upright, 1 = banked right, $1F = banked left).

| Held | Direction `D` | Frame | Effect on `h` |
|---|---|---|---|
| neither | either | 0 | falls by $60/256 per step, to 0 (`D` stays) |
| Right | right | 1 | grows by $28/256 per step, at most 7.0 |
| Right | left | 1 | `h` := 0 and `D` := right at once (growth starts next step) |
| Left | left | $1F | grows by $28/256 per step, at most 7.0 |
| Left | right | 1 | falls by $40/256 and then grows by $28/256 (net -$18); if the fall of $40 reaches 0 (`h` was at most $40), `h` := 0, `D` := left, frame $1F, and no growth in that step |

If both Left and Right are held, Left is used. `D` is the original's "moving left" bit; with `h` = 0 it keeps its last value.
Then the pod moves by `h` in direction `D`. x is
clamped: below 16 it becomes 16, at 240 or more (exactly 240 included) it becomes 240, and in both cases `h` := 0
(the x fraction is kept, so the pod stays at the limit). **[V]** (right wall: x went from 239.9 to 240.7 and was put
on 240 with `h` = 0, and stayed.) Up, Down and the buttons do nothing **[C]**. The asymmetry
between Right and Left in the "moving the other way" rows is how the original works. **[V]** (speeds sampled each
step with Left and Right held; both reversals, the wall and the decay checked).

**Hand-over:** when `v` reaches 0 the scene sets `$DB` = 1 and requests transition 6; the badge intermission follows,
then the level restarts from the saved position (`game-flow.md` 7.4). The crystal count at that moment is what the
intermission counts. **[V]** (with `$67` = 11).

### 11.2 Badge intermission (code 6) [V]

- **Background:** the nametable is empty (black) except for the crystal pictures below; the status panel stays.
- **Badge:** the 62 sprites of 2.4, with the same row layout and tiles and palettes, but 48 lines higher than in the
  title: the first row at y = 21 (instead of 69), x = 109. They rise from below the screen at 2 lines per frame
  (first row y = 233 at the first frame of the handler, 225 four frames later), landing after about 103-106
  frames. **[V]** Palette: preset 20 (`graphics.md` 7.2) (**[V]** palette RAM matched).
- **Timing:** `$5D` is set to 0 when the badge lands; the scene ends when `$5D` reaches 255 (`game-flow.md` 7.4), so
  about 255 frames after landing: 359 frames in all in the case watched (handler start to the end). **[V]**
- **Counting the crystals** (`economy.md` 5.4): from 16 frames after landing, every 16 frames while the crystal count
  `$67` is not 0, one crystal is taken: 100 points, a sound, and a gem picture is drawn: 2 x 2 background tiles $A0
  $A1 over $A2 $A3 (background pattern table, blue gem) at tile row 16 for the first five (columns 8, 12, 16, 20, 24)
  and at tile row 19 for the next ones (columns 8, 12, ..., 28), the picture appearing as the score is raised. The
  tenth counted crystal **and every one after it** give 1,100 each (the test is on the number counted so far, 10 or
  more). **[V]** (with `$67` = 11, 11 pictures: five in the first row, six in the second; with `$67` = 12: nine gains
  of 100, then three of 1,100, 16 frames apart. More than 10 cannot happen in play: each backdrop and the warp
  levels offer 10 crystals. The twelfth picture would be drawn at tile row 20, column 0 **[C]**.)
- **Ends:** `$5D` = 255; with `$DB` = 1 the level then resumes (`game-flow.md` 7.4); with `$DB` = 0 transition 3 follows
  (warp level back to its planet), and the planet's gameplay handler returns 28 frames after the last badge frame. **[V]**
- **Entered from a warp level (`$DB` = 0).** The warp level's countdown object (slot 3, `objects.md` 7) counts one per
  logic step from the level's first step; its digit frame is $73 + count / 64, and when that reaches $7E, that is
  at count 704 (= 11 x 64 steps = 1,408 frames), the same step sets `$DB` = 0 and requests transition 6. The badge
  handler starts 3 frames after the request and then runs 359 frames to its end as above (the counting of
  crystals included: two crystals gave two gains of 100). **[V]** (level 29, the level's first frame numbered 0: count 0
  on frame 1, request on frame 1,409, badge from 1,412 to 1,770, the planet's handler back on 1,799.)

## 12. Game over and last password [V]

- **Game over** (the screen stays as it was, nothing blanks; 240 frames; frame numbers c below count from the
  first frame of the scene): the playfield is frozen as it was (status panel included) and the sprite letters
  "GAME OVER" are added at c = 15. **The letter tiles ($00-$19 of table 0) are streamed in at the start of the
  scene, two per frame, from the 26-tile block at ROM 0x0F6AE (`graphics.md` section 4, the "animated tiles"
  stream, 13 frames of 2 tiles): all 26 match the ROM bytes from c = 14 on, which is why the letters first appear at
  c = 15 [V]** (compared tile by tile in Mesen, 2026-10-17; the tile numbers of the table below are indices into
  that block, in ROM order). They are 32 sprites (eight letters, each 2 × 2), palette 1 of the sprite
  palettes in force (unchanged during the scene), in front, tiles of table 0:

  | Letter | G | A | M | E | O | V | E | R |
  |---|---|---|---|---|---|---|---|---|
  | x (left sprite; right sprite +8) | 99 | 116 | 134 | 151 | 99 | 116 | 134 | 151 |
  | top row tiles | $00 $01 | $04 $05 | $08 $09 | $0C $0D | $00 $10 | $12 $13 | $0C $0D | $16 $17 |
  | bottom row tiles | $02 $03 | $06 $07 | $0A $0B | $0E $0F | $02 $11 | $14 $15 | $0E $0F | $18 $19 |

  The bottom row is 8 lines below the top row. Every letter bobs on one **triangle wave** `W(c)` of 64 frames, 8
  lines peak to peak, one line per 4 frames with 8-frame rests at the extremes: with k = (c - 18) mod 64, W = 59 for
  k 0-3, 58 for 4-7, 57 for 8-11, 56 for 12-19, 57 for 20-23, 58 for 24-27, 59 for 28-31, 60 for 32-35, 61 for
  36-39, 62 for 40-43, 63 for 44-51, 62 for 52-55, 61 for 56-59, 60 for 60-63 (so W = 60 at c = 15). The sprite-
  memory y of a letter's top row is: **G and M: W(c); A and E (of GAME): W(c - 16); O and E (of OVER): W(c - 16) +
  32; V and R: W(c) + 32** (so letters alternate 16 frames out of phase, and OVER hangs 32 lines below GAME). The
  hardware swaps which half of the sprite table holds the letters every frame: not observable.
  **[V]** (all 225 frames from c = 15 recorded and compared with these rules: no difference).
  - **Input:** no button is read: A at c = 35 and Start at c = 175 had no effect; the scene always lasts
    240 frames. **[V]**
- **Last password:** blank for 2 frames (table in 0.1), then a black playfield with "YOUR LAST PASSWORD WAS" row 9
  column 5 and the 12 letters row 12 column 10; the status panel stays. **No lock-out:** a button pressed in the
  first visible frame is accepted. **[V]**
- **Ends:** any newly pressed button, 3 frames after the press the screen blanks (14 blank frames) and the title
  starts as after a power-on without the licence screen: a warm reset (`game-flow.md` 7.4). **[V]**

## 13. Pause and planet map

### 13.1 Pause [V]

The picture freezes; nothing is drawn or changed (`game-flow.md` 4 for the rules). There are no blank frames in or out.

### 13.2 Planet map [V]

A top view of the whole planet in which every 8 × 8 block-pixel chunk is one tile, with the pod, the mothership and
(with the better device) the items marked. Verified in Mesen by pausing on planets 0, 6 and 11 and pressing Select
with the device flag RAM $060C or $060D set; the window, the sprites, the colour cycle, the timing and the exit were
read back frame by frame.

- **Trigger.** Select during pause, only on a planet (level index RAM $3B < 13) and only when RAM $060C (mapping
  device) or $060D (super mapping device) is not 0; otherwise nothing happens (`game-flow.md` 4). **[V]**
- **Entry.** The screen is black while the picture is built, then the finished screen appears at once. The build
  takes **15-58 video frames, depending on the planet** (not 2: the 2 counted before were the frames in which the
  game's frame interrupt ran, which is switched off during the build). **[V]** 2026-10-17 (every video frame
  logged; whole black frames after the frame of the Select, which itself goes black from line 197 on: planet 0 16,
  planet 3 27, planet 6 29, planet 11 59).
  - **Why: the original's CPU speed, not a designed delay [V].** The build never waits for the video (no read of the
    video status register at all in those frames); it only computes. Its first stage makes the chunk pictures at a
    steady 4 tiles (64 bytes) per video frame, so the time is about (tiles ÷ 4) + 5 to 11 frames (planet 6: 84
    tiles, 21 frames of tiles; planet 11: 188 tiles, 47 frames). The rest (frame art, nametable, window rows) takes 5-11 frames.
    It is loading time of the 1.79 MHz CPU, so the remake builds the map in one frame (`PLAN.md`, "Fidelity").
  - **Order of the video writes [V]:** the chunk pictures (table $1000 from tile 0 on), the frame tiles (from $1000 +
    16 B), the frame nametable and attributes, the title sprite tiles ($0800, $0860, $0B80), the **ship tiles last of
    the pattern loads** (0x29CB7 → $0A80, 9 tiles), then the 16 window rows; the screen comes on after the window.
  - **Sound [V]:** the Select silences the channels (status register 0, then enabled again) in its own frame; song 6
    is started (always, not "if different") when the build ends, 1-3 frames before the first picture, since the
    sound driver does not run while the frame interrupt is off. Nothing is audible during the black frames.

**Screen model [V].** Page 0, scroll (0, 0), background pattern table $1000, sprite table $0000, 8 × 8 sprites. The
status panel and the sprite-0 split are the gameplay's, **but one line higher**: lines 0-191 show page 0 (play:
0-192), the panel of page 1 exactly as it stood in play (fuel and weapon gauges, fitted-weapon icon, lives, score;
nothing in it changes while the map is up) starts one line earlier than in play (panel content on lines 201-224
instead of 202-225; rendering off at line 191 and back on at 194 instead of 192 and 195), because sprite 0 is
at y = 190 (screen line 191) instead of 191 (**Q37**, `behaviour.md` 10). The split tile is the same
(`graphics.md` 3). Rows 24-29 of page 0 are blank. **[V]** 2026-10-17 (screenshots of the paused game and of the
map compared row by row; sprite memory and the mask writes read back).

**Frame [V].** The frame art is menu screen A (`graphics.md` 5.9): nametable 0x36149 with tile base B, attributes
0x362F2, frame tiles 0x3684F at $1000 + 16 B, B = $62 when (surface picture number AND 3) = 2 (planets 1, 5, 8, 10)
and $54 otherwise. The frame leaves a window of **16 × 16 tiles at nametable rows 4-19, columns 8-23** (screen
pixels x 64-191, y 32-159). Attribute bytes put the window on background palette 2.

**What the window shows [V]** (all 13 planets' tile data compared against the pattern table read back from Mesen for
planets 6 and 11: 0 differences).

- **One tile per chunk.** Let `ids` be the chunk numbers of the planet's map (`levels.md` 4). Let `lo` and `hi` be the
  smallest and largest chunk number that is 3 or more anywhere in the map (W × H bytes, rows of stride W). Chunk
  numbers 0, 1 and 2 are drawn as the blank tile 0. A chunk number `c` from `lo` to `hi` is drawn as tile
  `c - lo + 1` of table $1000, so the tiles used are $01 to `hi - lo + 1`. (Only planets with `hi - lo + 1 < B` keep every
  picture: planet 11 has 187 tiles against B = $54, so the frame tiles, written after the chunk pictures, overwrite
  tiles 84-147 = chunks 86-149; the planet-11 map uses none of those chunk numbers, which are in the range only
  because the rows of tiles run from the smallest to the largest chunk number, so nothing visible is lost. Counted
  with the dump of planet 11: 64 tiles differ, all unused. **[V]**)
- **The tile pictures are built for the map.** For every `c` from `lo` to `hi` (also the ones the map does not use)
  the program makes an 8 × 8 picture of the chunk, one pixel per block, in the chunk's own row and column order, the
  left block being the leftmost pixel. With `block` the block number from the chunk (`levels.md` 4.1) and
  `class` its class from the block's collision class map (`levels.md` 6.2, the map of the planet's block set):

  | block | class | pixel colour |
  |---|---|---|
  | 0 | | 0 (transparent, black backdrop) |
  | not 0 | 0 (nothing to collide with) | 2 |
  | not 0 | not 0 | 1 |

  For chunk numbers **44 to 80** ($2C-$50) colour 2 is dropped: only colour-1 pixels are drawn (checked on planet 6, whose map
  uses them). **Why [V for the facts, H for the intent]:** chunk numbers are handed out per block set: set 0 uses
  3-80 (and 174-189), set 2 uses 81-173, set 1 uses 190-240 (all 14 maps read from the ROM). Chunks 44-80 are the
  deep-ground fill of set 0, and only planets 3, 6 and 11 use them, always in the bottom rows of the map (rows 10-13
  of 15, 20-31 of 32, 16-27 of 49). Their colour-2 pixels are the solid interior of the rock, so without them the
  deep ground shows as outlines and cave walls only instead of one slab. The test is on the chunk number alone
  (44 up to and including 80), a fixed range in the program, not data per planet. Not a bug that hides anything
  (the colour-1 surface pixels, which are what the player flies in, are all kept); the remake keeps the rule. The
  other block sets have no such rule (their interior chunks keep colour 2), which is an inconsistency of the
  original but does not matter, since those maps have no equally large solid mass. Tile 0 is all colour 0.
- **Colours.** Palette 2 of the window is set by palette preset 8 (`graphics.md` 7.2: data 0x075B3, 7 bytes from
  shadow entry 9): colour 1 = $3C (light cyan), colour 2 = $2C, colour 3 = $1C (unused), entry 12 = $00, palette 3 =
  $31, $21, $11 (not used by the window). The other palettes are the gameplay ones (`graphics.md` 7.3): the frame art
  uses palettes 0 and 1 of the level, the panel as in play.
- **Window origin (cx0, cy0), in chunks, kept in RAM $AD and $AE.** With the player's chunk (px, py) =
  (x / 128, y / 128) from object slot 0, W and H the map size in chunks:

  ```
  a = px - 8;  if a < 0 then a = 0
  if a >= W - 17 then a = W - 16        # W < 16: a = 0
  cx0 = a                               # same for cy0 with py and H
  ```

  (the test is `a + 16 < W - 1` to keep `a`; at `a = W - 17` the window is moved one chunk, so a player 16 chunks
  from the right edge sees the last 16 columns). The window shows chunks (cx0 .. cx0 + 15, cy0 .. cy0 + 15). **[V]**
  for planets 0, 6 and 11 (player in the middle, at the top left, near the sides), [C] for the exact clamp at
  `W - 17`.
- **Narrow and short planets (a defect of the original).** Row `r` of the window is 16 consecutive map bytes
  starting at `(cy0 + r) × W + cx0`, so on a map narrower than 16 chunks the right part of the row is the start of the
  next row (planet 0, 8 chunks wide, shows every row twice side by side **[V]**; the last row reads whatever bytes
  follow the map). Rows with `r >= H` are blank (planet 0 shows its 8 rows and blank below **[V]**; planet 3, H = 15, loses the 16th row **[C]**). The
  remake draws blank for everything outside the map (`docs/deviations.md`).
- **Static.** The window is the whole planet, not an explored part: nothing in it depends on where the player has
  been. **[V]**

**Sprites [V]** (sprite table $0000: tile $85 from the stream 0x0FF2F → $0800, tiles $AD and $AE from 0x29CB7 →
$0A80, `graphics.md` 5.9). Block coordinates are world pixels / 16 (bx, by); a marker is drawn only if its chunk
column is in `cx0 .. cx0 + 15` and its chunk row in `cy0 .. cy0 + 15`. Position of a block in the window:
`u = bx - 8 cx0`, `v = by - 8 cy0` (0-127).

| Sprites, in order | Tile | Palette | Position (sprite-memory y = screen y - 1) | Condition |
|---|---|---|---|---|
| 1: the player (slot 0, pod or Jetman) | $85 | 1 | x = 64 + u, y = 28 + v | always |
| next, one per items-list entry with a kind that is not 0, from the last entry down to entry 1 (entry 0, the mothership's fuel count, has none) | $85 | 2 | the same, from the entry's home position; the entry of the object held in the tractor beam uses the held object's live position **[C]** | RAM $060D (super device) not 0, otherwise the sprite exists but is hidden |
| last: the mothership, two sprites | $AD at x - 6, $AE at x + 2 (screen) | 0, **behind** the background | x = 64 + u, y = 28 + v of the mothership's home (RAM $D6-$D9) | always |

Tile $85 is a 4 × 4 round dot in the top-left of its cell, colour 1; $AD and $AE are a 16 × 7 picture of the ship
(colours 1-3). The dot's top-left is the block's pixel. The mothership marker is drawn behind the background so it
shows only on black. Entries whose kind has been set to 0 (collected) lose their dot **[V]** (kind poked to 0);
items outside the window have none.

**Colour cycling [V].** The only animation. With `t` the frame counter RAM $5D (it runs on in this scene from
where gameplay left it, so the phase is arbitrary):

| Palette | Colours 1-3 | Changes |
|---|---|---|
| sprite palette 1 (the player's dot) | step `(t >> 3) AND 3`: ($30, $20, $00), ($22, $12, $0F), ($14, $04, $0F), ($28, $18, $08) | every 8 frames |
| sprite palette 2 (the item dots) | `(t >> 4) AND 1`: ($30, $20, $00), ($0F, $00, $30) | every 16 frames |

Only colour 1 is used by the dots: the player's dot cycles white, blue, violet, yellow; the items' white, black.
Sprite palette 0 is the gameplay's (mothership marker colours $30, $22, $12).

**Input and exit [V].** Only Start does anything (A, B, Select, the pad: nothing, each pressed for 4 frames). It is
accepted on the first visible frame (no lock-out). On the frame the Start is taken the level's song is started again
(always; RAM $0637 holds it), 18 blank frames follow, and play resumes by the **reload** of `game-flow.md` 7.4: the
player and objects as they were (kept exactly), the frame counters $5D/$5E set to 0. There is no timeout.

**What follows:** gameplay on the same planet, same state as before the pause (`game-flow.md` 7.4, "Reload").

## 14. Ending and credits

The last scenes of the game, in order: the final message (14.1), the ending picture (14.2), the credits (14.3), then a
warm reset to the title (`game-flow.md` 7.4, "Final level"). All four were observed in Mesen (**[V]**); how they were reached (a forced entry and a natural play-through) is in
"Provenance" at the end of 14.3. Frame counts are of the first forced run unless a rule says otherwise. The message's length
is not fixed in the original (14.1, "Timing"): the remake's rules are in `docs/deviations.md` (final message length) and in 14.1.

### 14.1 Final message (message 23, kind 7)

- **Trigger [V] (played naturally, 2026-10-17).** On the final level (level 26, RAM $9A bit 7), once the camera route has ended (RAM $9A bit 6, the
  player in state 6, `levels.md` 5.2), the route's routine checks every frame whether any of the object slots 23-27
  holds an object whose state is 6 or more. When none does, and the frame counter RAM $5D AND $1E is 0 (2 frames in
  every 32), it blanks the screen, reloads the gameplay palettes, requests **song 32** (`sound.md` 8.1; it plays until
  the program restarts **[V]**: nothing in the ending or credits changes it), sets RAM $DB = 7 and starts the
  message screen as a delivery does (transition 1). **[V]** The condition was met in play by shooting the last of the five cores (type 5E, `behaviour.md` 4.16) after the route had
  ended: its explosion (state 5) does not count as an object of state 6 or more, so the screen began at the first step
  with $5D AND $1E = 0, 13 steps later in the run; the same happens when a smart bomb wipes the cores out (`behaviour.md`
  Q32 for its catch). Any other object of state 6 or more in slots 23-27 delays it.
- **Screen [V].** A message screen of kind 7 (`game-flow.md` 7.4, table of kinds): page 0 cleared to black, the text of
  message 23 (`text.md` 3, ids and positions) in the gameplay font colour (background palette 0 colour 1, white), the
  gameplay status panel below as in 10, no sprites except sprite 0's split tile, no picture. **The text is not typed:**
  message 23 has no typewriter marker, so the whole message is written in one call and **all seven lines are on the
  first visible frame** (screenshots 2 and 10 frames after the screen's entry, 2026-10-17, both showed all lines; the
  earlier statement "typed at one character per 4 frames" was wrong). No click sound ($3A is only made by typed text).
- **Timing [V].** The original: three blank frames of loading (forced entry; delivery messages have four), then the screen is
  visible for 319 frames (in the 2026-10-17 forced run: visible from frame 64 to 382 of the run, blank 383-398, the ending from
  399, i.e. 319 visible frames then **16 blank frames**, which are loading time the remake does not have); kind 7 starts with $DA = 255 and the
  usual 64 frames in which nothing counts (`game-flow.md` 7.4), so it
  ends after 64 + 255 = 319 frames from its first handler frame **when RAM $5D is 64 or more at the start**; **[V]** 2026-10-17
  in two natural runs: the message screen of the natural trigger does not reset $5D (as the other messages do), $DA falls only on
  frames where the low byte of $5D is 64 or more (as for kind 6, `game-flow.md` 7.4), so it depends on $5D at the start,
  s = (the value at the trigger step, a multiple of 32) + 3: s >= 64 gives 319 frames (the shot-down run, s = $A3: 319 timer
  frames, then 16 blank frames, 335 in all), s < 64 gives 383 - s (the smart-bomb run, s = $23 = 35: 348 timer frames, 363 in all).
  So the message lasts between 319 and 380 frames (Q33 in `behaviour.md`). A button pressed during the first 255 frames
  does nothing; from the point where $DA is below 64 (the last 63 frames) a newly pressed button ends the screen
  two frames later. **[V]**
- **The remake's rule (one place).** The message is shown whole for **319 frames** from its first visible frame (255 + 64, whatever
  $5D was); a newly pressed button is accepted from frame 256 on (the 64th-last frame onwards) and ends it two frames after
  the press; the 16 blank frames of the original are not kept (no loading). The same rule is in `game-flow.md` 7.4, the
  kind 7 row, and `economy.md` 5.5 (`docs/deviations.md`, final message length).
- **What follows [V].** The ending (14.2) after the original's loading frames (1 to 16 blank frames, depending on the run; the
  remake none); the song keeps playing.

### 14.2 The ending picture [V]

Jetman with his feet up in the warpship's cabin, a small creature on the console, a star in the window.

- **Entry.** One blank frame after the message. **Duration** 902 frames from the first visible frame (measured in the
  first run; 906 in a second run whose message was ended early, because the idle ticks and the flash steps below
  run on multiples of the frame counter RAM $5D, so the schedule shifts by a few frames). No input does anything:
  Start, A, B, Select and the pad were pressed during it without effect. Song 32 continues.
- **Screen model.** Page 0, scroll (0, 0) all through. **One split:** lines 0-135 use background pattern table $1000,
  lines 136-239 use table $0000 (the change is made at the end of line 135, triggered by sprite 0 at screen
  (185, 131), tile $FE, behind the background: a hardware aid the remake does not draw). Sprites use table $0000
  throughout. No status panel (the whole screen is the picture), no other scroll or palette effects until the
  flash.
- **Loads** (`graphics.md` 5.11): nametable 0x3D379 → $2000, attributes 0x3F11D → $23C0, tiles 0x3D663 → $1000
  (so tiles $00-$D9 of table $1000), 0x3E1BF → $1DC0 (the face tiles $DC-$FF of table $1000), 0x3E3CE → $0000 (table
  $0000 tiles $00-$D4), 0x3EED8 → $0D60 (tiles $D6-$FF of table $0000: the console button, the creature and the
  star). The picture's upper part (rows 0-16) uses table $1000, the lower part (rows 17-29) table $0000.
- **Palette.** Preset 22 (`graphics.md` 7.2; ROM 0x076C4, 31 bytes from shadow entry 1): background palettes
  ($0F), $38 $28 $07 | $00 $28 $18 $07 | $00 $0F $0F $0F | $00 $30 $10 $07, sprite palettes ($0F), $27 $17 $07 |
  $00 $28 $18 $07 | $00 $0F $0F $0F | $00 $30 $10 $07. (Entry 0 is $0F.)
- **Sprites** (screen x, y of the top-left pixel; order does not matter, the original rotates it each frame):

  | Object | Sprites | Palette | Notes |
  |---|---|---|---|
  | Creature (6 sprites) | two columns at x = 184 and 192, three rows at y = 134, 142, 150 when resting | 3, in front | tiles by pose below; the whole creature moves up in jumps |
  | Star | tile $EC at (208, 40), flipped horizontally at (216, 40), vertically at (208, 48), both at (216, 48) | 0 | growth frames below |
  | Sprite 0 | $FE at (185, 131) | | split trigger only |

  **Creature poses** (rows top to middle to bottom; tiles per column pair, left then right):

  | Pose (object frame) | Row 1 | Row 2 | Row 3 |
  |---|---|---|---|
  | $B3 at rest | $DC $DD | $DE $DF | $E0 $E1 |
  | $B6 head turned (one way) | $E8 $E9 | $EA $EB | $E0 $E1 |
  | $B5 head turned (the other way) | $E4 $E5 | $E6 $E7 | $E0 $E1 |
  | $B4 in the air, legs up | $DC $DD | $DE $DF | $E2 $E3 |

  The creature is an object at (187, 141) when resting; its sprites are at x - 3 and x + 5, rows at sprite-memory y
  - 8, y and y + 8.
- **Background cells that change** (nametable row, column): they animate Jetman's face and the console button. Base
  values are those of the stream. Face states, tile numbers by cell (row r, columns c):

  | State | Cells set | Cause |
  |---|---|---|
  | blink | r6 c3-5: $DC $DD $DE; r7 c3-5: $DF $E0 $E1 | random: see below |
  | button (rows 18-20, columns 23-24) | $D6 $D7 / $D8 $D9 / $DA $DB | the last jump lands |
  | face A | r6 c3-5: $E2 $E3 $DE; r7: $E4 $E5 $E6; r8: $E7 $E8 $E9; r9 c2-5: $EA $EB $EC $ED; r10 c4-5: $EE $EF | the frame after the last landing |
  | face B | r6 c3-5: $F0 $F1 $F2; r7: $F3 $F4 $F5; r8 c3: $F6, c5: $F7 (c4 stays $E8); r9 c3-5: $F8 $F9 $FA (c2 stays $EA); r10 c3-5: $FB $FC $FD; r11 c4-5: $FE $FF | 40 frames after face A (5 star steps) |

  The face changes of A and B replace the blink state and stay to the end. The **blink**: on every frame where RAM $5D
  is not a multiple of 64 the random number RAM $5A is tested, and when it equals $0E the blink is shown; at every
  frame where $5D is a multiple of 64 the base face is restored. In the measured run one blink happened
  (frames 306-318 of the scene). It is cosmetic: the remake may blink at random with the same average (1 in 256 per
  frame), lasting until the next multiple of 64.
- **Timeline** (frames counted from the first visible frame = 0, first run):

  | Frame | Event |
  |---|---|
  | 0 | picture, creature at rest (frame $B3) |
  | 127, 191, 255, 319 | creature frame $B6, $B5, $B6, $B3 (one idle tick every 4 frames; the pose is `table[tick / 16]` of $B3 $B3 $B6 $B5 $B6 $B3 $B3, and after tick 112 the first jump, so 448 frames) |
  | 447, 477, 518, 571, 637, 718 | the six jumps start (jump n = 1..6) |
  | 460, 501, 554, 620, 701, 798 | the jumps land |
  | 798 | the button cells change; the creature rests for good |
  | 799 | face A |
  | 807, 815, 823, 831, 839, 847 | star frames $ED, $EE, $EF, $F0, $F1, $F2 (it was $EC from 0) |
  | 839 | face B |
  | 855, 863, 871 | star frames: 12 sprites; 16 sprites; 36 sprites (below) |
  | 879, 887, 895 | star gone; background palette 2 colours 1, 2 and 3 (all black, $0F, until now) become $27 at frames 879, 887 and 895 in turn (so ($27 $0F $0F), ($27 $27 $0F), ($27 $27 $27)): **the disc is not a set of cells but the picture's own pixels of those colours in palette-2 areas** (the attribute table, ROM 0x3F11D, 2 bits per 2 × 2 tiles, value 2); a large disc in the upper right turns yellow (a sun; seen in a screenshot at frame 898). The remake needs only the picture and its attributes plus the three palette steps |
  | 902 | the screen goes blank (2 blank frames): the credits |

  **Creature jump physics** (one step per frame; y is the object's y, resting 141; fixed point 8.8, speeds in pixels
  per step): jump n starts at speed `n` upward (n = 1..6); each step moves y by the speed and then lowers the speed
  by $2C/256 = 0.172 until it is 0; then it falls, each step moving y by the speed and raising the speed by 0.172,
  capped at 3.0, until y reaches 141 again (it is then set to 141). **The next jump starts 17 frames after the landing
  frame** (16 resting frames between them; **measured**: landing 460, next start 477; 501 to 518; 554 to 571; 620 to
  637; 701 to 718, every gap 17; where an earlier line said "rest 16 frames" it meant the frames in between), frame $B3 at rest;
  while in the air the frame is $B4. Peaks 4, 13, 27, 49, 75 and 108 pixels (**[V]**: per-frame y of every jump
  matched a model of this rule; the measured peaks and durations are in the timeline). After the sixth landing the
  creature counts as finished and the star sequence begins.

  **Star growth.** The star is four mirrored copies, symmetric about the point (216, 48): a sprite at (x, y) has
  copies at (424 - x, y) flipped horizontally, (x, 88 - y) flipped vertically, (424 - x, 88 - y) flipped both; all
  palette 0. Frames, each held 8 frames (the first from frame 0): $EC to $F2 are one sprite per copy at (208, 40)
  with that tile; then:

  | Frames | Sprites per copy, top-left copy (x, y: tile) |
  |---|---|
  | 855 | (208, 32): $F3, (200, 40): $F4, (208, 40): $FF |
  | 863 | (200, 32): $F5, (208, 32): $F6, (200, 40): $F7, (208, 40): $FF |
  | 871 | (192, 24): $F8, (200, 24): $F9, (208, 24): $FA, (192, 32): $FB, (200, 32): $FF, (208, 32): $FF, (192, 40): $FC, (200, 40): $FF, (208, 40): $FF |

  Tile $FF of table $0000 is the one-dot split tile (practically invisible); the remake may skip it.
- **What triggers the end [C]/[V].** The star's counter reaches its last step, the creature's slot is freed, and the
  ending's logic then starts the credits in the same frame. The ending is not skippable. A fourth flash step,
  due on the frame the scene ends, would set all palette entries 1-16 to $27; the screen is already blank then.

### 14.3 Credits [V]

- **The first lit frame of the ending and of the credits is incomplete [V]** (round 13). The sprite buffer is filled over the
  last step of the previous scene and the first of the new one, and the first lit frame is drawn from a buffer that is
  still part old: in the credits the badge lacks two sprites (buffer slots 24 and 25, a 16 × 9 pixel patch at (101, 93):
  3,234 lit pixels in the first frame, 3,362 from the second on); in the ending the first lit frame lacks a strip of sprites
  at x 185-218, y 46-158 (27,272 against 27,316 lit pixels). From the second lit frame on the pictures are
  the steady ones. Cosmetic: the remake draws the full picture from the first frame (Q42).

- **Entry.** The blank frames after the ending (2 in the first forced run, 7 in the second), then the visible frames of the credits (2,271 in the first forced run; **2,274 in the second forced run and in the natural run**: the figure to use is **2,274**, the first run's 3 fewer frames are probably an offset in how its first visible frame was counted **[H]**). Song 32 continues. No input is read
  (Start, A, B, Select and Up pressed during the credits: no effect). **[V]**
- **Screen model.** Page 0, background pattern table $1000, sprite table $0000, x scroll 0. **Lines 0-231 are drawn in
  full, line 232 only up to x = 120 (the split switches rendering off during that line, a CPU-timing artefact: the
  write came at dot 101-115 of line 232 in every frame looked at), and lines 233-239 are black** (corrected
  2026-10-17, round 13; the earlier "bottom 16 lines from 224" was wrong; **[V]**: screenshots of every frame from the
  first text to the last, the lowest non-black line was 232 and its right-most pixel x = 120 in all 57 frames where text
  reached it, nothing below). Text therefore appears at line 232 and moves up; the remake hides lines 233-239 and draws line 232 whole
  (or hides it: one line of a glyph). Sprite-memory y of sprite 0 is $DF (screen
  line 224), the one-dot tile $FF, behind the background; its marker pixel is visible at (8, 224) in the frames
  where nothing else is there. The split fires 8 lines after the marker line (the marker's own line is 224): why
  the program waits that long is not known **[H]**.
- **Palette.** Preset 20 (the badge's palette, 2.4): background palette 0 = $0F $30 $10 $00 (text colour 1, $30,
  white on black), sprite palettes as 2.4.
- **Pictures.** Font: digits, © ™, letters and extras as the licence screen (`graphics.md` 5.1; seen in Mesen: table
  $1000 holds only tiles $30-$3B and $41-$65 and the split tile $FF). The badge: tiles $C0-$FD at $0C00 of table
  $0000 (`graphics.md` 5.3, 0x357C3), the 62 sprites of 2.4 in the same order and with the same palettes, **8 pixels
  further left** (first row at x = 101, y = 69; second row at x = 93; ... last row at x = 117, y = 133) and all
  **behind** the background: the text is drawn over the badge (white over pink, seen). It is fixed for the whole
  credits.
- **Text and scroll.** The text is in `text.md` 4.7 (lines, gaps, centring). Mechanics, **[V]** by frame:
  - The background scrolls vertically only: scroll y rises by 1 every 2 frames (y = floor(f / 2) for f frames since
    the first visible frame, counted from 0), wrapping from 239 to 0 (the page is 30 rows high).
  - Whenever the scroll reaches a multiple of 8, the program writes the next row: nametable row `(S / 8) - 1` modulo
    30 where `S` is the number of scroll steps so far (1 to 142), i.e. a new row every 16 frames, at the bottom of the
    screen (hidden by the split for its first 16 frames). The row is either blank or one centred text line.
  - **Row sequence.** Rows are numbered L = 0, 1, ...: 8 blank rows, then the data of `text.md` 4.7 (gap = blank
    rows, text = one row). Text row L has its top at screen y = 8 L + 240 - S, so it enters the visible area (y = 224)
    at frame 16 L + 32 and reaches the top edge at 16 L + 480. The first line, SOLAR JETMAN CREDITS (L = 11), is
    visible from frame 208, the last, A TRADE WEST PRODUCTION (L = 101), from frame 1648 and is gone from the top at
    frame 2096; the 39 blank rows after it leave about 160 frames of black with the badge only.
  - **Counting convention [V]** (round 13: every frame of five forced runs photographed, with the scroll shadow
    RAM $03 and the frame counter RAM $5D logged): the frame numbers above are the program's (scroll shadow
    = floor(f / 2) at f = 0, the frame the credits' set-up ends; the counter RAM $5D is 1 at f = -2). **The first two
    frames (f = 0, 1) are black: rendering is switched on in f = 2, the first lit frame.** What is on screen lags the
    shadow by one frame (lit frame f shows scroll floor((f - 1) / 2)). The last frame, f = 2273 (scroll 176), is a lit
    frame **cut at line 82** (the warm reset switches rendering off mid-frame; only the top 13 lines of the badge show),
    so the credits show **2,272 lit frames (2,271 complete, one cut)** after 7 black frames following the ending's last
    frame, then 15 black frames (the cut one excluded; 14 in the earlier count) to the title. 2,274 = 2 dark + 2,272 lit; the earlier runs'
    "2,271" counted complete frames only.
  - 141 rows are drawn (L = 0 to 140); the 142nd draw, at S = 1136 (frame 2272, scroll 176), reads the end marker. **The credits' last
    two frames after that read are still drawn** (frames 2272 and 2273, scroll 176, the frame counter stopped), and the screen is
    blank from frame 2274 on **[V]** (second forced run: the first visible frame was frame 0, scroll y $B0 = 176 at frames 2272
    and 2273, blank at 2274).
- **Ends [V].** The program restarts (warm reset, `game-flow.md` 2 and 7.4): 14-15 blank frames (15 counted after the cut frame, round 13), then the title, with its
  song started (song 1 replaces 32: "start if different").

- **Provenance.** The natural trigger needs the whole final level, so the first runs entered the program's own end-of-level
  routine with the program counter set to it at the start of a logic step; everything after that point ran unmodified. Those
  runs (planet 6; the second on level 26, 2026-10-17, from the start of the sequence of 14.1) gave the timings above, with
  button-press runs repeating them. The natural trigger was then played too (level 26 run along its route, the five cores shot
  down), with the same screens and timings: message 335 frames from the first frame of its handler (319 of timer and 16 blank
  frames), ending 908 and credits 2,274 frames counting the blank frames between, the title 13 frames after the credits. Small
  differences between runs (ending 902, 906 and 908 frames; credits 2,271 and 2,274) come from the frame counter's phase
  (`game-flow.md` 1.5): the idle ticks and flash steps run on multiples of RAM $5D. Suggested for the remake (a
  decision for Alex): 902 frames for the ending and 2,274 visible frames for the credits.

## 15. Music per scene

Song numbers are those of `sound.md` 8.1 ("start if different" does nothing when the same song is already playing;
"always" restarts). Observed by logging every song start while each scene was reached (power-on, title, attract
cycle for 60,000 frames, new game, password, descent, blast-off, flight, shop, equip list, and the transitions forced
with $3F), **[V]** unless marked; **[C]** = from the program's song requests (all of them were found: twelve call
sites). A scene that starts no song leaves whatever is playing (a "once" song may have ended: silence).

| Scene | Song | When |
|---|---|---|
| Licence screen | none (silence) | the sound driver is idle |
| Title sequence | 1 (once, 2,662 frames) | start if different, at the first frame of the title (frame 512 of a cold start) **[V]**. Coming back to the title from the attract cycle does not restart it (it is the song playing: $0637 = 1), so after the first 2,662 frames the title and the whole attract cycle are silent; after a game, when another song was last started, it starts again |
| Attract: badge, story, demo | none **[V]** | no start: the title song if still playing; the demo starts no level song |
| Password entry | none **[V]** | song 1 continues if still playing |
| Approach screen | 7 + 2 × L, L = level index (8 when L ≥ 12) | start if different, as the screen is built (`sound.md` 8.1) **[V]** |
| Surface, descent (mode 0) | 3 (once, 1,077 frames) | start if different, at the start of the descent, including when A skips it **[V]** |
| Surface menus (modes 2, 3) | none | song 3 may have ended |
| Surface, leaving (mode 1) | 5 | start if different, when mode 1 starts (after the 15 passes of mode 4) **[V]** |
| Launch scene | none **[V]** | |
| Interplanetary flight | 4 | start if different, two frames before the first pass **[V]** |
| Shop, equip list | none **[V]** | song 4 continues in the shop |
| Gameplay level | planet song 8 + 2 × m, or 5 (warp levels), or 33 (final level); none in the demo | `sound.md` 8.1 |
| Level-change gate (wormhole touched, player state 9) | 2 (once, 389 frames), started as the gate state begins **[V]**; the pod's destruction and the game over start no song (**[V]**, both run in Mesen) | `sound.md` 8.1 (this row replaces "pod destroyed") |
| Message screens (kinds 1-5, 7) | none [C] | the level song continues |
| Kind 6 warp-level screen | none [C] | |
| Badge intermission (transition 6) | 0 (silence), at entry; the level song restarts at the next level start **[V]** | the forced run: song 0 at the first frame, song 8 when play resumed |
| Backdrop interlude (transition 7) | 31 | start if different, at entry **[V]** |
| Game over, last password | none **[V]** | whatever was playing (the level song) |
| Planet map (pause, Select) | 6 | start always, on the frame Select is taken (after silencing the channels); on leaving the map the level song (RAM $0637) is started again (always), on the frame Start is taken **[V]** |
| Pause | none: the driver is not run (`sound.md` 2) | |
| Final message (message 23), ending, credits | 32 | started as the final level's end routine runs, before message 23 **[V]**; the ending and credits request nothing more, so song 32 keeps looping to the reset **[V]**; the title then starts song 1 |

Songs 7, 9, ..., 29 are the approach songs, 8, 10, ..., 30 the planet songs; songs 6 (map), 31 (backdrop) and
32 (congratulations) are the three that `sound.md` listed as "context unknown" or "no request found".

## Open questions

- Closed: the sun/logo camera (2.2, model verified); the floating Jetman scroll (3, exact change frames); the
  title's flying Jetman path (2.3, rule and model verified). Open: the story's y wave (3, table only).
- The menu-and-list pieces of the shop strip's hardware ring (6) and the sprite layouts of the six mothership
  pictures (8.1, by layout number in `objects.md` 6) are not tabulated here.
- (closed 2026-10-17: the map screen, ending and credits, 13 and 14; open there: the natural trigger of the ending, played 2026-10-17 in 14.1; chunks 44-80 losing colour 2, explained in 13.2.)
- (closed: the menu-box position depends on the layout only through its height, 8.)
