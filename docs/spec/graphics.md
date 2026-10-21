# Graphics: storage, decoding and loading

How the original stores its tile graphics, nametable pictures and palettes in PRG ROM, how it turns them into
CHR RAM / nametable / palette contents, and which sets are loaded in which scene. Everything here is **ripped**
data (graphics: never committed, imported from the player's ROM; see `rom-map.md` for the category of every
byte range).

Addresses follow `README.md`: `ROM 0x1A2B0` is a PRG ROM offset (header excluded), `bank 2:$A2DE` a CPU address
with that bank mapped. For any bank-local address, `ROM offset = bank × 0x8000 + (address − $8000)`.

Status: **[V]** verified (our decoder output compared byte for byte with CHR RAM / nametable / palette RAM dumped
from the original running in Mesen), **[C]** read from the program, **[H]** hypothesis.

## 1. Video memory as the game uses it

- **CHR RAM** is 8 KB: pattern table 0 at PPU $0000-$0FFF, pattern table 1 at $1000-$1FFF, 256 tiles each.
  Tile `n` of a table starts at `table + 16 × n`. **[V]**
- **Tile format** (standard NES 2 bits per pixel): 16 bytes per 8×8 tile. Bytes 0-7 are bit plane 0 for rows
  0-7, bytes 8-15 are bit plane 1 for rows 0-7. In each byte the most significant bit is the leftmost pixel.
  Pixel value = plane-0 bit + 2 × plane-1 bit (0-3); 0 is transparent / background colour. **[V]**
- **At power-on** the game fills all 8 KB of CHR RAM with zero, and does so again before the title screen. **[V]**
- **Pattern table use.** In most scenes backgrounds use the table at $1000 and sprites the table at $0000;
  sprites are always 8×8. Per-scene details, including mid-screen switches and scroll, are in section 1.1. **[V]**
- **Nametables.** The console has two 1 KB nametable pages (960 tile bytes + 64 attribute bytes each). The mapper
  shows one page at a time on the whole screen (one-screen mirroring), selected by bit 4 of the value written to
  switch banks (`rom-map.md` section 1); whichever page is selected is the one seen at PPU $2000-$23FF (and at
  $2400, $2800, $2C00). Page 0 holds every full screen: title, pictures, play field. Page 1 holds only the status
  panel shown at the bottom of the approach screen and of gameplay; the program switches to page 1 for the panel
  in mid-screen and back to page 0 during vertical blank (1.1). All ROM nametable streams of section 4 go to
  page 0. **[V]**
- **Status panel (page 1).** Tile rows 4-6 of page 1 hold the panel's contents (approach screen: text "SYSTEM",
  system number and name, lives, score digits; gameplay: the two gauge bars, pods, lives, score digits, no text; layout
  in 1.2); the rest of page 1 is blank. The program lays the panel out
  itself (text and digits through the text routines, see `text.md`); no ROM nametable stream is involved. Its
  tiles are the status-panel tiles at $1D60/$1E40, the font and the gauge tiles. **[V]**
- **Universal background colour** (palette RAM entry 0, mirrored at entry 16) is uploaded from palette shadow
  entry 16 (section 7.1); it is $0F (black) in almost every scene, $01 in the attract floating-Jetman scene, and
  forced to $0F on warp levels. **[V]**
- **Writes run on.** Every load writes its bytes to consecutive PPU addresses. When a decoded stream is longer
  than the space before the next region, its last bytes land in the next tile, or, for streams ending at $1FFF,
  in the nametable at $2000. The original does this; section 5 gives each scene's load order, and the final
  contents follow from replaying the loads in that order. An importer that keeps each decoded stream whole
  (including such trailing bytes) and replays the loads reproduces CHR RAM exactly. **[V]**

### 1.1 Pattern tables, splits and scroll per scene

"Scroll" is the background position at the top of the screen (x, y in pixels). Several scenes end their load by
setting the video address to $2000 (the nametable base) after the two scroll writes; the video address and the
scroll share bits, and the address $2000 has bit 13 set, which is a fine vertical scroll of 2 (a hardware effect of
that write, the same on real hardware). It stays until the scroll is written again, so the picture is drawn 2 lines
higher and the last 2 screen lines repeat lines 0-1 of tile row 0. This is noted as y = 2.

| Scene | Background / sprite tables | Mid-screen change | Scroll | Status |
|---|---|---|---|---|
| Licence screen | $1000 / $0000 | none | 0, 2: set once in the last load frame, never rewritten (the handler writes only the control register), so y = 2 holds for all 512 frames | [V] |
| Attract: sun and logo | $1000 / $0000 | none | animated (changes every frame) | [V] |
| Title | $1000 / $0000 | none | x = 4, y = 224 (the screen starts at nametable row 28 and wraps to row 0 at line 16) | [V] |
| Attract: "Space Loonies" logo | $1000 / $0000 | none | x = 4, y = 224 | [V] |
| Attract: floating Jetman | $1000 / $0000 | none | animated (camera moves) | [V] |
| Approach screen, gameplay | $1000 / $0000 | lines 0-192 (193 lines): page 0, the playfield or star field (consistent with `levels.md` 5.1). Lines 193-195: rendering off, the backdrop colour shows; the page switches to 1. Lines 196-239: page 1 from nametable pixel row 26 (tile row 3, line 2) with x scroll 252, sprites hidden. Page-1 row 3 is blank, so lines 196-201 are blank and the panel's content (rows 4-6) starts at line 202 | approach: 0, 0; gameplay: the camera position | [V] |
| Planet-surface picture | $1000 / $1000 | at line 126 the background table becomes $0000 (sprites stay at $1000), so tile rows 0-15 come from $1000 and rows 16-29 from $0000. No status panel (page 0 only) | 0, 2: with the 2-line offset the last two screen lines (238-239) show lines 0-1 of tile row 0 again, drawn from table $0000 | [V] |
| Mothership launch | $1000 / $0000 | none | 0, 0 | [V] |
| Password entry (menu screen B) | $1000 / $1000 | none | 0, 2 (as on the surface picture, the last two lines show tile row 0 again) | [V] |
| Shop and equip list (screen of 5.10) | $1000 / $0000 | a sprite-0 split (sprite at y = 44): below it, until line 103, the strip of item frames is scrolled horizontally by 64 × (selected item), then reset | 0, 0 above the split; strip animated | [V] (`scenes.md` 6) |
| Planet map (menu screen A) | $1000 / $0000, the gameplay's status-panel split **one line higher than in play** (sprite 0 at y = 190; page 0 on lines 0-191, panel content from line 201) | as gameplay | 0, 0 | [V] (`scenes.md` 13.2, Q37) |
| Ending | $1000 / $0000 above line 136, $0000 / $0000 from line 136 (sprite-0 split) | one split | 0, 0 | [V] (`scenes.md` 14.2) |
| Credits | $1000 / $0000; rendering off from line 224 | one split | x 0, y 0-239 | [V] (`scenes.md` 14.3) |
| Menus C, D (analysis screen), comet screen, backdrop, the screen of bank 2 entry $8036 | not observed (its load was only forced from the title screen; entry $8033 is the flight, `scenes.md` 5) | | | |

One more sprite-0 split exists, setting background and sprites both to table $0000 below the split, in a screen
that also shows the "Space Loonies" logo (probably the game-over screen); not observed. [C]

### 1.2 Gameplay status panel: tile layout **[V]** (level 0; values forced in RAM and the screen read back)

The panel is nametable page 1 (1.1: shown from line 196, scroll x = 252, so tile column `c` is at screen x =
8c + 4, and tile rows 4, 5, 6 are on lines 202-209, 210-217, 218-225). Everything else on the page is tile $00
(blank). Tile numbers are those of pattern table 1 ($1000). The panel is redrawn by the rotating HUD jobs
(`game-flow.md` 1.2), so a changed value appears a little later (it was settled well within 100 frames in every
test), not in the same frame.

| Rows, columns | Content |
|---|---|
| row 4, col 1 | $F7: the letter "F" (fuel gauge label) |
| row 6, col 1 | $F8: the letter "W" (weapon gauge label) |
| row 4, cols 2-15 | fuel gauge, 14 cells |
| row 6, cols 2-15 | weapon gauge, 14 cells |
| rows 4-5, cols 16-19 | jetpods: one icon per pod, 2 tiles tall ($F4 above $F5), filled from col 16 to the right |
| row 6, cols 16-19 | lives: one icon per life, 1 tile ($F6), filled from col 16 to the right. **The picture of tile $F6 depends on the level** (5.7.6): on every level that loads the common gameplay background (all but level 26 and the warp levels 29-41) it is the figure of tile $F6 in that stream (ROM 0x17159); on level 26 and on the warp levels it is the status panel stream's own tile (ROM 0x0FA6E), a slightly different figure |
| row 6, col 20 | score sign tile $E4 when the score is negative, else blank |
| rows 4-6, cols 28-30 | the fitted special weapon's icon, 3 × 3 tiles $C3-$CB in reading order (top row $C3 $C4 $C5, middle $C6 $C7 $C8, bottom $C9 $CA $CB); blank when none is fitted (see below) |
| rows 5-6, cols 21-26 | the six score digits, most significant at col 21; digit `d` is the tile pair (top in row 5, bottom in row 6) below |

- **Gauges** (value `v` = 0-224, `economy.md` 3.1: fuel uses the eased display copy, the weapon RAM $C9): the first
  `n = v / 16` (rounded down) cells are the full tile $F9; if `n < 14` the next cell is the partial tile (fuel
  $FA, weapon $FB, `graphics.md` 3; this tile is drawn even when the remainder is 0, as an empty cell); the rest
  are blank. Examples: v = 224: 14 x $F9, no partial; v = 56: $F9 x 3, $FA, blank; v = 144 (weapon): $F9 x 9, $FB.
  Rows 5, and row 4 / 6 outside the gauge, are blank.
- **Icons:** 1-4 pods or lives draw that many icons; more than 4 draws 4; 0 draws none for lives and for pods. The count is
  an **unsigned** byte (the byte can reach 0 and wrap to 255, `economy.md` 4): **every value from 4 to 255 draws 4
  icons**, there is no negative case. **[V]** 2026-10-17: 0-6, 8, 16, 100, 127, 128, 129, 192, 200, 250, 254 and 255 each set for
  both counters and read back after 150 frames (a fresh start for each value; the earlier "254 drew 1" came from a run in which
  the pod had docked and the screen was no longer the gameplay panel).
- **Fitted-weapon icon [V]** (2026-10-17; the equip list used for real, then the launch and the gameplay panel read
  back, for every special weapon and for the other fitting slots). It is background tiles, not sprites. Position:
  tile columns 28-30 of rows 4-6, i.e. screen x = 228-251 and lines 202-225 (the same scroll as the rest of the
  panel); background palette 0 (page-1 attribute byte 15 is 0: colours $30, $10, $00 of the level's palette 0 on
  the first level checked, white and greys).
  - **Which icon:** only the special weapon (RAM $D1, items 0-5) has one, and the picture is that item's own
    picture: item n shows picture n. Items 0, 1, 2, 3, 4, 5 (HOMING MISSILES, ANTI GRAVITY, SMART BOMBS, TIME BOMBS,
    STAR BULLETS, MULTI WARHEAD MISSILES; $D1 = 1, 2, 4, 5, 8, 6) each showed their own picture. The bullet
    upgrades (items 6, 7, $050E), the pod devices (items 8-11, $D3) and the mapping devices draw nothing, alone or
    together with a weapon: with items 0, 6, 8 fitted only item 0's picture shows.
  - **Tiles:** the 9 tiles of picture n are the raw bytes at 0x15D40 + 0x90 n (0x90 bytes; the same pictures as the
    shop's, 5.10), loaded to $1C30-$1CBF (table 1 tiles $C3-$CB) when the level is loaded, from the weapon fitted
    at that moment. (The load itself is not in the loader log; source and place are from the pattern table.)
  - **When it changes:** only when the panel is built (level load, reload); a weapon fitted or lost during play
    does not redraw it (RAM $D1 poked mid-level: panel unchanged), and the rotating HUD jobs never touch columns
    28-30. With nothing fitted the nine nametable cells are blank (tile $00) and the tiles stay unwritten. Not
    checked: whether the icon is cleared when the pod is destroyed (the weapon is lost, `economy.md` 6.4).
  - The icon's tiles are table-1 tiles $C3-$CB, which the terrain loads leave unwritten (5.7.1); no map of levels 0-28
    uses them (every placed tile of every map checked), so the icon covers nothing.
- **Digit tile pairs (top/bottom)** (bank 0 $9693 holds the ten top tiles, $969D the ten bottom tiles; ROM 0x01693): 0 $E5/$E6, 1 $E7/$E8, 2 $E9/$EA, 3 $EB/$EC, 4 $ED/$EE, 5 $EF/$EC,
  6 $F0/$E6, 7 $F1/$E8, 8 $F2/$E6, 9 $F3/$EC. All six digits are drawn, leading zeros included.
- **Attributes** (page 1 attribute bytes 8-14 are the only non-zero ones; 2 x 2-tile quadrants): cols 0-1 use
  background palette 0, cols 2-15 palette 1 (the labels at col 1 palette 0), rows 4-5 of cols 16-19 palette 1 and row 6
  of cols 16-19 (lives) palette 0, cols 20-27 palette 1. The palette colours are the level's (`graphics.md` 7).
- **No system name in play.** The gameplay panel has no "SYSTEM n" text and no system name: rows 4-6 hold only the
  items above (and, with a special weapon fitted, its icon at columns 28-30). The approach screen's panel is different (`scenes.md` 7, "Status panel, page 1": "SYSTEM n", the
  name, lives as pod icons, the score) and has none of the gauges.

### 1.3 Left-column mask (PPU mask register) **[V]**

The game writes the display's mask register as $18 (background and sprites on; **bits 1 and 2 clear**) for the main
picture of **every** scene, so the leftmost 8 screen pixels (columns 0-7) show the backdrop colour: no background and
no sprite is drawn there. This is a **hardware option (the left-column mask)** that the original chooses, not a
property of the pictures: the video memory behind it is intact. Written values, per scene (recorded over a power-on
run and forced runs of every scene; value at the start of the picture / at the status-panel strip):

| Scene | Mask |
|---|---|
| Licence, title, attract (badge, story), password entry, surface picture, launch scene, shop and equip list, kind-6 screen | $18 all frame |
| Interplanetary flight | $18; $10 (background off, sprites on) in the last 138 frames, when the stars collapse |
| Gameplay, attract demo, pause, approach screen, message screens (kinds with the panel), badge intermission, backdrop interlude, game over, last password | $18 for lines 0-191, off for lines 192-194, then **$0A** for the status-panel strip (background only, **left column shown**, sprites off) |

(The planet map, ending and credits were measured on 2026-10-17: `scenes.md` 13 and 14.) Effects: sprites with x < 8 are invisible (the story's
leftmost stars, for example), and the background in columns 0-7 is cropped (the password screen's frame art at its
left edge, for example). In the panel strip the left column is shown, but no sprite is. **For the remake:** this is
a feature of the display hardware that the original switches on; nothing else in the game depends on it. The remake
may draw the full 256 columns (showing a little edge content the original never showed), or keep an 8-pixel
backdrop-coloured margin on the left to look the same; it should treat it as a display option, not as game logic.
The colour-emphasis value $E0 appears once, for part of one frame, when the licence screen ends (rendering off): a
hardware blank, to be ignored.

## 2. Storage formats

Every graphics or picture blob in the ROM is in one of four formats (2.1-2.4; 2.5 is in practice 2.1). None
has a header: the location, length
and destination come from the program (catalogue in section 4).

### 2.1 Raw tiles **[V]**

`n` tiles = `16 × n` bytes copied unchanged, in tile format, to consecutive CHR addresses.

### 2.2 Raw nametable / attribute bytes **[V]**

Bytes copied unchanged to PPU addresses (a full picture nametable is 960 bytes for $2000-$23BF, its attribute
table 64 bytes for $23C0-$23FF). A few screens copy 49 attribute bytes.

### 2.3 Run-length stream **[V]**

Used for most tile sets, the title and launch nametables, and most attribute tables. Parameters given by the
program for each stream: start, **compressed length `L`** (number of stream bytes that make up the stream),
**bias `B`** (a byte added to every output byte; 0 for all tile sets) and destination.

The stream is a sequence of blocks, each starting with a control byte `c`:

| Control byte `c` | Block | Output |
|---|---|---|
| $00-$7F | `c`, then `c + 1` literal bytes | each literal byte plus `B` (mod 256), in order |
| $80-$FF | `c`, then one value byte | the value plus `B` (mod 256), repeated 257 − `c` times ($FF gives 2, $80 gives 129) |

The stream ends when all `L` bytes have been used, which can happen part-way through a block:

- a literal block cut short by the end of the stream outputs only those of its literal bytes that lie within
  the `L` bytes;
- a control byte that is the last of the `L` bytes outputs nothing;
- a run block whose value byte is the last of the `L` bytes outputs its whole run.

Notes:

- The decoded size is not stored anywhere. It is whatever the stream produces when exactly `L` bytes have been
  consumed. It need not be a multiple of 16: several tile streams end a few bytes into the next tile, and later
  loads (or nothing) occupy the rest. An importer should decode the whole stream and cut tiles from the
  output; a final partial tile is padding, not a real tile, unless another blob completes it on the hardware.
- The bias is used only by nametable streams whose tiles were loaded at a variable position (the title
  nametable uses B = $64; one menu nametable uses the current tile base, see 5.9).

**Worked example** (title-screen attribute stream, bank 2:$ACAB, ROM 0x12CAB, L = 16, B = 0, destination $23C0):

| Stream bytes | Meaning | Output |
|---|---|---|
| `FE 00` | run, 257 − 254 = 3 × `00` | `00 00 00` |
| `00 F0` | literal, 1 byte | `F0` |
| `FD 00` | run, 4 × `00` | `00 00 00 00` |
| ... | four more runs and literals | |

In all, the 16 stream bytes give 32 output bytes (attribute rows 0-3). With a bias, e.g. the title nametable (bank 2:$AB99,
B = $64): its first bytes `D8 00` are a run of 41 × ($00 + $64) = 41 × `64`, then `03 01 02 03 04` a literal of
4 bytes `65 66 67 68`.

### 2.4 One-bit font glyphs **[V]**

7 bytes per glyph. Each glyph becomes one tile: plane-0 row 0 = `M`, rows 1-7 = glyph bytes 0-6 XOR `M`; plane 1
all zero. `M` (a mask) is 0 in every use seen, which gives colour-1 text with an empty top row.

**Glyph order** (read from the glyph shapes; [V] for the placement rule below):

- Letter font, ROM 0x15BB6, 32 glyphs (31 are ever loaded): 0-25 = A-Z, 26 = a small diagonal cross (×),
  27 = hyphen, 28 = full stop, 29 = comma, 30 = the two letters "ED" squeezed into one tile; glyph 31 (ROM
  0x15C8F, never loaded) = colon.
- Digit font, ROM 0x15C96, 12 glyphs: 0-9 = digits 0-9, 10 = copyright sign ©, 11 = trademark sign (TM).

**Placement:** the main copy is loaded so that tile number = character code: digits at tiles $30-$3B
(PPU $1300), letters at tiles $41-$5F (PPU $1410), so text bytes in ASCII ('A' = $41, '0' = $30) are tile
numbers directly; a space (byte $20) is written as tile 0. Codes $3A and $3B (ASCII ':' and ';') show © and
TM; $5B-$5F show ×, hyphen, full stop, comma and "ED". A second copy at PPU $1C00/$1D10 gives tile = code +
$90.

### 2.5 Raw tiles XOR a mask **[C]**

Some loads copy raw tiles XOR-ed with a mask byte; the mask is 0 in every load found, so the data is plain
raw tiles (2.1).

## 3. Tiles the program makes itself

Not stored as tiles in the ROM; listed so an importer does not look for them and a renderer can recreate them.

| Tile | Content | Status |
|---|---|---|
| Tile 0 of table $0000 | cleared to zero at the start of gameplay and some screens | [V] |
| Solid tile, tile $FE of table 0 ($0FE0), shop and equip list (5.10) | all 16 bytes $FF (every pixel colour 3) | [V] |
| Star dot, tile $FF of either table ($0FF0 / $1FF0) | one pixel of value 1 at the top left (plane-0 row 0 = $80), rest zero | [V] |
| Split marker, tile $FF of table 1 ($1FF0), approach screen and gameplay | a hardware aid, never visible. Sprite 0 sits at screen (8, 192) with the one-dot tile $FF of table 0, behind the background, and its collision with a background pixel tells the program where the panel split goes. To be sure there is a background pixel there, every frame the program takes the page-0 background cell shown at screen (8, 192), i.e. column (x + 8) / 8, row (y / 8 + 24) mod 30 where x = RAM $02 and y = RAM $03 are the background scroll (both rounded down), copies its tile to tile $FF of table 1 with the plane-0 bit set at row y mod 8, column x mod 8 (bit mask from a table at bank 0:$A112), and sets that cell to $FF during vertical blank. In the blank lines 193-195 the cell gets its original tile back. Line 192 is the last playfield line, so the marker pixel is visible on screen at (8, 192), drawn in colour 1 (or 3) of that cell's palette: this is the single pixel that differs from a plain render of the map on every frame. It is a hardware artefact (the remake drops it) | [V] |
| Gauge pieces, tiles $FA and $FB of table 1 ($1FA0-$1FBF), gameplay HUD | the partly filled end of the two status-panel bars: tile $FA for the bar whose level is RAM $05CE, tile $FB for RAM $C9. All 16 bytes of the tile (both planes, so colour 3) are one entry of the 8-byte table at bank 0:$9973 (ROM 0x01973; ripped), entry = (level / 2, rounded down) mod 8 | [C] |
| Mirrored tiles, tiles $A0-$A3 of table 1 ($1A00-$1A3F) | written in the screen that shows the "Space Loonies" logo outside the attract sequence (probably game over): tile $A0 = tile 0 of bank 1:$E345, $A1 = the same mirrored left to right (bit order of every byte reversed), $A2 = tile 1 of $E345, $A3 = it mirrored | [C] |
| Terrain copies for the sprites (level 26 only, 5.7.5) | tiles $27-$5C of table 1 (the terrain set, 5.7 item 3) are read back and written **unchanged** to tiles $27-$5C of table 0, so that sprites can draw pieces of terrain. An earlier text said the 16 bytes were written in reverse order ("upside down, colours 1 and 2 swapped"); that was wrong: the loop stores each tile's bytes into a scratch area from the end and writes them back from the end, which is a straight copy | [V] (CHR RAM: all 54 tiles equal to the terrain set's tiles $27-$5C in every dumped step) |

### 3.1 Score pop-up pictures: sprite tiles $BC and $D0 of table 0 **[V]**

The floating "bonus" sign (`behaviour.md` 6.2: object state 6, frame $A0, two sprites with tiles $BC and $D0, attribute 1
(palette 1), at dx -4 and +4) draws these two tiles of sprite pattern table $0000. Nothing in the level loads writes them
(the start of play zeroes both, [V] writes of $00 over $0BC0-$0BCF and $0D00-$0D0F before step 1, so a sign shows nothing before the first job); the program writes
them **during play, one tile per frame, in the blank lines of the status-panel split**:

- Trigger: RAM $D2 bit 6 (set by whatever makes a marker, `behaviour.md` 6.2). Source: the nine pairs of ROM 0x08100
  (4.1), pair `n` = ROM 0x08100 + 32 n with n = RAM $D2 AND $0F; each tile is 16 raw bytes (plane 0 rows 0-7, then plane 1).
- Frame 1 (bit 5 of $D2 clear): $D2 bit 5 is set; the second 16 bytes (ROM 0x08100 + 32 n + 16) go to PPU $0D00-$0D0F (tile $D0).
- Frame 2 (bit 5 set): the first 16 bytes go to $0BC0-$0BCF (tile $BC) and $D2 AND $8F clears bits 4-6.
  The two uploads are on consecutive frames (logic step k's second frame and step k+1's first).
- The tiles then stay until the next job or the next level load; there is no per-marker copy. Two signs on screen share
  the picture of the last job.
- Pictures (4 x 8 px, digits 4 px wide, colours 1-3 of sprite palette 1): n = 0 "25", 1 "50", 2 "100", 3 "200",
  4 "400", 5 "1000", 6 "2000", 7 "5000", 8 "1UP". Seen on screen **[V]**: "25", "2000", "1UP"; the rest read from the tile shapes.
- The job runs only in the split variant of normal play (it is why the tractor beam's picture job skips those two frames, `player.md`). The remake has no split, so it may write both tiles at once when the marker is made.

## 4. Catalogue of graphics blobs

Every blob found, by ROM offset. Lengths are hex. "RLE" = run-length stream (2.3) with `L` = compressed length;
"raw" length in bytes; "font" = number of glyphs. "Out" = decoded bytes. Destination is the PPU address of the
first output byte when fixed.

### 4.1 Bank 1 (mostly raw sprite tiles)

| ROM | Bank:addr | Format | Length | Out | Destination | Use | Status |
|---|---|---|---|---|---|---|---|
| 0x08100 | 1:$8100 | raw | 0x120 (18 tiles) | | tiles $BC and $D0 of table 0 | 9 two-tile sprite pictures of point values (score pop-ups); pair `k` = ROM 0x08100 + 32k, chosen by RAM $D2 bits 0-3 (n = 0-7: "25", "50", "100", "200", "400", "1000", "2000", "5000"; 8: "1UP"). During play the program uploads one tile per frame in the blank lines of the status-panel split: first frame the second half of the pair → $0D00, next frame the first half → $0BC0 (3.1) | [V] |
| 0x0CB45 | 1:$CB45 | raw | 0x80 (8 tiles) | | $0F70 | common sprites (title and gameplay) | [V] |
| 0x0CBC5 | 1:$CBC5 | raw | 0x400 (64 tiles) | | $0010 | gameplay: jetpod sprites, pod type 0 | [V] |
| 0x0CFC5 | 1:$CFC5 | raw | 0x400 (64 tiles) | | $0010 | gameplay: jetpod sprites, pod types 1 and 3 | [V] |
| 0x0D3C5 | 1:$D3C5 | raw | 0x3C0 (60 tiles) | | ? | look like further pod frames; no load of them found | [H] |
| 0x0D785 | 1:$D785 | raw | 0x400 (64 tiles) | | $0010 | gameplay: jetpod sprites, pod type 2 | [V] |
| 0x0DB85 | 1:$DB85 | raw | 0x3C0 (60 tiles) | | ? | look like further pod frames; no load of them found | [H] |
| 0x0DF45 | 1:$DF45 | raw | 0x100 (16 tiles) | | $0DF0 | common sprites | [V] |
| 0x0E045-0x0F4C4 | 1:$E045-$F4C4 | raw | | | object area from $0410 | object (enemy) sprite sets, see 6.4; inside it the common sprite blocks below | [V] |
| 0x0E0B5 | 1:$E0B5 | raw | 0x20 | | $0C80 | common sprites | [V] |
| 0x0E255 | 1:$E255 | raw | 0x50 | | $0CA0 | common sprites | [V] |
| 0x0E2A5 | 1:$E2A5 | raw | 0xD0 | | $0D10 | common sprites | [V] |
| 0x0E345 | 1:$E345 | raw | 0x20 | | $1A00-$1A3F with mirrors | see section 3 (mirrored tiles) | [C] |
| 0x0F185 | 1:$F185 | raw | 0x120 (18 tiles) | | $0010 | title sprites; also Jetman's own figure in play: written over the pod's first 18 tiles when the pod is destroyed (5.7.4) | [V] |
| 0x0F4C5 | 1:$F4C5 | RLE | 0x1B2 | 0x1C1 | $0000 | level 26 only (RAM $3B = 26) and only when RAM $9A bit 7 is set (the final level's flag, `game-flow.md` 7.4): the final level's own sprite tiles, 5.7.5. The output is 28 whole tiles ($00-$1B) and one byte more, which lands in byte 0 of tile $1C | [V] |
| 0x0F6AE | 1:$F6AE | raw | 0x1A0 (26 tiles) | | tiles $00-$19 of table 0 | uploaded two tiles per frame through the per-frame update buffer while the frame counter RAM $5D is below 13: on frame `f` the 32 bytes at ROM 0x0F6AE + 32f go to PPU 32f. Run at the start of the game-over scene: `$5D` is 0 at its first frame, and the 26 tiles are complete when `$5D` = 14 (checked frame by frame in Mesen, 2026-10-17); they are the eight letters of the "GAME OVER" sprites of `scenes.md` 12 | [V] |
| 0x0F91E | 1:$F91E | raw | 0x60 | | object area | object sprite set (list B) | [V] |
| 0x0F97E | 1:$F97E | raw | 0xF0 | | object area | object sprite set (list B) | [V] |
| 0x0FA6E | 1:$FA6E | RLE | 0xD4 | 0x1B0 | $1E40 | status panel tiles (approach screen, gameplay HUD, surface menu) | [V] |
| 0x0FA74 | 1:$FA74 | RLE | 0x86 | 0xF0 | $1E50 | a second stream starting inside the previous one: the 15 large-digit tiles $E5-$F3 (money, count) of the shop and equip list (5.10) | [V] |
| 0x0FB42 | 1:$FB42 | raw | 0x50 | | $1D60 | status panel tiles | [V] |
| 0x0FB92 | 1:$FB92 | raw | 6 × 0x60 | | $0EF0 | six 6-tile blocks; block chosen by RAM $D1: 0 → no load, 1-4 → blocks 0-3, 5-8 → blocks 2-5 (value − 3). So 3 and 5 give block 2, 4 and 6 block 3; whether $D1 ever takes both values of such a pair is not known | [V] blocks 0-3, [C] 4-5 |
| 0x0FDD2 | 1:$FDD2 | raw | 0x20 | | $0F50 | common sprites | [V] |
| 0x0FDF2 | 1:$FDF2 | raw | 0x20 | | $0BA0 | common sprites | [V] |
| 0x0FE12 | 1:$FE12 | raw | 0x10 | | $0DE0 | common sprites | [V] |
| 0x0FE22 | 1:$FE22 | raw | 0xB0 | | $0BD0 | common sprites | [V] |
| 0x0FE52 | 1:$FE52 | raw | 0x20 | | $0B80 | title sprites | [V] |
| 0x0FF2F | 1:$FF2F | RLE | 0x19 | 0x60 | $0800 (title), $0E00 (attract) | sprite tiles | [V] |

### 4.2 Bank 2 (fonts, title, terrain)

| ROM | Bank:addr | Format | Length | Out | Destination | Use | Status |
|---|---|---|---|---|---|---|---|
| 0x112EE | 2:$92EE | RLE | 0x21 | 0x40 | $23C0 | attributes, the shop (5.10) | [V] |
| 0x1130F | 2:$930F | RLE | 0x1F | 0x40 | $23C0 | attributes, the equip list (5.10) | [V] |
| 0x1134C | 2:$934C | RLE | 0x10C | 0x340 | $2000 | nametable, the equip list (5.10) | [V] |
| 0x122DE | 2:$A2DE | RLE | 0x8BB | 0x9C0 | $1640 | title logo tiles | [V] |
| 0x12B99 | 2:$AB99 | RLE, B = $64 | 0x112 | 0x1C0 | $2000 | title nametable | [V] |
| 0x12CAB | 2:$ACAB | RLE | 0x10 | 0x20 | $23C0 | title attributes | [V] |
| 0x12DA5 | 2:$ADA5 | RLE | 0x242 | 0x261 | $1800 | screen of bank 2 entry $8036: the twelve pieces of the golden warpship (tiles $80-$A5 of table 1), shown with message 47 or 48; no nametable in ROM, the program lays the pieces out as 2 × 2 cells (`scenes.md` 10, "Warpship pieces") | [V] decode and use |
| 0x12FE7 | 2:$AFE7 | raw | 0x16 | | $23D1-$23E6 | attributes for the same screen | [C] |
| 0x12FFD | 2:$AFFD | RLE | 0x372 | 0x3F1 | $1000 | **the interplanetary flight's background tiles** (bank 2 entry $8033): the 63 tiles ($00-$3E, plus one trailing byte) of the mothership picture, which printer message 49 lays out as 14 × 7 tiles (`text.md` 3, `scenes.md` 5). No nametable or attributes in ROM. Loaded by the flight's set-up after the three sprite loads (`scenes.md` 5). The earlier "between the approach screen and the planet surface" guess was wrong | [V] decode, [V] use (flight reproduced pixel for pixel; every tile code of message 49 lies in $00-$3E) |
| 0x1336F | 2:$B36F | RLE | 0xDDD | 0x1000 | $1000 | terrain tile set, graphics types 1 and 6 | [V] |
| 0x1414C | 2:$C14C | RLE | 0xB57 | 0xB51 | $1000 | terrain tile set, graphics types 0 and 4 | [V] |
| 0x14CA3 | 2:$CCA3 | RLE | 0x1EB | 0x201 | $12A0 | terrain extra, graphics type 3 | [V] |
| 0x14E8E | 2:$CE8E | RLE | 0x11F | 0x120 | $1820 | terrain extra, graphics type 3 | [V] |
| 0x14FAD | 2:$CFAD | raw | 0x130 (19 tiles) | | $15C0 | terrain extra, graphics type 5 | [V] |
| 0x150DD | 2:$D0DD | raw | 0xF0 (15 tiles) | | $1B00 | terrain extra, graphics type 5 | [V] |
| 0x151CD | 2:$D1CD | RLE | 0x379 | 0x381 | $12D0 | terrain extra, levels 1 and 8 | [V] |
| 0x15546 | 2:$D546 | RLE | 0x256 | 0x341 | $0860 | title sprite tiles | [V] |
| 0x1579C | 2:$D79C | RLE | 0x266 | 0x291 | $1000 | the shop (5.10, RAM $63 bit 7 set) | [V] |
| 0x15A02 | 2:$DA02 | RLE | 0x1B4 | 0x200 | $0000, and $1000 in the equip list | the shop and equip list (5.10) | [V] |
| 0x15BB6 | 2:$DBB6 | font | 31 glyphs (28 in one load) | 31 tiles | variable | font: letters (order in 2.4) | [V] |
| 0x15C96 | 2:$DC96 | font | 12 glyphs | 12 tiles | variable | font: digits, ©, TM (order in 2.4) | [V] |
| 0x15CEA | 2:$DCEA | RLE | 0x56 | 0x60 | variable | font extras (box frame pieces) | [V] |
| 0x15D40 | 2:$DD40 | raw | 0x6C0 (108 tiles) | | $1600 and $0800 | the 12 item pictures (9 tiles each) of the shop and equip list (5.10); picture n is also the weapon icon of the gameplay panel (1.2) | [V] |
| 0x16400 | 2:$E400 | raw | 0x120 (18 tiles) | | after the previous | the two mapping-device pictures (items 12, 13), shop (5.10), when (level index AND 3) ≠ 3 | [V] |
| 0x16520 | 2:$E520 | raw | 0x120 (18 tiles) | | after the previous | the two jetpod pictures in their place, when (level index AND 3) = 3 | [C] |
| 0x16640 | 2:$E640 | RLE | 0xB19 | 0xBF1 | $1000 | terrain tile set, graphics types 2, 3 and 5 | [V] |
| 0x17159 | 2:$F159 | RLE | 0x24C | 0x330 | $1CC0 | gameplay background tiles common to all terrain (not loaded on level 26 and the warp levels; it holds older versions of panel tiles $E4, $F6, $FC-$FE, 5.7.6) | [V] |

The 7 bytes at ROM 0x15C8F-0x15C95, between the two fonts, form one more glyph (a colon-like shape) that no
load found includes. [C]

### 4.3 Bank 4 (mothership launch scene)

| ROM | Bank:addr | Format | Length | Out | Destination | Use | Status |
|---|---|---|---|---|---|---|---|
| 0x2159C | 4:$959C | RLE | 0x145 | 0x170 | $0000 | launch sprites, pod type 0 | [V] |
| 0x216E1 | 4:$96E1 | RLE | 0x15A | 0x170 | $0000 | launch sprites, pod type 1 | [V] |
| 0x2183B | 4:$983B | RLE | 0x153 | 0x170 | $0000 | launch sprites, pod type 2 | [V] |
| 0x2198E | 4:$998E | RLE | 0x56E | 0x791 | $0170 | launch sprites | [V] |
| 0x21EFC | 4:$9EFC | RLE | 0x23 | 0x40 | $23C0 | launch attributes | [V] |
| 0x21F1F | 4:$9F1F | RLE | 0x33F | 0x3C0 | $2000 | launch nametable | [V] |

The pod type is RAM $DC AND 3 (game state: the jetpod model the player has; types 0-2 occur, the password
decoder rejects 3, so type 3 never reaches the launch scene). The launch table (bank 4:$9590: start low, start
high, length low, length high, each a 3-byte column for types 0-2) has no entry for type 3. In gameplay type 3
would use the same set as type 1 (6.1). [C]

### 4.4 Bank 5 (planet-surface pictures, equipment pictures)

Seven surface pictures, numbered 0-6 (number 3 is empty). Each has a tile set for table $1000, one for table
$0000, extra tiles at $1A80, a raw nametable and raw attributes, plus 6 common tiles from ROM 0x2840A loaded at
$1A30. Tables in bank 5 (7 entries each, indexed by picture number) give the streams: tiles-$1000 start
low/high at $811D/$8124, length low/high $812B/$8132; tiles-$0000 start $8139/$8140, length $8147/$814E;
nametable start $8155/$815C, length $8163/$816A; attributes start $8171/$8178, length $817F/$8186; extra tiles
start $818D/$8194, length $819B/$81A2. [V]

Picture 6 is stored in bank 7. Its two tile-stream entries hold $FF in all four bytes; on reading a start high
byte of $FF the program loads that stream from bank 7 instead, through bank 7's entries $8000 (table-1 stream)
and $8003 (table-0 stream), which take start and length from a 2-entry table at bank 7:$844B (start low ×2 at
$844B, start high ×2 at $844D, length low ×2 at $844F, length high ×2 at $8451): entry 0 = bank 7:$905A,
L $795; entry 1 = bank 7:$97EF, L $C41. Its extra tiles, nametable and attributes are ordinary bank 5 entries.
[V]

**Load order** (bank 5 entry $8000) [V]: common tiles 0x2840A → $1A30 (0x60 bytes, tiles $A3-$A8), table-1
stream → $1000, table-0 stream → $0000, extra stream → $1A80, nametable → $2000, attributes → $23C0. Final
contents of $1A30-$1AFF:

| Picture | $1A30-$1A7F (tiles $A3-$A7) | $1A80-$1AFF (tiles $A8-$AF) |
|---|---|---|
| 0 | $1A30-$1A50 from the table-1 stream (it outputs 0xA51 bytes: tiles $A3, $A4 and the first byte of $A5); $1A51-$1A7F common tiles | extra stream (0x281 bytes, to $1D00) |
| 1 | common tiles | extra stream (0x90 bytes, to $1B0F) |
| 2, 4, 5 | common tiles | extra stream (0x281 bytes, to $1D00) |
| 6 | common tiles | $1A80-$1ABF extra stream (0x40 bytes); $1AC0-$1AFF keep what the scene loaded before the picture |

The 6th common tile ($1A80-$1A8F) is always overwritten by the extra stream. Tiles of table 1 that the
pictures' nametables use: picture 0 rows 0-15 use tiles up to $A4 (so its own $A3/$A4), the other pictures stay
below $A3; table 1 tiles from $A3 on are used by the sprites of the surface scene (sprites use table $1000
there). Every tile a picture's nametable uses (rows 0-15 in table 1, rows 16-29 in table 0) is written by the
picture's own loads. [V]

| Picture | Tiles $1000 (RLE) | Tiles $0000 (RLE) | Extra at $1A80 (RLE) | Nametable (raw 0x3C0) | Attributes (raw 0x40) |
|---|---|---|---|---|---|
| 0 | 0x29D29, L 0x8C3, out 0xA51 | 0x2A5EC, L 0xC00, out 0xD21 | 0x29A7E, L 0x239, out 0x281 | 0x287E3 | 0x2828A |
| 1 | 0x2BDC9, L 0x533, out 0x6C0 | 0x2C2FC, L 0xD80, out 0xEF0 | 0x29CB7, L 0x72, out 0x90 | 0x28F03 | 0x282CA |
| 2 | 0x2B1EC, L 0x7CA, out 0x901 | 0x2B9B6, L 0x413, out 0x471 | 0x29A7E, L 0x239, out 0x281 | 0x28BA3 | 0x2830A |
| 4 | 0x2D07C, L 0x814, out 0x920 | 0x2D890, L 0x961, out 0xAC0 | 0x29A7E, L 0x239, out 0x281 | 0x292C3 | 0x2834A |
| 5 | 0x2E1F1, L 0x6A0, out 0x720 | 0x2E891, L 0xA97, out 0xB41 | 0x29A7E, L 0x239, out 0x281 | 0x28423 | 0x2838A |
| 6 | 0x3905A, L 0x795, out 0x9A0 (bank 7) | 0x397EF, L 0xC41, out 0x1000 (bank 7) | 0x29A43, L 0x3B, out 0x40 | 0x29683 | 0x283CA |
| all | | | common: 0x2840A, L 0x19, out 0x60 → $1A30 | | |

All [V]. Picture 1's extra stream is also loaded at $0A80 by bank 5 entry $800C, as part of menu screen A (5.9). [C]

Item pictures (bank 5 entry $8003; RAM $04 = item kind 0-7 selects through the 8-byte map at bank 5:$81E0,
values `00 FF FF 00 01 FF 02 02`, $FF = nothing), each RLE to $1810 (tiles $81 on of table 1). They are loaded
during gameplay, by the object code, when an item object of that kind is set up, so the item is drawn with
background tiles (see `objects.md` for the objects) [C]:

| Picture | ROM | L | Out | Status |
|---|---|---|---|---|
| 0 (items 0 and 3) | 0x2F514 | 0x194 | 0x1C0 | [V] |
| 1 (item 4) | 0x2F6A8 | 0x1E7 | 0x201 | [V] |
| 2 (items 6 and 7) | 0x2F328 | 0x1EC | 0x220 | [V] |
| 3 (unused by the map) | 0x2F88F | 0x192 | 0x1A1 | [C] |

### 4.5 Bank 6 (approach screen, attract, menus, warp levels)

| ROM | Bank:addr | Format | Length | Out | Destination | Use | Status |
|---|---|---|---|---|---|---|---|
| 0x30973 | 6:$8973 | RLE | 0xEC5 | 0x1000 | $1000 | attract sequence: floating Jetman, background | [V] |
| 0x31838 | 6:$9838 | RLE | 0xB12 | 0xC00 | $0000 | attract sequence: floating Jetman, more tiles | [V] |
| 0x3234A | 6:$A34A | raw | 0x790 | | nametable, 16 bytes per row | attract: animation frames of the Jetman picture, 11 frames × 11 rows × 16 bytes; row `r` (0-10) of frame `f` = 16 bytes at 0x3234A + 176f + 16r, written to nametable $2000 + 32r (columns 0-15) | [C] |
| 0x32ADA | 6:$AADA | raw | 0x84 | | attributes | the same frames, 12 attribute bytes per frame: with row `r`, the 4 bytes at 0x32ADA + 12f + 4 × (r div 4) go to $23C0 + 8 × (r div 4) (attribute columns 0-3) | [C] |
| 0x32E56 | 6:$AE56 | raw | 18 blocks × 0x120 | | $1810, $1930 | equipment-block loader (5.8), **not used by the shop**: block 0 always at $1810, block (item + 1) at $1930; the analysis screen loads block (list + 1), so blocks 1-17 are used | [V] blocks 0-7, [H] 8-17 |
| 0x34296 | 6:$C296 | RLE | 0x20E | 0x400 | $2000 | screen of bank 6 entry $8012 (a comet picture; scene not identified, palette preset 21): nametable + attributes | [V] decode |
| 0x344A4 | 6:$C4A4 | RLE | 0xC89 | 0xF50 | $1000 | the same screen: tiles | [V] decode |
| 0x3512D | 6:$D12D | RLE | 0x1D0 | 0x1E1 | $0000 | the same screen: sprite tiles $00-$1D, the 30 lettering sprites (`scenes.md` 10.1) | [V] decode, [V] use |
| 0x35394 | 6:$D394 | RLE | 0x42F | 0x501 | $0800 | equipment/shop screen tiles | [V] |
| 0x357C3 | 6:$D7C3 | RLE | 0x312 | 0x3E1 | $0C00 | "Federation of Space Loonies" logo sprites (attract) | [V] |
| 0x35AD5 | 6:$DAD5 | RLE | 0xC4 | 0x2EF | $2000 | approach screen nametable | [V] |
| 0x35B99 | 6:$DB99 | RLE | 0x3A3 | 0x3E1 | $1800 | approach screen background tiles | [V] |
| 0x35F3C | 6:$DF3C | RLE | 0x11E | 0x131 | $0010 | approach screen sprite tiles | [V] |
| 0x36149 | 6:$E149 | RLE, B = tile base | 0x1A9 | 0x301 | $2000 | menu screen A (bank 6 entry $8006) nametable | [V] decode |
| 0x362F2 | 6:$E2F2 | RLE | 0x24 | 0x30 | $23C0 | menu screen A attributes | [V] decode |
| 0x36316 | 6:$E316 | RLE | 0x22A | 0x300 | $2000 | menu screen D (bank 6 entry $800F, equipment/shop) nametable | [V] decode |
| 0x36540 | 6:$E540 | RLE | 0x1E8 | 0x300 | $2000 | menu screen C (bank 6 entry $800C, message screens) nametable | [V] decode |
| 0x36728 | 6:$E728 | raw | 0x31 | | $23C0 | menu screen C attributes | [V] decode |
| 0x36758 | 6:$E758 | raw | 0x31 | | $23C0 | menu screen D attributes | [V] decode |
| 0x36788 | 6:$E788 | RLE | 0xB1 | 0x240 | $2080 | menu screen B (bank 6 entry $8009: password entry and message boxes) nametable | [V] |
| 0x36839 | 6:$E839 | RLE | 0x16 | 0x28 | $23C8 | menu screen B attributes | [V] |
| 0x3684F | 6:$E84F | RLE | 0x27C | 0x401 | $1000 + 16 × base | menu frame and font tiles | [V] |
| 0x36B43 | 6:$EB43 | raw | 12 sets × 0x110 (17 tiles) | | $1810 | warp levels 29-41 background tiles, set = level − 29 (level 41 uses set 10) | [V] |

Equipment/shop blocks: block `k` starts at ROM 0x32E56 + 0x120 × k. Blocks 1-7 were checked for items 0-6; the
data runs on to ROM 0x34295, which is room for blocks up to 17. [H]

### 4.6 Bank 7 (launch background, picture 6, ending, a backdrop)

| ROM | Bank:addr | Format | Length | Out | Destination | Use | Status |
|---|---|---|---|---|---|---|---|
| 0x3905A | 7:$905A | RLE | 0x795 | 0x9A0 | $1000 | surface picture 6, tiles $1000 | [V] |
| 0x397EF | 7:$97EF | RLE | 0xC41 | 0x1000 | $0000 | surface picture 6, tiles $0000 | [V] |
| 0x3A430 | 7:$A430 | RLE | 0x3DD | 0x591 | $1000 | backdrop screen (bank 7 entry $8006; used for levels 0, 4 and 8; context unknown) | [V] decode |
| 0x3A88D | 7:$A88D | raw | 0x1C0 (28 tiles) | | $0800 | the same backdrop | [V] decode |
| 0x3D379 | 7:$D379 | RLE | 0x2EA | 0x3C1 | $2000 | ending picture nametable | [V] decode |
| 0x3D663 | 7:$D663 | RLE | 0xB5C | 0xDA1 | $1000 | ending picture tiles | [V] decode |
| 0x3E1BF | 7:$E1BF | RLE | 0x20F | 0x241 | $1DC0 | ending picture tiles | [V] decode |
| 0x3E3CE | 7:$E3CE | RLE | 0xB0A | 0xD50 | $0000 | ending picture tiles | [V] decode |
| 0x3EED8 | 7:$EED8 | RLE | 0x245 | 0x2A0 | $0D60 | ending picture tiles | [V] decode |
| 0x3F11D | 7:$F11D | RLE | 0x2E | 0x40 | $23C0 | ending picture attributes | [V] decode |
| 0x3F15F | 7:$F15F | RLE | 0xE19 | 0xFC1 | $1000 | launch scene background tiles | [V] |

"[V] decode" = the loader was forced to run in Mesen from the title screen and its output matched our decoder;
the scene itself was not reached in play.

## 5. What is loaded when

Each scene starts from the CHR RAM left by the previous one; only the listed areas are rewritten. Loads in
order; a later load overwrites an earlier one where they overlap.

### 5.1 Power-on and licence screen [V]

CHR cleared; font: digits (ROM 0x15C96) at $1300, letters (0x15BB6) at $1410, font extras (0x15CEA) right
after the letters ($1600).

### 5.2 Title screen [V]

CHR cleared again, then: font as 5.1 at $1300/$1410/$1600 and again at $1C00/$1D10/$1F00, extras also at
$1100; logo tiles 0x122DE → $1640; title nametable 0x12B99 (B = $64) → $2000; attributes 0x12CAB → $23C0;
sprite tiles 0x0FF2F → $0800, 0x15546 → $0860, 0x0FE52 → $0B80; common sprite set (6.1) with 0x0F185 at $0010.

### 5.3 Attract sequence [V]

Sun and logo animation reuse the title tiles. "Federation of Space Loonies" logo: 0x357C3 → $0C00. Floating
Jetman: 0x30973 → $1000, 0x31838 → $0000, 0x0FF2F → $0E00; the picture's nametable rows are then animated from
0x3234A / 0x32ADA (4.5): a 16-column, 11-row window at the top-left of the nametable, while the scroll moves.
[C for the animation]

### 5.4 Approach screen ("APPROACHING PLANET") [V]

Font as 5.2 (both copies, extras at $1100), 0x35B99 → $1800, 0x35F3C → $0010, nametable 0x35AD5 → $2000,
status panel 0x0FA6E → $1E40 and 0x0FB42 → $1D60. The split marker is generated (section 3).

### 5.5 Planet-surface picture (landing / mothership menu) [V]

Loads in this order (observed): the level's gameplay background (5.7 steps 3-4: terrain set → $1000, common
background → $1CC0), letters glyphs 0-27 (A-Z, ×, hyphen) at $1020 (tiles $02-$1D), font extras at $11E0,
status panel 0x0FA6E → $1E40 and 0x0FB42 → $1D60, digits at $1300, letters at $1410, extras at $1600, digits at
$1C00, letters at $1D10, extras at $1F00 and at $1100, then the picture (4.4, in its load order): picture number
= the 7-entry map at bank 2:$9567 (`01 02 00 04 05 00 06`) indexed by the level's graphics type (6.2).

What survives at the top of table 1: the letters copy at $1D10-$1EFF (tiles $D1-$EF, used by the menu box
text: tile = character code + $90) and the extras at $1F00-$1F5F (tiles $F0-$F5); those two loads come after the
status-panel tiles and overwrite all of $1D60-$1DAF and $1E40-$1F5F, so of the status-panel tiles only
$1F60-$1FEF (from 0x0FA6E) remains. The picture streams end at or below $1D00 and do not reach these. The surface
scene shows no status panel (1.1), so the panel tiles are not needed there. The background uses table $1000 for
rows 0-15 and $0000 for rows 16-29 (1.1). [V]

### 5.6 Mothership launch [V]

Pod sprites by pod type (4.3) → $0000, 0x2198E → $0170, background 0x3F15F → $1000, nametable 0x21F1F → $2000,
attributes 0x21EFC → $23C0.

### 5.7 Gameplay (each level) [V]

1. Tile 0 of table 0 cleared (section 3).
2. Status panel 0x0FA6E → $1E40, 0x0FB42 → $1D60.
3. Terrain set by the level's graphics type (6.2) → $1000: types 0 and 4 → 0x1414C; types 1 and 6 → 0x1336F;
   types 2, 3 and 5 → 0x16640 (start/length table at bank 2:$94E4: 7 length-low, 7 length-high, 7 start-low,
   7 start-high bytes). Not loaded on warp levels (29-41).
4. Common gameplay background 0x17159 → $1CC0 (not on warp levels, **and not on level 26** either: there the panel stream (item 2) is written a second time at this point instead, 5.7.6; levels 27 and 28 do load it).
5. Extras: levels 1 and 8 → 0x151CD at $12D0; otherwise type 3 → 0x14CA3 at $12A0 and 0x14E8E at $1820;
   type 5 → raw 0x14FAD at $15C0 and 0x150DD at $1B00. Warp levels: 17 tiles from 0x36B43 + 0x110 × set at
   $1810 (4.5).
6. Common sprites (6.1) with the pod set of the current pod type at $0010.
   The fitted special weapon's picture (item 0-5: 0x15D40 + 0x90 × item, 0x90 bytes) → $1C30, the status panel's
   weapon icon (1.2); nothing when no special weapon is fitted. Its place in the order is not known; it is in
   place when gameplay starts. [V] for source and place, not for the order.
7. Object sprite sets of the level (6.4) from $0410 (not when RAM $9A bit 7 is set, i.e. on level 26, which registers one tile range instead, 6.4).
8. Level 26 only: 0x0F4C5 → $0000, then tiles $27-$5C of table 1 copied (unchanged) to table 0 (section 3). The result and what each tile is used for: 5.7.5. [V]
9. During play: gauge tiles, the split marker (section 3), the point pop-up tiles (4.1) and, on warp levels,
   the mesh tile (5.7.2).

#### 5.7.1 Tiles no gameplay load writes

Table-1 tiles that the level's own loads leave unwritten keep whatever an earlier scene left there (in practice
the launch scene's background tiles, or the second digit-font copy at $1C00). Comparing what each block set's
maps use (`levels.md` 4.1) with what is loaded [C]:

| Levels | Unwritten table-1 tiles | Used by the level's map? |
|---|---|---|
| graphics types 1, 6 (block set 2; terrain 0x1336F fills the whole table) | none | |
| types 2, 3, 5 (set 0; terrain 0x16640 ends 1 byte into tile $BF) | $BF (all but its first byte) to $CB | no (maps use up to $BE) |
| types 0, 4 (set 1; terrain 0x1414C ends 1 byte into tile $B5) | $B5 (all but its first byte) to $CB | yes on levels 2, 4, 7 and 9 (tiles $C0, $C1, $C2, $C4, $C7, levels 4, 7, 9 only $C1). Whether those blocks can be seen in play is not known; Mesen showed leftover digit glyphs in those tiles on level 4 |
| warp levels 29-41 (no terrain, no common background) | everything except $81-$91 (warp set), $D6-$DA and $E4-$FE (status panel), $FA-$FB (gauges) and $FF (split marker) | the warp map uses $03, $04, $43, $53 and $93 besides $81-$91; whether those blocks are visible is not known |

A remake can draw unwritten tiles as blank wherever the map is not visible; the cases marked "yes" are open
questions. **Tiles $C3-$CB are not unwritten when a special weapon is fitted:** they hold the panel's weapon icon
(1.2). Set 1's block table does define two blocks (122 and 123) that use $C2, $C4 and $C7, but no map cell of levels
0-28 holds those blocks (every placed tile of every map checked, 2026-10-17: no tile from $B5 up to $CB appears
on levels 2, 4, 7 or 9), so the overlap is harmless and the "yes" above is about the block table, not the placed maps.

#### 5.7.2 Warp-level mesh tile (animated) [V]

On warp levels tile $81 of table 1 (PPU $1810, the first tile of the warp set) is rewritten every frame, which
makes its pattern drift. Re-verified frame by frame on 2026-10-20 (`warp-level` = level 33, `warp-level-37`, `warp-level-40`: the CHR RAM bytes of the tile, the
RAM copy $86-$95 and the helper slots' bytes read at the end of every frame, 2,364 frames each; and the step pictures' CHR RAM), and the rules below predict the
tile of **every** step's picture exactly (703 of 703 pictures on each of the three levels, tile bytes equal):

- **Source:** at level start the program builds 8 variants of that tile from the set's first tile (ROM
  0x36B43 + 0x110 × set, bytes 0-15, set = level - 29: level 33 uses set 4, 37 set 8, 40 set 11; plane 0 is bytes 0-7, plane 1 bytes 8-15). Variant `k` (0-7): every byte of
  the tile rotated right by `k` bits (bits leaving at the right re-enter at the left), so the pattern moves `k` pixels to the right, wrapping. The 8 variants (128 bytes, 16 each, variant k at RAM
  $03D8 + 16 k) are kept in RAM, **in addresses that other arrays of the object table also use** (see "The variant table's holes" below).
- **Each logic step, in the A pass** (the pod's drawing stage, the routine of `game-flow.md` 9.2 item 3 that also eases the F gauge; the original also runs it once at the level start,
  `game-flow.md` 7.3): with offsets `ox` and `oy` (0-7), the low 3 bits of slot 23's x low byte (RAM $0217) and y low byte (RAM $0274) **as the previous step's B pass left them**
  (the helper slots are positioned in the B pass only, below), the 16 bytes written into RAM $86-$95 are variant `ox` with its rows rotated: tile row `i` of each plane = row `(oy + i) mod 8` of
  that plane of variant `ox`, read from the RAM table. (RAM $86-$95 on other levels hold sprite palette entries 16-31; see 7.1.) **[V]** 703 of 703 A passes on level 33, with and without the
  harness's slot setup. RAM $86-$95 changes only in A passes, never in a B pass.
- **Upload and picture [V]:** the NMI of every frame copies RAM $86-$95 to the tile; so the tile in CHR RAM at the end of frame F is the RAM copy at the end of frame F - 1 (1,408 of 1,408 frames
  on level 33). A step's A pass is the frame before its B-pass frame, and the picture of step n (the frame of the next step's B pass, the harness's `step-N.png`) shows the tile built in
  the A pass of step n + 1 from the helper positions the **B pass of step n** left. In terms of the replay log: **the picture of step n shows variant (wx_n AND 7) rotated by (wy_n AND 7) rows**, with
  wx_n and wy_n the log's `wx`, `wy` of the step n line (slot 23's x and y low bytes at the end of step n). The tile in the picture of step 1 is the one built from the positions
  of step 1's B pass. (Frames before the first step show the tile built by the level-start run from the helper slots' leftover positions: level 33 without the harness's setup: x low $82, y low $7F, i.e. variant 2 rotated by 7.)
- **Frame-by-frame example (level 33, harness):** slot 23 x low goes $00, $01 (written in the B pass of step 8), y low $10, $11 (B pass of step 9)...; RAM copy: variant 0, row 0 until
  the A pass of step 9, variant 1 at step 9, then variant 1 rotated 1 row at step 10; tile in CHR RAM: one frame after each of these.
- **Offsets:** `ox` = x low byte of object slot 23 AND 7, `oy` = y low byte of slot 23 AND 7 (`objects.md`
  2.1). On warp levels slot 23 is a helper whose position is set every step from per-level tables in bank 0,
  indexed by level − 29: flags at $A86E, slot-23 x source at $A87B, slot-23 y source at $A888 (and slot 24 at
  $A895/$A8A2). A source byte `v` names a value and a scale: value (bits 0-2) 0 = the constant 5, 1 = camera x
  low (RAM $4C), 2 = camera y low ($4E), 3 = pod x low, 4 = pod y low, 5 = frame counter ($5D), 6 = the pod's
  horizontal speed fraction byte (RAM $029B), **7 = the pod's vertical speed fraction byte (RAM $02D9)** **[V]** (round 13: level 34's slot-24 y
  source $87 = value 7, scale ÷ 2; the logged low byte of slot 24's y equalled the pod's $02D9 shifted right once on every frame
  checked, with the pod thrusting; $86 of slot 24's x on the same level = value 6 ÷ 2 likewise; every value 0-7 occurs in the tables,
  7 only there); scale (the high nibble) 0 = × 1, 1 or 3 = × 4, 2 = × 2, 8, 9, A, B = ÷ 2, 4-7 and C-F = ÷ 4 (shifts,
  results kept to 8 bits; the tables use only 0-4 and 8). The unusual nibbles follow from the program's test (on bits 6-7 only), so a
  stale or out-of-range byte (Q38) decodes by the full rule. Example: level 29 (`45`, `00`): `ox` follows the frame counter divided by
  4 (one step every 4 frames), `oy` = 5 AND 7 = 5. Level 34 (`41`, `41`): both follow camera x / 4.
- Levels whose flag byte has bit 7 set (33, 35: $80; 37, 39: $C0) move slot 23 (and slot 24 when bit 6 is also set) as a homing
  helper (`objects.md` 10.6: accel 2/256 per frame, maximum 0.3125 px per frame), and the mesh follows it. On 33 and 35 the tables
  are then not applied; on 37 and 39 they still are, with a stale index, which makes the mesh jump (`behaviour.md` Q38, the exact rule in `objects.md` 10.6). The routine
  runs on every video frame (`levels.md` 6.7).
- **Starting phase:** none of its own. The level-start run (before the first step) builds the tile from the helper slots' positions as the level load left them (leftovers of the previous level in a
  play; the harness's `obj 23 0 0 16 0 0` sets x = 0, y = 16 for both helpers, which is variant 0, 0 rows); from step 1 on, the B pass writes the positions (table-driven levels) or moves
  them (homing levels) and the rules above apply.
- **Sources on a table-driven level, checked [V]:** level 40 (source bytes $21 and $22 for slot 23, $01 and $02 for slot 24, `ox`/`oy` from camera x and y low bytes doubled): wx = 2 × camera x low, wy = 2 × camera y low
  (both mod 256), slot 24's x and y = the camera's low bytes, from the camera as it stands at the end of the step, on 704 of 704 steps (the first step: camera ($00C0, $00A0), wx = $80, wy = $40, vx = $C0, vy = $A0;
  the tile of step 1 is plain variant 0, `000000FFFFFF0000FFFFFFFFFFFF0000` on level 40). Levels 33 and 37 (homing helpers) follow `objects.md` 10.6.

**The variant table's holes [V]** (the likely cause of the remake's mismatch on every warp level): the 128 table bytes at RAM $03D8-$0457 share their addresses with the object table's
arrays work 5 ($0403 + slot), work 6 ($0422 + slot) and hit points ($0441 + slot), each 31 bytes long (`objects.md` 2.1). Writes into those arrays after the table is built destroy table bytes, so the
variants are not all pure rotations:

| Table byte (index = address - $03D8) | Address | Variant, plane, row | Why zero | When |
|---|---|---|---|---|
| 105 | $0441 (hit points of slot 0) | 6, plane 1, row 1 | slot 0's hit points are rewritten with 0 (a table entry) on every step, also on warp levels **[V]**: the write was watched on every step of level 33 | **every warp level**, from the first A pass on (the level-start run still sees the intact byte) |
| 66, 67 | $041A, $041B (work 5 of slots 23 and 24) | 4, plane 0, rows 2 and 3 | the replays' `obj 23 ...` and `obj 24 ...` setups clear the helper slots' work bytes | the replays only (a test setup; not in play where nothing writes there) |
| 97, 98 | $0439, $043A (work 6 of slots 23 and 24) | 6, plane 0, rows 1 and 2 | same setup | the replays only |

Each such byte reads 0 instead of the rotated value (level 33: $FB, $4B, $4A, $0A, $D2 become 0; on level 40 only the zeros that differ from the source show; level 37 loses
the byte-97 value $81 too). With the setup's zeros the pure-rotation model predicts only 554 of 703 pictures on level 33, 501 on level 40 and 626 on level 37, and 618 on level 33
without the setup (the byte-105 hole alone): the remake must either reproduce the zeros (the exact picture of the original, which `replay-compare` sees) or build clean variants, in which case
the pictures differ from the original in the rows listed and only while `ox` is 4 or 6 (`ox` = 6 for byte 105).


#### 5.7.3 Parallax stars (all levels except warp levels) [V model, C for the rest]

Eight one-pixel stars drift against the scroll over open sky. The job that moves them also decides the horizontal
thrust carry-in of `player.md` 4.6, so its state is part of the physics. Model checked against Mesen on 1,484 job
calls (positions, fractions, dead mask, the on/off flag and the carry-in byte; including the fade-out and the
restart); the rules marked [C] were not exercised.

**State.** Per star `i` (0-7): x, y (bytes, wrap at 256), x fraction, y fraction (RAM $04B7, $04BF, $04C7, $04CF + i)
and a *dead* bit (RAM $04D7, mask $80 >> i; a dead star is not on the field). A global mode `M` (RAM $D5): 0 = off,
1 = on, $FF = fading out. The mode is **not** reset at level start or respawn. The star y bytes are set to $FF at every level load
and respawn, but the dead mask is set to all dead (and the mode is not touched) **only by the start-of-game routine** (the first
level start after the title), not at later level loads, respawns or on level 26 **[V 2026-10-20]**: write watch on RAM $04D7 and
$D5 in the replays `level5`, `level13`, `respawn`, `warp-level` and `level26` saw exactly one all-dead write per run, at the first level start, and
otherwise only the star job's own writes. So a level begins with whatever mode and mask the previous level left (level 26 entered
from planet 0 in the `level26` replay: `M` = 1, mask $77 = stars 0 and 4 alive at x = 0, y = $FF; the first job call with a non-zero `T` turns that
into `M` = $FF, kills both (x below 8) and switches the job off in the same call, step 17). Cold boot clears everything.

**Per-star constants.** Layer factor of the camera movement: stars 0 and 5: 1, stars 1, 3, 4, 6: 3/4, stars 2 and 7:
1/2. Spawn phase (compared with the frame counter RAM $5D AND $7F): $02, $28, $32, $67, $16, $35, $56, $71. Spawn
position (x, y): (0, $45), (84, $FC), (0, $C4), (199, $FC), (0, $23), (135, $FC), (0, $87), (33, $FC).

**When the job runs.** At the end of the logic step's second frame (B pass) after the camera has moved, always; and
in the first frame (A pass), also after the camera, only if the terrain-list size `Z` (below) is $10, or is below
$20 while nothing is towed (RAM $66 = 0). Never on warp levels (RAM $9B bit 7). The original's drawer would still draw the frozen stars there if the mode
was non-zero on entry; the remake draws none on warp levels [C]. `Z` is the length of the player's terrain collision lists as last built (`levels.md` 6.4, window): 16 plus
3 bytes per collected segment in directions 0, 7, 8, 9 and 4 bytes per segment in directions 1-6 and 10-15. [V] for
the A-pass rule and for `Z` in samples ($10 = no terrain in the window, sky; $17, $18, $1E, $1F, $25, $26, $2C seen);
[C] that every list entry counts. The original thus skips the A-pass call whenever the window's segments add up to 16 bytes or more
(it happened on about one A pass in five in a sky-and-hill flight on level 0). It is a CPU-load shortcut; the remake
may run both passes always (the stars would then drift slightly faster over terrain), but the mode flag, and with
it the carry-in, then follows the original's exits only approximately (`docs/deviations.md`). The frame counter
is even at every job call **[V]** (re-checked on the rotate-thrust replay, both calls of every logged step): the
counter is incremented once per pass, in the middle of the pass (in the raster-split routine, after the wait for
the sprite-0 hit). In the A pass (counter even on entry) the job is called *before* that increment and sees the
even entry value; in the B pass (counter odd on entry) the job is called *after* it and sees the next, even value.
So yes: the B-pass call sees the incremented counter. The star drawing, in contrast, runs at the start of the B
pass, before the increment, and sees the odd value (see "Drawing") **[V]**. So the odd spawn phases ($67, $35, $71) never match: **stars 3, 5 and 7 never appear** [V: every one of the 1,484 logged calls, and the first calls from six level-start states, had an even counter; no
path that makes it odd was found].

**One call.** Inputs: `bx`, `by` = the frame's signed camera half-steps (RAM $50, $51, `levels.md` 5.2); `T` = the
chunk of the window's **bottom-right** block (RAM $D4; 0 = open sky; `levels.md` 6.4 window, 6.5 "Sky chunk"), as
of the last terrain test. **[C]** read from the program, and it is what the implementers' replays fit (the
top-left chunk does not). Reason: the terrain test (`levels.md` 6.4 step 1) walks the 3 x 3 blocks row by row
and stores the chunk id every time the walk fetches a chunk (at the start and whenever it crosses a chunk
border); the walk ends with the ninth block, so the last value stored is that block's chunk, i.e. the chunk of the
block two blocks right of and two blocks below the window's top-left block (the block containing world point
(x + 16, y + 16) for a player at (x, y); when x or y is below 16 the walk starts at the block of x resp. y
itself, with the first column resp. row flagged empty, so "start + 2 blocks" still holds). Everywhere in this
section "the window's top-left block" means this bottom-right block. Sky is thus detected 32 pixels diagonally
ahead (down and right) of where `levels.md` 6.5 puts it.

1. If `M` = 0: if `T` is not 0, the call ends here (nothing else is touched, in particular not the carry-in byte);
   else every star's y is set to 0 (dead stars too) and `M` = 1. If `M` is not 0: if `T` is not 0, `M` = $FF
   (when `T` is 0 again, `M` stays $FF).
2. Per-call movement values, 8.8 signed: `F` = (−bx, −by) (whole pixels), `H` = F / 2, `Q` = H / 2 (arithmetic
   shifts of the 16-bit value; fractions kept). Layer 1 moves by `F`, layer 3/4 by `H + Q`, layer 1/2 by `H`. The
   shared carry-in byte (RAM $0D) is left as the whole part of `H` for x: `floor(−bx / 2)` as a signed byte [V].
3. For stars 7 down to 0:
   - **Dead star:** if `M` is $FF, nothing. Else, if (`$5D` AND $7F) equals the star's spawn phase, it becomes alive
     at its spawn position (fractions are kept).
   - **Live star:** the movement is the star's layer value, or `F` when `M` is $FF. x fraction/x whole and y
     fraction/y whole each take their 16-bit addition (carry from the fraction), the bytes wrap. When `M` is $FF
     and the new x is below 8, the star dies (dead bit set, y = $FC) and its y is not moved; likewise, after its y
     move, when `M` is $FF and y is $F8 or more. Otherwise the new y is stored (with `M` = 1 the star simply wraps
     around and never dies).
4. If `M` is $FF and all eight stars are dead, `M` = 0.

**The carry-in at the end of a call, and camera arrival frames [V].** The value left in the carry-in byte is
`floor(-bx / 2)` with `bx` the *recorded* half-step (RAM $50), which is not always the distance the camera
moved. The camera code zeroes $50 and $51 at the start of every run, and zeroes $50 again in the frame in which
the camera arrives at a limit (`levels.md` 5.2, "Limits"): at the right limit, when the camera's high byte reaches
the limit's (the camera is put exactly on the limit, whatever it moved); at the left limit, when the move would go
below 0. Likewise $51 for the vertical limits (top: below 0; bottom: past the limit by 1 or more). A move that
ends exactly on 0 (left, top) or exactly on the bottom limit is not zeroed. All four limits were checked in Mesen on 2026-10-17
(`levels.md` 5.2, "Limits"). Example (rotate-thrust replay, level 0,
right limit x = 768): the recorded bx of the B frames is +1 up to step 148, then 0 from step 149 on, because in
step 149 the camera goes 766, 767 (A frame, bx = +1) and 768 (B frame, recorded bx = 0 although it moved 1 pixel).
The thrust of step 149 sees the job of step 148's B frame, `floor(-1/2)` = $FF; the thrust of step 150 sees
step 149's B frame, `floor(0)` = 0, where a model that feeds the job the camera's real displacement gets $FF.
This is the whole explanation of the replay mismatch at step 150 (it happens to coincide with the stars' fade-out
on that level, because the pod is below the sky there, but fade-out as such does not matter). The remake must feed
the job the recorded half-step (zero in arrival frames). The stars use the same recorded value, so they do not
move in an arrival frame either.

Consequences: stars appear once the window's top-left block is in chunk 0 (open sky) and keep running after the pod
dives into terrain; there `M` = $FF, the stars slide out at the full camera speed and, once all are gone, the job
switches off and stops touching the carry-in until the pod is back in the sky (each (re)start sets every star's y
to 0, then the live ones reappear at their phase times). At cold start the first 72 steps of level 0 (pod still
in the launch bay's chunk) have the job off.

**Drawing.** If `M` = 0, no stars. Otherwise each *live* star is one sprite: tile $FF (the single dot, section 3 table),
OAM (x, y), which puts the dot at screen pixel (x, y + 1); sprite priority *behind the background* (visible only
over background colour 0); sprite palette 3, except that for star `i` palette 1 is used when ((32 × i + $5D) AND
$FF) is 1 or 5 at the draw (the draw runs once per logic step, in the B pass, where $5D is odd; so star `i`
flashes palette 1 on two draws in four around $5D = 256 − 32 × i + 1..7). Clip: sprites in the leftmost 8 pixels
are hidden, as is everything below the playfield (line 192); y values $F0-$FF are off-screen. The original puts
the eight stars into the last eight OAM entries, so busy frames (more than 56 other sprites) lose stars; the
remake draws them always. Dead stars are in OAM too, at their last y ($FF at boot, $FC after dying, 0 right after
a restart: a dot on the top line for any dead star whose x is 8 or more); the remake does not draw dead stars. [C]
for the OAM placement, [V] for tile, attribute rule and order.

**The exact drawing rule** (read from the drawing routine, bank 1 $9023, and [V] by the replays) and the two cases that look like "missing stars":

1. The routine runs once per logic step at the start of the B pass, after all other sprites are in the table (sprite 0 first, X = the next free entry). If `M` = 0 it only
   hides the rest of the table (Y = $F8 from X on) and no star exists in sprite memory. This is the whole reason for a star-free frame **[V]**: with `M` = 0 nothing is drawn, whatever the
   sky looks like (level 26, steps 17-237 of the `level26` replay, e.g. step 21: sprite 0 and the pod's sprites only).
2. If `M` is not 0: the entries from X up to $E0 (entry 56) are hidden, then **all eight stars** are written, live or dead, with no test of the dead mask, no x or y limit and no check of
   the background: entry = (y, $FF, attribute, x) from the star's own bytes. If X is already past $E0 (more than 56 sprites), the first (X − $E0)/4 stars are skipped
   (they take the places that no longer exist) and the rest fill what is left up to entry 63. Dead stars sit at their last y (see above).
3. What the screen shows is the PPU's: a sprite with attribute bit 5 (behind the background, $23 or $21) is hidden wherever the **background pixel's colour index is not 0**,
   whatever its colour. A background tile can use index 1-3 of a palette whose entry is black ($0F); the star is then covered although the background looks like
   empty sky. **[V]** level 3, steps 2173-2228 of the `level3` replay: star 1 stands still at OAM (x 73, y 185, attribute $23, `M` = $FF, the camera at rest, no other sprite
   on that scanline); the picture shows nothing at (73, 186), while the same replay with the attribute poked to $03 (in front) shows the dot at (73, 186) in sprite palette 3
   entry 1 ($32). The remake must therefore test the background's colour **index** (0 = transparent), not its colour. The sprite pictures of the capture tool show the
   dot at (x, y + 1) in OAM coordinates; the sprite memory of a capture and its picture can differ by the step's camera move when the camera moves.
4. A star never spawned (stars 3, 5, 7; or after `M` goes 0 to 1 every star has y = 0) is still drawn, at its last x and y 0: a dot on the top line, visible only over index 0
   there (level 26 step 238: two such at x = 223, 230).

**Level 26 and the other levels [V 2026-10-20].** Nothing about the star job or its drawing is special on level 26, in player state 6 (the route flight) or while the route camera runs:
the job is called from the same two places as everywhere (A pass: `Z` test; B pass: always), and the only skip besides `Z` is warp levels (RAM $9B bit 7, which is 0 on level 26;
logged on every call of the replay). Neither the player state nor RAM $9A is read. Levels 27 and 28 use the same code and the same $9B (0), so the
rules apply unchanged [C, not run]; player state 9 only matters through the pass flow (no extra test at the call sites) [C]. What differs on level 26 is the data: `T` (RAM $D4, the sky
chunk) is non-zero for most of the flight, so `M` is 0 for long stretches and stars exist only around the sky stretches. Original, `level26` replay, per step (the value at the step's B-pass call; `T` = 0 means sky):

| `T` = 0 | steps 1-16, 238-301, 366-418, 430-493, 814-913, 1902-1989, 2030-2082, 2176-2184, 2302-2312, 2926-3249, 3378-3481, 3694-3737, 3758-3898 |
|---|---|
| `M` = 1 | 1-16, 238-301, 401-418, 447-493, 814-913, 1902-1989, 2176-2184, 2302-2312, 2926-3249, 3378-3481, 3694-3737, 3834-3898 |
| `M` = $FF | 302-400, 419-446, 494-574, 914-1000, 1990-2092, 2185-2266, 2313-2447, 3250-3333, 3482-3605, 3738-3833 |
| `M` = 0 | every other step (1-16 excepted, 17-237, 575-813, 1001-1901, 2093-2175, 2267-2301, 2448-2925, 3606-3693 and the gaps between the runs above) |

Star sprites (tile $FF with y below $EF, x of 8 or more, y above 0) are in sprite memory in 1,661 of the 3,898 captured steps (the OAM side is exact [V]); on screen, as on level 3 (item 3), some are covered by background pixels
of index 1-3 (124 steps have stars in sprite memory and no isolated dot in the picture). `M` = 0 whenever `T` has been non-zero since the last fade-out finished, even though the picture may show open sky:
`T` is the chunk 32 px right of and below the pod's top-left corner, not what is visible.

#### 5.7.4 Jetman's tiles: the pod's first 18 tiles are swapped during play [V]

(2026-10-20, CHR RAM of the sprite table read back at every step of `jetman`, `respawn` and a purpose-made `spare` replay.) The sprite pattern table's tiles $01-$12 (PPU $0010-$012F) hold two different
pictures in play:

| Tiles $01-$12 hold | Source | From | Until |
|---|---|---|---|
| the pod's art (the first 18 of the 64 pod tiles, 6.1) | ROM 0x0CBC5 (pod type 0), 0x0CFC5 (types 1 and 3), 0x0D785 (type 2): the pod set of the current pod type (RAM $DC AND 3) | every level load (6.1) | the pod's destruction |
| Jetman's own figure (the picture the title screen uses for its 18 title sprites) | ROM 0x0F185, 18 tiles (0x120 bytes) | the pod's destruction | a new pod: the level reload (a respawn after Jetman's death, 7.3 in `player.md`), or Jetman getting into a spare pod (below) |

Tiles $13-$40 (the rest of the pod set) never change. The swap is done two tiles per logic step, nine steps (the video buffer takes 32 bytes a step):
- **Destruction:** in the picture of the k-th step after the one that destroyed the pod (k = 1..9) the tile pair (2k - 1, 2k) already holds Jetman's tiles (pair 1 = tiles $01-$02 from ROM 0x0F185,
  pair 2 = $03-$04 from 0x0F1A5, ..., pair 9 = $11-$12 from 0x0F285); before that pair the pod's tile. While the swap runs the pod is hidden (frame $FF) and its explosion is drawn from other tiles, so the half-swapped state is never seen. Jetman appears on the 11th step
  (`player.md` 7.1) with his figure complete.
- **Spare pod:** the same pairs go back the other way, from the current pod type's set, in the k-th step after the touch (`player.md` 6.3).
- **Level reload:** the whole pod set is written again (the level-start loads, 5.7 item 6), which is why a respawn needs no swap back.

Colours: Jetman's frames use sprite palette 0 like the pod ($30, $22, $12: white, light blue, blue; entry 16 is the universal background). His art uses colour 1 (white) for 262 of its 1,152 pixels, colour 2 for 162, colour 3 for 82 and is transparent for 646; the pod's
first 18 tiles use colour 1 for 76 pixels, colour 2 for 379, colour 3 for 264. So with the pod's tiles in place of his, he is drawn in the pod's blue instead of white and light blue. No palette upload accompanies the swap: palette memory is identical in the pictures before the destruction, during Jetman's life and after the respawn.

#### 5.7.5 Level 26: pattern table 0 and the sprite tile bases [V]

(Mesen 2026-10-20: CHR RAM of `level26.replay` at steps 1, 2, 3, 10, 100, 500, 1,000-3,890 and 4,020-4,139: table 0 is identical in all of
them; tile bases read from RAM $05CF-$05FE and checked against the OAM of every step.) The final level draws its sprites from this
table, and its layouts (`objects.md` 5.2, 6.4, 6.5a) use absolute tile numbers (tile base 0, except frame $33 below). What the
tiles hold when play starts, in the order the loads run:

| Table-0 tiles | Content | Used by |
|---|---|---|
| $00-$1B | the 28 tiles decoded from ROM 0x0F4C5 (4.1). Used: $00 and $01 the pod's rear flame (two crescents; tile $01 only in the frame $2E, a direction held, `player.md` 9.1), $02-$07 the pod's body, $0A-$16 the end structure's cores (type 5E, frames $B4-$B6: $10-$13, $0C-$0F with $14, $0A/$0B with $15/$16), $17 a small fragment (frame $33, below). $08, $09, $18-$1B are not drawn by any frame seen; frames $B7 and $B8 (16 fragments, never seen in play) use $00-$19 | the final pod (frames $2D, $2E = layouts P$1E, P$1F), the cores, the pieces |
| $1C | the pod set's tile $1B (the pod set is written at $0010, so its tile `k` is table tile `k` + 1), except that **byte 0 is $09**, the one extra byte of the 0x0F4C5 stream | not drawn |
| $1D-$26 | the pod set's tiles $1C-$25 (the pod type's own art, 6.1/4.3) | not drawn on this level |
| $27-$5C | a copy of the terrain tiles $27-$5C (section 3), the terrain set of graphics type 1 (ROM 0x1336F): the same art as the terrain background | the drifting rocks and their splitting pieces (types 4F-58 and 59-5D, frames $AF-$B3, tiles $2D-$5C) |
| $5D-$B9 | not written by this level: whatever the previous level's object sets left (they are never drawn on this level; do not read them) | |
| $BA-$FF | the common sprites as on every level (6.1), the explosion and spark tiles, the stars' tile $FF | the usual |

Tile 0 is therefore **not blank** on this level (elsewhere gameplay clears it): the final pod's rear flame is tile $00. Sprite palettes:
the pod's layout uses palettes 0, 1 and 2 (white-blue, orange and green, from the level's header bytes and preset 10, 7.3); the rocks
and the structure's cores use palette 3 (header bytes 21-23 = $22 $12 $02, blues) with the layouts' own attribute bytes.

**Tile bases (RAM $05CF-$05FE) on level 26 [V].** The level start clears all table-A and table-B ranges ($05CF-$05DE, $05EF-$05F6 set to
$FF, so no frame matches), then, because RAM $9A bit 7 is set, **registers one table-A range: frames $33 up to (not including) $34, tile
base $17** (RAM $05CF = $33, $05D0 = $34, $05DF = $17) and returns without loading any object tile set. (The bases $05E0-$05E5 and
$05F7-$05FE keep the previous level's bytes; with no range matching they are never read.) Every other frame of both tables is drawn with
base 0. So the one frame that gets a base is $33, the single-sprite "dot" (a count-0 layout of tile 0 and attribute 0): on level 26 it is
tile $17 (and the pieces of the end structure's effect, `behaviour.md` 4.16, are drawn with it, mirrored because their flags X byte is 1: attribute
$40). The other object tile sets of 6.4 are **not** loaded; list A = {$33} and list B = {$53} in the per-level lists (bank 1:$C980, level 26)
are therefore only partly used: the A entry gives the range above (not through the usual record: the program writes the range itself), the
B entry gives nothing.

#### 5.7.6 Table 1 tiles $E4-$FE: the lives icon on level 26 and the warp levels [V]

(Mesen 2026-10-20: pattern table 1 read back after the level start of levels 0, 1, 5, 12, 13, 25, 26, 27, 28, 29 and 41, and the writes to tiles $D6, $E4, $F6, $FC and tile 0 followed frame by frame on levels 13 and 26.)

The status panel's tiles are written **twice**, and which copy survives depends on the level:

- Panel stream 0x0FA6E (RLE, L = 0xD4, 4.1) gives the tiles for $1E40-$1FEF, i.e. table-1 tiles $E4-$FF; tile `n` is output bytes 16 (n - $E4) .. +15 (so $F6 is bytes 0x120-0x12F). It is written at the start of the level load (5.7 item 2).
- The common gameplay background (0x17159 → $1CC0, 0x330 bytes = tiles $CC-$FE, 4.2) holds **older versions of most of the same tiles**. All of them are byte-identical to the panel's (digits, "F", "W", the full gauge cell, the pod icon pair $F4/$F5) except these, where the common background's version is different: **$E4, $F6, $FC, $FD, $FE** ($FA, $FB, $FF are rewritten during play anyway, section 3). On every level that loads it, it is written after the panel stream and wins.
- The terrain set of graphics types 1 and 6 (0x1336F, 0x1000 bytes, fills the whole table, 5.7.1) also contains the common background's versions of $E5-$F9, $F6 included (its $E4, $FA-$FF differ). Level 26 is type 1.

So the tile $F6 that the HUD's life icons draw (nametable cells row 6, cols 16-19, the same on every level) is:

| Levels | Tile $F6 at the start of play and for the whole level | Bytes (16, plane 0 then plane 1) |
|---|---|---|
| 0-25, 27, 28 | common background's version | `18 14 16 3C 7C 76 28 30  14 04 26 42 22 6A 1C 06` |
| **26** and **29-41** | **the panel stream's version** | `0C 14 14 3C 7C 36 28 30  10 04 24 42 02 0A 1C 06` |

(Plane 0 is the first 8 bytes, plane 1 the last 8; pixel colour = plane 0 bit + 2 x plane 1 bit, the icon draws in background palette 0, 1.2. The two are slightly different drawings of the small Jetman figure: the common background's has more colour-2 and colour-3 pixels in rows 0, 2, 4 and 5.) Nothing changes the tile during play (checked on levels 13 and 26 for 600 steps).

**Level 26 load order, seen frame by frame:** the panel stream (tile $F6 = panel version, 0x0FB42 → $1D60 right after), the terrain set (tile $F6 = common-background version), then one further write which makes tile $F6 the panel version again (nothing else in $E4-$FF changes visibly, because the terrain set already holds the same bytes there except $E4, $F6 and $FC-$FE, which end as the panel's too). On level 13 the corresponding last write is the common background, so the tile ends as the common background's version. Net result on level 26: **tiles $E4-$FE (except $FA, $FB, $FF) are the panel stream's, tiles $CC-$D7 are the terrain set's** (not the common background's; $D6 and $D7 are terrain-set bytes, not the 0x0FB42 tiles, and $D8-$E3 equal neither; nothing on the panel draws $D6-$E3).

Other differences that follow from it: the score sign tile $E4 (drawn when the score is negative, 1.2) is the panel stream's (a short horizontal bar, plane 1 rows 0-1 = $7E) on level 26 and the warp levels, and the common background's tile (`B7 A7 B7 7F B7 A7 B7 7F  F8 F8 F8 7F F8 F8 F8 7F`, terrain-like) on the other levels; $FC-$FE are not drawn by the panel on any level. The cells that the panel does draw ($F4-$F9, $FA, $F7, $F8, $E5-$E8, the digits) are identical on every level.

**For the remake:** keep two copies of the panel's tile set; on level 26 and the warp levels use the panel stream's tile $F6 (and $E4), on all other levels the common background's.

### 5.8 Equipment-picture loader (menu D) [V for the analysis screen; the shop does not use it]

Palette preset 26, then menu screen D (5.9), then 0x35394 → $0800, block 0 of 0x32E56 → $1810 and block
(item + 1) → $1930, where item is the item on offer (RAM $68 at the call). Menu D's tiles at $1000-$1400 come
first, so the equipment tiles at $1810-$1A4F overwrite nothing of it. [C]

**Not the shop.** The shop and the equip list use the screen of 5.10, not this loader (every loader call logged in
Mesen for both, 2026-10-17: no load of menu D, 0x35394 or 0x32E56 happens in either, and the pattern table
matches the 5.10 streams byte for byte). This loader's only known user is the analysis message screen below; the
older reading of it as the shop screen came from the program text, not from a run.

**The analysis message screen uses the same loader** (`scenes.md` 10, item kind 9): palette preset 26 (entries
13-15 only), menu screen D, then the same block loader with the analysis list number k (0-16, `text.md` 3) in place
of the item: block 0 → $1810 and block k + 1 → $1930, so blocks 1-17 are in use (block 17 only by list 16). The
picture area of menu D's nametable (rows 3-7, columns 13-20, tiles $81-$A4) shows those tiles. **[V]** (code read
and nametable read back in Mesen, list 4). Its palette entries 9-11 are not from a preset: 7.4.

### 5.9 Menu screens A-D [V decode]

All four load the frame and font tiles 0x3684F, then their nametable and attribute data (4.5):

| Screen | Bank 6 entry | Used for | Tiles 0x3684F at | Nametable | Attributes | Palette |
|---|---|---|---|---|---|---|
| A | $8006 | the planet map (`scenes.md` 13.2) **[V]**, entered with the sound switched off; also loads the title sprite tiles (0x0FF2F → $0800, 0x15546 → $0860, 0x0FE52 → $0B80) and then, **last of the pattern loads**, 0x29CB7 → $0A80 (the mothership marker's ship tiles) [V order, 2026-10-17] | $1000 + 16 × base, base = $62 when (surface picture number AND 3) = 2, else $54 | 0x36149, B = base, → $2000 | 0x362F2 → $23C0 | preset 8 |
| B | $8009 | password entry (followed by the font: digits $1300, letters $1410, extras $1600); the message boxes of the item-found and equipment messages | $1000 | 0x36788 → $2080 | 0x36839 → $23C8 | preset 24 (password screen: then preset 9) |
| C | $800C | further message screens | $1000 | 0x36540 → $2000 | raw 0x36728 (49 bytes) → $23C0 | preset 25 |
| D | $800F | the analysis message screen (5.8); not the shop or the equip list (5.10) | $1000 | 0x36316 → $2000 | raw 0x36758 (49 bytes) → $23C0 | preset 26 |

Screen B as the password screen was observed in play ([V]: choose CONTINUE on the title); the others are [C].
The palette for each is the preset loaded next to it; for the password screen palette RAM matched presets 24 and
then 9 ([V]).

### 5.10 The shop and equip-list screen (bank 2 entry $8003) [V]

The screen of `scenes.md` 6. One loader with two variants, chosen by RAM $63 bit 7 (the shop sets it, the equip list
leaves it clear). Verified 2026-10-17 by logging every loader call in Mesen (shop after a flight, equip list from the
planet menu) and comparing the pattern table with each stream byte for byte (0 differences apart from the program's
own tiles below).

| Load | Shop (bit 7 set) | Equip list (bit 7 clear) |
|---|---|---|
| fonts | digits 0x15C96 → $1300, letters 0x15BB6 → $1410, extras 0x15CEA → $1600 | the same |
| tiles for table 0 | 0x15A02 → $0000 (0x200 bytes) | the same |
| tiles for table 1 | 0x1579C → $1000 (0x291 bytes) | 0x15A02 → $1000 (0x200 bytes: the same stream as the table-0 load) |
| large digits | 0x0FA74 → $1E50 (15 tiles $E5-$F3, the digit pairs of 1.2) | the same |
| item pictures | 0x15D40 → $0800 and → $1600 (12 pictures of 9 tiles; picture n at $0800 + 0x90 n for the sprites, tiles $80 + 9 n, and at $1600 + 0x90 n for the strip, tiles $60 + 9 n [C for the strip]), then 18 tiles (the pictures of items 12 and 13) from 0x16400 → $0EC0 and → $1CC0 (0x16520 instead when ($3B AND 3) = 3, the jetpods: [C]) | the same |
| attributes | 0x112EE → $23C0 | 0x1130F → $23C0 |
| nametable | laid out by the program (logo, strip frames) | 0x1134C → $2000 (purple frame, chain) |
| palette bytes | table at bank 2 $932E (ROM 0x1132E) | table at bank 2 $933D (ROM 0x1133D) |

The program then writes tile 1 of table 0 (a one-pixel marker, $80 then zeros: sprite 0's tile, the split sprite at
y = 44), a solid tile at $0FE0 (section 3) and loads palette preset 4. The 12 pictures are the same ones as the
gameplay panel's weapon icon (1.2). The older reading of the variant with bit 7 clear as "no caller found" and of
the whole screen as "purpose not identified" was wrong: they are the equip list and the shop.

### 5.11 Ending [V decode]

0x3D379 → $2000, 0x3D663 → $1000, 0x3E1BF → $1DC0, 0x3E3CE → $0000, 0x3EED8 → $0D60, 0x3F11D → $23C0, in
this order. The picture shows Jetman relaxing. Two stream bytes run on (1, "Writes run on"): the nametable
stream's 961st byte lands on $23C0 and is replaced by the attribute stream; the last byte of 0x3E1BF (output
0x241 bytes from $1DC0) lands on $2000, the nametable's top-left cell, and its value equals the value the
nametable stream put there ($00), so nothing changes. The ending does split pattern tables: background table $1000 down to line 135, $0000 from line 136, at a sprite-0 hit (`scenes.md` 14.2). The palette is preset 22. **[V]**
[V decode]

## 6. Tables that choose graphics

### 6.1 Common sprite set (title and every gameplay start) [V]

Bank 1 entry $8024. Writes, in order: 16 zero bytes at $0000 (tile 0), then either 64 tiles of the pod set at
$0010, when the state of the object in slot 0 (the player; RAM $0317, see `objects.md` 2.1 and 2.4) is 1, 7 or 10,
which is the case at every gameplay start (pod type RAM $DC AND 3: 0 → 0x0CBC5, 2 → 0x0D785, 1 or 3 →
0x0CFC5), or otherwise (the title sets slot 0's state to 2) 18 tiles from 0x0F185 at $0010 (Jetman's own figure; gameplay uses the same 18 tiles after the pod's destruction, 5.7.4); then 2 tiles 0x0FDF2 at $0BA0, 11 tiles 0x0FE22 at $0BD0, 2 tiles 0x0E0B5 and 5
tiles 0x0E255 continuing at $0C80, 13 tiles 0x0E2A5 at $0D10, 1 tile 0x0FE12 at $0DE0, 16 tiles 0x0DF45
continuing at $0DF0, the 6-tile block of RAM $D1 (if $D1 ≠ 0) at $0EF0, 2 tiles 0x0FDD2 at $0F50 and 8 tiles
0x0CB45 at $0F70.

### 6.2 Level graphics type [V]

The level header (42 headers of 27 bytes; pointer tables at bank 2:$956E (low) and $9598 (high), indexed by
level 0-41) holds the graphics type in byte 11, bits 0-2. Bits 3-7 are the gravity (see `levels.md`). Types: levels 0, 6 → 3; 1 → 6; 2, 9 → 4; 3, 11, 16, 20, 23, 27, 28 → 2; 4, 7, 17, 19, 21, 25,
29-41 → 0; 5, 8, 10, 13, 14, 15, 18, 22, 24, 26 → 1; 12 → 5. Level index (RAM $3B; meanings from `levels.md`):
0-12 the planets, 13-25 caverns, 26 the long scrolled level, 27-28 small rooms, 29-41 warp levels. [V for 0-13
and 29, C for the rest] The approach screen's name list (bank 6:$D2FD) has 14 names; with the level index forced
to 13 the approach screen shows the 14th, "14 DRYN KUP LEEZ", with gravity 0 and diameter 1, unlike the 13
planets. We follow `levels.md`: 13 planets, level 13 is a cavern-shaped level (open question in Open questions).

### 6.3 Level palettes: see 7.3.

### 6.4 Object (enemy) sprite sets [V]

At the start of a level the program copies the sprite tiles needed by the level's objects, one block after
another from tile $41 of table 0 (PPU $0410), and records for each sprite layout frame the tile number its
block starts at (so layouts are drawn relative to that base). Frame ranges, tile bases and the layouts
themselves are specified in `objects.md` 5.2 and 6; this section covers only what is loaded where.

- Per-level lists, bank 1:$C980-$CAB3 (ROM 0x0C980): for level 0, 1, ... 41 in order: a count `n` and `n`
  bytes (list A), then a count `m` and `m` bytes (list B). The bytes are layout frame numbers (`objects.md`).
- List A entries are looked up in the 5-byte records at bank 1:$CAB4-$CB12 (19 records) and serve layout table
  A (slots 15-30); list B entries in the records at $CB13-$CB44 (10 records) and serve layout table B (slots
  1-14). Record: first frame, end of the frame range (exclusive), tile data start (low, high; bank 1), tile
  count.
- A list entry is matched against the first byte of each record only. The match registers the record's frame
  range with the block's tile base; the tiles are copied once.
- Every list entry has a record. An entry with no record is not handled: the search would not stop at the end
  of the records (it never happens in the original data). [C]
- Blocks are placed in list order: all of list A, then all of list B, consecutively, starting at tile $41.
- Level 26 (RAM $9A bit 7 set) loads no object tile set: after clearing the ranges the level start registers the single range [$33, $34) with base $17 and returns (5.7.5). **[V]** The per-level lists of level 26 (A = {$33}, B = {$53}) are not read. The loader's other callers (every level with RAM $9A bit 7 clear) are unchanged.

Example, level 0: list A = `18 83 9A` → 38 tiles from 1:$E525, 21 from 1:$EEB5, 8 from 1:$F445 (tiles
$41-$83); list B = `59 5C 8A` → 4 from 1:$E3B5, 4 from 1:$E485, 1 from 1:$E475 (tiles $84-$8C).

The tile data are graphics (ripped); the lists and records are sprite layout data (commit), see
`rom-map.md`.

## 7. Palettes

### 7.1 Shadow and upload [V]

RAM $76-$95 holds a copy of the 32 palette entries (byte $76 + i = entry i). Every frame the game uploads, in
order, shadow entries 1-15 to palette RAM $3F01-$3F0F, shadow entry 16 to $3F10 (which is the same cell as
$3F00, the universal background), and shadow entries 17-31 to $3F11-$3F1F. Shadow entry 0 is never uploaded.
So the universal background colour is shadow entry 16. [V]

**How often it reaches palette RAM during play [V]** (level 26, every step of `level26.replay`: shadow entry 11 and palette RAM $3F0B read each
frame). The shadow is copied to palette RAM only by the gameplay handler's video job 0 (`game-flow.md` 1.2 table: one frame in 8, the NMI frames where RAM
$5D mod 8 = 0) and by job 8 on the first frame of a level, not on every frame. A change of the shadow therefore reaches the screen 1-8 frames
later (palette RAM changed at the first job 0 after the shadow did: shadow changed at the A pass with $5D = $10, palette RAM at the NMI with $5D = $18).
The remake compares pictures against the original's: a palette written by the program in an A pass shows from the next job-0 frame on.
Levels 3 and 11 also rewrite shadow entries 9-11 every 16th step from a camera-dependent table (terrain colours below a certain depth): `levels.md` 6.6, with its upload timing (visible in the harness picture 3 steps after the write's step). [V] 2026-10-20

On warp levels shadow entries 16-31 hold the mesh tile instead of colours (5.7.2): the program uploads $0F in
place of entry 16 and does not upload entries 17-31, so the sprite palettes stay as set at level start (preset 10
and level-header bytes 18-23, 7.3). [V] palette RAM, [C] rule

### 7.2 Palette presets [V for the scenes checked]

28 presets in bank 0. For preset `p`: data address = 16-bit little-endian word at bank 0:$F71E + 2p (ROM
0x0771E), first shadow index = byte at $F756 + p (ROM 0x07756), number of entries = byte at $F772 + p (ROM
0x07772). Loading copies that many bytes from the data address into the shadow starting at that index. All
preset data lie in ROM 0x07526-0x0771D.

| Preset | Data (bank 0) | First index | Count | Used for |
|---|---|---|---|---|
| 0 | $F526 | 0 | 32 | approach screen, power-on [V] |
| 1 | $F546 | 1 | 16 | interplanetary flight (`scenes.md` 5) [V] |
| 2 | $F556 | 17 | 15 | interplanetary flight, sprites, rewritten on every pass (`scenes.md` 5) [V] |
| 3 | $F565 | 0 | 32 | no use found [C] |
| 4 | $F585 | 1 | 3 | shop and equip list (5.10) [V] |
| 5 | $F588 | 1 | 31 | title [V] |
| 6 | $F5A7 | 8 | 8 | screen of bank 2 entry $8036 (spaceship picture) [C] |
| 7 | $F5AF | 16 | 4 | no use found [C] |
| 8 | $F5B3 | 9 | 7 | menu screen A: the planet map [V] |
| 9 | $F5BA | 1 | 11 | password screen, after preset 24 [V] |
| 10 | $F5C5 | 16 | 8 | gameplay sprite palettes 0-1 [V] |
| 11 | $F5CD | 1 | 7 | gameplay background palettes 0-1 [V] |
| 12-17 | $F5D4, $F5EF, $F60A, $F5D4, $F625, $F640 | 1 | 27 | surface pictures 0-5 (preset 12 + picture number; 15 would be the empty picture 3) [V] |
| 18 | $F65B | 1 | 23 | surface picture 6 [V] |
| 19 | $F676 | 1 | 31 | attract: floating Jetman (entry 16 = $01, a blue background) [V] |
| 20 | $F695 | 0 | 32 | attract: "Space Loonies" logo [V]; also the credits [V] and the other screens with that logo [C] |
| 21 | $F6B5 | 1 | 15 | comet screen of 4.5 [C] |
| 22 | $F6C4 | 1 | 31 | ending [V] |
| 23 | $F6E3 | 1 | 15 | the logo screen with the sprite-0 split of 1.1 (probably game over) [C] |
| 24 | $F6F2 | 9 | 7 | menu screen B (password and message boxes) [V] |
| 25 | $F6F9 | 9 | 7 | menu screen C (message screens) [C] |
| 26 | $F700 | 13 | 3 | menu screen D: the analysis message screen (5.8, 7.4) [V]; not the shop |
| 27 | $F703 | 1 | 27 | launch [C] |

(Presets 12 and 15 share data. Preset numbers for scenes marked [C] come from where the program selects them,
next to the scene's loads.)

### 7.3 Per-level colours [V]

- **Gameplay:** presets 11 and 10, then level-header bytes 12-17 → shadow entries 9-11 and 13-15 (background
  palettes 2 and 3, colours 1-3) and bytes 18-23 → entries 25-27 and 29-31 (sprite palettes 2 and 3); entry 16
  is set to $0F. Checked against palette RAM for levels 0 and 3. **Level 3 and level 26 rewrite entries 9-11 during play** (level 3: `levels.md` 6.6; level 26: 7.5, the terrain colour cycles through hues 3-B).
- **Approach screen:** preset 0, then 3 bytes from bank 0:$F463 + 3 × level (ROM 0x07463) → entries 9-11 (background
  palette 2: the globe's cells), or → entries 21-23 when (surface picture number AND 3) = 1. **[V]** for levels 0-12
  (palette RAM read after the copy, ninth round). The picture number is RAM $05FF as the program has just set it for
  the level about to be shown (a level-header read for $3B is made a few instructions before the copy), so it is the
  picture of the planet itself: pictures 1 and 5 are used by planets 2, 4, 7 and 9, and for exactly those the
  planet's three colours go to entries 21-23, which nothing on the approach screen draws with (the globe's nine
  sprites use sprite palette 2, entries 25-27 = $30 $10 $00, in every case). Entries 9-11 then keep preset 0's own
  values ($22 $2A $19), so the globes of planets 2, 4, 7 and 9 are drawn in planet 0's colours. Not intended: **Q34**
  (`behaviour.md` 10). A remake that follows the intent writes entries 9-11 for every planet; one that follows the
  original reproduces the four exceptions.
- **Surface picture:** preset 12 + picture number, then 12 bytes from bank 0:$F48A + 12 × level (ROM 0x0748A)
  → entries 0-11 (entry 0 is not uploaded). Checked for levels 0-13.
- Both per-level tables have 13 entries, for the planets (levels 0-12). If the level index is 13 (only by
  forcing it; caverns do not normally show these screens), the program reads the first bytes of the next table
  (for the approach table: the first 3 bytes of the surface table; for the surface table: the first 12 bytes of
  preset 0's data) and shows those colours. [V]

### 7.4 Other palettes

- **Analysis message screen** [V] (palette RAM read in Mesen for list 4, code read): preset 26 → entries 13-15 =
  $32 $22 $12. Then entries 9 and 10 are the two bytes of pair k in the table at bank 0:$D599 (ROM 0x05599, 17
  pairs, k = the analysis list number 0-16, `text.md` 3), all hex: (38,18) (28,18) (38,2A) (34,14) (38,18) (31,11)
  (38,18) (20,00) (30,16) (34,14) (3C,1C) (30,16) (37,17) (20,00) (29,18) (37,17) (35,15); and entry 11 is a copy of
  entry 26 (sprite palette 2, middle colour: the level's own value, level-header byte 19, 7.3) as it stands at that
  moment. Entry 11 is therefore the only one of the three that depends on the level rather than on the list. Entries
  1-8 and 12 stay as the gameplay screen had them.

- **Shop and equip list (5.10)** [V] (palette RAM read in Mesen): during its loads, bytes 1-14 of the 15-byte table at
  bank 2:$932E (ROM 0x1132E; shop) or $933D (ROM 0x1133D; equip list) → entries 2-15; byte 0 of each table is not
  used. Then preset 4 → entries 1-3, and entry 16 = $0F. Resulting background palettes (entries 0-15): shop
  $0F $38 $22 $02 / $00 $27 $16 $06 / $00 $30 $28 $18 / $00 $32 $22 $12; equip list $0F $38 $22 $02 / $00 $30 $11 $01 /
  $00 $30 $10 $00 / $00 $34 $14 $04. Sprite entries (17-31) are not loaded and keep what the previous scene left: after the
  flight they are the flight's preset 2 ($37 $27 $17 / $00 $31 $21 $11 / $00 $39 $29 $19 / $00 $30 $30 $30), so the
  shop's item picture (sprite palette 0) is $37 $27 $17; the equip list comes from the planet menu, so they are the
  planet surface's (planet 0: $20 $10 $00 / $00 $38 $27 $16 / $00 $22 $12 $01 / $00 $32 $12 $26).
- **Bank 7 backdrop** [C]: 16 bytes at bank 7:$832A (ROM 0x3832A) → entries 16-31; then 4 bytes at bank
  7:$833A + 4 × k (ROM 0x3833A) → entries 12-15, where k (0-2) is the position of the level index in the 3-byte
  list at bank 7:$80F8 (`00 04 08`), so the backdrop is used on levels 0, 4 and 8.
- **Attract logo fade-in** [C]: on the sun screen, while the "SOLAR JETMAN" logo appears, every 8th frame one
  step of 7 is applied (steps 0-6) from six 7-byte columns at bank 0:$F7B3 (ROM 0x077B3): column 0 → entry 1,
  column 1 ($F7BA) → entry 2, column 2 ($F7C1) → entry 3, column 3 ($F7C8) → entries 5 and 9, column 4 ($F7CF) →
  entries 6 and 10, column 5 ($F7D6) → entries 7 and 11; byte `n` of each column is used at step `n`. At step 6
  entries 9-11 are then set to three colours embedded in the program code at ROM 0x00276, 0x0027A and 0x0027E
  (ripped). After step 6 the sequence moves on. The 42 bytes run to ROM
  0x077DC.
- **Object colour flash** [C]: a 4-step colour sequence of 3 bytes per step at bank 7:$904E (ROM 0x3904E-0x39059)
  used by one object's effect: every 8th frame the object's counter (values 10-13) selects step (counter − 10),
  whose 3 colours go to entries 9-11; at the last step (counter 13) entries 1-16 are all set to that step's third
  colour, a full-screen flash. Which object it is belongs in `objects.md`.

### 7.5 Level 26 (the final level): palettes during play [V]

(Mesen 2026-10-20: palette RAM of every step picture of `level26.replay`, 3,898 steps, and of two runs of 4,160 steps with the core-hit
flash forced, hooks on every write to shadow entries 9-11 (RAM $7F-$81).)

**Start of play.** The ordinary rules of 7.3 and the level's header bytes 12-23: background palette 0 = $30 $10 $00, palette 1 = $12 $28 $15, palette 2
(entries 9-11) = $34 $24 $04 (header bytes 12-14), palette 3 = $35 $24 $13 (the status panel and its pink gauges); sprite palettes
0-3 = $30 $22 $12 / $38 $27 $16 / $3A $1A $09 / $22 $12 $02; universal background $0F. **Every one of the level's 672 chunks uses
background palette 2** (palette map at bank 0:$EFE1: all values 2; `levels.md` 4.5), so the whole terrain is drawn in palette 2. A remake that
only applies these rules draws the terrain in $34 $24 $04, a pink-purple, **for the whole level**.

**The terrain colour cycles.** The final-level routine of the camera route (`levels.md` 5.2) runs in every A pass in which the player's state
(RAM $0317) is 6 and RAM $9A bit 6 (route finished) is clear. Its first job is to rewrite shadow entries 9, 10 and 11 (RAM $7F, $80, $81):
with h = the byte at bank 0:$AA82 + ($5D >> 4) (ROM 0x02A82, 16 bytes, `$5D` as the A pass starts, the frame counter, even in A passes; its
high nibble selects 0-15):

| index | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| h | 3 | 4 | 5 | 6 | 7 | 8 | 9 | A | B | A | 9 | 8 | 7 | 6 | 5 | 4 |

entry 11 = h, entry 10 = $20 + h, entry 9 = $30 + h (so the colours are the lightest, middle and darkest shades of hue h: 3 purple, 4 magenta, 5-6 red,
7 orange, 8 yellow-brown, 9-A green, B teal-green; the picture of step 100 shows hue 7, orange; that of step 2,000 hue A, green). One hue lasts 16 frames = 8 steps, the
whole cycle 256 frames = 128 steps, so the colour sweeps purple to green and back about 30 times in the 3,900 steps of the route. When
the state is not 6 or the route has finished, this code does not run and the three entries keep their last value.
**What the pictures show:** the palette of the picture of step `n` (`tools/mesen/replay-capture.lua`, `n` = 1, 2, ...) has hue index
((n - 4) >> 3) AND 15 (arithmetic shift, so steps 1-3 use index 15, hue 4, then steps 4-11 index 0, hue 3, steps 12-19 hue 4, ...), 3,898 of 3,898
pictures. That is the cycle above seen through the 8-frame upload of 7.1 (step `m`'s A pass has $5D = 2 (m - 1) counted from the level's first pass; the
counter starts at 0 at the level's first pass and is not otherwise touched in play). Whether a **pause** advances the counter was not checked **[C]**.

**After the route.** At the end of the route (step 4,034 of the run: the camera reaches x = 6,656, RAM $9A bit 6 is set, `levels.md` 5.2) the
cycle stops for good: entries 9-11 stay at the hue of the last A pass before the bit was set. The route takes the same number of steps in every
run (the camera does not depend on the player), so the frozen hue is the same: **hue B ($3B $2B $0B)**, i.e. the end structure and all terrain stay
teal-green. [V] (steps 4,036-4,299: $3B $2B $0B throughout.)

**The core-hit flash** (the final-level background effect's palette, `behaviour.md` 4.16). A shot that hits a core (type 5E, frame $B6) sets RAM
$EC = 1 and $ED = 0. The end routine (which runs once per A pass after the route) then plays a four-entry script from bank 0:$ABE5 into shadow entries 9, 10, 11, one entry
per step, and afterwards sets them from the live-core count n (RAM $03B1, 0-5, the number of objects in slots 23-27 with a state of 6 or more):

| step after the hit | entry 9 | entry 10 | entry 11 |
|---|---|---|---|
| 1 | $20 | T10[n] | $FF (palette RAM takes the low six bits: $3F, black) |
| 2 | $30 | $00 | $FF ($3F) |
| 3 | $30 | $20 | $FF ($3F) |
| 4 | $36 | $26 | $16 |
| then, for good | T9[n] | T10[n] | T11[n] |

T9 = 0F 20 22 37 39 34, T10 = 0F 10 12 27 29 24, T11 = 0F 0F 02 07 09 04 for n = 0-5 (bank 0:$ABF2, $ABF8, $ABFE; ROM 0x02BF2, 0x02BF8, 0x02BFE). So after
the first hit the terrain takes the colours of the number of cores alive: five alive $34 $24 $04 (pink, the header colours), four $39 $29 $09
(green), three $37 $27 $07 (orange), two $22 $12 $02 (blue), one $20 $10 $0F, none $0F $0F $0F (black); each further hit replays the script
and may change it. A core's destruction alone changes nothing (only the hit does). **[V]** by forcing RAM $EC/$ED at step 4,100 with five and with three cores
alive (palette RAM: $20 $24 $3F at the first upload, then $34 $24 $04, resp. $20 $27 $3F, then $37 $27 $07; at the 8-frame upload cadence only the first and last
entries of the four-step script can be seen); the script's bytes **[C]** from the ROM. The same routine also leaves the end structure's **tile animation
unused**: see `behaviour.md` 4.16.

## 8. How this was verified

- Mesen (headless) logged every call of the upload routines with their arguments, and dumped CHR RAM,
  nametable and palette RAM after each scene of a new game, for levels 0-41 (selected by RAM $3B) and the attract
  sequence. CHR rebuilt from those calls with the decoders above matched the dumps tile for tile (title: 512 of
  512 tiles; approach, surface picture, launch: all tiles written by the listed loads).
- Loaders for scenes not reached in play (menus, equipment, ending, the screens of 5.10 and 4.5, the backdrop)
  were run by calling them directly from the title screen; their output matched our decode byte for byte.
- A provenance trace attributed every CHR byte written in those runs to the ROM byte it came from; all such ROM
  ranges are inside the blobs of section 4.

## Open questions

- Users of 1:$D3C5, 1:$DB85, equipment blocks 8-17, item picture 3 (1:$FA74 is the shop's large digits, 5.10). Any caller of the 5.8 loader other than the analysis screen.
- Context of bank 2 entries $8033/$8036 (the shop and equip list, entry $8003, are now identified, 5.10), bank 6 entry $8012, menu screens A and C, the
  sequence that streams 0x0F6AE, and the bank 7 backdrop; their scroll and pattern-table use (1.1).
- Whether the ending picture splits pattern tables.
- Planet count: the 14th approach-screen name ("DRYN KUP LEEZ") and whether level 13 is ever shown as a planet
  in normal play (`levels.md` treats levels 0-12 as the planets).
- The meaning of the game-state flag RAM $D1 (6-tile block); RAM $9A bit 7 (level 26 extra tiles) is the final level
  according to `game-flow.md` [C].
- Animated tiles streamed at run time beyond those in section 3 (gameplay with input has not been traced in
  full).
