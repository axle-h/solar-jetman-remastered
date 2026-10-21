# The player: pod, Jetman, gun, tractor beam, death and respawn

How the player object (object slot 0) behaves in gameplay, per logic step: the jetpod's rotation, thrust, gravity
and collision response; Jetman outside the pod; the player states and every transition between them; the gun; the
tractor beam as far as it acts on the pod; death and respawn.

Status: **[V]** verified in Mesen (RAM per logic step compared with these rules), **[C]** read from the program,
**[H]** hypothesis. Addresses follow `README.md`.

Related specs: `levels.md` 6.4 (terrain test) and 6.5 (gravity, top of the planet, y clamp) are referred to, not
restated; `game-flow.md` (logic step, input bytes, transitions, RAM map); `objects.md` (object table, sprite
frames). Item values, prices and delivery are for the economy spec.

## 1. Conventions

- **Logic step** = one A pass plus one B pass (`game-flow.md` 1.2), 30 per second. All player logic runs in the A
  pass. "Per step" below always means per logic step; "frame" means a video frame (two per step).
- **Input** is RAM $18 (held) and $19 (newly pressed), read before the A pass: A = bit 7, B = bit 6, Select = bit 5,
  Up = bit 3, Down = bit 2, Left = bit 1, Right = bit 0.
- **Speeds** are unsigned 8.8 magnitudes in px per step with separate direction bits, as in `levels.md` 6.4:
  horizontal $02BA.$029B, direction $0336 bit 0 (1 = left); vertical $02F8.$02D9, direction $0355 bit 0
  (1 = down). Written "1.80" = 1 + $80/256 px per step. Accelerations are in 1/256 px per step, per step.
- **Sub-pixel positions:** x fraction RAM $0197 (slot 0; $0197 + slot for others), y fraction RAM $023E.
- **Directions and angles:** the pod's facing is its sprite frame RAM $0374, 0-31, 1/32 turn each, clockwise from
  0 = nose up (8 = right, 16 = down, 24 = left). Movement angles (RAM $9C) are bytes in 1/256 turn, same origin
  and sense (0 = up, $40 = right, $80 = down, $C0 = left).
- **Fuel** RAM $C7 (high) : $C6 (low), 16-bit, full = $E000; "1 unit" in this file = 1 in the 16-bit value; `economy.md` and `behaviour.md` count the gauge's high byte, 1 of those = 256 units here (so damage "$40" there is $4000 here, 2/7 of $E000). It is
  also the pod's and Jetman's hit points. Gauge, refills, consumption table and damage from objects:
  `economy.md` 3; this spec gives the rules where they touch the physics.
- **Weapon energy** RAM $C9 (high) : $C8 (low), the W gauge (`economy.md` 3.1): spent by fitted weapons and
  anti-gravity.

## 2. RAM used by the player

| RAM | Meaning | Status |
|---|---|---|
| $0317 | player state (slot 0), section 3 | V |
| $0374 | slot 0 frame: pod facing 0-31, Jetman pose $25-$2A, $FF hidden | V |
| $0393 | slot 0 timer: Jetman's shooting-pose timer; death timer in state 5; particle counter in state 7 | V |
| $03B2 | slot 0 counter of the explosion / assembly animation (states 4 and 8) | V |
| $C7:$C6 | fuel (section 1) | V |
| $C9:$C8 | weapon energy | C |
| $C4, $C5 | 8-bit sub-fraction accumulators for horizontal and vertical thrust (4.6) | V |
| $9C | movement angle of the player at the start of the step, 1/256 turn (5.1) | V |
| $99 | hit-pause counter: while non-zero the terrain does no damage; counts down by 1 per step (5.4) | V |
| $97 | invulnerability timer after emerging, assembling or respawning (5.4) | V |
| $65 | bit 7: shield on | V |
| $66 | slot of the object held by the tractor beam, 0 = none | C |
| $67 | load class (0-2) of the towed object, "mass" below | C |
| $68, $69 | rope angle: $69 = 0-127 in 1/128 turn (fraction in $68) | C |
| $6A-$6C | rope angular speed, 24-bit, whole part in $6C (0 or 1); $6D = its sense (0: angle increases) | C |
| $F1, $F2 | width and half-height used by the terrain test (`levels.md` 6.4 step 2) | V |
| $D1 | fitted weapon or device: shop item number + 1 (2 = anti-gravity), 0 = none | C |
| $D2 | bit 7: anti-gravity switched on | C |
| $D3 | fitted engine/shield item: shop item number (8 super shields, 9 momentum killer, 10 efficient engines, 11 double strength thrusters), 0 = none | C |
| $DC | pod type (bits 0-1); bit 6 boosters owned; bit 7 shields owned (`game-flow.md` 9) | V |
| $EF | autofire countdown (8) | V |
| $050E | gun upgrade level; the shot's strength is this + 1 | C |

Item numbers are those of `text.md` 4.5.

## 3. Player states and transitions

| State | Meaning | Handler summary | Pause allowed |
|---|---|---|---|
| 0 | no player (other scenes) | nothing | no |
| 1 | the jetpod flying | section 4 | yes |
| 2 | Jetman flying with his jetpack | 6.1 | yes |
| 3 | Jetman standing or walking | 6.2 | yes |
| 4 | the pod exploding | 7.1 | no |
| 5 | Jetman killed | 7.2 | no |
| 6 | the player on the final level (Golden Warpship flight) | 9.1 | yes |
| 7 | respawn: a new pod materialising | 7.3 | no |
| 8 | Jetman assembling a new pod he found | 6.3 | no |
| 9 | being drawn through a level-change gate | 9.2 | no |
| 10 | the player during the backdrop interlude (transition 7) | 9.3 | no |

Pause table: bank 0 $BE0E (10 bytes, states 0-9). [C], states 1-5 and 7 seen [V]

**Who runs the terrain test** (`levels.md` 6.4): the handlers of states 1, 2, 3 and 4 (state 8 shares state
4's handler), and the final-level handler of state 6 (9.1), which runs it at the end of its step, after shooting.
States 5, 7, 9 and 10 do not. **Who moves:** states 1, 2, 3 (horizontally only; vertically it is held on the
floor, 6.2) and 6 use the position update of 4.9; states 4 and 8 do not move (only 1 px up when their test
touches a floor or slope, 7.1); states 5 and 7 stay where the player died; state 9 is carried to the gate. [C]

Transitions (every one found):

| From | To | Trigger | Status |
|---|---|---|---|
| level start | 1 (6 on level 26) | the level is set up (`game-flow.md` 7.3) | V |
| 1 | 4 | fuel is 0 at the start of the pod's step (unless the towed object is being taken in by the mothership, RAM $0422 of it non-zero) | V |
| 4 | 2 | the explosion counter passes $12 (11 steps after the pod blew up) | V |
| 2 | 3 | soft landing on a flat floor (`levels.md` 6.4 step 3) | V |
| 3 | 2 | B held (take off), or no flat floor under him at the start of the step (walked off an edge or onto a slope) | V |
| 2, 3, 6 | 5 | fuel is 0 at the start of the step | V |
| 5 | (transition 4) | the death timer (96 steps) runs out | V |
| transition 4 | 7 | the level restarts after losing a life, lives remaining | V |
| 7 | 1 | the last materialising particle arrives, or the frame counter RAM $5D reaches 144 | V |
| 2, 3 | 8 | Jetman touches a spare jetpod lying on the planet (object kind 1, within 13 px on both axes) | C |
| 8 | 1 | the assembly counter passes $12 | C |
| 1, 2, 3 | dock (transition 2) | entering the mothership; for the pod only when the planet's flag allows it (`game-flow.md` 7.4) | C |
| 1, 2, 3 | 9 | a level-change gate (wormhole) is touched, the level-change request passes the transition guard and the gate is open (9.2.1) | V |
| 9 | previous state | the gate animation ends (9.2) | C |
| (transition 7) | 10 | the interlude scene starts | C |

## 4. The pod (state 1)

### 4.1 Steps run before every player state

Before the state's handler (all states except 0 and 10), in this order: [C], items 2 and 3 [V]

1. **Autofire.** If A is held and Select is not: when A was newly pressed, the countdown RAM $EF is set to the
   pod type's period (bank 0 $BE39: 6, 4, 2 steps for pod types 0, 1, 2); otherwise it counts down and on reaching
   0 the A press is repeated and the countdown restarts. In RAM terms: RAM $18 is the held-buttons byte and RAM $19
   the newly-pressed byte (A = bit 7, Select = bit 5, Start = bit 4); the test is $18 AND $A0 = $80 (A held, Select
   not); a new press ($19 bit 7 set) only reloads the countdown; the repeat sets bit 7 of $19. The countdown is
   decremented once per step and not on the step of a new press, so shots come at the press and then every
   period steps. On warp levels (RAM $9B bit 7) A is cleared from both input bytes instead: no shooting there.
   [C]
2. **Movement angle.** RAM $9C is computed from the current speeds (5.1). It is computed before anything moves,
   so it describes the movement of the previous step.
3. The position low bytes are stored for the camera (`levels.md` 5.2).
4. **Above y = 16 rescue:** if y < 16 and the last terrain test touched nothing, the vertical direction becomes
   down and the vertical speed 0.20. (The position update never lets the player above 16, so this is an edge
   case.)
5. **Anti-gravity drain:** while RAM $D2 bit 7 is set, weapon energy −$10 per step; when it runs out it stays
   at 0 and anti-gravity switches off.
6. **Invulnerability:** while $97 is non-zero it counts down by 1 and $99 is set to `($5D AND 7) + 1`, so the hit
   pause never runs out meanwhile.

After the handler, if an object is being towed, its horizontal direction flags are cleared.

### 4.1a Pod state at the start of play [C]

The routine that begins play (every launch, bank 0 $9000) sets the pod's facing to 0, the horizontal speed to 0,
the vertical speed to 1.80 (1.5 px per step) with the vertical direction up (RAM $0355 = 0) and clears the shield
byte $65 (shield off). Gravity and the first thrust then act on that speed.

**The pod's facing across level changes [V]** (round 13, Mesen; the frame byte RAM $0374 of slot 0 logged every frame):
- **After a wormhole gate (9.2.3): 0.** The gate's own picture overwrites the facing every step (frames 4, 4, 0, 0, ...), and the
  last gate step leaves 0, so whatever the facing was before the gate is lost; the next level starts with facing 0 (checked
  at gate phases that differed by two frames: always 0).
- **After the warp-level countdown (transition 6, the badge screen, `economy.md` 5.4): kept.** The pod was turned (left held)
  to facing 4 on level 29; the countdown was run out; after the badge screen and the load of the next level the first
  gameplay frame had facing 4 again. The begin-play routine above is not run on that path, which has no launch.
  (An earlier test of the gate with a nonzero facing before the gate was not possible: the poked-level start begins in the gate state.)

### 4.2 Order of the pod's step

1. Fuel 0: the pod is destroyed (7.1); nothing else this step.
2. Shield switching (4.7).
3. Momentum killer (4.7).
4. Gravity, including the towing pull (`levels.md` 6.5; 8.3 here), unless anti-gravity is on (4.7).
5. Speed cap: a speed whose whole part is 4 or more becomes exactly 4.00, on each axis.
6. Position update (4.9): x first, then y. The y clamp at 16 (`levels.md` 6.5) and the map bottom (below) are
   part of it. x is not limited.
7. Terrain test and responses (`levels.md` 6.4; section 5 here).
8. Rotation (4.3).
9. Thrust, if B is held (4.4-4.6).
10. Rope swing and placing of the towed object (8.2).
11. Exhaust flame and engine sound (4.8).
12. Shield upkeep (4.7): fuel −1, stopping at 0. With B held and the shield on, thrust (step 9) therefore takes its 1-2 first and the upkeep's 1 comes after it; with fuel 1 the thrust finds too little and gives no acceleration (4.5), and the upkeep then leaves 0; with fuel 2 the thrust works and the upkeep leaves 0. Step 1 of the next step sees the value after step 12, so the pod is destroyed then. **[H]**
13. Shooting, if A is newly pressed (section 10).

So thrust and rotation take effect on the next step's movement: a step's displacement uses the speeds after that
step's gravity and cap, and before its thrust. [V]

**Map bottom:** RAM $3A is the level header's map-height byte (header byte 9) shifted right by one (`levels.md` header table, byte 9); the remake uses that value, not the map height itself. If the y high byte would exceed RAM $3A, y becomes ($3A, $FF) and the vertical speed and y
fraction become 0. [C]

Towing (an item held by the tractor beam) changes the gravity and thrust steps: section 8.

### 4.3 Rotation

Left held: frame − 1; else Right held: frame + 1; modulo 32. One step of 11.25° per logic step (Left wins if both
are held). There is no rotation speed beyond this and no inertia. [V]

### 4.4 Thrust vector

The thrust is split into a vertical and a horizontal magnitude from one 16-byte table (bank 7 $8856, ROM 0x38856),
a cosine × 64:

`40 3E 3B 35 2D 23 18 0C 00 0C 18 23 2D 35 3B 3E`

- vertical magnitude `Tv = table[f AND 15]`, horizontal `Th = table[(f + 8) AND 15]`, f = facing frame;
- vertical direction: down for frames 9-23, up for the others; horizontal direction: left for frames 17-31,
  right for the others (bank 7 $8866, 32 bytes of 0/1). [V]

Acceleration per step from a table value T (all divisions round down): [C], cases 1 and 3 [V]

| Fitted / held | Acceleration |
|---|---|
| normal | (T / 2) / 2 |
| double strength thrusters (RAM $D3 = 11) | (T / 2) × 2 |
| boosters owned (RAM $DC bit 6) and Select held | T |
| boosters with Select, and double strength thrusters | T × 2 |

Example: facing 10, normal engine: vertical (24 / 2) / 2 = 6 down, horizontal ($3B / 2) / 2 = 14 right. [V]

### 4.5 Fuel used by thrust

Each step with B held, before accelerating, the fuel drops by: 2 units; 1 with efficient engines (RAM $D3 = 10);
16 with boosters and Select; 8 with boosters, Select and efficient engines. If the fuel is already 0, or this
takes it below 0 (it then becomes 0), there is no acceleration this step. [V] for 2 and 16

### 4.6 Applying the thrust

The acceleration of each axis (av, ah from 4.4) is applied to the current speed, with an 8-bit sub-fraction
accumulator per axis (vertical RAM $C5, horizontal RAM $C4). Each step the accumulator receives a carry-in byte:
vertical: the gravity value of this step (below); horizontal: see "Horizontal carry-in" below. Both are RAM bytes
shared with other code ($0C vertical, $0D horizontal), so the rules below say what they hold when the thrust runs.

**Vertical carry-in** ($0C), an unsigned byte [V]:

- Step 4 (gravity) stores the gravity value there: the header value or the top-of-planet value of `levels.md` 6.5,
  computed from the position **before** this step's move (it is stored even when anti-gravity skips the pull); when
  towing, 8.3 gives another value. The position update and the speed cap do not touch it.
- A **sloped-floor response** (`levels.md` 6.4 floors) sets it to 255. This overrides the gravity value for this
  step's thrust.
- A **sloped-ceiling response**, when no sloped-floor response happened this step, leaves there the whole part of
  the new horizontal speed of 5.2 before the clamp to 7 and before the halving (0-15), not the gravity value. If
  both happened, the 255 stays (the ceiling stage restores it). [C]
- Flat floors, flat ceilings and walls do not change it.

Vertical, every thrust step:

- Thrust direction equal to the current vertical direction: accumulator += carry-in; if it wraps past 255, av + 1.
  The speed grows by av; if its whole part reaches 3, it becomes exactly 3.00.
- Opposite: accumulator −= carry-in; if it wraps below 0, av + 1. The speed shrinks by av. If that reaches 0 or
  less, the leftover magnitude becomes the speed, the direction flips, the accumulator is set to the carry-in, and
  the speed grows once more by the same amount as the subtraction used, i.e. av plus the 1 from the borrow when
  the accumulator subtraction borrowed (capped as above). (A reversal therefore gains one extra step of
  acceleration.) [C]

Horizontal, only when the acceleration ah is not 0 (untowed: Th not 0, so facing 0 and 16 skip it; towing: the scaled ah, 8.3):

- Same direction: as vertical, with the cap at 3.00.
- Opposite: as vertical, but on reaching 0 or less the leftover becomes the speed, the direction flips and the
  accumulator is cleared; there is no second acceleration.

So thrust alone can never take either speed above 3.00 (the 4.00 cap of 4.2 only matters after collisions).
[V] except the vertical reversal, which is [C]

**Horizontal carry-in** ($0D), outside towing (towing: 8.3). It is not computed by the thrust: it is whatever the
last code before the thrust left in the shared byte. It is fully determined, as follows.
**During an attract demo the rules below do not apply** (except rule 1): the autopilot writes the waypoint's x high
byte there at the start of every A pass, ahead of the thrust (`game-flow.md` 7.1). [V] [V] on 1,400 thrust steps
of level 0 (no exception besides the listed ones), [C] for the cause

1. **Slope response this step** (sloped floor or ceiling, 5.2): the low byte (fraction) of the new horizontal speed
   before the halving, i.e. `((sin[s' AND $7F] × k) >> 3) AND $FF` of 5.2 (worked example there: $C8). If both a
   floor and a ceiling response happened, the later one (the ceiling's).
2. **Otherwise the parallax-star job of the step's second frame** (`graphics.md` 5.7.3, which owns the job). When
   the job runs and is on, it ends with this byte set to `floor(-b / 2)` as a signed byte, where b is the signed
   horizontal camera half-step of that frame (RAM $50, `levels.md` 5.2). The job runs after the camera in the
   B pass, so b is the B frame's half-step of the previous logic step (a positive b, camera moving right, gives $FF;
   b = 0 gives 0; b = +3 gives $FE; b = -1 gives 0; b = -2 or -3 gives 1). The A-pass call of the same job comes
   after the thrust and is overwritten before the next thrust, so it matters only through the stars' state.
   Nothing between that B pass and the thrust changes the byte.
3. **While the star job is off** (RAM $D5 = 0 and the window's top-left block outside chunk 0, `graphics.md`
   5.7.3), or on warp levels, or if the job does not run, the byte keeps what the **object placement scan** left in
   the B pass (`objects.md` 3.4; the scan runs in every B pass except in pod state 9 and on levels 29-41). The scan
   clears the byte at the start of its pass and, each time it reads a member of a group entry of the level's list
   (`objects.md` 3.2: the pairs `dx, dy`), loads it with that member's second byte, `dy` (a signed byte). So the carry-in
   is 0, or the `dy` of the last group member the pass read **[V for the value, C for "last member of the pass"]**. On
   level 0 the list has no group: 0. Elsewhere it is a fixed function of the scan's progress (up to 8 list entries per
   B pass), so of the step count: **[V]** level 6, star job off, 504 thrust steps: 322 had 0, 141 had `dy` bytes of the
   list's groups ($13, $2F, $DA in turn, the second bytes of three consecutive members), 41 had a non-zero value from a slope response (rule 1).
   **Read from the program (2026-10-17) [C], so the whole of rule 3 is now confirmed:** the scan (bank 4 $81E7, up to 8 list
   entries per call) clears this byte and its neighbour (RAM $0C, the member's dx) once at the start of its pass; while it
   is inside a group it loads the pair at each member read, dx into $0C and dy into $0D, and the loop that goes on to the
   next entry does not clear them again. Entries that are not group members do not touch them. So the thrust sees the dy of the
   last member read in the pass, or 0. The
   earlier wording, that the sound driver clears the byte every frame and so leaves 0, was wrong: the sound driver does
   not touch the byte, and the same value appears in the replay logs' "cc" column (that column is this byte, RAM $0D,
   not RAM $CC) as the periodic values $13, $2F, $DA, $D0 (all attract demos; the demo's thrust does not read them, the
   autopilot rewrites the byte first). The job is off at the start of a cold-booted game
   until the window's block first lies in chunk 0 (open sky, RAM $D4 = 0), so on level 0 the first 72 steps
   of the launch give 0; it switches off again when the window's block has left chunk 0 and all stars have slid off
   the screen. The mode survives level changes and respawns.

The remake follows the job exactly (`graphics.md` 5.7.3); the only approximation is the load-dependent A-pass skip
(`docs/deviations.md`). [C] for the cause, [V] for the byte.

**Worked example** [V] (level 0, gravity value 8, facing 0, normal engine, from 1.1A up): each step with B held:
gravity −2 (1.18 up), cap, move 1.18 up, then thrust +16 (1.28 up); the vertical accumulator grows by 8 each step
and adds 1 to the acceleration every 32nd step; fuel −2 per step.

### 4.7 Equipment that changes the physics

- **Shield** (needs RAM $DC bit 7, not on warp levels): Down newly pressed switches it on (RAM $65 = $80). Up
  newly pressed switches it off; so do the pod's destruction and a new pod taking over. While it is on:
  - fuel −1 unit per step; [V]
  - the terrain test's width and half-height grow (`levels.md` 6.4 step 2: $F1 up to 12 and $F2 up to 12; the object contact test of `behaviour.md` 3.1 uses the integer halves, 3 growing to 6); switching off sets both back to 6;
    [V]
  - terrain impacts cost less and start no hit pause (5.4);
  - nothing can be towed: any towed object is released each step, and none is caught (8.1);
  - slot 2 shows the shield ring at the pod's position (4.7a).
- **Down newly pressed** always releases a towed object (8.1), shield or not. [C]
- **Momentum killer** (RAM $D3 = 9): on every step B is not held, both speeds shrink by 0.20 towards 0 (they stop
  at 0, no reversal). The pod's palette flashes: palette 2 on two steps in four (steps 3, 4, 7, 8, ... since the level start), palette 0 on the
  others and on every step with B held; the exact rule and its interaction with the hit pause is `game-flow.md` 9.2, "Bits 0-1". [C], flash [V]
- **Anti-gravity** (RAM $D1 = 2): Select held with A newly pressed toggles RAM $D2 bit 7 (and spends special
  fuel as for a weapon shot). While it is on, the pod's gravity step (including the towing pull) is skipped and
  weapon energy drains by $10 per step (4.1). If the fitted device is not anti-gravity, the bit is cleared
  instead. While it is on (and $D1 = 2) the pod's palette bits go through all four palettes, one per step: bits 0-1 = (frame counter >> 1) AND 3 =
  (k - 1) mod 4 for the k-th step since the level start (`game-flow.md` 9.2, "Bits 0-1"; **[V]** 140 of 140 free steps of `antigravity.replay`).
  Pressing A + Select (on or off) clears them on that step. [C], flash [V]
- **Super shields** (RAM $D3 = 8): shielded impacts cost half again (5.4). [C]
- **Efficient engines**, **double strength thrusters**, **boosters**: 4.4 and 4.5.
- The pod's destruction clears RAM $D1, $D3 and $D2 bit 7: fitted items are lost with the pod. [C]

### 4.7a Slot 2 as the shield ring: how it is drawn **[V]** (2026-10-20, `shield`, `super-shields`, `momentum-killer` replays and 20 level replays with the shield on, 1,159 frames each in the first three; OAM read back)

- **Created / refreshed:** every pod step (A pass, in the pod's step after the move and the terrain test, i.e. at the
  pod's final position of the step, and after the shield's fuel upkeep) while RAM $65 bit 7 is set. Each such step writes
  slot 2 completely: state $0C (a picture only; its behaviour does nothing), position = the pod's (exactly equal, 1,159 of
  1,159 frames; no offset, no camera), frame as below. So it appears **in the very step Down is pressed** (the B pass of
  that step draws it) and is drawn at the pod's position of the step it is made in.
- **Flags:** while the shield is on, flags X and flags Y of slot 2 are set to 0 on every step the shield is on (when
  Down is newly pressed, and on every other step: the shield bookkeeping of the pod step clears them before the ring is
  made). So no mirror, no flip, the layouts' own attributes. Attribute bit 5 (behind the background) is added from
  RAM $98 bit 5 (OAM showed $81 to $A1, $C1 to $E1, $01 to $21, $41 to $61 while the pod was in the bay at the start of
  `shield.replay`), bit 4 is set (no effect); palette 1 from the layout; RAM $98 bits 0-1 (flash) are not copied.
- **Frame:** $69 + ((c >> s) AND 3) with c the frame counter RAM $5D at the start of the A pass (even; the same
  counter as 4.8) and s = 1 normally, s = 2 with super shields (RAM $D3 = 8; the momentum killer, $D3 = 9, uses s = 1).
  So the ring advances one frame per step and shows each of $69, $6A, $6B, $6C (a 4-step cycle) normally, and each frame for
  two steps (an 8-step cycle) with super shields. The cycle is tied to the absolute counter, not to the moment the shield
  went on: the first frame after switching on is $69 + ((c >> s) AND 3) for that step's c. Observed: normal 6C 69 6A 6B ...
  (first step c = 6), super 6A 6B 6B 6C 6C 69 69 ... (c = 6, 8, 10, ...). **[V]** 580 of 580 steps in each replay,
  also 3,915 steps in `level1`, 321 in `pod-nippon`.
- **Layouts** (table B, frames $69-$6C, tiles $CA-$CE of the static common set, `graphics.md` 6.1, the same pictures as
  the pick-up sparkle): four sprites each, around the pod point at radius about 12 px, e.g. $69 at (0,+12), (-12,0), (+12,0),
  (0,-12) and $6B at (+-8, +-9); they are ordinary layouts with count 4 (`objects.md` 6.2). The rotation is by frame only.
- **Freed:** by Up newly pressed (immediately, in the player step, so that step's picture has no ring), by the pod's
  destruction (7.1, same step), and by anything that clears every slot (level change, respawn, 7.3); Down while the
  shield is on keeps it. Fuel 0 destroys the pod first (7.1). A shield cannot be on in Jetman, on a warp level or while
  the level-change gate runs (9.2.1 needs the shield off), so the ring never exists there.
- **Hit pause (5.4) and blink:** shielded impacts start no hit pause; the ring is made every step regardless of the
  pod's own flashing or hiding.
- **Order:** drawn once per step in the B pass like every object; slot 2 (like slot 1) sits before the other objects in the
  build that runs from slot 1 upward and after them in the other build, behind the pod (OAM 1-4 are the pod's)
  (`objects.md` 6.6).
- **Interaction with the beam:** nothing can be caught while the shield is on (8.1) and turning it on releases a tow (the
  item moves 3 px toward the pod's side, 8.1) in the same step; the ring is written after that, so the beam's slot-2 state
  (9) is overwritten by $0C and the beam is not drawn on that step.

### 4.8 Exhaust flame and engine sound

Run once per pod step after the thrust (and in Jetman's flying step, see below). Slot 1 is the flame. `f` is the
pod's frame (0-31, `objects.md`), `c` the frame counter RAM $5D (+1 per frame; read in the A pass, i.e. even).
[C] except where marked; the sound request pattern and the table are [V] against the code and a short run.

**Placement and look (every step the flame is drawn or updating).**

- Position: pod position + (off[f + 8], off[f]) (the 40-byte `off` table listed under the shot in the gun section; it places the shot too). The flame is x-flipped when f is 16 or more, and v-flipped when (f − 8) AND 31 is below 16
  (f in 8-23). It does not move on its own and has no speeds.
- Base frame (table B, `objects.md` 6.4; bank 0 $CA93, ROM 0x04A93): flameBase[f AND 15] = `18 18 1B 1B 1E 1E 21 21 24 21 21 1E 1E 1B 1B 18`
  (hex). The shown frame is the base frame plus an offset `d`.
- The slot's state is 7 (a picture only) and its work byte $03B2 holds `d`.
- Its layouts (frames $18-$37, and $10 for Jetman) have count 0, a single sprite: it is drawn at exactly the slot's position, without the drawer's usual +1 pixel in x (`objects.md` 6.2, `behaviour.md` Q35). **[V]** 2026-10-20.
- Next to these tables (bank 0 $CAA3-$CAC2, ROM 0x04AA3) lies a 32-byte ramp, $E0 minus 9 per entry for 25 entries ($E0, $D7, ... $08), then 7 zeros. No code reads it; the remake does not need it. [C]

**B held.**

- Offset d: with boosters owned (RAM $DC bit 6) **and** Select held: d = pattern[(c AND $0E) / 2] with pattern =
  0, 1, 2, 1, 0, 1, 2, 1 (bank 0 $CAEB, ROM 0x04AEB; it changes every 2 frames, a 16-frame cycle). Otherwise d = 2 when c AND 2 is 0 and 1
  when it is 2 (an alternation every 2 frames, a 4-frame cycle).
- Engine sound: effect $10 is requested (through the second request pair, `sound.md`) on a step when boosters owned
  and Select held, or otherwise when c AND 2 = 0. (Boosters owned, Select not held: the 4-frame rule.)

**B not held.** If the slot is in use: the flame keeps its position and base frame (recomputed each step); each
time c AND 7 = 0 the stored d increases by 1, and when it reaches 3 the slot is freed and d cleared. In between,
the shown frame is the base frame plus the stored d. So the flame lingers after B is released:
with the last d = 2 it is freed at the next boundary (0-7 frames), with d = 1 after two boundaries, with d = 0
after three (up to 23 frames); it shows base + d, then base + d + 1 ... as d grows. If the slot is free nothing happens.

**Jetman (any state other than the pod's state 1).** The same routine with frame $10 (a small flame), placed at
y + 5 and at x ± 5 (right of him when his facing bit says so: RAM $0336 bit 0, taking bit 1 instead when bit 2
is set), x-flip equal to that bit, no v-flip, and no sound. Stages and fading as above.

**Slot 1 in use by something that is not the flame [V]** (2026-10-20, `respawn.replay`; the program's flame routine is called every step of Jetman's
flying and walking states and of the pod, and tests only whether the slot's state is not 0, never what it is): with B not held the routine
treats a live slot 1 as its flame, whatever object it holds. The pod's explosion (7.1) is the case that occurs (section 7.4 gives the whole timeline).
Its effect on the slot: position and flags are overwritten as above (the explosion jumps to the flame's place), the shown frame becomes $10 + the
work byte $03B2 of the slot (for the explosion that is its sequence position, $0C-$0F) and then, in the same step, the objects' step of slot 1 may
write the explosion's own frame over it; the state stays 5. Every step with $5D AND 7 = 0 (one A pass in four) the work byte is increased
and, when it reaches 3, the slot is freed (state 0) and the work byte set to 0 — for the explosion, whose work byte is $0B or more, that is the first such
step. With B held the routine instead makes the flame **over** whatever is in slot 1 (state 7, frame, position, flags and work byte all rewritten), so an explosion still
running there is replaced by the flame at once.

### 4.9 Position update

Common to every state that moves the player (pod, Jetman flying and walking, the final level; objects use the same
rule). Speeds are magnitudes with a direction bit, never signed. [V]

- **x:** the fraction is RAM $0197 (slot 0; $0197 + slot for other objects). Moving right: (x, fraction) += (whole,
  fraction of the speed) as one 24-bit addition (16-bit x, 8-bit fraction); moving left: −=. Example: 2.80 px per
  step to the right from x fraction $40 advances x by 2, 3, 2, 3, ... (fraction $C0, $40, ...).
- **y:** the fraction is RAM $023E (the same for other slots). Moving down: +=; moving up: −=. Then, for the
  player only: if the result is above y = 16 (high byte 0, low byte < 16) y becomes 16 and the vertical speed 0
  (the y fraction is kept); if the high byte went negative, y and the vertical speed become 0; the map bottom
  clamp of 4.2.
- x first, then y, once per step, at the place each state's step order gives (4.2 for the pod, 6.1-6.2 for
  Jetman). The fractions are never cleared by collisions (only by the map-bottom clamp).

## 5. Collision response

The terrain test itself, its order, the flat-floor bounce and landing and the wall pushes are in `levels.md` 6.4.
This section defines the parts that spec leaves to the player.

### 5.1 The movement angle (RAM $9C)

Computed at the start of every step from the speeds (h, v as 16-bit magnitudes, with their direction bits). [V]
(1305 consecutive steps matched)

1. Shift h and v left together, one bit at a time, at least once and at most 8 times, stopping as soon as bit 11
   of either is set. Keep only the high bytes H and V (each 0-15).
2. If V = 0: angle = $40 (right) or $C0 (left). Else if H = 0: angle = 0 (up) or $80 (down).
3. Otherwise take the angle `e` from the major axis (in 1/256 turn, 0-$20):
   - H = V: e = $20;
   - H > V: e = `atan[tri[H] + V] / 2`; if moving down, e = $40 − e;
   - H < V: e = `atan[tri[V] + H] / 2`; if moving up, e = $40 − e.
4. By quadrant: right-up: $40 − e; right-down: $80 − e; left-up: e + $C0; left-down: e + $80 (mod 256).

Tables (data, bank 7): `tri` at $8B98 (ROM 0x38B98), 16 bytes `00 01 03 06 0A 0F 15 1C 24 2D 37 42 4E 5B 69 78`
(n(n+1)/2); `atan` at $8B10 (ROM 0x38B10), 136 bytes:

```
3F 2A 3F 1E 34 3F 17 2A 39 3F 13 23 31 3B 3F 10 1E 2A 34 3D 3F 0E 1A 25 2E 37 3E 3F 0C 17 21 2A
32 39 3E 3F 0B 14 1E 26 2E 34 3B 3E 3F 0A 13 1B 23 2A 31 37 3B 3F 3F 09 11 19 20 27 2D 32 37 3D
3F 3F 08 10 17 1E 24 2A 30 34 39 3D 3F 3F 08 0F 15 1B 22 28 2D 31 35 3B 3E 3F 3F 07 0E 14 1A 20
25 2A 2E 34 37 3B 3E 3F 3F 07 0D 13 18 1E 23 28 2C 31 34 39 3B 3E 3F 3F 06 0C 12 17 1C 21 26 2A
2E 32 35 39 3D 3E 3F 3F
```

### 5.2 Slope response: the new speeds

`levels.md` 6.4 gives the new angle s' (stored in RAM $9C). The speeds are then rebuilt from it, keeping the
**sum** of the two magnitudes (not the Euclidean length): [V]

- S = h + v (8.8 sum); k = (S × 16) >> 8, kept to 8 bits (S in 1/16 px);
- new h = (sin[s' AND $7F] × k) >> 3, new v = (sin[($40 − s') AND $7F] × k) >> 3, both as 8.8 values (the result
  of `>> 3` is in 1/256 px); a whole part of 8 or more becomes 7;
- directions from the quadrant q = s' >> 6: q = 0 right-up, 1 right-down, 2 left-down, 3 left-up;
- then both are halved as `levels.md` describes (unless both whole parts are 0 and the fractions sum to $80 or
  less).

s' is written to RAM $9C at once, so a second slope response in the same step (floor, then ceiling) reflects about
s', not about the step's starting angle; the next step recomputes $9C from the speeds (5.1). [C]

`sin` is bank 7 $8C74 (ROM 0x38C74), 128 bytes (index 128, value 0, is the first byte of the next table), sin(πi/128) × 128 rounded:

```
00 03 06 09 0C 0F 12 15 18 1C 1F 22 25 28 2B 2E 30 33 36 39 3C 3F 41 44 47 49 4C 4E 51 53 55 58
5A 5C 5E 60 62 64 66 68 6A 6C 6D 6F 70 72 73 75 76 77 78 79 7A 7B 7C 7C 7D 7E 7E 7F 7F 7F 7F 7F
80 7F 7F 7F 7F 7F 7E 7E 7D 7C 7C 7B 7A 79 78 77 76 75 73 72 70 6F 6D 6C 6A 68 66 64 62 60 5E 5C
5A 58 55 53 51 4E 4C 49 47 44 41 3F 3C 39 36 33 30 2E 2B 28 25 22 1F 1C 18 15 12 0F 0C 09 06 03
```

**Square table.** The program's multiply helper (it yields the exact product of two bytes, so the remake just multiplies) looks up
squares: n² for n = 0-255 as 16 bits, low bytes at bank 7 $8CF4-$8DF3 (ROM 0x38CF4) and high bytes at $8DF4-$8EF3
(ROM 0x38DF4). Entry n is `n × n`; nothing else is stored. [C]

**Worked example** [V] (level 0): speeds 0.76 left, 1.67 down (after gravity); s = $8F; a direction-2 floor,
a = $EB. s' = $EB + clamp(signed($EB − $8F − $80)) = $EB + ($DC) = $C7. S = $01DD, k = $1D. New h = ($7E × $1D)
>> 3 = $01C8, new v = ($15 × $1D) >> 3 = $004C; q = 3: left, up. Halved: 0.E4 left, 0.26 up. Damage (5.3):
S × 2 = $03BA, × 4 = $0EE8 units: fuel $C830 → $B948, hit pause 48.

### 5.3 Impact damage

As in `levels.md` 6.4 "Impact damage". The amount is computed on the raw 16-bit speed value (whole × 256 +
fraction, i.e. in 1/256 px per step), kept to 16 bits: flat floors and flat ceilings: vertical speed (before
halving) × 4; walls: horizontal speed × 4; slopes: (h + v) × 2, the sum's carry out of 16 bits lost (it cannot
occur below 4.00 + 4.00). Nothing is taken when the
fuel is already 0, on warp levels, or while the hit pause $99 is non-zero. Otherwise: [V] for the unshielded
cases

- no shield: fuel −(amount × 4), and $99 = 48;
- shield: fuel −(amount / 2), or −(amount / 4) with super shields; $99 is not set;
- the fuel stops at 0. The pod is then destroyed at the start of its next step (3).

So an unshielded bounce costs 16 units per 1/256 px per step of speed: a flat-floor hit at 1.00 px per step costs
$1000, 1/14 of a full tank. Example [V]: vertical speed $2F → $02F0 units.

A pod left on flat ground never comes to rest: each bounce reverses and halves the vertical speed, gravity brings
it back, and it keeps hopping (on level 0 every 8 steps between 0.25 up and 0.4B down); damage is taken on each
bounce that falls outside a hit pause. [V]

### 5.4 Hit pause and invulnerability

- **$99, hit-pause counter** (the same byte `objects.md` 6.4 calls the pod flashing counter): set to 48 by
  unshielded terrain damage (and by unshielded hits from objects, `economy.md` 3.3); −1 per **logic step**, so 48
  steps = 96 frames. The countdown is in the pod's drawing stage at the end of the step, after the terrain test
  and the thrust: a hit that sets 48 in step n leaves 47 at the start of step n+1, and the test of the same step
  already saw the 48. [V] The first 4 steps ($99 ≥ $2C) show the spark pictures, then the pod flickers. While non-zero: no terrain damage, no
  impact sparks (the sound still plays), and the pod is drawn flashing (`objects.md` 6.4). Objects' hits neither set it nor are
  blocked by it (a mine took $4000 units at $99 = $30, $2F, $2E ... on consecutive steps). [V]
- **What the picture shows [V]** (`tow.replay`): the pod's palette bit 0 (RAM $98) is written in the drawing stage of the step from the value $99 has
  *after* the step's decrement: palette 1 while that value is $1F or more (a hit in step n shows palette 1 in steps n to n + 16, the step that
  leaves $1F being the last), palette 0 from the step that leaves $1E. Details and the other palette bits: `game-flow.md` 9.2, "Bits 0-1".
- **$97, invulnerability:** set to 96 when Jetman emerges from the explosion and when he assembles a new pod, to
  128 (kept there) during respawn, and to 64 after a gate. While non-zero it keeps $99 above 0 (4.1), and the
  impact effect is skipped completely. [V] for the emerge case

### 5.5 Impact effect

Run on every terrain bounce, wall hit and slope response of the player (`levels.md` 6.4): [C]

- Sound: effect $02 on pulse 1 and $04 on pulse 2 (`sound.md` 8.3), unless $97 is non-zero.
- Unless $97 or $99 is non-zero: up to three sparks, each in the highest free slot of 3-10, at the player's
  position, state 1, frame $62 (table B), with fixed speeds (1.80, 1.40), (0.40, 1.C0), (1.80, 0.40) (horizontal,
  vertical) and random directions: two right rotations of the random byte RAM $5A per spark (this changes the
  generator's state, `game-flow.md` 5). The spark's horizontal direction (1 = left) is bit 0 of $5A after the
  first rotation (the bit the second rotation then shifts out), its vertical direction (1 = down) is bit 0 of $5A
  after the second rotation. Their life RAM $0393 is (8 − whole h − whole v of the player) × 16, taken from the
  speeds at the moment of the impact effect (before the bounce's halving). The slot's other fields keep what the
  previous occupant left: hit points, the position fractions and the work bytes are not written (`objects.md` 2.2). [C]; the direction bits are consistent with the black-box fit of the objects implementer.
  - **Where the sparks start.** At the player's position at the moment the effect runs, which depends on the
    stage: flat floor, and the sloped floors and ceilings: after the position correction of that hit; wall hits
    (both directions) and flat ceilings: **before** the correction (the correction follows the effect). [C]
  - **Slots.** The first spark takes slot 10 if it is free (state 0), else the next lower free slot; each later
    spark continues downward from the slot after the previous one; the search stops at slot 3 (sparks that find no
    slot are not made, and then no more bits are rotated). Slots held by the pod's shots count as occupied. [C]
  - **Rotation carry.** `ROR` brings the processor carry into bit 7 of $5A. The carry entering each spark's first
    rotation is: for the first spark, bit 4 of (8 − whole h − whole v) as a byte (0 for any real speed) when slot
    10 was free, else 1; for the second and third spark, 1. (The second rotation of a spark takes the bit the first
    one shifted out.) [C]
  - **Life.** A spark's life counts up by 1 per logic step (not per frame) and the spark is freed when it reaches
    144 (`behaviour.md` 6.4). [V]

### 5.6 Hits by objects

Rules, amounts and Mesen checks in `behaviour.md` 3.1: contact never sets $99 or $97, is not blocked by $99, and repeats on every step the object still overlaps. The pod's whole reaction is half of the object's speed added on the pushed axis (and its direction set away); an unshielded touch by an enemy shot empties the tank, a shielded one costs $0800 units. The earlier one-off observation (−$4000, vertical speed 0.41 down) fits this push; it is not a separate rule. The terrain routine's x4 and 48-step pause are not used.

## 6. Jetman outside the pod

### 6.1 Flying (state 2)

Per step, in this order: [V] except where marked

1. Fuel 0: killed (7.2).
2. Shooting-pose timer $0393 − 1 (if non-zero).
3. Gravity with the fixed value 49 (a = 12 per step, cap 3.00; `levels.md` 6.5), cap 3.00 on each speed, position
   update, then **friction**: the horizontal speed shrinks by 0.02 (stops at 0).
4. Terrain test (`levels.md` 6.4, half-height normally 9). If it touched a sloped floor but no flat floor, and the
   whole vertical speed is 0: the vertical speed becomes 1.80, and moving up if the slope bits 1-4 of RAM $DD are
   set. [C]
5. Left held: facing left. Moving right: speed − 0.38, reversing to left through 0; moving left: speed + 0.28 (cap
   7.00). Right held (if not Left): the mirror image.
6. Down held: pose $2A (unless the shooting pose is on). Moving up: speed − 0.1C, stopping at 0 and then turning
   down; moving down: speed + 0.1A (cap 3.00).
   Otherwise B held (the jetpack): pose $29. Moving down: speed − 0.20, reversing through 0, and on reversal +0.30
   at once; moving up: speed + 0.30 (cap 3.00).
   Neither: pose $2A moving down, $29 moving up.
7. A newly pressed: pose $28 for 8 steps, and a shot (10).
8. Exhaust flame (as 4.8, B held).

Net jetpack climb: 0.30 − 0.0C = 0.24 per step. Example [V]: falling 0.0C down, B held: 0.0C + 0.0C − 0.20 → 0.08
up, + 0.30 → 0.38 up.

### 6.2 Standing and walking (state 3)

Per step: [V] except where marked

1. Fuel 0: killed (7.2).
2. If the facing flags of $0336 (bits 1-2) are set, the movement direction is set to the facing.
3. The vertical direction becomes down, the vertical speed 1.00, and y + 1; then the terrain test. Normally the
   flat floor catches him again: a soft landing, which sets only the **vertical** speed to 0 (the horizontal speed
   is kept; see the correction to `levels.md` in section 12).
4. If no flat floor was touched: state 2, pose $2A, vertical speed at least 1.00; flame; end of step.
5. Position update (horizontal only, the vertical speed is 0).
6. B held: take off: state 2, vertical speed 0.40 up; end of step.
7. Left held: moving right: speed − 0.28, reversing through 0. Moving left: if the whole part is 0, speed + 0.13,
   and once the whole part becomes 1 the speed is exactly 1.00; at 1.00 it stays 1.00. Right held: the mirror
   image. Neither: speed − 0.20 per step, stopping at 0. Walking speed is therefore 1.00 px per step, reached in 14
   steps from rest.
8. A newly pressed: a shot (10).
9. Pose: $25, $26, $27, $26 by `(x >> 2) AND 3` (bank 0 $C45E): the walk cycle follows the position.
10. Exhaust flame.

On a slope (direction 1-6) the terrain test's state-3 response (`levels.md` 6.4) pushes him down-slope at 1.xx or
2.xx; with no flat floor he becomes state 2 the same step. [V]

### 6.3 Finding a spare pod (state 8)

When Jetman (state 2 or 3) comes within 13 px on both axes of an object of kind 1 (a jetpod lying on the
planet), the object is removed, RAM $97 = 96, the fuel high byte = $E0, the pod sound ($3E on both pulses)
plays, Jetman is hidden and both speeds become 0, and the counter $03B2 = 0; state 8. [C]

Each step: if the counter is $12 or less, the explosion animation of 7.1 runs in reverse (the pod assembles), the
terrain test runs (half-height growing up to 9, y − 1 when a floor or slope is touched) and the counter += 2. Above
$12: state 1, facing 8 (right), jetpods RAM $3D + 1, width and half-height 6. [C]

**The pod's art comes back [V]** (`spare.replay`, built for this: Jetman touching a spare pod at step e): the pod set is written back over tiles $01-$12, two tiles per
step, the same way as the destruction took them (7.1): the pair of tiles 2j + 1, 2j + 2 is in CHR RAM in the picture of step e + 1 + j, j = 0..8
(states-8 steps e + 1 .. e + 9 upload, e + 10 idle, e + 11 the first state-1 step). The source is the pod set of the **current pod type** (RAM $DC AND 3: 0 → ROM 0x0CBC5, 1 or 3 →
0x0CFC5, 2 → 0x0D785, `graphics.md` 6.1; **[V]** for types 0 and 1: a run with RAM $DC = $81 restored the Nippon set from 0x0CFC5 although the level had loaded the plain set). The explosion
animation does **not** run in reverse in any slot: slots 1 and 2 stay empty during the assembly (**[V]**, only the tiles run backwards). A new pod after a death arrives by the
level reload instead, which loads the whole pod set again (`graphics.md` 5.7).

### 6.4 Docking

The pod, or Jetman, entering the mothership requests transition 2 (`game-flow.md` 7.4); Jetman docking gets a
jetpod back. The exact test (RAM $98 bit 5, the pod drawn behind the mothership) belongs to the game-flow/objects
specs. [C]

## 7. Death and respawn

### 7.1 The pod destroyed (state 1 → 4 → 2)

At the start of a pod step with fuel 0: [V] timing and values, [C] the rest

- If an object is towed: it is pushed 5 px outward along the rope (8.2) and released. **The only rope byte
  cleared is $66** (= 0); $67-$6D keep their values (8.3, "What persists"). Also: the item's four speed bytes are
  set to 0 and the beam (slot 2) is freed. Details of the test, [C]: (1) if the item's frame byte is 0 the pod is
  destroyed without the push (and $66 = 0); (2) **if the item is being taken in (taken-in flag $0422 + slot
  non-zero) the destruction does not happen at all**: the step continues as an ordinary pod step with fuel 0,
  and the test is repeated next step; (3) the pod's own speeds, both of them, are set to 0 and the frame to
  $FF, as below.
- Shield off (width and half-height 6, slot 2 cleared), fitted items cleared (4.7), anti-gravity off.
- State 4; fuel = $E000 (high byte $E0, low byte stays 0) for Jetman; jetpods RAM $3D − 1.
- Slot 1 becomes the explosion object (state 5, type $0B), the pod is hidden (frame $FF), both speeds 0, counter
  $03B2 = 0; sound $3E on both pulses.

State 4, each step: the counter selects the explosion picture (drawn by bank 1); the terrain test runs (no
movement; half-height grows to 9 while untouched; y − 1 when any floor or slope is touched); the counter += 2.
When the counter is above $12 (the 11th step): Jetman appears: state 2, pose $29, y − 2, RAM $97 = 96, palette
flag RAM $98 = 0. [V] 11 steps (22 frames)

**The launch (point 5) [V].** Jetman appears with both speeds 0; nothing in 7.1 or 6.1 gives him a launch. The
3.00-up start seen in `tow.replay` comes from the **object push on an invulnerable player** (`behaviour.md` 3.1,
"If the pod is invulnerable", now with the rule): on his first step ($97 = 96, frame $29 visible) Jetman overlapped
a mine (slot 28), and the vertical push set his vertical speed to ((0 + (y_player − y_object) AND 255) AND 3) =
$F3 AND 3 = 3, fraction 0, direction up. It depends on the geometry (0 to 3 in whole pixels), not on the tow; with
no object on him he starts at rest and falls. After it the usual 6.1 gravity applies (3.00 → 2.F4 → 2.E8 ...).

**Jetman's figure replaces the pod's tiles [V]** (CHR RAM read back at every step of `jetman`, `respawn`, `spare`): the pod's art occupies sprite tiles
$01-$40; while the pod is destroyed these first 18 tiles ($01-$12, 288 bytes) are overwritten, two tiles per step, by Jetman's own figure (ROM
1:$F185, 0x0F185, `graphics.md` 5.7.4). With step d the pod's destruction step (the step that sets state 4), the pair made of tiles 2j + 1 and 2j + 2
(j = 0..8, from the 18 tiles at 0x0F185 + 32 j) is in CHR RAM, i.e. visible, in the picture of step d + 1 + j: tiles $01-$02 in step d + 1,
..., $11-$12 in step d + 9. The state-4 steps are d + 1 .. d + 9 (counter 0, 2, ..., $10: one pair each), d + 10 (counter $12: nothing) and d + 11 (counter above $12:
state 2). Jetman's frames therefore use his own art from his first step (d + 11) on.

Hardware note: the original skips a state-4 step while its video upload buffer is busy (RAM $52 non-zero; for
state 8, RAM $53). The remake does not. [C]

The original also sets the video job phase RAM $60 to 9 when Jetman appears; $60 feeds the random generator
(`game-flow.md` 5), so a replay must do the same. [C]

### 7.2 Jetman killed (state 5)

At the start of a Jetman step (states 2, 3 or 6) with fuel 0: state 5, death timer $0393 = 96, gun upgrade RAM
$050E = 0; if the lives RAM $3E are 1 they become 0 (so the coming transition ends the game); the explosion object
and sound as in 7.1; Jetman hidden. Each step the timer − 1; at 0 transition 4 is requested. [V] 96 steps (192
frames) from death to the transition

### 7.3 Respawn (state 7)

Transition 4 with lives left (`game-flow.md` 7.4): sound effect $32 with the request lock (`sound.md` 8.2), state 7, particle counter $0393 = 7,
pod hidden, slots 3-10 cleared, and the level restarts with the player at the position where Jetman died. [V]
(position kept in the one run checked)

State 7, each step: RAM $97 = 128.

- If the frame counter RAM $5D (cleared at the level start) has reached 144: done (below).
- While the counter $0393 has bit 7 clear: when the slot `((RAM $5D >> 3) AND 7) + 3` is free, a particle is
  started in it (state 7, frame $37 table B, radius 112) and the counter − 1. That is one particle every 4 steps,
  8 in all (7 down to $FF).
- Each particle circles in towards the pod: its angular speed grows by $2E/256 per step, its radius shrinks by 3
  per step (bank 7 polar placement), and when the radius would go below 0 it disappears, the counter − 1 and the
  pod becomes visible (frame 0).
- With bit 7 set the pod flickers (hidden on steps where `($5D AND 6) = 0`); when the counter reaches $F7 (the 8th
  particle has arrived): done.
- Done: state 1, facing 0, fuel high byte $E0. The fuel is 0 during state 7. [V]

Measured: 133 frames (66.5 steps) from the start of play to state 1. [V]

### 7.4 Slots 1 and 2 from the pod's destruction to the new pod **[V]**

(2026-10-20, `respawn.replay` and `jetman.replay`, every step: the slots' bytes read at the end of each step; the program's writes watched.) Slot 2 (the shield
ring or the beam, 4.7a, 8.4) is freed by the destruction (7.1) and **stays empty** through Jetman's whole life and the new pod's arrival: Jetman has neither (the shield is
off, nothing can be towed). Everything below is slot 1, d = the step in which the pod is destroyed (fuel 0 at the step's start).

**Slot 1's flag bytes at the start of play [V]** (2026-10-20: `bounce`, `orb-held`, `wormhole-jetman` and all other replays, the program's writes to RAM $0337 and $0356 watched from power-on; pokes at a level change and a reload).
If the pod is destroyed before its flame was ever made, the explosion of the table row "d" below finds slot 1's **flag bytes as they were at the start of play: flags X (RAM $0337) = 1 and flags Y (RAM $0356) = 1** (not 0 and 1). Where they come from:
- Slot 1 is the title screen's helper object (`scenes.md` 2.2: the scroll homes on a target through object slot 1 for the 512 frames of the title's sub-state 0). The object is made at power-on frame 530 (flags cleared) and its
  direction bits flip as it swings around the target; the last homing step of sub-state 0 (power-on frame 1041) leaves **X = 1 (left) and Y = 1 (down)**, and nothing writes them again: the
  levels' set-up, the approach, the surface picture and the launch scene (frames 1,041 to about 2,500 in the replays) touch neither byte. In every replay Start is pressed at frame 1,300, after sub-state 0, so the first gameplay pass finds 1 and 1.
  (A Start pressed earlier would find whatever the helper's model of `scenes.md` 2.2 has at that frame; a title that restarts after a demo makes the object again and gives the same 1 and 1 after its 512 frames **[C]**.)
- **A level change, a reload and a respawn do not write them either [V]**: bytes poked to $55 / $66 during play were found unchanged at the first gameplay pass of the next level (transition 3, transition 5) and after a respawn (the level reload
  after Jetman's death). So at every first pass the flags are whatever the previous gameplay left in slot 1: the flame routine (4.8) and the objects' step write them (the flame: Jetman's facing bit and the vertical flip; an explosion: the values in the table below).
  After a respawn in `wormhole-jetman.replay` both were 0 (the last occupant, an explosion, had written 0).
- Nothing in the first A pass changes them before the objects' step; the flame routine writes them when B is first held.

| Steps | Player state | Slot 1 |
|---|---|---|
| ≤ d - 1 | 1 | the pod's flame if B was held lately (4.8), else free; its position, frame and flag bytes stay in the free slot (the "leftovers") |
| d | 1 → 4 | **explosion made** in slot 1 by the destruction (the large explosion of `objects.md` 2.5, sequence position 11): the slot's position is set to the **player's position** (pod x, y exactly, no offset), state 5, sequence position $03B2 = $0B, life $0393 = 1. Nothing else is written: the flag bytes (RAM $0337, $0356) and the other leftovers stay as the flame left them (in the run: flag Y $80, i.e. the vertical flip of a pod facing right, flag X 0; so the explosion is drawn with that flip). In the same step's objects' pass: life 1 → 0, reload to 4, frame := table entry 9, sequence position $0C |
| d + 1 .. d + 3 | 4 | explosion frame 9 until life 4, 3, 2, 1 at the ends of d, d + 1, d + 2, d + 3 (4 steps per frame, from d); frame $0A from d + 4 to d + 7; frame $0B from d + 8 to d + 11 |
| d + 11 | 4 → 2 | (the counter above $12: Jetman appears, 7.1.) The explosion is still at frame $0B with life 1 |
| d + 12 | 2 | Jetman's first flying step (state 2, y - 2, $97 = 96, $98 = 0). The flame routine of 4.8 runs (B not held) on the live slot 1: position := Jetman's x - 5 (left; + 5 when his facing bit says right) and y + 5, flags X := 0 (his facing bit), Y := 0, frame := $10 + $0E = $1E; then the explosion's own step runs: life 1 → 0, frame := $0C (table entry), sequence position $0F. End of step: state 5, frame $0C at the new position, life 4 |
| d + 13 | 2 | flame routine: frame := $10 + $0F = $1F; the explosion's life 4 → 3 only. End of step: state 5, frame $1F (the flame's, not an explosion frame), life 3 |
| d + 14 | 2 | the step has $5D AND 7 = 0: the flame routine raises the work byte to $10, which is 3 or more: **slot freed** (state 0, work byte 0); frame left $10, position (x - 5, y + 5 of Jetman), life 3 |
| d + 14 .. | 2, 3, 2 | free (state 0). The routine does nothing while the slot is free and B is not held. When B is held in the air: the flame (state 7, frame $10 + 1 or 2, 4.8) |
| Jetman's death step | 2 or 3 → 5 | explosion made exactly as at d (the same routine in bank 0, called by both deaths): position = Jetman's, state 5, sequence $0B, life 1, flags as the last user left them (here 0, 0), frames 9, $0A, $0B, $0C, $0D, $0E, $0F for 4 steps each (28 steps), then freed 4 steps after frame $0F appeared (frame $0F at death + 24 .. + 27, free from death + 28). Player state 5 does **not** call the flame routine, so nothing disturbs it |
| the level reload (new pod) | 7, then 1 | the reload leaves slot 1's leftovers as they were (state 0); the new pod arrives with slots 1 and 2 empty (its particles use slots 3-10, 7.3). The first flame (B held) writes state 7, position, frame $18-$37 + d, flags, work byte as 4.8, and leaves the other bytes (life, hit points) as they were |

The timeline in the run: d = 367. Explosion frames 9 (steps 367-370), $0A (371-374), $0B (375-378), at 379 state 5 at (x - 5, y + 5) = ($0324, $00DA) with frame $0C, at 380 state 5 frame $1F, free from 381 to 436
(Jetman flying from 378, walking from 403); Jetman's death at 437: state 5 at his position ($032C, $00E3) with frames 9-$0F from 437 to 464, free from 465.
Which step the freeing falls on depends on the frame counter: the flame routine frees the slot at the first step, from d + 12 on, that has $5D AND 7 = 0
(the A pass counter is even, so one step in four; in the run the counter is $F4, $F6, $F8 at d + 12, d + 13, d + 14); up to three steps with the explosion hijacked (position and frame as above) are possible before that. **[V]** for the run; the phase rule **[C]**.
The hardware skip of 7.1 (state-4 steps skipped while the video upload buffer is busy) does not apply to the remake and does not change the slot's timeline above.

## 8. Tractor beam and towing

### 8.1 Catching and releasing

Each step, in the object loop (not the player handler), for item objects in slots 11-14 (state ≥ $0A, not being
taken in by the mothership): [C]

- dx, dy = the item's position − the pod's; it is caught when |dx| < 32, |dy| ≤ `rope[32 + |dx|]` (the table of
  8.2, a circle of radius about 32), and: the player is the pod (state 1), the shield is off, nothing is towed,
  Down is not held, and the level is not a warp level. Some kinds are not caught but touched instead (kinds 1 and
  $11 depending on the pod type bits, kinds 2, $12, $14, $1A, $1B: economy spec).
- On catching: RAM $66 = the item's slot, the rope angle $69 is set as below, the swing speed is 0, the load class $67 = `mass[kind]` (bank 0 $A534 + kind), slot 2 shows the beam (8.4: state 9 at the pod, frame $38, flags untouched),
  and sounds $12/$14 play.

  `mass` for kinds 1-19: `02 00 01 01 01 02 02 00 01 00 01 01 00 00 00 00 02 00 02`.

  **The catch angle** [V]. With dx, dy as above, v = |dx| (0-31; the catch needs v < 32, and dx = −256 is refused) and
  `r` = the **largest index r from 0 to 31 with rope[r] ≤ v** (a search down the table of 8.2 from index v: `rope[v]` is
  always larger than v for v ≥ 1, so the search steps down to the first entry that is at most v). The angle is
  by the signs: dx ≥ 0 and dy < 0 (item right of the pod and above it): `r`; dx ≥ 0 and dy ≥ 0: `64 − r`; dx < 0
  and dy ≥ 0: `64 + r`; dx < 0 and dy < 0: `128 − r`. **dy is not used for the angle, only for its sign** (and for the
  catch test). The result is a staircase in |dx|, not the nearest point of the table nor an arctangent: many dx give
  the same angle (v = 12 and 13 both give r = 8, v = 9 and 10 both give r = 6), and the item is then moved to the table's point for that angle
  at the next step (8.2), which is why a caught item jumps by a pixel or two. Example (demo 1, step 698):
  item (3,257, 232), pod (3,245, 204): dx = 12, dy = 28: `rope[8]` = 12 is the last entry ≤ 12, r = 8, angle
  `64 − 8` = 56 = $38 as the original has. Reading the table backwards from the geometry gives a different angle: the
  table point nearest to (12, 28) is angle 55 (offset (12, 29)) and atan2 gives 55.75, which is how a model comes out one
  too low. All 13 catches in the replay logs (demos 1-3, tow, delivery, items) match the rule above, including this one.
- **Releasing:** Down newly pressed, the shield on, or the pod destroyed. The item is moved by d pixels on each
  axis (d = 3 for Down or shield, 5 when the pod is destroyed): x by −d if the rope angle is below 64, else +d;
  y by +d if (angle + 32) AND 127 is below 64, else −d (so each axis moves toward the pod's side of the item); $66
  becomes 0 and **all four speed bytes of the item are set to 0**. The released item does not fly on with the
  swing's speed and does not take the pod's speed: it starts at rest. [V] (Down released at a fast swing:
  speeds 0.3C / 0.F0 one step before, 0 after.)
- **No release while the item is taken in** (its taken-in flag RAM $0422 + slot is non-zero, `economy.md` 2.2) **[C]
  [V]** 2026-10-20. The one routine that releases (Down newly pressed, shield on, pod destroyed) starts by testing that flag and, when it is set, **does nothing at all**: no move, no speed zeroing, $66 stays
  the slot, the beam is not freed by it. The rest of the Down/shield code still runs: Down with the shield fitted
  still switches the shield on ($65 = $80, RAM $0338 and $0357 cleared), and a shield that is on (every step) calls the
  release each step with the same empty result; the shield ring and the pod's own movement go on as usual. The pod's
  destruction is deferred the same way (7.1). So a tow cannot end by the player's hand once the snap has set the
  flag; the item goes on being lowered and is delivered (`economy.md` 2.2, 2.3). [V] (two runs, planet 0, kind 3
  forced onto the beam above the mothership, Down pressed 11 steps after the snap, with and without the shield
  fitted: $66 stayed 11 on every step, the item descended 1 px per step from the snap to the delivery box and the
  delivery screen opened exactly as with no press; with the shield fitted $65 became $80 one step after the press
  and the item was still delivered.) Before the flag is set (a snap not yet made), the release works as described above.
- What ends a tow in the original, and when: the release above (flag clear only); the pod's destruction (flag clear
  only); a delivery that is handled on the spot, the fuel kind that does not open the screen (`economy.md` 2.2, 3.4:
  $66 = 0 at once); the end of the delivery screen's 64th frame (`economy.md` 2.3); the item's own destruction
  (the fuel pod, kind 11, which is never delivered); a level reload. Nothing in the pod's step or the item's behaviour
  clears $66 or the flag during taking in.

### 8.2 The rope

The item hangs on a rigid rope of length 32 at angle $69 (0-127, 1/128 turn, 0 = straight above the pod, $20
right, $40 below, $60 left). Its position is the pod's plus an offset from the 32-entry quarter table
`rope` (bank 0 $CA53-$CA92, ROM 0x04A53; the full 64 bytes are listed, entries 32-63 serve as the catch radius):

```
00 02 03 05 06 08 09 0B 0C 0E 0F 10 12 13 14 15 17 18 19 1A 1B 1B 1C 1D 1E 1E 1F 1F 1F 20 20 20
20 20 20 20 1F 1F 1F 1F 1E 1E 1E 1D 1D 1C 1B 1B 1B 1A 19 18 17 15 15 14 13 12 10 0E 0C 0B 08 04
```

With r = angle AND 31: angle 0-31: (+rope[r], −rope[31 − r]); 32-63: (+rope[31 − r], +rope[r]); 64-95:
(−rope[r], +rope[31 − r]); 96-127: (−rope[31 − r], −rope[r]). [C]

Swing and placing, each pod step after the thrust (step 10 of 4.2), skipped when the item's taken-in flag
(RAM $0422 + slot) is non-zero: the angle with its fraction ($68.$69) moves by W's upper two bytes (8.3) in the sense
$6D: sense 0: $68.$69 += ($6C.$6B); sense 1: −=; the whole part modulo 128. Then those two bytes lose 1/256
(`$6C.$6B` − 1, both 0 if that would go below 0). The lowest byte $6A is not used here. The item is then placed at
the pod's position after this step's move plus the offset above. [V] (rope offset checked on 5,173 steps)

### 8.3 What towing does to the pod's physics [V]

Rewritten 2026-10-17 from the program and checked against the original: five long tows (mass classes 0, 1, 2;
gravity +8; 5,173 logic steps), three more with negative gravity (−8, −16, −24; 2,171 steps) and one with the
facing forced to 1 (193 steps). Every stage below reproduced every step exactly, except where marked.

**Amended 2026-10-17 (second round).** The first fit started from the state at the gravity step, so it missed
what happens before it: **e. the pull on the pod's own speeds** (new), the object-table byte behind the lift's
sign (b, corrected index), the torque running on every thrust step (d), and the taken-in case (point 4 below).
[V] on the 758 towing steps of `tools/mesen/replays/tow.replay` (e, b's index) and on 450 + 656 thrust steps of
`tow` and `delivery` (d).

**Names.** A = rope angle ($69, 0-127) with fraction $68; W = the swing speed, a 24-bit magnitude
$6C.$6B.$6A (W = 1.00 is $01.00.00; the angle moves by $6C.$6B per step, so 1.00 is one angle unit = 1/128 turn
per step; $6A is a finer fraction that only gravity adds to); s = the sense ($6D, 0 = A increases); m = the load
class ($67, 0-2, 8.1); f = the facing 0-31 after this step's rotation; g = the gravity value of 4.6 and
a = |g| >> 2 (so a = 0 on gravity-0 levels); S(i) = the `sin` table of 5.2 (index 0-127). "Towing" below means
RAM $66 ≠ 0 and the item's taken-in flag ($0422 + slot) = 0; while the flag is set (the mothership is taking the
item in) the pod behaves untowed (no pull, no lift, no thrust scaling, no swing, no placing), with one exception:
the torque d, see "Taken in" below.

**What persists.** Catching (8.1) sets A (from dx, dy), m, and W's bytes $6C and $6B to 0. It does not touch $68,
$6A or $6D, which keep what the last tow left (0 if none). Releasing or destroying the pod clears nothing but
$66 (7.1); the bytes $67-$6D stay as they were, also when the pod is destroyed (checked: A, W and s unchanged
after the step in which the tank ran dry). The logs of `replay-capture.lua` earlier showed them as 0 while
nothing was held, which looked like a clearing. Between tows only the torque (d) moves W, and with the scratch
rule below it can be reproduced exactly, including the negation that flips $6D and changes $6A; the angle A does
not move (the swing is skipped when nothing is held). Reproduce: keep $68-$6D across tows and run the torque on
every thrust step. [V] for the carry-over (a second catch started with $68 = $53, $6A = $D0) and for the torque
on 314 untowed thrust steps.

#### Order within one logic step

Only what differs from the untowed step of 4.2 is listed, in 4.2's order.

0. **Before the player's step** (start of the step's first frame, before 4.1): the towed item's speed bytes are
   set from the state the previous step left, **and the pod's own speeds are changed by the pull e** (below, "The
   item's own speed and the pull on the pod"). 4.1 then computes the movement angle $9C from the pulled speeds.
4. **Gravity**, unless anti-gravity (4.7): the pod's own gravity first (4.6, unchanged), then, if towing:
   a. the pull on W, b. the lift on the pod, below. Both use A and W as the previous step left them and the same
   g. (The pull e of step 0 has already changed the speeds.)
5-7. Cap, move, terrain: unchanged. The lift can take a speed above 3.00; the cap of step 5 limits it to 4.00.
8. Rotation: unchanged (the thrust and the torque use the new facing).
9. **Thrust**, if B is held and the fuel allows (4.5): scaled acceleration and carry-in (c), then the torque on W
   (d). If the fuel does not allow thrust, neither c nor d happens.
10. **Swing and placing** (8.2): the angle moves by W, W loses 1/256, the item is placed.

#### a. The pull on W (gravity step) [V]

p = S((2A) AND 127) × ((8a) AND 255), a 16-bit number, added or subtracted at W's two lower bytes ($6B.$6A).
The direction is "add" when (g < 0) XOR (A ≥ 64) XOR s is 0, else "subtract".

- Add: $6A.$6B += p. If that carries out of $6B, W becomes exactly 1.00 ($6C = 1, $6B = 0) and $6A is kept.
  There is no other limit. (The carry rule is the whole of the "cap": see the glitch below.)
- Subtract: $6A.$6B −= p, borrowing through $6C. If W would become negative it becomes its 24-bit negation (so
  the low byte $6A is negated too) and s flips.

**[V] Glitch (point 2).** The cap is not a cap. W is limited only when an addition carries out of $6B. Once W
sits exactly on 1.00 ($6B = 0), the next small additions do not carry, so W goes on rising to 1.xx; only when $6B
overflows is it reset to exactly 1.00. W therefore runs in a sawtooth between 1.00 and just under 2.00, and each
reset is an instant halving of the swing speed. Observed: W up to 1.77 (35 steps above 1.00 in the first log
alone), and six drops from above 1.00 straight back to 1.00. The torque (d) has the same rule. The fit's rule
("a carry out of the low 16 bits sets W to 1.00 and keeps the low byte") is the same rule when W stays at or
below 1.00; it differs from a true cap above 1.00. The intent was clearly a cap at 1.00; a remake that clamps
would change the feel of fast swings and the deliveries that depend on them, so reproducing it is the default and a
true clamp is a candidate for `docs/deviations.md` (Alex to decide).

#### b. The lift on the pod's vertical speed (gravity step, after a) [V]

q = (S(127 − ((2A) AND 127)) × a) >> (2 − m), a 16-bit number (qh = high byte, ql = low byte). Then the pod's
vertical speed is changed in the units "fraction byte = qh, accumulator $C5 = ql":

- **Subtract** or **add** is decided by bit 0 of the RAM byte at $0355 + X, with X = |S(127 − ((2A) AND 127)) − a|
  >> 1 (0 to 63; a is the value the gravity step left in RAM $08, normally |g| >> 2; [V] 757 of 758 steps, the
  exception had a = $21 and read X = 0). Bit set: subtract; clear: add. **It is not the pod's own direction and
  not the sign of g.** The byte is picked as follows (the object arrays are 31 bytes long, `objects.md` 2.1, so
  the frame array follows the direction array 31 bytes later, not 32):

  | X | Byte read (bit 0) | What writes it |
  |---|---|---|
  | 0 | slot 0, flags Y: the pod's vertical direction (1 = down) | gravity, thrust, collisions and the pull e flip it; read after step 0, so the pull has already acted |
  | 1 | slot 1, flags Y (the flame) | the flame routine (4.8) copies the pod's flags Y byte into it, bit 7 flipped by the flame's v-flip, every step it runs (B held, or while the flame lingers); when the flame is free nothing writes it |
  | 2-10 | slots 2-10, flags Y | slot 2 (the beam): nothing writes it during a tow, so it keeps its old value; slots 3-10: shots and sparks write it when they are created or move; free shots keep the last value |
  | 11-14 | the items | the towed item: step 0 writes its "down" flag = (A < 64) XOR s every step; the other items: their own movement |
  | 15-30 | placed objects and their children | each object's own movement; a freed slot keeps its last byte |
  | 31 | slot 0, frame | the pod's facing 0-31 (the byte of 4.3), after this step's rotation of the previous step; $FF while hidden |
  | 32 | slot 1, frame | the flame frame: base from `flameBase` (4.8) plus d; it is rewritten whenever the flame routine runs, and the last value stays when the slot is freed (the base alone, since the free step does not add d) |
  | 33 | slot 2, frame (the beam) | rewritten twice per step (once per frame) while towing: `T[((A + 4) AND 63) >> 2]` with T (bank 1 $941B, ROM 0x0941B) = `38 6F 39 70 3A 71 3B 72 3C 72 3B 71 3A 70 39 6F`; bit 0 of that, A as the swing left it [V] 12 of 12 |
  | 34-61 | slots 3-30, frame | shots and sparks (3-10) write their frame while alive, items and objects their own; free slots keep it (seen: slot 4 stale $11) |
  | 62, 63 | $0393, $0394: slot 0's and slot 1's work byte (the shooting-pose timer of slot 0) | counters; 0 in the logged tow |

  Freeing a slot clears only its state byte (`objects.md` 3.5; the reload loop and the level-change clearing do the same), so these arrays are
  persistent. At power-on the original's RAM is undefined (Mesen: all 0); the remake starts all 0. Reproducing
  the lift needs the whole object table with these two arrays, also for free slots. Read at the moment of the
  gravity step: after the previous step's object pass and this step's step 0. Seen in `tow.replay`: X = 0 on 232
  of 758 steps (bit set on 196), 3, 6, 9, 13 and 16-30 often (stale or item bytes), 33-61 and 62 for the frames.
- Add: $C5 += ql, the speed's fraction += qh + carry, the whole part += carry. No cap here.
- Subtract: the same with borrows. If the whole part would go below 0, the speed, its fraction and $C5 all become
  0 and the vertical direction flips (the leftover is discarded, unlike a thrust reversal).
- With a = 0 (gravity-0 levels) q = 0 and nothing happens.

**[V] Glitch (point 4).** The program meant to choose by the pod's direction relative to the pull, but the
register holding the index is overwritten by the multiplication that just computed q, so the byte read is
$0355 + X, an unrelated place. For X 0-31 that is the "flags Y" byte of object slot X (`objects.md` 2.1, bit 0 =
moving down; slot 0 is the pod); for X 32-63 it is the frame byte of slot X − 32 ($0374 + X − 32). X depends only
on A and a; the bits read are those of the object table at that moment of the step (after the previous step's
object pass, before this step's). Free slots keep their last values. The result looks random against "with or
against the pull": it agreed with that rule on about half the steps (the same with negative gravity, so the sign
of g plays no part). The lift therefore averages to about zero rather than lightening the pod. Faithful
reproduction needs the object table's two arrays (bit 0 of the direction byte, bit 0 of the frame byte, also for
free slots). A candidate for `docs/deviations.md`: drop the lift, or apply the intended "against the pull"
rule; Alex to decide.

#### c. Thrust under tow (point 1) [V]

The acceleration of each axis is first found as in 4.4 (table value, then halving, doubling or boosters: call
these av0 and ah0, 0-64). Then, towing:

- F = $38 + 32 × (2 − m) + ((4 × f) AND 63), 56-180 (no wrap).
- av = the high byte and cv = the low byte of F × av0; ah and ch likewise from F × ah0. So the **acceleration**
  is scaled (not the table value) and the low byte is the carry-in of 4.6 for that axis. Both are replaced: the
  vertical carry-in is cv (not the gravity value), the horizontal one is ch (not the star job's value).
- Apply as 4.6, with these carry-ins. The horizontal part runs only if the scaled ah ≠ 0 (untowed, the same
  test on the unscaled value is the same as "Th ≠ 0"). With a heavy load (m = 2) and facing 1 or 17, Th = $0C,
  ah0 = 3, F = 60: ah = 0, so the horizontal accumulator is left alone, carry-in or not. [V] on 146 steps.

The load class keeps F/256 of the thrust: class 0 47-70 %, class 1 34-58 %, class 2 22-45 %, depending on the
facing (the previous text had classes 0 and 2 swapped).

#### d. The torque on W (after the thrust, same step) [V] (point 3)

d = |(A AND 63) − ((4 × f) AND 63)|, h = S(2d) >> 4 (sin/16, rounded down; 0-8). The direction is "add" when
(((A − 4 × f) AND 127) ≥ 64) equals s, else "subtract".

- Add: $6B += h; a carry out of $6B makes W exactly 1.00 as in a (same glitch). $6A is not touched.
- Subtract: $6B −= h, borrowing through $6C; if W goes negative it is negated as a 24-bit number and s flips.

#### The item's own speed and the pull on the pod (step 0) [V]

One routine (bank 0 $A5C2 calls bank 7 $84BA) runs at the start of every step while towing (RAM $66 ≠ 0 and the
taken-in flag 0), before the pod's step. It also plays the rope sound on every fourth value of $5D.

*The item.* u = (($6C.$6B) >> 3) AND 255; i = (2A) AND 63; P1 = u × S(i); P2 = u × S(64 − i); speeds (8.8) =
P >> 4. If A AND 32 = 0: vertical = P1, horizontal = P2; else swapped. Horizontal direction is left when
(32 ≤ A < 96) XOR s; vertical is down when (A < 64) XOR s. Used for nothing while towed: the item is placed from the
rope every step and the speed is zeroed on release (8.1). [V] for the placement and the zeroing; [C] that no other code reads it.

*The pull on the pod (e).* Both of the pod's speeds are changed by an amount that comes from the swing:

- u = ((W >> 13) AND 255), i.e. ($6C.$6B << 3) >> 8.
- Vertical: pv = (u × S(2 × (A AND 31))) << m, 16-bit (m = 0, 1, 2 doubles the product m times). The pull is down
  when 32 ≤ A < 96, else up. If the pod's vertical direction equals the pull's, add pv to the speed (fraction byte
  += high byte, accumulator $C5 += low byte, carries up); else subtract. If the whole part would go below 0, the
  speed, fraction and $C5 become 0 and the pod's vertical direction flips (leftover discarded), as in b.
- Horizontal: ph = (u × S(2 × ((32 − A) AND 31))) << m. The pull is left when A ≥ 64, else right. Add or subtract
  against the pod's horizontal direction exactly as above, with accumulator $C4 and the horizontal flag
  ($0336 bit 0 = moving left).
- There is no cap in e; the cap of 4.00 in step 5 of 4.2 comes later in the step. Both axes use A and W as the
  previous step left them. The speeds, flags and accumulators after e are what the gravity step then sees.

[V] 758 of 758 towing steps (both axes, all three mass classes' formula; class 0 in the log, the shift by m is [C]).
The earlier statement that the pod's speed never enters the load's is true; the opposite way (the swing acting on
the pod) is e and the lift b, the only couplings. e is why "a pull along the rope" shows in black-box fits.

#### Taken in (point 4) [V]

While the item's taken-in flag is non-zero, the pod's step has no a, b, c, e, no swing and no placing (the item
is moved by its own behaviour), but the **torque d still runs on every thrust step**. It is also run when nothing is
towed: the code after the thrust is not guarded by the beam. Only its scratch input differs: d's own index
2d is computed in the scaling code of c, which is skipped; the torque then uses the leftover RAM $09, which the
terrain test of the same step leaves as **20 + (pod y AND 15)** (y's low byte after the move; 20-35). So:

- h = S($09) >> 4 = S(20 + (y AND 15)) >> 4, which is 3 to 7 (4 or 5 over most of the range, so W changes by 4-5
  in $6B per thrust step);
- direction as in d (add when (((A − 4f) AND 127) ≥ 64) equals s, else subtract; W negated and s flipped when it
  goes negative, with the same carry-to-1.00 glitch on adding).

[V] on 314 untowed and 91 + 311 taken-in thrust steps (the result is exact with the logged $09; $09 equalled the
formula on all but four of the taken-in steps, where a collision had moved y and the result was the same).
A is frozen meanwhile, so W grows or shrinks by that amount each thrust step until the sense flips; at the next
catch only $6B and $6C are cleared.

**There is no coupling in the other direction.** The pod's speed never enters the load's. The load influences the
pod through c (thrust scaling), b (the lift), e (the pull) and, via the swing it causes, through their angle
dependence; the pod influences the load through the rope's position (its position follows the pod's, after the
move) and through a, d (gravity and thrust on W). The sawtooth of the glitch in a and b's pseudo-random sign make
fast swings look like a speed coupling.

### 8.4 Slot 2 as the beam: how it is drawn **[V]** (2026-10-20, `delivery`, `tow` and `items` replays: 2,066 + 1,516 + 92 frames with the beam up, every field read at the end of each frame)

The beam is one object in slot 2 (state 9), drawn as a **two-sprite picture at the middle of the rope**, plus a
**tile whose pixels are rewritten every frame**. Both halves are needed: with a fixed tile the picture is a blob.

**Life of the slot.**

| Event | What happens to slot 2 | Pass |
|---|---|---|
| Catch (8.1) | state 9; position = the pod's (so it sits on the pod for this one picture); frame $38; flags X and flags Y **untouched** (they keep whatever the slot held: 0, or flags X of the previous tow) | A step 11 (after the status split) |
| Each step while held | state-9 behaviour, in the object loop (A step 4, slot 2 is the second slot run): if $66 = 0 (released this step) **or** the held item is being taken in (taken-in flag RAM $0422 + slot non-zero, 8.2) the slot is **freed at once** (state 0) and RAM $6E is cleared; otherwise it is placed (below) | A step 4 |
| Released by Down, by the shield, or by the pod's destruction (8.1, 7.1) | $66 = 0 in the player step (A step 3), so the object loop of the same step frees the slot: **the beam is never drawn on the step of the release** (destruction clears it directly) | A step 3-4 |
| Item taken in by the mothership (`economy.md` 2.3) | freed on the first step the taken-in flag is set (logged: state 9 to 0, 1 step after the last placed picture) | A step 4 |
| Jetman, level change, respawn | no beam: Jetman cannot tow (8.1), a level load clears every slot (`objects.md` 3.5), a new pod starts with $66 = 0 | |
| Gate state (9.2) | not needed: the gate touch requires nothing on the beam | |

The slot is never used for anything else than the beam (state 9) and the shield ring (4.7a, state $0C). Turning the
shield on while towing releases the item and the ring replaces the beam in the same step (4.7a).

**Position (every step while held, A step 4, after the pod's move in step 3 and after the item's placement on the
rope in step 3).** Let d = item position − pod position on each axis (16-bit world pixels, no camera, no
fractions). Slot 2 = pod + d / 2 on each axis, **the halving truncating toward zero** (take |d|, shift right once,
restore the sign; not a floor: with d = −3 the offset is −1, not −2). So it is the rope's midpoint, within 16 px of the
pod (the rope is 32 px, 8.2). **[V]** 2,062 of 2,066 frames exactly; the other four are the catch step (position =
pod) and the step the item is taken in (the mothership moves the item after slot 2 was placed). The picture is drawn
by the generic sprite drawer (`objects.md` 6) at this position with the camera of the B pass (`objects.md` 6.2).

**Frame (layout in table B) and the flag.** Written **twice per step, once in each pass**, by the video job of the
status-bar split (`game-flow.md` 1.2, step 7 of A and step 3 of B), from the rope angle A = RAM $69 as the swing
update of this step left it (8.2):

- i = ((A + 4) AND 63) >> 2 (0-15); frame = T[i] with T = `38 6F 39 70 3A 71 3B 72 3C 72 3B 71 3A 70 39 6F`
  (bank 1 $941B, ROM 0x0941B; the table of 8.3);
- flags X = (A >> 5) AND 1: 0 for angles 0-31 and 64-95, 1 for 32-63 and 96-127. Flags X is *replaced* by this
  (bits 1-7 are 0). It means "mirrored" (`objects.md` 6.3, fx = 1): every dx is negated and the sprites' own
  horizontal mirror bit is toggled (a layout attribute $41 becomes $01);
- flags Y is **never written** by the beam: it is 0 in all 5,000-odd frames logged (shield off sets it to 0 and nothing
  else writes slot 2); no vertical flip.

The job runs only while $66 is non-zero and the item is not being taken in; it is **skipped on the frames where the
score pop-up job of RAM $D2 bit 6 takes the split's time** (`graphics.md` 4.1, 0x08100: two frames, one step) - on
those two frames the frame, the flag and the tile below keep their last values. **[C]** (one step; not isolated in a replay)

Layouts (table B, ROM layouts via `objects.md` 6.1; the generic drawer needs nothing special): $38-$3C and $6F-$72 are
each **two sprites of tile $CF, attribute $41** (palette 1, horizontally mirrored by the layout), at

| Frame | $38 | $39 | $3A | $3B | $3C | $6F | $70 | $71 | $72 |
|---|---|---|---|---|---|---|---|---|---|
| sprite 1 (dx, dy) | 0, +4 | -2, +4 | -3, +3 | -5, +2 | -6, 0 | -1, +4 | -3, +4 | -4, +3 | -5, +1 |
| sprite 2 (dx, dy) | 0, -4 | +1, -3 | +2, -3 | +2, -2 | +2, 0 | 0, -3 | +2, -3 | +2, -3 | +2, -1 |

The drawer's usual +1 in x applies (`objects.md` 6.2), the mirror negates dx first. **[V]** (OAM read back for both
flags X; e.g. frame $72, flags X = 1, slot at screen (210, 74): sprites at x 216 and 209, y 75 and 73, attribute $01).

**The tile.** Sprite tile $CF (pattern table 0, PPU $0CF0, 16 bytes) is not part of any static set
(`graphics.md` 6.1): it is **rewritten on every frame** of a tow, in the same status-bar split, with 16 bytes
(8 low plane, 8 high plane) taken from nine images stored at ROM 0x0805D-0x080EC (bank 1 $805D, 16 bytes each,
numbered k = 0-8; the region 8.3 and `rom-map.md` list as "small tables"). The image number is k = i for i <= 8 and
16 - i for i > 8, i.e. the offsets 0, 16, ..., 128, 112, ..., 16 of the table at bank 1 $940B (ROM 0x0940B)
`00 10 20 30 40 50 60 70 80 70 60 50 40 30 20 10`. The images run from a short vertical bar (k = 0) through
diagonals to a short horizontal bar (k = 8); the two mirrored copies in the layout and the flags X mirror turn them into
the zigzag from the pod to the item, which is where the tile's pixels have to come from (read them from the ROM, they
are not in this repository). **[V]**: the RAM copy of the tile equalled image k in 2,065 of 2,066 frames (the exception
is the first frame of the tow, before the first upload).

*Timing of the image.* The upload is in the split at about line 191, below every sprite of the play area, so a
picture shows the image uploaded **in the previous frame**: the sprites of step n (built in B pass n) are shown for the
two frames after it, the first with the image of angle n (uploaded in B pass n), the second with the image of angle
n + 1 (uploaded in A pass n + 1). A remake drawing once per step can use the image of the step's own angle; the
one-frame difference is a hardware ordering, not a design (`docs/deviations.md`). **[C]** from the order of the
writes; the equality above was measured at the end of frames.

**Draw order and visibility.** The sprites are made once per step, in the B pass (`game-flow.md` 1.2), not twice. Slot 2
comes after slot 1 (the flame) and before every other object in the build that runs upward (slot 1 first) and last
among objects in the build that runs downward; the two builds alternate step by step (`objects.md` 6.6), which is the
original's rotation to avoid hardware flicker, so a remake can pick either, but it is **never in front of the pod**
(the pod uses OAM entries 1-4). The towed item (slots 11-14) is therefore behind the beam on half the steps in
the original. Attribute bits: the palette (bits 0-1) comes from the layout (1), bit 5 (behind the background) is set
from RAM $98 bit 5 (the pod in the mothership's bay area, `economy.md` 2.2; also set for the flame), bit 4 is set
(no hardware effect). The pod's palette flash (RAM $98 bits 0-1) and the pod's hit pause/blink do **not** touch the beam.
Nothing hides the beam while it exists (frame $FF is not used); it is culled with the other objects only if it is
far off-screen, which the rope length prevents.

## 9. Other states

### 9.1 Final level (state 6, level 26)

Eight-way flight without gravity or thrust, carried along by the camera's route (`levels.md` 5.2). Everything
below up to "Checked" is **[V]** unless marked: a model of these rules reproduced the original's position, speeds,
direction bytes and camera in every step of three runs (no input for 4,200 steps; random directions for 3,900 and
5,500 steps, across the whole route and 1,400 steps past its end), except in the steps of a collision response
(about 25 per run, whose result is the ordinary one of 5).

**Frame and bytes read again from the program (2026-10-17) [C]:** the horizontal and vertical byte logic is exactly the
whole-byte compare described here; the frame is the constant $2D, or $2E when the input byte has any of bits 0-3 set and
bit 2 of RAM $5D is set (so it alternates with the frame counter only while a direction is held). Drawing: layouts P$1E / P$1F, picture and palettes in `objects.md` 6.4a; the mirror test of `objects.md` 6.4 reads RAM $0336, so a horizontal byte of 1 (after a bounce from the right-facing 0) draws the pod mirrored and 0, 4 and 5 do not.

**Bytes.** Input (RAM $18, held buttons): bit 0 right, 1 left, 2 down, 3 up; left beats right, up beats down.
The horizontal direction byte (RAM $0336) is 0 for right and 5 for left, the vertical one (RAM $0355) is 0 for up
and 1 for down; the pod's own step compares them as **whole bytes**. A collision response only flips bit 0, so
after a bounce the horizontal byte can be 1 or 4, which equals neither 0 nor 5 (holding the same way then brakes
the pod to 0 first, and the byte is set there).

**Start (level entry).** Pod at (96, 1021), horizontal speed 0, vertical 0.40 up, both bytes 0, camera (0, 928),
camera move (0, 0).

**Order of one logic step** (items 1-9 are the pod's own step in the A pass; 10 and 11 are the camera's passes):

1. If the fuel (RAM $C7:$C6) is 0, the pod is destroyed (7.1; the level has no lives, so the game ends: state 5,
   the camera stops, play is left 96 steps later) and nothing below runs.
2. **Horizontal speed.** With left or right held, d = 5 (left) or 0 (right) and `d` equals the horizontal byte:
   speed + 0.80 (cap 7.00), then a whole part above 4 loses its fraction (so 5.00 is the top). Otherwise (not
   held, the other way, or the byte is neither 0 nor 5): speed − 0.FF, not below 0; when it reaches exactly 0 the
   byte becomes d (0 when nothing is held).
3. **Vertical speed.** With up or down held, d = 0 (up) or 1 (down) and `d` equals the vertical byte: speed +
   0.FF (cap 3.00). Otherwise speed − 0.80, not below 0; at exactly 0 the byte becomes d (0 when nothing is
   held).
4. Frame $2D; $2E instead when any of the four directions is held and bit 2 of RAM $5D is set. [C]
5. **Move.** x − horizontal speed if bit 0 of the horizontal byte is set (left), else x + speed; y + vertical
   speed if bit 0 of the vertical byte is set (down), else y − speed (16.8 positions). No other limit (the map
   edges are not reached).
6. **Screen limits**, below.
7. The gun (10): a newly pressed fires the special shot (state $20, frame $6D, speed 7.00 right). With a smart bomb
   fitted (Select held, RAM $D1 = 4) the press does not make a shot, but see Q32 in `behaviour.md`: the program then treats it
   as one and writes the shot's bytes to the wrong slot, clearing the pod's state.
8. The ordinary terrain test and collision response (`levels.md` 6.4, 5 of this file); there is no gravity and
   no thrust, and no fuel is used. The movement angle (5.1) is computed from the speeds at the start of the step,
   before items 2-3. [C]
9. **Contact kills.** If the surface flags RAM $DD are non-zero after the test (anything touched except a flat
   ceiling, `levels.md` 6.4 step 6), the fuel becomes 0: the pod is destroyed at the start of the next step. A flat
   ceiling costs only the usual impact damage (5.3). The damage and the response still happen in the same step.
10. Later in the A pass, after the objects: the camera moves by the **previous** step's move (mx, my), then the
    route code of `levels.md` 5.2 runs once: it advances the route, computes this step's move (mx, my) (each −1,
    0 or +1 on the route) and applies the **carry** to the pod's speeds (below).
11. The B pass moves the camera by the new (mx, my); the pod does nothing. So the camera moves 2 px per axis per
    step on a straight stretch, and at a change of move by one of each.

**Screen limits (item 6).** With the camera (cx, cy) as it was at the end of the **previous** step (after both
of that step's moves): let D = x − 24 − cx; if D < 0, x = cx + 24; else if D ≥ 208, x = cx + 232. Vertically,
D = y − 16 − cy; if D < 0, y = cy + 16; else if D ≥ 160, y = cy + 176. Only the position changes: the fraction,
the speeds and the direction bytes are untouched, so a pod held at a limit keeps its speed. The left limit is what
really carries the pod along the route: on a straight stretch the pod's own step leaves it 1.01 px of
movement against the camera's 2, so it slides back to cx + 24 within about 70 steps and is then set there every
step.

**Carry (item 10), each axis separately.**

- Vertical, only if neither up nor down is held and my ≠ 0. D = 1 if my > 0 (down), else 0. If bit 0 of the
  vertical byte equals D: when the whole speed is below 2, the speed becomes 2.00 and 0.20 is then subtracted
  at once (a slip of the program: the result is **1.E0**; the byte is not changed because the result is not 0).
  Otherwise (opposite): speed − 0.20, not below 0, and at exactly 0 the byte becomes D.
- Horizontal, only if neither left nor right is held and mx ≠ 0. D = 1 if mx < 0 (left), else 0. If bit 0 of the
  horizontal byte equals D: when the whole speed is below 2 the speed becomes **2.00**. Otherwise: speed − 0.20,
  not below 0, and at exactly 0 the byte becomes 0 if ((byte AND $FE) OR D) is 0, else 5 (so a byte of 4 or 5 does
  not become 0 when the camera moves right; it is set by item 2 at the next step).
- Nothing else changes the pod's speeds, and the carry never moves the pod.

**After the route's end** (`levels.md` 5.2, "End"): the carry rule above is unchanged, but (mx, my) now come from
the end routine's swaying of the camera (|mx|, |my| ≤ 3), once per step, in the same place. The first step of the
end has no carry (the move is (0, 0)). Two things change in the pod's step: at item 6 the camera's vertical move my
(left from the previous step's end routine) is first cleared, and the vertical limits become pushes. In both
limit cases, with d the excess (negative above the top, d − 160 below the bottom):

- Top (D < 0): if cy is 256-463 (cy high byte 1, low byte below $D0), y = cy + 16 as before. Otherwise y is **not**
  corrected and my = (low byte of D) / 2 with bit 7 set (−128..−1).
- Bottom (D ≥ 160): if cy is 576-767 (high byte 2, low byte $40 or more), y = cy + 176 as before. Otherwise my =
  (low byte of D − 160) / 2 (0 to 3 in practice); if that is 0 (an excess of 0 or 1) y is corrected, else it is not
  corrected. A slip of the program: for an excess of exactly 0 outside the band the correction subtracts 1, so y
  ends 1 px above cy + 176.
- The camera move of item 10 (between the pod's step and the end routine) uses this my, and the end routine then
  replaces it for the B pass. So a pod pressed against the top or bottom drags the camera by half its excess in
  the A pass only, and is itself not held back that step. **[V]** 731 top and 41 bottom pushes, the top band 232
  times, the slip once.

Checked in Mesen with `level26` and random-input replays (scratch, 2026-10-17). In the first step the camera does
not move in its own A pass (the move is still (0, 0)), and the pod does not move either (speed 0 at its step); the
route code then sets the move (1, 0) and the carry gives the pod 2.00 horizontally.

### 9.2 Level-change gate: wormhole touch, bounce and the gate state (state 9)

All of 9.2 was run in Mesen on 2026-10-17 (planet 0, planet 5, levels 13 and 18, the pod and Jetman alone) and the
gate state was reproduced step for step by a model (9.2.3): **[V]** unless marked.

#### 9.2.1 The touch

The wormhole (item kind 18, `economy.md` 5.3) is tested by the common touch routine for the item slots 11-14, once per
logic step, **in the A pass after the player's step and the object loop** of that step, and before the transition
request is looked at (`game-flow.md` 1.2). The test needs all of:

- the player state is 1 (pod), 2 or 3 (Jetman flying or walking) or 6. **Jetman alone can use a wormhole**, with
  the same rules as the pod; states 0, 4, 5, 7, 8 and 9 never touch it (the table of who may touch is in bank 0 at $A143; state 10 exists only in the interlude scene);
- the probe shield is off (RAM $65 bit 7 = 0) and nothing hangs on the tractor beam (RAM $66 = 0). Otherwise nothing
  at all happens: no bounce, no sound;
- |item x − player x| ≤ 13 and |item y − player y| ≤ 13 (the two objects' own positions, no offsets: the same 13-pixel
  box as the other touch tests, `economy.md` 2.5).

Then the gate is **open** when the level is 13 or above, or when the mothership's fuel count `c` (RAM $0513, list
entry 0, `economy.md` 3.4) is exactly 25. A closed gate bounces the player (9.2.2). An open gate asks for transition
code 3 and the transition guard decides (`game-flow.md` 7.4): the request is refused when the last transition
code RAM $40 is 3 **and** the guard counter RAM $41 is not 0. **A refused touch is ignored completely**: no state
change, no bounce, no sound, nothing. An accepted one is withdrawn at once (RAM $3F is cleared) and the player goes to
state 9. This corrects the earlier table row ("any to 9 while the guard refuses"): the gate starts when the guard
**lets the request pass**.

What the guard means in play **[V]** (level 13 held on top of its wormhole; Jetman and the pod): on a planet the
reload leaves $40 = 2, so the guard never refuses a wormhole there, also in the first 64 steps after a launch or after
a return. On a second level ($40 = 3 after the code-3 transition that brought the player there) the first 64 logic
steps after the reload ($41 counts down by one per A pass from 64) are refused, so the player who arrives standing on
the second level's wormhole (`levels.md` 3.3: it stands at the home point) enters it by himself at the 65th step
(measured: reload frame 556, gate state at frame 680 with the player held inside the box all the time). Nothing on a
second level clears or re-arms the guard (`game-flow.md` 7.4 lists who does elsewhere).

Entering the state: the previous state is kept in RAM $69; RAM $0393 of slot 0 gets the gate's slot number; $67 and $68
are cleared; the spawn margins $050C/$050D become $80; the parallax stars are reset; **song 2 is requested** (the
level song stops; `sound.md` 8.1).

#### 9.2.2 The bounce of a closed gate

A closed gate (level below 13 and `c` below 25) acts on the player in the same step as the touch **[V]**:

- the **direction bit** of one axis is flipped, speeds and position unchanged: the horizontal one (RAM $0336 bit 0) when
  |dy| ≥ |dx|, otherwise the vertical one (RAM $0355 bit 0); dx, dy = the player's offset from the wormhole, as in the
  test. A player moving towards the wormhole along that axis therefore leaves again. If he is still inside the box
  in the next step the whole thing repeats, every step;
- a **sparkle** (object state $0B, the pick-up sparkle ring: frame $69 first, life RAM $0393 = 8, frames $6C-$69 as in
  `objects.md` 7) is made in **slot 3** at the wormhole's own position (offset 0, 0), overwriting whatever slot 3
  held, e.g. a shot. [C] The program first looks through slots 4-10 for an object already in state $0B and then makes
  none; no other creator of that state in those slots was seen;
- sound effect **$44** on both pulse channels (`sound.md` 8.3), requested in the same step;
- RAM $99 (the terrain-hit pause, `economy.md` 3.3) is set to 1: it counts down in one step and only blocks a terrain
  hit in that step. There is **no damage, no fuel change, no invulnerability ($97 unchanged) and no visible flash of
  the pod** (the drawing rule of `objects.md` 6.4 draws the pod normally for $99 = 1). Jetman alone is bounced in the same
  way (checked).

#### 9.2.3 The gate state, step by step

State 9 runs in the A pass only (one handler call per logic step; the frame counter RAM $5D has its low nibble forced
to $0D by the first call, which makes the first two calls fall on consecutive video frames and every later pair 2 frames
apart: a hardware cadence, the remake counts steps). Let `n` = 1, 2, ... be the step of the state and `G` = the gate's
position (x, y). Gravity, terrain, fuel use, thrust, the gun and damage do **not** act; the pause is not allowed
(Start was held during the whole state: no pause, `player.md` 3). The camera follows the player as usual
(`levels.md` 5.2). The status panel is unchanged. Each step does, in order:

1. **n = 1 only.** Every object slot 1-30 except the gate's is **removed** (state 0, item field cleared): enemies,
   shots, sparks, the mothership indicator, anything. Nothing spawns while the state lasts: the placement scan and the
   items scan do not run (`levels.md` 3.3, `objects.md` 3.4).
2. **The player homes on the gate** (`behaviour.md` 2.3 rule, accel 0.50, no gate position offset, x and y both
   evaluated first and the two moves done after):
   - per axis, `toward` = the direction bit points from the player to G (G at the same coordinate counts as right or
     down). If toward: speed magnitude += 0.50 (8.8: $0080); the horizontal speed is then capped at 3.50 and the
     vertical one at **3.00** (the vertical add has its own 3.00 cap; **[V]** the vertical speed never reached 3.0 in
     the checked run, the horizontal one reached 3.5). If not toward: speed −= 0.50, floored at 0, and when the result is
     0 the direction bit flips;
   - then the ordinary position update (4.9) moves x, then y. There is no brake: the player overshoots and swings back
     and forth through the wormhole for the whole state. In the checked run (start at (+8, +5) from the gate with 2.0 px/step of
     speed) the offset from the gate stayed within −11..+13 px horizontally and −3..+5 px vertically for all 193 steps;
3. **The picture of the pod** (only if the previous state is 1; Jetman keeps his own frame): the frame is
   `2 × (RAM $5D AND 3)` with $5D as the step sees it, which gives 2 in step 1 and then 4, 0, 4, 0, ... one frame per
   step. RAM $98 is flipped between 0 and 3 every step; the pod's sprite palette is the byte $98 (palette 0 and
   palette 3 alternate every step, 2 video frames: **this is the "flash"**), while the particles have a fixed
   palette 1.
4. **The counter and the particles.** A counter `k` (RAM $68, set to 0 on entry) goes up by 1 on every step where
   `$5D AND 3` = 0, that is on steps 3, 5, 7, ..., so after step `n` it is `floor((n − 1) / 2)`. In the same step
   **a burst of 8 particles starts** (below), in this order of preference:
   - `k` even: group A (slots 15-22) if its first slot is free, else group B (slots 23-30) if its first slot is free;
   - `k` odd: group B if its first slot is free;
   - otherwise no burst. At most two groups exist; a group dies as below and its slots then become free again.
5. **The particle groups** are updated, group A first, then B (each only when alive). All 8 slots of a group share one
   radius `r` and one angle `a`; they differ only in their phase. Fields: `r` (whole pixels, starts 1), `v` radius
   speed (8.8, starts `(k AND $FC) / 4 + 4`, that is `floor(k / 4) + 4`, as a whole number: 4 for k below 4, 5 for 4-7,
   ...), `w` angular speed (8.8 of 1/256 turn per step, starts $0050), `a` angle (8.8, starts 0).
   Per update, in this order: `r += floor(v)`; if r is then above 255 the group dies; `v −= $12/256`; if v is
   then below 0 the group dies; `a += w`; `w += $60/256`. The ring thus grows (fast, then slower), turns faster and
   faster and dies when its radius reaches 255 or its radius speed reaches 0 (for k = 0 after 57 steps, r about 114; for
   large k the radius overflow comes first). The group's `k` at birth fixes its speed, so later groups are larger and
   faster. **If a group dies in a step, the update of the other group, and the position update of a group born in that
   step, are skipped for that step** (a stale position shows for one step: cosmetic, the remake need not copy it).
   **Position** of particle `i` (0-7) of a group: `G + polar(r, A)` with `A = (floor(a) + 32 i + o) AND 255`,
   `o` = 15 for group A and 0 for group B, and `polar` the rotation of `scenes.md` 5.4a: dx = floor(S × r / 256) with
   the sign of the angle (negative when A ≥ 128), dy the same with the angle + 64 (S = the sine table of 5.1,
   index folded to 0-128). Each is drawn as the star of object frame $BC (a 16 × 16 shape of 4 sprites, palette 1).

   **The pictures of the original differ from this in the following ways (2026-10-20, [V] against the OAM of `wormhole.replay`,
   `wormhole-jetman.replay`, `wormhole-warp.replay`; none is a rule to copy):**
   - **The gate object is drawn on every second step only.** It is an ordinary object (slot 11, frame B$52, four sprites in palette 1, drawn at
     its own position like any item); the sprite builder's 64-entry limit (`objects.md` 6.6a) cuts it, and the last stars, on the builds that run
     through the slots in descending order, and the gate comes last. Not a hidden slot, not a changed frame, not a skipped draw: the slot, its
     state ($0A) and its frame ($52) stay the same throughout. The remake draws the gate on every step.
   - **A star is cut to three sprites** (and the one or two stars after it are missing) in the same builds: the "torn" star. Every
     other star's four sprites sit at its one position (the tile $C1 pair at Y + 4, the $C0 pair at Y − 4, x at X − 3 and X + 4 for the
     usual unmirrored picture, the +1 of `objects.md` 6.2 included).
   - **A star takes the slot's old flag bytes.** A burst writes position, state and frame but not the flags X/Y of its slots, so a slot that
     last held an object with flags X bit 0 set (moving left) draws its star **mirrored** (`objects.md` 6.3: dx negated, attribute
     bit 6 toggled; the star is not symmetric, so the picture shifts a pixel and its two halves swap order). Seen: slot 24 on level 13 (flags
     X = 1 left by the level's own object in that slot, state $91, frame $2D, that the first step removed): every star of slot 24 mirrored for the whole state; on planet 0, where
     the slots were empty, none. The remake need not copy it (stars drawn unmirrored); one that keeps per-slot leftovers (`behaviour.md` 2.5) gets it by itself.
   - **Stars at the top edge: negative world y is normal, and the lower half of a star shows above the first lines.** (2026-10-20, **[V]**
     `wormhole.replay` and `wormhole-jetman.replay`, the stars' RAM and the OAM of every gate step. The second gate state of both replays, steps 273-465, is
     at level 13's wormhole, world (124, 42), with the camera at its top limit (0, 0), so stars of radius above 42 get a **negative world y**
     down to -84.) Rules for the remake:
     1. A star's position is a **signed 16-bit** world position, `G + polar(r, A)`. It is never clamped, wrapped to the level's other end or
        treated as dead or off-screen for being above y = 0 or above the camera: in RAM it reads $FFxx (log `oy` of slots 15-30, $FFFF = -1,
        $FFAC = -84) and the star carries on like any other. A group dies only by its radius and radius speed (above), never by position.
        This holds for every star slot, 15-30, group A or B alike (no special slot or table entry).
     2. Visibility is decided **per sprite**, by the sprite's own screen y = star y + dy - camera y (dy = -4 for the two upper sprites, tile $C0,
        +4 for the two lower ones, tile $C1), not by the star's centre: a sprite is drawn when that 16-bit value is 0-255 (`objects.md` 6.6a, skipped
        sprites). There is no wrap: no star sprite ever had a y byte of $F9-$FF in these captures (the only $F8 entries are unused ones). So a star
        whose centre is at screen y -4 to -1 shows its **two lower sprites only** (sprite y 0-3, picture lines 1-12), the upper pair being dropped. A centre at
        -12 to -5 shows nothing in the original (the lower pair would be at -8 to -1: dropped whole, although part of it would reach line 0;
        "one partly above the top is hidden whole", `docs/deviations.md`, "Sprites at the screen's edges": the remake may draw the visible rows
        instead, but must not drop a star for its centre being above the top). Sprite x is tested the same way. Counted from the logs: some star
        has its centre at -4 to -1 in 16 steps of the first gate state and 45 of the second one of `wormhole.replay` (the same in `wormhole-jetman`),
        the "about 40 steps" in which the remake drew nothing.
     3. All 380 + 380 gate steps of both replays match this rule together with the 64-entry limit of `objects.md` 6.6a, the top-edge steps
        included.
   - **The 1 px at steps 62-66 of `wormhole-jetman` is not an error of the original's picture: it is the camera the picture was taken at.** **[V]**
     (Mesen, 2026-10-20: writes to the sprite table logged with the camera RAM $4E/$4F and the OAM-DMA writes.) The shot of step N is the
     frame after the A pass of step N+1: its sprites were written, and its scroll set, at the camera **after the A half-step of step N+1**
     (`levels.md` 5.2: trunc(d / 2) of the player's position change in step N+1, when the band test passes), i.e. the logged camera of step N plus
     that half-step. The log's camera is the one after the B pass of step N. In steps 62-66 the half-steps of steps 63-67 were 1 px each
     (changes 3, 3, 2, 2, 2) and every star and the gate sat exactly 1 px above the position given by the logged camera (camera used $26F, $272, $275,
     $277, $279 against the logged $26E, $271, $274, $276, $278, and the OAM-DMA frames' camera RAM read the same); from step 67 on (change 1,
     half-step 0, then rest) they match the logged camera exactly. Stars and background are drawn at the same camera in that frame, so nothing
     is out of place on screen. A comparison of the stars against a logged camera must use the step's logged camera plus the A half-step of the next step
     (0 when the camera is at rest). In the pod run the camera never moved in the state, which is why it matched. The remake draws one picture per step
     and has nothing to copy.
6. **The palette fades.** On every step where `$5D AND $1F` = 0 (steps 3, 19, 35, 51, ...) each of the 8 palette
   shadow entries $7E-$85 (background palettes 2 and 3) is darkened: if its high nibble is 0 it becomes $0F (black),
   else its high nibble goes down by 1. After the fourth fade (step 51) everything in those two palettes is black, so the
   terrain goes dark in about 100 steps while the player and the stars keep their (sprite) colours. [V] palette
   values logged: $3C → $2C → $1C → $0C → $0F.
7. **The end.** When `k` reaches 96 (step 193: 383 video frames after step 1 and 385 after the touch, which is
   4 frames less than song 2's 389) the state ends:
   the previous state is restored (RAM $69), **RAM $97 = 64** (64 steps of invulnerability, 5.4), the gate's item
   entry is removed (its RAM kind set to 0: the wormhole does not exist again until the level is loaded again), the
   gate object is freed, $67, $68, $69 and the whole parts of the player's speeds ($02BA, $02F8) are cleared. Then:
   - on a planet (level below 13) or when the planet's flag is not set (`economy.md` 5.1; the flag is that of the
     planet recorded in RAM $3C): transition code 3, at once, without the guard (`game-flow.md` 7.4);
   - on a level of 13-28 whose planet's flag is set: the kind-6 message screen (`game-flow.md` 7.4, `scenes.md`
     10.1) is built, which requests code 3 when it ends.

   Where the player appears on the next level: `game-flow.md` 7.4, "Code 3".

### 9.3 Interlude (state 10)

The player walks left and right on the interlude screen: x is kept within 16-240 and the speed is reset at the
edges; it has no terrain test. Details belong to the interlude's spec. [C]

## 10. The gun

- **Firing:** A newly pressed (including the autofire repeats of 4.1) in states 1, 2, 3 and 6. Not on warp
  levels. [V]
- **Slot:** the first free slot of 3-10; if none is free, the first slot of 3-10 whose state is 1 (a spark) is
  taken over; otherwise no shot. So at most 8 shots, sharing the slots with sparks. [C]
- **Sound:** effect $06 on pulse 2, started directly (not in the demo), **only when a slot was found** (no slot: no shot, no sound). [V]
- **The shot** (state 8, frame $37 table B, strength $0441 = RAM $050E + 1, life $0393 = 32):
  - from the pod: placed at the nose: with n = (f + 16) AND 31, pod position + (off[n + 8], off[n]);
    speed = (|off[f + 8]| / 2, |off[f]| / 2) (8.8, e.g. 5.00 straight ahead), with the facing's directions
    (horizontal: left for facing 17-31, the pod's own direction for 0 and 16; vertical: down for 9-23, the pod's
    own for 8 and 24);
  - from Jetman: placed at (±4, +1) from him on his facing side, 6.00 px per step horizontally, vertically 0;
  - on the final level: placed at (6, 9), then turned into the special shot of 9.1;
  - **then the shooter's velocity is added** (vector sum, through the direction bits). [V] pod: facing 8 at 2.80
    right gave 7.80 right
- `off` is bank 0 $CAC3 (ROM 0x04AC3), 40 signed bytes (it also places the exhaust flame at
  (off[f + 8], off[f])):

  `0A 0A 08 07 07 05 03 01 00 FE FC FA F9 F8 F7 F6 F6 F6 F8 F8 F9 FB FD FE 00 02 03 06 07 08 09 0A 0A 0A 09 08 07 05 03 02`
- **Life:** the shot flies straight at constant speed (no gravity, no terrain test); every 4th step (when
  (slot + $5D / 2) AND 3 = 3: the handler adds the processor carry left by the movement routine, which is always
  1 for a shot) its life − 1, and at 0 it is removed: at most 128 steps. In practice it ends sooner,
  by the off-screen rule (`objects.md` 3.5) or by hitting an object. [V] movement, life count and the AND 3 = 3
  phase (24 samples, one of them a downward shot), [C] removal
- **Picture by age:** at each life tick (the same every-4th-step event) the picture may change, when the new life is a
  multiple of 4 and the bullet upgrade (RAM $050E) is not 0: frame $37 becomes $62; and frame $62 becomes $63 if the
  upgrade is 2 (military). Life is 32 at the start, so with an upgrade the picture is $62 from life 28 (after 4 life ticks,
  about 16 steps) and, with military bullets only, $63 from life 24 (after 8 ticks); then it stays. Plain (0) bullets keep
  $37 all the way, titanium (1) ones stop at $62. **[V]** 2026-10-17 (Mesen, all three upgrades: $62 at life $1C, $63 at $18).
- **Rate:** one shot per A press, or one per 6 / 4 / 2 steps with A held for pod types 0 / 1 / 2. [V] type 0
- **Fitted weapons:** with Select held, a fitted weapon (RAM $D1) and weapon energy left, the press makes that
  weapon's shot instead of a plain bullet: 10.1. Their behaviour: `behaviour.md` 5.11.

### 10.1 Special weapons: choosing and making the shot **[V]**

A press is special when all of these hold: Select is held (RAM $18 bit 5), RAM $D1 is not 0, and the weapon
gauge (RAM $C9 high byte, $C8 low byte) is not 0. **The test is on the whole 16-bit value** (high byte OR low byte not 0; a
gauge of $0040 still makes the special shot, then the cost empties it; **[V]** 2026-10-17, missile with $C9 = 0, $C8 = $40, and the
same with both 0, which made the plain bullet). Otherwise it is the plain bullet above. The press always asks for
sound effect $06 first (not in a demo), whatever the weapon, **but only once the slot search has found a slot**: with no slot there is no shot and no sound **[V]** (Mesen, `sound.md` 8.3).

1. The slot is found as for a plain bullet (first free slot 3-10, else the first spark, state 1, in 3-10; no slot, no
   shot). The plain bullet's position, flags, frame, hit points and life field are worked out as above; then **only the slot's
   state byte** is cleared (so a spark that was taken over is gone) and everything else of the plain bullet stays
   (read from the program, 2026-10-17, [C]; Q30 in `behaviour.md`).
2. By D1:

   | D1 | Weapon (shop item) | Made | Gauge cost |
   |---|---|---|---|
   | 1 | homing missiles | state $0F, frame $27, hit points 2, work 5 = $C0 | 4 |
   | 5 | time bombs | state $10, frame $3D, hit points $40, work 5 = $C0 | 32 |
   | 6 | multi warhead missiles | state $14, frame $27, hit points $FF, work 5 = $C0 | 16 |
   | 8 | star bullets | state $12, frame $27, hit points $50, work 5 = $18 | 32 |
   | 4 | smart bombs | no object; the effect of 10.2 | 128 |
   | 2 | anti-gravity | no object: toggles RAM $D2 bit 7 (4.7) and clears bits 0-1 of RAM $98 (the flash bits; they are rewritten from the next step on while it is on, `game-flow.md` 9.2); no cost | 0 |
   | 3, 7 | none sold | as anti-gravity | 0 |

   (Shop item numbers `economy.md` 6.) For D1 = 1, 5, 6 and 8 the shot is made **only if the slot found is 3, 4, 5 or
   6**; with a higher slot the state is cleared, nothing is made and nothing is charged (the press is lost), but the slot
   keeps the plain bullet's bytes: position, direction bits, frame $37, hit points, $0393 = $20, $03B2 = 0 **and** its speed
   vector (the plain speed routine still runs on this path; none of it is visible, the state being 0). A smart bomb and the
   switches do not depend on the slot; they leave the slot's old speed (they return before the speed routine) and
   the rest of the plain bullet's bytes in the same way. **[V]** (`economy.md` 6; four missiles in flight lost the fifth press.)
   **[C]** for the leftover bytes (Q30).
3. The shot: position and direction bits as the plain bullet. Hit points as in the table (the bullet's 1-3 is not
   used). Field $0393 stays at the plain bullet's $20 (a missile reads it as "target slot $20", `behaviour.md` 5.11.1),
   $03B2 = 0, work 5 as in the table, work 6 not cleared. The speed vector is that of the plain bullet
   (|off| / 2 per axis, 5.00 straight ahead) for homing and multi warhead, divided by 16 for star bullets, and 0 for
   bombs; then the shooter's velocity is added by the vector sum of the plain bullet. Jetman and the final-level pod fire these weapons too, with their own plain-bullet speed (6.00 / 0) as the base. **[V]** (star bullet at facing 4:
   0.2188 / 0.2188 = 00.38 / 00.38 less the pod's 00.02 downwards = 00.36 vertically; at facing 14: 00.18 / 00.4A; a time
   bomb: the pod's 00.00 / 00.02 only.) The nose position is the plain bullet's.
4. The cost is taken after the shot: the gauge high byte − the cost; if that is 0 or negative both bytes of the gauge
   become 0, otherwise the low byte is kept. **[V]** (224, 192, 160 for star bullets).

The special shots live in slots 3-10 like every shot, so they fill the same eight slots; a star parent and its children
share them (`behaviour.md` 5.11.2).

### 10.2 Smart bomb **[V]**

No object. Every object in slots 15 to 30 with a non-zero state and a frame other than $FF is destroyed by the rule of
`behaviour.md` 3.5, in slot order, wherever it is (also off screen; enemy shots, children, explosions in progress). Each
one in 23-30 scores +5 (+3 if small) and is removed from its level's list for this visit. Not affected: items and
bonus objects (11-14), the pod's own shots, invisible objects (frame $FF, e.g. the generators). Only the last victim in
23-30 is left exploding (`behaviour.md` Q29). **On the final level** the bomb wipes out all five cores (type 5E) at once:
the rule of `behaviour.md` 3.5 has no exception for them, so their "hit only while the frame is $B6" rule and their hit points
do not matter (all five gone in the step of the press, the last one left exploding, no other object of slots 23-27 alive).
**[V]** 2026-10-17 (Mesen, at the end of the route with the cores closed, weapon fitted and gauge poked). But the press on that
level can also freeze the game: Q32 (`behaviour.md`): with a gauge above 128 after the press has been paid for, the pod's own
state is cleared and the level can never end.

## 11. How this was verified

Mesen, headless, level 0 from a new game, savestates at the start of play and in flight; RAM logged at the end of
every B pass (`game-flow.md` 1.4) with scripted input:

- A model of the pod's step (gravity, top-of-planet gravity, cap, move, y clamp, rotation, thrust with both
  accumulators, fuel) reproduced 1277 of 1279 steps without terrain contact exactly (x, y with fractions, both
  speeds and directions, facing, fuel, vertical accumulator). The two others: one horizontal-carry-in step
  (4.6, now explained) and one enemy hit (5.6).
- The movement-angle algorithm (5.1) matched 1305 of 1305 steps.
- Towing (8.3): per-stage RAM logged inside each logic step (before and after gravity, thrust, swing) on nine long
  tows with the load class, the gravity value or the facing forced by the test script; a model of 8.3 reproduced all
  7,500 or so steps exactly (swing speed and sense, the pod's speeds and accumulators, the item's speed bytes and
  position).
- Two slope responses, flat-floor bounces with and without damage, the hit pause, shield on/off (width and height,
  fuel), boosters with Select, the pod's destruction, Jetman's emergence, flight, jetpack, Down, walking and take
  off, his death, the respawn, and the gun were watched step by step; values above marked [V] come from these runs.

## 12. Fidelity notes

- Towing glitches (8.3): the swing-speed "cap" that lets W run to just under 2.00 and drop back to 1.00, and the
  lift whose sign is read from an unrelated object byte. Both are bugs; both are kept by default and are candidates
  for `docs/deviations.md` (Alex to decide).
- Keep: the step order of 4.2 (thrust after the move), the L1-sum speed rebuild of 5.2, the vertical reversal's
  double acceleration, the never-resting bounce, the random-state rotations by impact sparks, the respawn timing.
- Drop: the state 4/8 skips when the video buffer is busy (7.1). Approximate: the horizontal carry-in (4.6,
  `docs/deviations.md`).
- Correction to `levels.md` 6.4: the soft landing sets only the vertical speed to 0, not both speeds (Jetman keeps
  walking; [V]).

## Open questions

- Vertical thrust reversal is read, not yet measured. Towing (8.3) is measured; open there: the junk that untowed thrust steps leave in $6A and $6D, and whether anything but the release reads the towed item's own speed bytes.
- Docking conditions (6.4) and the gate and interlude states (9.2, 9.3) only summarised.
