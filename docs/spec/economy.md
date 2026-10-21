# Economy and progression

The original's rules for score, item deliveries, fuel, damage, lives and jetpods, the warpship pieces and the order
of planets, and the shop. Physics (thrust, towing, speeds) belongs to the physics spec; this file says only which
RAM flag each piece of equipment sets and what it changes in the rules. Scene changes are numbered as in
`game-flow.md` 7.4 ("transition 1" etc.). Message texts and ids are in `text.md` section 3.

Status: **[V]** verified in Mesen (state forced by writing RAM in gameplay, then RAM and screens read back),
**[C]** read from the program, **[H]** hypothesis. Addresses follow `README.md`. "Per step" means per gameplay logic
step (two frames, `game-flow.md` 1.2).

## 1. Score (the money)

- **Storage:** six decimal digits in RAM $CB-$D0, most significant first, plus a sign flag in RAM $CA (bit 7 set =
  negative). **[V]** digits, **[C]** sign
- **Arithmetic:** decimal, digit by digit. Adding to a negative score subtracts from its magnitude and clears the
  sign when the result crosses zero; subtracting works the same way the other way round. Nothing in the game was
  found that makes the score negative: the only subtraction is a shop purchase, which is refused when the score
  is too low (6.3). A carry out of the top digit is lost (the score wraps past 999999). **[C]**
- **Display:** the status panel shows the six digits in a large two-row digit font, with a minus sign in front
  when negative. It is redrawn by one of the rotating HUD jobs in gameplay (`game-flow.md` 1.2); message screens do
  not redraw it, so a value added during a message appears when play resumes. **[V]**
- **Money:** the shop spends the score directly ("IN YOUR ACCOUNT", section 6). **[V]**
- **New game:** 001000. The password keeps all six digits (`password.md`). **[V]**

### 1.1 Every way of scoring

| Event | Points | When added | Status |
|---|---|---|---|
| Alien item delivered | 1000 | 64 frames into its message screen | V |
| Radioactives delivered | 750 | 64 frames into its message screen | V |
| Cargo delivered (analysis screen) | per cargo, table 2.4 | 64 frames into its message screen | V (cargo 2: 194) |
| Crystal delivered | 500 | at the delivery | V |
| Crystal delivered that completes a set of seven | 2000 (instead of 500), plus a life and a jetpod | at the delivery | V |
| Warpship piece, equipment pickups, fuel | 0 | | V |
| Crystal spun on the tractor beam (2.6) | 25, 50, 100, 200, 400, 1000, 2000, 5000 for swings 1-8; swing 9 gives a life and a jetpod | at each counted swing | C |
| Small pickup touched by the pod or hit by a shot (2.5) | 25 (box form, which also tops up the W gauge) or 50 | at the touch or hit | C |
| Crystals collected on a warp level (5.4) | 100 each, 1100 for the tenth | during the badge intermission, one every 16 frames | C |
| Placed object destroyed (object slots 23-30 only) | 3 for a small explosion, 5 for a medium or large one; the amount depends only on the explosion size, not on the enemy (`objects.md` 2.5 owns this rule) | when the explosion starts | C, V (`objects.md` 2.5) |

Score is added in exactly these places: the delivery value and crystal bonuses (2.3, 2.4), the spin points (2.6), a
pickup touch (pod, 2.5) or a pickup shot (`behaviour.md` 3.3), the badge intermission (5.4) and an explosion of a
placed object. Slots 1-22 (shots, mines, drones, items) give no kill points. Points use the same decimal addition
everywhere, so a total over 999999 wraps.

## 2. Items and deliveries

### 2.1 Item kinds

Each planet level has a second object list, in its level header (`levels.md` 3, 3.3): x, y and a **kind**. These are
the items and fixtures the mothership cares about. At level start the list is copied to RAM (x low $052D, x high
$0561, y low $0547, y high $057B, kind $0513, one entry per index; count in RAM $BA). An entry whose kind is set to
0 is gone for the rest of the visit. While near the camera an entry occupies one of the object slots 11-14
(`objects.md` 2.2) with its list index in the slot's list-index field and its kind in the slot's type field. Its
initial frame and state come from two tables indexed by kind: bank 2 $86B1 (frame, table B) and $86CD (state) (the values per kind are in `levels.md` 3.3; `behaviour.md` 6.1 lists them again).
**[C]**, list contents **[V]** by decoding

| Kind | What it is | Interaction | Delivered gives |
|---|---|---|---|
| 1 | spare pod (frame B$54) | Jetman alone (player state 2 or 3) touching it, both axes within 13 pixels, gets back into a pod with a full fuel tank (`player.md` 6.3); only the Italian racing jetpod (pod type 2) can tow it | never delivered |
| 2 | small pickup (frames B$55 and B$64) | not a header kind: see below | not towable |
| 3 | probe shields | towed | shields (2.3) |
| 4 | probe boosters | towed | boosters |
| 5 | fancy alien item | towed | 1000 |
| 6 | navigators | towed | the mapping device |
| 9 | cargo container | towed | analysis screen and its value |
| 10 | fuel | towed | fuel count of the mothership (section 3.4) |
| 11 | fuel pod (heavy object) | towable only by an upgraded pod (type 1 or 2) and never delivered; shooting it destroys it and sets the fuel's high byte to $E0 (full), leaving the low byte, so the tank holds $E000 plus its old low byte; harmless, `behaviour.md` 3.3 and Q26 **[C]** | |
| 12 | radioactives | towed | 750 |
| 15 | crystal (orange) | towed | crystal (2.4) |
| 16 | crystal (blue): a kind-15 crystal after nine spins (2.6), also a warp-level crystal | towed / touched | crystal |
| 17 | transporter ring | see 2.2. With the Nippon sports jetpod (type 1, bit 0 of $DC) in the flying state the ring is towed; with the Italian racing jetpod (type 2) or any other player state, touching it (both axes within 13 pixels) jumps the pod to the next kind-17 ring in the list (searching forward and wrapping; nothing if it is the only one). The plain pod (type 0) can do neither **[C]** | never delivered |
| 18 | wormhole | touched: changes level (section 5) | |
| 19 | warpship piece (in the second levels 13-25) | towed | piece (5.1) |
| 20 | exit of a junction level (27, 28) | touched: transition 5 to the level named by its list index (5.3) | |
| 21, 22, 24 | the mothership's fuel indicator; always list entry 0 on a planet. Its kind is the fuel count (3.4) | | |
| 26 | special ring (planet index 1 only): touched, puts the pod at a fixed position (x 1058, y 1404) and restarts the level there. It disappears 512 frames after the level starts | | |
| 27 | star marker: touched, starts the backdrop interlude (5.4), or on planet index 7 the shortcut to planet 12 (5.2). Used up by the touch; if not touched it rises and vanishes for good 56 steps after it appears (`objects.md` 10.2) | | |

Kinds 7, 8, 13, 14, 23 and 25 are in no list **[V]** by decoding; they behave as the table below the list (verified by writing them into a list in Mesen). **Kind 2** is not a list kind at all: the small
pickups are made during play when a placed object with the pickup flag is hit (`objects.md` 10.3, `behaviour.md`
5.10). Such an object has type byte 0 and the list-index field $82; bit 7 of that field means "no list entry" and
the low bits (2) are what the interaction code reads as its kind. It is one description: `objects.md` 10.3 and this
table agree. **[C]**

**Mapping of kinds to placement:** on planets (levels 0-12), a kind-15 entry whose list index equals RAM $0100 or
$0101 is loaded as kind 16 (those bytes remember up to two crystals already turned blue; rules in 2.6). **[C]**

**Run in Mesen, 2026-10-17 (item kinds checked one by one).** Each kind was put into a level's items list by writing its
RAM entry (kind, x, y) and the pod was held next to it or let fall onto it. Kind numbers are decimal here; the
hexadecimal value, as `behaviour.md` 6.1 writes it, is in brackets. **[V]**

| Kind | Frame, state | Beam | Load class ($67) | Other result |
|---|---|---|---|---|
| 3, 4, 5, 6 | B$59, B$5A, B$58, B$57; state $0A | caught | 1, 1, 1, 2 | as 2.3 |
| 7 | B$5A; $0A | caught | 2 | delivered: message 1, text 6 then 13, nothing else (no score, no flag), list entry cleared |
| 8 | B$5B; $0A | caught | 0 | the same, texts 6 then 14 |
| 13 ($0D), 14 ($0E) | B$62, B$64; $0A | caught | 0, 0 | the same, texts 6 then 19 and 6 then 20 |
| 19 ($13) | B$53; $0A | caught | 2 | delivered at the mothership: message kind 5, text 47, the planet's flag set (RAM $DE bit 7 on planet 0) |
| 20 ($14) | B$52; $0A | not caught | | touched (both axes within 13): RAM $DB = the entry's list index, the level change runs at once (the exit of a junction level) |
| 21, 22, 23, 24, 25 ($15-$19) | B$FF, B$8A, B$8B, B$8C, B$8D; state $07 | not caught | | inert: standing on one does nothing (mothership parts) |
| 26 ($1A) | B$67/B$68 alternating every 4 steps; state $0E | not caught | | touched: the pod jumps to (1058, 1404) as in the table above (tried on level 0: the pod arrived at (1058, 1404) and two steps later stood at y 1023, probably moved by the shorter map of that level **[H]**) |
| 27 ($1B) | B$7E/B$7F; $0A | not caught | | touched: pod state 10 (the backdrop interlude), entry cleared; untouched: it rises and the slot is freed 56 steps after the call that placed it (the vertical speed fraction reaches $E0 on the 56th step), entry cleared; the frame is B$7F when (n / 2) is even and B$7E when it is odd, n counting the steps since placement (n = 0: B$7E) |

Delivering kinds 7, 8, 13 and 14 (not in any list) changes no byte other than the list entry and the message: a
whole-RAM comparison of kind 13 and kind 14 deliveries with kind 8 differed only in the sprite shadow, the list
kind and the load class. The unused kinds therefore have no effect; their messages are text 6 and then text kind +
6. The kind 19 piece, delivered on planet 0 (a test setting), set the flag of planet 0 ($DE bit 7).

**Items creep back to their list position (bank 2 entry $8027).** An unheld item in slot 14 (or in 13 while the beam holds 14) moves 1 px per
axis per step toward its place in the level's ROM list, and the RAM list entry is reset to that place, when slots 11 and 12
are both occupied (`behaviour.md` 6.1, Q23). **[V]**

**The beam's catch region (corrects `player.md` 8.1 by one pixel).** With the pod held still and a kind-3 item at
(dx, dy) from it, the item was caught for |dx| < 32 and |dy| <= rope[32 + |dx|] (the table of `player.md` 8.2), but
|dy| never above 31: at dx 0-5, where the table says $20 = 32, dy 31 is caught and 32 is not (above and below
alike). For example (0, 31), (10, 30), (20, 23), (31, 4) and (-31, 4) are caught; (0, 32), (10, 31), (20, 24), (31, 5)
and (32, 0) are not.

### 2.2 What counts as a delivery **[C]**, outcome **[V]**

An item counts only while it is held by the tractor beam (RAM $66 = its slot). Kinds 1, 11 and 17 are never
delivered. A held item is delivered in one of two ways:

1. **At the mothership.** The mothership's home position is x = RAM $D6 (low) / $D7 (high), y = RAM $D8 (low) /
   $D9 (high). The point tested is the item's own position (its object x, y). With dx = item x - home x and
   dy = item y - home y, the delivery box is |dx| <= 7 and -6 <= dy <= 31 (all bounds inclusive). When |dx| <= 7
   and dy is from -208 up to -7 (above the box), the item is snapped to the home x (whole pixel; the sub-pixel is not
   touched) and a lowering flag (work 6) is set, and that is all the snap does. **The speed of 1.00 downward is not set at
   the snap**: it is set, with direction down and one move of 1 px, by the delivery test of the **next** step, which
   finds the flag set before it tests the box (vertical speed 1.00, fraction 0, flag Y = down, then the move), and it
   sets it again on every following step while the flag stays. The flag is set again by every snap. No horizontal speed
   is written by this code. **[V]** 2026-10-17 (kind 3 item towed and moved above the mothership: the flag written on one
   step, the speed 1.00 written on the next, position +1 px per step from then); **[C]** for the code. The test is in the
   pod's step, not in the item's own state routine.
   **Once the flag is set it stays set** and the item is lowered every step (the code that sets the speed, direction
   down and moves it by 1 px runs before the box test, on every pod step while $66 = the slot) until the item is in the
   delivery box (-6 <= dy <= 31) and the transition is requested. **The tow cannot be released meanwhile**: Down, the
   shield and the pod's destruction all do nothing to a held item whose flag is set (`player.md` 8.1, 7.1), so the
   item always reaches the box and is delivered. **[C]**, and **[V]** 2026-10-20 (Down pressed and shield switched
   on during the lowering; item delivered 1 px per step later in both). The remake must not free the beam on a
   release in this state; one that does leaves the item frozen, because a still item has no movement of its own
   (`objects.md` 10.2) and the lowering belongs to the pod's step, which only runs while $66 holds the slot.
   - **Fuel** (kind 10) is handled here without a message unless it is the one that fills the tank (3.4).
   - Every other kind requests transition 1.
2. **Through a transporter ring** (kinds 17 and 26): the held item's position is within 15 pixels of the ring's on
   both axes (|dx| <= 15 and |dy| <= 15, inclusive). This requests transition 1 for any deliverable kind, fuel
   included, anywhere a ring exists (rings are how the warpship piece leaves a second level). Kinds 1, 11 and 17
   never count. A ring that is itself being towed (Nippon pod) is not a delivery target. **[C]**

A Jetman on his own (player state 2 or 3, nothing held) within 15 pixels of a ring on a planet (levels 0-12) is
carried home instead: transition 2 (dock), with the repeat guard cleared. **[C]**

**The bay area.** The pod's x is within -13..+12 pixels of the home x and its y within -16..+39 pixels of the home
y (dx, dy as above, pod position). While this holds, bit 5 of RAM $98 is set (the pod is drawn behind the
mothership); otherwise it is cleared. This one condition is "inside the mothership's bay area" everywhere in the
specs (`game-flow.md` 7.4). A pod with nothing held whose dy is from 0 to 7 inside the x range requests transition
2 (dock) directly; with an item held, that strip leaves bit 5 unchanged and the delivery test runs instead. **[C]**

### 2.3 Transition 1: what the delivery does **[V]**

The objects are frozen, the message screen opens (RAM $DA = 240 frames) and the kind decides:

| Kind | Effect at once | Message kind $DB | Text (`text.md`) |
|---|---|---|---|
| 3 | RAM $DC bit 7 set (probe shields) | 1 | 7 |
| 4 | RAM $DC bit 6 set (probe boosters) | 1 | 8 |
| 5 | | 1 | 45 |
| 6 | RAM $060C = 1 (the mapping device) | 1 | 9 |
| 9 | cargo number k chosen (2.4) | 3 | 27, then 28 + k |
| 10 | fuel count + 1 (3.4) | 1 | 21, or 22 when the tank is now full |
| 12 | | 4 | 46 |
| 15, 16 | crystal (2.4) | 2 | 25, or 50 for the seventh |
| 19 | planet flag set (5.1) | 5 | 47, or 48 when it was the last piece |
| others (7, 8, 13, 14) | (not reachable in normal play) nothing | 1 | 6, then kind + 6 **[V]** |

- **64 frames** into the message screen (frame counter $5D reaching 64), for message kinds below 6: the kind's
  value (table 2.4) is added to the score, the item is released (RAM $66 = 0) and its list entry is cleared, so
  it never comes back. **[V]** The cargo number k lives in RAM $68, which is also the rope angle's fraction while an
  item hangs on the beam; the item is released at the end of the screen so the two uses do not meet.
- The screen ends when its timer runs out, or on any newly pressed button once the timer is below 64.
  `game-flow.md` 7.4 describes the timer and what follows. **[V]**
- **After the screen [C]:**
  - if all twelve planet flags are set: the final level (5.5);
  - else, if RAM $98 bit 5 is set (the bay area, 2.2): a Jetman on his own docks; a pod docks if this planet's
    flag is set (so delivering the piece lands the pod with BLAST OFF available);
  - otherwise play resumes where it was.

### 2.4 Values and crystals

**Value table** (bank 0 $D52A, ROM 0x0552A): three bytes per kind, six decimal digits (two per byte, most
significant first). Kinds 0-19: 5 → 001000, 12 → 000750, 9 → 000450 (not used: cargo has its own), 1 → 004000,
2 → 006000, 0 → 000260 (kinds 0-2 are never delivered); all others 0. **[V]** for 5 and 12

**Cargo** (kind 9). k = (the item's list index + level index $3B) mod 17, kept in RAM $68. k selects the analysis
list (text 28 + k), the cargo name (`text.md` 4.4) and the value, read from the same table at bank 0 $D566 + 3k:
**[V]** k = 2 gave 194

| k | Value | k | Value | k | Value |
|---|---|---|---|---|---|
| 0 | 423 | 6 | 554 | 12 | 93 |
| 1 | 618 | 7 | 663 | 13 | 2453 |
| 2 | 194 | 8 | 299 | 14 | 493 |
| 3 | 865 | 9 | 354 | 15 | 893 |
| 4 | 1 | 10 | 1287 | 16 | 4872 |
| 5 | 748 | 11 | 402 | | |

**Crystals** (kinds 15 and 16). RAM $E0 counts crystals stored, 0-6. On delivery: **[V]**

- if $E0 is below 6: $E0 + 1, 500 points, text 25 ("MORE CRYSTALS NEEDED FOR A BONUS LIFE"; no number is shown);
- if $E0 is 6: $E0 = 0, 2000 points, lives $3E + 1, jetpods $3D + 1, text 50.

So every seventh crystal earns a life (`game-flow.md` 7.4 and 9 must say 0-6 stored and the seventh gives the bonus,
and 25 as the fuel cap, section 3.4). $E0 is kept across planets, is not in the password, and is cleared when the
final level starts. **[C]**

### 2.5 Small pickups (kind 2) **[V]** 2026-10-17

Collected by touching the pod or Jetman, or by hitting it with a shot. The tractor beam does not catch them
(a pickup 27 px from the pod, inside the beam's region, was left alone).

- **Touch:** both axes within 13 pixels, inclusive: with the pod held still, pickups at offsets of 11, 12 and 13 on
  either axis (left or right, above or below, and the corner (13, 13)) were collected, offsets of 14 were not.
  The probe shields on or off make no difference; Jetman flying (player state 2) collects as well. **[V]**
- **Shot:** within -6..+5 on both axes (`behaviour.md` 3.3). **[V]** (box and star)
- **Effect, box (state $25, frame B$64):** score +25; the weapon energy high byte RAM $C9 rises by $1F, at most $E0
  (from $40: $5F; from $C0: $DF; from $C1 or more: $E0); a marker (state 6, `behaviour.md` 6.2) with field $0393 = 0 is
  made in the first free slot of 10 down to 3 at the pickup's position; RAM $D2 = ($D2 AND $80) OR $40 on the step
  of the pickup (read as $60 with bit 5 from the bonus-message routine, and $00 on the next step); sound request $42;
  the slot is freed. **[V]**
- **Effect, star (state $24, frame B$55/B$56):** score +50 (the score is kept as six digit bytes, RAM $CB-$D0, most significant first), $C9 unchanged, RAM $D2 = ($D2 AND $80) OR $41 ($61, then $01 from the next step),
  sound $42, the slot freed. By a shot the marker is made with field $0393 = 1 and floats up (`behaviour.md` 3.3).
  **By touch the marker is made with field $0393 = 0**, so it differs from the nibble 1 of $D2 and is freed on the
  next step before it ever shows (`behaviour.md` Q22).
- **No tag removal.** The earlier reading that a pickup whose tag differs from the low nibble of $D2 removes itself
  was not seen: pickups stood for their whole 256 steps with $D2 equal to $00, $01, $02, $40 or $41. The tag rule belongs to the
  markers alone (`behaviour.md` 6.2). Bit 6 of $D2 is cleared again by the next step. **[V]**
- **Lifetime:** a pickup left alone is freed after 256 steps (box: first seen at step 4, gone at step 259). **[V]**

### 2.6 Spinning a crystal **[C]**

Only a kind-15 crystal on the beam counts (not kind 16). The beam angle is RAM $69, 0-127 in 1/128 turn, 0 straight
above the pod, 64 straight below (`player.md` 8.2). Each pod step the count byte $6E is updated:

- if $69 is 64-79 (high nibble 4): bit 7 of $6E is set ("armed");
- if $69 is 112-127 (high nibble 7) and $6E has bit 7 set: the count is the low 7 bits + 1 and the arming is cleared.

So a "turn" is one swing from the 64-79 window to the 112-127 window, either direction (through 0 or through 96),
not a full circle; it must re-arm in the lower window before the next count. The points for the count n just
reached are the table 25, 50, 100, 200, 400, 1000, 2000, 5000 (n = 1-8, as decimal pairs of the middle and low
score bytes). Reaching n = 9: count back to 0, lives + 1, jetpods + 1, the crystal becomes kind 16 (frame B$66)
and its list index is stored in RAM $0100 if that is $FF, otherwise in $0101 (overwriting it, even if taken).
Every counted swing (including the ninth) also makes the floating points sign (`behaviour.md` 6.2): a marker with number
$0C AND $0F = swing count before the increment (0-8) in the first free slot of 10 down to 3, RAM $D2 = ($D2 AND $80) OR $40 OR
that number, whose picture is pair n of the pop-up pictures ("25" ... "5000", "1UP"; `graphics.md` 3.1). **[V]** 2026-10-20
(swing counts poked in Mesen: scores +50, +100, +200 (a crystal's swings 2-4), +400, +1000, +2000, +5000 and the 1UP with
lives and jetpods + 1, nibbles 1-8).
$6E is cleared when the beam takes an item; its low 7 bits are also read
as "swings so far" by `behaviour.md` 5.4.

**Blue crystals across visits.** $0100 and $0101 are set to $FF by START LEVEL (`game-flow.md` 7.2) and again at
every launch from the mothership (the same step that fills the gauges, 3.2). They are not reset by a death, a
pod change, a wormhole or a warp level. Only the list index is stored, so an index recorded on one planet would
also match the same index on any other level loaded before the next reset. **[V]** (a poked $0100 survived a
death and a level-13 transition) **[C]** for the rest

## 3. Fuel, the W gauge and damage

### 3.1 The status panel **[V]**

Exact tile positions, gauge cell rules and digit tiles: `graphics.md` 1.2.

| Row | Shows |
|---|---|
| "F" gauge (nametable row 4, column 2) | fuel, RAM $C6 (low) / $C7 (high) |
| "W" gauge (row 6, column 2) | weapon energy for the fitted special weapon, RAM $C8 (low) / $C9 (high) |
| Pod icons (rows 4-5, from column 16) | jetpods $3D, at most 4 drawn |
| Jetman icons (row 6, from column 16) | lives $3E, at most 4 drawn |
| Six digits (right) | the score |

- **Gauge drawing:** a gauge is 14 cells of 16 units, value 0-224 (the high byte). Each full 16 draws a full cell;
  the cell holding the remainder draws a partial cell (fill level = (remainder / 2) AND 7, eight pictures); the
  rest are blank.
- **F gauge:** it shows a display copy, RAM $05CE (high byte only), which follows $C7 by easing once per logic
  step, in the pod's drawing stage (the per-step routine after the objects' step, `game-flow.md` 9.2: it runs in every player
  state **including the gate state 9** **[V]**: with the fuel lowered during a gate the display kept easing). Let g = $C7 - $05CE:
  - g > 0 (moving up): add 1 when g < 64, else add g / 64 (1, 2 or 3 for g of 64-127, 128-191, 192-255);
  - g < 0 (moving down): subtract 1 when the gap is 16 or less, else 3;
  - g = 0: nothing.
  **[V]** (`delivery.replay`: 3, 6, 9, 12, ... up to the step; `gatefuel` test: +1 steps near the target, down by 3 and 1 on the way back.)
  - **At each launch from the mothership** $05CE is set to 0 while $C7 is $E0 (the launch's setup, together with $C6, $C8 and $0595) **[V]**.
  - **The extra step before the first logic step [V]** (2026-10-20, every replay's first gameplay pass): the level's set-up runs the drawing
    stage **once** (without an A pass: frame counter cleared to 0 just before) as the last thing before play starts (`game-flow.md` 7.3, "Level start"), so the
    gauge has already eased once when the first A pass begins: **$05CE = 3 at the first gameplay pass after a launch**
    (0 + 3, since g = $E0 = 224 gives 3), 6 after logic step 1, 9 after step 2, and so on (3 per step while g >= 192, then 2, then 1: e.g.
    $18 after step 7). The remake should start gameplay with the display copy at 3, not 0, or run the stage once at the level start.
    The same extra step happens at every reload (a respawn, a level change): there $05CE is not reset, and with $C7 = $05CE it changes nothing.
    The extra step also decrements the guard RAM $41 once when it is not 0 (`game-flow.md` 7.4), tests the bay (RAM $98 bit 5, so the first pass already sees it)
    and does the hit-pause bookkeeping of `game-flow.md` 9.2 (nothing at a level start: $99 is 0); on a warp level it also builds the
    mesh tile from the helper slots' leftover positions (`graphics.md` 5.7.2).
- **The terrain test can overwrite the display copy [V]** (2026-10-20, `level11.replay` steps 7,129-7,300). `$05CE` is also a byte of the terrain test's segment buffer (`levels.md` 6.4 step 1, "Storage and its overflow"): when the
  nine blocks around the pod hold so many segments that the buffer is 56 bytes or more ($B8 = $38+), the test writes a list byte into $05CE every step, in the player's step, **before** the drawing stage's easing. The easing then
  starts from that byte instead of the previous display value. In the replay the byte was $0F at every step and the fuel high byte $DE (g = $CF), so each step ended with $0F + 3 = $12: the display copy stays **$12 for as long as the spill
  lasts** (207 steps there) while the fuel is near $DE, and the easing resumes from $12 when the spill ends (the values then climb by the easing rule above and end at the fuel's value, $D8-$DF in the log). Not the easing's own rule
  and not intended: **Q45** (`behaviour.md`). Decided (Alex, 2026-10-20): the remake keeps the easing rule alone and never lets the buffer touch the display copy; its F gauge will differ from the original's picture during such a spill.
- **W gauge:** shows $C9 directly.

### 3.2 Fuel **[V]** for the rates

Full tank: $C7/$C6 = $E000 (224 units of 256). Both gauges are filled ($C7 = $C9 = $E0, low bytes 0) at every
launch from the mothership (after landing, EXPLORE, or the equip list). Other refills to $E000 (fuel only):

- when the pod is destroyed (Jetman starts with a full tank);
- when a new pod arrives after Jetman was lost;
- when Jetman gets into a spare pod (kind 1).

The fuel pod (item kind 11, shot) is the one refill that writes only the high byte ($E0): the tank then holds $E000 plus the
old low byte (`behaviour.md` Q26). **[C]**

Docking does not refuel by itself; the next launch does. **[C]** for the list

**Consumption**, subtracted from the 16-bit value each step, never below 0:

| Condition | Per step |
|---|---|
| thrust (B held) | 2 |
| thrust with EFFICIENT ENGINES fitted (RAM $D3 = 10) | 1 |
| boost: boosters owned ($DC bit 6), B and Select held | 16 |
| boost with EFFICIENT ENGINES | 8 |
| probe shields switched on (RAM $65 bit 7), in addition | 1 |
| idle, or Select alone | 0 |

At 2 units per step a full tank lasts 28,672 steps of thrust; it is damage (3.3) that empties it in practice.

**Empty tank [V]:** "empty" means the 16-bit value $C7:$C6 is 0 (both bytes). It is tested at the start of the
pod's (or Jetman's) step, before that step's consumption (`player.md` 7.1, 7.2): the gauge reaches 0 in one step
and the pod is destroyed on the next. Exception: a pod whose towed item is being lowered into the bay is not
destroyed. A pod hit that takes the fuel to 0 therefore always leaves one more step of play.

- **Pod:** the pod is destroyed (player state 4). Jetpods $3D - 1; the held item is dropped; the fitted special
  weapon $D1 and the fitted device $D3 are lost, and the probe shields are switched off; Jetman then flies on
  alone (state 2) with a full tank (`player.md` 7.1).
- **Jetman:** he is killed (state 5). The bullet upgrade $050E is lost. If lives $3E is 1 it is set to 0 at that
  moment. 96 steps (192 frames) later transition 4 runs (`game-flow.md` 7.4): if $3E is 0 the game is over with no
  further decrement; otherwise lives - 1, and the game is over if that leaves 0. So lives are decreased once in
  total, never twice. Otherwise a new pod arrives (state 7, then 1, full tank).

**Consumption is pod-only [V].** Jetman's jetpack (B held) uses no fuel, and falling to the ground costs him
nothing (terrain gives Jetman no damage outside the final level). A Jetman's tank empties only through contact
damage from objects (3.3), because he cannot raise probe shields. The probe-shield drain of 1 per step also
runs only while the pod's shield flag is set.

### 3.3 Damage **[C]**

Damage is taken from the fuel, in two separate ways. This file does not own the amounts; it only joins them.

- **Terrain impacts** (floors, walls, slopes): `player.md` 5.3 and `levels.md` 6.4 give the amount (speed-based),
  the multipliers (unshielded x4, shielded /2, super shields /4) and the 48-step hit pause RAM $99, which also
  blocks further terrain damage. No terrain damage on warp levels or when the fuel is already 0. Only the player
  uses the terrain test, and Jetman's own ground contact does not take fuel.
- **Contact with objects in slots 15-30** (enemies, their shots, hazards): `behaviour.md` 3.1 gives the order and
  the push; the fuel cost is, in units of the gauge's high byte (1 unit = 256 of the 16-bit value):

  | Object's collision flags (bits 4-5) | Shields off | Shields on |
  |---|---|---|
  | both clear | 64 | none |
  | bit 4 set | tank emptied | 32 |
  | bit 5 set, bit 4 clear | tank emptied | 8 |

  Flags are looked up by object state (bank 1 $879D for placed objects, $87F8 for the others). The result stops at
  0. These amounts are fixed per object kind; there is no per-hazard base amount, and the super shields (RAM $D3 = 8)
  do not reduce them. Contact does not set the hit pause $99 and is not blocked by it; it is blocked entirely while
  $97 (the invulnerability counter) is non-zero, during which overlapping objects are only pushed. Jetman takes
  contact damage like the pod, always unshielded. The contact test has no warp-level check, but there is nothing
  to touch: no warp level has a damaging object (5.4, "Warp-level hazards").
  A correction for the other specs: `behaviour.md` 3.1 says $97 is "set after a hit". Contact sets neither $97 nor
  $99; $97 comes only from Jetman emerging, a new pod and a gate (`player.md` 5.4). The routine that applies the
  x4 and the 48-step pause is used by the terrain test only.

**Probe shields** (RAM $65 bit 7): switched on by a new press of Down when $DC bit 7 is set (shields found) and the
level is not a warp level; switched off by a new press of Up. While on they cost fuel (3.2), and the tractor beam
cannot take items, and wormholes and exits are not entered. **[C]**, cost **[V]**

### 3.4 Enough fuel to leave: the mothership's fuel count **[V]**

List entry 0 of a planet is the mothership's fuel indicator, and its kind is the fuel count c. The level header
sets the start value:

| Planet index ($3B) | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Start c | 24 | 22 | 21 | 22 | 21 | 21 | 22 | 21 | 21 | 21 | 21 | 21 | 21 |
| Fuel cells needed | 1 | 3 | 4 | 3 | 4 | 4 | 3 | 4 | 4 | 4 | 4 | 4 | 4 |

The tank is full at c = 25, which is also the cap (c never exceeds 25). Fuel cells needed = 25 − start c.

| Fuel delivered | At the mothership | Through a ring |
|---|---|---|
| c < 24 | c + 1, no message (a sound plays); the cell is used up and the mothership's indicator picture becomes frame B$(new c + $74) | c + 1, message 21 "YOU NEED MORE FUEL TO LEAVE THIS PLANET" |
| c = 24 | c = 25, message 22 "YOU HAVE COLLECTED ENOUGH FUEL ..." | the same |
| c = 25 | the cell disappears silently, c stays 25 | message 22, c unchanged |

- **What c = 25 opens:** the planet's wormhole (kind 18). Touching it sooner bounces the pod back with a sparkle.
  The wormhole leads to the planet's second level, where the warpship piece is (section 5).
- **Saving c:** when the pod goes from a planet to its second level, c is saved in RAM $0636 and restored on the
  way back. It is reset to the header value whenever the planet is loaded afresh (a new START LEVEL).
- **Blast-off:** depends on the warpship piece (5.1), not on c.

## 4. Lives, jetpods and game over

| | Jetman lives $3E | Jetpods $3D |
|---|---|---|
| New game | 4 | 4 |
| Password | the saved value | set equal to the lives |
| Pod destroyed | | − 1 (it can go below zero: the byte wraps to 255) **[V]** |
| Jetman, alone, docks (transition 2 from a state other than 1 or 5) | | + 1: the refund for the lost pod **[V]** |
| Jetman killed | with 1 left, set to 0 at the kill; at transition 4: 0 = game over, else − 1 and game over if that gives 0 (3.2) **[V]** | unchanged |
| Seventh crystal, ninth crystal spin | + 1 | + 1 |
| Final level starts | set to 0 | set to 0 |

- **Jetpods do not limit play:** nothing but the status panel reads $3D. A pod lost and Jetman brought home costs
  nothing; a pod lost and Jetman killed shows one pod fewer. **[C]**
- **Game over [V]:** Jetman is killed with 1 life, or any loss on the final level. The game-over animation and the
  last password follow (`game-flow.md` 7.4).

## 5. Progression

### 5.1 Warpship pieces and the planet flags

- **Flags:** RAM $DE-$DF, one bit per planet index 0-11 (`game-flow.md` 6). Planet index 12 uses index 10's bit.
- **Setting a flag:** delivering a warpship piece (kind 19) sets the flag of the current planet. On a second
  level (13-28) "current planet" is the level the player came from, RAM $3C, with 12 counted as 10. **[V]** planet
  0 sets $DE bit 7
- **Rebuilding:** every approach screen rebuilds the flags as "every planet index below this one", so planets
  skipped by a shortcut count as done. **[C]**
- **Last piece:** text 48 instead of 47 when all twelve bits are set ($DE = $FF and $DF ≥ $F0). **[C]**
- **The planet menu after docking (`game-flow.md` 7.3):** BLAST OFF is offered exactly when the current planet's
  flag is set. EQUIP POD is offered when anything is owned or fitted: any of $0600-$060B, or $D1, $D3 or $050E
  non-zero. **[V]** (flag set, nothing owned: EXPLORE and BLAST OFF; flag clear, one item owned: EXPLORE and EQUIP
  POD)

### 5.2 Order of planets

| From | Rule | Status |
|---|---|---|
| Blast-off from planet index p | p + 1; from 12, to 11 | C |
| Planet index 7, touching its kind-27 star marker | straight to planet 12 (START LEVEL with $3B = 12, $3C = 7), skipping planets 8-10 | C |
| Planet index 2 (junction trigger) | the pod's position (x, y) with x in 1536-1543 and y in 864-871 (inclusive), while four or more of the object slots 23-30 are in the moving formation's state ($AA, i.e. alive and not yet exploding): transition 5 to level 27 | C |
| Planet index 5 (junction trigger) | the pod's position with x in 2876-2883 and y in 3436-3451 (inclusive): transition 5 to level 28 | C |
| Level 27, exits 0/1/2 (kind 20, list index) | planets 4 / 5 / 6 | C |
| Level 28, exits 0/1 | planets 8 / 9 | C |

The two junction triggers fire only in player states where pause is allowed. A transition 5 that lands on a
planet goes through START LEVEL: flight, shop on odd planets, approach. The remap table also lists "destination 21
from level 3 → 10" and "destination 5 from level 0 → 7"; no exit was found that produces them. **[C]**

### 5.3 Within a planet: wormholes, second and third levels

`game-flow.md` 7.4 (transition 3) gives the level arithmetic. In play: **[C]**

1. **Planet p:** fill the mothership's tank (3.4); the wormhole (kind 18) then takes the pod, **or Jetman alone**, to the
   second level 13 + p. The full rules of the touch (state, shields, the 13-pixel box, the guard) and of the bounce are in
   `player.md` 9.2.1 and 9.2.2 **[V]** (Mesen, 2026-10-17). In short: the touch needs the probe shield off and nothing
   on the beam (otherwise nothing happens, no bounce); on a planet with c below 25 it bounces the player (a direction
   bit flips, a sparkle in slot 3, sound $44, no damage); on levels 13-41 there is no fuel condition, so it is always
   open and nothing bounces there. An open gate starts the gate state (`player.md` 9.2.3), which ends in transition 3
   (or in the kind-6 screen, 5.4).
2. **Second level 13 + p:** a small cavern with the warpship piece (kind 19), a transporter ring (kind 17) and the
   wormhole back (kind 18). Towing the piece into the ring delivers it (5.1).
3. **Wormhole out of the second level:**
   - this planet's flag set: to the warp level 29 + p;
   - flag clear: back to planet p.
4. **Warp level 29 + p:** collect crystals by touch until the countdown ends (5.4), then back to planet p.
5. **Planet p again:** dock, BLAST OFF.

### 5.4 Warp levels, the badge intermission and the backdrop interlude **[C]**

- **Warp levels** (29-41, RAM $9B bit 7; their world, camera, collision and HUD are described in the last bullet of
  this list):
  - The ten kind-16 crystals are collected by touching them (the same test that tows on a planet: pod flying, or
    Jetman standing or flying, within 13 pixels), not towed (the pod needs its probe shields off and nothing on the beam). Each adds 1 to RAM $67 and
    disappears. On planets 0, 2-12 a kind-16 crystal that was placed directly (not made by spinning) is towed, like
    a kind 15, and delivered for 500 or 2000 (2.4). **[C]**
  - A countdown object shows 10 down to 0, one digit per 64 steps (about 2.1 s, 23.5 s in all; `objects.md` 7 agrees).
  - When it passes 0: transition 6 with $DB = 0. The counter is object slot 3 (state $0D), made by the warp-level routine
    in the first pass of the level; its counter starts at 0 and counts one per logic step from the first step, so the
    request comes 704 steps (1,408 frames) after the first gameplay frame **[V]** (counter 1 at frame 3, request at
    frame 1,409). The digit is drawn 56 px below the pod every step (x = pod x, y = pod y + 56).
  - **Warp-level hazards (all of 29-41; surveyed 2026-10-17, including the warp levels 33-36 of planets 4-7): none.**
    **[V]** Each warp level was loaded just after its launch and its object table logged for 720 logic steps (a
    little over the countdown's 704 steps), once with the pod left alone on every level 29-41 and once with the pod
    under thrust on level 33. Slots 15-30 stayed empty throughout: no enemy, shot, flame, mine or drone appears. The
    only objects are the countdown digit (slot 3, state $0D), the pod's own sparks (slots 5-11, states 1 and 2) and the
    crystal items (slots 11-14, state $0A, at most four at a time). The reasons match the other specs: the placement
    scan does not run on levels 29-41 and the shared placement entry is a dummy (`objects.md` 3.3, 3.4), the generators
    and the green-orb spawner (planets 0-12 only, `behaviour.md` 5.4) have no placed objects or planet to work
    from, and terrain damage is switched off there (3.3). The thirteen warp levels also share one items list, so the
    result is the same for every level. The only way to lose on a warp level is to run out of fuel (thrust still
    burns it: the high byte fell from $DF to $DC in 500 steps of thrust).
- **The warp level's world [V]** (Mesen, 2026-10-17; map decoded from the ROM, scripted and random flights on levels 29, 33, 34 and 37):
  - **Collision: yes.** The pod collides with the warp map by the ordinary terrain test (`levels.md` 6.4) and bounces like on
    any level; terrain damage is switched off (3.3), so a bounce costs nothing: no fuel, no hit pause $99, no
    invulnerability (a pod flying up at 1.50 hit a wall and came back at 0.75 down, the usual half-speed reflection, fuel
    unchanged). The shapes are those of block set 1 (graphics type 0, `levels.md` 4.1, 6.2). `levels.md` 6.7 describes the
    map (a closed arena).
  - **Camera:** the ordinary camera rules and limits of `levels.md` 5.2 (x up to 512, y up to 568 for this 6 × 6 chunk map);
    the pod cannot reach the limits, so the camera only follows it. The camera at the start is placed from the
    pod's start (320, 253): (192, 160).
  - **HUD:** the ordinary status panel: fuel gauge, weapon gauge, jetpods and lives, score. Nothing shows the crystal
    count; the only counter on screen is the countdown digit. The panel's background is the level's backdrop colour
    (white-blue here), the only visible difference (screenshots, planet 0 against level 29).
  - **Helper for the mesh:** `objects.md` 10.6 (levels 33, 35, 37 and 39 are different).
  - **Crystals** (the ten kind-16 items of the shared items list): touched by the pod, or by Jetman standing or flying, in
    the same 13-pixel box; the touch removes the item (its RAM kind is cleared), adds 1 to RAM $67, requests sound $1C on
    both pulses and makes a spark burst; **no score**; nothing is towed. **The burst [V]** (round 13, Mesen: every object-table byte of slots 3-14 for 120 frames,
  a model of the three sparks matched step for step): three sparks of **state 2**, in the three highest free slots
  of 3-10 (10, 9, 8 when all were free), created in the step of the touch at the crystal's position (the pod was put on it), frame $62,
  life ($0393) $20, with the speeds of the impact sparks (`player.md` 5.5), by slot: slot 10 (h 1.80 left, v 1.40 down), slot 9 (h 0.40 right,
  v 1.C0 up), slot 8 (h 1.80 left, v 0.40 down). The directions are fixed, not random (one crystal logged; they belong to the slot). Each logic step a spark first **moves** (x by h in its direction, y by v in its vertical
  direction, 8.8 with the fraction bytes), then **accelerates upwards by $40 (0.25)**: if it is moving down, v -= $40 and when that
  reaches 0 or below the vertical direction flips to up (the magnitude becomes the negated remainder, 0 at an exact zero);
  if it is moving up, v += $40 capped at 3.00. Life +1 per step. It is freed at life 144 (`behaviour.md` 6.4) or earlier by
  the ordinary off-screen rule (`objects.md` 3.5: seen freed at y = camera y - 34), whichever comes first. Frame by life mod 4: 0 = $37, 1 = $62, 2 = $63, 3 = $62
  (made with $62; `behaviour.md` 6.4). So the sparks **rise** (they do not fall): the "falls" of the earlier
  text was wrong. The score comes only from the badge
    screen afterwards.
  - **Jetman:** he appears only when the pod is destroyed, which on a warp level means fuel 0 (nothing else hurts).
    While he lives he collects crystals the same way, and the countdown continues (checked: the counter kept running
    through the pod's destruction and Jetman's flight).
- **Badge intermission** (transition 6, the Space Loonies badge screen):
  - RAM $67 (the crystal count) is cleared at every level load, at the start of the backdrop interlude, and starts
    each warp level at 0. (While a planet item is towed, the same byte holds its weight, which is why it is
    cleared on loading.)
  - The badge drops in for about 107 frames. When it lands (y = 48) the scene resets RAM $5D to 0 itself. After
    that, each time $5D is a multiple of 16 (so 16, 32, ... frames after landing) and $67 is not 0, one crystal is
    taken and 100 points added, with a sound; the tenth counted crystal adds 1100 instead (100 plus 1000 for the
    full set). **[V]** with $67 = 10: nine gains of 100, then 1100, 16 frames apart.
  - The scene ends when $5D reaches 255, that is 255 frames after the badge lands, whatever $67 is. **[V]** The
    scene does reset $5D itself, at the landing (entering with $5D = $14 still gave the same timing), so the end does
    not depend on the value of $5D at entry; only the 107 or so frames before landing run on it, and nothing is
    decided in them. `game-flow.md` 1.5 should say this rather than "does not reset it".
  - End: with $DB = 0, transition 3 (warp level → its planet); with $DB ≠ 0, the saved player position is
    restored and the level resumes.
- **Backdrop interlude** (transition 7):
  - Started by touching a kind-27 star marker on planet indexes 0, 4 and 8 (on 7 the marker is the shortcut
    instead).
  - It saves the player object (RAM $05A6-$05AB), clears $67, and plays a bonus flight on a separate backdrop
    (`scenes.md` 11.1): the pod climbs for 2,048 steps, steered left and right, and each gem it touches adds 1 to
    $67 (10 gems on each of the three backdrops). Rocks push it sideways; on backdrops 1 and 2 (planets 4 and 8)
    mines destroy it. **[V]**
  - The scene ends when the scene's vertical speed returns to zero. Then transition 6 with $DB = 1: the
    intermission counts the $67 gems (100 each, 1,100 for the tenth), then back to the same place on the planet.
    **[V]**

### 5.5 The final level and the ending **[C]**

- **Start:** after the message screen of the last warpship piece (in fact after **any** message screen of kinds 1-5 once
  all twelve flags are set), the game sets lives, jetpods and the fitted weapon $D1 to 0, RAM $9A = $80 (final level),
  $3B = 26, $E0 = 0, and starts the level directly: **no mothership and no launch scene** (the last piece is often delivered in
  a cavern), the pod is simply placed at the level's home point flying up. The full list, with the scenes skipped and the
  music, is in `game-flow.md` 7.4, "Entry to the final level" **[V]**.
- **Losing:** any loss is game over.
- **Completing:** the level's script runs to its end (its waypoint list, RAM $E0 reused as the index; then $9A
  bit 6). Then text 23, "CONGRATULATIONS JETMAN ...", shows as message kind 7 for 319 frames (255 counted and 64 in which the
  timer does not count; the remake always 319, `behaviour.md` Q33), whole on its first frame, then the ending
  (`game-flow.md` 7.4).

## 6. The shop and the equipment

### 6.1 What each system sells **[V]**

Shops appear after the flight to odd planet indexes 1, 3, 5, 7, 9, 11 (`game-flow.md` 7.3); system = $3B / 3.

- **Stock lists:** bank 0 $8CEE, one list per system, each ended by 0. A list byte is equipment number + 1:

  | System | List bytes | Items |
  |---|---|---|
  | 0 | 1, 4, 7, 11, 13 | 0, 3, 6, 10, 12 |
  | 1 | 2, 6, 9, 14 | 1, 5, 8, 13 |
  | 2 | 5, 10, 12 | 4, 9, 11 |
  | 3 | 3, 8 | 2, 7 |

- **Stock is cumulative:** a shop in system s offers the lists of systems 0 to s, in item order.
- **Planets 3, 7 and 11** ($3B AND 3 = 3): list bytes 13 and 14 stand for the NIPPON SPORTS JETPOD and the
  ITALIAN RACING JETPOD instead of the two mapping devices.
- **Hidden entries:**

  | Entry | Hidden when |
  |---|---|
  | MAPPING DEVICE | either mapping device is owned |
  | SUPER MAPPING DEVICE | the super one is owned |
  | NIPPON SPORTS JETPOD | the pod type ($DC AND 3) is not 0 |
  | ITALIAN RACING JETPOD | the pod type is 2 |

- **Seen:**

  | Planet | Items offered |
  |---|---|
  | 1 | 0, 3, 6, 10, 12 |
  | 3 and 5 | 0, 1, 3, 5, 6, 8, 10, then 12 and 13 (on 3: the two jetpods) |
  | 7 | items 0-11 except 2 and 7, plus the two jetpods |
  | 9 and 11 | everything (items 0-13; jetpods on 11) |

### 6.2 Prices **[V]**

Price = 10 × a 16-bit value: low byte at bank 1 $96F6 + b, high byte at bank 1 $9706 + b, where b is the list
byte (1-16; on planets 3, 7, 11 the jetpods use b = 15, 16). All prices were read on the shop screen:

| Item | Price | Item | Price |
|---|---|---|---|
| 0 HOMING MISSILES | 400 | 8 SUPER SHIELDS | 500 |
| 1 ANTI GRAVITY | 1000 | 9 MOMENTUM KILLER | 800 |
| 2 SMART BOMBS | 2500 | 10 EFFICIENT ENGINES | 500 |
| 3 TIME BOMBS | 150 | 11 DOUBLE STRENGTH THRUSTERS | 1000 |
| 4 STAR BULLETS | 400 | 12 MAPPING DEVICE | 1500 |
| 5 MULTI WARHEAD MISSILES | 400 | 13 SUPER MAPPING DEVICE | 5000 |
| 6 TITANIUM BULLET PACK | 800 | 14 NIPPON SPORTS JETPOD | 10000 |
| 7 MILITARY BULLET SYSTEM | 1600 | 15 ITALIAN RACING JETPOD | 25000 |

The price is shown in five digits above the item (e.g. 00400).

### 6.3 Buying **[V]**

A ("FIRE A TO SELECT") buys the item shown when all hold:

- the score is not negative and is at least the price (499 for a 500 item: refused; 700: bought, 200 left);
- for an ordinary item: fewer than 9 owned (RAM $0600 + item number); for a mapping device: none owned;
- for a jetpod: the hiding rule of 6.1 does not apply.

**Paying:**

- The A press itself only starts the purchase (the check happens in that frame, afford test: the score, as five
  digits plus the top digit, minus the price's five digits, must not borrow; so a score equal to the price is enough).
  Payments begin on the first later frame where the frame counter $5D is a multiple of 8, so 1-8 frames after the press.
  Then, at each such frame, the price's highest non-zero digit goes down by one and the score loses that power of
  ten (10000, 1000, 100 or 10), with a sound.
- A 500 item takes five such steps (40 frames); input is ignored until it is paid.
- At the next such frame after the last digit is paid:
  - an item adds 1 to its count;
  - a jetpod replaces the pod type: $DC = ($DC AND $C0) + 1 for Nippon, + 2 for Italian (shields and boosters are
    kept).

The list is built once when the shop opens and is not rebuilt after a purchase, so a mapping device or jetpod just
bought stays shown (buying it again is refused). "YOU OWN n OF THEM" shows the count; for a jetpod entry it is 1 when
that pod's bit is held in $DC (bit 0 Nippon, bit 1 Italian; owning the Italian shows 0 for the Nippon) and 0
otherwise. B ("FIRE B TO EXIT") leaves to the approach screen. Left/Right scroll the list;
it does not wrap.

### 6.4 Fitting equipment (EQUIP POD) **[C]**

Opening the shop or the equip list first returns whatever is fitted to the owned counts. The equip list shows the
owned items 0-11 (count non-zero). A on an item fits it, taking one from its count. Fitting into a slot that holds
something returns that item to its count first. Choosing the item already fitted just removes it. The unfit-all
step that opens both screens clears $D1, $D3 and $050E and adds one back to the count of what they held (reverse of
the table below: $D1 value 1, 2, 4, 5, 6, 8 is item 0, 1, 2, 3, 5, 4; $050E 1, 2 is item 6, 7; $D3 is the item
number). The equip list is built once from the items with a non-zero count and is not rebuilt: an item whose count
falls to 0 stays listed, and A on it does nothing (the count is tested first), so it cannot be un-fitted until the
screen is reopened.

| Items | Slot (RAM) | Value stored |
|---|---|---|
| 0, 1, 2, 3, 4, 5 | $D1, the special weapon (fired with Select, paid from the W gauge) | 1, 2, 4, 5, 8, 6 respectively (table bank 1 $96B7) |
| 6, 7 | $050E, the bullet upgrade | 1 (titanium), 2 (military) |
| 8, 9, 10, 11 | $D3, the pod device | the item number, 8-11 |
| 12, 13 | none: owning one sets $060C / $060D (the counts) and is permanent | |

B leaves to the launch scene, which refills both gauges (3.2). The demo fits $D1 = 1, 8, 6.

**What each does in the rules** (physics details in the physics spec):

| Item | Rule |
|---|---|
| Special weapons ($D1) | Fired with A newly pressed (autofire repeats count) while Select is held, a weapon is fitted and the W gauge is not empty; holding A fires once. There is no cooldown. Costs are in gauge units (the high byte, 0-224): HOMING MISSILES ($D1 = 1) 4, TIME BOMBS (5) 32, MULTI WARHEAD (6) 16, STAR BULLETS (8) 32 (verified: 224, 192, 160), SMART BOMBS (4) 128 per use and act on every live object (`player.md` 10.2). The cost is taken after the shot, and a gauge at or below the cost goes to 0 (the last shot still fires). With an empty gauge the press fires a plain bullet. The object states made are $0F, $10, $14 and $12 for $D1 = 1, 5, 6, 8; their creation is `player.md` 10.1 and their behaviour `behaviour.md` 5.11 (the smart bomb reaches every live object of slots 15-30, on screen or not: `player.md` 10.2). Weapons share the shot slots 3-10 with all shots and sparks (`player.md` 10); a special other than the smart bomb needs the first free slot to be one of 3-6 and otherwise nothing fires and nothing is charged (four homing missiles in flight blocked a fifth); some presses therefore fire nothing. ANTI GRAVITY (2) is a switch, RAM $D2 bit 7: each press toggles it at no cost (and no shot); while on it drains the W gauge by $10 per step (16 of 256) and the pod's gravity is skipped; it switches off when the gauge is empty |
| Bullet upgrade ($050E) | the pod's normal shots get $050E + 1 hit points of strength (1 plain, 2 titanium, 3 military). Lost when Jetman is killed |
| SUPER SHIELDS ($D3 = 8) | with probe shields on, shot damage is halved again (3.3) |
| MOMENTUM KILLER ($D3 = 9) | while B is not held the pod's speed is damped |
| EFFICIENT ENGINES ($D3 = 10) | thrust and boost cost half (3.2) **[V]** |
| DOUBLE STRENGTH THRUSTERS ($D3 = 11) | thrust doubled |
| Jetpod type ($DC bits 0-1) | 1 Nippon, 2 Italian: different handling (physics). Either may tow kind-11 objects; Nippon may tow rings; Italian may tow spare pods and jump between rings (2.1) |
| Probe shields ($DC bit 7), boosters ($DC bit 6) | found on planets (2.3), never bought; kept by the password |

Fitted $D1 and $D3 are lost with the pod (3.2).

### 6.5 Mapping devices and the planet map **[V]**

- RAM $060C = MAPPING DEVICE owned (also set by delivering the navigators, kind 6); RAM $060D = SUPER MAPPING
  DEVICE. Both are kept by the password.
- **Opening the map:** in pause, Select opens it on planets (levels 0-12) when either is set (`game-flow.md` 4).
- **Map with either device:** the whole planet's map (not only explored parts: `scenes.md` 13.2) and a marker for the mothership's home.
- **With the super mapping device:** also a marker for every list item still present (section 2.1).

## 7. RAM used by the economy

| RAM | Meaning | Status |
|---|---|---|
| $CA | score sign (bit 7 = negative); not in the password | C |
| $CB-$D0 | score digits | V |
| $C6, $C7 | fuel (low, high), full = $E000 | V |
| $C8, $C9 | W gauge (low, high), full = $E000 | V |
| $05CE | displayed F gauge value (eased) | C |
| $D1 | fitted special weapon (1, 2, 4, 5, 6, 8) | V |
| $D2 | bit 7 anti-gravity on; bit 6 and low nibble pickup tag (2.5) | C |
| $D3 | fitted device (8-11) | V |
| $050E | bullet upgrade (0-2) | C |
| $D6, $D7 / $D8, $D9 | mothership home x (low, high) / y (low, high) | C |
| $0636 | fuel count saved while on a second level (3.4) | C |
| $65 | bit 7 probe shields on | V |
| $66 | slot of the item on the beam | V |
| $67 | warp-level crystal count; also the weight of the towed item on planets; cleared at level load | C |
| $68 | cargo number k at a delivery; rope angle fraction while towing | C |
| $69 | rope angle 0-127; in the gate state 9 the previous player state | C |
| $6E | spin count, bit 7 armed (2.6) | C |
| $98 | bit 5: pod in the bay area (2.2) | C |
| $99 | terrain hit pause (3.3) | V |
| $97 | invulnerability (3.3) | V |
| $E0 | crystals stored 0-6 (2.4); the final level script index | V |
| $0100, $0101 | blue crystal list indices, $FF = none (2.6) | V |
| $DC | bit 7 probe shields found, bit 6 boosters, bits 0-1 pod type | V |

## Open questions

- The pickup tag rule ($D2 low nibble) is a reading of the code, not watched in play.
- Kind 1 (spare pod via the Italian pod's tow) was not watched in play. The interlude's gems were forced from the
  planet (`scenes.md` 11.1), not reached by touching a star marker.
- Whether the score can become negative in normal play (no source found).
