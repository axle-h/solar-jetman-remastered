# Objects: placement, sprite layouts, animation and the object table

What the original stores about the things that move or sit on a planet besides the terrain: where each level
places its objects, which object types exist, how an object's picture is composed of 8×8 sprites, how frames
change over time, and the RAM table that holds every live object. Behaviour (AI, physics, damage rules) is out of
scope except where a field needs naming; open behaviour questions are listed at the end.

Category (`rom-map.md`): placement, the type tables, sprite layouts, frame ranges and the collision-size tables
are **commit** data. Tile pixels stay **ripped** (`graphics.md`).

Status: **[V]** verified in Mesen (RAM/OAM of the running original matched our decode), **[C]** read from the
program, **[H]** hypothesis. Addresses as in `README.md`.

## 1. Coordinates

- World coordinates are unsigned 16-bit pixels, x to the right, y downwards. An object's position is one point;
  its sprites are placed relative to it (section 6). **[V]**
- The camera (RAM $4C/$4D = x low/high, $4E/$4F = y low/high) is the world position of the screen's top-left
  pixel. Screen position = world position − camera (16-bit, wrapping). **[V]**
- The visible playfield is described in `levels.md` 5.1 (lines 0-192). The 224-line bound in 3.4 is a
  constant of the spawn rule, not the playfield height.

## 2. The object table (RAM)

### 2.1 Layout

31 slots, numbered 0-30. Each field is an array of 31 bytes: field `F` of slot `s` is at `F + s`. **[V]**

| Address | Field | Meaning | Status |
|---|---|---|---|
| $0200 | x low | world x, low byte | [V] |
| $021F | x high | world x, high byte | [V] |
| $023E | y sub-pixel | fraction of y (1/256 px) | [V] for the pod |
| $0197 | x sub-pixel | fraction of x (1/256 px); this array lies below the others, at $0197-$01B5 | [V] for the pod |
| $025D | y low | world y, low byte | [V] |
| $027C | y high | world y, high byte | [V] |
| $029B | work 1 | pod: horizontal speed, fraction byte; some placed types: copy of the spawn x low (home) | [V] pod, [C] home |
| $02BA | work 2 | pod: horizontal speed, whole pixels (speed = work2 + work1/256); home x high | [V] pod, [C] home |
| $02D9 | work 3 | pod: vertical speed fraction; home y low | [V] pod, [C] home |
| $02F8 | work 4 | pod: vertical speed whole pixels; home y high | [V] pod, [C] home |
| $0317 | state | 0 = slot free. Bit 7 set = the object came from the level's placement list (section 3); the value selects the behaviour (section 2.4) | [V] |
| $0336 | flags X | bit 0: moving left (speeds are magnitudes; direction is in this bit). Also selects mirroring for drawing (6.3) | [V] |
| $0355 | flags Y | bit 0: moving down. Bit 7: draw upside down (6.3) | [V] |
| $0374 | frame | sprite layout number (section 6); $FF = invisible | [V] |
| $0393 | list index | for placed objects (state bit 7): position of the object in its level list (0-based). Other objects use it as a work byte | [V] |
| $03B2 | type | for placed objects: the object type (section 4). Other objects: a sub-type or work byte | [V] |
| $0403 | work 5 | behaviour-specific (counters, a waypoint toggle); cleared at spawn | [C] |
| $0422 | work 6 | behaviour-specific timer; cleared at spawn. Non-zero keeps a towed object alive off-screen (3.5) | [C] |
| $0441 | hit points | decremented by damage; when it reaches 0 the object is destroyed (3.5). 255 in practice means indestructible | [V] |

Other RAM used with the table:

| Address | Meaning | Status |
|---|---|---|
| $3B | level index 0-41 | [V] |
| $66 | slot number of the object currently held by the pod's tractor beam (0 = none) | [C] |
| $0460-$04B4 | working copy of the current level's type list (3.2); an entry is set to 0 when that placed object is destroyed or collected, so it never spawns again in this visit | [V] copy, [C] clearing |
| $BC | number of objects in the current level's list | [V] |
| $BD | placement cursor: list index the placement scan examines next | [C] |
| $BE/$BF | address of the current placement record (bank 4) | [V] |
| $C0-$C3 | placement-scan state: "records still carry a count byte" flag / remaining group count / position in group / offset within the record | [C] |
| $050C | spawn margin `M` (level header byte 10); $050D = 256 − M | [V] |
| $05CF-$05EE | frame ranges and tile bases for slots 15-30 (5.2) | [V] |
| $05EF-$05FE | frame ranges and tile bases for slots 1-14 (5.2) | [V] |
| $0700-$07FF | sprite buffer copied to OAM each frame | [V] |

### 2.2 Slot ranges

The program searches different slot ranges for different kinds of object. **[C]** from the searches,
**[V]** for the uses seen in Mesen.

| Slots | Used for |
|---|---|
| 0 | the jetpod (the player); also the pilot outside the pod and the pod's explosion |
| 1-2 | attached to the pod: slot 1 carries the exhaust flame and small effects drawn behind the background when the pod is; **slot 2** the tractor beam (state 9, `player.md` 8.4: two sprites at the rope's midpoint, tile $CF rewritten every frame) or the shield ring (state $0C, `player.md` 4.7a: four sprites around the pod) |
| 3-10 | the pod's shots (up to 8) and impact sparks, sharing these slots; specified in `player.md` |
| 11-14 | the **items** of the level header's items list (section 10; `economy.md` 2.1) |
| 15-22 | objects created by other objects: enemy shots, released items, debris |
| 23-30 | objects from the level's placement list (3.4); a few behaviours also create helpers in 23-25 or 26-30 |

Slots 1-14 draw with layout table B, slots 15-30 with table A, slot 0 with table P (section 6). **[V]**

**Slot reuse leaves fields behind.** Freeing a slot only sets its state to 0. A creator writes just the fields it
needs, so the others keep what the previous occupant left, and the sub-pixel position fractions (RAM $0197 and
$023E + slot) carry over into the new object's motion. [C], consistent with the black-box fit of the objects
implementer. Known cases:

| Creator | Writes | Does not write (keeps old value) |
|---|---|---|
| Impact sparks (`player.md` 5.5) | position (whole), state, frame, the four speed bytes, both direction bits, life | hit points, both fractions, work bytes, flags beyond the direction bits |
| The pod's and Jetman's shots (`player.md` 10) | position, state, frame, directions, speeds, hit points = RAM $050E + 1, life, mode byte $03B2 = 0 | both fractions |
| Enemy shots (turrets, orbs, drones, ...; `behaviour.md`) | position, state, frame, directions, speeds, life | hit points, fractions (the hit-point byte is pulled down to the state's table maximum at the start of the object's step, so an over-large stale value is capped) |
| Crystal pick-up sparks (`economy.md` 5.4) | as the impact sparks, but state 2, life $20 and fixed directions | as the impact sparks **[C]** (not poked in the sentinel test; the creation itself and the motion were checked) |
| Warp-level countdown (`behaviour.md` 6.3) | position x and y (both bytes), state $0D, frame, flags X = 0, its counter in the horizontal speed bytes | both fractions, flags Y, list index ($0393), type ($03B2), work 5, work 6, hit points **[V]**: they keep the last occupant of slot 3 |
| Placement scan, placed helpers that call the clear routine (bank 7 $B846 / bank 0 $D3ED) | everything above, all zero first: directions, mode, both fractions, life, four speed bytes | |

**What a level change clears [V]** (round 13, Mesen: sentinel values poked into every free slot's 18 arrays, then a
wormhole gate from level 13 to the planet; the same pokes made while a warp level loaded): the level load sets the four
speed/work arrays (RAM $029B, $02BA, $02D9, $02F8) of **all 31 slots, slot 0 included,** to 0 (at the load, before the
first logic step), and every state byte of slots 1-30 to 0. **Nothing else is cleared:** the position bytes (whole and
fraction), both flags bytes, frame, list index ($0393), type ($03B2), work 5, work 6 and hit points of a free slot all
keep what they held before the change. So a stale byte survives into the new level and into the first object that is
made in that slot without writing it. The implementer's "keep all" is right except for those four arrays, which are 0.
The countdown's slot (3) is the clearest case: its fields not listed above carry the last occupant's values (a pod
shot or spark of the old level); the countdown's own counter starts at 0 because the speed arrays are cleared.

### 2.3 Field use by the drawing code

Only `x`, `y`, `frame`, `flags X`, `flags Y` and the slot number decide what is drawn (section 6). A slot whose
state is 0 is not drawn and is free. **[V]**

### 2.4 States

The state value is a behaviour selector. For placed objects it is set from the type (4.1) and stays fixed for
most types; its low 7 bits index two per-state tables in bank 7:

- Initial hit points: one 93-byte table at bank 7:$AB07 (ROM 0x3AB07). For a state s below $80 the entry is at
  $AB07 + s; for a state s of $80 or more it is at $AB29 + (s − $80), i.e. $AB07 + (s − $5E). The program reads
  the low states at $AB07 + s without a limit, so states $22-$25 take the bytes at $AB29-$AB2C, which are also the
  entries of states $80-$83: hit points 0, 1, 0, 0 for states $22, $23, $24, $25. Placed objects, and the helper
  objects that use the same initialisation, start with this many hit points (items do not: their spawn leaves
  the hit-point byte as it was).
  **[V]** (every surveyed placed type started with exactly its table value), **[C]** for $22-$25
- Handler address tables in bank 7 ($AA4E-$AB06): code pointers, not data.

**State bit 7** means "this slot is an instance of entry $0393 of the level's placement list". **[C]**

- It is set only when the placement scan spawns an object (every initial state of 4.1 is $86-$BA), and when one
  behaviour moves a placed object into another slot whole (state, type and list index copied; that behaviour also
  sets the object's type-list copy entry to 0, so the moved object is never spawned a second time).
- It is cleared whenever the state is replaced by a value below $80: destruction (explosion states 4 and 5, 2.5),
  pick-up, and freeing (state 0). No behaviour sets one placed state to another placed state.
- Only bit-7 slots count for the "already live" test of the placement scan (3.4) and for clearing the type-list
  copy on destruction (3.5). Objects that behaviours create in slots 23-30 (states $19 and $23, for example) have
  bit 7 clear; they use $0393 as a timer, so they never collide with list indices.
- Item kind 2 would give state $82 (bit 7 set in slots 11-14) if it were spawned from a list, but no items list
  contains it.

States below $80 seen in play (**[V]** seen, meaning **[H]**):

| State | Slots | Frames seen | Probably |
|---|---|---|---|
| $01, $02, $04, $05, $07 | 0 | 0-31 in $01; $28-$2A in $02; $FF and the pod explosion frames in $04/$05; 0 in $07 | the player: meanings in `game-flow.md` 9.1 |
| $01, $07, $08 | 3-10 | B$37, B$62, B$63 | shots and sparks (`player.md`) |
| $04, $05 | 5-6, 15, 23 | A$00-$0F, B$00-$03 | debris of a destroyed object |
| $07 | 11 | B$8C | mothership |
| $0A | 11-14 | B$52-B$66 | an item (FUEL sign B$5C, gems B$54/B$65/B$66, radioactive B$61, box B$5E, bottle B$60) |
| $0B | 3 | B$69-B$6C | pick-up sparkle ring |
| $15 | 15-18 | A$33, A$50, A$84, A$88 | small enemy shots |
| $1A | 15-19 | A$52-A$54 | flame / rising bar shot |
| $22 | 15-22 | A$89-A$8C | small orange shots |
| $23 | 23 | A$B9-A$BB | green orb |
| $24, $25 | 11 | B$55, B$64 | object released when certain enemies are hit |

### 2.5 Explosions **[C]**, timing **[V]**

When an object's hit points reach 0 it becomes an explosion in the same slot:

| Object | Explosion | Sequence position (work byte $03B2) | Frames shown |
|---|---|---|---|
| slots 15-30, half-width + half-height < 8 (section 8) | state 4 | 0 | 0, 1, 2, 3 |
| slots 15-30, sum 8-27 | state 5 | 5 | 4, 5, 6, 7, 8 |
| slots 15-30, sum ≥ 28; any object in slots 1-14 | state 5 | 11 | 9, 10, 11, 12, 13, 14, 15 |

- The frames come from the 19-byte sequence table at bank 7:$8297 (ROM 0x38297): `00 01 02 03 FF 04 05 06 07 08 FF
  09 0A 0B 0C 0D 0E 0F FF`; $FF ends a sequence. They are layouts $00-$0F of table A (slots 15-30) or table B
  (slots 1-14), which are the same layouts.
- At destruction $0393 is set to 1. On every logic step (A pass) $0393 is decreased; when it reaches 0 it is reset
  to 4, the next table byte is read and $03B2 advanced: a frame number becomes the frame, $FF frees the slot. So
  the first frame appears on the first step after destruction and every frame lasts 4 logic steps (8 video
  frames); the slot is freed 4 steps after the last frame appeared. Small: 17 steps from destruction to free,
  medium 21, large 29.
- If the object was a placed object, its type-list copy entry is set to 0 at destruction (3.5).
- **Points for a kill.** Destroying an object in slots 23-30 adds points read from the 4-byte BCD table at bank
  1:$89B8 (`03 05 08 11`) at position (new state − 4). The new state is the explosion state, so the amount depends
  only on the explosion size, not on the enemy: **3 points** for a small explosion (state 4), **5 points** for a
  medium or large one (state 5); the last two table bytes are never used. Kills in slots 1-22 add nothing by this
  rule. **[C]**, **[V]** in Mesen: random play on levels 4, 6, 9 and 11 gave 11 kills of +5 and one of +3 (score
  RAM $CB-$D0 checked before and after).
- Measured: large explosions showed 9, 10, 11, ... 15 for 8 video frames each; small ones 0-3 for 8 frames each.

## 3. Placement data (bank 4)

This is not the 5-byte-per-object list in bank 2 that the level header points to (`levels.md` 3.3, copied to
RAM $0513-$0594). That second list is a separate set of objects with its own type numbering and is not covered
by this spec yet (Open questions).

### 3.1 Tables

Per level (index `L` = 0-41), bank 4 holds five 42-byte tables: **[V]**

| Bank 4 | ROM | Content |
|---|---|---|
| $80F3 + L | 0x200F3 | record list address, low byte |
| $811D + L | 0x2011D | record list address, high byte |
| $8147 + L | 0x20147 | `n` = number of objects |
| $8171 + L | 0x20171 | type list address, low byte |
| $819B + L | 0x2019B | type list address, high byte |

The type list is `n` bytes: the type of object 0, 1, ..., n − 1. All records and type lists lie in bank 4
$8417-$94C6 (ROM 0x20417-0x214C6), back to back with no gaps; levels 29-41 share one list. **[V]**

### 3.2 Record list format

The record list gives the positions of objects 0 ... n − 1 in order. It consists of entries of two kinds:

- **Group entry** (only while "counted mode" is on, which it is at the start of every list):
  `x low, x high, y low, y high, k`, then `k` pairs `dx, dy` (signed bytes). It places `k + 1` objects:
  the first at (x, y), then one at (x + dx, y + dy) for each pair, in order (16-bit wrapping addition of the
  sign-extended offset). If `k` = 0 the entry is just the 5 bytes and places one object, **and counted mode
  switches off for the rest of the list**.
- **Plain entry** (after counted mode is off): `x low, x high, y low, y high`, one object.

The shape of a record list, read until `n` objects have been placed:

```
list        = group* [ last-group plain* ]
group       = x y k (dx dy) × k      k ≥ 1: places k + 1 objects
last-group  = x y 0                  places one object; no groups follow
plain       = x y                    places one object
x, y: 16-bit, low byte first.  k: unsigned byte.  dx, dy: signed bytes.
```

Objects are numbered 0 ... n − 1 in the order they are placed; object `i` has the type at type-list address +
`i`. Every list in the ROM ends exactly when `n` objects have been placed (never inside a group). **[V]** (decoded lists matched the
positions and types of the placed objects found in RAM for every object surveyed, section 9)

Worked example, level 0 (records $8417, types $843C, n = 9):

| Bytes | Objects |
|---|---|
| `72 03 AF 01 00` | #0 at ($0372, $01AF), k = 0: counted mode off |
| `22 01 A7 01` | #1 at ($0122, $01A7) |
| `AF 00 A5 02` | #2 at ($00AF, $02A5) |
| `3F 01 F0 02` ... `D4 03 6E 01` | #3 ... #8 |

Types `5F 0B 0F 0C 06 08 49 05 0E`. Level 1 starts with a group: `2F 0B F6 03 03 55 9C 32 1A C1 18` = #0 at
($0B2F, $03F6), #1 at +85/−100, #2 at +50/+26, #3 at −63/+24.

### 3.3 Per-level summary

| L | Records | Types | n | Types used (hex, ×count) |
|---|---|---|---|---|
| 0 | $8417 | $843C | 9 | 05, 06, 08, 0B, 0C, 0E, 0F, 49, 5F |
| 1 | $8915 | $8A2B | 79 | 01×6, 03×5, 04×2, 05, 07×4, 08, 09, 0A×2, 0B×3, 0C×5, 0D, 0F×3, 10×3, 15×2, 16×3, 1F×15, 20×9, 4D×5, 4E×8 |
| 2 | $8BE4 | $8C41 | 27 | 21×2, 33, 34, 37, 38, 3B, 3C, 3F, 40, 43×5, 44, 45, 46, 4B×5, 4C×4 |
| 3 | $8450 | $8538 | 71 | 01×6, 02×5, 03×11, 04×8, 05×5, 06×3, 07×5, 08, 09×5, 0A×3, 1F×4, 20, 21×3, 4D×5, 4E×5, 5F |
| 4 | $86EC | $877F | 38 | 11, 19×3, 1B×3, 1C×2, 1D×2, 1E×2, 22×2, 48×3, 49×10, 4B×2, 4C×8 |
| 5 | $87A5 | $88C0 | 85 | 01×2, 02×2, 05, 06×3, 07×4, 08×7, 0A, 0B×5, 0C×6, 0F×2, 10×3, 12×9, 13×3, 14×10, 18, 1F×11, 20×5, 47×7, 48×3 |
| 6 | $857F | $869E | 78 | 01×3, 02, 03×6, 04×4, 07×2, 08, 0A, 0B×3, 0C×6, 0D×3, 0E×3, 0F×4, 10×2, 11×6, 19×6, 1A×2, 1E, 1F×11, 20×3, 4B×3, 4C×7 |
| 7 | $8C5C | $8CDA | 35 | 11, 19×4, 1A×2, 1B, 1C, 1D×2, 1E×6, 22, 23, 25, 27, 29, 2B, 2D, 43×3, 44×3, 45, 46×2, 60, 61 |
| 8 | $8A7A | $8B96 | 78 | 01×5, 03, 05×2, 07×6, 0A×2, 0B×4, 0C×2, 0E×2, 0F×3, 10×4, 12×4, 17×4, 18×5, 21×4, 47, 4A×18, 4D×3, 4E×8 |
| 9 | $8CFD | $8DA6 | 49 | 12×7, 1F×9, 20×5, 48, 49×2, 4A×16, 4D×5, 4E×4 |
| 10 | $8DD7 | $8EF5 | 84 | 01×9, 03×4, 04, 05×3, 07×4, 08×2, 09×2, 0A, 0B×3, 0C, 0D×5, 0E, 10×4, 19×4, 1A, 1C×2, 1E, 1F×10, 20×6, 48×3, 49×4, 4A×11, 60, 61 |
| 11 | $8F49 | $9085 | 85 | 01×4, 02, 03×3, 04×3, 05×2, 07×4, 09, 0B×2, 0C×5, 0F×3, 10×2, 12×5, 17×2, 18, 1F×11, 20×4, 47×2, 48×6, 49×8, 4A×16 |
| 12 | $90DA | $91CC | 70 | 01×8, 02×2, 03×5, 04×6, 05×4, 07×6, 08, 09, 0A, 0C×6, 0D, 0E×2, 0F×4, 1F×6, 20×7, 22×3, 48×5, 49×2 |
| 13 | $8445 | $844E | 2 | 07, 0C |
| 14 | $926C | $9290 | 12 | 07×2, 08×3, 0B×2, 0C, 0E, 0F×3 |
| 15 | $92B0 | $92CB | 8 | 17×3, 18×3, 21×2 |
| 16 | $9212 | $9230 | 9 | 02, 09, 0A, 0E, 10, 12×4 |
| 17 | $924B | $9250 | 1 | 1E |
| 18 | $9251 | $9264 | 8 | 1F×8 |
| 19 | $9239 | $9247 | 4 | 1C×2, 1D×2 |
| 20 | $92D3 | $9316 | 20 | 01×2, 02×2, 03, 04×4, 05×3, 07, 08, 09, 0A, 0B×2, 0C, 0F |
| 21 | $929C | $92AC | 4 | 1C, 1D, 1E×2 |
| 22 | $932A | $9361 | 19 | 10×12, 1F×7 |
| 23 | $9374 | $9385 | 4 | 13, 14, 16×2 |
| 24 | $9389 | $93B0 | 11 | 19×3, 1F×5, 20×3 |
| 25 | $93BB | $93E5 | 11 | 12×4, 43×2, 44×4, 46 |
| 26 | $93F0 | $9486 | 47 | 50×2, 51×2, 52×3, 53×6, 54, 55×6, 56×8, 58, 59×4, 5A×2, 5B×4, 5C, 5D×2, 5E×5 |
| 27 | $94B5 | $94BA | 1 | 1E |
| 28 | $94BB | $94C0 | 1 | 1E |
| 29-41 | $94C1 | $94C6 | 1 | 1E at ($8000, $8000), never in range: these levels have no placed objects |

963 list entries in all, counting the shared dummy entry once for each of levels 29-41 (950 real objects). Several types sit at the same point in a group with
offsets 0, 0 (level 2: types 33-40; level 7: types 23-2D and 60/61): those are multi-part objects. **[V]**

### 3.4 Spawning (the placement scan)

Placed objects exist in RAM only while near the camera. **[C]** for the rules, **[V]** for the effects
(objects appeared in slots 23-30 with the decoded index, type and position).

- **Window.** With rx = object x − camera x and ry = object y − camera y as signed 16-bit values, an object is
  *in range* when −M ≤ rx < 256 + M and −M ≤ ry < 256 + M, where M is the level's spawn margin ($050C, level
  header byte 10; e.g. $90 on level 0, $40 on level 1).
- **Level start.** After the level is set up, the scan runs once over the whole list (cursor 0 to n − 1) with
  the visibility test switched off: every object that is in range and still present (type-list copy entry ≠ 0) is
  spawned, in list order, as long as one of slots 23-30 is free. There is no per-step limit in this pass.
- **Each logic step** (in its second video frame, the B pass of `game-flow.md` 1.2). The budget is: at most 8
  list entries examined, at most 1 spawn, and never past the end of the list. Starting at the cursor, each entry
  examined advances the cursor by one; the step ends after the 8th entry, after a spawn, or when the cursor
  passes the last entry (it is then reset to 0 and the next step starts at entry 0). Entries inside a group count
  one each, like plain entries. It skips objects whose type-list copy
  entry is 0. An object is spawned only if it is in range **and not already visible**: not (0 ≤ rx < 256 and
  0 ≤ ry < 224), and not (−16 ≤ ry < 0). The second exclusion is a band of 16 px just above the screen's top edge,
  **for every rx** (above the screen's columns and beside them alike): an object there would pop into view at the
  top, so it waits; objects that are further above (ry < −16), below the screen (ry ≥ 224), or beside it (rx < 0 or
  rx ≥ 256) with ry outside that band are spawned. (The earlier wording, "ry < −16 waits", had the band the wrong way
  round.) [V]: the test's four inputs (rx high byte, ry high byte, ry low byte) were forced one by one in Mesen and the
  outcome read: skip for ry high $FF with low ≥ $F0 at rx high 0 and 1, spawn for low $E0 and $EF; skip for rx high 0,
  ry high 0, low < $E0 (on screen); spawn for low $E5, for ry high 1, and for rx high 1.
  The scan stops for the
  step after one spawn. It is not run while the pod's state is 9, nor on levels 29-41.
- **Slot choice.** The lowest free slot in 23-30. No spawn if the object is already live (a slot with state bit 7
  holding the same list index) or if 23-30 are all busy (it is tried again on a later pass).
- **Initial values.** type = list type; frame = initial frame (4.1); state = initial state (4.1); hit points
  from the state (2.4); flags X = flags Y = work 5 = work 6 = 0; list index = its index; position = decoded
  position. For types $12, $17, $18, $47 and $5E the four work bytes receive a copy of the position (home);
  for all other types they are 0. A per-state initialisation routine then runs (behaviour).

### 3.5 Despawning and removal

- Each logic step, in the sprite-building pass (B pass, `game-flow.md` 1.2), every live object in slots 1-30 is
  tested (the same rule for all of them, including the item slots 11-14, whose spawning follows `levels.md` 3.3)
  with rx, ry as in 3.4 (pixels, object point minus camera top-left, signed 16-bit), M the level's margin
  and W = max(M, 32): **[C]**

  | Object | Kept when | Drawn when |
  |---|---|---|
  | frame $FF (invisible) | −W ≤ rx < 256 + W, and then −M ≤ ry < 256 + M (the lower y bound is −M, not −W: with M < 32 an invisible object between −32 and −M is freed while a visible one is kept) | never |
  | any other frame | −W ≤ rx < 256 + W, and: −W ≤ ry < 256 + M if −32 ≤ rx < 288, otherwise −M ≤ ry < 256 + M | −32 ≤ rx < 288 and −32 ≤ ry < 256 |

  All lower bounds are inclusive and all upper bounds exclusive. An object that is not kept is freed (state ← 0),
  except the object held by the tractor beam (slot = RAM $66) while its work 6 is non-zero; this exception applies to the
  x test of visible frames and to the y test of both kinds, but not to the x test of an invisible object. **[C]**
  (Earlier text: "y is not tested" for invisible objects; the program does test it, 2026-10-17.) A freed placed object or item respawns later from its list with fresh initial values (state,
  frame, hit points).
- Items (slots 11-14) spawn in a fixed window, −48 ≤ rx, ry < 304 (`levels.md` 3.3), but are freed by the rule
  above. Where W = 32 (levels with M ≤ 32: 13, 20, 24, 29-41), an item with 288 ≤ rx < 304 is spawned in one A pass
  and freed in the next B pass, again and again while the camera stays there; it is never drawn (rx ≥ 288), so
  nothing shows, but it occupies a slot for half of each cycle. The remake does not reproduce this (`docs/deviations.md`, "Item spawn churn"). **[V]** (level 29: an item at rx = 295 was spawned
  and freed every 3 logic steps; the levels analyst owns the item side of the rule)
- When a placed object is destroyed (hit points reach 0) or collected, its type-list copy entry ($0460 +
  index) is set to 0, so it never spawns again until the level's list is copied afresh. Destroyed objects turn
  into an explosion in the same slot. **[C]**
- Within a drawn object, each sprite whose screen position (rx + dx or ry + dy, 16-bit) has a non-zero high
  byte is left out; the others are written (and take consecutive sprite-table entries; 6.6a). **[C]**

### 3.6 When the type-list copy is refreshed **[C]**

The copy at $0460 (and with it the items list, section 10) is rebuilt from ROM each time the program reads a level
header for play: on arriving at a planet (before the surface picture and launch), when play moves to another level
(the level reloads of `game-flow.md` 7.4) and on entering level 26. Four places in the program do this, always
together: read header and items list, then rebuild the type-list copy, then apply the memory below.
Rebuilding makes every placed object present again, so the game keeps a small memory of destroyed objects:

- **Saving.** Just before a new level's data is read, the presence of the level being left (RAM $3C) is packed
  into one of two blocks: $0626-$0631 for levels 0-12 (up to 12 bytes: the largest list has 85 entries, so 1 + 11
  bytes), $0632-$0635 for levels 13-25 (up to 4 bytes: the largest list has 20 entries). The second block ends
  before $0636 (the mothership fuel count saved on a second level, `economy.md` 3.4). Levels 26-41 are not
  saved. Byte 0 = the level index; then one bit per list entry, entry 0 in bit 7 of byte 1, 8 entries per byte,
  the last byte padded with 0 bits at the bottom; a bit is 1 if the entry's copy is non-zero (still present).
  Each block holds only the most recently left level of its group.
- **Restoring.** After the copy is rebuilt, if the block of the new level's group holds the new level's index,
  every entry whose bit is 0 gets copy entry 0 (stays destroyed).
- So destroyed placed objects stay away when the player goes from a planet to its second level and back, but come
  back after visiting another planet. The blocks are set to $FF at power-on / new game.

### 3.7 Screen-relative rules: index for a view wider than 256 px **[C]**, mostly **[V]** (analyst audit, 2026-10-25)

Every rule of the original that measures an object or position against the screen's horizontal extent, with rx = x − camera x
(signed 16-bit, `levels.md` 5.1) and the screen being rx = 0 ... 255. A remake with a wider view must widen the rules marked
"widen"; the others are tied to the world or to a fixed picture and stay. The audit covered every instruction in the program
that reads the camera's x bytes (RAM $4C/$4D; found by scanning all eight banks for every addressing form, including the
parts Ghidra's listing leaves undecoded, then reading each hit): there are no others. In particular **nothing in the program tests
an object's horizontal place against the screen except the rules below**: no behaviour acts, fires or turns only while on screen
(behaviours depend on being alive, which is the window of 3.5, and on the pod), no aiming or homing clamps to the screen (all
of it is pod-relative), no sound request is gated on visibility, no collision test is skipped for being off-screen (they run
for every live slot), and shots, particles, explosions and sparks have no screen test of their own: they are freed by 3.5 like any
object (slots 1-30), so widening 3.5 widens them. Warp levels add no horizontal screen test (their arena is world-fixed,
`levels.md` 6.7).

| # | Rule | Where | Bounds (rx) | Status | Remake |
|---|---|---|---|---|---|
| 1 | Items activation window | `levels.md` 3.3 | -48 <= rx < 304 (also ry) | confirmed [C] (numbers re-read) | widen |
| 2 | Placed-object in-range window (spawn margin M = header byte 10) | 3.4 | -M <= rx < 256 + M (also ry) | confirmed [C]/[V] | widen |
| 3 | Placement scan's "not on screen" test: an in-range object is not spawned while 0 <= rx < 256 and 0 <= ry < 224, nor while -16 <= ry < 0 (every rx); not applied in the scan at level start | 3.4 | 0 <= rx < 256 | confirmed [V] | widen the 256 |
| 4 | Despawn: kept window, W = max(M, 32) | 3.5 | -W <= rx < 256 + W | confirmed [C] | widen |
| 5 | Drawn test (object level) | 3.5 | -32 <= rx < 288 | confirmed [C] | widen |
| 6 | **Per-sprite cut** of a drawn object: a sprite is written only if its screen x (rx + dx, + 1 for layouts with count above 0, 6.2) is 0 ... 255 as a 16-bit value (high byte zero), likewise y; skipped sprites take no OAM entry (6.6a). The pod (slot 0) has no object-level test at all, only this one. | 3.5, 6.2, 6.6a | 0 <= sprite x <= 255 | **new to this list** (already in 3.5 and 6.6a) [C], pod drawer [C] | widen (sprites within the wide view are drawn) |
| 7 | Edge spawn of generators, mines and the green orb (just outside the screen) | `behaviour.md` 2.10 | left x = camera x - 32 (rx = -32), right x = camera x + 288 (rx = 288 = screen width + 32) | confirmed; fallbacks completed [C] | place at the wide view's edges -/+ 32; map-edge tests on the spawn x itself |
| 8 | Level 26 pod limits | `player.md` 9.1 | 24 <= rx <= 232 (x - 24 - camera x clamped to 0 ... 208) | confirmed [V] | widen if the wide view should let the pod roam; the original lets it use 24..232 of 0..255 |
| 9 | Level 26 type 59-5D ($B6) activation: waits until camera x >= object x (16-bit), i.e. rx <= 0, then becomes visible at that spot and flies right | `behaviour.md` 4.16 | rx <= 0 | **new** [V] | the object is invisible while waiting and appears at the screen's left edge; with a wide view test against the view's left edge (rx <= -(extra width on the left)) so it does not appear inside the view |
| 10 | Level 26 prelude of $B4/$B5/$B6 by the camera's x page P (RAM $4D): P = $16-$17 speeds up, P >= $18 frees | `behaviour.md` 4.16 | camera x 5632-6143 / >= 6144 | **new to this list**; a camera position, not an edge | unchanged (world-fixed) |
| 11 | Camera follow band: right when the player's rx >= 144, left when rx < 112; only counts when rx is 0-255 | `levels.md` 5.2 | 112, 144, 0-255 | confirmed [C] | stays at 256 by design (view centred on the original camera) |
| 12 | Camera placement at the start of play: camera x = player x - 128 (clamped to 0 ... map width - 256) | `levels.md` 5.2 | 128 = half of 256 | confirmed [C] | stays (same reason) |
| 13 | Parallax stars: x byte wraps at 256 in screen space, spawn x values 0, 84, 199, 135, 33; while fading a star dies when its new x < 8 | `graphics.md` 5.7.3 | 0-255, 8 | confirmed [V] | widen the wrap/spawn range; the "x < 8" death is the left column mask |
| 14 | Delivery to the mothership (transition 1): the held item is moved to camera x + 130, y + 36 (kinds other than 3, 4, 6, 9) or the container's three objects to camera x + 131 / y + 18 (kind 9, `scenes.md` 10) so that the message screen draws it at screen x 130 / 131 | `scenes.md` 10, message screens | rx = 130/131 | **new** [C] | a fixed picture position of the 256 px message screen: keep it relative to the original 256 view (centre + 2) |
| 15 | Backdrop interlude (`scenes.md` 11.1): camera x is 0 (screen = world); pod x clamped to 16 ... 240 (at 240 or more it becomes 240, speed 0); the object x lists use 3 ... 250; objects freed by the margin rule with M = 128 | `scenes.md` 11.1.2, 11.1.3 | 16 <= x <= 240 | **new to this list** [V] | scene-level, in 256 px coordinates; keep (or widen the walls and lists together) |
| 16 | Background streaming: the map column that enters is camera x + 256 (right) or camera x + 8 (left, rounded to 8), rows likewise; the leftmost 8 px column is masked | `levels.md` 5.3, 5.1 | 256, 8 | confirmed [C] | rendering only |

Notes:

- **Rule 6 and the sprite budget.** The original also limits OAM to 64 entries and applies the left-column mask (`graphics.md` 1.3);
  those are hardware limits (`docs/deviations.md`), not screen rules.
- **Rule 5 against rule 6.** Rule 5 decides whether an object's sprites are built at all; rule 6 then decides per sprite. With a wide view
  both bounds move together with the view's edges; rules 4, 3, 2 and 1 keep their relation to each other (kept >= in range >= drawn).
- **Rule 7 detail** is in `behaviour.md` 2.10.
- **What does not scale:** distances between objects and the pod (the 256-px "far" thresholds of `behaviour.md` 2.4/Q19, the
  tractor beam's rope), the camera's follow rules, level 26's route, and every world-fixed trigger. They are not screen
  rules.

## 4. Object types

### 4.1 Per-type tables (bank 4)

Indexed by type 0-$61 (98 entries). **[V]**

| Bank 4 | ROM | Content |
|---|---|---|
| $A25D + type | 0x2225D | initial frame (layout number, table A) |
| $A2BE + type | 0x222BE | initial state |

The two tables overlap by one byte: $A2BE is both the frame of type $61 and the state of type 0. Type 0 is not
used by any list.

### 4.2 Catalogue

Names are from the sprites' appearance and from what the objects did in the survey (section 9); behaviour labels
are **[H]** unless marked. "HP" = initial hit points (2.4). "Frames" = initial frame and the frames seen while
the object was watched for 100 frames.

| Type | Frame | State | HP | Appearance / observed behaviour |
|---|---|---|---|---|
| 01 | $1B | $86 | 3 | fixed gun turret (shape 1): opens and closes in a cycle and fires while open; shots damage it only while it is open (work 6 = 2 or 3), otherwise they glance off. Rules: `behaviour.md` 4.1 **[V]** |
| 02 | $1E | $87 | 3 | turret, shape 2 |
| 03 | $21 | $88 | 3 | turret, shape 3 |
| 04 | $21 | $89 | 3 | as 03, mirrored variant |
| 05 | $24 | $8A | 3 | turret, shape 4 |
| 06 | $24 | $8B | 3 | as 05, mirrored variant |
| 07 | $27 | $8C | 3 | turret, shape 5 |
| 08 | $27 | $8D | 3 | as 07, mirrored variant |
| 09 | $2A | $8E | 3 | turret, shape 6 |
| 0A | $2A | $8F | 3 | as 09, mirrored variant |
| 0B | $2D | $90 | 3 | turret, shape 7 (blue wedge) |
| 0C | $2D | $91 | 3 | as 0B, mirrored variant |
| 0D | $30 | $92 | 3 | turret, shape 8 |
| 0E | $30 | $93 | 3 | as 0D, mirrored variant |
| 0F | $18 | $94 | 3 | turret, shape 9 (blue crescent) |
| 10 | $18 | $95 | 3 | as 0F, mirrored variant |
| 11 | $34 | $97 | 48 | domed ground installation (red/green dome on a base); fires |
| 12 | $51 | $98 | 8 | ground gun (arm with a ball joint, frames $51-$54); fires; has a home position |
| 13 | $5D | $99 | 255 | two-legged walker (frames $5D-$5F); stood still while watched |
| 14 | $5D | $9A | 255 | walker, second variant; releases B$5C-framed objects (state $1B) |
| 15 | $68 | $9B | 255 | extending pillar (frames $68-$7D, 4 to 9 sprites tall); does spawn (level 1, list index 59) |
| 16 | $68 | $9C | 255 | extending pillar: grows from $68 to $7D, one frame every 2 frames |
| 17 | $51 | $9D | 8 | ground gun, variant with home position |
| 18 | $51 | $9E | 8 | ground gun, variant with home position (seen upside down) |
| 19 | $42 | $9F | 10 | hourglass pedestal turret; fires |
| 1A | $42 | $A0 | 10 | hourglass turret, upside down (ceiling) variant |
| 1B | $43 | $A1 | 14 | larger turret block (frame $43) |
| 1C | $3F | $A2 | 24 | wide machine (frames $3F-$41) |
| 1D | $3F | $A3 | 24 | wide machine, variant (seen upside down; animates $3F→$40→$41) |
| 1E | $34 | $A4 | 48 | domed installation, second kind; on levels 17, 27, 28 and the warp levels it is the only entry |
| 1F | $7E | $A5 | 4 | magnet (pulls the pod), small block alternating frames $7E/$7F (most common type); `behaviour.md` 4.9 |
| 20 | $80 | $A6 | 8 | magnet (pushes the pod away), small block alternating $80/$81 every 4 frames; `behaviour.md` 4.9 |
| 21 | $83 | $A7 | 16 | large creature (35 sprites); moves |
| 22 | $85 | $A8 | 16 | large machine (frames $85-$88); moves |
| 23-32 | $8D-$90 | $A9 | 8 | parts of a moving formation (level 7: 23, 25, 27, 29, 2B, 2D at one point); each cycles $8D-$90, 32 frames per step |
| 33-42 | $91-$95 | $AA | 6 | parts of a moving formation (level 2: 33, 34, 37, 38, 3B, 3C, 3F, 40 at one point): the parts spread out; each steps through $91-$95 and back, 32 frames per step |
| 43, 44 | $96 | $AB | 2 | green creature, frames $96/$97 alternating every 8 frames; 44 mirrored |
| 45, 46 | $98 | $AB | 2 | green creature, frames $98/$99 alternating every 8 frames |
| 47 | $51 | $AC | 0 | ground gun with home position (hit points 0) |
| 48 | $FF | $AD | 255 | invisible generator: releases shots / objects |
| 49 | $FF | $AE | 255 | invisible generator; its work 6 counts up continuously |
| 4A | $9E | $AF | 3 | blue sphere: rests on $9E, then $9F, $A0 (8 frames each), $A1; about a third of them moved |
| 4B, 4C | $FF | $B0, $B1 | 255 | invisible generators |
| 4D, 4E | $FF | $B2, $B3 | 255 | invisible generators |
| 4F-53 | $AF-$B3 | $B4 | 1 | level 26 objects, drifting |
| 54-58 | $AF-$B3 | $B5 | 1 | level 26 objects, drifting |
| 59-5D | $FF | $B6 | 1 | level 26 invisible objects |
| 5E | $B4 | $B7 | 10 | level 26 object (frames $B4-$B6), home position |
| 5F | $9A | $B8 | 1 | egg (frames $9A→$9B→$9C→$9D, 16 steps each, then it disappears for good; nothing hatches, `behaviour.md` 4.17); destroying it adds an item of kind 20 (an exit, 10.2) at its position to the end of the items list (count $BA + 1) **[C]** |
| 60 | $BF | $B9 | 16 | satellite / station (frame $BF), placed together with 61; moves; fires A$C0 shots |
| 61 | $BE | $BA | 4 | small companion of the satellite (single sprite); moves with it |

Types 01-10 share these rules. Types 24, 26, 28, 2A, 2C, 2E-32, 35, 36, 39, 3A, 3D, 3E, 41, 42, 4F and 57 are defined but placed nowhere.

On level 26 the program loads no object sprite sets (5.2), so its frames use absolute tile numbers and show
other tiles than in the frame catalogue below. **[V]** What is in pattern table 0 there and which tile base applies (base 0 everywhere except frame $33: tile base $17, a single
range [$33, $34) set by the level start) is `graphics.md` 5.7.5; the frames the level uses are listed in 6.4a.

## 5. Sprite sets and tile numbers

### 5.1 Tiles

Sprites use pattern table 0 ($0000). Tile 0 is blank; the pod set occupies tiles $01-$40; object sets start at
tile $41; during play the common sprites occupy $BA-$FF (`graphics.md` 6.1, 6.4). **[V]**

### 5.2 Frame ranges and tile bases

`graphics.md` 6.4 describes the per-level lists at bank 1:$C980. Their bytes are **frame numbers** (layout
numbers), not object types: **[V]**

- Each list byte names a record by its first byte. Record (5 bytes): `first frame, end frame (exclusive), tile
  data address low, high (bank 1), tile count`.
- List A uses the 19 records at bank 1:$CAB4 and serves **table A** (slots 15-30); list B uses the 10 records at
  $CB13 and serves **table B** (slots 1-14).
- Tiles are copied in list order from tile $41 on. For each record the program notes the range
  [first, end) and the tile number where its tiles start (the **tile base**): up to 8 ranges for table A
  (RAM $05CF + 2i = first, $05D0 + 2i = end, $05DF + 2i = base) and up to 4 for table B ($05EF/$05F0/$05F7).
- When a frame is drawn, the tile base is that of the first range containing the frame; frames in no range use
  base 0 (their tile numbers are absolute, e.g. the common sprites).

| Table A record (first, end, tiles) | | Table B record | |
|---|---|---|---|
| $18-$33, 38 tiles | turrets 01-10 | $53, 6 | (levels 13-28) |
| $33, 1 | single dot | $57, 4 | |
| $34-$46, 26 | installations, turrets, machines | $58, 2 | |
| $47-$50, 12 | turret gun | $59, 4 | |
| $51-$54, 6 | ground gun | $5A, 4 | |
| $55-$5C, 30 | pink block structures | $5B, 4 | |
| $5D-$5F, 10 | walker | $5C, 4 | FUEL sign |
| $60-$63, 4 | wriggling flame | $73-$7D, 15 | countdown digits 10 ... 0 (warp levels) |
| $64-$67, 4 | flat orange bar | $8A-$8D, 1 | |
| $68-$7D, 5 | extending pillar | $AF-$B2, 6 | |
| $7E-$82, 18 | small blocks | | |
| $83-$84, 21 | large creature | | |
| $85-$88, 24 | large machine | | |
| $8D-$95, 12 | formation parts | | |
| $96-$99, 14 | green creatures | | |
| $9A-$9D, 8 | egg | | |
| $9E-$A2, 7 | blue sphere | | |
| $A3-$AE, 14 | jointed arm (frames grow in length) | | |
| $BF-$C0, 4 | satellite | | |

Example (level 0): ranges A: [$18,$34) base $41, [$83,$85) base $67, [$9A,$9E) base $7C; B: [$59,$5A) base $84,
[$5C,$5D) base $88, [$8A,$8E) base $8C. These values were read back from RAM $05CF-$05FE after the level started.
**[V]**

## 6. Sprite layouts (metasprites)

### 6.1 Tables (bank 1)

| Table | Used for | Address low / high bytes | Entries |
|---|---|---|---|
| P | slot 0 (the pod) | $A859 / $A879 (ROM 0x0A859 / 0x0A879) | 32 |
| A | slots 15-30 | $A899 / $A95A (ROM 0x0A899 / 0x0A95A) | 193 ($00-$C0) |
| B | slots 1-14 | $AA1B / $AAF5 (ROM 0x0AA1B / 0x0AAF5) | 218 ($00-$D9) |

Entry `f` of a table is the layout address (low byte at low + f, high at high + f). Unused entries hold
addresses outside $A54C-$C88B: table A $10-$17; table B $13-$17, $42-$50 and $5F. All layouts and the three
tables fill bank 1 $A54C-$C88B (ROM 0x0A54C-0x0C88B) completely; several entries share a layout. **[V]**

### 6.2 Layout format

```
byte 0: count c
if c > 0: c entries of 4 bytes: dx (signed), dy (signed), tile, attributes
if c = 0: byte 1 = tile, byte 2 = attributes: one sprite with dx = dy = 0
```

For each entry the sprite's top-left corner is at screen (X + dx + 1, Y + dy) for an object drawn **without** the
vertical flip of 6.3, and at (X + dx, Y + dy) for one drawn with it, where (X, Y) is the object's screen position and
dx is negated first for a horizontal mirror: the plain case carries a stray +1 that the vertical-flip case lacks. **Only
layouts with count c > 0 get this.** A count-0 layout (the single-sprite form above) is placed at exactly (X, Y), with
no +1 whether flipped or not (the drawer's count-0 path stores the screen position directly; the flips still apply to
its attributes). **[C]**, and **[V]** 2026-10-20 (the pod's flame, slot 1 frame $19, drawn at OAM x = X - camera x
exactly over steps 82-230 of the delivery replay). Count-0 frames, from the layout tables: slots 1-14 (table at bank 1
$AA1B/$AAF5): $10-$12, $18-$37, $46, $49, $4E, $51, $62, $63, $6D, $6E, $9F (so the pod's flame, frames $18-$37 =
flameBase + d, and Jetman's small flame $10, are exact); slots 15-30 (table $A899/$A95A): $33, $37-$3E, $50, $84,
$88-$8C, $A2, $BE, $C0. Every other frame of those tables has c > 0 and carries the +1. **[V]** (ninth round, c > 0 layouts: the ship of the surface
scene, fx = 0, 1 and fy = 0, $81 poked each frame, OAM x read back; a level-0 object at rest, dx = 2: OAM x = X + 3;
the pod, slot 0, has its own drawer and is exact: OAM x = X - 4 and X + 4). So a mirrored picture sits one pixel
further right than "X - dx" and an unflipped one one pixel further right than "X + dx"; both of those were
mis-stated here before. Not intended: **Q35**. The NES shows a sprite one line below its OAM Y value; the original writes Y + dy into OAM unchanged.
**[V]** (predicted sprites matched OAM in 391 of 395 checked frames; the rest were frames where the object or
camera had just moved)

**Which camera.** Every test and every drawn position uses the camera RAM $4C-$4F as it stands at that moment of
the pass, never an earlier copy: the object sprites and removal test (B pass step 2), the player's sprite (B step 4)
and the placement scan (B step 5) all run after that pass's camera move (B step 1), and the items scan (A pass
step 9, `game-flow.md` 1.2) runs after the A pass's camera move (A step 5). The picture then shows the sprites
and the background of that camera together: the sprites reach the screen in the next frame's sprite transfer, and
the scroll written in that frame comes from the same camera (`levels.md` 5.3.1, one frame of lag for both).
The "±3 px jitter" below is the checking method (camera sampled at another moment), not a property of the game.
**[C]**

- Tile number = layout tile + tile base (5.2) (mod 256). Table P uses no base (tiles are absolute; the pod set
  sits at $01-$40). **[V]**
- Attributes: bits 0-1 palette (sprite palettes 4-7), bit 5 behind background, bit 6 mirror horizontally,
  bit 7 flip vertically; layouts also use these bits to build symmetric shapes from one tile. **[V]**
- dx ranges −63 ... +89, dy −71 ... +85. Entries are listed in priority order: the first entry's sprite is in
  front. **[V]**
- Layouts of 2 × 2 pictures (the pod, many small objects) use offsets −4 and +4, so the picture covers x − 4 to
  x + 11 and y − 4 to y + 11: the object point is 4 pixels left of and above the picture's centre.

### 6.3 Mirroring

Per object (slots 1-30), from `flags X` (fx = $0336) and `flags Y` (fy = $0355): **[C]**, verified for the values
0, 1 and $80 **[V]**

A mask `m` is formed and every sprite's attribute byte is XORed with it:

| m bits | Value |
|---|---|
| 1-6 | if fx bit 0 = 1: fx bits 2-7 (fx bit `i` gives m bit `i − 1`); else 0 |
| 6 | also set (whatever fx bit 7 is) if fx bit 0 = 1 and fx bit 1 = 0: mirror horizontally |
| 7 | fy bit 7: flip vertically |
| 0 | always 0 |

- Horizontal mirror (m bit 6): every dx is negated (no width correction: a tile at dx = −4 moves to +4, so the
  picture mirrors about the vertical line 4 pixels right of the object point).
- Vertical flip (fy bit 7): every dy is negated and the entries are drawn in reverse order (last entry first).
- So fx = 1 (moving left) shows the mirrored picture; fx = 3 does not. Some objects set fx bits 2-3, which change
  the palette.
- Slots 1 and 2 also set attribute bit 4 (no effect on the hardware) and copy bit 5 (behind background) from the
  pod's state (RAM $98 bit 5); the object held by the tractor beam, when it is in slots 3-14, also gets
  bit 4. **[C]** For slot 2 the flag bytes are written by the beam job / the shield bookkeeping (`player.md` 8.4, 4.7a): flags X = 1 mirrors the two-sprite beam picture (dx negated, tile mirror bit toggled), **[V]** against OAM.

### 6.4 The pod (slot 0)

The pod's frame value selects the layout as follows: **[V]** (rotation), **[C]** (the rest)

| Frame | Layout | Mirror |
|---|---|---|
| $00-$10 | P$00-P$10 | none |
| $11-$1F | P(32 − frame), i.e. P$0F ... P$01 | horizontal |
| $20-$2E | P(frame − $0F) = P$11-P$1F | horizontal when the **facing bit** of fx (RAM $0336) is 1 (the pod's own drawer, not the object rule of 6.3): the facing bit is fx bit 1 when fx bit 2 is set, otherwise 1 when fx is not 0 (bit 0 for every value the game gives: 0, 1, 4-7). So fx = 1, 6, 7 mirror; 0, 4, 5 do not. **[V]** (`jetman.replay`, 223 steps with a frame of $20 or more: the pod sprites' attribute bit 6 matched in every step (4 steps where the flame shared the entries aside); the earlier "bit 0 = 1 and bit 1 = 0" is wrong for fx = 5, 6 and 7); read from the program too. Same bit as the flame's side, `player.md` 4.8 |

- Frames 0-31 are the 32 directions of the pod, 11.25° apart, clockwise from 0 = nose up; 8 = right, 16 =
  down, 24 = left. P$00-P$10 are 2 × 2 sprites (tiles $01-$40 of the pod set; P$00 and P$10 are mirrored halves
  of one tile pair). Holding left or right turns the pod one step every second frame, i.e. once per logic step. **[V]**
- P$11-P$15: pod explosion (expanding fragments, tiles $F7-$FE, absolute). P$16-P$1B: six 2 × 2 frames, Jetman outside the pod
  (frames $25-$2A). **[V]** (2026-10-20, `jetman`, `respawn`, `spare` replays: OAM and CHR RAM read back) They use tiles $01-$12 of the sprite
  table, but **not the pod's art**: from the pod's destruction until a pod exists again those 18 tiles hold Jetman's
  own figure (ROM 0x0F185, `graphics.md` 5.7.4; the same tiles the pod's frames use while a pod flies). Tiles per frame, in OAM order (frame: tiles):
  $25: 03 04 01 02; $26: 05 06 01 02; $27: 07 08 01 02; $28: 09 0A 01 02; $29: 0C 0D 01 0B; $2A: 0E 0F 01 0B. Palette 0 from the layout (the
  pod's palette 0, colours $30 / $22 / $12 on every level); the sprites are mirrored (attribute bit 6) when his facing bit is 1 (6.4 table: he then faces **left**, backpack on the right; the unmirrored art faces right), and
  RAM $98 is ORed in like for the pod. Drawn from the pod's art in the pod's blue he looks wrong: his art is mostly colour 1 (white) with
  colours 2 and 3 (light blue, blue), where the pod's is mostly colours 2 and 3. P$1C/P$1D: a small and a large
  spark (tiles $C8/$C9). P$1E/P$1F: the **final-level pod** (6.4a).
- Flashing. The pod is drawn according to RAM $99, the hit-pause counter (`player.md`) in the B pass of each step: $99 = 0: normal; $99 ≥ $2C:
  the spark P($1C + ($99 AND 1)) instead of the pod, i.e. P$1C on even values and P$1D on odd ones; otherwise
  hidden when ($99 AND 3) = 0 and normal when not. Frame $FF also hides the pod. **[C]**, **[V]** in Mesen. $99 is
  driven in two ways (both in A passes, once per logic step):
  - when the game sets $99 to $30 (probably when the pod is hit **[H]**), it decreases by 1 per step: steps with $30-$2C show the spark, alternating
    P$1C, P$1D, P$1C, P$1D, P$1C (one step = 2 video frames each); from $2B down to 1 the pod is hidden on the
    steps where $99 is $28, $24, ..., $04 (one step in four: shown 6 frames, hidden 2) and RAM $98 bit 0 is 1 while
    $99 is $1F or more **as the step leaves it** (the bit is written in the drawing stage after the step's decrement from the new value,
    equivalently: while the value before the decrement was $20 or more), which ORs palette 1 into the pod's attributes (it shows in another
    colour). **[V]** `tow.replay`: palette 1 at the steps that leave $99 = $1F, palette 0 at $1E (`game-flow.md` 9.2);
  - while RAM $97 (an invulnerability timer, set to $40, $60 or $80 by the game) is non-zero it decreases by 1
    per step, and $99 is recomputed every step from the frame counter so that the B pass sees 2, 4, 6, 0, 2, ...
    on successive steps: the pod is hidden on the step where it is 4, again one step in four.
  The other writers ($99 = $10 or 1) follow the same drawing rule.
- The pod's (and Jetman's) attribute bytes are ORed with RAM $98 (bit 5 = behind background, bits 0-1 = palette bits). **[V]**
  The bits 0-1 are the hit pause's palette 1 above, the momentum killer's and anti-gravity's colour flashes and the gate state's flash: who writes
  them, in what order and on which steps is in `game-flow.md` 9.2, "Bits 0-1". The hit pause (and the invulnerability timer) overrides the
  flashes completely while $99 is not 0.

### 6.4a The final-level pod and the level's other pictures **[V]** (2026-10-20)

The pod in state 6 (`player.md` 9.1) has frame $2D, or $2E in an A pass in which a direction is held and bit 2 of RAM $5D is set; the layout is P(frame − $0F) = P$1E or
P$1F, drawn by the pod's own drawer (6.2: exact positions, no +1). The mirror rule of 6.4 applies to it with RAM $0336 = 0 or 5 (not mirrored) and also after a wall bounce flipped bit 0 (`player.md` 9.1: the byte is then 1, which **mirrors** the whole picture (every dx negated, attribute bit 6 set), or 4, which does not); not seen in the replay (no bounce on the open route), read from the program. Seven sprites, (dx, dy, tile, attribute), tiles of
pattern table 0 as `graphics.md` 5.7.5 gives them (tile 0 is not blank on this level):

| Layout | (-8, 4, $05, 2) (0, 4, $06, 1) (8, 4, $07, 1) (-16, 3, **$00 or $01**, 0) (-8, -4, $02, 2) (0, -4, $03, 1) (8, -4, $04, 0) |
|---|---|
| $1E (frame $2D) | the fourth sprite is tile $00 |
| $1F (frame $2E) | the fourth sprite is tile $01 (the rear flame in its other phase) |

OAM order: entries 1-7 in this order (OAM entry 0 is the split marker, the objects start at entry 8, 6.6); sprite palette 2 = $3A $1A $09 (green), 1 = $38 $27 $16 (orange), 0 = $30 $22 $12 (white and blues);
RAM $98 is ORed in as for the pod (nothing on this level).
Verified against the OAM of 3,898 steps (tiles $05 $06 $07 $00 $02 $03 $04, attributes 2 1 1 0 2 1 0 in every step; tile $01 was not in the replay, the layout is read from the ROM).

The level's other layouts (tile numbers are layout tile + base, base 0 except frame $33): rocks $AF-$B3, cores $B4-$B6, pieces $33 then $AF: `behaviour.md` 4.16 (last paragraph before "The same state").
Frames for hits, explosions and sparks are the common ones of every level (table B $00-$0F and table A $00-$0F, tiles $F6-$FE and $C8/$C9, 6.5).

### 6.5 Frame catalogue

Table A (slots 15-30), by range (tile bases from 5.2): **[V]** (rendered from Mesen CHR dumps)

| Frames | Content |
|---|---|
| $00-$0F | explosion debris, growing from 4 to 23 sprites and fading (absolute tiles $F6-$FE) |
| $18-$32 | nine turrets (types 01-10) × 3 frames each: [$18-$1A], [$1B-$1D], [$1E-$20], [$21-$23], [$24-$26], [$27-$29], [$2A-$2C], [$2D-$2F], [$30-$32]; 2-3 sprites; frames 2 and 3 show the object turned |
| $33 | single dot (c = 0) |
| $34-$36 | domed installation (15-17 sprites) |
| $37-$3E | single-sprite pieces (c = 0) |
| $3F-$41 | wide machine, 3 frames |
| $42, $43 | hourglass turret, larger turret block |
| $44-$46 | flat bars (2 sprites) |
| $47-$50 | turret gun in 8 directions plus a single sprite ($50) |
| $51-$54 | ground gun, then three tall flame/bar shots |
| $55-$5C | pink block structures |
| $5D-$5F | walker, 3 poses |
| $60-$63 | wriggling flame, 4 frames |
| $64-$67 | flat orange bar, 4 frames |
| $68-$7D | extending pillar: 22 frames, 4 to 9 sprites |
| $7E-$82 | small blocks |
| $83, $84 | large creature (35 sprites); single sprite |
| $85-$88 | large machine, 3 frames; single sprite |
| $89-$8C | small shots (c = 0, absolute tiles) |
| $8D-$95 | formation parts |
| $96-$99 | green creatures, 2 × 2 frames |
| $9A-$9D | egg, 4 frames |
| $9E-$A1 | blue sphere, 4 frames |
| $A2 | small spark (c = 0) |
| $A3-$AE | jointed arm or tentacle in several lengths and bends (4-5 sprites) |
| $AF-$B3 | explosion growing from 4 to 19 sprites (absolute tiles) |
| $B4-$B6 | small fragments (4-5 sprites) |
| $B7, $B8 | 16 fragments flying apart |
| $B9-$BB | green orb, 3 sizes |
| $BC, $BD | star (absolute tiles) |
| $BE | small spark (c = 0) |
| $BF, $C0 | satellite; single sprite |

Table B (slots 1-14):

| Frames | Content |
|---|---|
| $00-$0F | same layouts as table A $00-$0F |
| $10-$12 | flame pieces |
| $18-$26 | exhaust flame / shot in 15 directions (single sprite) |
| $27-$41 | single sprites (c = 0): sparks and dots |
| $51 | green orb |
| $52 | diamond marker |
| $53-$5E | items and signs: $54 spare pod (item kind 1, `economy.md` 2.1), $55/$56 star, $5C FUEL sign, $5D ring, $5E box, ... |
| $60-$68 | items: $60 bottle, $61 radioactive symbol, $64 box, $65 orange gem, $66 blue gem (item kind 16), $67/$68 rings |
| $69-$6C | pick-up sparkle ring opening, 4 frames |
| $6D-$7F | single sprites; $73-$7D countdown digits 10 down to 0; $7E/$7F star marker |
| $80-$D9 | larger pictures built from the common sprite tiles (absolute tile numbers): mothership pieces and others **[H]** |

### 6.5a Layouts whose tiles look wrong **[C]**, **[V]** where noted

Frames whose tile numbers fall outside the tiles their range loads, or which have no range and use low absolute
tile numbers. None is a decoding error; they fall into four groups.

| Frames | Explanation |
|---|---|
| A$BF (satellite, type 60) | the count byte says 12 entries but the picture has 10; entries 11 and 12 are read from the next layout's bytes (A$C0) and give tiles base + $41 and base + $EE (mod 256), i.e. tiles of other sets. The original draws all 12 **[V]** (OAM on level 7 showed both extra sprites). Most likely a data slip; a remake that does not reproduce glitches draws the first 10 entries |
| A$AF-$B8 | use absolute tiles $00-$5C: on level 26, whose own sprite tiles are loaded at the bottom of the table instead of object sets (`graphics.md` 5.7.5): $AF-$B3 (types 4F-58 and the pieces of a shot object) draw the terrain tiles $2D-$5C copied into table 0, $B4-$B6 (type 5E) tiles $0A-$16, $B7/$B8 tiles $00-$19 (not seen in play). On other levels A$AF-$B8 show unrelated tiles and were not seen |
| B$10-$12, B$54, B$6D, B$6E, B$9A-$9C, B$C1-$C7 | absolute tiles below $41: the pod set ($01-$40). B$54 is the pod picture (item kind 1); the others are pod pieces and small pod-set tiles |
| B$80-$99, B$9E, B$9F, B$A1-$AE, B$C8-$D9, P$1E, P$1F | absolute tiles $00-$C0 that match none of the gameplay sets; they are meant for a scene that loads its own sprite tiles at those numbers (the mothership launch loads sprites at $0000-$08FF, `graphics.md` 5.6) **[H]**; not seen in play on the planet levels |

Table A $10-$17 and table B $13-$17, $42-$50 and $5F have no layout at all (6.1) and are never used.

### 6.6 Sprite order in OAM

Not needed by the remake (which has no sprite limit) but useful when comparing screens: OAM entry 0 is a fixed
marker sprite used to time the status-panel split; the pod uses entries 1-4; objects follow from entry 5 (entry
8 on level 26), slots 1-30 in ascending order on one build of the sprite table and descending on the next (it is built once
per logic step, `game-flow.md` 1.2), each object's sprites in
layout order; unused entries are hidden (Y = $F8). **[V]** entries 0-4, **[C]** the rest

#### 6.6a How the table is built: the 64-entry limit, skipped sprites, stale entries (2026-10-20)

Everything here is a **hardware limit or a glitch of the original's sprite builder** (`PLAN.md`, "Fidelity": no sprite limit, no
flicker); the remake does not copy it. It is written down because it explains pictures of the original that differ from a drawing
made from the objects' state, and so that a comparison can predict them. **[V]** against the OAM of `wormhole.replay`,
`wormhole-jetman.replay`, `wormhole-warp.replay` (every one of the 380 + 380 + 190 steps in the gate state matched the rule below) and `level8.replay`
(steps 1597-2500); the loop itself **[C]** (bank 1, the object drawer).

- **Capacity.** The sprite table has 64 entries of 4 bytes. The object loop writes sprites at entry 5 on (entry 8 when RAM $9A bit 7 is
  set: level 26), one entry per sprite written, in the object order below. **When the entry after 63 is reached
  (the write position wraps to 0), the whole build stops at once**: the rest of the current object's sprites and all objects not yet
  reached are not drawn, and nothing is hidden or rewritten afterwards (the entries just written stand). A build that does not fill the
  table ends by hiding every remaining entry (Y = $F8). So 59 sprites fit besides entries 0-4.
- **Order.** The loop goes through slots 1-30 **ascending on one build and descending on the next**, alternating on every call
  (one call per logic step, in the B pass), the first call after a reset going ascending **[C]**. Which parity a given step has depends on
  the number of builds since the reset; it is not tied to the step's number. Objects drawn first keep their sprites when the limit
  is hit; the last ones lose theirs (a partly drawn object shows the first sprites of its layout, in layout order: for the star layout $BC the two
  lower sprites and one upper, not the fourth).
- **Skipped sprites take no entry.** A sprite whose screen x or y has a non-zero high byte (outside -0 ... 255 as 16-bit; 3.5) is not
  written and the write position does not advance, so the next sprite takes its entry: an object's drawn sprites are packed.
- **The pod's own drawer (slot 0, entries 1-4) skips the same way but never hides the entries it did not write.** The entries
  after the last written one keep what an earlier build left there: **stale sprites**, at the pod's old screen position and with its old
  tiles. This shows when the pod is partly or wholly beyond the screen's right or left edge. The one case seen is
  `level8.replay` from step 1597, a typing slip of the test setup (warp "1198 1923 940 1812" puts the pod 258 px right of the camera
  instead of 148 like every other warp; the camera did not follow it: its follow rule moves the camera only while the pod's offset from the camera has a zero high byte
  (0-255 px) **[C]**, and the camera stayed put for all 900 steps): the two left sprites at x = 254, the two right ones skipped (x = 262) and entries 3-4 still the previous
  pod's top row at (144-152, 107); the shield ring (slot 2, 4 sprites around the pod) likewise shows only its sprites with x below 256
  (one at 251). Not a rule the remake lacks: the remake draws the visible parts of the pod and the ring and nothing else. The
  frame $FF pod hides entries 1-4 explicitly (Y = $F8), so only partial off-screen positions leave stale entries.

**Where the limit was met.** In the gate state (`player.md` 9.2.3), while both star groups are alive (16 stars = 64 sprites, plus the gate's 4, against 59; 138 of the 193 steps of `wormhole.replay`'s first gate state), every build loses
sprites: the **gate object (slot 11, item kind 18, frame B$52, four sprites) is drawn only on the builds that go ascending** (it comes
first, before the stars of slots 15-30; odd steps in `wormhole.replay`, steps 29 and 31 yes, 30 and 32 no) and **never on descending ones**
(it comes last, after 59 star sprites): the picture flickers. The stars lose the last star or two on every step: 13 complete stars and 3 sprites of the 14th on ascending
builds, 14 and 3 on descending ones; the dropped ones are the highest slots on an ascending build, the lowest on a descending one (and the gate). That
partial star (three of four sprites) is the "torn" star of those pictures; no star's sprites sit at different positions otherwise
(the sprites of every complete star in all three replays share one position). The remake draws all 16 stars and the gate on every step.

## 7. Animation

Frames are changed by each behaviour, mostly by counting frames; there is no general animation-script format.
Durations below are in **logic steps** (one step = one A pass + one B pass = 2 video frames, `game-flow.md`
1.2); frames change only in A passes. **[V]** measured in Mesen (as video frames, halved); whether rest periods
depend on hidden timers is **[H]**.

| Objects | Sequence (durations in logic steps) |
|---|---|
| Pod | frame = direction 0-31; one direction per step while left/right is held |
| Spinning objects 01-10 | rest on the base frame, then base+1 (4), base+2 (12-24), base+1 (4), back to base; rest periods of 15-45 vary |
| Small blocks 1F / 20 | two frames alternating ($7E/$7F, $80/$81), 2 steps each |
| Green creatures 43-46 | two frames alternating, 4 steps each |
| Formation parts 33-42 | through $91-$95 and back, 16 steps per frame |
| Formation parts 23-32 | through $8D-$90 and back, 16 steps per frame |
| Extending pillar 16 | $68 to $7D, one frame per step |
| Blue sphere 4A | $9E (about 22), $9F (4), $A0 (4), $A1 (about 20) |
| Egg 5F | $9A → $9B → $9C → $9D, 16 steps each, then it is removed (nothing hatches) |
| Wide machine 1D | $3F (16) → $40 (9) → $41 (≥ 20) |
| Explosions | 4 steps per frame (2.5) |
| Pick-up sparkle (table B, state $0B) | $6C, $6C, $6B, $6B, $6A, $6A, $69 (one per step: 2 steps per frame), then freed **[C]** |
| Rings, item kinds 17 and 26 (state $0E) | $67 and $68 alternating, **4 steps each** (period 8): frame = $67 + bit 3 of the frame counter, which grows by 2 per step **[C]**, [V] 2026-10-17 (the earlier "8 steps, bit 4" was wrong) |
| Star marker, item kind 27 | $7E and $7F alternating, 2 steps each, following bit 2 of the frame counter **[C]** |
| Countdown digits (warp levels) | B$73 (10) down to B$7D (0): the frame is $73 + (a step counter ÷ 64), so one digit per 64 steps (128 video frames, about 2.1 s); when it passes B$7D the level ends (transition code 6, `game-flow.md` 7.4) **[C]** |

## 8. Collision sizes (bank 1)

Two 193-byte tables give a half-width and half-height in pixels for every table-A frame; the program uses them
for collisions between the pod (and its shots) and objects in slots 15-30, always added to the pod's own half-sizes (3 each normally; 2 for shots; `behaviour.md` 3.1, 3.4), and to choose the explosion size of
a destroyed object in slots 15-30 (half-width + half-height < 8: small, < 28: medium, otherwise large; objects in
slots 1-14 always get the large one). **[C]**

| Bank 1 | ROM | Content |
|---|---|---|
| $8685 + frame | 0x08685 | half-width |
| $8746 + frame | 0x08746 | half-height |

Examples: turrets 01-10 (frames $18-$32) 5-8 × 5-9; domed installation $34-$36 20 × 21; extending pillar
$68-$7D 8 × (6 + 2 × (frame − $68)). Per-state flag tables follow at $87F8 (states $00-$24) and $881D-$8857
(states $80-$BA, index = state AND $7F); they are behaviour data (damage and collision rules) for a later spec.

**Overlaps.** The half-height table ($8746, 193 bytes) runs to $8806, so its last 15 entries (frames $B2-$C0)
are the same bytes as the flag entries of states $00-$0E at $87F8-$8806; likewise the flag entry of state $25
would be the entry of state $80 ($881D). Both readings give plausible values (e.g. half-heights 2, 3, 4 for the
growing orb $B9-$BB, 16 for the satellite $BF), so the sharing looks deliberate packing, but nothing depends on
it. An implementer should extract each table separately at its own address and length (half-heights: 193 bytes
from $8746; low-state flags: 37 bytes from $87F8; high-state flags: 59 bytes from $881D) and store them as
independent arrays; the shared bytes then simply appear in both. **[C]**

## 9. How this was verified

- Placement: every level's list decoded; in Mesen the pod and camera were moved next to each placed object of
  every level in turn (approaching from just off-screen) and the object table logged for 100 frames. 593 of the
  963 objects spawned (all types except 15). All had the decoded list index, type and initial state; 559 also
  had exactly the decoded position and the hit points of 2.4 when first seen; the other 34 were moving objects
  first seen a step after spawning (or already hit).
- Layouts: the sprites predicted from x, y, frame, flags and the tile bases were compared with OAM for every
  static object on level 0 (391 of 395 frames matched exactly up to a camera jitter of ±3 pixels).
- Sprite sets: RAM $05CF-$05FE after the start of level 0 matched 5.2.
- Pod rotation and speed fields: RAM logged while flying.
- Frame catalogue: every layout was rendered from CHR and palette dumped in Mesen on a level that loads its
  tiles.

## 10. Items (the level header's items list)

Besides the placement list (section 3) every level has a second, smaller object list reached from its level
header: the **items list**; its entries are **items** and their type byte is the **item kind** (decimal, as in
`economy.md`). The list format, RAM copy, activation window, spawn scan and per-kind initial frame are specified in
`levels.md` 3.3; what each kind is and what delivering it yields is in `economy.md` 2.1. This section covers how
items behave as objects once spawned. **[C]** unless marked; **[V]**: in the survey of section 9, 227 items
appeared in slots 11-14 with the list index, kind, position and initial state that the list and the two tables
give.

### 10.1 Relation to the placement list

| | Placement list (section 3) | Items list |
|---|---|---|
| Source | bank 4, per-level tables | bank 2, level header bytes 24-26 |
| Type numbers | object types $01-$61 (4.1) | item kinds 1-27, a separate numbering |
| Slots | 23-30 | 11-14 |
| Scan | B pass, up to 8 entries, window from header byte 10, only off-screen | A pass, up to 4 entries, fixed ±48 window, may appear on screen (`levels.md` 3.3) |
| RAM state | type-list copy $0460 (0 = gone) | kind copy $0513 (0 = gone) and positions $052D-$0594 |
| In the slot | state from 4.1 (bit 7 set), $03B2 = object type, $0393 = list index | state from bank 2:$86CD, $03B2 = item kind, $0393 = list index |
| Memory across visits | 3.6 | 10.4 |

- The byte at bank 2:$86CD is the item's initial **state** ($0317): $0A for kinds that sit still and can be
  towed or touched, $0E for the rings (kinds 17 and 26), $07 for the mothership indicator (kinds 21, 22, 24),
  $82 for kind 2 (never in a list).
- Items get no hit points (2.4) and are never shot to pieces. They leave play by being used up (kind set to 0 in
  the RAM list) or by leaving the window (3.5), after which the scan brings them back.
- **Initial values of a spawned item.** Flags X, flags Y and the four speed/work bytes (work 1-4) are 0; work 6 is
  0 except on warp levels (RAM $9B bit 7), where it is left as it was; work 5 is not written, so it keeps what
  the slot's previous occupant left. Work 6 of a towed item is set to 1 by the mothership code when the item
  reaches the mothership's column while above the screen (it is then pulled down into the bay), which is what
  keeps it alive off-screen under 3.5. **[C]**
- At the start of play the items scan is called once in the level-start pass, before the first player step: one
  call, up to 4 entries examined, at most one spawn (on planets, the mothership indicator of entry 0); the others
  appear over the next steps, one call per step (`levels.md` 3.3). [V] on level 0 in Mesen.

### 10.2 Behaviour once spawned

- **Still items (state $0A).** They have no movement of their own. While one is held by the tractor beam (RAM
  $66 = its slot), its RAM list position is overwritten every step with its current position, so a dropped item is
  found again where it was left, also after it despawned. Delivery is a transition (`economy.md` 2.2-2.3).
  **A still item that is being taken in** (taken-in flag RAM $0422 + slot non-zero) is moved by the pod's delivery
  test, not by itself: each pod step sets its vertical speed to 1.00 downward and moves it 1 px, until it is in the
  delivery box. The tow cannot be released during this (Down, shield and destruction are ignored for it,
  `player.md` 8.1), so it is never left hanging; only an implementation that frees the beam in this state leaves it
  frozen. [C] [V] 2026-10-20.
- **Spinning crystal (kind 15 towed, `economy.md` 2.6).** A counter in RAM $6E is armed when the high nibble of RAM
  $69 is 4 and advanced each time it then becomes 7; each advance awards points (`economy.md` 1.1) and the ninth
  turns the item into kind 16 (frame B$66), adds a jetpod and a life, and records its list index in RAM $0100 (or
  $0101 if $0100 is already used) so that the change survives reloading.
- **Rings (kinds 17 and 26, state $0E).** Frames B$67/B$68 alternate every 4 steps (a period of 8; **[V]** 2026-10-17, kinds 17 and 26 in Mesen). Who may touch or tow a ring, and the teleport
  (kind 17: to the next kind-17 entry, forward and wrapping; kind 26: to (1058, 1404)), is owned by `economy.md`
  2.1 (kind 17 is towable only by the Nippon pod, type 1; the Italian pod, type 2, and a lone Jetman teleport by
  touching it; the plain pod does neither). A towed object brought within reach of a ring (other than kinds 1, 11
  and 17) requests transition code 1 (`game-flow.md` 7.4), as at the mothership (`economy.md` 2.2); a ring that is
  itself being towed is not a delivery target. On planets 0-12 Jetman alone (player state 2 or 3) reaching a ring
  requests code 2 (back in the mothership).
- **Touched items (pod contact, nothing towed).** Kind 18 requests code 3 (change level within a planet; the pod,
  or Jetman alone, touching it with the shields off and nothing on the beam; the rules, the bounce of a closed gate with
  its sparkle in slot 3 (the state $0B ring above) and the gate state: `player.md` 9.2). The only kind that reaches the generic
  rule is the junction exit (kind 20): it requests code 5 with destination RAM $DB = the item's list index, which is
  how the exits on levels 27 and 28 choose their destination (`game-flow.md` 7.4 remap table). Every other kind
  is either towed instead of touched (kinds 3-6, 9, 10, 12, 15, 16, 19, and 11 by an upgraded pod; pod contact
  alone does nothing, and on warp levels, RAM $9B bit 7, the tow kinds are collected by touch, `economy.md`
  5.4), touched with its own rule (1: Jetman alone, 17, 18, 26, 27), or inert (21, 22, 24). Per-kind
  interactions: `economy.md` 2.1. Kind 27 (star marker): on level 7
  it moves play to level 12, elsewhere it is used up and requests code 7 (touch box 13 px each axis like the other
  touch-only kinds; used up = the slot is freed and its RAM kind set to 0, so it is gone for the visit).
- **Star marker (kind 27, state $0A) rises and vanishes by itself.** Every logic step, while its slot exists and
  it is not the towed item: frame = $7E + (bit 2 of the frame counter RAM $5D) (alternating every 2 steps); flags Y
  = 0 (moving up); the fraction of its vertical speed (RAM $02D9) grows by 4/256 (cap 3.00, never reached); if
  that fraction is then $E0 or more, the slot is freed and the RAM list kind set to 0 (gone for the visit, like
  a touch); otherwise it moves up by its vertical speed and its position is written back to the RAM list like any
  item (10.2, first bullet). The speeds are zero when the slot is filled, so a marker lasts 56 steps (about 112
  frames) from the call that spawns it and has risen by about 24 px by then; leaving the window and coming back
  starts the 56 steps again from the list position it had reached, until it has been used up or has vanished once.
  There is no horizontal movement. [C]
- **Pull-back of the fourth slot (bank 2 entry $8027).** An unheld item in slot 14 (or in slot 13 while the beam holds slot 14) with slots 11 and 12
  both occupied moves 1 px per axis per step toward its ROM list position, and the RAM list entry is overwritten by that position
  every step (`behaviour.md` 6.1, Q23). **[V]** 2026-10-17.
- **Mothership indicator (kinds 21, 22, 24, state $07).** No behaviour of its own; its frame ($FF, B$8A or B$8C)
  is a marker at the home position, the mothership picture itself being part of the map.

### 10.3 Items that are not in the list

The small pickups (`economy.md` 2.5: frames B$55 and B$64, states $24 and $25) are created in a free slot of
11-14 when an object whose state has flag bit 3 (section 8) is hit; they start at that object's position with
type byte 0 and list index field $82 (no list entry), so using them up does not touch the list. **[V]** 2026-10-17
(`economy.md` 2.5, `behaviour.md` 3.3 and 5.10): they stand still for 256 steps, are collected by touch (13 px) or by a shot, and are not
caught by the tractor beam. The field $03B2, which is the item kind for list items, is their step counter; a bonus object
with a list index in the field $0393 (instead of $82) would be taken for that list entry and caught by the beam, which does not happen in play.

### 10.4 Memory across visits

After the list is copied to RAM, the program looks up the level in a record list kept in RAM from $060E (records
of the form: level index, number of entries `b`, then ceil(b / 8) bytes of one bit per entry, entry 0 in bit 7;
the list ends at a byte with bit 7 set, initially $FF at $060E). For the level's record, every entry whose bit is
0 has its RAM kind cleared (used up), except kind 11, which is never cleared this way, and kind 18 on planets,
which is cleared only when a per-planet flag in RAM $DE/$DF is set; for entries whose bit is 1, a kind 19 is
cleared when that flag is set. Which code writes the records was not found (for the game-flow analyst). **[C]**

### 10.5 Level 26

Level 26's header points to level 25's items list (`levels.md` 3.3): kinds 17, 18 and 19 at (118, 252),
(368, 384) and (640, 96). In every other second level the kind-18 entry stands exactly at the home position; here
the home is (96, 1024), 640 px below it, and level 26 is unlike the other levels (its own sprite tiles, no sprite
sets, 47 placed objects of its own types). The list does not look designed for level 26; most likely the
pointer was reused as a placeholder. **[H]** Whether the player can reach those points was not checked.

### 10.6 Warp-level helpers (slots 23 and 24) **[V]**

On warp levels (29-41) the routine of `levels.md` 6.6/6.7 runs **every video frame** and keeps two "helper" positions in the
object arrays at slots 23 and 24. They are not drawn and have no state; only their position bytes matter. The warp-level mesh
tile (`graphics.md` 5.7.2) takes its offsets from slot 23's x and y low bytes AND 7. A flag byte per level
(bank 0 $A86E, index level − 29: levels 33 and 35 = $80, levels 37 and 39 = $C0, every other level $00) says what the helpers
do:

- **Bit 7 clear (levels 29-32, 34, 36, 38, 40, 41):** no helper movement. Slot 23's x and y low bytes and slot 24's are *set* every
  frame from the per-level source tables, as `graphics.md` 5.7.2 describes (constants, camera, pod and frame-counter
  values).
- **Levels 33 and 35 ($80):** slot 23 **homes on the pod** every frame (`behaviour.md` 2.3, both axes, x first, each followed by its
  move): accel 2/256 per frame, maximum speed $50/256 = **0.3125 px per frame**; the tables are not applied. Start: x 0,
  y 16, speeds 0, direction bits right and up. Checked: the helper's x low byte was 6, 18, 31, 43, 56 at frames 40, 80, 120, 160, 200
  (the closed form 2 n/256 up to 40 frames, then 0.3125 per frame) and the x low byte at frame 400 (118) matched the rule too (the 400 frames logged). The mesh
  therefore drifts slowly after the pod.
- **Levels 37 and 39 ($C0):** both helpers home: slot 23 as above, slot 24 with accel 3/256 and maximum $40/256 = 0.25 px per
  frame. **Then the tables are applied as well**, and their lookup uses a stale index (the index register
  still holds a value left by the homing code, so the lookup reads bytes beyond the 13-entry tables), which overwrites the
  helpers' low bytes with unrelated values every frame **[V]**. **The exact rule [V]** (round 13: the four lookups
  recomputed from the ROM and the live RAM on every call, levels 37 and 39, 701 and 702 calls with the pod thrusting, no
  mismatch): let `Y` be the x low byte of slot 24 **after its homing move of that frame** (the move's own result is what the
  index register holds). Instead of the table entries of the level, the four source bytes are the PRG ROM bytes at
  0x287B + Y (slot 23 x), 0x2888 + Y (slot 23 y), 0x2895 + Y (slot 24 x) and 0x28A2 + Y (slot 24 y), Y = 0-255, which run
  past the 13-entry tables into the bytes that follow them in bank 0 (the program's own data and code; a model that wants the
  original shimmer reads them from the ROM by offset). Each is decoded with the rule of `graphics.md` 5.7.2 (value 0-7, scale)
  and written to the helper's low byte. Slot 24's x low byte is itself one of the written values, so the next frame's `Y`
  is the value just written, plus or minus 1 on the frames where slot 24's own homing move crosses a pixel (1,401 consecutive pairs on the two
  levels: equal 1,057 times, off by one 344 times, never otherwise). The helpers' homing
  state (speeds, fractions) is not touched by the write, only the low bytes. The mesh offsets jump between values that look arbitrary (samples
  every 25 frames on level 37 while the pod flew: x low bytes $E5, $32, $05, $13, $05, $13, $05, $32, $03, $69, $05, $77, ...; $05 is the
  constant the tables give for source 0). A slip of the program, **Q38** in `behaviour.md` (awaiting Alex); the effect is only a
  shimmering of the mesh pattern. Proposed for the remake: the level-33 behaviour for these two levels (a `docs/deviations.md` row for Alex to approve).

## Open questions

- Behaviour of the invisible generators (48, 49, 4B-4E) and of every enemy; which states fire, home in or patrol.
  The routine at bank 4 $8057, which reads the placement record as 8-byte waypoint pairs, is used only by state
  $96, which nothing creates (`behaviour.md` 5.9).
- Meaning of work 5/6 per state, and the low states not listed in 2.4.
- What RAM $69 counts during the crystal spin (10.2), and which code writes the item records at $060E (10.4).
- The egg (type 5F) appends a kind-20 exit with list index = the old count: 21 on level 3, which matches the
  `game-flow.md` 7.4 remap pair (21 from level 3), but 6 on level 0, where the remap pair is 5 from level 0.
- Level 26's items list (10.5) and the P$16-P$1B pictures (6.4) were not checked against the running game; level 26's own sprite tiles, tile bases and pictures are now (6.4a, `graphics.md` 5.7.5).
