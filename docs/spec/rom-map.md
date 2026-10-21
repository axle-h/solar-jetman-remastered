# ROM map

Layout of the 256 KB PRG ROM of *Solar Jetman: Hunt for the Golden Warpship* (USA), by bank and region: what each
region holds, where it is, how sure we are, and its clean-room **category**, which decides what may be committed
(`docs/CLEAN_ROOM.md`, rule 5):

| Category | Meaning |
|---|---|
| **ripped** | graphics (tiles, pictures and their nametables/attributes, palettes) or sound (music and effects): imported from the player's ROM at first start, never committed |
| **commit** | level design, object placement, sprite layouts and animation, text, mechanics tables: may be extracted into `data/` by our tools |
| **constants (spec table)** | a numeric table (ramp, curve, offsets, frame numbers) embedded in a code region, whose every value is given in a spec; the remake holds it as Rust constants. Never an instruction stream |
| **code** | 6502 program; never copied, neither ripped nor committed |
| **fill** | unused bytes ($FF or $00 runs) |
| **unknown** | not identified yet; treat as ripped (never commit) until a spec says otherwise |

Tables that only say where graphics or sound are or which to use (offsets, lengths, pointers, palette preset
indexes, per-level choices) are **commit**; tables of pixel rows or colours are **ripped** (`docs/CLEAN_ROOM.md`,
rule 5).

Status per region: **[V]** verified (the program was seen using it this way in Mesen, or our decode matched the
running original), **[C]** read from the program, **[H]** hypothesis. Coverage is not complete; regions marked
unknown are honest gaps. Graphics details (formats, every blob) are in `graphics.md`.

Address conventions (`README.md`): `ROM 0x1A2B0` = offset into PRG ROM after the 16-byte header; `bank 3:$9C40` =
CPU address with that bank mapped. Bank `b` occupies ROM `b × 0x8000` to `b × 0x8000 + 0x7FFF`, and
`ROM offset = b × 0x8000 + (address − $8000)`.

## 1. The cartridge and bank switching

- **File:** NES 2.0 header (16 bytes), then 256 KB PRG ROM, no CHR ROM; 8 KB CHR RAM. SHA-1 of the PRG ROM
  `caa4d1ab710bd766f8505ef24f5702dac6e988af`. [V]
- **Mapper 7 (AxROM):** eight 32 KB banks; the whole of $8000-$FFFF is one bank at a time. A write of value `v`
  anywhere in $8000-$FFFF selects bank `v AND 7` and the visible nametable page (bit 4; one-screen mirroring). [V]
- **Writes avoid bus conflicts:** every bank switch writes to a ROM byte that already holds the value written,
  using two 8-byte tables present in every bank: bank 0-7 values `00`-`07` at $FFCB-$FFD2 and `10`-`17` at
  $FFDB-$FFE2 (same banks, other nametable page). [V]
- **Common block, identical in all eight banks: $FF7B-$FFE8** (ROM `b × 0x8000 + 0x7F7B` to `+ 0x7FE8`). It holds
  the reset entry, the interrupt entry, the end of the interrupt handler, the cross-bank call and the two
  bank-select tables. Everything else differs between banks. [V]
- **Vectors** (every bank): NMI $FFE3, reset $FF88. [V] The IRQ vector differs per bank; the game does not use
  IRQs. [C]
- **Reset** selects bank 0 and starts at bank 0:$8000. [V]
- **NMI** selects bank 0 and continues at the address held in RAM $050A-$050B; the end of the handler selects bank
  4, runs the sound update (bank 4 entry $800F), increments the frame counter RAM $5D, and selects again the bank
  recorded in RAM $75. [C]
- **RAM $75** always holds the value last written to select the bank (bank in bits 0-2, page in bit 4). [C]
- **Bank entry points:** every bank begins with a table of 3-byte jumps at $8000 (bank 0: one byte, then the table
  at $8001). Code in other banks is reached only through these entries: the caller gives the bank and the entry
  offset (the low byte of $80xx), the common block switches bank, calls the entry and switches back to the
  caller's bank. [V]

Number of entries: bank 0: 9, bank 1: 31, bank 2: 23, bank 3: 4, bank 4: 12, bank 5: 5, bank 6: 15,
bank 7: 17. [C]

For locating data, only the bank number matters: data used by a bank's code lives in that bank (the program
never reads data across banks except through the same switching), and all pointers stored in tables are 16-bit
CPU addresses within the bank that holds the table, unless stated otherwise in a spec.

## 2. Overview

| Bank | ROM | Main content |
|---|---|---|
| 0 | 0x00000-0x07FFF | main program (game loop, NMI, objects, screens), palettes, some text |
| 1 | 0x08000-0x0FFFF | code (text printer, equipment, objects), text, object definitions, sprite tile graphics |
| 2 | 0x10000-0x17FFF | code (level setup, menus), level headers, title, fonts, terrain tile graphics |
| 3 | 0x18000-0x1FFFF | map renderer code and planet map data |
| 4 | 0x20000-0x27FFF | object/level scripts, launch scene graphics, sound driver and music data |
| 5 | 0x28000-0x2FFFF | planet-surface pictures and equipment pictures (almost entirely graphics) |
| 6 | 0x30000-0x37FFF | attract sequence, approach screen, menus, equipment screen, warp-level tiles |
| 7 | 0x38000-0x3FFFF | code (gameplay parts, backdrops), surface picture 6, ending picture, launch background |

Totals (approximate): graphics and palettes about 113 KB, sound about 22 KB, code about 55 KB, level/object/text
data about 50 KB, fill about 8 KB, the rest unknown.

## 3. Bank by bank

Regions are listed in address order. Small embedded tables inside code areas are listed separately when known.

### Bank 0 (ROM 0x00000-0x07FFF)

| ROM | Bank 0 | What | Category | Status |
|---|---|---|---|---|
| 0x00000-0x0001B | $8000-$801B | reset start and entry table | code | [V] |
| 0x0001C-0x065E8 | $801C-$E5E8 | main program: start-up, title and attract control, approach screen, gameplay, objects, NMI handler, screen updates. Contains the small tables below; parts not executed in our runs are probably code too | code | [V] where executed, [H] elsewhere |
| 0x00066-0x0006B | $8066-$806B | 6-byte signature compared with RAM to detect a warm reset | commit | [C] |
| 0x01693-0x0169C, 0x0169D-0x016A6 | $9693-$969C, $969D-$96A6 | (inside the code region) score digit tile numbers, top row then bottom row, 10 each (`graphics.md` 1.2, digit tile pairs) | constants (spec table) | [C] |
| 0x017EA-0x0180E | $97EA-$980E | "SYSTEM" and the five system names, 6 characters each (ZANDOR ...) | commit (text) | [V] |
| 0x01973-0x0197A | $9973-$997A | gauge tile row values (`graphics.md` section 3) | ripped | [C] |
| 0x00276, 0x0027A, 0x0027E | $8276, $827A, $827E | (inside the code region) three single colour bytes, the operands of three loads, used at the end of the title logo fade-in (`graphics.md` 7.4) | ripped | [C] |
| 0x01F12-0x0200A | $9F12-$A00A | (inside the code region) a 16-digit string, an offset table, and object names ("TENNIS BALLS", "HAMBURGERS", ...) separated by 0 bytes | commit (text) | [C] |
| 0x02112-0x02119 | $A112-$A119 | one-bit-per-column masks for the split-marker tile (`graphics.md` section 3); a trivial bit pattern, not artwork | commit | [C] |
| 0x0286E-0x028AE | $A86E-$A8AE | (inside the code region) warp-level mesh tables for levels 29-41, five consecutive 13-byte tables indexed by level - 29: a flag byte per level (bit 7 = helper 23 homes on the pod, bit 6 = helper 24 also homes), then the source bytes for helper 23 x, helper 23 y, helper 24 x, helper 24 y (`objects.md` 10.6, `graphics.md` 5.7.2). All 65 bytes are pure data; none is program code | commit (level design) | [V] |
| 0x02B97-0x02BB0 | $AB97-$ABB0 | level 26 camera route: 26 waypoints (`levels.md` 5.2) | constants (spec table) | [V] |
| 0x02BB1-0x02BCA | $ABB1-$ABCA | level 26 camera route: 26 move codes, 0 = end (`levels.md` 5.2) | constants (spec table) | [V] |
| 0x04A53-0x04A92 | $CA53-$CA92 | tractor rope offsets: 64 values, quarter table and catch radius (`player.md` 8.2) | constants (spec table) | [V] |
| 0x04A93-0x04AA2 | $CA93-$CAA2 | exhaust flame base frame by facing, 16 values (`player.md` 4.8) | constants (spec table) | [V] |
| 0x04AA3-0x04AC2 | $CAA3-$CAC2 | a ramp of 32 bytes: $E0 minus 9 per entry for 25 entries ($E0 ... $08), then 7 zeros. No code reads it (`player.md` 4.8, note) | constants (spec table) | [C] |
| 0x04AC3-0x04AEA | $CAC3-$CAEA | shot and flame placement offsets `off`, 40 signed bytes (`player.md` 10, 4.8) | constants (spec table) | [V] |
| 0x04AEB-0x04AF2 | $CAEB-$CAF2 | boosted-flame frame offset pattern 0, 1, 2, 1, 0, 1, 2, 1 (`player.md` 4.8) | constants (spec table) | [C] |
| 0x051A1-0x052DF | $D1A1-$D2DF | credits text ("SOLAR JETMAN CREDITS" ... "A TRADE WEST PRODUCTION") with control bytes | commit (text) | [V] |
| 0x052E0-0x052E1 | $D2E0-$D2E1 | last two bytes of the credits text (control bytes $DA, $80) | commit (text) | [H] |
| 0x052E2-0x052F6 | $D2E2-$D2F6 | launch scene pilot walk: x of each of 21 steps (`scenes.md` 9, phase 1) | constants (spec table) | [V] |
| 0x052F7-0x0530B | $D2F7-$D30B | launch scene pilot walk: y of each of 21 steps (`scenes.md` 9, phase 1) | constants (spec table) | [V] |
| 0x0530C-0x05320 | $D30C-$D320 | launch scene pilot walk: frame of each of 21 steps (`scenes.md` 9, phase 1) | constants (spec table) | [V] |
| 0x05321-0x05323 | $D321-$D323 | three zero bytes, padding | fill | [C] |
| 0x05787-0x057C2 | $D787-$D7C2 | (inside the code region) nametable address of each of the 30 tile rows: 30 high bytes, then 30 low bytes; used for the status-panel split marker (`graphics.md` section 3). Addresses only, not picture data | commit | [C] |
| 0x061D5-0x06228 | $E1D5-$E228 | (inside the code region) per-level pointers to the chunk palette maps: 42 low bytes, then 42 high bytes (`levels.md`) | commit | [C] |
| 0x065E9-0x065F4 | $E5E9-$E5F4 | small mask and pattern tables used by the code before them | commit | [C] |
| 0x065F5-0x071E8 | $E5F5-$F1E8 | per-level chunk palette maps (which attribute palette each map chunk uses; level design, `levels.md`), including runs of $FF | commit (level design) | [C] |
| 0x071E9-0x07462 | $F1E9-$F462 | code (memory clear, PPU helpers) | code | [V] |
| 0x07273-0x07278 | $F273-$F278 | (inside the previous region) 6-byte template of the sound effect kept in RAM (`sound.md`) | ripped (sound) | [C] |
| 0x07463-0x07525 | $F463-$F525 | per-level palette colours: approach globe (3 bytes per level) then surface picture (12 bytes per level) | ripped | [V] |
| 0x07526-0x0771D | $F526-$F71D | palette preset colour data | ripped | [V] |
| 0x0771E-0x0778D | $F71E-$F78D | palette preset index tables (pointers, first entry, count) | commit | [V] |
| 0x0778E-0x077B2 | $F78E-$F7B2 | palette preset loader | code | [V] |
| 0x077B3-0x077DC | $F7B3-$F7DC | title logo fade-in colours: six 7-step columns (`graphics.md` 7.4) | ripped | [C] |
| 0x077DD-0x07820 | $F7DD-$F820 | title screen layout tables (positions, tile numbers of the menu box) | commit | [C] |
| 0x07821-0x0784C | $F821-$F84C | title menu text "START GAME", "CONTINUE", copyright line | commit (text) | [V] |
| 0x0784D-0x07F87 | $F84D-$FF87 | $FF | fill | [V] |
| 0x07F88-0x07FE8 | $FF88-$FFE8 | common block (section 1) | code | [V] |
| 0x07FE9-0x07FF9 | $FFE9-$FFF9 | continuation of the NMI entry (jump through RAM $050A), then $FF | code | [V] |
| 0x07FFA-0x07FFF | $FFFA-$FFFF | vectors | code | [V] |

### Bank 1 (ROM 0x08000-0x0FFFF)

| ROM | Bank 1 | What | Category | Status |
|---|---|---|---|---|
| 0x08000-0x0805C | $8000-$805C | entry table | code | [V] |
| 0x0805D-0x080EC | $805D-$80EC | nine 16-byte tile images (low plane then high plane) of the tractor beam, uploaded to sprite tile $CF (`player.md` 8.4) | ripped (graphics, not committed) | [V] |
| 0x080ED-0x080FF | $80ED-$80FF | $FF | fill | [V] |
| 0x08100-0x0821F | $8100-$821F | point pop-up sprite tiles | ripped | [C] |
| 0x08220-0x09599 | $8220-$9599 | code: objects, collisions, HUD, text and equipment helpers, with the tables below | code | [V] mostly |
| 0x08685-0x08857 | $8685-$8857 | (inside the previous region) per-frame collision half-widths ($8685, 193 bytes) and half-heights ($8746, 193 bytes), then per-state flag tables $87F8-$881C and $881D-$8857 (`objects.md`) | commit | [C] |
| 0x0941B-0x0942A | $941B-$942A | tractor beam frame by phase, 16 values (`player.md` 8.3, 8.4); the 16 image offsets `00 10 ... 80 ... 10` precede it at $940B-$941A | constants (spec table) | [V] |
| 0x0959A-0x096A7 | $959A-$96A7 | equipment names ("HOMING MISSILES", "ANTI GRAVITY", ...), 0-separated | commit (text) | [C] |
| 0x096A8-0x096C9 | $96A8-$96C9 | small equipment tables | commit | [C] |
| 0x096CA-0x09CFF | $96CA-$9CFF | code (equipment, password, text printer, common sprite loader) | code | [V] |
| 0x098E1-0x098F0 | $98E1-$98F0 | (inside the previous region) the 16 password letters `BDGHKLMNPQRTVWXZ` | commit | [V] |
| 0x09D00-0x0A4FB | $9D00-$A4FB | screen texts (licence screen, password screen, messages, analysis lists, approach screen) with control bytes $FB-$FF | commit (text) | [V] |
| 0x0A4FC-0x0A54B | $A4FC-$A54B | run-length decoder | code | [V] |
| 0x0A54C-0x0C88B | $A54C-$C88B | sprite layouts and the layout tables P, A and B (low/high pairs at $A859/$A879, $A899/$A95A, $AA1B/$AAF5; `objects.md`) | commit (sprite layout) | [V] |
| 0x0C88C-0x0C97F | $C88C-$C97F | object sprite set loader | code | [V] |
| 0x0C980-0x0CB44 | $C980-$CB44 | object sprite set lists per level and tile records (`graphics.md` 6.4) | commit (sprite layout) | [V] |
| 0x0CB45-0x0F676 | $CB45-$F676 | sprite tile graphics (pod sets, common sprites, object sets) and one tile stream | ripped | [V] |
| 0x0F677-0x0F6AD | $F677-$F6AD | animated-tile streamer | code | [C] |
| 0x0F6AE-0x0F84D | $F6AE-$F84D | animated tiles, 13 frames of 2 tiles | ripped | [C] |
| 0x0F84E-0x0F91D | $F84E-$F91D | not identified (looks like tile data) | unknown | [H] |
| 0x0F91E-0x0FED1 | $F91E-$FED1 | sprite and status panel tiles | ripped | [V] |
| 0x0FED2-0x0FF2E | $FED2-$FF2E | small loaders | code | [V] |
| 0x0FF2F-0x0FF47 | $FF2F-$FF47 | sprite tile stream | ripped | [V] |
| 0x0FF48-0x0FF7A | $FF48-$FF7A | IRQ return byte, $FF | fill | [V] |
| 0x0FF7B-0x0FFFF | $FF7B-$FFFF | common block, vectors | code | [V] |

### Bank 2 (ROM 0x10000-0x17FFF)

| ROM | Bank 2 | What | Category | Status |
|---|---|---|---|---|
| 0x10000-0x10044 | $8000-$8044 | entry table | code | [V] |
| 0x10045-0x112ED | $8045-$92ED | code: font loading, level setup (reads the level header, loads terrain and palettes), menus | code | [V] mostly |
| 0x1022C-0x10267 | $822C-$8267 | (inside the previous region) planet menu box text (EXPLORE, EQUIP POD, BLAST OFF, BUY) with frame characters | commit (text) | [V] |
| 0x102A4-0x102C7 | $82A4-$82C7 | planet menu layouts: 4 lists of record numbers (9, 12, 6, 9 entries) (`text.md` 4.6) | constants (spec table) | [V] |
| 0x106B1-0x106CC | $86B1-$86CC | first frame by item kind 0-27 (`behaviour.md` 6.1, table B frames; `levels.md` 3.3) | constants (spec table) | [V] |
| 0x106CD-0x106E8 | $86CD-$86E8 | initial state by item kind 0-27 (`behaviour.md` 6.1; `levels.md` 3.3) | constants (spec table) | [V] |
| 0x112EE-0x11457 | $92EE-$9457 | attributes, palettes ($932E, $933D: 15 colours each) and nametable of the screen of `graphics.md` 5.10 | ripped | [V] |
| 0x11458-0x114E3 | $9458-$94E3 | terrain loader | code | [V] |
| 0x114E4-0x114FF | $94E4-$94FF | terrain stream table (start and length for 7 graphics types) | commit | [V] |
| 0x11500-0x11566 | $9500-$9566 | run-length decoder, PPU address helper | code | [V] |
| 0x11567-0x1156D | $9567-$956D | graphics type → surface picture map | commit | [V] |
| 0x1156E-0x115C1 | $956E-$95C1 | level header pointer tables (42 low bytes, then 42 high bytes) | commit | [V] |
| 0x115C2-0x115C7 | $95C2-$95C7 | per-block-set table of collision class maps (`levels.md`) | commit | [C] |
| 0x115C8-0x11A35 | $95C8-$9A35 | 42 level headers, 27 bytes each. Bytes 12-23 of each header are palette colours (ripped); the other 15 bytes are level data (commit) | commit, except bytes 12-23: ripped | [V] |
| 0x11A36-0x1209D | $9A36-$A09D | per-level object lists, 5 bytes per object, reached from the headers (pointers in header bytes 25-26; `levels.md`) | commit | [C] |
| 0x1209E-0x122DD | $A09E-$A2DD | code and small tables (title loaders) | code | [C] |
| 0x122DE-0x12CBA | $A2DE-$ACBA | title logo tiles, nametable, attributes | ripped | [V] |
| 0x12CBB-0x12DA4 | $ACBB-$ADA4 | screen loader | code | [C] |
| 0x12DA5-0x15D3F | $ADA5-$DD3F | tile graphics: unknown screens, three terrain sets and extras, title sprites, fonts | ripped | [V] |
| 0x15D40-0x1663F | $DD40-$E63F | raw tiles of the bank 2 entry $8003 screen | ripped | [V] |
| 0x16640-0x173A4 | $E640-$F3A4 | terrain set (types 2, 3, 5), common gameplay background | ripped | [V] |
| 0x173A5-0x17464 | $F3A5-$F464 | data of unknown use (`levels.md`) | unknown | [C] |
| 0x17465-0x176A9 | $F465-$F6A9 | code (collision) | code | [C] |
| 0x176AA-0x176EB | $F6AA-$F6EB | collision geometry constants | commit | [C] |
| 0x176EC-0x178E9 | $F6EC-$F8E9 | three block-to-collision-class maps | commit | [C] |
| 0x178EA-0x17953 | $F8EA-$F953 | collision class pointer tables | commit | [C] |
| 0x17954-0x17AC9 | $F954-$FAC9 | collision class records | commit | [C] |
| 0x17ACA-0x17ACB | $FACA-$FACB | not identified | unknown | |
| 0x17ACC-0x17F87 | $FACC-$FF87 | $FF | fill | [V] |
| 0x17F88-0x17FFF | $FF88-$FFFF | common block, vectors | code | [V] |

### Bank 3 (ROM 0x18000-0x1FFFF)

| ROM | Bank 3 | What | Category | Status |
|---|---|---|---|---|
| 0x18000-0x18593 | $8000-$8593 | entry table and map renderer (draws planet maps into the nametable during play) | code | [V] |
| 0x18594-0x18599 | $8594-$8599 | block-set pointer table | commit | [C] |
| 0x1859A-0x185ED | $859A-$85ED | the 42 map pointers | commit | [C] |
| 0x185EE-0x1870A | $85EE-$870A | code | code | [C] |
| 0x1870B-0x1BA01 | $870B-$BA01 | maps (`levels.md`) | commit (level design) | [C] |
| 0x1BA02-0x1C1F9 | $BA02-$C1F9 | block sets: 3 × 170 blocks × 4 bytes | commit (level design) | [C] |
| 0x1C1FA-0x1FF79 | $C1FA-$FF79 | chunks: 246 × 64 bytes | commit (level design) | [C] |
| 0x1FF7A | $FF7A | not identified (one byte) | unknown | |
| 0x1FF7B-0x1FFFF | $FF7B-$FFFF | common block, vectors | code | [V] |

### Bank 4 (ROM 0x20000-0x27FFF)

| ROM | Bank 4 | What | Category | Status |
|---|---|---|---|---|
| 0x20000-0x20023 | $8000-$8023 | entry table | code | [V] |
| 0x20024-0x200F2 | $8024-$80F2 | code | code | [V] |
| 0x200F3-0x201C4 | $80F3-$81C4 | per-level placement tables, 42 entries each: record address low, high, count, type-list address low, high (`objects.md`) | commit (level design) | [C] |
| 0x201C5-0x20416 | $81C5-$8416 | code that walks the per-level placement records and compares positions with the camera | code | [V] |
| 0x20417-0x214C6 | $8417-$94C6 | placement records and type lists, no gaps | commit (level design) | [V] |
| 0x214C7-0x2158F | $94C7-$958F | run-length decoder, launch scene loader | code | [V] |
| 0x21590-0x2159B | $9590-$959B | launch scene pod table: start low, start high, length low, length high, a 3-byte column each (pod types 0-2) | commit | [V] |
| 0x2159C-0x2225D | $959C-$A25D | launch scene graphics: pod sprites, sprites, attributes, nametable | ripped | [V] |
| 0x2225D-0x2231F | $A25D-$A31F | per object type: initial-frame table ($A25D, 98 entries) and initial-state table ($A2BE, 98 entries), overlapping by one byte (`objects.md`). The first byte, $A25D, is also the last byte of the launch nametable stream | commit | [C] |
| 0x22320-0x22341 | $A320-$A341 | music: tempo per song (34 songs) | ripped (sound) | [C] |
| 0x22342-0x22846 | $A342-$A846 | sound driver | code | [V] |
| 0x22847-0x228C0 | $A847-$A8C0 | note period table: 61 16-bit values (tuning constants; `sound.md`) | commit | [C] |
| 0x228C1-0x22906 | $A8C1-$A906 | sound-effect pointers (location table; `sound.md`) | commit | [C] |
| 0x22907-0x22A16 | $A907-$AA16 | track pointer tables: 34 songs × 4 channels, low bytes at $A907, high at $A98F (location table) | commit | [C] |
| 0x22A17-0x27E4B | $AA17-$FE4B | music and sound-effect track data | ripped (sound) | [C] |
| 0x27E4C-0x27E54 | $FE4C-$FE54 | 9 stray bytes, a copy of bank 0 code (`sound.md`) | code | [C] |
| 0x27E55-0x27F7A | $FE55-$FF7A | $FF | fill | [V] |
| 0x27F7B-0x27FFF | $FF7B-$FFFF | common block, vectors | code | [V] |

### Bank 5 (ROM 0x28000-0x2FFFF)

| ROM | Bank 5 | What | Category | Status |
|---|---|---|---|---|
| 0x28000-0x2811C | $8000-$811C | entry table, surface picture and palette-row loaders | code | [V] |
| 0x2811D-0x281A8 | $811D-$81A8 | surface picture stream tables (start and length per picture) | commit | [V] |
| 0x281A9-0x281CF | $81A9-$81CF | item picture loader | code | [V] |
| 0x281D0-0x281E8 | $81D0-$81E8 | item picture tables: start low ×4, start high ×4, length low ×4, length high ×4, then the 8-entry item-to-picture map and one more map byte ($03 for item 8) that the loader never reads (it accepts items 0-7) | commit | [V] |
| 0x281E9-0x28289 | $81E9-$8289 | PPU address helper, raw copier, run-length decoder | code | [V] |
| 0x2828A-0x2FA20 | $828A-$FA20 | surface pictures (attributes, nametables, tile streams) and equipment pictures | ripped | [V] |
| 0x2FA21-0x2FF87 | $FA21-$FF87 | $FF | fill | [V] |
| 0x2FF88-0x2FFFF | $FF88-$FFFF | common block, vectors | code | [V] |

### Bank 6 (ROM 0x30000-0x37FFF)

| ROM | Bank 6 | What | Category | Status |
|---|---|---|---|---|
| 0x30000-0x3002C | $8000-$802C | entry table | code | [V] |
| 0x3002D-0x30035 | $802D-$8035 | demo lookup: the 3 demo levels (6, 4, 10), then the low and high bytes of each level's waypoint list start (`game-flow.md` 7.1) | commit | [V] |
| 0x30036-0x301ED | $8036-$81ED | demo waypoint lists for levels 6, 4 and 10, stored back to back: 88 entries of 5 bytes (x 16-bit, y 16-bit, radius), 27 + 25 + 36 (`game-flow.md` 7.1). Map coordinates the demo pilot steers by; not sprite data, not placement data | commit (demo script) | [V] |
| 0x301EE-0x30972 | $81EE-$8972 | code: attract sequence, approach screen, loaders | code | [V] |
| 0x30973-0x32349 | $8973-$A349 | attract Jetman picture tiles | ripped | [V] |
| 0x3234A-0x32B5D | $A34A-$AB5D | attract Jetman animation: nametable and attribute frames | ripped (picture) | [C] |
| 0x32B5E-0x32E55 | $AB5E-$AE55 | code | code | [C] |
| 0x32E56-0x34295 | $AE56-$C295 | equipment screen tile blocks | ripped | [V] first blocks, [H] rest |
| 0x34296-0x352FC | $C296-$D2FC | comet screen picture (nametable, tiles) | ripped | [V] |
| 0x352FD-0x35393 | $D2FD-$D393 | planet numbers and names for the approach screen ("1", "PRELUDON" ... "14", "DRYN KUP LEEZ"; last character of each name has bit 7 set). 14 names; levels 0-12 are the planets, the 14th name shows only if level 13 is forced (`levels.md`) | commit (text) | [V] |
| 0x35394-0x36059 | $D394-$E059 | equipment screen tiles, Space Loonies logo, approach screen picture | ripped | [V] |
| 0x3605A-0x36148 | $E05A-$E148 | menu screen loaders | code | [C] |
| 0x36149-0x36ACA | $E149-$EACA | menu screen nametables, attributes, frame tiles | ripped | [V] |
| 0x36ACB-0x36B42 | $EACB-$EB42 | warp-level tile loader | code | [V] |
| 0x36B43-0x37802 | $EB43-$F802 | warp-level background tiles (12 sets) | ripped | [V] |
| 0x37803-0x3791B | $F803-$F91B | not identified | unknown | |
| 0x3791C-0x37F87 | $F91C-$FF87 | $FF | fill | [V] |
| 0x37F88-0x37FFF | $FF88-$FFFF | common block, vectors | code | [V] |

### Bank 7 (ROM 0x38000-0x3FFFF)

| ROM | Bank 7 | What | Category | Status |
|---|---|---|---|---|
| 0x38000-0x38032 | $8000-$8032 | entry table | code | [V] |
| 0x38033-0x38FFF | $8033-$8FFF | code (backdrop scrolling, loaders, gameplay helpers) with small tables: $80F8 (3 level numbers), $832A-$8345 (backdrop palette colours, ripped), and the constant tables in the following rows | code | [C] |
| 0x38297-0x382A9 | $8297-$82A9 | explosion frame sequences, 19 bytes, $FF ends a sequence (`objects.md` 2.5) | constants (spec table) | [C] |
| 0x38856-0x38865 | $8856-$8865 | thrust magnitude by facing, 16 values, cosine x 64 (`player.md` 4.4) | constants (spec table) | [C] |
| 0x38B10-0x38B97 | $8B10-$8B97 | `atan` table, 136 bytes (`player.md` 5.1) | constants (spec table) | [C] |
| 0x38B98-0x38BA7 | $8B98-$8BA7 | `tri` table, 16 values n(n+1)/2 (`player.md` 5.1) | constants (spec table) | [C] |
| 0x38C74-0x38CF3 | $8C74-$8CF3 | `sin` table, 128 values; its 129th value (index 128, 0) is the first byte of the next table (`player.md` 5.2, `behaviour.md` 4.18) | constants (spec table) | [C] |
| 0x38CF4-0x38DF3 | $8CF4-$8DF3 | low bytes of n squared, n = 0 to 255 (`player.md` 5.2, "Square table") | constants (spec table) | [C] |
| 0x38DF4-0x38EF3 | $8DF4-$8EF3 | high bytes of n squared, n = 0 to 255 (`player.md` 5.2, "Square table") | constants (spec table) | [C] |
| 0x39000-0x3904D | $9000-$904D | code | code | [C] |
| 0x3904E-0x39059 | $904E-$9059 | palette colours of an object's flash effect, 4 steps × 3 (`graphics.md` 7.4) | ripped | [C] |
| 0x3905A-0x3A42F | $905A-$A42F | surface picture 6 tiles | ripped | [V] |
| 0x3A430-0x3A80C | $A430-$A80C | backdrop tiles | ripped | [V] |
| 0x3A80D-0x3A88C | $A80D-$A88C | not identified | unknown | |
| 0x3A88D-0x3AA4C | $A88D-$AA4C | backdrop raw tiles | ripped | [V] |
| 0x3AA4D | $AA4D | not identified (one byte) | unknown | |
| 0x3AA4E-0x3AB06 | $AA4E-$AB06 | per-state handler address tables (code pointers; `objects.md`) | commit | [C] |
| 0x3AB07-0x3AB63 | $AB07-$AB63 | initial hit points per object state ($AB07 for states $00-$21, $AB29 for states $80-$BA) | commit | [V] |
| 0x3AB64-0x3D378 | $AB64-$D378 | code (gameplay helpers run during levels) with unknown tables between, e.g. $CF65-$D2FF | code | [V] where executed, [C] elsewhere |
| 0x3B399-0x3B418 | $B399-$B418 | formation circle table `T`, 128 values; one more byte (0) follows at $B419 and is never read (`behaviour.md` 4.12) | constants (spec table) | [V] |
| 0x3B41A-0x3B421 | $B41A-$B421 | formation frames, state $A9, 8 values (`behaviour.md` 4.12) | constants (spec table) | [V] |
| 0x3B422-0x3B441 | $B422-$B441 | formation frames, state $AA, 16 values twice (`behaviour.md` 4.12) | constants (spec table) | [V] |
| 0x3D379-0x3F14A | $D379-$F14A | ending picture | ripped | [V] |
| 0x3F14B-0x3F15E | $F14B-$F15E | launch background loader | code | [V] |
| 0x3F15F-0x3FF77 | $F15F-$FF77 | launch scene background tiles | ripped | [V] |
| 0x3FF78-0x3FF7A | $FF78-$FF7A | IRQ return byte, $FF | fill | [V] |
| 0x3FF7B-0x3FFFF | $FF7B-$FFFF | common block, vectors | code | [V] |

## 4. Where to find things (quick index)

| Looking for | Where |
|---|---|
| Tile graphics, pictures, palettes | `graphics.md` (complete catalogue with ROM offsets) |
| Level headers (42 levels) | bank 2, pointer tables ROM 0x1156E / 0x11598, headers ROM 0x115C8-0x11A35 |
| Level index meaning | RAM $3B: 0-12 planets, 13-25 caverns, 26 the long scrolled level, 27-28 small rooms, 29-41 warp levels (`levels.md`) |
| Planet maps | bank 3 (ROM 0x1859A-0x1FF79), `levels.md` |
| Object sprite sets per level | bank 1 ROM 0x0C980 (lists), 0x0CAB4 (records) |
| Object layouts / placement | bank 1 ROM 0x0A54C-0x0C88B (layouts), bank 4 ROM 0x200F3-0x201C4 and 0x20417-0x214C6 (placement); `objects.md` |
| Text | bank 1 ROM 0x09D00-0x0A4FB and 0x0959A; bank 0 ROM 0x017EA, 0x01F12, 0x051A1, 0x07821; bank 2 ROM 0x1022C; bank 6 ROM 0x352FD |
| Music and sound effects | bank 4 ROM 0x22320-0x22341, 0x22847-0x27E4B; bank 0 ROM 0x07273; `sound.md` |

## Open questions

- Unknown regions: bank 1 $805D-$80EC, $F84E-$F91D; bank 2 $F3A5-$F464,
  $FACA-$FACB; bank 6 $F803-$F91B; bank 7 $A80D-$A88C, $AA4D and any tables inside
  $8033-$8FFF and $AB64-$D378 not listed in section 3.
- Planet count: 14 approach-screen names but 13 planet levels (`levels.md`); whether the 14th name is ever used.
- Code coverage comes from Mesen runs of the title, attract sequence, all 42 levels' start and random play;
  code for situations not reached (shop, password entry, ending, game over) was identified statically.
