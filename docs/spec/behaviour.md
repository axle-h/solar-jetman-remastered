# Object behaviour

What every object other than the player does on each logic step: movement, firing, animation, uses of the random
generator, collisions with the pod and its shots, damage, destruction and score. The object table, placement,
sprite layouts and collision sizes are in `objects.md`; the logic-step schedule and the random generator in
`game-flow.md`; the player (pod, Jetman, gun, tractor beam, energy) in `player.md`; deliveries, item values and the
shop in `economy.md`.

Category (`rom-map.md`): all tables named here are **commit** data (mechanics).

Status: **[V]** a model written from this text reproduced the original's object table step by step in Mesen (the
run is named in section 11), **[C]** read from the program, **[H]** hypothesis. Addresses follow `README.md`.
The types that first appear on levels 4-7 were run in Mesen on 2026-10-17 (section 11, second table); what was wrong
in this text about them has been corrected in place and is marked **[V]**, and the behaviours that look like
mistakes in the original are listed as quirks Q12-Q18 in section 10 (Alex decided Q1 and Q12-Q18, see
`docs/deviations.md`). The types that first appear on level 8 or later (ground-gun pairs, the blue sphere, levels 12,
15 and 26) and everything else that was still only read (all turret types, pillars, generators 4B and 4E, the green
orb, the companion's phase 5, bonus objects) were run the same day (section 11, third table); corrections are marked
in place and the new quirks are Q19-Q21. A fourth round (2026-10-17) ran the items of every kind, the box, the
beam-held orb, the fourth-slot event of bank 2 and the final level's background effect (section 11, fourth table);
it adds Q22 and Q23 and explains the one orb that did not appear. A fifth round (2026-10-17) settled thirteen points that
an implementer had fitted black-box (2.4, 2.8, 2.9, 3.1, 3.3, 4.6, 4.7, 4.13, 5.3, 5.5, 5.7; quirks Q24-Q26). A seventh-round check (2026-10-17, from the program and Mesen) settled six more fitted points: the start steps of the level 26 objects (4.16), the exact cases of the distance routine and its stale byte (2.4, Q24), the marker's stale fields (6.2, Q31), and the press that is lost (Q30, `player.md` 10.1). An eighth round (2026-10-17, Mesen) played level 26 to its end and fired a smart bomb at the cores (4.16, 5.11.4; quirks Q32, Q33).

Contents: 1 timing; 2 shared rules; 3 collisions, damage and destruction; 4 placed objects by type; 5 objects
made by other objects; 6 items, mothership pieces and pod-side objects; 7 work fields per state; 8 random numbers;
9 sounds; 10 quirks and fidelity; 11 verification; open questions; corrections to other specs.

## 1. Timing and order **[C]**, order of effects **[V]**

One logic step = one A pass plus one B pass (`game-flow.md` 1.2). In each step:

1. **A pass**: the player is updated first (`player.md`). Then every slot 1, 2, ..., 30 with a non-zero state runs
   its state's behaviour, in ascending slot order. An object created during this sweep in a higher slot than the
   creator also runs in the same sweep; one created in a lower slot first runs on the next step.
2. Still in the A pass: the status-bar split steps the random generator and increments the frame counter; then
   the green-orb spawner runs (5.4), then the collision checks (section 3).
3. **B pass**: drawing, despawning of objects that left the activity window, and spawning from the placement list
   (`objects.md` 3.4, 3.5).

Values the behaviours see:

- **F** = the frame counter RAM $5D during the A pass. It is always even and grows by 2 per step. Timings below
  are written as conditions on F (e.g. "F mod 8 = 0" holds on every 4th step) or on **k** = F / 2 (the step count
  modulo 128). Some conditions add the slot number s to k or F, which staggers identical objects.
- **R** = RAM $5A, **R1** = RAM $5B, **R2** = RAM $5C: the generator as left by the previous B pass split. A few
  behaviours change $5A themselves (section 8); later readers in the same sweep see the changed value.
- The collision checks (section 3) see the generator after the A-pass split.

Before the sweep, the hit points of one slot (whichever the player routine left selected) are lowered to its
state's initial value if they are above it (`objects.md` 2.4). Nothing in normal play raises hit points above
that value, so a remake can ignore this. **[C]**

## 2. Shared rules

### 2.1 Position and speed fields **[V]**

| Field | Meaning for objects 1-30 |
|---|---|
| x, y | world position (`objects.md` 2.1) |
| RAM $0197 + s | **horizontal sub-pixel** (fraction of x, 1/256 px). Lies in the stack page; cleared when an object is created |
| RAM $023E + s | **vertical sub-pixel** (fraction of y). This is the meaning of $023E for every slot |
| work 1/2 ($029B/$02BA) | horizontal speed, 8.8 (fraction, whole) px per step, a magnitude |
| work 3/4 ($02D9/$02F8) | vertical speed, 8.8, a magnitude |
| flags X bit 0 | horizontal direction: 0 right, 1 left |
| flags Y bit 0 | vertical direction: 0 up, 1 down |

**Move** (the standard step): x += speed in the direction of flags X bit 0, y likewise with flags Y bit 0, each as a
24-bit sum of position and sub-pixel (16.8), wrapping. One exception: if the horizontal whole byte has bit 7 set
it is a signed (negative) speed and the object moves the other way by its magnitude; only aimed shots (2.8) make
such speeds. Some behaviours move one axis only ("move x", "move y").

Objects never test the terrain: the terrain test of `levels.md` 6.4 is called for slot 0 only. **[C]** Objects fly
through walls; they are limited only by their own rules and by despawning.

### 2.2 Speed changes **[C]**, used in every verified model **[V]**

On one axis, with amount `a` in 1/256 px:

- **speed up**: speed += a; if the whole part reaches 3 or more the speed becomes exactly 3.00. So no speed made
  this way exceeds 3 px per step. (Speeds set directly, e.g. shots, can be higher.)
- **slow down**: speed −= a, stopping at 0.
- **slow down and turn**: as slow down; if the speed reaches 0 the direction bit flips (the next step speeds up
  the other way). Variant used by the magnets (4.10): speed −= a, and if that would go below 0 the speed becomes
  the overshoot (|speed − a|) and the direction flips; reaching exactly 0 also flips.

### 2.3 Homing **[V]**

"Home toward T with accel a, max m" on one axis:

```
toward = the direction bit points from the object to T on this axis
         (T at the same coordinate counts as right/down)
if toward: speed up by a
else:      slow down by a; if the speed reached 0, flip the direction bit
if speed >= m: speed = m
```

then the object moves on that axis. Homing on both axes does x first (and moves x), then y (and moves y), with
the same a and m. The maximum is written as one byte `M` in the tables below: whole part = M mod 8, fraction =
M − (M mod 8), so M = $83 is 3.5 px per step and M = $C2 is 2.75. Because of the 3.00 cap of 2.2, no homing speed
exceeds 3.00. T is the pod (slot 0) unless stated.

### 2.4 Distance to the pod **[V]**

`d = |dx| + |dy|` between the object and the pod, each part counted only if below 256; if either |dx| or |dy| is
256 or more, d = 255 (and the vertical part is not computed); a sum of 256 or more also gives 255, so d never
exceeds 255. The same measure gives two side flags: **pod-right** (object x < pod x) and **pod-below** (object y <
pod y). The vertical flag is only computed once |dx| is known to be below 256: **with |dx| ≥ 256 the pod-below flag
is always 0** (Q19; seen on level 26, section 4.16). The horizontal flag is always right. "Within 16" for waypoints
is 2.7.

**Early exits [C]** (Q24). With |dx| ≥ 256 the routine returns 255 before it looks at the y difference, so the
scratch value |dy| that callers read afterwards (RAM $05) is whatever an earlier routine left there (the drone and the
hopper read it, 5.5 and 5.7). With |dx| < 256 and |dy| ≥ 256 it returns 255 after setting the pod-below flag
correctly, but |dy| is the low byte of the signed difference, not made positive when the object is above the pod. A
difference of exactly −256 on either axis counts as 255. The creatures of 4.13 and the aimed shot of 2.8 use full 16-bit
distances instead and are not affected.

*Read from the program, seventh round (2026-10-17), [C] unless marked.* The routine works on the 16-bit differences
d = object − pod. For each axis it looks at the borrow of the 16-bit subtraction and at the high byte of the result:
- **Side flags.** Pod-right is set when the horizontal difference is negative (borrow), before any range test, so it is
  right at any distance. Pod-below is set when the vertical difference is negative, again by the 16-bit borrow (not by
  comparing low bytes or an unsigned byte), and before the range test of that axis, so it is right for any |dy|
  *provided the vertical part is reached at all*, that is, only when the horizontal difference passed (|dx| < 256, or
  exactly −256, below).
- **Horizontal range.** High byte 0 (difference 0 to 255): |dx| = the low byte. High byte $FF (−1 to −256): |dx| = the
  low byte negated, where a low byte of 0 is turned into 255 (so **−256 counts as 255 and is not an early exit**, while
  +256 and every other difference of 256 or more in size exits). Any other high byte: exit.
- **Vertical range**: the same, with the same −256 rule (as 255), and the flag already set.
- **Exits** leave the answer 255 (the routine's result byte and the "far" byte RAM $07 are both $FF). What the working
  bytes hold on an exit: on a horizontal exit RAM $04 is the raw low byte of the signed difference (not negated) and RAM $05
  (|dy|) is **not written at all** (stale); on a vertical exit RAM $04 is the true |dx| and RAM $05 is the raw low byte of the
  signed vertical difference (not negated). The sum |dx| + |dy| of 256 or more exits the same way with both $04 and $05
  true. The result is the sum otherwise.
- **The stale $05 [V].** On a horizontal exit the byte is whatever the last writer left. Tracing the drone (level 2, 1,500
  drone-steps with the pod 300 px to the side) and the hopper (level 1, 30 calls), the last writer before every call was
  the sprite-grid drawing of the B pass (the nine-cell pod drawing: a loop that stores the cell's x offset in $04 and y
  offset in $05, offsets 0, 16, 32 in turn, ending with 32), so the byte read was **$20 = 32** in all of them. So
  a drone climbs (32 ≥ 16) and a hopper takes its far branch (32 ≥ 24). If another object earlier in the same sweep has
  run the routine (or any code that uses $05), its leftover is read instead; that was not seen.

### 2.5 Creating an object **[C]**

A behaviour that creates an object looks for the **first free slot** (state 0) in a range: enemy shots and most
children use 15-22 (searched upwards); others are named per case. If none is free nothing is created, except where
noted. The new object's fields not named in the rule are cleared to 0 (position, speeds, sub-pixels, flags, work
bytes), except that some rules reuse a slot without clearing the fraction bytes of its speed (noted). Its hit points
come from its state's table entry (`objects.md` 2.4) unless set explicitly.

Several creators do not clear the whole slot **[V]**: the aimed shots of domes and wide machines (2.8), the shots
of ground guns (4.2) and of walker and pillar heads (5.3), the volley of type 22 (4.11) and the dome exhaust (5.1)
set only the fields their rules name. The position fractions ($0197 + s and $023E + s) keep the values of the
slot's previous occupant, so a new shot can start up to 1 px off its nominal position; the type 22 volley and the
exhaust also keep the slot's work bytes and field $03B2 (and, when the volley or the exhaust in slot 22 overwrites a
live shot, its speed bytes and counter). Shots and missiles made by these routines have **hit points 0**, not the 1
and 2 listed in 3.6 (nothing can hurt them, so this does not matter). Every drone keeps the previous occupant's fire timer
(work 5): those of the hourglass turrets (4.5), of the type 11 dome (4.7) and of the generators 4B/4C (4.14). The routine that
makes drones, mines and hoppers (generators, domes, turrets) does clear both position fractions, both speeds, the flags and the
fields $0393 and $03B2, so these objects start with **no** stale fractions, and only work 5 and work 6 are left (work 6 is
set by the hopper's maker; a mine uses neither; a hopper writes its work 5 on its first step before reading it, so only the
drone's timer is visible) **[C]**. The remake may clear everything (Q17).

### 2.6 Standard enemy shot (state $15) **[V]**

Created with frame $33 (a single dot, unless the rule sets another frame), lifetime counter (field $0393) $C0 unless
set otherwise, at the creator's position plus an offset, with speeds and directions given by the rule. Every step:
move, lifetime − 1; at 0 the slot is freed. Contact with the pod destroys the shot and hurts the pod (section 3).

### 2.7 Waypoint shuttle (types 21, 22) **[V]** for the observed path, **[C]** table

Per level, bank 7 $CF65 + L (low) / $CF7F + L (high) (ROM 0x3CF65 / 0x3CF7F, levels 0-25) point into one list of
9-byte records at bank 7 $CF99-$D031: `list index, x1 (16-bit), y1, x2, y2`. An object uses the first record from its
level's pointer onward whose first byte equals its placement-list index. Records for the levels where these types
occur:

| Level | List index | Type | P1 | P2 |
|---|---|---|---|---|
| 2 | 25, 26 | 21 | (1280, 1627), (2684, 1495) | (1280, 1920), (2684, 1288) |
| 3 | 51, 68, 69 | 21 | (690, 1585), (2108, 1468), (4290, 1113) | (690, 1665), (2108, 1673), (4290, 1289) |
| 4 | 5, 7 | 22 | (1798, 3075), (2005, 3075) | (1592, 3075), (2195, 3075) |
| 7 | 34 | 22 | (2454, 1401) | (2662, 1401) |
| 8 | 74, 75, 76, 77 | 21 | (3392, 1006), (2366, 1279), (2361, 1660), (825, 2722) | (3392, 772), (2366, 868), (2361, 1397), (825, 2499) |
| 12 | 32, 68, 69 | 22 | (2820, 2070), (3218, 2173), (3162, 1691) | (2657, 2070), (3381, 2173), (3295, 1691) |
| 15 | 6, 7 | 21 | (460, 102), (115, 267) | (460, 398), (115, 442) |

Run on 2026-10-17 **[V]**: level 8 index 76 (turning points and the stuck behaviour below), 75 (reaches P2, turns
back at y about 1008), level 12 indexes 32, 68, 69 (the machines of 4.11, 1,138-1,397 steps each, positions of P1 and
P2 as in the table) and level 15 index 6 (P1 as in the table, turns back at y about 127). The other rows are [C].

Rule: work 5 bit 7 clear means "heading for P1", set means "heading for P2". Each step the object measures its
distance to its current target; if it is "within 16" the bit flips and the target becomes the other point.
"Within 16": `|x − tx| + |y − ty| < 16`, where |x − tx| must be below 256, but the vertical part is taken from the
low byte only when the object is above the target, so a target 256-271 px below the object also counts as
reached (section 10, quirk Q1). Observed on level 2: the type-21 object at (1280, 1627) never reaches P2; it turns
back near y = 1650-1665 and shuttles between about y 1612 and 1681. **[V]** The same happens wherever the vertical
distance between P1 and P2 is 256-271 px or more: on level 8 index 75 (411 px) the creature reaches P2 (y 883), then
turns back at y about 1008 on the way down, 271 px short of P1, and shuttles between y 852 and 1008; index 76 (263 px)
reaches P2 and then flips its target on every step (the test is already true when it starts back), so it hovers
at y 1374-1405 for good and never returns to P1; on level 15 index 6 (296 px) it turns back at y 127, 271 px
short of P2, and shuttles between y 85 and 158. **[V]**

### 2.8 Aimed shot with spread **[V]**

Used by domes and wide machines (4.6, 4.7). The shot is placed with an offset/flags entry of the table at bank 7
$B108 (dx), $B113 (dy), $B11E (flags Y), $B129 (flags X) (ROM 0x3B108.., 11 entries):

| i | dx | dy | flags Y | flags X | i | dx | dy | flags Y | flags X |
|---|---|---|---|---|---|---|---|---|---|
| 0 | −21 | 8 | $FF | 1 | 6 | 12 | −3 | 0 | 0 |
| 1 | 21 | 8 | $FF | 0 | 7 | 0 | −12 | 0 | $FF |
| 2 | 0 | −16 | 0 | $FF | 8 | −12 | 3 | 1 | 3 |
| 3 | −12 | −7 | 0 | 3 | 9 | 12 | 3 | 1 | 0 |
| 4 | 12 | −7 | 0 | 0 | 10 | 0 | 12 | 1 | $FF |
| 5 | −12 | −3 | 0 | 3 | | | | | |

Speed: take |dx|, |dy| from the shot's position (after its offset) to the pod (16-bit) and double both together up to
3 times: the first doubling only if both are below 128, the second and third only if both are still below 256
(a high byte of 0). The tests are nested: the loop stops at the first test that fails, and the first one is the strictest.
In effect the factor is **8 when both |dx| and |dy| are below 64, 4 when both are below 128, else 1** (never 2; and 1
whenever either is 128 or more, including 256 or more) **[C]**. So (40, 16) becomes (320, 128) and (100, 10) becomes (400, 40) **[V]**. Then add a random offset to
each, as signed 16-bit values in 1/256 px: horizontal += (R1 mod 64) − 32, vertical += (R2 mod 64) − 32. The sums
are the speeds (8.8). A negative horizontal sum is a signed speed (2.1), but **a negative vertical sum is not**: it
is read as a magnitude of about 255.9 px per step (Q12). If the entry's flags Y has bit 7 set, the shot's vertical
direction is pod-below (1 if the pod is below); if its flags X has bit 7 set, the horizontal direction is toward
the pod.
Frame $33, state $15, lifetime $F0; sound $08. The shot's position fractions are the slot's old ones (2.5).

### 2.9 Eight-way homing missile (state $0F; the pod's homing missiles are also state $0F, its multi warheads state $14, 5.11) **[V]**

Direction index 0-7 clockwise from up (0 up, 2 right, 4 down, 6 left). Direction from the missile to a target, with
ax = |dx|, ay = |dy| (16-bit), left = target x < missile x, above = target y < missile y (equal counts as right and
below) **[V]**, 337 recomputations, all reproduced:

- If ax ≥ ay: straight left or right (6 or 2) when ax ≥ 2·ay, else the diagonal. The original does this in bytes:
  lo = (ax_lo − ay_lo) mod 256, borrow = 1 if lo < ay_lo, straight when ax_hi ≥ ay_hi + borrow. Below 256 px this is
  exactly ax ≥ 2·ay; beyond it is slightly off ((257, 209) counts as straight).
- If ax < ay: **the test is the wrong way round (Q13)**: straight up or down (0 or 4) when ay < T, the diagonal when
  ay ≥ T, with T = 2·ax computed byte by byte without the carry from the low byte (T = (ax_hi·2 mod 256)·256 +
  (ax_lo·2 mod 256)). Below 256 px: ay < 2·ax flies straight vertically, ay ≥ 2·ax diagonally.
- Diagonals: 1 up-right (target right, above), 3 down-right, 5 down-left, 7 up-left.

Enemy missile (created by domes and wide machines, work 6 = $20, work 5 = $C0, hit points 0). Its first frame is
$38 for an entry on the right and $3E for one on the left when the maker is upright, and $3A and $3C when the maker
is upside down (flags Y bit 7, type 1D); once the 32 straight steps are over the frame is $37 + direction (an earlier reading
took the first frame from the entry's flags and was wrong for left entries):

1. While work 6 > 0: work 6 − 1; both speeds speed up by $20; move; **and nothing else**: work 5 is not touched and the
   step ends. (The first 32 steps go straight.)
2. Then the target is the pod. When (s + k) mod 8 = 0 the direction is recomputed; flags X gets bit 1 set.
3. Frame = $37 + direction.
4. Per direction, horizontal acceleration (0, $14, $20, $14, 0, $14, $20, $14) toward the side of the direction,
   vertical acceleration ($20, $14, 0, $14, $20, $14, 0, $14). An axis with acceleration 0 slows down by $12; an
   axis whose current direction disagrees slows down and turns (2.2, the plain version: stop at 0 and flip, not the
   magnets' overshoot); otherwise it speeds up. Then move.
5. Work 5 − 1 every step **of steps 2-4 only** (the straight steps of 1 do not count); at 0 the missile becomes a medium
   explosion (state 5, sequence 5, 3.5; no score, its slot is below 23). So it homes for 192 steps after its 32
   straight steps, 224 steps of life in all **[C]**.

The pod's missiles (states $0F and $14, slots 3-10) run the same direction and speed rules but pick a random target among
slots 23-30, have no straight phase and live differently: 5.11.

### 2.10 Random edge position (generators, green orb) **[C]**

Places a new object just outside the screen, near the pod's row or column:

- If bit 1 of R2 is set: a left or right edge. Left (bit 2 of R2 set): x = camera x − 32; if that is negative use the
  right edge. Right: x = camera x + 288; if that is past the map's last page use the left edge. y = pod y + r − 64
  + (1 if r ≥ 64), r = R1 mod 128, added to the low byte only (the high byte is the pod's).
- Else: a top or bottom edge. Top (bit 2 of R2 set): y = camera y − 48. Bottom: y = camera y + 208, or the top if that
  is below the map. x = pod x + r − 32 + (1 if r ≥ 32), r = R2 mod 64, on the low byte only.

## 3. Collisions, damage and destruction

All of this runs once per step, after the A-pass split, in this order: pod against objects (3.1), then shots
against bonus objects (3.3), then shots against placed objects (3.4). Nothing runs while the pod's frame is $FF or
while the player state is 0, 4, 5 or 7-11 (table bank 1 $FF24). **[C]**

### 3.1 Pod against objects in slots 15-30 **[V]** box, push and damage

Candidates: slots 15, 16, ..., 30 in order, with state ≥ 6 and frame ≠ $FF. Sizes: the object's half-width w and
half-height h from its frame (`objects.md` 8); the pod's halves are pw = (RAM $F1) / 2 and ph = (RAM $F2) / 2,
normally 3 and 3 (`player.md` for when they change).

Overlap on one axis, with d = pod coordinate − object coordinate (16-bit signed):

- d < 0 (pod left of / above the object): overlap if p = w + pw − |d| + c ≥ 0, where c = 1 if |d| ≤ w else 0.
- d ≥ 0: overlap if p = w + pw − d + c > 0, where c = 1 if d < w else 0.

(The extra 1 is a carry the original does not clear; it is part of the measured push. Use h and ph for y.) Both
axes must overlap. Then, with px and py the two penetrations:

- **Push.** If py < px: the object (not the pod) moves vertically by py away from the pod and the pod's vertical
  direction is set away from the object. Else the same horizontally with px. **[V]** On the pod's side, on that
  axis, the only reaction of the pod to an enemy touch is: half of the object's speed on that axis is added to the
  pod's speed on that axis (both as magnitudes, the sum's whole part kept to 0-3, nothing else clamps it; the cap
  of 4.00 in `player.md` 4.2 step 5 applies to the next step as usual, so the pod can leave this step with up to
  3.FF plus nothing more). There is no fuel-independent bounce, no speed reversal and no pause: the pod keeps
  its own speed, plus that half. [V] (Mesen: a mine moving at about 0.1C vertically added 0.0A to 0.0E per touch
  to the pod's speed.) The impact routine (below) adds only sound and sparks. If RAM $D3 = 8 (super shields), the
  pod's vertical fraction byte is cleared instead of the speed being added (the whole part stays), and the horizontal speed is left alone. **[C]**
- **If the pod is invulnerable** (RAM $97 ≠ 0: Jetman emerging, a new pod, a gate; `player.md` 5.4; an enemy
  touch never sets it): the object is pushed, the pod's direction on the axis is set away, and **nothing else
  happens** (no damage, the object loses no hit point, no sound); the next object is checked. The pod's speed on
  that axis is not given the half-speed. **Rule [V] (Mesen, six placements with $97 set, and the 3.00 case in
  `tow.replay`):** on a **vertical** push the pod's vertical speed fraction becomes 0 and its whole part becomes
  (old whole + D) AND 3, where D = the low byte of (pod y − object y) taken before the push (so 0 to 3 whole
  pixels: pod 5 above the object gives $FB AND 3 = 3, up; pod 5 below gives 1, down; 7 below gives 2); the
  carry into the addition is 0 on this branch. On a **horizontal** push the speeds are not changed at all (the
  routine returns early). The same code runs with $97 = 0 when RAM $D3 = 8 (super shields): vertical fraction
  cleared and the whole part kept (D is 0 there); horizontal unchanged. The remake may keep "set to 0" as a
  simplification only if Alex accepts the deviation; the faithful rule is the one above (quirk Q11).
- **Hit pause RAM $99 does not matter here [V]:** an enemy touch neither sets $99 nor is blocked by it. A touch
  with $99 = $30 was damaged exactly like one with $99 = 0. $99 (`player.md` 5.4) only gates terrain damage and
  sparks. Because the object is pushed away by py or px and the pod is not paused, an object that survives and
  still overlaps next step hurts again: **the pod takes damage on every logic step of contact**, and a
  surviving object loses a hit point every step. [V] (A mine held on the pod took $4000 and 1 hit point on
  each of four consecutive steps.) Enemies that keep overlapping are rare because the push usually separates
  them (a 14 px push in the measured case).
- **Otherwise** the state's flag byte (3.6) decides, and the check ends after this object (only one object per
  step can hurt the pod or be hurt by it). **The whole collision routine ends there**: in a step where an object touched
  the pod in this way (also the shield-on case with no damage flag), no shot is tested at all (3.2-3.4: no shot hits a
  placed object or collects a bonus object). In the invulnerable case above the loop goes on to the next object and the
  shots are tested as usual **[C]**:
  - flag bit 6: the object is destroyed (3.5);
  - else flag bit 0 clear: the object loses 1 hit point and is destroyed at 0; if it survives, sound $16;
  - else (bit 0 set): sound $16.
  - Pod damage, on the pod's fuel (RAM $C7 high byte of the 16-bit pair $C6/$C7, full = $E0 in the high byte,
    $E000 in the pair; a "unit" in this section is one step of the high byte = 256 in the pair, so damage 64 =
    $40 high byte = $4000 of $E000, 2/7 of a full tank, `economy.md` 3.3):
    - shield on (RAM $65 bit 7): no flag in bits 4-5: **nothing at all** (no fuel loss, and the routine ends
      here: no sound and no sparks either) [C]; bit 4 set: $C7 − $20; else (bit 5): $C7 − $08;
    - shield off: no flag in bits 4-5: $C7 − $40; bits 4 or 5 set: fuel set to 0 (the pod is destroyed at the
      start of its next step);
    - a result below 0 sets the fuel to 0. Then (in every case except the shield-on, no-flag one) the impact
      routine (bank 2 $8127) runs: sound $02/$04, sparks
      (`player.md` 5.5; skipped while $97 or $99 is non-zero). It is not the terrain damage routine: no x4, no
      $99. **[V]** for all four cells that were tried:

    | Test (Mesen, level 0 start, object placed on the pod) | Fuel change |
    |---|---|
    | enemy shot (state $15, flags $61), shield off | emptied, pod destroyed |
    | enemy shot, shield on | −$0800 (plus the shield's −1 per step) |
    | mine (state $1E, flags $04), shield off | −$4000 per step of contact, 1 hit point per step |
    | mine, shield on | no fuel change, 1 hit point per step |

    So enemy shots are instant kills without the probe shield, and cost 8 high-byte units ($0800) with it. A shot
    is destroyed by the touch (flag bit 6).

Worked case of the overlap test [C]: object w = 4, pod pw = 3 (so p = 7 − |d| + c). d = −7: |d| > w so c = 0,
p = 0 ≥ 0, overlap, penetration 0. d = −4: c = 1, p = 4. d = +3: c = 1 (d < w), p = 5. d = +4: c = 0, p = 3.
d = +7: c = 0, p = 0, not > 0: no overlap. d = −8: p = −1: no overlap. So the pod touches from 7 px to its left
but only up to 6 px to its right.

The pod's half sizes use integer halving: pw = $F1 / 2, ph = $F2 / 2, rounding down; $F1 and $F2 are the full
width and the half-height of the terrain test (`levels.md` 6.4 step 2), 6 and 6 normally (pw = ph = 3), growing with
the shield to 12 (pw = ph = 6), 9 (ph = 4) as Jetman or an exploding pod. Distances d are between the position
coordinates of the two objects; no reference-point offset is added (the offset (ow / 2, 4) belongs to the terrain
test only). **[V]** for 6, [C] for the grown values.

Contact with the types of levels 4-7 **[V]** (Mesen, 2026-10-17: probe shields off, $97 = 0, the pod placed on the object
for one step; the amounts follow the flag table of 3.6 exactly):

| Object (state) | Pod fuel (high byte, from $DF) | Object | Push seen |
|---|---|---|---|
| dome ($97, $A4), flags $20 | 0 (pod destroyed next step) | hit points −1 (48 to 47) | 24 px sideways |
| wide machine ($A3), flags 0 | $9F (−$40) | −1 (24 to 23) | 12 px up |
| ground gun ($AC), flags $81 | −$40 on each step of contact (4 steps: $9F, $5F, $1F, 0) | unchanged | 10 px sideways |
| walker ($99), flags $01 | $9F | unchanged (255) | 20 px up |
| formation part ($A9), flags $10 | 0 | −1 (8 to 7) | 13 px |
| satellite ($B9), companion ($BA), flags $20 | 0 | −1 | |

### 3.2 Shots

"Shots" are slots 3-10 with state ≥ 7 (the pod's bullets and missiles, and the captured companion of 4.18). A shot's damage is its own
hit points (field $0441 of the shot slot), set by the pod's weapon (`player.md`). **[C]**

### 3.3 Shots against bonus objects (slots 11-14) **[V]** for the star ($24) and the box ($25), **[C]** for the rest

Targets: slots 11-14 holding a bonus object (state $24 or $25, 5.10) or an item whose item kind is $0B (6.1).
For each shot (slots 3-10 in order) and each such target: hit if −6 ≤ shot x − target x ≤ 5 and the same for y.

- Target state $24: score +50; RAM $D2 = ($D2 AND $80) OR $41; a marker (state 6, 6.2) with field $0393 = 1 is
  created in the first free slot searched from 10 down to 3. **[V]** 4 stars (score +50 each, markers in slots 10, 9,
  8, ...; RAM $D2 read $61 right after the hit, the bonus-message routine adds bit 5, and $01 from the next step).
- Target state $25: score +25; RAM $C9 += $1F (at most $E0); RAM $D2 = ($D2 AND $80) OR $40; a marker with $0393 = 0. **[V]** 2026-10-17 (level 0, a box poked into slot 13 with field $0393 = $82 and a shot flown into it: score 1000 to 1025, $C9 $40 to $5F, $D2 $60 then $00, marker in slot 10 at the box's position, frame $FF for two steps and then B$A0, rising 1 px per step, the shot kept flying; with $C9 = $D0 or $C1 the result was $E0, with $C0 it was $DF). The same box held 27 px below the pod was not caught by the beam, so a pickup is not towable.
- Both: sound $42 (request slot A, RAM $F4, seen for a shot and for a touch); the target slot is freed; the shot is not removed.
- **Touch** (the pod or Jetman within 13 px on both axes, inclusive, `economy.md` 2.5) does the same for the box. For the star it does the same except that the marker gets $0393 = 0 (Q22). **[V]**
- Special type $0B item (the fuel pod): it explodes (state 4, field $0393 = 1, frame sequence from 0), its entry in the level's
  special list (RAM $0513 + index) becomes 0, and the high byte of the pod's fuel, RAM $C7, is set to $E0. **Only the high
  byte is written; the low byte RAM $C6 keeps its value**, so the tank holds $E0xx with xx the old low byte, up to 255/65536
  of a unit above the $E000 of a full tank (Q26) **[C]**. The shot is not removed.

Worked case: target x = 100: shot x = 94 (offset −6) hits, 105 (+5) hits, 106 (+6) misses, 93 (−7) misses; both bounds are inclusive, and the y test is the same. [C]

A shot that hits one of these goes on to the next shot (it is not also tested against placed objects this step).

### 3.4 Shots against placed objects (slots 23-30) **[C]**

Only slots 23-30 are targets (state ≥ 6, frame ≠ $FF): objects in 15-22 (enemy shots, children) cannot be shot.
With d = shot − object, the overlap test of 3.1 is used with pw = 2 horizontally; vertically ph = 2 when the shot is
above the object (d < 0) but ph = (RAM $F2) / 2 (normally 3) when below. The first overlapping object gets the hit
response (3.5), then:

- if the response was a **deflection**, the shot survives and the next shot is checked;
- otherwise the shot becomes an explosion (state 4, field $0393 = 1, sequence 0, sound $0A on the noise channel),
  unless its state is $12, $14 or $BA (those survive), **and the whole collision check ends for this step** (at most
  one shot hits a placed object per step).

### 3.5 Hit response and destruction **[C]**

Response to a shot, by the target state's flag byte F (3.6):

- **F bit 7: special response**, chosen by F mod 16:

  | F mod 16 | States (types) | Response |
  |---|---|---|
  | 0 | $86-$95 (turrets 01-10) | destroyed if the turret is open (work 6 = 2 or 3, 4.1); else deflect |
  | 1 | $98, $9D, $9E, $AC (ground guns 12, 17, 18, 47) | deflect if the shot's horizontal direction equals the gun's; else destroyed (only a shot coming from the front kills it). **[V]** on type 47 (flags X 1): a shot flying right killed it (medium explosion, +5), a shot flying left was deflected |
  | 2 | $A7 (type 21) | deflect if the vertical directions differ; else hit points − shot damage (destroyed at 0) |
  | 3 | $A8 (type 22) | deflect if the horizontal directions differ; else hit points − shot damage |
  | 4 | $B4-$B6 (level 26) | splits into up to 7 pieces (state $21) in slots 30 downward, one per frame step above $AF |
  | 5 | $B7 (level 26) | only when its frame is $B6: RAM $EC = 1, $ED = 0, hit points − 1 (destroyed at 0); else deflect |
  | 6 | $B8 (egg 5F) | adds a special-list entry of type $14 (an item, 6.1) at the egg's position, then destroyed |

- **Otherwise:** if F bit 3 is set, a bonus object is released into the first free slot 11-14 at the target's
  position: state $24, frame $55 (for a target in state $23: state $25, frame $64), field $0393 = $82. Then hit
  points − shot damage (not below 0); at 0 the target is destroyed; if it survives and F bit 2 is set, it is knocked
  back: both whole speeds = 6 and its direction bits copied from the shot. **[V]** Earlier text said F bit 0 set means
  no damage; that holds for the pod's touch (3.1) but **not for shots**: a walker (flags $01, 255 hit points) lost 8
  hit points per hit from a strength-8 shot, and with 5 left a hit destroyed it (large explosion, +5) (Q16). The
  damage is the shot's strength ($050E + 1, field $0441 of the shot). **[V]** A strength-16 shot took a dome (48)
  to 32, 16 and 0 in three hits; each hit turned the shot into a small explosion (state 4) and ended the step's
  shot checks.
- **Deflect:** sound $0E (pulse 1, second request slot). Every shot bounces: if
  px < py (3.1 penetrations) its horizontal direction flips and R2 + 0 is added to the fraction byte of its
  **vertical speed** (RAM $02D9 + slot), else its vertical direction flips and R2 + 1 is added to the fraction byte of
  its **horizontal speed** ($029B + slot); the byte wraps without carrying into the whole speed, and the position
  fractions are untouched. (The 0 and 1 are the carry of the comparison that chose the branch; the amount R2 itself is **[C]**.) **Then the state
  differs: an ordinary shot becomes state 1 (a spark); a shot of state $12, $14 or $BA (star parent, multi warhead,
  companion) keeps its state and goes on.** An earlier reading, that those three do not bounce at all, was wrong:
  they bounce and survive. **[V]** (Mesen, 2026-10-17: a homing missile and a multi warhead flown into a closed
  turret, state $88: the missile turned into a spark, the multi warhead stayed state $14 and bounced; in the demos 23
  of the 9,638 multi-warhead steps are such bounces, each flipping one direction bit and changing one speed fraction.)
  Because the bounce does not separate the shot from the object, a multi warhead that touches an object it cannot
  destroy bounces again on every following step until it has moved clear.

**Destruction** of the object in slot s (not if its frame is $FF):

1. Every slot 23-30 whose state is below 6 (explosions in progress) is freed.
2. A placed object's entry in the type-list copy (RAM $0460 + list index) becomes 0: it never respawns in this visit.
3. Field $0393 = 1. Size: slots 1-14 always large. Slots 15-30 by w + h of the frame: below 8 small, below 28
   medium, else large.
4. Small: state 4, sequence 0, sound $0A (noise). Medium: state 5, sequence 5. Large: state 5, sequence 11.
   Medium and large: sound $0C (noise). In all three sizes, if s is 23-30 the score rises by the entry of a
   four-byte table (bank 1 $89B8: hex 03 05 08 11) at (new state − 4): **3 for a small explosion, 5 for a medium or
   large one** (RAM $CB-$D0, units digit; `economy.md` for display). Slots below 23 give no score. An earlier
   reading ("small: no score") was wrong: the score call follows the common tail. **[C]**, [V] `objects.md` 2.5.

**Explosion** (states 4 and 5, field $03B2 = position in the sequence table bank 7 $8297, ROM 0x38297:
`00 01 02 03 FF 04 05 06 07 08 FF 09 0A 0B 0C 0D 0E 0F FF`): on the first step after the destruction and then
every 4th step, the frame becomes the next entry; $FF frees the slot. Small = frames 0-3, medium 4-8, large 9-15,
4 steps each. **[C]**

### 3.6 Per-state hit points and flags **[C]**

Flags: bank 1 $87F8 + state (states $00-$27) and $881D + (state − $80) (ROM 0x087F8, 0x0881D). Bit 7 special
response (low 4 bits = case); bit 6 destroyed by touching the pod; bit 5 harms the pod; bit 4 harms the pod more;
bit 3 releases a bonus when shot; bit 2 knocked back when shot; bit 0 no hit point loss on the pod's touch (it does
not stop shot damage, 3.5); bit 1 unused.

| States | HP | Flags | | States | HP | Flags |
|---|---|---|---|---|---|---|
| $0F missile | 2 | $61 | | $97, $A4 domes | 48 | $20 |
| $15, $16 shots | 1 | $61 | | $98, $9D, $9E guns | 8 | $81 |
| $17, $18 flames | 255 | $21 | | $AC gun | 0 | $81 |
| $19 drone | 1 | $40 | | $99-$9C walkers, pillars | 255 | $01 |
| $1A, $1B shots | 255 | $51 | | $9F, $A0, $A1 turrets | 10, 10, 14 | 0 |
| $1C, $1D heads | 16, 12 | 0 | | $A2, $A3 machines | 24 | 0 |
| $1E mine | 4 | $04 | | $A5, $A6 magnets | 4, 8 | $04 |
| $1F hopper | 4 | $0C | | $A7, $A8 | 16 | $A2, $A3 |
| $20 | 2 | $40 | | $A9, $AA formations | 8, 6 | $10, $20 |
| $21 | 1 | $40 | | $AB green creature | 2 | $24 |
| $22 | 0 | $01 | | $AD-$B3 generators | 255 | 0 (never hit: frame $FF) |
| $23 green orb | 1 | $0C | | $AF blue sphere | 3 | $04 |
| $86-$95 turrets | 3 | $80 | | $B4-$B6, $B7, $B8 | 1, 10, 1 | $84, $85, $86 |
| | | | | $B9, $BA | 16, 4 | $20 |

The pod's weapon shots (5.11) have flags 0 in this table (they never touch the pod). Their hit points are the damage
they do: $0F homing missile 2, $10 time bomb $40, $12 star parent $50, $13 star child 1 in the table but $10 as made,
$14 multi warhead $FF. A shot's table value is written into the shot at its creation (5.11), so the gun's own 1-3 does
not apply to special weapons.

## 4. Placed objects (slots 23-30), by type

Initial frame, state and hit points: `objects.md` 4.2. Field $0393 holds the list index and $03B2 the type unless a
rule reuses them.

### 4.1 Turrets, types 01-10 (states $86-$95) **[V]**

These are fixed guns, each firing in one direction; `objects.md` calls them collectables (a correction). Types 04,
06, 08, 0A, 0C, 0E and 10 (states $89, $8B, ..., $95) set flags X = 1 every step (mirrored picture). Work 6 = phase, work 5 = timer, both 0 at spawn:

| Phase | Each step | Next |
|---|---|---|
| 0 rest | work 5 − 1 (8-bit) | when work 5 mod 64 = 0: work 5 = 0, phase 1. From spawn this takes 63 steps |
| 1 open | when F mod 8 = 0: work 5 + 1, frame + 1 | at work 5 = 2: phase 2, work 5 = (R mod 16) + 4 |
| 2 wait | work 5 − 1 | at 0: phase 3, work 5 = 4 |
| 3 wait | work 5 − 1 | at 0: **fire**, phase 4, work 5 = 0 |
| 4 close | when F mod 8 = 0: frame − 1, work 5 + 1 | at work 5 = 2: phase 0, work 5 = (R1 AND $2F) + 9 |

So the turret shows its base frame, base + 1, base + 2 (open, 4-19 + 4 steps), base + 1, base; rests 9-24 or 41-56
steps (63 the first time). It can only be destroyed while open (phases 2-3, 3.5).

**Fire** (first free slot 15-22): standard shot (2.6) at the turret plus an offset, with fixed speeds, per type
(t = type − 1; tables bank 7 $C634 dx, $C644 dy, $C654 x speed, $C664 y speed, $C674 vertical direction, ROM
0x3C634..):

| Type | dx, dy | Speed x, y (px/step) | Vertical | Horizontal |
|---|---|---|---|---|
| 01 | 0, −6 | 0, 2 | up | right |
| 02 | 0, 7 | 0, 2 | down | right |
| 03, 04 | 5, −8 / −5, −8 | 0.5, 1.5 | up | right / left |
| 05, 06 | 5, 8 / −5, 8 | 0.5, 1.5 | down | |
| 07, 08 | 8, −6 / −8, −6 | 1.25, 1.25 | up | |
| 09, 0A | 8, 6 / −8, 6 | 1.25, 1.25 | down | |
| 0B, 0C | 10, −4 / −10, −4 | 1.5, 0.5 | up | |
| 0D, 0E | 10, 4 / −10, 4 | 1.5, 0.5 | down | |
| 0F, 10 | 6, 1 / −6, 1 | 2, 0 | up | |

Horizontal direction: right for types 01 and 02, else the turret's flags X (0 for the odd types, 1 for the even
ones). Speeds are the table byte × 4 / 256. Sound $08. Turrets do not aim at the pod.

**[V]** every type: types 05, 06, 08, 0B, 0C, 0F on level 0 (2026-10-16) and, on 2026-10-17, types 01, 02, 03, 04,
07, 08, 09, 0A, 0B, 0D, 0E and 10 on levels 3, 6 and 8 (phase, timer and frame of 17,000 turret-steps and the
position, speeds and directions of the shots, each type fired at least four times, all as in the tables above). A
turret whose shot finds slots 15-22 all in use fires nothing (the phase still advances), which happens when
several turrets fire slow shots at once.

### 4.2 Ground guns, types 12, 17, 18, 47 (states $98, $9D, $9E, $AC) **[V]** single guns and pairs

Types 12, 17, 18 and 47 start with a copy of their position in work 1-4 (home, `objects.md` 3.4). Type 47 sets flags
X = 1 every step; type 18 sets flags Y = $81 (upside down, firing down) every step. Every step the gun moves 1 px
per axis toward home (it returns after a knock-back).

- **Single gun** (12, 47, and 17/18 without a partner): work 5 − 1; when work 5 mod 32 = 0, fire (slot 15-22):
  state $1A, frame $52, lifetime $80, at (x ± 2, y ∓ 4) (dx = −2 if flags X bit 0, else +2; dy = +4 if upside down,
  else −4), vertical speed 8.00 in the gun's vertical direction, sound $08. State $1A: move y only; frame + 1 each
  step until $54; lifetime − 1, freed at 0. **[V]** types 12 (level 5, 400 steps), 18 (alone on level 5, 500 steps)
  and 47 (500 steps): every field of the guns and their shots reproduced; 8 px per step takes a shot out of the
  window long before its 128 steps are up. The shot keeps the slot's old position fractions (2.5).
- **Pair** (a type 17 floor gun, state $9D, and a type 18 ceiling gun, state $9E, with the same x low byte; first on
  level 8, also levels 11 and 15; in the level data the 18 is 53-82 px above the 17) **[V]** (level 8, 400 steps with
  two pairs; level 15, 100 steps): **no shots**, an electric barrier between the two guns.
  - **Finding the partner.** A gun whose work 6 is 0 looks, in ascending order, through slots 23-30 for an object in
    the partner's state (the 17 looks for state $9E, the 18 for $9D) **with the same x low byte** (the high byte and
    the y are not compared, so two guns 256 px apart in x would pair; no level has that). Found: the slot is kept in
    work 6. Every later step the link is only checked: the partner's slot must still be in the partner's state;
    if not, work 6 is cleared and the gun acts as a single gun (above) for that step and searches again on the
    next. A gun that runs before its partner exists (the partner is created later in the same spawn scan, or sits
    in a higher slot) acts as a single gun for that one step: work 5 goes from 0 to $FF, which fires nothing.
  - **Who acts.** Only the **floor gun (type 17)**. The ceiling gun does nothing once it has a partner (work 5 stays where it is, 0 or $FF
    as above; work 6 = the partner's slot).
  - **Floor gun.** Work 5 − 1 every step; when work 5 mod 64 = 8: work 5 = $30 and a flame (state $18, frame $60,
    5.2; its first drawn frame is $61) is created in the first free slot 23-30 at (x, y − d / 2), x the gun's, d = (y −
    partner y) mod 256 (so 32 px up from the floor gun for guns 64 px apart); if no slot is free nothing is made. The
    first flame comes 56 steps after the gun appears, then one every 40 steps; each lives 11 steps
    (frames $61 $62 $63 $62 $61 $61 $62 $63 $62 $61 $60). 99 flame-steps and 9 flames reproduced exactly.
    The flame harms the pod like the dome's flames (5.2).
- Shot from behind destroys it at once; from the front deflects (3.5). **[V]** (3.5, case 1). Touching it costs the pod
  $40 per step and the gun nothing (3.1).

### 4.3 Walkers, types 13, 14 (states $99, $9A) **[V]**

On its first step the walker creates a head in the first free slot above its own (s + 1 ... 30): type 13 → state $1C
frame $50, type 14 → state $1D frame $53, at (x, y − 25), work 6 = walker slot; the walker keeps the head's slot in
work 6. The head runs in the same sweep (its slot is higher). If the head slot later becomes free, the walker makes
a new head on the next step; if the head is exploding (state 4 or 5) the walker is destroyed **[C]**, those two
cases were not seen.

Movement (vertical only), frames $5D standing, $5E crouching, $5F in the air:

- Standing (vertical speed 0): work 5 − 1. If the frame is $5E (crouching): when work 5 mod 8 = 0, with chance 1/4
  (R mod 4 = 0) jump: remember y in work 1-2, direction up, vertical speed 8.00, frame $5F; else frame $5D. If the
  frame is not $5E: when work 5 mod 64 = 0, frame $5E.
- Jumping: each step going up the speed slows down by $80 (from 8.00, so 7.50, 7.00, ... 0.50), then the walker
  moves by the new speed. When the speed reaches 0 the direction becomes down and the speed becomes 0.01 (the
  fraction byte is set to 1), and **that step has no move**. Going down: speed up by $80, then move. When y is back at
  the remembered y (or below it) the walker lands there: speed 0, frame $5E.

A jump rises exactly 60 px (7.50 + 7.00 + ... + 0.50), takes 16 steps up and then 23 steps down (the fall speeds up by
0.50 per step but stops at the 3.00 cap, 2.2), 39 steps in all. **[V]** Walkers 13
(1,190 steps) and 14 (1,199 steps) on level 5 reproduced field by field, including 7 jumps; the crouch to jump
timing and the 1/4 chance use R as written. The head rules are in 5.3. Shots do hurt a walker: 3.5, Q16.

### 4.4 Pillars, types 15, 16 (states $9B, $9C) **[V]**

Like the walker, a pillar makes a head in the first free slot above its own: type 15 → state $1C frame $58, type 16
→ state $1D frame $55, at (x, y − 17). Head slot gone → new head next step; head exploding → pillar destroyed.
Pillars cannot be damaged (flags $01).

Every step: vertical speed = 1.00. Going up: if the frame is $7D, turn (direction down, no move this step); else
frame + 1 and move y (1 px up). Going down: if the frame is $68, turn; else frame − 1 and move 1 px down. So a
pillar grows from $68 to $7D and back, 21 steps each way plus one step at each end. The head is kept at
y − (frame − $57) above the pillar's point (5.3).

**[V]** 2026-10-17, level 1: type 15 (397 steps) and type 16 (497 steps), pillar and head, frame by frame including the
head's animation and its shots toward the pod, 0 mismatches.

### 4.5 Turrets, types 19, 1A, 1B (states $9F, $A0, $A1) **[V]**

- 19 / 1A (hourglass; 1A sets flags Y = $81 every step, ceiling): work 5 − 1; when (work 5 before the decrement) mod
  128 = 0, launch a drone (5.5) in the first free slot 26-30 at (x, y − 16), or (x, y + 16) for 1A; the drone's
  vertical direction = up for 19, down for 1A. The first launch is on the first step (work 5 starts at 0).
- 1B: work 5 − 1; when (work 5 before) mod 64 = 0, launch a drone at y + 24 going down if work 5 bit 6 was clear, else
  at y − 24 going up (alternating: down, up, down, up at work 5 = 0, $C0, $80, $40).
- **[V]** 19 (level 4), 1B (level 4) and 1A (level 6): 5, 5 and 4 launches, each at the stated place, time and
  direction; the drone is already one step old in the first record (it has moved about 2 px; its fire timer, work
  5, was left over from the slot's previous occupant, 2.5).

### 4.6 Wide machines, types 1C, 1D (states $A2, $A3) **[V]**

1D sets flags Y = $81 every step. Work 6 − 1; when work 6 mod 32 = 0 and a slot 15-22 is free, an aimed shot (2.8) with
table entry 7 (1C) or 10 (1D) **and the step ends there** (no opening cycle in that step) **[C]**. Otherwise
the opening cycle of 4.8 with rest frame $3F, open frame $41 and missile entries 5/6 (1C) or 8/9
(1D). **[V]** both, level 4, 800 steps: 1,595 object-steps, shots and missiles reproduced.

### 4.7 Domes, types 11, 1E (states $97, $A4) **[V]**

Work 5 bit 1 = lifted off (type 1E only; it is never cleared again, the dome does not land). Each step:

1. **Lifted off:** with m = (s + F) mod 256: for m below 128 move (with its current speeds); for m exactly 128 create
   an exhaust (state 3, frame $44, work 6 = dome slot, at (x, y + 24)) in the first free slot searching 30 down to 23
   (slot 22 is used if none is free, 5.1) and do nothing else this step; for m above 128 home on the pod on both axes
   (accel 3, M = $80, x first), set flags X bit 1, and **then move once more with the same speeds**: in this half of
   the cycle the dome travels at twice its speed (Q14). Then go to 4.
2. **Type 1E on the ground:** when F mod 64 = 0 (every 32nd step) and R1 mod 8 = 3, lift off (work 5 bit 1); the
   first step of the lifted behaviour is the next step. Then go to 4 (no flames). An earlier reading had F mod 128.
   **[V]** lifts seen at F mod 128 = 0 and 64 alike (the frame counter was poked to shift the phase).
3. **Type 11:** if d < 255, and 16 ≤ |dx| < 56, |dy| < 8 and the dome is above the pod (dome y < pod y): the pod's
   vertical direction flips and the pod moves |dy| in its new direction; a flame (state $17, frame $60) is created
   toward the pod's side at (x ± 31, y + 7) in slot 25 (pod right) or 24 (pod left), if free. Independently, when
   slots 24 and 25 are both free and (R1 / 2) mod 64 = $17, a flame on the side given by R1 bit 0 (set: right).
   **[V]** the limits to the pixel (|dx| 15 and 56, |dy| 8 give no flame; 16, 55 and 7 do; the left side the same),
   both flame slots, and the pod's move: a pod held 4 px below the dome row ended on the dome's row, 4 px up (it was
   heading down; the original flips whatever the direction was, so a pod heading up would be sent down; not seen).
   The pod's move comes after the dome's own test and before its aimed shot, so the shot sees the moved pod. The
   flames repeat every 12 steps while the pod stays there.
4. Work 6 − 1; when work 6 mod 32 = 0 and a slot 15-22 is free, an aimed shot (2.8) with entry 0, 1 or 2 (R mod 4 = 1 → 0,
   2 → 1, 0 or 3 → 2) **and the step ends there: steps 5 and 6 do not run** (no opening-cycle change, no drone launch, no
   RAM $5A change) **[C]**. With no free slot nothing is shot and steps 5 and 6 run.
5. The opening cycle (4.8) with rest frame $34, open frame $36, missile entries 3/4.
6. Type 11 only: if R1 mod 64 = $17, launch a drone (5.5) at (x, y + 16) in a slot 26-30. **[V]** (5 launches, each
   at y + 16 plus the drone's first move).

**[V]** Mesen, level 4: dome 11 (about 900 steps over three runs, 4,500 shot-steps, 650 missile-steps, 740 flame-steps)
and dome 1E (about 3,200 steps over five runs on levels 4 and 7 with shifted frame counters, 63 exhaust steps, 40
lift-offs): every field of the dome, its shots, missiles, flames and exhaust reproduced, except the corrections
above and the near-pod pushes (3.1).

### 4.8 Opening cycle with missile (domes, wide machines) **[V]**

Work 5 bit 0 = closing. While the frame is not the rest frame, every 16 steps ((s + k) mod 16 = 0): opening and
not yet at the open frame → frame + 1; at the open frame → fire a missile (2.9) from a free slot 15-22 at the entry
(R1 bit 0 chooses which of the two entries; the missile's first frame depends on the side and on the maker's
orientation, 2.9), then set closing (also when no slot is free); closing → frame − 1, and at the rest frame clear
closing. At rest: when F mod 32 = 0 and R mod 8 = 0, frame + 1 (start opening) and RAM $5A += $49 (later readers
in the same sweep see the new value).

### 4.9 Small blocks, types 1F, 20 (states $A5, $A6): magnets **[V]**

They do not move; they pull (1F) or push (20) the pod. Each step, with d, |dx|, |dy| from 2.4:

- If d ≥ 191: nothing.
- Animation: the 16-bit counter work 5 (high) : work 6 (low) += 2 × (192 − d); frame = table[work 5 mod 4], flags X
  = (0, 0, 1, 1)[work 5 mod 4]; table 1F: $7E $7F $7F $7E, 20: $80 $81 $81 $80. Closer means faster spinning.
- Force on the pod (skipped while RAM $D2 bit 7 is set and RAM $D1 = 2, an equipment effect, `player.md`):
  vertical amount = (192 − |dy|) / 8, horizontal amount = (192 − |dx|) / 8 for type 20 but (192 − d) / 8 for type 1F
  (in 1/256 px per step; integer division). Type 1F: on each axis, if the pod moves toward the block, speed up the pod
  by the amount (2.2 cap), else slow down and turn (magnet variant). Type 20: the opposite (speed up away, slow down
  and turn when moving toward). Several blocks add up.

Example (verified): pod 83 px left and 24 px above a 1F block on level 1, gravity 6: after the step the pod moves
right at 10/256 and down at 6 + 21 = 27/256 px per step.

### 4.10 Large creature, type 21 (state $A7) **[V]**

Shuttles vertically between its two waypoints (2.7): vertical homing toward the current target's y (accel 1, M =
$80, i.e. 0.5 px per step); no horizontal movement; flags Y = $81 while moving down (drawn upside down), else 0.

- While its vertical speed is increasing, two thruster flames (state $22, frame $89 + (R mod 4)) are written into
  slots 15 and 16 (whatever was there) at (x + 18, y + e) facing left and (x − 18, y + e) facing right, with e = +25,
  or −25 while moving down, and the creature's flags Y. "Increasing" means its vertical fraction byte grew this step; when not speeding up, the frames of slots 15 and 16 are set to $FF (hidden). State $22 does nothing
  but free itself when its owner (field $03B2) has a state below 6.
- Work 6 − 1. When work 6 mod 32 = 0 and |dy| to the pod < 80: a shot (slot 17-22) toward the pod's side at
  (x ± 23, y), 4 px per step, frame $84, the creature's vertical flags. When work 6 mod 32 = 16: a shot (slot 17-22) at
  (x, y ± 32) (below when upside down) with vertical speed 5.00 in the creature's direction, horizontal speed R1/256
  in a random direction (R bit 1), frame $33. Both shots are state $15 with lifetime $C0.
- Shots kill it only when moving vertically in its direction (3.5).

**[V]** 2026-10-17, level 8 index 76 (1,296 creature-steps, 2,590 thruster-steps, 4,400 shot-steps; a second run with
the pod close enough for the aimed shots, 695 creature-steps): movement, flags, thrusters, both shots (position,
speeds, direction, frame, lifetime; the second shot's horizontal fraction is R1 and its direction R bit 1), 0
mismatches; the shots that leave the window are freed at once as usual. The pod stayed within 256 px
horizontally, so the |dy| test of the first shot was only seen with a valid |dy|. The waypoint results are in 2.7.

### 4.11 Large machine, type 22 (state $A8) **[V]**

Shuttles horizontally between its waypoints (2.7) (accel 1, M = $80, x only; the first step flips to P2 because the
spawn point is within 16 of P1). Work 6 − 1; when work 6 mod 128 = 0 (a "tick"): if the frame is $85, the frame
becomes $86, work 6 = 7 and the low three bits of work 5 are cleared; else work 5 + 1 and frame = ($86, $87, $87, $86,
$85)[work 5 mod 8], and on the third of these (index 2, frame $87) the machine fires its volley; then work 6 = 0 if
the frame is now $85 (the next tick is 128 steps later), else 7. So the cycle is 128 steps of rest, then ticks 8
steps apart: $86, $87, $87 (fire), $86, $85.

- **Volley:** four state-$15 shots **written unconditionally into slots 15, 16, 17 and 18** (whatever they held, Q15)
  at (x + e, y − 20) for slots 15 and 16 and (x + e, y + 12) for 17 and 18, e = +1 when the machine faces left (flags
  X bit 0), −1 otherwise; frame $88, lifetime $C0, the machine's flags X; slots 15 and 17 go up, 16 and 18 go down. The
  speeds are: horizontal 3.00 plus the direction flag (so 3.004 for the down-going ones), vertical $D8/256 = 0.84 px
  plus 1.00 for the down-going ones (1.84). The slot's old position fractions and work bytes stay (2.5).
- Between ticks (work 6 mod 128 ≠ 0), when R1 mod 64 = $16 and slot 19 is free: a shot from slot 19 at (x ± 12,
  y + 1) (left when facing left), horizontal 4.00 in the machine's facing direction, vertical 0, frame $33, lifetime
  $C0.
- Shots hit it only from the front (3.5, case 3) **[C]**.

**[V]** Level 4 (two machines, indexes 5 and 7, waypoints 2.7) and level 7 (index 34): 3,000 machine-steps, volleys
and extra shots reproduced; the rows of the waypoint table in 2.7 for levels 4 and 7 are confirmed (the machine
coasts past P2: x down to 1576 for P2 = 1592). Level 12 (indexes 32, 68 and 69, 2026-10-17): 3,700 more machine-steps
reproduced, the three table rows confirmed; the only differences are respawns after the machine left the window.

### 4.12 Formations, types 23-32 (state $A9) and 33-42 (state $AA) **[V]**

(State $A9 was run on level 7 on 2026-10-17: six parts, types 2D, 23, 25, 27, 29, 2B placed at one point; 7,179
part-steps reproduced exactly, including the leader hand-over and the frame table of $A9. Note: the part's first step
also copies the leader's phase, so a follower's phase is the leader's current phase + 1 from its first step.)

All parts of one group are placed at one point (`objects.md` 3.3); they then move on a circle of radius 64 around
it.

- First step: the part copies its position into work 1-4 (home) and sets work 6 bit 0. If another part of the same
  state in slots 23-30 has work 6 bit 1 set (a leader), it follows it (work 5 = leader's slot); otherwise it becomes
  the leader: phase (its horizontal sub-pixel byte, RAM $0197 + s) = 0, work 6 bit 1 set, and every other part of the
  same state gets work 5 = this slot and loses bit 1.
- Each step: a follower whose leader slot no longer holds this state becomes the leader. A follower copies the
  leader's phase. Then the phase increases by 1. (The leader runs first, so followers are one step ahead.)
- Angle a = phase + 16 × (type − first type) (mod 256), first type $23 or $33. With q = a / 64 (0-3), i = a mod 64,
  j = 63 − i when q is odd else i, and T the 128-byte table at bank 7 $B399 (ROM 0x3B399):

  ```
  x = home x + T[j]        when q = 0 or 1,   home x − T[j]        when q = 2 or 3
  y = home y − T[j + 64]   when q = 0 or 3,   home y + T[j + 64]   when q = 1 or 2
  ```

  T: `01 03 05 07 09 0B 0D 0E 10 12 13 15 17 18 1A 1B 1D 1E 20 21 22 24 25 26 27 29 2A 2B 2C 2D 2E 2F 30 31 32 33 34
  34 35 36 37 37 38 39 39 3A 3A 3B 3B 3C 3C 3C 3D 3D 3D 3E 3E 3E 3F 3F 3F 3F 3F 3F 40 40 40 40 40 40 40 40 3F 3F 3F 3E
  3E 3E 3D 3D 3C 3C 3B 3B 3A 39 39 38 37 36 36 35 34 33 32 31 30 2F 2E 2D 2C 2B 29 28 27 26 24 23 22 20 1F 1D 1C 1A 19
  17 15 14 12 10 0F 0D 0B 09 07 05 03 01`. So the point turns clockwise from the top, 256 steps per turn.
- Frame, state $AA: table bank 7 $B422 [a / 8] (`91 92 92 93 93 94 94 95 95 94 94 93 93 92 92 91` twice); with n =
  a / 8 + 1, flags X = 1 if n mod 32 ≥ 16, flags Y = $80 if 8 ≤ n < 24. State $A9: frame = bank 7 $B41A [j' / 8]
  (`8D 8E 8E 8F 8F 90 90 8D`), j' = i when q is odd, 63 − i when q is even; flags unchanged.
- **Firing (state $AA only)**, before moving (so from the position and frame of the previous step): the vertical sub-pixel byte is a timer: − 1 each step; at 0 it is
  reloaded with R (1-256 steps) and the part fires (slot 15-22) a standard shot from its current position plus
  (dx, dy) = (0, 14), (7, 11), (9, 9), (11, 7), (14, 0) for frames $91-$95; flags X of the shot = part's flags X
  XOR 1, dx negated when that is 1 (left); dy negated and vertical direction up when the part's flags Y bit 7 is set,
  else down; speeds |dx| / 4 and |dy| / 4 px per step. Sound $08.

### 4.13 Green creatures, types 43-46 (state $AB) **[V]**

- First step: flags X, Y = (0, 0), (1, 0), (0, 0), (0, $81) for 43, 44, 45, 46. Then for 127 steps it only animates:
  work 5 counts down from $FF; frame = base + (work 5 / 4 mod 2), base $96 (43, 44) or $98 (45, 46). Active when work 5
  mod 128 = 1.
- Active: it chases the pod along one axis at a time, the vertical sub-pixel (horizontal mode) or the horizontal
  sub-pixel (vertical mode) serving as the timer.
  - **Choose:** |dx|, |dy| to the pod (16-bit; quirk: when the creature is left of the pod the low byte of |dx| is
    taken as 64). If |dy| < |dx|: horizontal mode, timer = (R mod 16) + 64, vertical speed 0. Else vertical mode,
    timer = (R mod 16) + 65, horizontal speed 0. If no mode was set (work 6 = 0) the new mode starts at once; if the
    new mode equals the old one it continues (on a repeated vertical choice the creature stands still for that
    step, and the next repeated vertical choice is taken for a change of mode and gives a pause: Q25); if it differs,
    work 6 = 16: a pause.
  - **Horizontal mode** (work 6 = 1): timer − 1, at 0 choose again; home in x (accel $40, M = $83, so at most 3.00);
    frame $96 + (F / 8 mod 2).
  - **Vertical mode** (work 6 bit 7): timer − 1, at 0 choose again; home in y (same); flags Y = $81 while moving down,
    else 0; frame $98 + (F / 8 mod 2).
  - **Pause** (other work 6): work 6 − 1 each step, no movement and no frame change; when it was 2, choose again.
- **Order within a step.** Everything above happens in the step in which it is decided; nothing waits for the next one:
  - a new mode (work 6 was 0) runs one step of the mode at once, with its timer already counting (the timer is set
    and then decremented in that step), so a mode with timer T = 64 + r (65 + r) steps lasts T steps, T − 1 of them with
    a move and the last one only choosing;
  - when a timer reaches 0 the creature chooses again in that same step and, if the mode is unchanged, moves in it (a
    changed mode starts a pause, below) **[C]**;
  - a pause is set to 16 and decremented in the same step, so its first step shows work 6 = $0F and it counts down
    to 2 ($0F ... 2: 14 steps without a move); in the step after the one that showed 2 the creature chooses and starts
    the new mode at once. **[V]** work 6 of the level 2 creature (slot 25) over the 900-odd steps of the run: a pause is first shown as $0F
    in the step it begins (steps 208, 289, 507, 665, 831), counts down to $02, and the step after that already shows the
    new mode ($FF or $01).

### 4.14 Generators, types 48, 49, 4B-4E (states $AD-$B3) **[V]**

Invisible (frame $FF) and never hit. States $AE, $B1, $B3 (types 49, 4C, 4E) act only every 8th step (work 6 + 1
each step; act when work 6 mod 8 = 0); $AD, $B0, $B2 (48, 4B, 4D) consider every step. When acting:

1. RAM $5A += $61 (this changes the generator state seen by every later reader).
2. Continue only if the new $5A mod 64 = $1F and RAM $5E is even (half of each 512-frame period).
3. Create, at a random edge position (2.10):
   - 48, 49: a mine (state $1E, frame $9A, 5.6) in the first free slot 27-30;
   - 4D, 4E: a hopper (state $1F, frame $A3, work 6 = $A3, 5.7) in the first free slot 27-30;
   - 4B, 4C: a drone (5.5) in the first free slot 26-30, with sound $18.

On average one creature per 128 steps (every-step kinds) or per 1024 steps (every-8th kinds) while the window is open.
The new object starts with all position fractions and speeds cleared (2.5); a drone keeps the previous occupant's fire
timer (work 5), mines and hoppers have no stale field that matters **[C]**.

**[V]** type 48 (level 4, 2,000 steps in each of two runs): 1,998 generator-steps reproduced, and the 8 mines it made
appeared on the predicted steps, in the predicted slot and at the predicted edge position (2.10). The mines are
never destroyed in these runs, so the four slots 27-30 fill up and the generator makes nothing more (a real
player would shoot them).

**[V]** 2026-10-17: type 4B (state $B0, level 4 index 20, 3,000 steps: 3 drones, the first one step old at its
first record) and type 4E (state $B3, level 8 index 15, 4,000 steps: 3 hoppers, plus one creation at step 2,559
whose edge position (2.10, Q10: offset added to the low byte only) fell 161 px left of the screen, outside the window, so the hopper
was freed in the same step); type 4D (level 8 index 44, 1,500 steps: 5 hoppers) reproduced on the same steps, in the
same slots, at the same edge positions. The every-8th-step kinds act when work 6, after its increment, is a
multiple of 8.

### 4.15 Blue sphere, type 4A (state $AF) **[V]**

Hit points 3, flags $04 (knocked back when shot). Work 6 counts up once per step until it reaches 63 and then stays
there: nothing else happens on the steps up to 63, so the sphere sits for 63 steps after spawning on frame $9E (a
small sphere). Then, while the frame is below $A1, frame + 1 when F mod 8 = 0 (every 4th step: $9E → $9F → $A0 →
$A1). At frame $A1 it chases the pod: home in x (accel 8, M = $04: 3.00 max after the cap) and y (accel $18, M =
$04), x first, each followed by its move. Work 5 − 1 (it starts at 0, so it counts down from $FF); when work 5 mod 32
= 0, |dy| to the pod < 24 and it is moving toward the pod horizontally (flags X 0 with the pod right, 1 with the
pod left): a shot (slot 15-22, none free: nothing) at (x ± 24, y − 2), the sphere's flags X and Y, horizontal speed
4.00 (the **fraction byte of that slot's horizontal speed keeps its old value**, Q17), vertical speed = the sphere's
vertical speed, frame $A2, lifetime $C0, sound $08. **[V]** 2026-10-17, level 8 (type 4A index 61, 680 sphere-steps
in two runs): the idle time, the three frame changes, both homing axes, and all 25 firing decisions (11 shots; the
check at every 32nd step) as written. The pod's push (3.1) explains the few differences within 16 px of the pod.

### 4.16 Level 26 objects, types 4F-5E (states $B4-$B7) **[V]**

Level 26 is the final level; its camera follows a fixed route (`levels.md` 5.2). Types 50-5E are placed (47 objects;
4F is placed nowhere). All have the pod's normal collisions (3.1) and shot responses (3.5). Four behaviours, three of
them sharing one prelude:

**Prelude of $B4, $B5, $B6** (run before their own behaviour, with the camera as the objects see it, that is the
camera of the previous step: the objects run before the camera moves): let P = the high byte of the camera x (RAM
$4D, the 256-px page).
- P < $16: nothing, the behaviour below follows.
- P = $16 or $17 (camera x 5632-6143): the object speeds up horizontally by $20 per step (cap 3.00) and moves (both
  axes, its flags X and Y), and **nothing else runs**. **[V]** (objects set into the table at that camera, 40 steps.)
- P ≥ $18 (camera x 6144 or more): the object is freed. **[V]**

**$B4 (types 4F-53; initial frame $AF-$B3 in type order; hit points 1):** work 6 counts 0, 1, ..., $10, one per step
from the first step after the object was made (the object waits 16 steps; the 17th step is the one that sees $10).
On that step: work 6 = $80, flags X (RAM $0336) = 3 (moving **left**, whatever side the pod is on; the byte is
a constant, not computed from the pod), flags Y = 1 (down) if the pod-below flag of 2.4 says the object is above the
pod, else 0 (up), horizontal and vertical whole speeds = 2 (the fraction bytes are kept, Q17), and **the step ends
there: no move and no homing**. The next step is the first that homes and moves **[V]** (p26full, slot 23: counter 1 at
step 89 ... $10 at 104, speeds 2.00 and flags set at 105 with the position unchanged, first move 106). From then on (work 6
bit 7 set) it homes on the pod on both axes (accel 8, M = $04, x first, each followed by its move: it speeds up to 3.00). Because
the pod-below flag is 0 whenever |dx| ≥ 256 (2.4, Q19), an object far to the side of the pod starts upward even
when it is above the pod (the 3 of 6 starts seen with |dx| ≥ 256 that were above it).

**$B5 (types 54-58; initial frames $AF-$B3; hit points 1):** on its first step: horizontal whole speed 2, flags X
= 1 (left), moves once along x (this one **does** move on its start step), work 6 = $80; from then on it homes on the
pod exactly like $B4 (accel 8, M = 4, both axes).

**$B6 (types 59-5D; initial frame $FF (invisible), hit points 1):** waits (not moving) until the camera x (16-bit)
is at or beyond the object's x. Then: horizontal speed = 4 + R / 256 (whole 4, fraction = RAM $5A at that moment),
flags X = 0 (right), frame = type + $56 (so types 59-5D become the frames $AF-$B3 of types 54-58), work 6 = $80, and
**the step ends there: it does not move on its start step** (like $B4, unlike $B5); from the next step it homes like $B4
(accel 8, M = 4, both axes). Its vertical speed and flags Y are not touched (zero from its making). (Types 59-5D occur 4, 2, 4, 1 and 2 times.)

**$B7 (type 5E, five of them at x 6794-6850, y 555-714; initial frame $B4, hit points 10):** every step: returns 1 px per
axis toward its home position (a copy of its list position in work 1-4, as for the ground guns), hit points are
capped at 10, flags X = 1. Every 4th step ((s + F) mod 8 ∈ {0, 1}, s its slot, F as in section 1; this is one step in
four): with work 6 = 0 it only counts work 5 + 1 and, when work 5 mod 32 = 0, loads work 5 = the low 5 bits of R (of R1 when s is odd)
and starts the animation (work 6 = 1); in the animation work 6 steps 1, 2, ..., 7, 0 on these 4th steps
and the frame is ($B4, $B5, $B6, $B6, $B6, $B6, $B6, $B5)[work 6]. So it rests on $B4 for (32 − work 5) × 4 steps
(up to 128, then shorter by the random start), then opens ($B5, five steps of $B6, $B5) over 28 steps and rests again.
It is hit only while its frame is $B6 (3.5 case 5): hit points − 1 per hit (one hit seen, 10 to 9). At other
frames a shot is deflected.

**Direction bytes and frames [C], seventh round.** Nothing here is computed: the flags X byte (RAM $0336) is set to the
constants above (3 for $B4 at its start, 1 for $B5 at its start, 0 for $B6 at its start, 1 for $B7 on every step) and is
afterwards only changed by the homing (2.2). The frame is a constant (the table, or type + $56 for $B6). The pod's own
frame $2D, or $2E when any direction is held and bit 2 of RAM $5D is set, and its direction bytes in the final level
are `player.md` 9.1 (read again: the frame is chosen only by that rule; the horizontal byte is 0 for right, 5 for left,
compared as a whole byte; the vertical one 0 for up, 1 for down).

**Shots on $B4-$B6 (3.5 case 4) [V]:** with n = frame − $AF: at n = 0 (frame $AF) the object is destroyed. Otherwise
its frame becomes $AF, its hit points 1, and up to n pieces (state $21, below) are made: for Y = 7, 6, ..., 0 in
turn, if slot 23 + Y is free, a piece is put there until n are made (so fewer when slots are busy: a frame-$B2 object
(n = 3) with slot 30 busy made 2). Piece Y: position = the object's, frame $AF, hit points 1, work 6 = $80, whole
speeds and directions from the table (Y = 0 ... 7):

| Y | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| horizontal whole speed | 3 | 2 | 1 | 0 | 0 | 1 | 2 | 3 |
| vertical whole speed | 0 | 1 | 2 | 3 | 3 | 2 | 1 | 0 |
| flags X | 1 | 0 | 1 | 0 | 1 | 1 | 0 | 0 |
| flags Y | 1 | 1 | 0 | 0 | 0 | 1 | 0 | 1 |

(ROM 0x08AF9-0x08B10 in bank 1; fraction bytes and the counter $0393 are left over from the slot's previous
occupant, Q17, Q21.) Destroying one of these objects counts as a placed object (3.5: +5 for a medium or large
explosion).

**State $21 pieces [V]:** $0393 + 1 each step; at 128 or more (bit 7 set) the piece is freed. When the new count is
exactly 8 the frame becomes $AF (and, by a slip of the program, the homing is skipped on that one step, Q20). While the
count is below 32 the piece homes on the pod (accel $10, M = 4, x first, each followed by its move; at 8 and from
32 on it only moves). The pieces made by a split start with the counter of the slot's previous occupant (Q21).
**[V]** 5,350 piece-steps.

**How these objects are drawn [V] 2026-10-20** (OAM of `level26.replay` against the layouts): with tile base 0 for every frame but $33 (`graphics.md` 5.7.5). Frames $AF-$B3 (the rocks and
their pieces, palette 3 = blue) use the terrain tiles $2D-$5C copied into table 0 (layouts at bank 1:$C399 ($AF), $C3AA, $C3C3, $C3E4, $C40D ($B3): 4, 6, 8, 10 and 19 sprites, all attribute 3); the cores'
frames $B4, $B5, $B6 are layouts of 4, 5 and 4 sprites of tiles $0A-$16 (attributes 3 and 0; with the core's flags X = 1 every sprite is mirrored); the effect's pieces use frame $33 (tile $17 through the one tile
range, mirrored by their flags X = 1, so attribute $40) until their counter reaches 8 and then frame $AF. The sprite palettes
are those of `graphics.md` 7.5.

**The same state is made by the final level's background effect** (a routine in bank 0, $AD99-$AE81; RAM $E9, $EB-$ED are its variables). **It draws nothing of its own [V]:** it computes
the nametable addresses of a 2 × 2 tile cell at the point and keeps a table of tile numbers (bank 0:$AE82-$AE95), but neither is read by any
code, so the "tile animation" the routine looks built for never happens (nametable and CHR RAM identical, apart from the split marker, over steps 4,020-4,139 of `level26.replay`,
with five activations in that window). What it does is the piece spawn below and the palette flash: once the camera has reached the end of the route
(camera x about 6,660), one piece every 24 to 64 steps (first at step 4,053 of the run) in a free slot from 15 up,
at one of 12 points: x = 6,656 + 8 × (18, 20, 22, 22, 24, 24, 26, 26, 28, 28, 28, 30) = 6800-6896, y = 512 + 12 + (64,
80, 16, 112, 64, 144, 192, 208, 32, 176, 208, 96) (ROM tables bank 0 $ABCB and $ABD7), frame $33, hit points 1,
whole x speed 2, flags X = 1 (left), counter $0393 = 0. The shot response of $B7 also writes RAM $EC = 1 and $ED = 0,
variables of the same routine. **[V]** that the pieces appear as described; the routine's own timing and gates **[V]**
2026-10-17 (the route run of 4.16 below, 4,700 steps, hooks on the routine, 22 pieces):

*The effect's timing.* The routine runs once per logic step (in the A pass) from the step the camera stops at the end of
the route (step 4,035 of the run; RAM $9A bit 6 is set by then) and is gated by the **count of live placed objects in slots 23-27**
(states of 6 or more, the five type-5E objects of 4.16): the count is written to RAM $03B1 (the counter field of slot 30) every
step; with the count 0 the effect does not run and the level-end rule below takes over. With at least one core alive:
- **Idle** (RAM $EB bit 7 set; $EB is the active point, $80 = none): a check is made only when RAM $5D AND $0E = 0 (the value the routine sees is odd, $A1, $D1, ...), that is
  on every 8th step. The check takes R = the low nibble of RAM $5A; if R is 12 or more nothing happens (4 of 16);
  otherwise point R is eligible when its screen row (512 + the y of the point - the camera y) is within $30..$CF, and
  then $EB = R and the phase counter $E9 = $FF, and the first active step follows at once, in the same step. (Only y is
  tested, not x.)
- **Active:** every step the routine computes the nametable address of the point's tile (for the tile animation of
  the end structure). The phase counter $E9 rises by 1 on the steps where RAM $5D AND $06 = 0 (every 4th step; the first one is the
  activation step itself, $E9 = $FF to 0). When it becomes 2, that is 8 steps after the activation, **one piece (state $21,
  4.16) is made** in the first free slot from 15 up (taking a free slot with the search of 2.5; none free: no piece) at the point's
  position, as listed above. When it becomes 5, 20 steps after the activation, $EB = $80 again. The checks keep coming on every 8th step from the activation, so
  the next activation is possible 24 steps after the last one, and each later check succeeds with probability about (12/16) times the share of visible points.
- So the pieces come 8 steps after an activation and the gaps between pieces are 24 steps or more, always a multiple of 8. Observed in
  22 pieces (the first at step 4,052, 17 steps after the camera stopped): gaps 24, 48, 32, 64, 24, 24, 40, 24, 24, 32, 24, 24, 24, 24, 24, 24, 48, 32, 32, 24, 24.
- **Hit flash [V] 2026-10-20.** A hit on a type-5E object (frame $B6) sets RAM $EC = 1 and $ED = 0. The routine then reads a short script of
  three-byte entries, one per step ($20 $FF $FF, $30 $00 $FF, $30 $20 $FF, $36 $26 $16, then an end mark; bank 0 $ABE5) into RAM $7F, $80
  and $81 (the palette shadow entries 9-11, the terrain's colours; a $FF in the second byte takes the second table's value, a $FF in the third is stored as is) and afterwards sets those three bytes from three six-entry tables
  indexed by the count of live cores (bank 0 $ABF2: 0F 20 22 37 39 34; $ABF8: 0F 10 12 27 29 24; $ABFE: 0F 0F 02 07 09 04), so the
  terrain's colours depend on how many cores are alive at the last hit (five: pink, four: green, three: orange, two: blue). Run with the flash forced; the full rule and the colours: `graphics.md` 7.5. (Until the first hit the terrain keeps the green of the stopped colour cycle.)
- **Level end, played naturally [V] 2026-10-17.** The route was run as below, the pod held its fuel by poking and fired with A
  (a press every other step) while its height was kept level with the nearest live core. The cores were destroyed one by one (a
  core dies when its hit points reach 0: one hit per shot while its frame is $B6, 10 at the start; the five went in 1,150 steps
  after the route ended, the first 247 steps after it, the last at step 1,154): its state became 5 (exploding), which
  does not count (only states of 6 or more), so the count was 0 at once. The first step with RAM $5D AND $1E = 0 after that came
  13 steps later ($5D = $A0, an even value, so one A pass in 16); two frames after it RAM $DB = 7 and $DA = $FF (the message
  screen's first frame), the exploding core still running; the pod's state is 6 until the end screens. Then message screen
  335 frames, ending 908, credits 2,274, title 13 frames after (all counted from the trigger frame, with the blank frames
  between: `scenes.md` 14). So the trigger is exactly the rule below: **route ended (RAM $9A bit 6), no object of state 6
  or more in slots 23-27, RAM $5D AND $1E = 0.** Anything else that sits in slots 23-27 with such a state (for example a piece of
  the end structure's effect, state $21, or an enemy shot) holds the end back until it is gone.
- **Level end (forced).** With the count 0 (all five cores gone) and RAM $5D AND $1E = 0, the end of the level starts: RAM $DB = 7 (message
  kind 7, the final message) a few steps later (9 in the run) and the message timer $DA = $FF. **[V]** (the five states poked to 0 at step 4,100:
  $DB = 7 at step 4,109; the pod state changed from 6 to 0 at step 4,282, 173 steps later).

**How it was run.** The savestate of level 26 from the usual start, with RAM $9A = $80 and the player state ($0317) = 6
poked at the first step and the camera set to (0, 928), makes the game run the route by itself (2 px per logic
step, the pod carried along): 4,050 steps from the start to the end of the route, 6,000 steps logged, the pod kept
alive by poking its fuel. Models of the rules above reproduced 1,900 + 1,700 + 8,600 + 10,200 object-steps of $B4, $B5,
$B6 and $B7 with no difference other than objects leaving the window and respawning; a second run with the pod
firing (button A held) showed the splits and the one hit on a $B7.

### 4.17 Egg, type 5F (state $B8) **[V]**

Work 6 + 1 each step; frame = $9A + work 6 / 16 for work 6 < 64; at 64 the egg is removed and its list entry cleared
(it never comes back). So frames $9A, $9B, $9C, $9D, 16 steps each, then nothing hatches. Shot: becomes an item
(3.5 case 6).

### 4.18 Satellite, type 60 (state $B9), and companion, type 61 (state $BA) **[V]** (phases 0-5)

- Satellite: home in x (accel 3, M = $C0) and y (accel $10, M = $C2), moving each axis after its homing. When R AND
  $3E = $16 and a slot 15-22 is free: a state-$15 shot at (x ± 16, y) toward the pod's side (left when the satellite
  is at or right of the pod's x), 6.00 px per step horizontally, frame $C0, lifetime $C0, sound $08. **[V]** (level 7,
  about 340 satellite-steps away from the pod). It chases the pod without limit, so with a distant pod it leaves the activity
  window (3.5 of `objects.md`) and is respawned from its list entry.
- Companion (work 6 = phase, work 5 = satellite's slot, hit points 4, flags $20):
  - Phases 0-3, searching (work 5 = 0): every 8 steps ((k + s) mod 8 = 0) look for a satellite (state $B9) in slots
    23-30 in ascending order; found → remember its slot and orbit it this step; not found → phase + 1. The phase
    counts failed searches in all, it is never reset.
  - Phases 0-3, linked (work 5 ≠ 0), **every step**: the satellite's slot empty (state 0) → the companion frees
    itself; state $B9 → orbit; any other state (an explosion, or another object) → phase + 1 and work 5 = 0, which
    returns it to searching.
  - **Orbit** **[V]**: the target is the satellite's position plus an offset on a circle of radius 36 (the original
    asks for radius $48 from a half-amplitude sine table, so the result is $48 × sin/256): with A = F (the frame
    counter RAM $5D, now moving by 2 per step), s = A as a signed byte, offset x = sign(s) · ($48 × S[|s|]) / 256 and
    offset y = sign(b) · ($48 × S[|b|]) / 256 with b = (|s| + $40) as a signed byte, S the 129-byte table at bank 7
    $8C74 (ROM 0x38C74: 0, 3, 6, 9, 12, 15, 18, 21, 24, 28, ... rising to $80 at index 64 and falling back to 0
    at index 128), divisions truncating towards zero before the sign is applied. So the offset is (36 sin, 36 cos) of
    F · 360 / 256 degrees: at F = 0 the companion sits 36 px below the satellite, and one lap takes 128 steps. The
    companion homes on this point on both axes (accel $F0, M = $83, x first, each followed by its move). 1,300
    steps reproduced.
  - Phase 4 (no satellite found four times): chase the pod on both axes (accel $16, M = $02) **[V]**, 390 steps; when
    d < 48 (2.4; d = 255 beyond 255 px) it moves into the first free pod-shot slot of 3-10 as frame $51, work 6 = 5,
    hit points 128, keeping its state; its own slot is freed and its list entry cleared. **[V]** seen (slot 3).
  - **Phase 5** (now a pod-side helper) **[V]** (level 7, poking the companion into phase 4 at step 3 and holding the
    pod 164 px from it: it arrived in slot 3 at step 80; 600 steps, 596 companion-steps and the 38 shots it
    fired reproduced, all in slots 4-6): state $BA, work 6 = 5, **work 5 = 0** (so the orbit centre is slot 0, the pod), frame $51 (it keeps
    it), hit points 128. Every step, in this order:
    1. **Fire.** Look for a free slot among 3, 4, 5 and 6 (its own slot counts as busy; none free: skip to 2). Choose a
       target: slot 23 + (R1 AND 7); if that slot is empty (state 0), slot 23 + (R AND 7); if that is empty too,
       skip to 2. A slot in any non-zero state is a target, explosions and the object's own children included.
       Otherwise make a shot in the free slot: state $15, frame $62, hit points 1, lifetime $C0, at the companion's
       position (the fractions are those of the slot's previous occupant, Q17); flags X = 1 if the companion's x is at
       or right of the target's, flags Y = 1 (down) if its y is above the target's; |dx| and |dy| to the target (px,
       as 16-bit numbers) are doubled while neither high byte reaches 4 (i.e. neither is 1,024 or more), at most 5
       times, and are the speeds in 1/256 px per step (so the longer axis ends at 4-8 px per step for targets 32 px
       or more away, e.g. 6.25 at 100 px; nearer targets get |d| / 8); sound $08. The shot is in a slot 3-6, so it
       also takes part in 3.4 (shots against placed objects); its flags and hit points are those of 2.6 (it explodes
       on the first thing it hits, 3.4).
    2. **Orbit.** As the satellite's orbit (above) with the pod as centre: the target is the pod's position plus
       (36 sin, 36 cos) of F · 360 / 256 degrees, homed on with accel $F0, M = $83, x first, each followed by its move.
    The helper never leaves the pod's service while it lives (hit points 128). Its own slot and the three slots its
    shots use are pod-shot slots (3-10), shared with the pod's bullets (`player.md`).

## 5. Objects made by other objects (states below $80)

| State | Made by | Section |
|---|---|---|
| $03 | lifted dome | 5.1 |
| $04, $05 | destruction | 3.5 |
| $06 | shots on bonus objects, tractor-beam event | 6.2 |
| $0F | domes, wide machines (slots 15-22); the pod's homing missiles (slots 3-10) | 2.9, 5.11 |
| $10, $12, $13, $14 | the pod's time bombs, star parents, star children, multi warheads (slots 3-10) | 5.11 |
| $15 | most shooters | 2.6 |
| $16 | state $96 (unused) | 5.8 |
| $17, $18 | domes, gun pairs | 5.2 |
| $19 | turrets 19-1B, domes, generators 4B/4C | 5.5 |
| $1A | ground guns | 4.2 |
| $1B | heads | 5.3 |
| $1C, $1D | walkers, pillars | 5.3 |
| $1E | generators 48/49 | 5.6 |
| $1F | generators 4D/4E | 5.7 |
| $21 | level 26 splitting | 5.8 |
| $22 | type 21 | 4.10 |
| $23 | the green-orb spawner | 5.4 |
| $24, $25 | shots on objects with flag bit 3 | 5.10 |

### 5.1 Exhaust (state 3) **[V]**

While (F mod 256) ≥ 128 it sits at its dome's position + (0, 22) with a random frame from ($44, $45, $46, $45)[R mod
4]; otherwise, or when the dome's slot is empty, it is freed. **[V]** (63 steps). When it was made in slot 22 for
want of a free slot, it overwrites whatever shot was there and keeps that shot's counter and speed bytes (Q9, 2.5).

### 5.2 Flames (states $17, $18) **[V]** (state $17), **[C]** (state $18)

Field $0393 + 1 each step; with i = $0393 after the increment (1 on the first step, which is the step of creation when
the creator's slot is lower): frame = ($64, $65, $66, $67, $66, $65, $65, $66, $67, $66, $65, $64)[i] with the first entry
unused (i = 1 gives $65), so i = 1-11 show $65 $66 $67 $66 $65 $65 $66 $67 $66 $65 $64 (state $18: 4 less, $60-$63);
flags X = 0 for i = 1-5 and 11, 1 for i = 6-10; at i = 12 the slot is freed. (An earlier reading of the table was one
entry off.) They do not move. Harmful to the pod, not destroyed by touching it.

### 5.3 Heads on walkers and pillars (states $1C, $1D) **[V]** (walker heads), **[C]** (pillar heads), pillar head position **[V]**

Each step the head is placed at its owner's x and y − h: on a pillar h = owner frame − $57; on a walker h = (25, 20,
30) for owner frames $5D, $5E, $5F. If the owner's slot is empty the head is freed.

- **State $1C:** animation index p = ($03B2 mod 16) / 4. If p ≠ 0: $03B2 + 1; frame ($55, $56, $57, $56)[p], flags X
  (0, 0, 0, 1)[p]. If p = 0: frame $55 and flags X 0; and if F mod 16 = 8, $03B2 + 1; if F mod 16 = 0, fire (if no slot
  is free, $03B2 + 1 instead). It fires every 8 steps between animations (4 shots, then 12 steps of animation).
- **State $1D:** q = ($03B2 mod 32) / 4 (0-7); frames ($58, $59, $57, $59, $58, $5B, $5A, $5B)[q], flags X (0, 0, 0,
  1, 1, 1, 0, 0)[q]. If q mod 4 ≠ 0: $03B2 + 1. If q is 0 or 4: if F mod 16 = 8, $03B2 + 1; if F mod 16 = 0 it fires
  only when facing the pod: q = 0 when the pod is left, q = 4 when right.
- **Shot head [C]:** a head has hit points 16 (state $1C) or 12 ($1D) and flags 0. When a shot takes its last hit
  point the destruction of 3.5 runs in the collision phase of the step: its frame, flags, position and owner link (work 6)
  are left as they were, so the head is drawn unchanged in that step; field $03B2 (its animation index) is overwritten
  with the explosion's sequence position and field $0393 with 1. The explosion's first frame shows from the next step,
  and the owner, finding its head in a state below 6, is destroyed. A head has no list index: only objects in a placed
  object's state (≥ $80) clear an entry of the type-list copy when destroyed.
- **Fire** (slot 15-22): state $1B, frame $5C, at (x ± 24, y) toward the pod's side (a pod at exactly the head's x
  counts as left), horizontal speed 8.00, lifetime $40, sound $08. State $1B: move x only; lifetime − 1; freed at
  0. **[V]** for position and speed; **[V]** walker heads of both kinds, level 5, about 2,400 head-steps and 315
  shots reproduced (the shots usually leave the window first: 8 px per step).

### 5.4 Green orb (state $23) and its spawner **[V]**

**Spawner** (after the A-pass split, planets only, levels 0-12, and not while the player state is 9): once every
128 steps (when RAM $5D, now odd, is 1): if the tractor beam holds something and RAM $6E mod 128 ≥ 3, or else if
RAM $5E mod 4 = 3 and R bit 7 is clear, an orb is created in the first free slot 23-25 at a random edge position
(2.10), frame $B9, state $23. **[V]** (level 8, 4,200 steps: the 33 checks at $5D = 1 agreed 32 times, with 3 orbs
created; the one disagreement, step 1,856, had every condition met but no orb appeared: **explained and [V]**
2026-10-17. The orb was created (the state byte of slot 25 was written at that step) at x = 2,688 and y = 1,282, and
freed in the same step by the activity window (`objects.md` 3.5). The edge position (2.10) was a side edge: x =
camera x + 288 = 2,688 (R2 bit 1 set, bit 2 clear: the right edge). Its y is the pod's y plus an offset of +38, added
to the low byte only: the pod's y is 1,500 = $05DC, the low byte $DC + 38 wrapped to $02, and the high byte stayed
$05, so y = $0502 = 1,282 instead of $0602 = 1,538. That is 118 px above the top of the screen (camera y 1,400) and
well outside the window. It is Q10 at work: whenever the pod's low byte plus the offset passes $FF or goes below $00, the new object is
placed 256 px away from where it should be (offsets are -64..+64 on a side edge and -32..+32 on the top or bottom edge,
so this happens in about one spawn in eight on a side edge and one in sixteen on the top or bottom, **[C]** arithmetic).
The beam-held case was run too (below).

**Orb:** frame ($B9, $BA, $BB, $B9)[F / 8 mod 4]. Two cases by the counter $0393 (field "a"):
- **$0393 < 16** (it starts at 0): the orb homes on the pod (accel 2, M = $80, x first, each followed by its move), or
  (accel $10, M = $82) while the beam holds something and RAM $6E mod 128 ≥ 3. Then, **only in this case**, the
  counter is raised by 1 when (s + F) mod 256 is 0 or 1 (once every 128 steps, so 16 takes 2,048 steps). Then, when
  $0393 is odd and (s + F) AND $3E = 0 (F = RAM $5D, so (s + F) mod 64 is 0 or 1; in the A pass F is even, so the
  slot's parity picks 0 or 1: one step in 32): a shot from slot 15-22 (overwriting slot 22 when none is free, **[C]**)
  with the orb's position, speeds and directions, frame $33, lifetime $FF, sound $08.
- **$0393 ≥ 16**: it speeds up horizontally by 2/256 per step (in the direction of its flags X, cap 3.00) and moves
  (both axes, its own vertical speed unchanged); no counting, no shots, no homing: the orb drifts off.

**Beam-held case [V]** (2026-10-17, level 8, pod held at (2500, 1500) with a kind-3 item caught on the beam and
RAM $6E poked to 3 each frame, 1,700 steps): orbs were made at steps 63 and 191 and then not again, because
slots 23-25 were full (a type 21 creature in 23, the two orbs in 24 and 25). With the item still held but $6E = 2 or
0 the first orb came at step 319 (the random gate of the unheld rule, as in the plain case); with $6E = $83 (low 7 bits 3,
bit 7 set) the orbs came at 63 and 191, like $6E = 3. So the held rule creates an orb at every check without the random
gate, as written. The model of the orb with accel $10 and cap M = $82 reproduced 1,011 orb-steps with no difference
(the two creations apart); the plain parameters (accel 2, M = $80) reproduced none of them. The shots (position, speeds, lifetime $FF) agreed.

Shooting it releases a state-$25 bonus (3.5). **[V]** 2026-10-17 (level 8, orbs of 380 + 370 + 200 steps in three
runs, 481 + 137 shot-steps, and one run with the counter poked to 16 for the second case): both cases and the
shots' fields as written (a shot's lifetime $FF distinguishes them from the other shots of the level).

### 5.5 Drone (state $19) **[V]**

Created with direction down (except where the creator sets it), vertical speed 2.00, frame $4D, field $0393 = 16;
hit points 1. Field $03B2 = mode b, work 5 = fire timer. Each step:

1. $0393 − 1. At 0: $0393 = 18 and re-plan: b = 0 if the drone is above the pod, else 1. If b = 1 and |dy| ≥ 16 →
   climb. (With |dx| ≥ 256 the distance routine reports "not above" and leaves |dy| unset, 2.4, so the drone is
   in mode b = 1 and tests a stale byte; it climbs whenever that byte is 16 or more (it is $20 in the traced cases, 2.4 [V]), which is what is seen: a drone more
   than 256 px to the side keeps climbing. Likewise with |dy| ≥ 256 and the drone below the pod: the test sees the low
   byte. Q24.) Else flags X bit 0 = 0 if the pod is right, 1 if left; then if |dx| ≥ |dy| → fly with b = 0; else if |dx| <
   |dy| / 2 → climb; else fly with b = 1. (Not re-planning: fly if the frame is below $4D, else climb.)
2. **Fly:** if flags X bits 0 and 7 differ (turning): slow down by $0A (b = 1) or $0F (b = 0); when the speed reaches
   0, flags X bit 7 flips, and then, if flags X is now non-zero, it speeds up as below in the same step; if flags X is
   0, or the speed is not yet 0, it bobs. Not turning: b = 1: speed up by 2; b = 0: speed up by 4 and bob. Move:
   horizontally in the direction of flags X **bit 7** (not bit 0), vertically as usual. Frame $47 + (0, 1, 2, 2)[k mod
   4], plus 3 when b = 1.
3. **Climb:** on entering, frame $4D and $0393 = 16. b = 1: going down, slow down by 8 and set direction up at 0; going
   up, speed up by $0C; frame $4E + (k mod 2). b = 0: bob; frame $4D. Move as in fly.
4. **Bob** (a vertical wobble): going up, slow down by 6, direction down at 0; going down, speed up by 4. Each bob
   also does step 5.
5. Work 5 − 1 (once per step, plus once per bob); when work 5 mod 64 = 0 after a decrement: a shot (slot 15-22) at (x
   − 10, y − 2) if flags X ≠ 0 else (x + 10, y − 2), with the drone's flags X, its vertical direction and speed,
   horizontal 4.00, frame $50, lifetime $80, sound $08.

### 5.6 Mine (state $1E) **[V]**

Homes on the pod on both axes with accel 8 and a maximum that cycles: with c = $0393 mod 32, max = 1 + (c mod 8) / 8
+ (1 if c ≥ 16) px per step (1.000 to 1.875, then 2.000 to 2.875). $0393 + 1 when F mod 8 = 0. Frame = $9A + (4 −
hit points) (its cracks show damage). Touching it costs the pod energy and the mine 1 hit point; shots knock it
back.

### 5.7 Hopper (state $1F) **[V]**

Field $0393 = step counter, $03B2 = mode b, work 6 = base frame, work 5 = walking flag.

1. When $0393 mod 8 = 0 (every 8 steps, including the first): with |dy| to the pod (2.4: with |dx| ≥ 256 this is a stale byte, $20 in the traced cases
   [V], and the pod counts as "not below" the hopper, so it behaves as if the pod were above it, Q24): if |dy| < 24, b = 1 and it fires
   if it faces the pod (flags X = 0 and pod right, or flags X ≠ 0 and pod left): slot 15-22, at (x ± 8, y), its
   vertical direction and vertical speed, horizontal whole speed 4 (the fraction byte of that slot is not reset),
   frame $33, lifetime $C0, sound $08. Else b = 5 if it is above the pod, else b = 2 + ((|dy| − 24) / 8 mod 4), at most
   4.
2. $0393 + 1.
3. b = 5 (pod below): work 5 = 0; horizontal speed slows down by $0E; work 6 + 1 every other step (F mod 4 = 0) until
   $A6; then fall.
   b = 1 (level with the pod): if work 6 ≠ $A3: work 6 − 1 every other step; else walk toward the pod: moving toward →
   speed up by $0C (work 5 = 1); moving away → slow down by 4 (work 5 = 0), and at 0 turn and speed up by $0C (work 5 =
   1). Then fall.
   b = 2-4 (pod above): work 6 = $A2 + b. Moving away from the pod horizontally: work 5 = 0, slow down and turn by $0A;
   **the vertical speed is not touched in that case** (it keeps whatever it had, and there is no fall).
   Toward: work 5 = 1, speed up by (4 − b) × 8; and only now vertically: going down, slow down and turn by (b − 2) × 8;
   going up, speed up by that. No fall.
   **Fall:** going up, slow down by $1E and at 0 turn down and speed up by $1A; going down, speed up by $1A.
4. Move. Frame = work 6, plus (4, 8, 4, 0)[$0393 mod 4] while work 5 = 1.

Shooting it releases a bonus star (flag bit 3) and knocks it back.

### 5.8 Other low states **[V]** ($21), **[C]** ($16)

- $16 (made only by state $96, which nothing creates): counts $0393 up to 256 and is freed; homes on the pod (accel
  $18, M = 4) while $0393 < 64, else moves straight.
- $21 (level 26 pieces and sparks): $0393 + 1, freed when it reaches 128 (bit 7 set); frame $AF when it reaches 8,
  and on that step the homing is skipped (Q20); homes on the pod (accel $10, M = 4) while $0393 < 32. **[V]** (4.16)

### 5.9 State $96 and the bank 4 waypoint routine **[C]**

State $96 (hit points 0) has a behaviour, but no type has it as initial state and no code creates it. It would wait
until the pod is within d < 72, then chase it (M = $81) while firing state-$16 shots every 64 steps, and patrol before
that using the routine at bank 4 $8057, which reads the level's placement-record list as 8-byte pairs of points
indexed by the list index (the "waypoints" asked about in `objects.md`). Because the routine's result is lost on the
way back to bank 7, the patrol would always steer toward a fixed point read from the start of the record list. Not
needed by the remake.

### 5.10 Bonus objects (states $24, $25, slots 11-14) **[V]**

Released by hits on objects with flag bit 3 (3.5), with field $03B2 = 0, work 6 = 0 and flags Y = 0. Field $03B2
counts up by 1 per step and the slot is freed when it wraps (256 steps). State $24 (a
spinning star): flags Y = 0, frame $55 + ((count / 2) mod 2); count / 2 mod 4 = 2 → flags Y = $80; = 3 → flags X = 1,
else flags X = 0. State $25 (a box, frame $64): no animation. They do not move. Shooting them gives the bonus of 3.3;
whether the tractor beam can take them is `player.md`. **[V]** 2026-10-17 (level 1, a hopper held in front of the
firing pod, 150 steps): the star appears in slot 11 at the hopper's position with $0393 = $82, its counter and frames
as written (frames $55 $55 $56 $56 $55 $55 $56 $56, flags Y $80 at counts 4-5, flags X 1 at counts 6-7, repeating
every 8); four stars were released by the four hits that destroyed the hopper (hit points 4; the last hit
releases one too, as 3.5 says), in slots 11, 11 (after the first was shot), 12 and 13; each star shot: +50 (3.3);
destroying the hopper: +5. The box ($25) was seen released by orbs (frame $64, slot
11-14) and, **[V]** 2026-10-17, followed as a poked object: a box in slot 13 with field $0393 = $82 and counter 0 stands still
for 256 steps (placed at step 3, gone at 258), is collected by a shot or by a touch (3.3, `economy.md` 2.5), is not
caught by the tractor beam, and gives +25 and $C9 + $1F (3.3). Its field $03B2 counts up by 1 per step as for the star.
The tag rule is not a rule of the pickups: pickups stayed with $D2 at $00, $01, $02, $40 and $41.

*Caution for models.* The beam catches any object in slots 11-14 whose state is $0A or more, and takes the kind from the
list entry named by the field $0393. A poked box with $0393 = 0 (list entry 0) was caught and then shot to leave a stale
hold; with the true field $82 it is not caught. Do not poke bonus objects with a list index.

### 5.11 The pod's special weapons (slots 3-10) **[V]**

The shot objects of the weapons in `player.md` 10.1 (creation, costs, slot rule). All of them are "shots" for the
collision rules (3.2-3.5: any state of $07 or more in slots 3-10, so they also collect bonus objects, 3.3), run in
the object loop of the A pass after the pod's step (so a shot made in a step moves once in that same step if its slot
is above the maker's, and always acts for the first time in the step it is made), and are freed by the generic
off-window rule (`objects.md` 3.5). Fields used: $0393 (a missile's target slot), $03B2 (a missile's direction; a
star child's stage), work 5 ($0403, a counter), work 6 ($0422, only the missile's dead branch below).
$15 below is RAM $15 = $5D / 2 at the start of the pass; $5D is the frame counter as the A pass sees it (even).
None of these weapons changes fuel, none uses the random number except as stated, and the score effect is only the
one of 3.5 (a placed object destroyed gives +5, or +3 for a small one; other slots give nothing).

#### 5.11.1 Homing missile (state $0F) and multi warhead (state $14)

The same code as the enemy missile of 2.9 for direction, frame and speeds, with a different start, a target chosen
at random, and a different life. Made with: hit points 2 (homing) or $FF (multi), work 5 = $C0, field $0393 = $20
(see "target"), direction $03B2 = 0 (up), flags from the plain bullet's rule, speed vector = the plain bullet's
speed (`player.md` 10) plus the pod's velocity, first frame $27. The missile does **not** start with 32 straight
steps. Each step, in this order:

1. **Target.** $0393 holds the target slot, 0 meaning none.
   - target T ≠ 0 with state ≥ 6: keep it;
   - T = 0: look for a target only when (2·slot + $5D) AND $0E = 0 (once in 8 steps, at a phase set by the slot);
   - T ≠ 0 with state below 6 (destroyed, exploding, freed): look at once, in the same step;
   - at creation $0393 = $20, which is no slot; the state read for "slot 32" is a byte of the flags array, always
     below 6, so the first step always looks.
   **Looking:** R += $57 + c (8-bit; c = bit 8 of 2·slot + $5D when T was 0, else 0); candidate = 23 + (R AND 7).
   It is taken when its frame is not $FF and its state is 6 or more; otherwise T = 0 (and nothing else is tried).
   So at most one candidate per look, any placed object in the window (visible or not, harmless or not). **[V]** all 684
   looks of the three demos (the new R and the candidate, and c in 34 cases with c = 1).
   (Only if work 6 is non-zero at this point does the missile skip the rest: it counts work 6 down, speeds up both
   axes by $20 and moves straight; the gun does not clear work 6, but nothing in play leaves it non-zero. **[C]**)
2. **Direction.** When (slot + $15) AND 7 = 0 the direction is recomputed from the missile to T's position (2.9;
   flags X gets bit 1). T = 0 is slot 0, **the pod** (Q27). Otherwise the direction stays. Because it starts as 0 (up), a
   fresh missile flies up for 0-7 steps whatever its aim. **[V]**
3. **Frame** = $27 + direction when the sum of the two whole speeds is 3 or more, else $2F + direction.
4. **Speeds** as 2.9 step 4 (horizontal acceleration 0, $14, $20, $14, 0, $14, $20, $14 by direction, vertical
   $20, $14, 0, $14, $20, $14, 0, $14; zero acceleration slows by $12; against the direction it slows and turns at 0;
   with it, speeds up; cap 3.00). A speed above the cap is snapped to 3.00 by the first speed-up on that axis (the
   creation speed of a missile is above it); an axis with zero acceleration just slows by $12 from the creation
   value. Then the move (2.1).
5. **Life.** Work 5 counts down by 1 when $5D AND 2 = 0 (homing: every 2nd step) or when $5D AND $0E = 0
   (multi: every 8th step); at 0 the shot is destroyed by the rule of 3.5 (slots below 15: large explosion,
   sequence 11, sound $0C, no score). That is at most 384 steps (homing) or 1,536 steps (multi) if nothing else
   ends it.
6. **Collisions** (3.4, 3.5): a homing missile does 2 damage and becomes a small explosion (state 4) at once. A
   multi warhead does $FF damage (it destroys anything but a deflecting object, whatever its hit points, a walker's
   255 included), is **not** removed, and flies on. A deflection bounces it (3.5) and it keeps its state; a
   homing missile that deflects becomes a spark (state 1).

**[V]** Mesen, attract demos 1 and 3 (16 homing and 8 multi-warhead flights, 9,638 logic steps): the model above,
with the targets' recorded positions (a target is seen at its position before the step), reproduced every field
(position and fractions, both speeds, both direction bits, direction, frame, work 5) in 9,615 steps. The other 23
are multi-warhead steps that bounced (the flipped direction bit and one changed fraction byte). 593 of the 1,207
direction recomputations had T = 0 and aimed at the pod. Harness runs on level 0: a homing missile took a dome from 48
to 46 and vanished in a small explosion; a multi warhead destroyed it in one hit (+5) and flew on; both
bounced off a closed turret as described in 3.5.

#### 5.11.2 Star bullets (parent state $12, children state $13)

*Parent.* Made with hit points $50, work 5 = $18 (children left to make), first frame $27, speed = the plain
bullet's speed divided by 16 (so about 0.3 to 0.6 px per step) plus the pod's velocity, and **no steering**.
Each step: the frame is $27 + (($5D / 8) AND 1); the shot moves; then it tries to make one child:

- the child goes into the first free slot among 3-6 (the parent was itself made in one of them, so at most 3 are
  available); if none is free, nothing happens;
- target: slot 23 + (R1 AND 7) if its state is not 0, else slot 23 + (R AND 7) if its state is not 0, else no child.
  Only the state is tested: an exploding object or one with frame $FF can be chosen. R, R1 are not changed;
- the child: state $13, frame $27, hit points $10, position = the parent's position (whole pixels), work $03B2 = 0.
  Its speed is the vector from the parent to the target, **|dx| and |dy| in pixels, each doubled** while both
  high bytes are below 4 (at most 5 times), read as 8.8 px per step; its direction bits point from the parent
  to the target. So a child flies straight at its target at a speed of about 1/8 to 1/32 of the distance per step
  and up to 7.99 px per step, and passes through it if it misses. Its position fractions are the previous occupant's
  (at most 1 px off, Q17); all four speed bytes and both direction bits are written. **[V]** 68 of 73 children of the demos (the 5 others were first seen away from
  their parent) and the harness;
- after a child is made, work 5 − 1; at 0 the parent is freed. So a parent makes at most 24 children, and only
  while targets exist.

The parent does 80 damage to every object it touches (a dome of 48 died in the step it was made), is never
removed by hitting, and bounces off what it cannot destroy (3.5, state $12 keeps its state).

*Child.* Each step: move; then its stage counter $03B2 goes up by 1 when ($5D + slot + 1) AND 15 = 0 and the frame
becomes the next of $27, $28, $29, $2F, $30, $31 (stage 0-5); the 6th increment frees it. **A slot of even number can
never satisfy the test, so a child in slot 4 or 6 never advances and never expires** (Q28); a child in slot 3 or 5
lives 41-48 steps. **[V]** all 147 stage changes of the demos were in slots 3 and 5 at the predicted frame counter;
children in slots 4 and 6 kept frame $27 for as long as they were seen (up to 194 steps). A child does 16 damage
and becomes a small explosion on a hit (like a bullet; the kills are +5 as usual). Three children took a dome from 48
to 0.

#### 5.11.3 Time bomb (state $10)

Made with hit points $40, work 5 = $C0 (not used), first frame $3D, speed = the pod's velocity only (it does not
move: the handler never calls the move). Each step: when (slot + $15) AND 31 = 0, the frame goes up by 1; when it
would reach $42 it goes back to $41 and the bomb goes off: the bomb itself is destroyed (3.5: slot below 15, large
explosion) and then every object in slots 23-30 that is not free and whose position lies within −64 ≤ dx ≤ +63 and
−64 ≤ dy ≤ +63 of the bomb (dx, dy = object − bomb, 16-bit, in pixels) is destroyed by the same rule, whatever its
hit points, flags or frame (an object of frame $FF is spared). It takes four frame increments to $41 and a fifth to go
off: 4 × 32 + p steps with p = 1 to 32, the steps to the first increment (so 129 to 160 steps, 4.3 to 5.3 s).
**[V]** harness: created f11, frames $3D → $3E at f57, $3F f121, $40 f185, $41 f249, off f313 (an increment every 32
steps; the first at $15 + slot ≡ 0 mod 32); of seven placed objects the three at (+40, −5), (+63, +58) and (0, +59)
from the bomb died, those at −70, −65, +64 horizontally and at −70 vertically survived. Score +15 for three kills.
The bomb is also a shot for the collision rules: any placed object it touches before that takes $40 damage and
turns the bomb into a small explosion (it does not go off), and a dome of 48 touched at creation was destroyed that
way. **[V]**

#### 5.11.4 Smart bomb (D1 = 4)

No object is made. The press destroys, in slot order 15 to 30, every object whose state is not 0 and whose frame is
not $FF, by the rule of 3.5, wherever it is. The score is +5 or +3 for each in 23-30. Enemy shots, children and
explosions in progress are included. Objects already exploding in 23-30 count as live and are destroyed again (restarted as small explosions, +3) **[C]**.
**[V]** harness: four objects in slots 23-30, an enemy shot (slot 15) and a mine (slot 17): all gone or
exploding in the step, +20 (four kills in 23-30); gauge −$80. Only the last object destroyed in 23-30 is left
exploding: each destruction frees the explosions of the earlier ones (Q7). It is the same for the time bomb (three
kills, one explosion left).
**On the final level** it destroys all five cores (4.16) like any other object, whatever their frame or hit points **[V]**
2026-10-17; the press can then freeze the pod: Q32.

#### 5.11.5 Anti-gravity

A switch, not a shot (`player.md` 10.1, 4.7).

## 6. Items, mothership pieces and pod-side objects (slots 1-14)

### 6.1 Specials from the level's bank 2 list **[V]** (frames, states, catch, delivery, the fourth-slot event), **[C]** (the rest)

The items list of each level, the second object list in the header (`economy.md` 2.1 "item kinds"; `levels.md` 3.3; RAM copy $052D-$057B positions, $0513 types, count RAM $BA) is
scanned by bank 2 $85AD (four entries per call, from the A pass and at level start): an entry with type ≠ 0 within
−48 ... 303 px of the camera on both axes, not already in a slot 11-14 (field $0393 = its index), is placed in the
first free slot 11-14 with field $03B2 = its item kind and:

| Special type | Frame (table B) | State |
|---|---|---|
| $01 | $54 | $0A |
| $02 | $55 | $82 (no valid behaviour; not placed by any list) |
| $03, $04, $05, $06, $07, $08 | $59, $5A, $58, $57, $5A, $5B | $0A |
| $09, $0A, $0B, $0C, $0D | $5E, $5C (FUEL sign), $60, $61, $62 | $0A |
| $0E, $0F, $10 | $64, $65, $66 | $0A |
| $11, $1A | $67 | $0E (docking ring) |
| $12, $13, $14 | $52, $53, $52 | $0A |
| $15, $16, $17, $18, $19 | $FF, $8A, $8B, $8C, $8D | $07 (mothership parts) |
| $1B | $7E | $0A |

**[V]** 2026-10-17 (all the logs of the runs below, about 50,000 item-steps): the frame and state of kinds $01 ($54), $09
($5E), $0A ($5C), $0B ($60), $0C ($61), $0F ($65), $10 ($66), $11 (state $0E, $67 and $68 alternating) and $12 ($52)
agree with the table. **[V]** 2026-10-17 (fourth round: every other kind, written into a list in Mesen, one run each): the frames
and states of $03 ($59), $04 ($5A), $05 ($58), $06 ($57), $07 ($5A), $08 ($5B), $0D ($62), $0E ($64), $13 ($53), $14
($52), $15-$19 ($FF, $8A, $8B, $8C, $8D with state $07), $1A (state $0E, frames $67/$68 switching every 4 steps) and
$1B ($7E/$7F, state $0A) agree with the table. Of these, $03-$08, $0D, $0E and $13 were caught by the beam with the load class
($67) 1, 1, 1, 2, 2, 0, 0, 0, 2 in that order, delivered at the mothership (`economy.md` 2.1); the others are touched or inert. The rising
$1B was seen to rise by 4/256 per step, with the frame order $7E, $7F, $7E, $7E, $7F, $7F, ... (so the "every 2 steps" below is
exact from the second step), and to be freed with its entry cleared on the 56th step after its placement.

What each item is worth or does when delivered is `economy.md`. Behaviours:

- **State $07** (mothership parts) and **$0C**: nothing (they stand still). **[C]**
- **State $0A** (item): when held by the tractor beam (RAM $66 = this slot): if its work 6 is 0 and slot 2 is not in
  state 9 the beam lets go ($66 = 0); its position is written back to the list arrays; flags X = 0. Not held: special
  type $1B rises (direction up, speeds up by 4/256 per step, frame $7E/$7F alternating every 2 steps) and is removed
  together with its list entry when its vertical fraction reaches $E0; other items stay where they are (position
  written back each step). If this is slot 14, or slot 13 while the beam holds slot 14, and slots 11 and 12 are both
  occupied, bank 2 entry $8027 runs: **the pull-back [V]** 2026-10-17, below.
- **The pull-back (bank 2 entry $8027) [V].** The condition is read in the state-$0A handler after the held test and the
  rising $1B test: the item is not held by the beam, **its slot is 14, or it is 13 while the beam holds slot 14**, and
  the states of slots 11 and 12 are both non-zero (any object, the mothership indicator counts). Then, every step, the
  item moves 1 px toward the position of its own entry in the **ROM** items list (the list the level header points
  to, `levels.md` 3.3, entry index = field $0393) on each axis independently, 16-bit, until it is there on that axis,
  and the RAM copy of the list entry (x low $052D, x high $0561, y low $0547, y high $057B) is **overwritten with
  the ROM position** every step. So when four things are in the window at once (the indicator and three items), the
  fourth item, if the beam is not holding it, creeps back to where the level put it, and a later respawn of
  any item that was dropped elsewhere starts from the original place. Seen on level 0: three items poked to
  (700, 250), (740, 262) and (780, 274) in slots 12-14 (slot 11 = the indicator): the slot-14 item began to move one step after
  it appeared, 1 px left and 1 px down per step toward its ROM position (580, 840), and was at (704, 350) after 80 steps; the
  others stood still. With the pod holding slot 14 instead, the slot-13 item crept toward its ROM position in the
  same way (from (900, 262), 1 px per axis per step). The effect is the same for any kind in state $0A except the
  rising $1B. Intent: unknown (Q23).
- **State $0E** (docking ring, frame $67/$68 alternating every 4 steps): if it is being held, as $0A. If the beam holds
  an object whose item kind is not $01, $0B or $11 and that object is within 16 px on both axes of the ring:
  transition request 1 (the object is delivered, `game-flow.md` 7.4). If the beam holds nothing, the player is
  Jetman (player state 2 or 3), the level is a planet (< 13) and he is within 16 px: RAM $41 = 0 and transition request
  2 (dock).

Shooting a special of type $0B: 3.3.

### 6.2 Marker (state 6) **[V]**

Made in slots 3-10 by 3.3 and by the tractor-beam counting event (`player.md`; and by the touch of a star or box; the shot-hit creator leaves stale fractions and horizontal speed, Q31): frame $FF, moving up at 1.00 px per
step (vertical speed 1, direction up), field $03B2 counting to 256 (then freed), field $0393 = its bonus number (low
nibble of RAM $D2). While its frame is $FF: if RAM $D2 AND $60 = 0 the frame becomes $A0 (table B), else it stays
invisible. It is freed early when RAM $D2 bit 6 is set while it is visible, or when RAM $D2 mod 16 differs from its
$0393. It is the floating "bonus" sign; RAM $D2 belongs to `player.md` / `economy.md`. **[V]** (level 1, 3 markers):
made in slot 10, then 9 when 10 is taken, at the star's position; rises 1 px per step from its first step; frame $FF for
its first two steps (RAM $D2 is $61 on the step of the hit, $01 after it: bit 5 is added by the bonus-message
routine), then $A0.

**The sign's picture (2026-10-20).** Frame $A0 (table B) is two sprites, tiles $BC at dx -4 and $D0 at dx +4, attribute 1
(sprite palette 1), and **those two tiles of sprite pattern table 0 are blank until the program writes the points
into them**. The writing is a **pop-up picture job** run in the status-panel split (`graphics.md` 3.1), triggered by
RAM $D2 bit 6, and the picture is chosen by the low nibble of RAM $D2, i.e. by the same number that the marker keeps in
$0393:

| Nibble = $0393 | Picture | Made by |
|---|---|---|
| 0 | 25 | box (shot or touched); first swing of a spun crystal (+25) |
| 1 | 50 | star (shot; the touched star's marker is freed at once, Q22); second swing (+50) |
| 2-7 | 100, 200, 400, 1000, 2000, 5000 | swings 3-8 (+100 ... +5000), nibble = swing number - 1 |
| 8 | 1UP | the ninth swing (lives + 1, jetpods + 1, `economy.md` 2.6): it makes a marker too |

Every creator sets, in the same step as the object, RAM $D2 = ($D2 AND $80) OR $40 OR nibble (the beam's counting event:
nibble = swing count before the increment, 0-8; the shot-hit and touch code: 0 for the box, 1 for the star). When no marker
slot (3-10) is free, nothing is made and $D2 is left alone, so no picture is written (the score and effects still
happen). The job then takes **two consecutive frames**: the first frame (bit 5 clear) writes the second half of the pair
into tile $D0 and sets bit 5, the next frame (bit 5 set) writes the first half into tile $BC and clears bits 4-6 of $D2
(nibble kept). The marker keeps frame $FF (invisible) while bit 5 or 6 is set (the two steps seen) and shows $A0 after.
**[V]** 2026-10-20 (Mesen, `items.replay` and a spin replay with the beam angle and swing count poked: the writes to
PPU $0D00-$0D0F then $0BC0-$0BCF in consecutive frames, 16 bytes each, equal to ROM 0x08100 + 32 n + 16 and + 0 for the
nibble n, for n = 1, 2, 4, 5, 6, 7, 8 and for n = 0 and 1 in `items.replay` at steps 244, 256, 280, 290; $D2 $41, $61
(job, bit 5 added), $01.) The tiles are **not cleared when the sign vanishes**; they hold the last picture written until
the next job or the next level load (which zeroes them), so every sign visible at the same time shows the same picture.
That never contradicts the numbers: a new job always changes $D2's nibble, and a visible marker whose $0393 differs from
it is freed on the next step (above); markers with the same number simply share the picture (in `items.replay` the box
sign of step 244 shows "25" until step 256, when the star's job rewrites the tiles and frees it, and the star sign
shows "50" from step 258). The sign is 16 px wide (two 8-px tiles, four 4-px digits, centred on the marker). The
remake draws it by uploading the pair for the nibble into sprite tiles $BC / $D0 of table 0 when the marker is made.

### 6.3 Warp-level countdown (state $0D, made at bank 0 $A902) **[C]**, slot 3 and its stale bytes **[V]**

The slot is always 3 (the first spawn after the load). It writes x, y (both bytes), state, frame, flags X and the counter;
every other field keeps the previous occupant's value (`objects.md` 2.2, round 13).

Every step it is placed 56 px below the pod (x = pod x, y = pod y + 56) with flags X = 0. Its horizontal speed bytes
are used as a 16-bit counter, + 1 per step; frame = $73 + counter / 64. When the frame would leave $73-$7D (after 11 ×
64 = 704 steps, about 23.4 s): transition request 6 with RAM $DB = 0. So each digit 10 ... 0 shows for 64 steps (128
frames), not one second.

### 6.4 Pod-side states (owned by `player.md`) **[C]**

States $01 and $02 (pod bullets and falling sparks: move, field $0393 + 1, freed at 144; frames $37 $62 $63 $62 by
$0393 mod 4; state $02 also **rises** [V, round 13]: after each move the vertical speed changes by $40 towards up: moving down it slows and turns at 0, moving up it speeds up to the cap 3.00; `economy.md` 5.4, the crystal pick-up's three sparks), $08 and $20 (pod bullets
with lifetime counted every 4th step and frame changes by RAM $050E), $09 (tractor-beam marker), $0B (pick-up
sparkle: $0393 − 1, freed at 0, frame $69 + ($0393 / 2 mod 4)), $10, $11 (nothing), $12, $13, $14 (pod weapons). They
take part in 3.2-3.4 as shots.

## 7. Work fields per state

| State(s) | $0393 | $03B2 | work 5 ($0403) | work 6 ($0422) | Sub-pixel bytes |
|---|---|---|---|---|---|
| placed (≥ $80) | list index | type | per type below | per type below | normal |
| $86-$95 turrets | | | phase timer | phase 0-4 | |
| $98/$9D/$9E/$AC guns | | | fire / barrier timer | partner slot | |
| $99-$9C walkers, pillars | | | jump timer | head slot | |
| $9F-$A1 turrets | | | launch timer | | |
| $A2/$A3, $97/$A4 | | | bit 0 closing, bit 1 lifted | fire timer | |
| $A5/$A6 magnets | | | spin counter high | spin counter low | |
| $A7/$A8 | | | bit 7 target P2, low bits animation | fire timer | |
| $A9/$AA formations | | | leader slot | bit 0 started, bit 1 leader | x: phase; y ($AA): fire timer |
| $AB green creatures | | | idle count ($FF → $81) | mode | x or y: mode timer |
| $AD-$B3 generators | | | | step counter (gated kinds) | |
| $AF blue sphere | | | fire timer | idle counter | |
| $B8 egg | | | | age | |
| $BA companion | | | satellite slot | phase | |
| $03 exhaust | | | | owner slot | |
| $04/$05 explosion | timer (4 steps) | sequence position | | | |
| $06 marker | bonus number | age | | | |
| $0D countdown | | | | | x speed bytes: counter |
| $0F/$14 missiles | target slot | direction | lifetime | straight-flight steps | |
| $15/$1A/$1B shots | lifetime | | | | |
| $17/$18 flames | age | | | | |
| $19 drone | re-plan timer | mode | fire timer | | |
| $1C/$1D heads | | animation counter | | owner slot | |
| $1E mine | speed cycle | | | | |
| $1F hopper | step counter | mode | walking flag | base frame | |
| $22 thruster | | owner slot | | | |
| $23 green orb | counter | | | | |
| $24/$25 bonus | | age | | | |

Work 1-4 hold speeds except for the home copies of types 12, 17, 18, 47, 5E (`objects.md` 3.4) and of the formations
(4.12), and the walkers' jump origin (4.3).

## 8. Random numbers

Uses by objects (R = RAM $5A, R1 = $5B, R2 = $5C as defined in section 1). **[C]**; turret, formation and (2026-10-17)
all the rows below for the types of levels 4-7 **[V]** (walkers, domes, wide machines, aimed shots, generator 48, type
22, satellite, edge positions, shot deflection); the rows added or checked on 2026-10-17 for the later levels are
marked [V] in the table (the shot deflection by R2 and the beam-held orb case were not run).

| Who | Value | Drives |
|---|---|---|
| Turrets 01-10 | R mod 16 + 4 | open wait before firing (4-19 steps) |
| Turrets 01-10 | (R1 AND $2F) + 9 | rest between shots (9-24 or 41-56 steps) |
| Formation $AA | R (0 → 256) | steps until the next shot of each part |
| Green creatures | R mod 16 | length of each chase leg (64-79 steps) |
| Walkers | R mod 4 = 0 | jump (chance 1/4 every 8 crouching steps) |
| Dome 1E | R1 mod 8 = 3 | lift off (checked every 32nd step, F mod 64 = 0) |
| Dome 11 | (R1 / 2) mod 64 = $17, R1 bit 0 | side flame and its side |
| Domes | R mod 4 | aimed-shot table entry |
| Domes, wide machines | R mod 8 = 0 | start opening (checked every 16 steps) |
| Domes, wide machines | R1 bit 0 | which missile entry |
| Dome 11 | R1 mod 64 = $17 | launch a drone |
| Aimed shots | R1 mod 64, R2 mod 64 | spread of the speed |
| Generators | ($5A + $61) mod 64 = $1F | create a creature |
| Edge position | R2 bits 1-2, R1, R2 | edge and offset |
| Green-orb spawner | R bit 7 **[V]** | create an orb |
| Type 21 | R mod 4, R bit 1, R1 | thruster frames, dropped shot's direction and speed |
| Type 22 | R1 mod 64 = $16 | extra forward shot |
| Satellite | R AND $3E = $16 | fire |
| Level 26 $B7 (type 5E) **[V]** | R, or R1 for odd slots, AND $1F | start of the next rest (work 5) |
| Level 26 $B6 **[V]** | R | fraction of its horizontal speed when it sets off |
| Companion phase 5 **[V]** | R1 AND 7, then R AND 7 | which slot 23-30 it shoots at |
| Shot deflection | R2 | added to the speed's fraction byte of the shot (3.5); **[V]** that one fraction changes in a bounce (23 cases), **[C]** that the amount is R2 |
| Pod's homing missile and multi warhead | R (+ $57 + c), R AND 7 | which slot 23-30 to look at, at each look for a target (5.11.1) **[V]** |
| Pod's star parent | R1 AND 7, then R AND 7 | the target slot of each child (5.11.2) **[V]** |

Changes to $5A made by objects: domes and wide machines += $49 when they start opening; generators += $61 each
time they act; the pod's missiles += $57 + carry when picking a target (5.11.1; the carry is bit 8 of 2 × slot + $5D when the missile had no target, else 0 **[V]**). These are in addition to the three listed in
`game-flow.md` 5.

## 9. Sound requests by objects **[C]**

| Effect | Channel | When |
|---|---|---|
| $08 | both pulses | an enemy fires (the shooters of sections 4-5 except the large creature, type 21, and the large machine, type 22, whose shots make no sound **[V]**, `sound.md` 8.3) |
| $04 (noise channel) | noise | a shot destroys a fuel pod (3.3) **[V]** |
| $44 | both pulses | the wormhole bounces the pod (`economy.md` 5.3) **[V]** |
| $0A | noise | a shot hits a placed object and explodes; a small object is destroyed |
| $0C | noise | a medium or large object is destroyed |
| $0E | pulse 1, second request slot | a shot is deflected |
| $16 | pulse 1 | the pod touches an object that survives |
| $18 | pulse 2 | a generator launches a drone |
| $42 | pulse 1 | a shot collects a bonus object |
| $02 + $04 | pulses | impact on the pod (bank 2 $8127) |

## 10. Quirks and fidelity

These are faithful descriptions of the original; whether to keep each is a fidelity decision (`PLAN.md`). Alex's
decisions so far are in `docs/deviations.md`: Q1 (2026-10-16), Q11, Q12-Q18 (2026-10-17) are corrected in the remake
(each as a deviation, running through `Gameplay::set_original_rules` in validation); Q2-Q10 stay as in the
original. Q19-Q26, Q30-Q37 and Q42-Q44 (2026-10-17) await a decision.

- **Q1** Waypoint distance (2.7): a target 256-271 px below counts as reached, so type 21 creatures with a long
  vertical path turn back early (seen on level 2, and on level 8 indexes 75 and 76 and level 15 index 6, where the
  creature of index 76 never leaves the neighbourhood of P2). Not intended: the route is in the level data. Decided:
  the remake patrols the whole route (`docs/deviations.md`, "Waypoint patrols").
- **Q2** Collision push carry (3.1): penetrations are 1 larger when the centres are close. Small; measured.
- **Q3** Shot box (3.4): the vertical half-size added is 2 above the object but the pod's half-height below it.
- **Q4** One hit per step: only the first object can hurt or be hurt by the pod, and only one shot can hit a placed
  object per step (3.1, 3.4).
- **Q5** Green creature (4.13): when left of the pod its horizontal distance is taken as (|dx| high byte, 64).
- **Q6** Type 1F's horizontal pull uses the whole distance d (4.9).
- **Q7** Destruction frees every explosion in slots 23-30 (3.5).
- **Q8** Type 21 overwrites slots 15 and 16 with its thrusters, and hides whatever is in them (4.10).
- **Q9** Some creators search 30 down to 23 and use slot 22 when nothing is free (dome exhaust), or overwrite slot 22
  (green orb's shot).
- **Q10** Edge positions add the offset to the low byte only (2.10): a carry or borrow past the low byte is lost, so in about one spawn in
  ten the object lands 256 px from where it should and is freed at once. This is the reason for the one missing orb of 5.4 (level 8, step 1,856).
  Decided (2026-10-17): the remake fixes it (`docs/deviations.md`, "Edge spawn wrap").
- **Q11** The pod's speed change on a vertical push while invulnerable ($97 non-zero) uses a leftover scratch value, the low byte of the y distance (rule in 3.1); a horizontal push leaves the speeds alone. Decided (2026-10-17): the remake sets the pod's speed on that axis to 0 (`docs/deviations.md`, "Push while protected").
- **Q12** Aimed shots (2.8) keep the sign of the horizontal speed but not of the vertical one. When the pod is nearly
  level with the muzzle the scaled |dy| is smaller than the random offset, the vertical sum goes negative and is read
  as about 255.9 px per step in the shot's vertical direction: the shot is gone within one or two steps. It happens
  whenever |dy| × 2^k + (R2 mod 64) − 32 < 0 where 2^k is the scaling of 2.8: for a pod roughly level and 128 px or more
  away (k = 0), |dy| < 32 gives a chance of (32 − |dy|) / 64 per shot. Evidently intended: a signed vertical speed,
  so the shot drifts a little the other way, as it does horizontally. Seen once on level 4 (y 2333 to 2077 in one step). Decided (2026-10-17): corrected in the remake (`docs/deviations.md`, "Aimed shots near level").
- **Q13** Missile steering (2.9). When the target is farther along the vertical than the horizontal, the original
  flies straight up or down only if ay < 2·ax and diagonally if ay ≥ 2·ax, the opposite of the horizontal case (straight
  only when the long side is at least twice the short one). A missile directly above or below its target therefore
  flies diagonally and zig-zags, and one at 45-63 degrees flies straight. Evidently intended: the same rule on both
  axes. Beyond 256 px the byte-wise arithmetic is also slightly off. Decided (2026-10-17): corrected in the remake (`docs/deviations.md`, "Missile steering").
- **Q14** A lifted dome (4.7) moves twice per step in the half of its cycle where it homes on the pod (the homing
  routine moves, then the dome moves again), so it is twice as fast as in the first half of the cycle, where it
  coasts and moves once. Evidently intended: one move. Decided (2026-10-17): corrected in the remake (`docs/deviations.md`, "Lifted dome speed").
- **Q15** The type 22 volley (4.11): (a) the four shots are written into slots 15-18 whether or not they are free, so
  they destroy any shots (and children) already there; (b) the two down-going shots get a vertical speed of 1.84 and
  a horizontal speed of 3.004 instead of 0.84 and 3.00 of the two up-going ones, because the loop counter 0 or 1 is
  stored as the whole part of the vertical speed and the fraction of the horizontal one. Evidently intended: equal
  speeds for all four; (a) may be intended. Decided (2026-10-17): corrected in the remake (`docs/deviations.md`, "Type 22 volley").
- **Q16** Collision flag bit 0 ("no damage") protects only against the pod's touch. Shots take hit points from every
  such object (3.5): walkers (and, by their flags, pillars and the flames in slots 24 and 25) can be shot to pieces
  at 255 hit points. Evidently intended: invulnerable obstacles (the 255 and the flag say so). Decided (2026-10-17): corrected in the remake (`docs/deviations.md`, "Indestructible obstacles").
- **Q17** Several creators leave stale fields in a reused slot (2.5): the sub-pixel position of the previous
  occupant (at most 1 px), and for the type 22 volley, the exhaust in slot 22 and the hourglass drones also work
  bytes, speed bytes or the fire timer. Harmless; the remake may clear them. Decided (2026-10-17): corrected in the remake (`docs/deviations.md`, "Stale slot fields").
- **Q18** The type 11 dome's pull on the pod (4.7 step 3) flips the pod's vertical direction and moves it |dy| in the
  new direction. With the pod heading down this lifts it onto the dome's row, which is evidently the aim (the side
  flame then hits it); with the pod heading up the same rule would send it down, away from the row (inferred, not
  seen), and it also reverses the pod's vertical motion. Decided (2026-10-17): corrected in the remake (`docs/deviations.md`, "Dome pull").
- **Q19** The pod-below side flag (2.4) is only computed once |dx| is known to be below 256: with |dx| ≥ 256 it stays
  0, so the pod counts as "not below the object" (above or level) whatever their y. Seen on level 26: a type 4F-53 object far to the side
  of the pod starts its flight upward even when it is above the pod (3 of the 6 starts seen with |dx| ≥ 256; with |dx| < 256 it
  starts toward the pod's row). Other readers of the flag at long range (the drone's mode choice, 5.5; the hopper,
  5.7) are affected the same way, and by Q24 also read a wrong |dy|. Evidently intended: the flag holds for any distance (the
  distance itself is capped at 255 for the same reason).
- **Q20** State $21 (4.16) skips its homing on the step its counter is 8: the program loads the new frame ($AF) into
  the register it then compares with $20, which is no longer the counter. The piece moves straight for that step.
  Harmless (one step in 32 of the homing). Evidently intended: homing for the counter's first 32 steps.
- **Q21** The pieces made when a level 26 object is shot (4.16) do not clear their counter $0393: they start with
  whatever the slot's previous occupant left there (a placed object: its list index, 0-46 on level 26; an explosion
  in progress: 1-4; an earlier piece: its last count). A piece lives 128 − counter steps (as few as 82) and homes on the pod only if the
  counter is below 32. The sparks of the final-level effect set the counter to 0 and are not affected. A case of
  Q17 with a visible effect; evidently intended: counter 0 (128 steps of life, homing for the first 32).

- **Q22** A star (state $24) collected **by touch** makes its marker with field $0393 = 0 while the star sets RAM $D2 to $41 (nibble 1):
  the marker (6.2) is freed on the next step, before it ever shows its frame, whereas a star shot gets a marker with $0393 = 1 that
  rises and shows (3.3). The score +50 is given either way. Seen: level 0, a star poked onto the pod (marker made at step 4, gone at
  step 5), against the shot star (marker lives). The box has no such difference (its nibble is 0). Evidently intended:
  the same floating sign for both ways of collecting. **[V]** 2026-10-17.
- **Q23** The fourth-slot pull-back (6.1): an item in slot 14 (or 13 while the beam holds 14) creeps to its ROM position when slots 11 and 12 are
  occupied, and the RAM list is reset to the ROM position. The condition looks like a leftover or a mistaken slot test; no gameplay reason
  is evident (the effect also drags an item the player has placed deliberately, and it hits whichever item happens to be fourth). Intent
  **[H]**: items that were pushed or dropped far from their place return when the window is crowded, or an unfinished
  "mothership collects items" feature. Seen only with four objects in the window at once, which on the original levels needs the mothership
  indicator (near home) and three items close together, or four items.

- **Q24** The distance routine (2.4) leaves |dy| unfinished when it returns its capped 255: with |dx| ≥ 256 it never
  computes |dy| (the callers read a leftover byte of an earlier calculation), and with |dy| ≥ 256 the byte is the low byte of
  the signed difference. The drone (5.5) and the hopper (5.7) test that byte (drone: climb if ≥ 16; hopper: < 24 means level
  with the pod, else the distance class), so a drone more than 256 px to the side always climbs when the leftover is 16 or
  more (as seen), and a hopper far to the side acts as if the pod were above it. The same early exit is behind Q19 (the
  flag). **[C]** for the mechanism; the leftover byte itself is **[V]** (seventh round, 2026-10-17: $20 in every traced call, the
  last value stored by the B pass's sprite-grid drawing; see 2.4; it would differ if an earlier object in the same sweep had
  used the routine). A difference of exactly −256 is not an early exit (it counts as 255 and the y part is computed), while
  +256 exits: an asymmetry of the same slip. Evidently intended: a true |dy| and flag at any range (or a plain "far away" answer that makes
  the drone and the hopper ignore the pod).
- **Q25** The green creature (4.13) takes a repeated vertical choice as "same mode" only once: that step decrements its
  mode byte from $FF to $FE (it stands still for the step), and the next vertical choice compares the new $FF with the
  stored $FE, sees a change and starts a 16-step pause although the mode is the same. A repeated horizontal choice is
  not affected. **[V]** level 2, slot 25, |dx| 6 and |dy| about 20 throughout: work 6 went $FF, $FE (step 585, no move),
  then a pause (step 665), and again $FF, $FE (755), pause (831). Evidently intended: continue the mode without a stop,
  as with horizontal choices.
- **Q26** The fuel pod (kind 11, 3.3) sets only the high byte of the fuel ($E0) and leaves the low byte, so a refill gives
  $E000 + (old low byte) instead of exactly $E000: up to 255/65536 of a unit too much, never visible on the gauge.
  Harmless. **[C]** Evidently intended: the 16-bit value $E000 (as at every other refill, `economy.md`, full tank).

- **Q27** A pod missile (homing missile or multi warhead, 5.11.1) with no target steers at **the pod**. "No target" is stored
  as slot 0, and slot 0 is the pod: every 8th step the missile recomputes its direction towards the pod, and keeps flying
  that way; with an empty window (no live object in slots 23-30) or after the target died and the next look failed,
  the missile turns back on its owner. **[V]** 593 of the 1,207 direction recomputations in the demos, 19 of 24 flights
  (the pod is not hurt by its own shots, so it is only a wasted missile that circles the pod until its 384 steps are
  up or it leaves). Evidently intended: keep the last heading, or fly straight on.
- **Q28** The star bullet's children age only in odd slots (5.11.2). The aging test adds the slot number and the frame
  counter and one (a carry left by the move routine) and wants a multiple of 16; the counter is even in the
  step that runs the objects, so only odd slots can pass. Children are made in slots 3-6 (the parent is in one of them),
  so of the three child slots, those of number 4 and 6 hold children that never change frame and never expire: they
  fly on at their creation velocity until they hit something or leave the window (up to 194 steps seen). The odd ones last
  41-48 steps. Evidently intended: every child lasts about 48 steps (six stages of 8 steps). **[V]** all 147 stage changes in
  the demos were in slots 3 and 5; slots 4 and 6 never changed in the demos or the harness.
- **Q29** Mass destruction frees explosions: the destruction rule of 3.5 first frees every explosion in slots 23-30, and the
  smart bomb and the time bomb call it once per victim, so only the last victim of a bomb is seen exploding; the others vanish
  in the step they die (their score is given). This is Q7 made visible. Evidently intended: all explode. **[V]** harness
  (smart bomb: 4 victims, 1 explosion; time bomb: 3 victims, 1 explosion).

- **Q30** A special-weapon press whose slot is above 6 (D1 = 1, 5, 6 or 8) or that is a smart bomb or a switch leaves the plain
  bullet's bytes in the slot (player.md 10.1): the program writes the whole plain bullet (position, direction bits, frame $37,
  hit points, field $0393 = $20, $03B2 = 0), then clears **only the state byte**. A spark that was taken over in 7-10 is
  therefore destroyed, and the free slot keeps those bytes (and, in the above-6 case, the plain bullet's speed vector as
  well, since the speed routine still runs). Later creators that "do not clear" a field (2.5) inherit them. No effect is
  seen in play because every creator in 3-10 sets what it needs. **[C]** (the code path; the slot-above-6 loss itself is
  [V], `economy.md` 6). Evidently intended: the press makes nothing and leaves nothing (or: a missile slot is simply
  not available), i.e. a fully cleared slot.
- **Q31** The marker made by a **shot** hit on a star or a box (3.3, bank 1) copies only the target's position and clears
  the direction bytes, the vertical speed fraction, its counters and $0393; it does **not** clear the horizontal speed
  (whole and fraction) or the two position fractions (RAM $0197 and $023E plus slot), so these keep what the slot's previous occupant
  (a shot, a spark or another marker) left. Harmless: the marker's behaviour (6.2) moves it vertically only, 1 px per step with a
  zero speed fraction, so neither the stale horizontal speed nor a stale fraction changes the path (a fraction only shifts
  the sub-pixel phase). The two other creators (the touch of a star or box, and the beam counting event) clear all ten
  fields. **[V]** 2026-10-17 (level 0, star in slot 13 shot; slot 10 poked with horizontal speed 2.03 and fractions $80 beforehand: the
  marker kept vx 02.03, sx $80, sy $80, and rose 1 px per step at constant x = 617 for 30 steps). Evidently intended: a clean slot.
- **Q32** A smart bomb fired on the **final level** clears the pod's own state, so the pod stops, the camera stops and the
  level can never end (the game is stuck until reset). Cause: the final-level pod's gun (`player.md` 9.1 item 7) calls the
  ordinary gun routine, takes any non-zero result as "a shot was made in slot X" and writes the shot's bytes (state $20,
  frame $6D, speed 7.00 right) at index X. After a smart bomb, X is not a shot slot but 31 (the end of the slot loop the
  bomb ran), so the writes land on slot 0 and on the fields beyond it: the pod's state byte (RAM $0317) becomes 0, its
  horizontal direction byte ($0336) $20, its vertical whole speed 0 and the next bytes get $6D and 7. The routine's result is the
  weapon gauge's high byte after the payment, so the damage is done whenever it is non-zero: **gauge above 128 before the
  press**. With a gauge of 128 or less the result is 0 (the gauge is emptied) and the press is harmless: the five cores
  are destroyed and the level ends in the usual way (`levels.md` 5.2, "End"). **[V]** 2026-10-17 (Mesen at the end of the
  route, smart bomb fitted: gauge $FF and $81 froze the game for 5,000 steps with the pod in state 0; gauge $80 and $40 ended the
  level 11 steps later with the message screen). Evidently intended: the bomb takes effect and nothing is written, as the
  special shots' "slot above 6" case leaves the slot alone (Q30); a smart bomb on the final level should wipe the cores and
  let the level end. (The other no-object weapons, the switches D1 = 2, 3, 7, return through a different exit; whether they make a
  stray shot on that level is not checked **[H]**.)
- **Q33** The final message (kind 7, started by the level-26 end routine) does not reset the frame counter RAM $5D as the other
  message screens do, and its timer RAM $DA falls only on frames where the low byte of $5D is 64 or more, so how long the
  screen stays depends on when the last core died: 319 frames when $5D is 64 or more at its start, 383 - s frames when it is s
  below 64 (the trigger waits for $5D AND $1E = 0, so s is 3 or 35 in two cases of eight, giving 380 or 348 frames), plus 16
  blank frames either way. Harmless; the remake may use 319. Evidently intended: a fixed length. **[V]** 2026-10-17 (two
  natural runs, $DA read every frame: s = $A3, 335 frames; s = $23, 363 frames).

- **Q34** The approach screen puts a planet's three globe colours into palette entries 21-23 instead of 9-11 when the
  planet's surface picture number AND 3 is 1 (pictures 1 and 5: planets 2, 4, 7 and 9), so those globes are drawn
  in the preset's default colours ($22 $2A $19, planet 0's). The test seems to belong to the surface scene's
  palette (a per-picture rule); on the approach screen it hides four planets' own colours (`graphics.md` 7.3,
  `scenes.md` 7). Evidently intended: every planet's globe in its own colour. **[V]** 2026-10-17 (palette RAM read
  for planets 0-12).
- **Q35** Every sprite of a layout with one or more entries (count above 0) drawn by the object drawer without a
  vertical flip is shifted one pixel to the right (x = X ± dx + 1), while the same layout drawn with the vertical
  flip is not (x = X ± dx); the pod, with its own drawer, is exact. A stray +1 from the drawer's arithmetic; harmless,
  as only a few objects set the vertical flip. **Layouts with count 0 (a single sprite at the object's position) never
  get the +1, flipped or not: x = X exactly.** That covers the pod's exhaust flame in slot 1 (frames $18-$37) and
  Jetman's small flame (frame $10), among others (the frame list is in `objects.md` 6.2). Evidently intended: the
  same x for all (`objects.md` 6.2). **[V]** 2026-10-17 (surface-scene ship with fy poked, and an object at rest in
  level 0); count-0 exception **[V]** 2026-10-20 (black-box screen comparison of the delivery replay, steps 82-230:
  flame at X exactly) and **[C]** (the drawer's count-0 path stores the screen position without the offset sum).
- **Q36** In the surface scene's descent the mothership's exhaust is placed at the ship's position of the previous
  pass (the copy is made before the ship's move), so the flame's y lags the ship's by one pass; while leaving it is
  placed after the move and does not lag (`scenes.md` 8.2). Evidently intended: flame always at the ship. **[V]**
- **Q37** The planet map's status panel sits one line higher than in play: sprite 0, the marker that starts the panel's
  split, is at y = 190 in the map instead of 191, so the page-0 picture ends at line 191 instead of 192 and the panel
  (rendering off at 191-193, back on at 194) starts and ends one line earlier (content on lines 201-224 instead of
  202-225). Harmless, a one-line jump between the paused game and the map. Evidently intended: the same split as in
  play (`scenes.md` 13.2, `graphics.md` 1.1). **[V]** 2026-10-17 (OAM and the mask writes read back, screenshots
  of the paused game and the map compared row by row).

- **Q38** On warp levels 37 and 39 (flag byte $C0, `objects.md` 10.6) the mesh helpers home on the pod and are then overwritten
  every frame by the per-level table step, whose index is stale, so the mesh tile's offsets jump about arbitrarily; on
  33 and 35 (flag $80) the helper's own path is used and the mesh drifts smoothly after the pod. Evidently a slip (the two
  levels should behave like 33 and 35, or like the table levels). **[V]** 2026-10-17 (level 37, positions of slots 23 and 24 logged
  against the camera and the pod; levels 33 and 34 compared); the exact stale lookup (index = slot 24's x low byte after its move; ROM bytes 0x287B-0x28A2 + index) **[V]** round 13, `objects.md` 10.6. Decided (2026-10-17): the remake uses the level-33 behaviour on 37 and 39 (`docs/deviations.md`).
- **Q39** A closed wormhole (`player.md` 9.2.2) always makes its sparkle in slot 3, overwriting a pod shot that may be there: a
  bullet in flight can vanish at the moment of a bounce. Harmless; the remake may use its own effect slot.
- **Q40** On planet 4 the backdrop interlude (`scenes.md` 11.1.2) starts its spawn index at 125 instead of 0, because the
  index is taken from the camera's high byte and only planets 0 and 8 start with it at $FF. The first three spawns read
  bytes beyond the object lists (x = 3, types 5, 232, 224): objects that are never drawn and never touchable, yet fill
  three of the four object slots for several hundred steps, so the gem of entry 33 (x = 64) is lost and the planet offers
  9 gems instead of 10. Evidently a slip (the lists are laid out for an index from 0). **[V]** 2026-10-17 (Mesen, every
  spawn logged and compared with the ROM bytes on all three backdrops). Proposed: the remake starts the index at 0 on
  planet 4 as on the others (10 gems, no phantom objects). Awaits Alex.
- **Q41** The interlude's objects keep the count and y fraction of the slot's previous occupant (`scenes.md` 11.1.2): a spawn
  does not clear them. Harmless (animation phase and less than 1 pixel of y); the remake may start clean. **[V]** 2026-10-17.

- **Q42** The first lit frame of the ending and of the credits is drawn from a sprite buffer that is not yet complete: two badge
  sprites missing in the credits' first lit frame (a 16 x 9 patch), a strip of sprites missing in the ending's (`scenes.md`
  14.3). Cosmetic, one video frame; the remake draws both full. **[V]** 2026-10-17, round 13 (every frame of five forced runs
  photographed; the buffer read back).
- **Q43** The credits' last frame is cut: the warm reset switches rendering off in the middle of that frame (line 82), so only the
  top 13 lines of the badge show for one video frame before 15 black frames and the title. Cosmetic, the remake may end the credits
  on the complete last picture. **[V]** 2026-10-17, round 13.
- **Q44** Inconsistent pod facing across level changes: 0 after a wormhole gate, kept after the warp-level countdown (`player.md` 4.1a). Harmless;
  the remake may reset it in both cases, or copy the original. **[V]** 2026-10-17, round 13.

- **Q45** The terrain test's segment buffer (RAM $0596-, `levels.md` 6.4 step 1, "Storage and its overflow") is 56 bytes long, but the program does not check its length: when the nine blocks around the player hold enough
  segments (B = $38 or more) the lists spill over RAM $05CE (the F gauge's display copy) and $05CF-$05DC and beyond (the table-A tile ranges). Two visible effects, both seen on level 11 from step 7,129 of
  `level11.replay`: (a) the gauge: every step the test writes a list byte into $05CE and the easing (`economy.md` 3.1) then adds only 1-3 to it, so the "F" bar sticks at a small value ($12 in the run: one full cell and a little) while
  the fuel is near $DE, and climbs again by the easing once B falls below $38; (b) the objects: the ranges of `objects.md` 5.2 are overwritten and stay overwritten until the level is loaded again (the list bytes are written, not
  restored), so table-A objects in the window afterwards get another tile base (the magnet of frame $7E was drawn with tiles $0A-$0D instead of $7B-$7E: pod and spark tiles). A bug of the original (an unchecked buffer), not
  intent. Picture comparisons against the original differ for as long as a spill lasts.
  Decided (Alex, 2026-10-20): the remake does not reproduce it (the F gauge follows the fuel by the easing rule alone; tile bases are never clobbered); `docs/deviations.md` row to be added by the implementer. **[V]** 2026-10-20 (hooks on every write of $05CE: the two writers per step and the buffer's arithmetic, the RAM at steps 7,100, 7,129-7,131 and 7,140).

## 11. How this was verified

Mesen runs (`tools/mesen/run.sh`), loading savestates taken just after the launch of levels 0-3, holding the pod at a
chosen point (speeds cleared before each A pass, RAM $97 kept non-zero so no damage happens), and recording the
object table at the end of every B pass (`game-flow.md` 1.4). A model written from this spec was run on each recorded
step and compared field by field (position, sub-pixels, speeds, flags, frame, work bytes):

| Object | Level | Steps compared | Mismatches |
|---|---|---|---|
| Turrets 05, 06, 08, 0B, 0C, 0F (phases, frames, timers) and their shots' spawn values | 0 | 1,191 (17 shots) | 0 |
| Enemy shots, movement | 0 | 1,842 | 0 |
| Formation 33-40 (positions, frames, flags) | 2 | 2,884 | 0 |
| Formation shots (position, speeds, flags, timer reload) | 2 | 14 | 0 |
| Green creatures 43 (with the pod push of 3.1) | 2 | 2,693 | 8, all explained by the held pod drifting 1 px |
| Mine (state $1E) | 0 | 1,074 | 0 |
| Generators 49, 4C, 4D (when creatures appear) | 0, 2, 1 | 1,495; 1,498; 1,491 | 0 |
| Hopper (state $1F) | 1 | 3,488 | 1 (pod more than 255 px away, 2.4) |
| Drone (state $19) | 2 | 2,029 | 0 |
| Magnets 1F (pull) and 20 (push): pod speed after the step | 1 | 513 | 0 |
| Pillar 15 and head: growth, head height, head shots | 1 | 400 | checked by eye |
| Type 21 waypoint path (Q1) | 2 | 1,300 | observed |
| Egg 5F: frames and removal | 3 | 72 | observed |

Not verified in play at that time: damage and destruction rules (3.1 damage part, 3.3-3.5), score, the types of levels
4-28 (4.2-4.8, 4.10-4.11, 4.15-4.18), and section 6. The types of levels 4-7 were run on 2026-10-17 (next table) and
those of levels 8-28 and the rest the same day (third table).

For the types that first appear on levels 4-7 the same method was used with the launch savestates of those levels
(`tools/mesen/run.sh`, hold the pod, hold the camera so that the object is just off screen when it spawns and stays
inside the window, 3.4 and 3.5 of `objects.md`; some runs move the pod at a chosen step, shift the frame counter, or
press A with the pod facing a target). Models written from this text were compared field by field and step by step;
where they differed, the cause was found in the program and the text corrected (the corrections are marked in
place). The pod's position as the objects see it is its position after the player step of the same logic step.

| Object | Level | Steps compared | Mismatches left |
|---|---|---|---|
| Dome 11, its aimed shots, missiles, flames, drones | 4 | 890 steps, 4,500 shot-steps, 650 missile-steps, 740 flame-steps | 0 (shots and missiles within 14 px of the pod excluded: pushes) |
| Dome 1E: ground, lift-off, homing, exhaust | 4, 7 | about 3,200 | 0 |
| Wide machines 1C, 1D, shots, missiles | 4 | 1,595 | 0 |
| Ground guns 12, 18, 47 and their shots | 5 | 391, 493, 498 | 0 |
| Walkers 13, 14 with heads 1C, 1D and head shots | 5 | 1,190, 1,199 | 0 |
| Hourglass drones 19, 1A, 1B: place, direction, timing | 4, 6 | 14 launches | 0 |
| Large machine 22: movement, ticks, volley, extra shot | 4, 7 | about 3,000 | 0 |
| Formation parts, state $A9 | 7 | 7,179 part-steps | 0 |
| Generator 48 and the mines it makes | 4 | 2 × 2,000 steps, 8 creations | 0 |
| Satellite 60, companion 61 (orbit, search, chase) | 7 | 340, 1,300, 390 | 0 |
| Missile steering, all recomputations | 4, 7 | 337 | 0 |
| Pod touch (3.1), shots against guns, domes, walkers (3.5) | 4, 5, 7 | 12 probes | as in 3.1, 3.5 |
| Warp levels 29-41 (`economy.md` 5.4) | 29-41 | 720 steps each | no hazards |

Third round, 2026-10-17: everything that first appears on level 8 or later, and the rules that were still only read.
Same method, with two additions: the savestates of levels 13-28 in the first set were made by poking the level number
(`levels.md` Appendix A), which leaves the previous planet's object table in place, so level 15 was entered again by
the game's own transition (a second savestate), and level 26 was run along its route (4.16, "How it was run").
Models written from the text above were compared step by step.

| Object | Level | Steps compared | Mismatches left |
|---|---|---|---|
| Ground-gun pairs 17 + 18 (partner search, who acts, flames) | 8, 15 | 400 + 100 | 0 (flames: 99 steps, 9 flames) |
| Blue sphere 4A and its shots | 8 | 680 | 0 (pod within 16 px excluded: pushes) |
| Type 21 on a long route, its thrusters and shots | 8 (index 76) | 1,296 + 695 | 0 |
| Waypoint behaviour (Q1) | 8 (75, 76), 15 (6) | 1,500-2,200 per run | as 2.7 |
| Large machine 22, three table rows | 12 | 1,397 + 1,154 + 1,138 machine-steps | 0 (respawns excluded) |
| Turrets 01-04, 07-0B, 0D, 0E, 10 and shots | 3, 6, 8 | 17,400 turret-steps, about 260 shots | 0 |
| Pillars 15, 16 with heads and head shots | 1 | 397 + 497 | 0 |
| Generators 4B, 4D, 4E | 4, 8 | 3,000 + 1,500 + 4,000 | 0 (11 creations) |
| Green orb and spawner | 8 | about 1,000 orb-steps, 33 spawner checks | 1 unexplained (5.4) |
| Companion in the pod's service (phase 5) and its 38 shots | 7 | 596 | 0 |
| Level 26: types $B4, $B5, $B6, $B7, pieces $21, splits | 26 | 1,900 + 1,700 + 8,600 + 10,200 + 5,350 | 0 (windows and respawns excluded; Q19-Q21 are the model's differences from a naive reading) |
| Stars ($24) and markers (6), scores of 3.3 | 1 | 150 steps | as 3.3, 5.10, 6.2 |
| Items (6.1), frame by kind | all runs | about 50,000 | as the table, for the nine kinds seen |

Fourth round, 2026-10-17: the items of every kind, the box, the beam-held orb, the fourth-slot event and the final level's
background effect. Same method: savestates of levels 0, 8 and 26, a pod held still or free, RAM written at a chosen step
(list entries, object slots, the pod's state), hooks on the program counter for the routines concerned. The items and pickups
were poked in because most kinds are in no list; the numbers (frames, states, offsets) are from the logs.

| Subject | Level | Run | Result |
|---|---|---|---|
| Item kinds $03-$08, $0D, $0E, $13-$1B: frame, state, catch, load class, delivery, touch | 0 | one run per kind, 420 steps (delivery) or 150 (touch) | as 6.1 and `economy.md` 2.1 |
| Catch boundary of the beam | 0 | 40 placements | as `economy.md` 2.1 (one pixel less than the table for dx below 6) |
| Box ($25) and star ($24): touch, shot, caps, markers, sound, lifetime, tag rule | 0 | about 40 runs | as 3.3, 5.10, `economy.md` 2.5; Q22 |
| Beam-held orb and the held spawner rule | 8 | 700 + 1,700 steps, five settings of $6E | as 5.4; 1,011 orb-steps, 0 differences |
| The missing orb of step 1,856 | 8 | the original 4,200-step run replayed to step 1,900, write hooks | as 5.4 (Q10) |
| Pull-back of the fourth slot (bank 2 entry $8027) | 0 | two runs of 80 steps | as 6.1; Q23 |
| Final-level background effect: gate, check, phase counter, spawn, end | 26 | the route run, 4,700 steps; two more of 4,200 and 4,600 with the cores poked away | as 4.16: 22 activations, every spawn 8 steps after its activation |

Fifth round (2026-10-17), answering thirteen fitted points from the code and the earlier logs, no new Mesen run except as
noted: the aimed-shot scaling, the missile's two phases, the dome and machine step end, the hopper's away branch, the heads'
destruction, the generators' clean-up and the fuel pod are **[C]**; the green creature's order within a step and Q25 are
**[V]** against the level 2 log of 900 steps (work 6 over time and the positions); the pod-collision early end is **[C]**.
The level 3 gravity is in `levels.md` 6.5 (**[V]** in Mesen).

Sixth round (2026-10-17), the pod's special weapons (5.11) and the bounce rule of 3.5: the three attract demos (homing missiles
in demo 1, star bullets in demo 2, multi warheads in demo 3; every frame of every special-weapon shot in slots 3-10 and every
object of slots 23-30 logged, 11,470 frames), plus harness runs on a level-0 start savestate with the weapon fitted by writing
RAM $D1, the gauge set full, the pod held still, objects poked into slots 15-30 and write hooks on the shot's fields.

| Subject | Run | Result |
|---|---|---|
| Homing missile and multi warhead, all fields every step (5.11.1) | demos 1, 3: 16 + 8 flights, 9,638 steps | 9,615 exact; 23 = bounces |
| Target looks: R, carry, candidate (5.11.1) | all demos, 684 looks | 0 differences |
| Star parent and children: speed rule, stage test, counter, hit points (5.11.2) | demo 2: 7 parents, 73 children, 147 stage changes; harness | 68 of 73 speeds exact (5 first seen away from the parent), all stage changes |
| Time bomb: counter phase, area, score (5.11.3) | harness, 420 frames, 7 objects | as written |
| Smart bomb: reach, score, cost (5.11.4) | harness, 2 runs; level 26 cores, 5 runs (gauge $FF, $81, $80, $40) | as written; Q32 |
| Damage: missile 2, multi $FF, star parent $50, child $10, bomb $40 (touch) | harness against a dome of 48 | as written |
| Bounce keeps state $12 / $14, ordinary shot becomes a spark (3.5) | harness against a closed turret | as written |

Still not run: the shot deflection by R2 (3.5), the damage numbers of 3.1 for shield-on cases of the later types, the
towing physics of the kinds with load class 0 and 2 against class 1 (the loads are `player.md` 8.3), the tractor beam
holding the fuel pod (kind 11), the flash script of the final level's cores, and a bonus object taken in by the beam
(not possible: pickups are not caught).


## Open questions

- The meaning of RAM $D2 bits (power-up state), $C9 and $6E, and the tractor-beam counting event; they belong to
  `player.md` / `economy.md`.
- Bank 2 entry $8027 is the pull-back of 6.1 (verified); only its purpose is unknown (Q23).
- The final-level background effect (4.16) is now followed (gate, check every 8 steps, spawn after 8, end after 20); what its
  nametable writes draw (the tile animation of the end structure) belongs with `scenes.md`.
- State $26 (bank 7 $8EFC) has a behaviour but nothing seen creates it in gameplay.

## Corrections for other specs

- `economy.md` 2.5 (small pickups) rewritten: touch box 13 px inclusive for the pod and Jetman, the beam does not catch them, no tag removal, the star's
  marker by touch (Q22). `economy.md` 2.1 has the per-kind results and the beam's catch boundary (`player.md` 8.1: |dy| at most 31).
- `objects.md` 10.2 (rings: 4 steps, not 8) and the pull-back (6.1 here, Q23).
- `objects.md` 3.5: an invisible object (frame $FF) is also tested on y, with −M instead of −W (2026-10-17).
- `levels.md` 6.5: RAM $0595 is not dead: per-level routines write it (levels 3, 5, 6 and 11 change the pod's gravity).

- `objects.md` 4.2 and open questions: types 01-10 are fixed gun turrets (4.1), not collectables; types 1F/20 are
  magnets (4.9); type 15 does spawn (seen on level 1, index 59); the egg 5F disappears after its fourth frame, nothing
  hatches (4.17); the countdown digits change every 64 steps (6.3), not every second.
- `objects.md` 2.1: $023E is the vertical sub-pixel of every slot; the horizontal sub-pixel is RAM $0197 + slot;
  work 5/6 per state are in section 7 here.
- `objects.md` open question on bank 4 $8057: used only by state $96, which nothing creates (5.9).
- `objects.md` 8: the collision sizes are used with the pod's own half-sizes (3.1) and with 2 for shots (3.4).
- `game-flow.md` 5: more readers and three more writers of RAM $5A (section 8). The comment "carry" in the
  generators' change means $5A += $61 exactly in practice.
- `sound.md` 8.3: $08 = an enemy fires; $16 = the pod bumps an object that survives; $18 = a drone is launched; $0E =
  a shot is deflected; $42 = a shot collects a bonus object.
- `levels.md` 6.4 open question: no object other than the player uses the terrain test.
- `economy.md` 3.3 and its open questions: warp levels have no damaging object of any kind (surveyed, `economy.md` 5.4).
- `objects.md` 4.1 (rows 4F-5E, "drifting", "home position") and the level 26 paragraphs of its 10.5: the objects of
  level 26 chase the pod (types 50-5D) or animate in place (5E); see 4.16 here. The list of level 26 holds 47
  objects of types 50-5E.
- `levels.md` Appendix A: how to run level 26 with its route (4.16 here) and why the poked savestates of levels
  13-28 have the wrong object table.
- `objects.md` 2.4 / `behaviour.md` 3.6: objects made by the shooting routines of 2.6-2.9 have hit points 0, not the
  state's table value.

