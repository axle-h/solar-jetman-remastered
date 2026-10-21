# Text: character set, printer, and every string

How the original stores and prints its text: the character codes, the screen-text printer with its control
bytes and typewriter mode, the message pointer table, and a catalogue of every message and string found, with
where it is used. Text is **commit** data (`docs/CLEAN_ROOM.md`, rule 5), so the strings are quoted here.

The exceptions are the few tile pictures embedded in messages (the shop sign, the probe pictures, one logo). Those
runs are nametable layouts of pictures, so they are **ripped**: this spec gives their location and size, not their
contents.

Status: **[V]** verified in Mesen or by decoding the data with the rules below, **[C]** read from the program,
**[H]** hypothesis. Addresses follow `README.md`. For bank 1, the CPU address and the ROM offset have the same
value (bank 1 $9CDA = ROM 0x09CDA).

## 1. Character set

Text bytes are tile numbers in the background pattern table ($1000). Every screen that shows text first loads the
font so that the codes come out as ASCII-like letters (`graphics.md` 2.4 and 5.1: digits at tile $30, letters at
tile $41). **[V]**

| Code | Glyph |
|---|---|
| $00 | blank tile (the usual space) |
| $20 | space in plain ASCII. The printer turns it into $00 unless the message starts with $FC (section 2). The credits and the title copyright line use $20 directly |
| $30-$39 | 0-9 |
| $3A | © |
| $3B | ™ |
| $41-$5A | A-Z |
| $5B | × (multiplication sign) |
| $5C | - |
| $5D | . (full stop at the left of the tile, so it also acts as the following space) |
| $5E | , (comma at the left of the tile, same) |
| $5F | a glyph that looks like a small "ED"; no text found uses it |

- There is no colon in the loaded font. A colon-shaped glyph sits unused between the two font blocks
  (`graphics.md` 4.2).
- Codes $01-$1F, $21-$2F and $80-$FA are other tiles: picture pieces, menu frame pieces.
- Lower case does not exist.

**[V]** (glyph shapes decoded from the font data; strings checked against screenshots)

In this spec, "." and "," at the end of a word are written followed by a space, as they appear on screen. For
example, the letters TRADEWEST, the byte $5E, the letters of INC, the byte $5D and then PRESENTS display as "TRADEWEST, INC. PRESENTS".

## 2. The screen-text printer **[V]**

A message is drawn by the printer (bank 1 entry $8018) given a message id in RAM $04. The id indexes two tables
of 52 entries in bank 1: low bytes at $9C72 (ROM 0x09C72), high bytes at $9CA6 (ROM 0x09CA6). Each entry is a CPU
address in bank 1. The message data lies at $9CDA-$A4FB (ROM 0x09CDA-0x0A4FB).

### 2.1 Message bytes

| Byte | Meaning |
|---|---|
| $FC | only as the first byte: "picture mode". Code $20 is not turned into a blank, so it can be a picture tile. $FE, $FD, $FB and $FF still act as below (messages 0 and 49 use $FE and $FF in picture mode) **[C]** |
| $FE `col` `row` | move the print position to nametable column `col` (0-31), row `row` (0-29): PPU address $2000 + 32 × row + col. Row 30 reaches the attribute table (one message uses it) |
| $FD `n` then `n` bytes | write the `n` bytes as they are, with no conversion (no message uses it) |
| $FB | switch to typewriter mode for the rest of the message (2.3) |
| $FF | end of message |
| anything else | one tile at the print position; the position advances by one column. Outside picture mode, $20 becomes $00 |

- There is no automatic line break and no wrapping. Every line starts with its own $FE.
- Most texts are not centred by the printer: the column in $FE places them. (Planet names and the credits are
  centred, by their own code: section 4.)

### 2.2 Immediate mode

By default the printer writes the whole message in one call, straight to the PPU. Screens call it while rendering
is off, during scene setup. So the text appears complete on the first frame shown.

### 2.3 Typewriter mode **[V]**

After a $FB byte, the rest of the message is typed out:

- **Rate:** one item per 4 frames. A scene that shows typed text calls the resume entry (bank 1 entry $801B)
  once per pass, and an item is typed when the frame counter RAM $5D is then a multiple of 4.
  - **Message screen:** the call comes after the pass's own $5D increment. The screen starts with $5D = 0, so
    the first item is typed in its 4th pass (when $5D has just become 4), then every 4 passes. **[V]** (first item
    3 frames after the screen's first frame, then one every 4)
  - **Last-password screen:** the call comes before the increment. That screen's text is not typed, so this does
    not show. **[C]**
- **What counts as an item:**
  - a character takes one 4-frame slot, blanks included;
  - a $FE position change also takes one slot, and draws nothing;
  - $FF ends typing.
- **Display:** each character goes through the NMI's video buffer, so it appears on the next frame.
- **Sound:** every character other than $00 and $20 typed by the resume entry plays sound effect $3A, the typing
  click. Text written in immediate mode (2.2) makes no click. **[C]** (the click is in the resume path only)
- **$FB takes no slot:** it ends the immediate part of the message; the first typed item is the byte after it.
  The conversion of $20 to $00 applies in typewriter mode as well (only picture mode keeps $20). **[C]**
- **Resume state:** RAM $0596-$0597 (address of the next byte, $FF in $0597 when finished), $0598 (mode flags),
  $0599-$059A (PPU address of the next character).

**Example, message 7.** The probe picture appears at once. Then "YOUR PROBES ARE NOW" starts typing on the first
frame with $5D a multiple of 4, one letter every 4 frames, each new line costing one extra slot. Measured: the
"$FE 7,15" slot, then "Y" 4 frames later, "O" 4 frames after that, and so on.

## 3. Message catalogue (printer ids 0-51) **[V]** text, **[C]** use

`{col,row}` is a $FE position, `{TYPE}` the start of typewriter mode, and `[picture W×H at col,row]` a ripped
tile picture (picture mode, or tile codes $80 and above). Line breaks in the table follow the $FE commands.

| Id | Address (bank 1 / ROM) | Text | Used for |
|---|---|---|---|
| 0, 1, 3 | $9CDA-$9D15 | [picture: shop sign, rows 2-5 from column 12] {21,22} IN YOUR {21,23} ACCOUNT | shop: sign and money label (3 in shop mode only; 0 and 1 share the data) |
| 2 | $9D16-$9D52 | {3,20} YOU OWN {3,23} OF THEM {23,26} FIRE A {22,27} TO SELECT {2,26} FIRE B {2,27} TO EXIT | shop and equip list |
| 4 | $9D53-$9D7D | {9,9} ENTER PASSWORD {5,15} OR PRESS START TO PLAY | password entry |
| 5 | $9E37-$9E50 | {5,9} YOUR LAST PASSWORD WAS | after game over (the 12 letters follow at row 12, column 10) |
| 6, 7 | $9E51-$9EC7 | [picture of a probe with shields, rows 9-13] {TYPE} {7,15} YOUR PROBES ARE NOW {6,16} EQUIPPED WITH SHIELDS {5,17} -PRESS DOWN TO ACTIVATE | shields found (6 and 7 share the data) |
| 8 | $9EC8-$9F48 | [picture, rows 8-14] {TYPE} {5,15} YOU HAVE EQUIPPED YOUR {6,16} PROBES WITH BOOSTERS {4,17} -SELECT AND B TO THRUST | boosters found |
| 9 | $9F49-$9FB6 | [picture, rows 8-15] {TYPE} {5,16} YOU HAVE EQUIPPED YOUR {5,17} PROBES WITH NAVIGATORS | navigators found (sets the mapping-device flag RAM $060C) |
| 10-20 | $9FB7 | (empty: a lone $FF) | unused |
| 21 | $9FB8-$9FE5 | {TYPE} {7,14} YOU NEED MORE FUEL {6,16} TO LEAVE THIS PLANET | fuel delivered, not yet enough |
| 22 | $9FE6-$A034 | {TYPE} {7,12} YOU HAVE COLLECTED {10,13} ENOUGH FUEL {5,15} NOW FIND THE SHIP PART {9,17} AND BLAST OFF | fuel delivered, enough (the data has two blanks between ENOUGH and FUEL) |
| 23 | $A080-$A117 | {5,6} CONGRATULATIONS JETMAN {7,10} YOU HAVE COMPLETED {10,12} YOUR MISSION {8,16} TIME TO PUT YOUR {7,18} FEET UP AND RELAX {5,20} IN THE COMFORT OF YOUR {5,22} LUXURY GOLDEN WARPSHIP | end of the final level (two blanks between UP and AND). **No {TYPE}: the 152 data bytes contain no $FB, so the printer writes the whole message at once** and all seven lines are on screen on the first visible frame of the message screen **[V]** (2026-10-17, bytes checked and the message screen entered in Mesen with screenshots 2 and 10 frames after the entry). Earlier text that said it is typed at one character per 4 frames was wrong |
| 24 | $9D7E-$9E36 | {6,3} TRADEWEST, INC. PRESENTS {10,8} SOLAR JETMAN™ {2,10} HUNT FOR THE GOLDEN WARPSHIP {8,13} © 1990 RARE LTD. {3,18} LICENSED TO TRADEWEST, INC. {6,20} BY RARE COIN-IT, INC. {10,23} LICENSED BY {4,25} NINTENDO OF AMERICA INC. | licence screen at cold power-on |
| 25 | $A035-$A07F | {8,8} STORING CRYSTAL {11,13} VALUE 500 {TYPE} {4,16} MORE CRYSTALS NEEDED FOR {10,17} A BONUS LIFE | a crystal delivered (two blanks between STORING and CRYSTAL) |
| 26 | address $0000 | (null pointer, never printed) | unused |
| 27 | $A118-$A12B | {13,2} ANALYSIS {10,18} VALUE | analysis screen frame |
| 28 | $A12C-$A145 | {7,10} CARBON {7,12} OXYGEN {7,14} ZINC | analysis list 0 |
| 29 | $A146-$A162 | {7,10} LEAD {7,12} SILICON {7,14} TARROGON | analysis list 1 |
| 30 | $A163-$A17E | {7,10} OXYGEN {7,12} HYDROGEN {7,14} IRON | analysis list 2 |
| 31 | $A17F-$A19A | {7,10} PINEAPPLE {7,12} WOOD {7,14} MOUSE | analysis list 3 |
| 32 | $A19B-$A1B9 | {7,10} CALIFORNIUM {7,12} GOLD {7,14} NICKLE | analysis list 4 |
| 33 | $A1BA-$A1DA | {7,10} ALUMINIUM {7,12} LITHIUM {7,14} KRYPTON | analysis list 5 |
| 34 | $A1DB-$A1F4 | {7,10} XENON {7,12} ZIPPON {7,14} FLUON | analysis list 6 |
| 35 | $A1F5-$A217 | {7,10} UNOBTANIUM {7,12} LINOLIUM {7,14} LONGONE | analysis list 7 |
| 36 | $A218-$A234 | {7,10} WATER {7,12} SUGAR {7,14} COLOURING | analysis list 8 |
| 37 | $A235-$A250 | {7,10} STONE {7,12} PAPER {7,14} SCISSORS | analysis list 9 |
| 38 | $A251-$A26E | {7,10} PRAWNS {7,12} COCKLES {7,14} MUSSLES | analysis list 10 |
| 39 | $A26F-$A290 | {7,10} CHEESE {7,12} STODGE {7,14} FRENCH FRIES | analysis list 11 |
| 40 | $A291-$A2AC | {7,10} RUBBER {7,12} MOXXY {7,14} VOOFORS | analysis list 12 |
| 41 | $A2AD-$A2D3 | {7,10} DETERMINON {7,12} PHENOMENON {7,14} EXISTENON | analysis list 13 |
| 42 | $A2D4-$A2EE | {7,10} HEALTHY {7,12} BONES {7,14} TEETH | analysis list 14 |
| 43 | $A2EF-$A308 | {7,10} NEW {7,12} BIOLOGICAL {7,14} OMO | analysis list 15 |
| 44 | $A309-$A325 | {7,10} PLASTIC {7,12} GLASS {7,14} PEBBLES | analysis list 16 |
| 45 | $A326-$A354 | {9,11} FANCY ALIEN ITEMS {13,14} VERY RARE {12,17} VALUE 1000 | an alien item delivered (two blanks between VALUE and 1000) |
| 46 | $A355-$A39A | {11,8} RADIOACTIVES {12,11} URANIUM 235 {12,13} PLEBIUM 232 {12,15} QUORUM 220 {13,17} VALUE 750 | radioactives delivered (two blanks between QUORUM and 220) |
| 47 | $A39B-$A3CE | {TYPE} {5,15} YOU HAVE FOUND A PIECE {5,17} OF THE GOLDEN WARPSHIP | warpship piece delivered |
| 48 | $A3CF-$A40A | {TYPE} {7,14} THE GOLDEN WARPSHIP {9,15} IS NOW COMPLETE {9,17} DESTINY SUMMONS | last warpship piece delivered |
| 49 | $A40B-$A465 | [picture mode: a logo, rows 0-6, columns 1-14, plus one attribute byte at row 30 column 10] | interplanetary flight screen |
| 50 | $A466-$A4AF | {TYPE} {4,8} YOU HAVE ENOUGH CRYSTALS {12,13} YOU GAIN {10,15} A BONUS LIFE {8,17} AND 2000 CREDITS | crystal bonus |
| 51 | $A4B0-$A4FB | {7,2} APPROACHING PLANET {3,15} GRAVITY {17,15} DIAMETER {5,18} PASSWORD {6,21} PRESS START TO LAND | approach screen |

**Notes.**

- **Shared entries:** ids 0, 1 and 3 point at the same data, as do 6 and 7.
- **Analysis lists:** ids 28-44 are picked as 28 + k, where k = (an object parameter + level index) mod 17. The
  same k selects the cargo name (4.4), the block of equipment-picture tiles and the palette pair of the analysis screen
  (`graphics.md` 5.8, 7.4). **[V]** (k = 4 on level 0 with parameter 3 in Mesen; the rule itself by code.)
- **Fuel messages 21 and 22 (item kind 10):** with c the mothership's fuel count $0513 as the screen starts (`economy.md`
  3.4), message 22 is shown when c + 1 is 25 or more (c = 24 or 25), message 21 otherwise; c becomes c + 1 when c + 1 is
  25 or less (24 to 25; c = 25 stays). **[V]** (forced with c = 23, 24 and 25 on level 0: 21; 22 and c = 25; 22 and
  c = 25.)
- **Warpship messages 47 and 48** come with a picture (`scenes.md` 10, item 4).
- **Picture runs (ripped):**
  - id 0: picture mode, the tiles (with their position bytes) before the first text position, ROM
    0x09CDB-0x09D00;
  - ids 6-9: the tile codes ($81 and up) before {TYPE};
  - id 49: all of it.

  An importer copies these runs from the ROM, as with any other picture.

**Message call sites (bank 0) [C]:**

| Message(s) | Called from |
|---|---|
| 24 | $807D (licence) |
| 4 | $86A5 (password) |
| 49 | $888B (flight) |
| 3, 2 | $8DE4, $8DE9 (shop/equip) |
| 5 | $94B0 (last password) |
| 7, 8, 45, 9 | $9B7E, $9B95, $9BA6, $9BBC |
| 27, then 28 + k | $9BEA, $9BF2 |
| 21 or 22 | $9C6A |
| 50 | $9C9A |
| 25 | $9CB6 |
| 46 | $9CC7 |
| 47 or 48 | $9CF1 |
| 6 | $9D00 |
| 23 | $AC3B (end of the final level) |
| 51 | bank 6 $86C9 (approach) |

**Message screen kind 6** (`game-flow.md` 7.4, the warp-level introduction shown when the level-change gate ends on
a level 13 or above with the planet's flag set) **calls no printer message [V]**. Its words (a title word, CYBERZONE, and five
short lines: COLLECT, 10, GEMS, BEFORE TIME, CATCHES UP, **[V]** by screenshot 2026-10-17) are 30 sprites with the tiles $00-$1D
loaded from ROM 0x3512D (`graphics.md` 4.5, `scenes.md` 10.1), not background and not printer strings, so they are
ripped data like the other pictures in this section. Printer id 6 above (the shields message, called at $9D00) is unrelated to message kind 6:
kind and printer id are different numbers.

## 4. Other strings (not in the printer table)

### 4.1 Title screen **[V]**

Bank 0 $F821-$F84C (ROM 0x07821):

- "START GAME" (10 bytes), at nametable row 18, column 12;
- "CONTINUE" (8 bytes), row 20, column 13;
- "© COPYRIGHT 1990 RARE LTD." (26 bytes: $3A, $20, text, $5D), row 25, column 4.

Tile $20 is blank on the title screen, so the copyright line's $20 shows as a space. **[V]** (CHR RAM dumped at
the title)

The pointers, lengths and nametable positions (low bytes) of START GAME and CONTINUE are at bank 0
$F7DD-$F7E4; the copyright line is not in that table. The title scrolls vertically while it builds, so
screen rows differ from nametable rows. START GAME and CONTINUE are written one at a time in title sub-state 3
(`game-flow.md` 7.1). The menu box around them is drawn with tiles $10-$15 of pattern table 1 (corners $10, $11,
$12, $13, bar $14, side $15) in 26 single-frame writes, in an order given in `scenes.md` 2.3 **[V]**; it is not
the planet menu box of 4.6.

### 4.2 Status panel: system name **[V]**

Bank 0 $97EA-$980E (ROM 0x017EA):

Written by the status-panel code (bank 0 $980F), not by the printer, into the HUD's nametable page (the page
selected with the mapper's page bit, `levels.md` 5.1), at that page's rows 4 and 6. The same rows are the panel's
rows on the approach screen, where the system name is shown too (`scenes.md` 7). **[C]**

- "SYSTEM" followed by a blank (7 bytes), at row 4, column 1;
- then the system number as a digit ($31 + system) and a blank;
- the five system names, 6 characters each, from $97F1: ZANDOR, FROBOF, MUGRIM, SHOBAS, XEXORM. Name number
  `s` (0-4) is at $97F1 + 6s, printed at row 6, column 2.

### 4.3 Planet names **[V]**

Bank 6 $D2FD-$D393 (ROM 0x352FD): 14 entries, one after another. Each is the planet number in digits, a $00
blank, and the name ($00 between words); the **last character has bit 7 set**.

| | | | | |
|---|---|---|---|---|
| 1 PRELUDON | 2 MEXOMORF | 3 OMEBRU | 4 CORSO QWERO | 5 BOKKY |
| 6 LEMONTE | 7 CHORLTON | 8 SHISHKEBAB | 9 ZLAZ TORDUS | 10 SHAMMY GEN |
| 11 SHANKOO | 12 MIPLEZUR | 13 UROWND | 14 DRYN KUP LEEZ | |

- **Lookup:** the level index L (RAM $3B) selects entry L, found by skipping L terminated entries.
- **Centring:** the entry is printed on row 5, starting at column 15 − ((length − 1) / 2, rounded down), with
  bit 7 cleared on output. The length counts every byte of the entry: the number, its blank and the name. For
  example, "1 PRELUDON" (10 bytes) starts at column 15 − 4 = 11. **[V]** (screenshot)
- **Numbers on the approach screen:** gravity (RAM $6F) at row 15, column 11, and diameter (RAM $37) at row 15,
  column 26.
  - Signed decimal: a "-" ($5C) for negative values, then "1" when the magnitude is 100 or more, then the tens
    digit (left out when zero and there is no hundreds digit), then the units digit.
  - Examples on screen: "GRAVITY 8", "GRAVITY -24", "DIAMETER 47".

### 4.4 Cargo names **[V]** data, **[C]** use

Bank 0 $9F22 (ROM 0x01F22): 18 offsets, then from $9F34 the names. The bytes are packed with $00 as the space
between words; name `k` runs from $9F34 + offset[k] to $9F34 + offset[k+1].

| k | Name | k | Name |
|---|---|---|---|
| 0 | TENNIS BALLS | 9 | GEOLOGICAL SAMPLES |
| 1 | HAMBURGERS | 10 | ROBOT KIT |
| 2 | JELLYBEANS | 11 | ENGLISH PHONE BOX |
| 3 | EMPTY CONTAINERS | 12 | COMPUTER GEAR |
| 4 | ABSOLUTELY NOTHING | 13 | EASTER ISLAND HEADS |
| 5 | TRIBBLES | 14 | SOME GRASS |
| 6 | BANANAS | 15 | MICROWAVE OVENS |
| 7 | SPEAKERS | 16 | A PILE OF GEMS |
| 8 | SOFT DRINKS | | |

On the analysis screen (message screen kind 3, `game-flow.md` 7.4) **[V]** for the digits, **[C]** for the rest:

- **Digits:** next to each of the three element names (rows 10, 12 and 14, section 3) three digits flicker, at
  columns 24-26. Each digit is taken from the 16-character table at bank 0 $9F12, "0143865498728380", indexed by
  the low four bits of a random generator byte ($5A, $5B and $5C for the three digits; `game-flow.md` 5).
  - They are written again on every frame (new random bytes each frame) while $5D is below 96.
  - **Source of the random bytes [V]:** the program's one global generator (`game-flow.md` 5): $5A is its output byte,
    $5B and $5C the high and low bytes of its shift register, as the pass's generator step leaves them (the step is
    in the status-bar split, before this routine runs in the same pass). Nothing reseeds it for this screen: it simply
    goes on from the state gameplay left, one step per pass, so the digits are as random as the gameplay's. They reach
    the screen one frame after the pass that computed them (the video buffer is flushed at the start of the next pass).
    230 of 230 digit cells read back in Mesen equal the table entry of the previous frame's final $5A, $5B and $5C
    (column 24 from $5A, column 25 from $5B, column 26 from $5C). The remake may use any generator; if it
    follows `game-flow.md` 5 the digits stay faithful.
  - Row 10's digits change while $5D is 0-31, row 12's while 32-63, row 14's while 64-95 (the PPU address low
    bytes are $58, $98, $D8 of page $2100, i.e. rows 10, 12, 14, column 24).
  - Rows 12 and 14 have no digits before their windows (the spaces stay blank); each row then keeps its last
    digits.
- **Value:** shown when the message timer $DA reaches 64, as the decimal digits of the cargo's value (table in
  `economy.md` 2.4) with leading zeros left out, starting at row 18, column 16, after "VALUE" (row 18, column 10)
  and one blank. **[C]**
- **Cargo name:** shown when the message timer $DA reaches 128. It is right-aligned: its last character is at row
  16, column 26.

### 4.5 Equipment names (shop and equip list) **[V]** data

Bank 1 $959A-$96A6 (ROM 0x0959A): 16 names packed one after another, $00 as the space. Their lengths are at bank
1 $96A7-$96B6 (16 bytes). Number `n` is also the index of the ownership count RAM $0600 + n.

| n | Name | n | Name |
|---|---|---|---|
| 0 | HOMING MISSILES | 8 | SUPER SHIELDS |
| 1 | ANTI GRAVITY | 9 | MOMENTUM KILLER |
| 2 | SMART BOMBS | 10 | EFFICIENT ENGINES |
| 3 | TIME BOMBS | 11 | DOUBLE STRENGTH THRUSTERS |
| 4 | STAR BULLETS | 12 | MAPPING DEVICE |
| 5 | MULTI WARHEAD MISSILES | 13 | SUPER MAPPING DEVICE |
| 6 | TITANIUM BULLET PACK | 14 | NIPPON SPORTS JETPOD |
| 7 | MILITARY BULLET SYSTEM | 15 | ITALIAN RACING JETPOD |

- **Name shown:** item n shows name n, except that items 12 and 13 show names 14 and 15 on levels where ($3B AND
  3) = 3. **[C]**
- **Position in the shop and equip lists:** the name of the selected item is centred in a 32-tile row starting
  at column (32 − length) / 2, rounded down, on nametable row 15. Only columns 0-27 of that row are written, which
  the longest name fits. **[C]**, row **[V]** (screenshot)
- The price, the ownership count and the rest of those screens are scene layout: `scenes.md`.

### 4.6 Planet menu box **[V]** data, **[C]** drawing

**Row records.** Bank 2 $822C-$82A3 (ROM 0x1022C-0x102A3): 10 records of 12 bytes. Each is 11 tiles of the box
and a twelfth byte, always $20 (a blank column).

| Record | Content | Record | Content |
|---|---|---|---|
| 0 | top edge | 5 | `6  EQUIP  6` |
| 1 | `6         6` (empty row) | 6 | `6   POD   6` |
| 2 | bottom edge | 7 | `6  BLAST  6` |
| 3 | 12 blanks | 8 | `6   OFF   6` |
| 4 | `6 EXPLORE 6` | 9 | `6   BUY   6` |

Characters `1`-`6` are frame tiles: `1` top-left, `2` top-right, `3` bottom-left, `4` bottom-right, `5`
horizontal, `6` vertical.

**Layouts.** Each layout (RAM $A1) is a list of record numbers. The lists are at bank 2 $82A4, $82AD, $82B9 and
$82BF (pointers at $821C low / $8220 high). Their row counts are at $8224 (9, 12, 6, 9) and the number of options
at $8228 (2, 3, 1, 2): the cursor ranges from 1 to that number.

| $A1 | Records, top to bottom | Options |
|---|---|---|
| 0 | 0 1 4 1 5 6 1 2 3 | EXPLORE, EQUIP POD |
| 1 | 0 1 4 1 5 6 1 7 8 1 2 3 | EXPLORE, EQUIP POD, BLAST OFF |
| 2 | 0 1 4 1 2 3 | EXPLORE |
| 3 | 0 1 4 1 7 8 1 2 3 | EXPLORE, BLAST OFF |

Record 9 (BUY) is in no list, so it is never shown. The last row of every layout is record 3: it blanks the row
under the box.

**Drawing.** The printer is not used. The surface scene redraws the box one row per frame, cycling through the
layout's rows. Row r is written to nametable row 4 + r, columns 10-21. Tile codes are converted with that
screen's own tile base in RAM $A0: $20 becomes the blank tile 0; a letter becomes code + $A0 − $3F; a frame
character `1`-`6` becomes code + $2F + $A0 − $3F. This points them at the menu font loaded for the surface
picture. The surface scene sets RAM $A0 = $CF each time it is built, so a letter's tile is its code + $90 (`A` =
$D1) and a frame character's is code + $BF. **[C]** The cursor is two sprites (tile $A7, palette 0) at x = 90 and
x = 154, with sprite-table y = $31, $41 and $59 for option 1, 2 and 3 (the y of the option's text row; option 2 of
layout 3 is BLAST OFF and uses the same y as EQUIP POD). Up and Down (newly pressed) move it, stopping at the
first and last option. **[C]** On first appearance the box builds up from its top row, one row per frame, and
then goes on redrawing the rows in turn (9, 12, 6 or 9 frames per cycle). Each drawn row also rewrites attribute
bytes of the box's rows (a palette change for the row pair; not analysed further). **[C]** Positions on screen:
`scenes.md`.

### 4.7 Credits **[V]**

Bank 0 $D1A1-$D2E1 (ROM 0x051A1-0x052E1, the last byte being the $80 end). The credits scene draws one 32-tile row every 8 pixels of scroll (the scroll
moves 1 pixel every second frame, so one row every 16 frames), at the bottom of the screen; the layout, scroll and
timing are in `scenes.md` 14.3. Played in Mesen (reached by forcing the end of the final level) and read back: the
text, the rows and the columns below are what the screen showed.

- **Text:** a run of bytes below $80 is one line, centred: it starts at column (32 − length) / 2, rounded down, the
  rest of the 32-tile row is blank (tile 0). The character codes are the tile numbers (ASCII).
- **Gap:** a byte $81-$FF is a gap of 257 − byte blank rows (so $FF gives 2, $FE 3, $FC 5, $DA 39). A gap may
  come first: the data starts with $FE.
- **End:** a byte $80 ends the credits and restarts the program (warm reset).
- **Start:** the scene begins with 8 blank rows of its own, then the data's first gap (3 rows), so 11 blank rows
  come before "SOLAR JETMAN CREDITS" (row L = 11). Rows are counted L = 0, 1, ...: 141 rows come before the end
  marker.
- Spaces are $20 here and are written as they are. Tile $20 is blank on that screen, which loads only the font
  and the badge (`graphics.md` 5.1, `scenes.md` 14.3).

The credits as data (every line, its row L, its first column, and the blank rows after it; two spaces between
words where the data has them, which also shifts the centring):

| Row L | Column | Text (exact, double spaces kept) | Blank rows after |
|---|---|---|---|
| 11 | 6 | `SOLAR JETMAN CREDITS` | 5 |
| 17 | 11 | `CONCEPT BY` | 2 |
| 20 | 14 | `RARE` | 5 |
| 26 | 10 | `SOFTWARE  BY` | 2 |
| 29 | 10 | `STEVE HUGHES` | 5 |
| 35 | 10 | `GRAPHICS  BY` | 2 |
| 38 | 9 | `LYNDON  BROOKE` | 2 |
| 41 | 10 | `STE PICKFORD` | 5 |
| 47 | 8 | `SOUND EFFECTS BY` | 2 |
| 50 | 8 | `MARK  BETTERIDGE` | 5 |
| 56 | 12 | `MUSIC BY` | 2 |
| 59 | 11 | `DAVE  WISE` | 5 |
| 65 | 9 | `CRITIQUING BY` | 2 |
| 68 | 10 | `SIMON FARMER` | 2 |
| 71 | 10 | `GREGG MAYLES` | 2 |
| 74 | 8 | `STEPHEN STAMPER` | 2 |
| 77 | 12 | `HUW WARD` | 5 |
| 83 | 5 | `ADDITIONAL MATERIAL BY` | 2 |
| 86 | 9 | `JOHN  PICKFORD` | 2 |
| 89 | 10 | `TIM  STAMPER` | 5 |
| 95 | 6 | `PRODUCED BY RARE LTD` | 5 |
| 101 | 4 | `A TRADE WEST PRODUCTION` | 39 |

(Wording as in the data: "A TRADE WEST PRODUCTION", "PRODUCED BY RARE LTD".)

### 4.8 Password letters

Bank 1 $98E1 (ROM 0x098E1): "BDGHKLMNPQRTVWXZ", the 16 letters of the password (`password.md`).

- The password entry screen shows the 12 letters from RAM $06EE-$06F9 at row 12, column 10. Its cursor is
  described in `password.md` 9.
- The approach screen writes the 12 letters after "PASSWORD" (message 51: row 18, column 5) on row 18 from column
  14, leaving column 13 blank. **[V]** (nametable dumped)

**[V]** (screenshots)

## Open questions

- The exact on-screen placement of the approach screen's planet number and of the HUD digits (score, lives
  icons). The score is drawn from tiles of the HUD digit set, not this font (`graphics.md` 4.1).
- (closed 2026-10-17: the credits were played in Mesen, `scenes.md` 14.3.)
