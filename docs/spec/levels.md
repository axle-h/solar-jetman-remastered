# Levels: headers, planet maps, terrain collision

What the ROM holds for each of the 42 levels, how a planet map is stored and drawn, how terrain solidity is
described, and which per-level parameters exist. Enough to write an extractor that turns the ROM's level data into
our own map files (category **commit**, `docs/CLEAN_ROOM.md` rule 5), a renderer, and a terrain collision model.
Object and enemy placement is only described as far as the level data points to it (the objects spec owns it).

Addresses follow `README.md`: `ROM 0x1A2B0` is a PRG ROM offset, `bank 3:$9C40` a CPU address with that bank
mapped, `ROM offset = bank × 0x8000 + (address − $8000)`. Status: **[V]** verified against the running original in
Mesen, **[C]** read from the program, **[H]** hypothesis.

## 1. Units and coordinates

| Unit | Size | Name used here |
|---|---|---|
| tile | 8 × 8 px | one background tile of CHR pattern table $1000 |
| block | 16 × 16 px = 2 × 2 tiles | the map's metatile |
| chunk | 128 × 128 px = 8 × 8 blocks | the cell of the map grid |

World positions are 16-bit pixel coordinates, x to the right, y downwards, (0, 0) the top-left corner of the map.
In RAM a position is kept as separate high and low bytes (high byte = 256-px page). [C]

## 2. The 42 levels

The level index is RAM $3B (0-41). It selects the header (section 3), the map (section 4), the palette map (4.5),
the terrain and object sprite sets (`graphics.md` 5.7, 6.4), the items list (3.3) and the placed-object list
(`objects.md` 3).

| Levels | What they are | Status |
|---|---|---|
| 0-12 | the planets: large maps with the mothership (item kind 21, 22 or 24) standing on the surface at the level's home position, non-zero gravity, an approach screen | [V] for the maps; [C] for the rest |
| 13-25 | small closed caverns (6 to 48 chunks, e.g. 2 × 3, 6 × 8, 10 × 4; see 3.1), gravity 0, three items each (kinds 17, 18, 19) | [V] maps, [H] meaning |
| 26 | one long map (56 × 12 chunks), gravity 0, items of kinds 17-19; the program enters it with a forced-scroll flag set (RAM $9A bit 7, under which the camera is not driven by the ship) | [C] flag, [H] meaning |
| 27, 28 | small "technical" rooms (8 × 2 and 6 × 4 chunks), items of kind 20 only | [V] maps, [H] meaning |
| 29-41 | warp levels: all thirteen share one map (bank 3:$B9DE, 6 × 6 chunks), one items list (ten items of kind 16) and all header bytes except the palette bytes 12-23. Terrain tiles are not loaded; the warp tile set is (`graphics.md` 4.5) | [V] map, [C] rest |

Two object lists belong to each level, with **different type numberings**: the *items* list of the level header
(bank 2, section 3.3) and the *placed objects* list of bank 4 (`objects.md` 3). Number of entries per level
(items from header byte 24; placed objects from bank 4:$8147 + L): [V by decoding]

| L | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| items | 6 | 24 | 17 | 21 | 20 | 25 | 25 | 23 | 23 | 21 | 24 | 25 | 20 | 3 | 3 | 3 | 3 | 3 | 3 | 3 | 3 |
| placed | 9 | 79 | 27 | 71 | 38 | 85 | 78 | 35 | 78 | 49 | 84 | 85 | 70 | 2 | 12 | 8 | 9 | 1 | 8 | 4 | 20 |

| L | 21 | 22 | 23 | 24 | 25 | 26 | 27 | 28 | 29-41 (each) |
|---|---|---|---|---|---|---|---|---|---|
| items | 3 | 3 | 3 | 3 | 3 | 3 | 3 | 2 | 10 |
| placed | 4 | 19 | 4 | 11 | 11 | 47 | 1 | 1 | 1 |

Relations seen in the program (they belong to the game-flow spec; listed so the level data makes sense) [C]:

- Level transitions go as follows: from a planet `L` (0-12) to `L + 13`; from levels 13-28 either back
  to `L − 13` or on to `L + 16` (a warp level), chosen by a per-planet flag bit; from a warp level `L` to `L − 29`.
  So caverns 13-25 pair with planets 0-12, and warp levels 29-41 pair with caverns 13-25.
- A small table (bank 0:$9DBA keys, $9DC2 current level, $9DC9 destination) sends level 27 to 4, 5 or 6, level 28 to
  8 or 9, level 3 to 10 and level 0 to 7, keyed by RAM $DB.
- The "next planet" step adds 1 to the level index, except that level 12 is followed by 11 (as if it were 10);
  per-planet flags treat level 12 as level 10, and planet 7 has a route straight to 12. Level 12 is therefore an
  alternative to planet 10 (`game-flow.md` 6). [H]
- Levels 0-13 have an approach-screen name (`rom-map.md`, bank 6:$D2FD, 14 names). Level 13 is shaped like the
  caverns 14-25 and is played as planet 0's second level; the 14th name only appears if the approach screen is
  drawn with level 13 (`game-flow.md` 6). [V by game-flow]

## 3. Level header (bank 2)

Pointer tables: bank 2:$956E (42 low bytes) and $9598 (42 high bytes), indexed by level, point to 27-byte headers in
bank 2:$95C8-$9A35 (ROM 0x115C8). Multi-byte positions are stored **high byte first**. [V for the bytes used by
the map and palettes; C for the others]

| Byte | Meaning | RAM | Status |
|---|---|---|---|
| 0, 1 | camera x at level setup (high, low) | $4D, $4C | [C] |
| 2, 3 | camera y at level setup (high, low) | $4F, $4E | [C] |
| 4, 5 | home x (high, low): where the player starts; on planets 0-12 the mothership object stands here | object x of slot 0 ($021F, $0200); copy in $D7, $D6 | [C], [V] for level 0 |
| 6, 7 | home y (high, low). The player starts 3 px higher (y − 3) | slot 0 y ($027C, $025D) = value − 3; copy in $D9, $D8 | [C], [V] level 0 |
| 8 | map width in chunks − 1 (always odd: width is a whole number of 256-px screens). Also shown as DIAMETER on the approach screen | $37; $38 = byte >> 1 | [V] |
| 9 | map height in chunks − 1 (see 4.2 for how many rows are really used) | $39; $3A = byte >> 1 | [V] |
| 10 | spawn margin `M` in px for the bank 4 placed objects (`objects.md` 3.4). Not used by the items list | $050C; $050D = 256 − byte | [C] |
| 11 | bits 0-2: graphics type (`graphics.md` 6.2), selects terrain tiles, surface picture and block set (4.3). Bits 3-7, read as a signed byte with bits 0-2 cleared: **gravity** (multiples of 8; e.g. level 7 = −24). Shown as GRAVITY on the approach screen | $6F = the byte with bits 0-2 cleared | [V] display (level 7 shows "GRAVITY -24  DIAMETER 29"); [C] use |
| 12-23 | palette colours (ripped, `graphics.md` 7.3) | | [V] |
| 24 | number of entries in the level's items list | $BA | [C] |
| 25, 26 | pointer (**low byte first**, bank 2) to the items list, 5 bytes per entry (3.3) | | [C] |

For levels ≥ 13 the copy of the home y high byte (RAM $D9) is overwritten with $80 after loading, i.e. those levels
have no reachable home (no mothership). [C]

### 3.1 Per-level values (from the headers)

Positions in pixels; "type" = graphics type; items = count @ list address.

| L | camera at setup | home | W × H chunks | byte 10 | type | gravity | items | map | palette map |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 512, 96 | 635, 194 | 8×8 | 144 | 3 | 8 | 6 @ 2:$9A36 | 3:$870B | 0:$F195 |
| 1 | 1536, 128 | 1659, 194 | 32×26 | 64 | 6 | 24 | 24 @ 2:$9E2D | 3:$9032 | 0:$EA07 |
| 2 | 256, 128 | 379, 194 | 24×20 | 192 | 4 | 16 | 17 @ 2:$9C7F | 3:$9F00 | 0:$EC7B |
| 3 | 384, 96 | 507, 194 | 40×15 | 144 | 2 | 8 | 21 @ 2:$9A54 | 3:$874B | 0:$E5F5 |
| 4 | 1792, 128 | 1915, 194 | 30×26 | 224 | 0 | 32 | 20 @ 2:$9C1B | 3:$9BF4 | 0:$E7B5 |
| 5 | 1536, 512 | 1659, 578 | 48×29 | 64 | 1 | 40 | 25 @ 2:$9DB0 | 3:$89A3 | 0:$E887 |
| 6 | 1792, 128 | 1915, 194 | 36×32 | 192 | 3 | 32 | 25 @ 2:$9ABD | 3:$9774 | 0:$E695 |
| 7 | 1408, 128 | 1531, 194 | 30×28 | 64 | 0 | −24 | 23 @ 2:$9CD4 | 3:$A0E0 | 0:$ECF3 |
| 8 | 896, 512 | 1019, 578 | 38×26 | 64 | 1 | 24 | 23 @ 2:$9EA5 | 3:$9372 | 0:$EB71 |
| 9 | 1152, 0 | 1275, 66 | 34×22 | 208 | 4 | 24 | 21 @ 2:$9D47 | 3:$A40A | 0:$EAC7 |
| 10 | 2176, 64 | 2299, 194 | 40×22 | 64 | 1 | 56 | 24 @ 2:$9F18 | 3:$A6F6 | 0:$EDAF |
| 11 | 512, 1088 | 635, 1218 | 46×49 | 64 | 2 | 72 | 25 @ 2:$9B3A | 3:$AA66 | 0:$EE9F |
| 12 | 2816, 64 | 2939, 194 | 32×20 | 64 | 5 | 32 | 20 @ 2:$9BB7 | 3:$B334 | 0:$F0F5 |
| 13 | 0, 0 | 127, 44 | 2×3 | 32 | 1 | 0 | 3 @ 2:$9F90 | 3:$B5B4 | 0:$F1DD |
| 14 | 0, 0 | 128, 128 | 2×8 | 40 | 1 | 0 | 3 @ 2:$9FDB | 3:$B60A | 0:$F1DD |
| 15 | 128, 512 | 256, 608 | 6×6 | 184 | 1 | 0 | 3 @ 2:$9FF9 | 3:$B63E | 0:$F1A5 |
| 16 | 0, 128 | 96, 288 | 6×4 | 40 | 2 | 0 | 3 @ 2:$9F9F | 3:$B5BA | 0:$F1DD |
| 17 | 0, 0 | 128, 128 | 6×4 | 240 | 0 | 0 | 3 @ 2:$9FBD | 3:$B5E2 | 0:$F1DD |
| 18 | 64, 0 | 192, 112 | 4×4 | 40 | 1 | 0 | 3 @ 2:$9FCC | 3:$B5FA | 0:$F1DD |
| 19 | 0, 0 | 96, 128 | 8×2 | 40 | 0 | 0 | 3 @ 2:$9FAE | 3:$B5D2 | 0:$F1DD |
| 20 | 384, 192 | 512, 256 | 6×6 | 24 | 2 | 0 | 3 @ 2:$A008 | 3:$B662 | 0:$F1B1 |
| 21 | 0, 0 | 128, 96 | 6×6 | 40 | 0 | 0 | 3 @ 2:$9FEA | 3:$B61A | 0:$F1DD |
| 22 | 0, 768 | 128, 928 | 6×8 | 40 | 1 | 0 | 3 @ 2:$A017 | 3:$B686 | 0:$F1C7 |
| 23 | 1024, 0 | 1184, 128 | 10×2 | 40 | 2 | 0 | 3 @ 2:$A026 | 3:$B6B6 | 0:$F1BD |
| 24 | 0, 256 | 128, 384 | 10×4 | 24 | 1 | 0 | 3 @ 2:$A035 | 3:$B6CA | 0:$F1D3 |
| 25 | 256, 256 | 368, 384 | 6×6 | 40 | 0 | 0 | 3 @ 2:$A044 | 3:$B6F2 | 0:$F1DD |
| 26 | 0, 896 | 96, 1024 | 56×12 | 128 | 1 | 0 | 3 @ 2:$A044 | 3:$B716 | 0:$EFE1 |
| 27 | 64, 32 | 192, 128 | 8×2 | 200 | 2 | 0 | 3 @ 2:$A053 | 3:$B9B6 | 0:$F1DD |
| 28 | 64, 160 | 192, 256 | 6×4 | 200 | 2 | 0 | 2 @ 2:$A062 | 3:$B9C6 | 0:$F1DD |
| 29-41 | 192, 160 | 320, 256 | 6×6 | 24 | 0 | 0 | 10 @ 2:$A06C | 3:$B9DE | 0:$F1DD |

(Levels 25 and 26 share one items list. The map and palette-map columns come from 4.1 and 4.5.)

### 3.2 When the header is used [C]

- At level setup (before the planet-surface picture) bytes 0-8 and 11 are read: camera, home position, width, the
  graphics type and gravity.
- At the start of play (the launch) bytes 8-11, 12-23 and 24-26 are read: map size, margin, graphics and palettes,
  and the items list is copied to RAM (x low $052D, x high $0561, y low $0547, y high $057B, kind $0513, one entry
  per object).
- Header bytes 0-3 are only the camera during set-up. Every start of play and every reload replaces them with the
  rule of 5.2 ("Camera at the start of play"), which depends only on the player position. (A set-up with RAM $EB
  non-zero places the camera 128 px left of and above the player instead, but the reload that follows replaces that
  too.)

### 3.3 Items list (interface to `economy.md` and `objects.md`)

The header's list is not the list of placed objects in `objects.md` 3 (bank 4). It is a second, smaller list, the
**items list**. Its entries are **items**, and their type number is the **item kind**. Kinds are written in decimal
as in `economy.md` 2.1, which owns what each kind is and yields, with the hex value in brackets. The two numberings
must not be mixed: item kind 16 ($10) is not placed-object type $10. [C]

- **Format.** At the address in header bytes 25-26: `count` (byte 24) entries of 5 bytes: **x (16-bit, low first),
  y (16-bit, low first), item kind**. Positions are world pixels. All lists lie in bank 2:$9A36-$A09D
  (ROM 0x11A36), the "level data reached from the headers" region of `rom-map.md`. [V by decoding]
- **In RAM.** At the start of play the list is copied to RAM (3.2). An entry whose RAM kind is 0 is skipped from then
  on (`economy.md` 2.1). [C]
- **Activation window** (fixed; header byte 10 does not apply). With rx = item x − camera x and ry = item y −
  camera y (signed 16-bit), an item is in range when −48 ≤ rx < 304 and −48 ≤ ry < 304. [C]
- **Spawning** (`objects.md` 10 for the behaviour once spawned):
  - **When.** One call per logic step, in the A pass, after the camera move. Calls also happen on warp levels, but
    not while the player's state is 9. **Start of play:** the first call happens one A pass *before* the first
    player step (in the level-start pass, when the player's step does not yet run), so the first in-range item is
    already in its slot when step 1 begins; the next items follow one per step. [V: Mesen, level 0 launch: scan call
    at the level-start frame with cursor 0 spawned the mothership indicator in slot 11; the first player step came
    one frame later; on level 13 the three items appeared on three consecutive steps]
  - **One call.** It examines up to 4 entries, continuing from a cursor (RAM $BB). Entries whose RAM kind is 0 are
    skipped but count toward the 4. **Passing the last entry ends the call** at once (the cursor goes to 0; the
    entries before the end that were not yet examined in this call wait for the next call), so a call near the end
    of the list can examine fewer than 4 entries. The call stops at the first in-range entry:
    - if that entry is already live (a slot 11-14 holds its index), the cursor moves past it;
    - if it is not live, it is spawned in the lowest free slot of 11-14 and the cursor moves past it;
    - if no slot is free, the cursor stays on it and the entry is tried again next time.
  - **The slot** gets the entry's position, its list index (RAM $0393 + slot) and its kind (the type field, RAM
    $03B2 + slot). Its initial state (RAM $0317 + slot) is the byte at bank 2:$86CD + kind, and its first frame
    the byte at bank 2:$86B1 + kind.
  - **Despawn** follows `objects.md` 3.5, with that rule's margin from header byte 10. Where that margin is smaller
    than the 48 px spawn window (levels 13, 20, 24, 29-41), an item just outside the screen can be spawned and
    freed again on alternate steps. That is harmless: it is never on screen. The remake does not reproduce it
    (`docs/deviations.md`, "Item spawn churn"; `objects.md` 3.5).

  [C] unless marked
- **Kinds used** (count of entries over all levels; state and frame from those two tables; meanings in
  `economy.md` 2.1):

  | Kind | Count | State | Frame | | Kind | Count | State | Frame |
  |---|---|---|---|---|---|---|---|---|
  | 1 ($01) | 26 | $0A | $54 | | 16 ($10) | 166 | $0A | $66 |
  | 3 ($03) | 1 | $0A | $59 | | 17 ($11) | 50 | $0E | $67 |
  | 4 ($04) | 1 | $0A | $5A | | 18 ($12) | 27 | $0A | $52 |
  | 5 ($05) | 2 | $0A | $58 | | 19 ($13) | 14 | $0A | $53 |
  | 6 ($06) | 1 | $0A | $57 | | 20 ($14) | 5 | $0A | $52 |
  | 9 ($09) | 29 | $0A | $5E | | 21 ($15) | 9 | $07 | $FF (invisible) |
  | 10 ($0A) | 50 | $0A | $5C | | 22 ($16) | 3 | $07 | $8A |
  | 11 ($0B) | 26 | $0A | $60 | | 24 ($18) | 1 | $07 | $8C |
  | 12 ($0C) | 22 | $0A | $61 | | 26 ($1A) | 1 | $0E | $67 |
  | 15 ($0F) | 13 | $0A | $65 | | 27 ($1B) | 4 | $0A | $7E |

  [V by decoding]
- **The mothership entry.** On planets 0-12 entry 0 is the mothership (kind 21, 22 or 24) at the home position
  (bytes 4-7). [V by decoding, all 13] As `economy.md` 3.4 says, its kind is also the mothership's fuel count
  (full at 25). This agrees with the decoded program:
  - a fuel delivery adds 1 to RAM $0513, which is the kind byte of entry 0 in RAM;
  - the value is saved to RAM $0636 when the player leaves the planet for its second level, and written back to
    $0513 on return.

  So the header gives the starting fuel count: 21, 22 or 24. [C]
- **Kind 15 → 16.** On planets 0-12 an entry of kind 15 whose index equals RAM $0100 or RAM $0101 is loaded as
  kind 16 instead. Both bytes are set to $FF at START LEVEL (`game-flow.md` 7.2). When a towed crystal has been spun
  nine times (`economy.md` 2.6), its kind becomes 16 and its list index is stored in RAM $0100 if that is still $FF,
  otherwise in $0101. [C]
- **Entries outside their map.** Level 1 entries 19 (x 5005, kind 11) and 20 (x 5230, kind 17) lie right of a
  4096-px-wide map. Level 9 entry 15 (kind 9) has x 2770, inside the map, but y 3540, below a 2816-px map. The
  window above is two-dimensional, and the camera's limits (5.2) keep rx ≥ 1165 for the level 1 entries and
  ry ≥ 924 for the level 9 entry, so none of them can ever be spawned. [C]
- **Level 26** uses level 25's list: three items (kinds 17, 18, 19) at (118, 252), (368, 384), (640, 96), all in
  the top-left corner of a 7168 × 1536 map whose home is (96, 1024). The data says so; whether it is intended is
  open. [V by decoding]

## 4. Planet map format (bank 3, with a palette map in bank 0)

A map is a grid of chunk numbers; a chunk is 8 × 8 block numbers; a block is 2 × 2 tile numbers. Nothing is
compressed. [V]

### 4.1 Where the data is

| What | Where | Format |
|---|---|---|
| Map pointer per level | bank 3:$859A-$85ED (ROM 0x1859A): 42 little-endian words | address in bank 3 |
| Maps | bank 3:$870B-$BA01 (ROM 0x1870B-0x1BA01) | 1 byte per chunk, row by row |
| Block sets | three sets of 170 blocks, 4 bytes each: set 0 at bank 3:$BA02, set 1 at $BF52, set 2 at $BCAA (ROM 0x1BA02-0x1C1F9). Set pointers: low bytes at bank 3:$8594-$8596, high at $8597-$8599 | per block: top-left, top-right, bottom-left, bottom-right tile number |
| Chunk definitions | bank 3:$C1FA-$FF79 (ROM 0x1C1FA-0x1FF79): 246 chunks of 64 bytes, shared by all levels | per chunk: 8 rows of 8 block numbers, top row first |
| Palette map pointer per level | bank 0: low bytes at $E1D5, high at $E1FF (42 each; ROM 0x061D5, 0x061FF) | address in bank 0 |
| Palette maps | bank 0:$E5F5-$F1E8 (ROM 0x065F5-0x071E8) | 2 bits per chunk (4.5) |

The block set is chosen by the level's graphics type through the surface picture number (`graphics.md` 5.5):
`set = picture number AND 3`, which gives graphics types 2, 3, 5 → set 0; 0, 4 → set 1; 1, 6 → set 2. The three
sets match the three terrain tile sets. [V]

Tile numbers are background tiles of pattern table $1000 as loaded for that level (`graphics.md` 5.7). [V]

### 4.2 Size and storage

- `W = byte 8 + 1` chunks across, `H = byte 9 + 1` chunks down; map width in px = 128 W, height = 128 H. [V]
- Row stride `S = 2 × ((byte 8 >> 1) + 1)`, which equals W because byte 8 is always odd. Chunk (cx, cy) is the
  byte at `map + cy × S + cx`. [V]
- **Rows beyond H.** The program can address rows 0 to `H` (one more than the header says) and reads whatever
  bytes follow the map. When byte 9 is even, the camera's lower limit (5.2) lets that extra row be seen. The
  extractor should therefore read `H + 1` rows of `S` bytes for every level and keep the extra row as part of the
  map. Maps share bytes: level 9's map (bank 3:$A40A) starts at level 7's header row 27 ($A0E0 + 27 × 30), its last row within H; level 7's extra row 28 is then bytes 30-59 of level 9's map (level 9's stride is 34); levels 29-41 use one map; the extra
  row of level 3 is the first row of level 5. Bank 3:$8F13-$9031 (between the maps of levels 5 and 1) and
  $974E-$9773 (between levels 8 and 6) belong to no map; the extra rows of levels 5 and 8 read the start of
  them. [V] (rows up to H compared in Mesen; the screen
  only ever showed rows the camera can reach)
- No horizontal or vertical wrap-around. [C]

### 4.3 Decoding a tile

For a tile at tile coordinates (tx, ty) (8-px units):

```
chunk  = map[(ty / 16) * S + (tx / 16)]
block  = chunks[chunk * 64 + ((ty / 2) mod 8) * 8 + (tx / 2) mod 8]
tile   = blockset[block * 4 + (ty mod 2) * 2 + (tx mod 2)]
palette = palette_of_chunk(tx / 16, ty / 16)          (4.5)
```

All divisions round down. Chunk numbers used: 0-245; block numbers used: 0-169 (sets have exactly 170 blocks).
Chunk 0 is entirely block 0, and block 0 of every set is four copies of tile 0 (empty sky). [V]

### 4.4 Extractor outline

For each level 0-41: read the header; W, H, S as above; copy `(H + 1) × S` chunk bytes; note the block set number;
copy the palette map (4.5): `H / 4 + 1` rows (rounded down) of `S` bytes, which covers chunk rows 0 to `H`. Write
once, shared by all levels: the 246 chunk definitions, the three block sets of 170 blocks, the three block →
shape-class maps and the 53 shape classes (6.2). Levels 29-41 can reference one map. None of these bytes are
graphics; the tile numbers refer to the ripped tile sets.

### 4.5 Palette map (which background palette each chunk uses)

Each 128 × 128 chunk uses one background palette for all its tiles. The palette map of a level has the same row
stride `S` as its map and one row per **four** chunk rows:

```
byte    = palette_map[(cy / 4) * S + cx]
palette = (byte >> (2 * (cy mod 4))) AND 3
```

Values seen: 0, 2 and 3 (palettes 0-1 are the fixed gameplay palettes, 2-3 come from header bytes 12-17;
`graphics.md` 7.3). The caverns, technical rooms and warp levels use mostly palette 3 (bytes $FF). Palette maps
overlap one another like the maps. [V] (attribute tables matched in all 126 checked screens)

Because a chunk is 128-px aligned and the screen's 16 × 16 attribute areas stay aligned to blocks while scrolling,
every block shows exactly its chunk's palette; the remake can colour per chunk. [V]

## 5. What is on screen

### 5.1 Layout

- The playfield is the top of the screen, lines 0-192; the status panel (HUD) is below it, drawn from the other
  nametable page. The leftmost 8 pixel columns of the playfield are blanked. [V]
- The playfield shows the map window whose top-left corner is the camera (RAM $4D/$4C x, $4F/$4E y): screen pixel
  (sx, sy) shows world pixel (camera x + sx, camera y + sy). [V]

### 5.2 Camera

- **Timing.** The player moves only in the A pass of each logic step (`game-flow.md` 1.2). The camera code runs in
  both passes, so every frame, and spreads each logic step's movement over the two frames. The frame counter RAM
  $5D is even while the camera runs in the A pass and odd in the B pass. [V]
- **What `d` is.** Per axis, `d` is the change of the player's position low byte over the A pass's player handling,
  as an 8-bit signed difference. RAM $42/$43 keep the x/y low bytes taken at the start of that handling, before
  the state handler moves the player and before the terrain corrections; the camera compares them with the position
  afterwards. The B pass sees the same `d`. It is a position change, not derived from the speed. [C]
- **Half-steps.** The A frame moves `d / 2` truncated toward zero; the B frame moves the rest. [V] Examples:
  - d = +5: A frame +2, B frame +3.
  - d = −5: A frame −2, B frame −3.
  - d = ±4: ±2 on each frame.
- **Size limit.** A half-step larger than +8 becomes +8. A half-step of −9 is let through and anything below −9
  becomes −8. This off-by-one asymmetry looks unintended; normal play never reaches it (speeds are at most 4 px per
  logic step). [C]
- **Band test, every frame.** Each frame's half-step is applied only if, at that moment (before that frame's move),
  the player's screen position is outside the band in the direction of the step. The screen position is the
  player position minus the camera, as a 16-bit difference, and counts only when it is 0-255 (otherwise there is no
  move):
  - right: screen x ≥ 144 ($90); left: screen x < 112 ($70);
  - down: screen y ≥ 96 (RAM $4B, always $60); up: screen y < 64 (RAM $4A, always $40).

  So the B half-step can be refused after the A half moved the camera. A refused half-step is lost: the camera never
  catches up on its own. [C; vertical band V]
- **RAM $96** is added to the vertical half-step and then cleared on every camera run, but nothing in the program
  ever sets it, so it is always 0. [C]
- **Limits.** x from 0 to `256 × (byte 8 >> 1)` (= map width − 256); y from 0 to `256 × (byte 9 >> 1) + 56`. A
  half-step that would pass a limit stops at the limit. In the frame in which the camera arrives at a limit, the
  recorded half-step (RAM $50 for x, $51 for y, the values the parallax-star job reads, `graphics.md` 5.7.3) is set
  to 0 although the camera may have moved: x right, when the high byte reaches the limit's (camera put exactly on
  the limit); x left and y top, when the move would go below 0; y bottom, when the move passes the limit by 1 or
  more. Ending exactly on 0 or exactly on the bottom limit is not zeroed. **[V]** for all four limits (level 0, Mesen, 2026-10-17: pod held at a
  steady 2-3 px per step towards each limit from many start offsets; x limit 768, y limit 824): the camera is stopped at the limit
  in every case, the recorded half-step is 0 in the frame in which a move would pass the limit (below 0, or the bottom limit by 1
  or more; also in every frame spent at a limit), and it keeps its value (e.g. -1, -2, +1, +2 per frame) when the move ends exactly on 0
  or exactly on the bottom limit. The playfield is 193 lines tall (5.1), so with odd byte 9
  the bottom 7 pixel lines of the map are never shown; with even byte 9 the top 121 lines of the extra chunk row
  (4.2) can be. [C]
- **Frozen.** The camera code does nothing (no move, no map streaming) while the player's state is 4 (pod exploding)
  or 8 (Jetman assembling a spare pod, `player.md` 6.3). The player's movement during those steps is not made up
  later. [C]
- **Camera at the start of play** (every reload: the launch, level changes, respawn). The camera is placed from the
  player position (x, y):
  - cx = x − 128, at least 0; if its high byte reaches `byte 8 >> 1`, cx = `256 × (byte 8 >> 1)`.
  - cy = y − 80, at least 0; if its high byte reaches `byte 9 >> 1`, cy = `256 × (byte 9 >> 1) + 48`. Note +48,
    not the +56 of the scroll limit. This also raises a cy whose high byte equals the limit page but whose low byte
    is below 48.
  - Then both are rounded down to a multiple of 32 and the screen is drawn once.

  Examples (player position → camera): level 0 (635, 191) → (480, 96); level 22 (128, 925) → (0, 800). [C, matches
  the reviewer's observations on levels 0, 5, 7, 11, 22]
- **Level 26 (final level, RAM $9A bit 7).** The camera follows a fixed route, not the player.
  - **Route.** Two 26-byte tables in bank 0, indexed by the route step RAM $E0 (0 at the start). The waypoints are
    at $AB97 (ROM 0x02B97); waypoint `w` is camera x = 256 × (w AND $1F) + (w AND $E0). The moves are at $ABB1:
    bits 0-1 give x (01: +1, bit 1 set: −1, 00: none) and bits 2-3 give y (bit 2 alone: +1 down, bit 3 set: −1
    up). 0 means the end.
  - **Parallax stars.** Not suppressed here: the star job runs as on every non-warp level (RAM $9B is 0), in state 6 too; it is off only while its mode is 0, which is
    most of the flight because the sky chunk is non-zero (`graphics.md` 5.7.3, with the replay's step table) [V 2026-10-20].
  - **Start camera.** The rule of "Camera at the start of play" applies unchanged (it does not read the final-level
    flag): player (96, 1021) gives cx = 0 and cy = 1021 − 80 = 941, 928 after rounding to 32 (the map is 12 chunks high,
    so no limit applies). The header camera y of 896 is only the set-up value and is replaced. The route's
    movement starts from camera (0, 928). [C]
  - **Movement.** While the player's state is 6 (the final-level flight), the camera moves by a move (mx, my),
    each −1, 0 or +1 px per frame, with no band test and no limits. Step i's move applies until the camera x
    reaches waypoint i (its high byte equal to w AND $1F, and its low byte equal to w AND $E0 or one more); then
    $E0 advances. **[V]** (13,600 steps in three runs)
  - **Order within a logic step.** (mx, my) starts at (0, 0). In the A pass, after the player's step and the
    objects: the camera moves by the previous (mx, my); then the route code runs once: it compares the camera
    (as just moved) with waypoint $E0 (advancing $E0 by at most 1), sets the new (mx, my) from the move byte, and
    applies the carry to the pod (`player.md` 9.1). In the B pass the camera moves by the new (mx, my). So in a
    step the camera moves by the old plus the new move: 2 px per axis on a straight stretch, one of each at a
    change, and 1 px in the level's first step. The route code does not run when the player's state is not 6: at
    the pod's death (`player.md` 9.1) (mx, my) is zeroed and the camera stops. **[V]**
  - **End.** In the step in which the move byte is 0 (step 25, after the camera reaches x = 6656): (mx, my) =
    (0, 0), RAM $9A bit 6 is set, $E0 = $80, the end routine's variables below are cleared (except the vertical
    speed fraction $40), and this step has no carry. **The camera does not stop.** From the next step the **end
    routine** replaces the route code (same place, once per step) and sways the camera around x about 6,690 and y
    about 470-560 (the end structure). The rules below are **[V]** over 1,470 end steps with random input (n = 5);
    the extent beyond that is from the rules ([C]: x 6,661-6,712, y 466-562 over 26,000 steps without input).
    - n = the number of the objects in slots 23-27 with a state of 6 or more. If n = 0 and RAM $5D AND $1E is 0,
      the final sequence starts (`scenes.md` 14.1, `game-flow.md` 7.4) and nothing below runs; if n = 0 otherwise,
      n = $5D AND $1E (never below 2). [C] The first part is **[V]** by a natural play of the level, 2026-10-17: the last core
      destroyed by shots, n = 0 at once (its explosion, state 5, does not count), the sequence began at the next step with
      RAM $5D AND $1E = 0 (`behaviour.md` 4.16).
    - It keeps one *guide* per axis (not an object; slot 30's bytes): a speed (8.8), a direction byte and a
      position byte, all 0 at the end except the vertical speed 0.40. Only differences of the position byte matter.
    - **Vertical.** u = 1 if cy ≥ 512, else 0. If u equals the direction (0 up, 1 down): speed + 0.04, and any
      whole part becomes exactly 1.00 (so the speed is at most 1.00). Otherwise speed − 0.03 (not below 0), and at
      exactly 0 the direction flips. Move the position by the speed (down adds, up subtracts, 8.8). If n = 1, add
      j to the position byte: j = (RAM $5A AND 7) − 4, or 4 if that is 0 [C]. my = old position − new position
      (signed byte), limited to −3..+3: the camera goes the opposite way to the guide.
    - **Horizontal.** c = 1 if (cx AND 255) ≥ $1C, else 0. If c differs from the direction (0 right, 1 left):
      speed + 0.03 (cap 3.00). Otherwise speed − 0.02 (not below 0), and at exactly 0 the direction flips. Move the
      position (left subtracts). mx = old − new (signed byte, at most 3 either way). Then, if mx ≥ 0 and
      (cx AND 255) ≥ $38, or mx < 0 and (cx AND 255) < 8, the direction becomes 0 resp. 1 and the speed 0.
    - Then the pod's carry (`player.md` 9.1) with this (mx, my). In the A pass the camera's move uses this
      step's *previous* mx and the my that the pod's step left (`player.md` 9.1, "After the route's end"). **[V]**
  - **Terrain colour [V] (2026-10-20).** The same routine, at the start of every A pass in which the player's state is 6 and the route is not finished (RAM
    $9A bit 6 clear; it runs before the route code and so also in the last step of the route), rewrites the palette shadow entries 9-11 (RAM $7F-$81, background palette 2, which
    every chunk of this level uses, 4.5) from the frame counter: hue h = table[$5D >> 4] with the 16-entry table 3 4 5 6 7 8 9 A B A 9 8 7 6 5 4 (bank 0:$AA82), entry 9 = $30 + h, entry 10 = $20 + h,
    entry 11 = h. The terrain therefore changes colour continuously (8 steps per hue); the header's $34 $24 $04 is only the value of the first three steps. After the
    route the cycle stops at hue B (green). Full rule, the screen-visible timing and the core-hit flash: `graphics.md` 7.5.
  - Moves: `01 09 01 05 06 05 06 05 01 09 0A 09 0A 09 01 09 01 05 01 05 06 05 01 09 01 00`.
  - Waypoints: `82 84 66 47 86 07 C6 07 8D 0E 0D 8D 0D 8D 8E CE 11 D1 D3 D5 95 D5 17 19 1A 3F`.
  - The player is carried along the route by the rules of `player.md` 9.1 (the pod's speeds are set by the
    route code, and the screen limits do the rest).

  [C]

### 5.2a Beyond the map's left and right edges, and header byte 8 (analyst, 2026-10-25) **[V]**

- **Byte 8 is odd on every one of the 42 levels** (values 7, 31, 23, 39, 29, 47, 35, 29, 37, 33, 39, 45, 31, 1, 1, 5, 5, 5, 3, 7, 5,
  5, 5, 9, 9, 5, 55, 7, 5, then 5 on levels 29-41; the width is a whole number of 256-px pages, `levels.md` 4.2). So the camera's x
  limit 256 × (byte 8 >> 1) is exactly map width − 256: **in the original the screen never extends past the map's right edge, nor
  past the left edge (camera x ≥ 0)**, and the horizontal question does not arise. (The vertical analogue does arise: with
  an even byte 9, levels 3, 5, 11 and 13, the camera's lower limit shows the extra chunk row of 4.2.) A byte 8 that was even
  would give a limit 128 px short of the true width plus 256, i.e. the screen would end 128 px past the edge; no level has it.
- **What the original's own drawing puts there** if the camera is forced past an edge (the full-screen draw at level start redirected to a
  chosen camera, level 0 whose limit is 768, Mesen, name table dumped 40 frames later), for a wide view that wants to know:
  - **Right of the map (camera 896: 128 px beyond):** the draw reads the map as one row-major list of chunks, so the columns past the last
    chunk column show **the next chunk row's first chunks**: tile (tx, ty) for tx ≥ W×16 is tile (tx − 16 W, ty + 16), and the attribute
    (palette) cells come from the same chunks (384 of 384 tile cells and 96 of 96 attribute cells matched). For the last chunk row that
    next row is the extra row of 4.2. This is a **wrap into the next map row**, not sky and not tile 0.
  - **Left of the map (camera −128, high byte $FF):** the chunk index is computed from the unsigned page, so the columns left of x = 0 show
    tiles of chunk bytes from far outside the map (terrain-looking fragments unrelated to the level): **garbage**, not the previous
    row's last chunks.
  - Collision (6.4, "Beyond the map") is separate: the pod cannot get there.
  - **Remake:** for a view wider than the map's edges use empty sky (tile 0, palette 0) on both sides; no original behaviour exists
    to copy, and the two behaviours above are artefacts of an unchecked index.

### 5.3 Drawing while scrolling

The original keeps the playfield in one 32 × 30-tile nametable used as a ring: map tile column `tx` is kept in
nametable column `(tx − camera column at the full draw + scroll column) mod 32`, rows likewise mod 30, and each
frame it rewrites the one tile column and/or tile row that enters view (on the side the camera moved to), with the
matching attribute bytes. The visible result is always exactly the map window of 5.1; nothing else is drawn by the
map code. A remake can draw the window from the map directly. [V] (126 nametable dumps: 100 800 visible tiles
compared, 28 differed, all in an edge column being rewritten; 24 316 attribute areas, 72 differed, at edges)

### 5.3.1 Which camera value a frame shows

**Rule.** The picture output during frame n shows the camera as RAM $4C-$4F held at the end of frame n − 1.
[V] (level 0 launch flight and level 5 free flight, 330 frames: the scroll written in each vertical blank always
equalled the camera of the frame before; the raw picture of every frame matched our render at that camera)

**Why** (hardware timing):

- The scroll registers can only be set safely in the vertical blank. The game writes them there, at the start of
  each frame's handler, from RAM $02/$03 (scroll x, y) and the page bit.
- Those RAM bytes change together with the camera in the camera code (5.2), which runs later, during the frame's
  logic pass.
- So each pass's camera move reaches the screen one frame later. The new tile row or column for that move is
  written in the same vertical blank as the scroll, so they appear together. [C]

The lag is one frame in every frame, A or B. Overruns do not change it: the only overruns seen happen after the
status-bar split, which comes after the camera code (`game-flow.md` 1.4). [C]

**For replays.** Pair the screen of frame n with the camera sampled at the end of frame n − 1. In Mesen (observed
with MesenCE in this project):

- `emu.getScreenBuffer()` called in an end-of-frame callback returns the picture of the frame that just ended
  (lag 1, as above);
- `emu.takeScreenshot()` called at the same point returns the picture of the frame before it, so it appears to lag
  the camera RAM by 2 frames. [V]

A capture that samples RAM elsewhere (for example when the pass reaches its idle loop, `game-flow.md` 1.4) can
appear to lag by yet another frame. Anything beyond the one-frame rule is an artefact of the capture, not of the
game.

**Remake.** The one-frame delay is a hardware-timing effect. A remake that draws the camera of the current logic
tick differs from the original by exactly this frame; whether to keep it is a `docs/deviations.md` decision.

### 5.4 The mothership is map tiles

On planets, the mothership seen on the surface at the home position is drawn by the map itself (blocks of the
block set; e.g. level 0, chunk row 1). During the launch sequence the program overwrites some of those nametable
tiles to animate it (seen in level 0, map tile rows 19-25 around x 600-750); that animation belongs to the
game-flow/objects specs. [V]

## 6. Terrain collision

### 6.1 Model

Solidity is not stored per tile or per pixel. Each block number has a **shape class** (0-52), and each class is a
short list of **line segments** in the block's own pixel coordinates (0-15, a few reach to −2 or 17 to close
gaps). Each segment has one of 16 **directions** that says whether it is a floor, a wall or a ceiling, its slope,
and which side is solid. A block whose class has no segments (class 0 and others listed in 6.3, including all fully
solid interior blocks and all sky) never collides: the surfaces of the terrain are what stops the player, not the
interior. Block 0 and block numbers ≥ 170 never collide. [C] (data decoded and checked for consistency: every class
parses, every separator repeats the header direction)

### 6.2 Data

- Block → class maps (bank 2), one per block set, 170 bytes each: set 0 at $F796, set 1 at $F6EC, set 2 at $F840
  (ROM 0x17796, 0x176EC, 0x17840). Their addresses are also listed per set in bank 2:$95C2 (low bytes, 3) and $95C5
  (high bytes, 3), indexed by `picture number AND 3`.
- Class records: pointer tables bank 2:$F8EA (53 low bytes) and $F91F (53 high bytes); records in
  bank 2:$F954-$FACB.
- Record format: the directions of its `n` segments (one byte each, 0-15), then $FF; then the `n` segments in that
  order. A segment is 3 bytes for directions 0, 7, 8, 9 and 4 bytes for the others. Between two segments stand two
  bytes: the next segment's direction again, and $FF. After the last segment: $FF. A record that is just $FF has no
  segments. Coordinate bytes are signed 8-bit.

Segment fields (block-local pixels; `t` = column − x0, `/2` rounds down):

| Dir | Surface | Solid side | Bytes | Line, for x0 ≤ column ≤ x0 + w | Contact angle | Push |
|---|---|---|---|---|---|---|
| 0 | flat floor | below | x0, w, y | y | (0) | up |
| 1 | floor, shallow, falling to the right | below | yLow, x0, w, yHigh | yHigh + t/2 | $15 | up |
| 2 | floor, shallow, rising to the right | below | yLow, x0, w, yHigh | yLow − t/2 | $EB | up |
| 3 | floor, 45°, falling | below | yLow, x0, w, yHigh | yHigh + t | $20 | up |
| 4 | floor, 45°, rising | below | yLow, x0, w, yHigh | yLow − t | $E0 | up |
| 5 | floor, steep, falling | below | yLow, x0, w, yHigh | yHigh + 2t | $2A | up |
| 6 | floor, steep, rising | below | yLow, x0, w, yHigh | yLow − 2t | $D6 | up |
| 7 | wall, face points right | left of x | x, y0, h | vertical from y0 to y0 + h | | right, 1 px |
| 8 | wall, face points left | right of x | x, y0, h | vertical from y0 to y0 + h | | left, out of the wall |
| 9 | flat ceiling | above | x0, w, y | y | | down |
| 10 | ceiling, 45°, rising | above | yTop, x0, w, yBottom | yBottom − t | $60 | down, x + 1 |
| 11 | ceiling, 45°, falling | above | yTop, x0, w, yBottom | yTop + t | $A0 | down, x − 1 |
| 12 | ceiling, steep, rising | above | yTop, x0, w, yBottom | yBottom − 2t | $55 | down, x + 1 |
| 13 | ceiling, steep, falling | above | yTop, x0, w, yBottom | yTop + 2t | $AA | down, x − 1 |
| 14 | ceiling, shallow, rising | above | yTop, x0, w, yBottom | yBottom − t/2 | $6A | down, x + 1 |
| 15 | ceiling, shallow, falling | above | yTop, x0, w, yBottom | yTop + t/2 | $95 | down, x − 1 |

"yLow" is the larger y (lower end on screen), "yHigh" the smaller. The contact angle is a direction in 1/256 turns
(0 = straight up, increasing clockwise), approximately the surface normal; `player.md` 5 says what the
player does with it. [C]

### 6.3 The 53 classes, decoded

`dir: (bytes) → endpoints` in block-local pixels (x, y).

| Class | Segments |
|---|---|
| 0, 3, 6, 9, 20, 23, 26, 29, 33, 35, 37 | none |
| 1 | 0: (00 0F 00) → (0,0)–(15,0) |
| 2 | 2: (0F 00 0F 08) → (0,15)–(15,8); 8: (00 10 01) → (0,16)–(0,17) |
| 4 | 2: (07 00 0F 00) → (0,7)–(15,0) |
| 5 | 4: (0F 00 0F 00) → (0,15)–(15,0); 8: (00 10 01) → (0,16)–(0,17) |
| 7 | 6: (0F 00 07 00) → (0,15)–(7,1); 8: (00 10 01) → (0,16)–(0,17) |
| 8 | 6: (0F 08 07 00) → (8,15)–(15,1); 8: (08 10 01) → (8,16)–(8,17) |
| 10 | 8: (00 00 0F) → (0,0)–(0,15) |
| 11 | 13: (00 09 08 13) → (9,0)–(17,16) |
| 12, 14, 16 | 13: (0E 00 02 10) → (0,14)–(2,18) |
| 13 | 13: (00 00 08 11) → (0,0)–(8,16) |
| 15 | 11: (00 00 10 10) → (0,0)–(16,16) |
| 17 | 15: (00 00 11 08) → (0,0)–(17,8) |
| 18 | 15: (08 00 11 10) → (0,8)–(17,16) |
| 19 | 9: (00 0F 10) → (0,16)–(15,16) |
| 21 | 14: (00 00 11 08) → (0,8)–(17,0) |
| 22 | 14: (08 00 11 10) → (0,16)–(17,8) |
| 24 | 10: (00 00 10 10) → (0,16)–(16,0) |
| 25 | 12: (00 08 08 11) → (8,17)–(16,1) |
| 27 | 12: (00 00 08 11) → (0,17)–(8,1) |
| 28 | 7: (10 00 0F) → (16,0)–(16,15) |
| 30 | 5: (0F 00 07 00) → (0,0)–(7,14); 7: (07 10 01) → (7,16)–(7,17) |
| 31 | 5: (0F 08 07 00) → (8,0)–(15,14); 7: (0F 10 01) → (15,16)–(15,17) |
| 32 | 3: (0F 00 0F 00) → (0,0)–(15,15); 7: (0F 10 01) → (15,16)–(15,17) |
| 34 | 1: (07 00 0F 00) → (0,0)–(15,7) |
| 36 | 1: (0F 00 0F 08) → (0,8)–(15,15); 7: (0F 10 01) → (15,16)–(15,17) |
| 38 | 0: (00 0F 00) → (0,0)–(15,0); 8: (00 00 0F) → (0,0)–(0,15) |
| 39 | 0: (00 0F 00) → (0,0)–(15,0); 7: (10 00 0F) → (16,0)–(16,15) |
| 40 | 8: (00 00 0F) → (0,0)–(0,15); 9: (00 0F 10) → (0,16)–(15,16) |
| 41 | 7: (10 00 0F) → (16,0)–(16,15); 9: (00 0F 10) → (0,16)–(15,16) |
| 42 | 8: (08 00 0F) → (8,0)–(8,15) |
| 43 | 7: (00 00 0F) → (0,0)–(0,15) |
| 44 | 0: (00 0F 08) → (0,8)–(15,8) |
| 45 | 0: (00 1F 00) → (0,0)–(31,0); 8: (0F FE 04) → (15,−2)–(15,2) |
| 46 | 3: (03 00 04 00) → (0,0)–(4,4); 7: (04 04 0B) → (4,4)–(4,15) |
| 47 | 4: (03 0C 04 00) → (12,3)–(16,−1); 8: (0C 04 0B) → (12,4)–(12,15) |
| 48 | 7: (04 00 0F) → (4,0)–(4,15) |
| 49 | 8: (0C 00 0F) → (12,0)–(12,15) |
| 50 | 0: (00 0F 0F) → (0,15)–(15,15); 7: (04 00 0F) → (4,0)–(4,15) |
| 51 | 0: (00 0F 0F) → (0,15)–(15,15); 8: (0C 00 0F) → (12,0)–(12,15) |
| 52 | 3: (0F 08 08 08) → (8,8)–(16,16); 4: (0F 00 08 08) → (0,15)–(8,7) |

Classes 12, 14 and 16 share one record. The flat segment of class 45 spans two blocks.

### 6.4 How the program tests the player against it [C]

Only the player is tested: all five places that run the test pass object slot 0. They are the handlers of
player states 1, 2, 3 and 4 (`game-flow.md` 9.1) and one more player routine. Everything below is read from the
program; only the decoded data is verified. The responses (bounce, damage, turning) are summarised so the terrain
side is complete; `player.md` owns them.

**Speeds.** Speeds are kept as unsigned magnitudes in 8.8 fixed point (whole pixels per logic step, then 1/256),
with separate direction bits: horizontal speed RAM $02BA (whole) / $029B (fraction), direction RAM $0336 bit 0
(0 = right, 1 = left); vertical speed RAM $02F8 / $02D9, direction RAM $0355 bit 0 (0 = up, 1 = down). Position
fractions: x RAM $0197, y RAM $023E (`player.md` 4.9). Below, "whole vertical speed" means the unsigned whole part
(RAM $02F8), whatever the direction. "Halved" always means the whole 8.8 value is halved (fraction included,
rounded down). Fractions are written in hex after the point: 1.80 = 1.5 px. Every position correction below
changes only the whole-pixel position; both fractions are kept.

**Place in the logic step** (A pass). The player routine first stores the position low bytes for the camera
(5.2), then runs the state's handler. For the order inside each state's step see `player.md` 4.2 (pod: gravity
with the towing pull, the 4.00 cap, the position update, this test, then rotation and thrust) and 6.1-6.2
(Jetman). This agrees with the decoded program. The three stages below (floors, walls, ceilings) always all
run, in that order; a hit ends only its own stage.

1. **Window.** Take the 3 × 3 blocks whose top-left block contains world point (x − 16, y − 16), where (x, y) is
   the player's position. If x − 16 is negative the left column is empty; if y − 16 is negative the top row is
   empty (the window keeps its origin 16 px left of / above the player's block). Window coordinates: the top-left
   block's corner is (0, 0); each segment is moved by 16 × its block's column and row. All comparisons are on
   unsigned 8-bit window coordinates (a segment coordinate below 0 wraps to 240-255).
   - **Collection order.** Blocks row by row from the top-left (top row left to right, then the middle and bottom
     rows), and within a block in record order. Each direction keeps its own list in that order.
   - **Storage and its overflow [V] (2026-10-20).** The original keeps all the window's segments in one byte buffer that starts at RAM $0596: for each direction d = 0-15 in order a
     count byte followed by that direction's segments (3 bytes each for directions 0, 7, 8 and 9, 4 bytes each for the others), and after direction 15 one more zero byte. The index of that last byte is
     B = the sum over d of (1 + stride(d) × segments(d)) (kept in RAM $B8). The buffer is only 56 bytes long: **when B is $38 or more the lists run past $05CD into the RAM that follows**,
     $05CE (the F gauge's display copy, `economy.md` 3.1), then $05CF-$05DE (the table-A tile ranges of `objects.md` 5.2) and on up to $0596 + B, and the program writes them there on every step with
     the terrain test (zeros first, then the segments). Seen on level 11 (`level11.replay` from step 7,129, pod at (1,167, 5,951) in the middle of terrain pieces: 4, 3, 6 and 5 segments of directions 0, 7, 8 and 9, B = $46, so $05CE-$05DC overwritten).
     The effects (the F gauge stuck at a low value, the objects of table A drawn with a wrong tile base) and what the remake does (decided by Alex, 2026-10-20: it does not reproduce them, the buffer never clobbers other data): `behaviour.md` Q45. Eight other replays (levels 1, 2, 3, 5, 8, 12, 15 and `terrain.replay`, B watched on every write) never went above $2C, so the spill needs an unusually crowded window; it was not found outside `level11.replay`.
   - **Beyond the map** (edge cases that normal play never reaches, because maps are closed by terrain):
     - A block column right of the map's last chunk column is read from the next chunk row's first chunks (the
       map is read as one row-major list).
     - A chunk row whose top y is at or beyond `256 × ((byte 9 >> 1) + 1)` is not read from the map. For each
       of its blocks the window uses the chunk at **map** column 0 of the window's first (upper) chunk row. With
       odd byte 9 that threshold is the map's bottom edge, so every row below the map is replaced this way. With
       even byte 9 the one extra chunk row (4.2) still comes from the bytes after the map, and rows below it are
       replaced.
     - If the player's x high byte equals header byte 8 itself (the width in chunks − 1, compared as if it were a
       page number) and its x low byte is 240 or more, the right column is empty. This was re-checked: the
       comparison really is with byte 8. It can only happen off the map (on levels 13 and 14 at x ≥ 496, on
       wider maps further out), so it never matters in play.
2. **Size and reference point.** Two variables: width `ow` (RAM $F1) and half-height `h` (RAM $F2).
   P = (Px, Py) = (x AND 15) + 16 + ow / 2, (y AND 15) + 20 in window coordinates, i.e. the position + (ow / 2, 4)
   (`/` rounds down). With the usual ow = 6 that is the position + (3, 4).
   - **Values.** Both are 6 at level start and whenever a new pod takes over (back to state 1), and when the
     player switches the shield off. If gameplay starts with the player in state 2, 3 or 4, h = 9.
   - "Touched nothing" below means the surface flags RAM $DD are 0 after the test (step 6), so a step whose only
     hit was a flat ceiling counts as touching nothing.
   - **Pod exploding** (state 4): after each test that touched nothing, h grows by 1, up to 9. Jetman (states 2
     and 3) keeps the 9 reached by the explosion.
   - **Shield** (state 1, RAM $65 bit 7 set): after each test that touched nothing, while the player's y is 32 or
     more, ow grows by 1 up to 12. h grows by 1 up to 12 too, but only while the window's collected lists hold no
     direction-0 and no direction-9 segment at all (hit or not).
   - **Horizontal extent.** The player is not a box horizontally: floors and ceilings are tested at the single
     column Px, walls at the columns Px + 1 (direction 7) and Px + ow / 2 (direction 8; Px + 3 with ow = 6).
   - All tests in one step use P as it was at the start of the step; position corrections do not move P for the
     later tests.
3. **Floors.** Bottom B = Py + h. Band top T = B − (whole vertical speed, at the start of the stage) − 3. B and T
   are computed once and used by the whole stage, also after a flat-floor hit has moved the player.
   - **Flat floors (direction 0)**, first match in collection order: hit if x0 ≤ Px ≤ x0 + w and T ≤ y < B. The
     player is moved so that its bottom is y (B becomes y). Then:
     - **Soft landing:** state 2 or 3 and whole vertical speed below 2. Nothing else happens yet. If the sloped
       floors (next item) also hit, only the slope's correction and response follow. If no slope hits, the
       vertical speed becomes 0 (the horizontal speed is kept: Jetman keeps walking, `player.md` 6.2), the state
       becomes 3 (Jetman standing) and the vertical direction becomes down.
     - **Bounce**, otherwise: the vertical direction is reversed, the impact effect runs (below), the impact damage
       is taken from the vertical speed, and the vertical speed is halved.
   - **Sloped floors**, tested whether or not a flat floor hit: directions 1, 2, 3, 4, 5, 6 in that order, each in
     collection order; the first hit ends the floor stage.
     - The test uses T' = T + 1, and T + 2 when the y fraction (RAM $023E) is **not smaller than** the fraction of
       the vertical speed (RAM $02D9) (an earlier version had the comparison the other way round). Both bytes are
       taken at that moment: the y fraction after this step's position update, the speed fraction after any
       flat-floor halving of this step. The comparison does not look at the vertical direction. [V]: about 30 tests in
       Mesen (moving up and moving down; the "smaller" case seen once, moving up) matched
       T' = B − speed − 3 + 1 + (y fraction ≥ speed fraction). The objects implementer's black-box fit said the bonus
       applies only when moving up; the program has no such condition, so that fit probably absorbed a different
       fraction convention: re-fit with this exact byte comparison.
     - Hit if: the **stored** yLow byte (first byte, plus the block offset, not the computed line end) is ≥ T';
       x0 ≤ Px ≤ x0 + w; and the line height y(Px) from 6.2 is ≤ B. yHigh is used only as the line's start for
       directions 1, 3 and 5.
     - The player is moved so that its bottom is y(Px).
     - Response in state 3: the whole part of the horizontal speed is set to 1 (directions 1, 2) or 2 (3 to 6);
       its fraction is kept. The horizontal direction becomes down-slope (left for the rising slopes 2, 4, 6, right
       for 1, 3, 5). The vertical direction becomes down; the vertical speed is not changed.
     - Response otherwise: the slope response with the segment's contact angle (below).
4. **Walls**, using the original P: vertical span top Py − h to bottom Py + h (Py + h − 1 in state 3). A segment
   (x, y0, len) overlaps when y0 < bottom and y0 + len ≥ top. Direction 7 is tested first, then direction 8, each
   in collection order; the first hit ends the wall stage, so the two never both hit in one step.
   - **Direction 7** (face pointing right): hit if x < Px + 1 ≤ x + 8. Px + 1 is on the open side of the face (the
     test is not the mirror image of direction 8). The player moves 1 px right and its horizontal direction becomes
     right.
   - **Direction 8** (face pointing left): hit if x < Px + ow / 2 ≤ x + 8, i.e. inside the solid. The player is
     placed so that Px + ow / 2 = x exactly, and its horizontal direction becomes left.
   - Both directions: the impact effect runs, the impact damage is taken from the horizontal speed, then the
     horizontal speed is halved. Walls have no contact angle.
5. **Ceilings**, using the original P: top T = Py − h; band bottom U = T + (whole vertical speed, after the floor
   stage) + 6.
   - **Flat ceilings (direction 9)**, first match: hit if x0 ≤ Px ≤ x0 + w and T ≤ y ≤ U. The player is placed so
     that its top is y + 1. The impact effect runs, the impact damage is taken from the vertical speed, the vertical
     speed is halved and the vertical direction becomes down. The ceiling stage ends.
   - **Sloped ceilings**: directions 10 to 15 in order, each in collection order; the first hit wins. Hit if the
     stored yTop byte ≤ U, x0 ≤ Px ≤ x0 + w, and the line height y(Px) ≥ T. There is no upper check against U for
     the line itself. The player is pushed 1 px sideways (right for 10, 12, 14, left for 11, 13, 15) and placed so
     that its top is y(Px) + 1, then the slope response with the contact angle follows.
6. **Surface flags.** RAM $DD is cleared at the start of each test. Afterwards it shows what was touched: bit 0 flat
   floor; bit 1 direction 1; bit 2 direction 2; bit 3 directions 3 or 5; bit 4 directions 4 or 6; bit 5 a wall or a
   sloped ceiling. A flat ceiling sets no bit.

**Responses** (summary for `player.md`, which owns them):

- **Impact effect:** spawns debris objects and a sound (`player.md` 5.5).
- **Impact damage** (`economy.md` 3.3 and `player.md` 5.3 own the rules; $99 is the hit-pause counter of
  `player.md` 5.4): taken from the fuel, RAM $C7:$C6 (16-bit, the F
  gauge), only if it is non-zero, the level is not a warp level and the hit-pause counter RAM $99 is 0. The amount
  is the 8.8 speed × 4 (flat floors and ceilings: the vertical speed; walls: the horizontal speed).
  - Without the shield: the counter $99 is set to 48, and the fuel drops by 4 × the amount.
  - With the shield: the fuel drops by the amount / 2 (/ 4 when RAM $D3 is 8).
  - The fuel stops at 0.
- **Slope response** (sloped floors not in state 3, and sloped ceilings), with contact angle `a` and the player's
  movement direction `s` (RAM $9C, 1/256 turns):
  - `s` is reflected about the surface normal, the turn limited to ±$38. Exactly (all arithmetic mod 256,
    `signed()` reads a byte as −128..127, `clamp()` limits to −$38..+$38):
    - if a < $80 ≤ s: s' = a + clamp(signed(a − s − $81));
    - if $80 ≤ s ≤ a: s' = a + clamp(signed(a − s − $80));
    - otherwise: s' = a − clamp(signed(s + $80 − a)).

    Example: a = $20 (45° floor), s = $90: s' = $20 + clamp(signed($20 − $90 − $81)) = $20 + clamp(+$0F) = $2F.
  - The impact damage is taken with the amount (horizontal + vertical speed) × 2.
  - The speeds are then recomputed from the new direction (`player.md` 5.2).
  - Both speeds are halved unless both whole parts are 0 and the sum of the fractions is at most $80.
- **Flat surfaces** (directions 0 and 9) have no contact angle: their response is the vertical bounce or the
  landing above.

The player states are named in `game-flow.md` 9.1. The half-height of 9 is reached as described in step 2. In
practice it belongs to Jetman and the exploding pod, not to a state number as such. The value 2 seen in slot 0
during the launch belongs to the launch scene, not to gameplay.

### 6.5 Special terrain, gravity and the top of the map

- **Gravity value** for the pod (state 1): header byte 11 with bits 0-2 cleared, signed (RAM $6F). Positive pulls
  down, negative pulls up (level 7, −24). Jetman flying (state 2) always uses 49, on every level. No other player
  state applies gravity through this rule; objects are for `objects.md`. RAM $0595, when non-zero, replaces the
  header value (a signed byte, the same unit). It is cleared when a level is loaded and **written by the per-level routines of
  6.6**, which change the gravity on levels 3, 5, 6 and 11 (an earlier version of this text said the program only ever
  clears it; that was wrong). [V]
- **Applying it**, once per logic step, at the place in the step given by `player.md` 4.2. With g the gravity value
  and a = |g| / 4 (rounded down) in 1/256 px per logic step:
  - **Pull direction.** Down for g ≥ 0, so also for g = 0 (the second levels and warp levels); up for g < 0.
  - **Moving in the pull direction** (or not moving, with the direction bit pointing that way): the speed grows by
    a. If its whole part reaches 3 or more, it becomes exactly 3.00 (fraction cleared). This cap holds for either
    sign of g (upward on level 7), and also for g = 0, where it caps a downward speed at 3.00.
  - **Moving against the pull:** the speed shrinks by a. If the result is below 0, the leftover becomes a speed in
    the pull direction (the direction bit flips). If it is exactly 0, the direction bit also flips.
  - Example: speed 0.05 up (fraction $05, 5/256 px), g = 24 (a = $06): the result 0.05 − 0.06 is below 0, so the
    speed becomes 0.01 down.
  - The per-axis speed caps that follow (pod 4.00, Jetman 3.00) are in `player.md` 4.2 and 6.1. [C]
- **Top of a planet.** On planets 0-12, while the pod's y (the position, not Py) is below 64, the pod's gravity
  value is replaced by `(65 − y) × 2`, in the same unit (4 at y = 63, 130 at y = 0, always down). This replacement
  comes after the header value is chosen, so it wins. It belongs to the pod's step only: Jetman's fixed 49 is never
  replaced. [C]
- **Ceiling at y = 16.** On every level, the player (slot 0, any state that moves through the common position
  update, `player.md` 4.9) can never move above y = 16: moving up past it puts it at y = 16 with vertical speed 0,
  and the y fraction is kept. Other objects are not clamped there. [C]
- **Sky chunk.** The program checks whether the window's **bottom-right** block (two blocks right of and below the
  top-left block, 6.4 step 1) lies in chunk 0 (open sky), RAM $D4; the parallax stars use it (`graphics.md`
  5.7.3). Corrected: the chunk is the one fetched last in the 3 x 3 walk, not the top-left block's. [C], fits the
  replays.
- No fuel pads, hazards or gravity zones are encoded in the map, block or chunk data; fuel, items, pods and enemies
  are objects (3.3, `objects.md`).

### 6.6 Per-level routines (and the gravity changes they make)

Once per logic step the program calls a routine chosen
by the level index from a table (bank 0 $A81A low / $A844 high bytes, 42 entries). **Where in the step [V] (2026-10-20, Mesen, levels 3 and 11, the return address on the stack at
every palette write): in the A pass, in the periodic-events block (`game-flow.md` 1.2, item 9), i.e. after the status-bar split has incremented $5D
(so $5D is odd while the routine runs), after the pause test and before the collision tests; it is not called in the B pass on these levels (RAM $9B bit 7
clear). Rare tow branches (an object being delivered) were not checked and may skip it [C].** Most are empty. The ones that do
something **[C]** (the gravity ones **[V]** in Mesen, pod held at chosen heights, RAM $0595 read):

| Level | Routine |
|---|---|
| 0, 3 | When the frame counter's high byte (RAM $5E) is 5 or more (1,280 frames), the egg's list entry (type 5F: entry 0 on level 0, entry 70 on level 3) is set to 0, so an egg that has not been met yet never spawns. Level 3 also has the gravity change below. |
| 1 | When RAM $5E is 2 or more (512 frames), the special ring of item kind 26 (list entry 1) is removed (`economy.md` 2.1). |
| 2 | The junction trigger to level 27 (`economy.md` 5.2). |
| 5 | The junction trigger to level 28 (`economy.md` 5.2), and a gravity change below. |
| 6, 11 | A gravity change below. |
| 26 | The final level's end check and background effect (`behaviour.md` 4.16). |
| 29-41 | The warp-level routine (6.7): the countdown's slot, the mesh helper. It runs on **every video frame** (both passes) **[V]** (a hook counted one call per frame), unlike the others. |
| 4, 7-10, 12-25, 27, 28 | Nothing. |

**Gravity changes** (RAM $0595, set every step from the pod's position y, the pod's own position, not its screen row; 0
means "use the header value"). Applied by the rule of 6.5, so a = |g| / 4:

| Level | Header g | Pod y (pixels) | $0595 = g used |
|---|---|---|---|
| 3 | 8 | below 1,280 (y high byte 0-4) | 0: header, 8 (down) |
| 3 | 8 | 1,280 and more | $F8: −8, **up**, a = 2: the pull reverses below the line y = $500 [V] |
| 5 | 40 | x in 2,876-2,883 and y in 3,328-3,499 (x high byte 11, y high byte 13, y low byte below $AC), except y in 3,436-3,451 (the junction trigger fires there instead) | $7F: 127 down, a = 31 (a strong pull in a shaft) [V] |
| 5 | 40 | anywhere else | 0: header |
| 6 | 32 | below 2,560 | 0: header |
| 6 | 32 | 2,560 and more | $FF: −1, a = 0: no pull at all (and a vertical carry-in of 255 in the thrust, `player.md` 4.6) [V] for the value |
| 11 | 72 | below 2,048 | 0: header |
| 11 | 72 | 2,048-3,583 | $E8: −24, up, a = 6 [V] |
| 11 | 72 | 3,584 and more | $20: 32, down, a = 8 [V] |

The top-of-planet replacement of 6.5 (y below 64) is applied after these and still wins there. The routine runs in the A pass
after the pod's update of the same step, so a change takes effect with the next step's gravity.

**Terrain colours of levels 3 and 11 [V] (2026-10-20).** Only table entries 3 and 11 (the planets 3 and 11 themselves; the paired levels 16 and 24 use the
empty routine at bank 0:$A941, and no other level touches these entries during play) also rewrite three palette-shadow entries,
RAM $7F, $80, $81 (shadow entries 9-11 = background palette 2, colours 1-3, `graphics.md` 7.1; every chunk of both levels uses background
palette 2, 4.5). The same code ends both routines, after the gravity part of the routine above.

- **When.** In each A pass, at the place given above, if ($5D AND $1E) = 0 with $5D as it stands after the split's increment, i.e. $5D mod 32 = 1
  (the A pass began with $5D mod 32 = 0): every 16th step, counted from the level's first step (steps 1, 17, 33, ...; $5D is 0 at the level's
  first pass and counts frames, `graphics.md` 7.5), and only when the player state (RAM $0317) is not 9 (being drawn through a gate; all other states,
  including the destroyed pod, state 7, still write). It is keyed to the frame counter, not to a counter of its own. 178 of 178 consecutive firings on `level3.replay`
  were exactly 16 steps apart; none in a B pass.
- **Which row.** The camera's y high byte, RAM $4F (the camera as the A pass left it: its camera move, item 5, comes before this routine), selects a row
  of the 4-row, 12-byte table at bank 0:$A9B7 (ROM 0x029B7). Level 3: row 1 if the high byte is 3 or more (camera y of 768 or more), else row 0
  (so 0, 1 and 2 give row 0; checked at high bytes 0, 1, 2, 3, 4). Level 11: row 3 if the high byte is 9 or more (camera y of 2,304 or more), else row 2 (checked at
  0-8 and 9, 10, 11, 12, 15, 19, 20, 22, 23).
- **The table.** Entries 9, 10, 11 (RAM $7F, $80, $81):

  | Row | Level | Camera y high byte | $7F | $80 | $81 |
  |---|---|---|---|---|---|
  | 0 | 3 | 0-2 | $28 | $16 | $05 |
  | 1 | 3 | 3 or more | $0F | $32 | $12 |
  | 2 | 11 | 0-8 | $38 | $29 | $19 |
  | 3 | 11 | 9 or more | $16 | $33 | $13 |

  Row 0 and row 2 are the colours the header gives the level at its start (header bytes 12-14), so on going back up the routine simply rewrites the
  same values: there is no separate "restore" and no hysteresis (checked on level 3: high bytes 4, 1, 3, 2, 4, 3, 1 in turn gave rows 1, 0, 1, 0, 1, 1, 0 at the next firing each time). Row 0
  is yellow, red, dark red; row 1 black, pale blue, blue (the black-and-blue lower terrain of level 3). Nothing else is written: no other shadow entry,
  no sprite palette, no universal background.
- **When it reaches the screen.** Palette RAM takes the shadow only at the gameplay handler's video job 0 (`graphics.md` 7.1), the NMI whose $5D at its start has
  $5D mod 8 = 0. The write happens in the A pass of step n (n = 1 mod 16, that pass starts with $5D mod 32 = 0), so palette RAM changes 8 frames later, at the start of the
  A pass of step n + 4, and the harness's picture `step-N` (`tools/mesen/replay-capture.lua`) shows the new palette from N = n + 3 (palette RAM of step 17's write: changed in
  the frame of step 21's A pass, first seen in `step-20`; step 81's write back to row 0: first seen in `step-84`).
- **Level start, respawn.** A level load puts the header's colours in the shadow (row 0 / row 2) and job 8 copies them on its first frame. The routine also runs with
  $5D = 0 in the load's first frame and on the first A pass ($5D = 1 after the split), so after a respawn deep in the level (the pod destroyed at camera y high
  byte 4, Jetman lost, level restarted there) palette RAM held the header colours for the one load frame and the row 1 colours from the next frame on: effectively at once. [V]
  A planet start at the level's home position (camera y high byte 0) shows row 0 throughout. Only a test poke of the position (replay "warp" setups) leaves the old colours for up to
  16 steps (until the next firing) plus the 3 steps of upload delay.
- **Level 11** [V] the same on `level11.replay` (all 705 firings had the rows above for their camera high byte; none at an even $5D; the level load's own copy of the header colours is $38 $29 $19, row 2).

### 6.7 Warp levels 29-41 **[V]**

(Mesen 2026-10-17: scripted and random flights on levels 29, 33, 34 and 37, 7 runs of 1,380 frames; the map and the shape
classes decoded from the ROM.)

- **One map, one block set, ordinary collision.** All thirteen levels share the map at bank 3:$B9DE (6 × 6 chunks = 768 × 768 px, chunk
  rows 0-5: `245 172 172 172 244 143` / `245 245 143 244 244 143` / `245 143 144 143 244 143` / `245 143 143 143 244 143` /
  `245 143 172 143 244 143` / `245 172 172 172 244 143`, extra row `0 0 0 0 2 3`). The graphics type is 0, so the block
  set is set 1 and the shapes come from its class map (6.2). The solid blocks are
  classes 1 (flat floor), 19 (flat ceiling), 10 and 28 (left and right walls) and the corner classes 38-41; every
  other block of the map is sky for the test.
- **Layout (world pixels).** A closed arena. Its limits are the faces of four walls: left x = 128 (the right edge of the wall
  blocks in block column 7), right x = 512 (the left edge of block column 32), top y = 64 (the underside of the bar in block row
  3) and bottom y = 640 (the top of the bar in block row 40). Inside it stand four solid obstacles: two pillars
  x 192-256 and x 384-448, both y 128-256 (blocks 12-15 and 24-27 at block rows 8-15), and two slabs x 256-384 at
  y 320-384 and y 512-576 (blocks 16-23 at rows 20-23 and 32-35). The rest of the map (an outer frame at x = 64 and 576, side corridors, a
  second bar pair at rows 41-43) lies outside the arena and cannot be reached. The pod starts at (320, 253) and, in seven
  runs, was found at x 132-506 and y 67-630 and never left. The solid blocks come from the map data by the ordinary
  decode (4.3, 6.2), so the remake needs the data, not this paragraph.
- **Bounds.** Nothing else bounds the player: beyond the arena there is empty map (blocks without segments) and, outside the
  768 × 768 map, the usual unread bytes (4.2). Since the arena is closed, nothing can reach them.
- **Camera.** 5.2 as for any level: camera limits x 512 and y 568 (byte 8 = 5, byte 9 = 5), band rules unchanged; in the
  arena the camera was never at a limit (camera x 0-363, y 3-535: the y of 3 comes from the band test, the pod
  cannot go above y = 67).
- **Gravity.** The header gravity byte is 0 (byte 11 = $00 on every warp level), so the pull is nil as on the second levels (6.5):
  a pod left alone moved 5 px in the whole countdown (arrival at level 29), and with thrust it keeps its speed until it
  bounces. Collisions cost nothing (`economy.md` 5.4).
- **Start.** Pod at (320, 253) (home (320, 256) − 3), facing 0, speeds 0 with the vertical fraction $40, camera (192, 160) as
  placed by 5.2.
- **The routine of 6.6** runs on every frame (not only in the B pass): it forces slot 3 into state $0D (the
  countdown, `behaviour.md` 6.3) and places or moves the two mesh helpers of slots 23 and 24 (`objects.md` 10.6,
  `graphics.md` 5.7.2).

## 7. How this was verified

- Mesen, headless: for each of the 42 levels (RAM $3B poked on a new game) the full-screen draw at the start of play
  was redirected to three camera positions (map origin, middle, far corner); 40 frames later the 2 KB nametable RAM,
  CHR and palette were dumped and compared with our decode of map, chunks, blocks and palette map (5.3 for the
  numbers). A render of the decoded map with the dumped CHR and palette matched the screenshot pixel for pixel in the
  visible playfield (from x = 8, lines 0-192).
- The approach screen of level 7 showed "GRAVITY -24  DIAMETER 29", matching header bytes 11 and 8.
- The collision data was decoded completely; the collision behaviour (6.4, 6.5) and the camera rules (5.2) are read
  from the program and not yet compared with play. Exceptions, checked in Mesen: the vertical camera band, the
  camera moving every frame while the player moves every second frame, the half-step pattern of 5.2 (one run), and
  the playfield height (line 192 shows map pixels on all checked screens, line 193 does not).
- The item and placed-object counts, the item-kind table of 3.3 and the reflection formula of 6.4 (checked
  against the decoded program for all 65 536 angle pairs) are decodes, not observations.

## Open questions

- Whether level 26's items list (3.3) is intended.
- Level 26: the end routine's jitter at n = 1 is still not measured. (The final sequence's trigger is settled: played
  through naturally, 2026-10-17, `behaviour.md` 4.16 "Level end, played naturally".)
- Bank 2:$F3A5-$F464 (192 bytes before the collision code) is data of unknown use.

## Appendix A. Starting any level in Mesen (black-box method)

Poking RAM $3B alone works only for the planets. Every other level must be entered through the transition the game
itself uses, so that its paired planet, entry position, graphics, palettes and item list are set up as in play.
All steps below were run with `tools/mesen/run.sh`.

The base sequence (the same as the planet case, which is also the first half of every other case):

1. Power on.
2. Press Start at frame 1300 (title: START GAME).
3. Write the planet index P (0-12) to RAM $3B at frame 1301 (START LEVEL then sets the system from it).
4. Press Start at frame 1500. This skips the interplanetary flight (P > 0), then the approach screen ("PRESS START
   TO LAND"); it does not hurt for P = 0.
5. The surface picture, the mothership's descent and the launch run by themselves. Gameplay begins when the first
   full-screen map draw happens (about frames 2330-2700, depending on the level).

The other levels are reached from there by writing a transition code to RAM $3F during gameplay
(`game-flow.md` 7.4). Do it some frames after the first map draw, while the pod is still in flight; about 150
frames was used. Without input the pod falls back and docks after about 600 frames, and then the request does
nothing useful.

| Target | Method | Entry position | Status |
|---|---|---|---|
| Planet P (0-12) | base sequence with $3B = P | header bytes 4-7 (pod 3 px higher) | [V] all 13 |
| Second level L (13-25) | base sequence with P = L − 13, then write 3 to $3F | header home, bytes 4-7 of level L (y − 3); the camera is placed from the pod (5.2), not taken from bytes 0-3 (`game-flow.md` 7.4, "Code 3") | [V] 13, 15, 18, 20, 25 |
| Warp level L (29-41) | as for second level L − 16; once there, set this planet's flag (RAM $DE bit 7 − P for P = 0-7, RAM $DF bit 7 − (P − 8) for P = 8-11, P = 12 uses P = 10's bit; `game-flow.md` 6), write 0 to RAM $41 (the transition guard, since code 3 repeats), then write 3 to $3F | header of level L | [V] 29 (from 13) |
| Junction 27 or 28 | any planet in flight, write 27 or 28 to RAM $DB, then 5 to $3F | header of the level | [V] both, from planet 0 |
| Final level 26 | as for 27, with $DB = 26, and also write $80 to RAM $9A in the same frame | header of level 26 | map and graphics [V]. The forced scrolling did not start and lives and jetpods were not zeroed, unlike the real entry (`game-flow.md` 7.4, "Final level"); use it to check the map only |

Notes:

- A transition re-reads the new level's header, graphics type, palettes, map and items (3.2) through the same level
  set-up as a planet start, so screens are correct. Poking $3B straight to a second level instead runs the planet
  path (approach screen, surface picture, mothership) with a level that has none, which gives the broken screens.
- The planet flag for warp levels is tested on RAM $3C, which the code-3 transition sets to the level being left,
  so the warp level must be entered from its second level, not from the planet.
- On entering a level by a transition the camera is placed from the player position, then rounded down to 32 px for
  the full draw (5.2). It is not exactly the header camera. Use the camera RAM after the draw when comparing.
- **Poked levels 13-28 can carry the wrong objects** (2026-10-17). Writing the level number to $3B (the planet path)
  for a level above 12 leaves the object table of another level in place: checked by comparing the first spawned
  objects with the level's own list, levels 13-15 and 18-25 carry the paired planet's objects (for example level 15 shows
  planet 2's generator at (121, 401)), while levels 16, 17, 26 and 28 carry their own. For object behaviour on a
  second level use the "Second level" row (base sequence, then $3F = 3), which reloads the level's own list. [V] level 15:
  its own guns, flames and creatures appeared.
- **Level 26 with its route** [V] (2026-10-17). From a state in which level 26 is loaded (even the poked planet-path
  one), write $9A = $80 and the player state (RAM $0317) = 6 and set the camera RAM $4C-$4F to (0, 928) on the same
  step, and keep the pod's fuel up: the camera then follows the route of 5.2 by itself, 2 px per logic step on a
  straight stretch (1 px per frame per axis), with the pod carried along, from (0, 928) to the end of the route at
  camera x 6,656 after about 4,050 steps; the level's objects spawn as the camera passes them. This is how
  `behaviour.md` 4.16 was run. (The row above, with its single $9A write, did not start the scroll; the player state
  was the missing part.)
- To compare a given camera position, the full-screen draw can be redirected: when bank 3's full-draw entry
  starts (RAM $75 AND 7 = 3), write the wanted camera to RAM $4C-$4F. This is how section 7 checked every level.
- **Redrawing the whole background after a warp [V]** (2026-10-20). A test setup that writes the pod's position and the camera RAM
  $4C-$4F in the middle of a run (`tools/mesen/replay-capture.lua`, "warp X Y CX CY") leaves the name tables as they were: the game
  only draws the one tile row and column that enter view when the camera moves (5.3), so the background stays stale for the rest of the
  run. The fix is to make the game run its level-start full-screen draw (bank 3 entry 0, through the far-call helper at $FFAC with A = 3,
  Y = 0) at the new camera, **at the idle loop** (bank 0 $9570, where the capture already pokes from: after a B pass, before the next A
  pass). It cannot be done from inside the NMI, so not for a warp that is requested before step 1 (level start has just drawn its own). The
  playfield is the single name table page 0 (`graphics.md` 1.1); the status panel is page 1 and is not touched.

  Recipe, as the last part of the poke for that step, after every other poke (position, speeds, camera, fuel, objects):
  1. Save RAM $00-$FF (the draw uses scratch bytes in it) and the CPU's stack pointer and PC (the idle loop's).
  2. Let (CX, CY) be the camera wanted, o = CX AND 31, **extra = ((o + 255) >> 3) − 31** (0 when o = 0, else 1-4) and **k = (CY AND 31) >> 3** (0-3), so the camera's own
     tile row is ty = CY >> 3 = ty0 + k.
     The draw rounds the camera down to a multiple of 32 px (RAM $4C and $4E AND $E0, 5.2), fills 32 tile columns from there and **only the first 29 of the 30 rows**
     (ring rows 0-28 = map tile rows ty0 to ty0 + 28, ty0 = rounded CY / 8; ring row 29 is left as it was). What is missing: `extra` tile columns on the
     right of the picture when o > 0 (side draw, step 3), and **the map rows ty0 + 29 to ty0 + 29 + k, that is k + 1 rows, below it, for every CY** (steps 3-4).
     **Why k + 1 rows (amended 2026-10-20; the first version of this recipe patched k rows and was wrong by one).** The game's ring invariant is "map rows ty to ty + 29
     are in the ring", kept by drawing, on a vertical move, the one row at pixel CY + 232 (map row (CY + 232) >> 3 = ty + 29 for the camera *after* the move) and, on an upward move,
     the row at CY. A camera put there by a warp has rows ty to ty + 28 (the full draw's ring rows k to 28 and the k rows the recipe's first version patched), and **row ty + 29 is drawn only if the first downward move
     stays inside tile row ty**; if it crosses into ty + 1 (CY AND 7 + the move ≥ 8: always when CY AND 7 = 7, as at level 12's camera (295, 1415), whose first move was 1 px) the game draws
     ty + 30 instead and row ty + 29 stays stale for good, until it scrolls off. It comes into view (the playfield is 200 lines, so its bottom row is (CY + 199) >> 3) once the camera
     has moved down about 33 − (CY AND 7) px, and then shows as one row of wrong tiles in the bottom lines, rising as the camera goes on (seen: level 12 camera (295, 1415), k = 0, from step 374; level 10 camera (4640, 7), k = 0,
     from step 4550, one row at lines 125-130 when the camera stopped at y 107; level 9 camera (1018, 1586), k = 2, from step 4564 with row ty + 29 = 227; level 10 camera (1856, 757), k = 2: no stale row from this cause). The old text said that with CY a multiple of 32 nothing is missing
     because the first move draws the row; that holds only for a first move of at most 7 px (8 px, the camera's largest step per pass, crosses at once). **Patch the k + 1 rows always** (when the
     first move would have drawn the row, the patch is the same tiles).
  3. The **extra draws**, each the draw call (below) with camera RAM = (X, Y) as listed; after each has returned keep the bytes named. All reads are from **raw name table
     RAM, page 0** (see the draw call), "slot" = column 0-31, byte row × 32 + slot. Let CXr = CX AND NOT 31 and CYr = CY AND NOT 31.
     - **B** (extra > 0): camera (CXr + 32, CYr). Keep the tile bytes of slots 28 to 27 + extra in each of the 30 rows, and the 8 attribute bytes of attribute column 7 (bytes $3C0 + row × 8 + 7).
     - **D** (extra > 0): camera (CXr + 32, CYr + 32). Its ring row j holds map row ty0 + 4 + j, so its rows 25 to 25 + k are the map rows ty0 + 29 to ty0 + 29 + k. Keep the tile
       bytes of slots 28 to 27 + extra in those k + 1 rows, and the byte at $3C0 + 6 × 8 + 7.
     - **C** (always): camera (CXr, CYr + 32). Keep the 32 tile bytes of each of its rows 25 to 25 + k, and the 8 bytes $3C0 + 6 × 8 + c (c = 0-7).
  4. The **main draw**, last: the draw call with camera RAM = (CXr, CYr). When it has returned, patch name table page 0 (raw RAM):
     - Rows below: write C's row 25 + i into ring row (29 + i) mod 30 (all 32 slots), for i = 0 to **k** (k + 1 rows; ring rows 29, 0, 1, 2 hold map rows ty0 + 29 to ty0 + 32; ring rows 0 to k − 1
       are above the picture and no longer needed there). If k ≥ 2, ring rows 0-1 are now other map rows than the attribute bytes of row 0 describe: for every attribute column c set the **upper**
       quadrants (bits 0-3) of byte $3C0 + c to the **lower** quadrants (bits 4-7) of C's kept byte c. (Ring row 29 shares its attribute quadrant with row 28: nothing to do. For k = 1 and k = 3 the last patched ring
       row, 0 or 2, shares its attribute quadrant pair with the picture's top row, which keeps the quadrants: the picture wins, as in the game's own steady state; no attribute change.)
     - Side columns (extra > 0): write B's kept tiles into slots 0 to extra − 1 of the same rows, **except** ring rows (29 + i) mod 30 for i ≤ k, which take D's kept tiles of row i instead; and copy
       the kept attribute bits of column 7 into attribute column 0 (byte $3C0 + row × 8) for the quadrants those slots use: slots 0-1 are bits 0-1 and
       4-5 of the byte, slots 2-3 bits 2-3 and 6-7 (copy the quadrants of slots 0 to extra − 1, rounded up to pairs). For attribute row 0 and k ≥ 2 take the upper quadrants (bits 0-3) from
       D's kept byte >> 4 instead of B's.
  5. Finish: write back the zero page saved in 1; write the PPU mask and control registers (the draw leaves rendering and the NMI off) from the restored RAM
     $01 and $00; set camera RAM $4C-$4F to (CX, CY) exactly; **scroll RAM $02 = CX AND 31 and $03 = CY AND 31**; $EE = 0; and
     **$49, $50, $51, $52, $53, $54 = 0**.

  The **draw call** is the far call the game's own loader makes, started by the script from the idle loop: write RAM $73 = $80 (the helper's jump-table page);
  RAM $75 AND $EF (the helper keeps bits 3-4 of $75, and bit 4 selects the nametable page: the idle loop has the panel's page 1 selected, and the draw
  would overwrite it); RAM $00 AND $7F (NMI off while the routine runs; it writes the PPU control register from $00 at its end); push on the stack, in this order,
  the idle loop's address − 1, the original $75, and $FFBF; set the CPU to PC = $FFAC, A = 3, Y = 0. The helper's tail then runs twice, the second time restoring
  $75 and the mapper's page bit from the byte pushed, and returns to the idle loop. The call has ended when the CPU is at the idle loop again with the stack pointer saved in 1.
  - **Raw name table RAM [V].** The kept tiles, the patches and the attributes above are read and written in the name table RAM itself (Mesen: `emu.memType.nesNametableRam`, page 0 = offsets
    $000-$3FF, attributes $3C0-$3FF), **not** through the PPU's address $2000 (`nesPpuMemory`): $2000 shows whichever page the mapper has selected, and once the far call has returned it is the status panel's
    page 1 again (the idle loop's), so a read or write through $2000 reads or clobbers the panel (seen: its F and W letters were overwritten). During a draw call the helper has page 0 selected; the
    script's reads after the return must not rely on that.
  - **Camera rounding.** It applies to the draw itself and the recipe undoes it: the scroll bytes $02 and $03 carry the remainder (CX AND 31, CY AND 31;
    the draw's ring holds the rows from the rounded camera; of its 30 rows only 29 are drawn, which is why steps 2-4 add k rows when CY AND 31 ≥ 8), and the camera RAM is written exactly after the draw.
    The game's later incremental draws stay consistent because they work from $02, $03 and the camera. With CX a multiple of 32 no side draw is needed.
  - **What the draw touches.** Name table page 0 (the 30 rows, and its 64 attribute bytes). RAM, all restored by step 5 except where said: $00 (masked above), $04-$13 (scratch),
    $57, $72, $74, $75 (far call), **$5D** (the frame counter: the draw uses it as a loop counter and leaves 8; restored), $9F-$A6 and $E1-$E8 (the attribute column and row
    buffers), $EE (cleared), $4C/$4E (masked, then rewritten); and stack bytes below the stack pointer ($01F2-$01FE). Nothing else: RAM $0100-$07FF compared before and after
    shows only those stack bytes, so the object table, the sprite table, the random-number bytes, the item bytes and the status panel are untouched.
  - **$49 and $52-$54 must be cleared (step 5).** The pass before the warp has left pending uploads for the old camera. $49 = $C0 after every pass: bit 7 = a row of 8 attribute
    bytes (from $E1-$E8 to PPU $23xx, address low byte in $57), bit 6 = a column of attribute bytes ($9F-$A6); $53/$54 (and $52) = the pending tile row/column uploads (2 after a
    downward move). The next NMI would write them into the new picture (7 wrong attribute bytes at $23D9-$23DF were seen, for a few frames, until the rotating
    refresh rewrote them). RAM $42/$43 (the pod's previous position, used by the camera's follow rule, 5.2) are left alone, as the existing warp does.
  - **Cost.** The draw runs with NMI and rendering off and takes **about 5 frames per draw call, and 2 + 2 × (extra > 0) calls** (main draw A, rows below C, and with side columns B and D): 10 frames when CX is a multiple of 32, 19-20 otherwise,
    whatever k is (frames counted from the idle loop to the return: 19 for four calls in all 72 warps of the level 12 run, 2026-10-20; before the k + 1 amendment 5 / 10 / 19-20 with the calls omitted when k = 0 or extra = 0). In those frames the screen shows no picture (rendering is off: backdrop colour, black), and no NMI runs: **no frame-counter increment, no
    sprite-table upload (the sprites keep the last table), no sound-driver tick (music and effects pause for those frames) and no HUD job**. The step that follows
    starts at the next vertical blank as normal; **no step is lost or added and no pass is cut short**. Mesen's `takeScreenshot` lags 2 frames (5.3.1), so a shot within 2 frames of
    the return can still show the blank.
  - **Two more things the script does that the recipe's first version left out (implementer amendments in `tools/mesen/replay-capture.lua`, 2026-10-20; the analyst re-read them in the script).**
    (i) **The controller.** The full draw polls the controller during its frames. With nobody pressing anything in those frames the game's memory of the buttons held (and of the buttons newly pressed) would change
    (seen on level 13: a turn lost after a warp). The script therefore gives the draw's reads the buttons of the **last poll before the redraw** (`last_held`), so the game's button bytes stay as the pass before left them.
    (ii) **When to give the picture back.** After the last draw call has returned, the script waits in the idle loop until the PPU is on a picture line (scanline 0-239) and only then writes the PPU control register
    (NMI enable) back from the restored RAM: enabling the NMI inside a vertical blank starts a pass at once, a frame early (seen on level 13 and level 14: the frame counter one ahead and the game state
    diverging from the run without the redraw). Both are properties of the capture, not of the game.
  - **Rows the game's own drawer skips are not the recipe's (2026-10-20).** The game draws at most **one tile row per logic step, in the B pass only** (a row draw needs the frame counter odd, RAM $5D AND 1, except on levels with RAM $9A bit 6 set),
    for the camera as it is at that moment, however far the A pass and the B pass moved it (up to 8 px each). So a step that moves the camera by 9 px or more across two tile-row boundaries (A pass 7 + B pass 7 after a
    collision bump of the pod, 14 px; or the follow rule's catch-up after a warp, 8 + 6) **leaves one row undrawn**, and it stays wrong until it scrolls off (or is redrawn after the ring wraps). Seen after the k + 1 amendment, and no other wrong rows were left:
    level 10 step 2954 (camera 823 to 837; map row 132: wrong tiles from step 2973, lines 152-192, rising), level 9 step 4521 (1586 to 1600; row 228 = ty + 30: wrong from step 4570), 102 of the 108 steps with wrong tiles in the first 1,100 steps of level 12
    (the six others were taken during a warp's redraw). Traced in Mesen at level 10 step 2954 (a write of the camera's y byte in each pass, one row draw, in the second). **An original-side picture with such a row is not a reference for the map**: the
    remake draws the background from the map and is right; a comparison should mask rows whose camera move crossed two or more tile boundaries in one step (row (CY + 232) >> 3 for the boundaries crossed except the last, going down; the same for the top row going up).
    The script cannot repair these rows without calling the game's row routine for each skipped one; it does not.
  - **Which row, what it holds, and when it is seen (2026-10-20, `level10.replay`, Mesen [V]).** The rule, exactly: in one logic step the camera's y byte is written in the A pass (RAM $4E, then $4F) and again in the B pass, and the row draw (one row, B pass only, only when the frame counter is odd) draws **only the row for the camera as the B pass leaves it**: going down map row (CY + 232) >> 3, going up map row CY >> 3. With CY0 the camera at the start of the step and CY2 at its end, the map rows **(CY0 + 232) >> 3 + 1 to (CY2 + 232) >> 3 − 1** (down; rows CY2 >> 3 + 1 to CY0 >> 3 − 1 going up) are never drawn. That needs the step's move to cross two or more tile-row boundaries, so at least 9 px (8 px crosses one at most), in a **single step**: the camera's own 2-3 px steps never do it, nor a reversal of direction (the row is drawn for the direction of the end of the step; no busy video buffer, palette, status-panel, star or other job was involved: the row draw ran in every B pass that moved the camera across a boundary in steps 6125-6135, once each, and never left a row out except by the rule above). What the skipped row holds is **what its ring row held before: the map row 30 rows above** (the ring has 30 rows), unchanged, not garbage and not another job's data.
    The case in `level10.replay`: the warps "1391 1428 1243 1317" (before steps 6125, 6247 and 6347; CY AND 31 = 5, so k = 0). The recipe leaves the picture's camera at (1243, 1317) with map rows 164-193 in the ring. The pod is at (1391, 1428), so the first step (A pass, then B pass) moves the camera **1243, 1317 → 1244, 1323 → 1244, 1329**, 6 px in each pass (the follow rule's catch-up per pass; the 8 + 6 of the other catch-up above is the same mechanism with different numbers). Bottom row 193 → 194 (A pass, no draw) → 195 (B pass: the one draw, ring row 1). **Map row 194 (ring row 0) is skipped and holds map row 164's tiles** (checked in the name table: ring row 0 equals map row 164, ring rows 29 and 1 equal map rows 193 and 195, and no draw wrote anywhere in the A pass or in steps 6126-6130). It is not visible when skipped (at CY 1329 the playfield's last row is (1329 + 199) >> 3 = 191, row 194 is 3 rows below it): it enters the playfield when CY + 199 ≥ 1552, **CY = 1353-1360 (step 6159, line 191-192; the camera then moves down 2-3 px a step, the pod falling)**, stays at world y 1552-1559 (line 1552 − CY) and is gone from the top once CY ≥ 1560 (step 6229). The remake's picture is right, so a comparison must mask it. The same three warps repeat the strip from steps 6281-6284 and 6381-6384 (the same camera path), seen as a row of 29 wrong tiles at map row 194 in the name table at steps 6200, 6290 and 6390; every other warp of `level10.replay` (the 74 of them) has no first-step catch-up (camera exactly the warp's, checked in the log) and no strip. A one-row-wrong 171 appears in the dumps of 6290 and 6390 above the picture's top (CY 1380: tile row 171 = ty − 1): never visible.
  - **Mask rule for a comparison [V].** From the original's own log: for every step n with camera CY0 (after step n − 1) and CY2 (after step n), moving down, mask map rows (CY0 + 232) >> 3 + 1 to (CY2 + 232) >> 3 − 1 (none when the move is under 9 px) from the step when they enter the playfield (CY + 199 ≥ 8 × row) until they leave it (CY ≥ 8 × row + 8, i.e. the camera's top passes the row); going up, rows CY2 >> 3 + 1 to CY0 >> 3 − 1 from when the playfield's top reaches them (CY ≤ 8 × row + 7) until CY + 200 ≤ 8 × row. A warp counts as a step like any other (the move is from the warp's camera to the camera the first step leaves, not from the previous position). Rows that never reach the playfield need no mask (a catch-up after which the skipped row is hidden below the bottom for good, as when the pod never descends).
  - **Checked, the k + 1 rows (2026-10-20, Mesen, a copy of the script with the amended steps 2-4).** `level12.replay` steps 326-1100 (10 warps: k 0, 1, 3 and extra 0-4; 19 frames with side columns, 10 without), `level10.replay` steps 2900-4700 (warps at 2905, 3027, 3127, 3227,
    3349, 3449, 3549, 3871, 3993, 4093, 4193, 4315, 4415, 4515, 4637: k 0, 1, 2, extra 0, 1, 2, 4) and `level9.replay` steps 4400-4700 (k 2): after every warp no stale row appears for 45 steps except the skipped ones above (before: level 12 camera (295, 1415) had one stale row from step 374 in
    all 29 dumps up to 400; level 10 camera (4640, 7) one row from step 4550 to the end; level 9 two rows). The name table's tile rows were compared with the map as in the earlier checks, the attributes of the picture against the chunk palette map: identical to the old recipe's, including a transient of 6 to 2 cells in the first ~35 steps after
    the warp with k = 1, extra 2 (level 12 step 973) which the old recipe has as well (the game's rotating refresh clears it). k = 3 (extra 4): `level10.replay` steps 5156-5330 (warps 5159 and 5281), tiles and attributes clean in every dump outside the redraw.
  - **Checked** (Mesen, a copy of `replay-capture.lua` with the recipe, 2026-10-20). `level1.replay` (3,918 steps, 32 warps, five cameras, all multiples of 32), `level2.replay` (4,105 steps,
    40 warps, six cameras) and the first 1,302 steps of `level10.replay` (13 warps, four cameras, none a multiple of 32, extra 2 and 3): the step ("s") and sound ("e", "m") lines are **identical** to the
    run without the redraw (pod, objects, counters unchanged). After every warp all visible tiles of the picture (32 columns × 25 rows, minus the masked column) equal the map's decoded tiles (800/800
    or 775/775 in every dump, 1-8 frames after the return); name table page 0 equals the level start's own full draw at the same camera (level 1, camera 2,592/960: all 960 tiles and all 64 attribute bytes
    identical); the `extra` side columns equal the columns of the draw at camera + 32 px; the Mesen picture matches the level start's at the same camera. Attribute bytes of an unaligned camera can
    differ from a plain draw's by the bytes the game's own rotating refresh rewrites afterwards (its attribute rule, not the recipe's); the status panel (page 1) is unchanged.
  - **Checked, rows below and attributes (2026-10-20, Mesen, a copy of the script with steps 2-4 above).** `orb-held.replay` (camera (600, 200): extra 3, k 1; 20 frames), `level10.replay`
    in full (24 distinct cameras, k 0-3, extra 0-4, 5/10/19 frames) and `bounce.replay` (aligned, 5 frames): after every warp and then every 4 steps of the first 1,400 steps of level 10 (and steps 5-200 of orb-held) the visible
    picture (playfield columns 1-31 × lines 0-199 as the PPU shows them from the name table, scroll RAM $02/$03) equals the map's decoded tiles in **all** dumps taken outside the redraw itself (350 dumps; the six exceptions
    were taken during a warp's redraw, the camera RAM already new). Without steps 3-4's row patch the same dumps show 50-76 wrong tiles in the bottom screen rows from step 30 (level 10) and 26 from step ~90 (orb-held, 1 row missing). Log lines ("s", "e", "m", "init") of orb-held, level10 and bounce are **byte-identical**
    to those of the script before this amendment.
  - **Screen column 0 and the attributes of slots 0-1 are not the recipe's.** The left screen column (slot 0 of the ring, x 0-7) is hidden by the PPU's left-column mask and the game does not keep it up to date as the camera scrolls (hundreds of stale
    tiles accumulate there in a long run); and the attribute quadrants of slots 0-1 (attribute column 0, upper/lower left quadrants) are not always the map's: at an aligned camera (level 0, camera (800, 640)) they hold palette 3 where the map has 2,
    because the game's own full draw leaves them so. **A level-start full draw at the same camera gives the identical name table** (all 1,024 bytes equal, bytes $3C0-$3C7 = BB AA AA AA AA AA AA FF), so it is the
    original's own picture, not the recipe's; only slot 1 (x 8-15) of it is ever visible.
  - **"Wrong attribute column at x 224-239" (bounce.replay, camera (800, 640)) is not an original-side problem.** There the original is correct and equals the natural draw: attribute column 7 is $FF (palette 3 in all quadrants), and the map
    agrees: the chunk column 8 (world x 1,024-1,151 = screen x 224 on) has palette 3 on chunk rows 5-7 (chunk column 9 has 2). The original's screenshot has the olive (palette 3) rock from x 224 on, in every row,
    for the whole run. If the remake shows green at x 224-239 and olive only from x 240, its palette boundary is 16 px too far right.
