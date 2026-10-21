# Game flow, timing, input, pause and random numbers

How the original is organised in time (what runs on each frame, what a logic step is, whether it lags), how it
reads the controller, how pause works, how its random number generator works, and the top-level state machine of
scenes from power-on to the ending. A RAM map of the globals found is at the end (section 9): it names what a
state replay should compare and when.

Status: **[V]** verified in Mesen (RAM watched while the game ran with scripted input, or a model checked against
a trace), **[C]** read from the program, **[H]** hypothesis. Addresses follow `README.md`. Words: a **frame** is
one video frame; a **pass** is the logic run started by one frame's NMI; a **logic step** is one pass outside
gameplay and two passes (A then B, section 1.2) in gameplay. Text shown on screens is in `text.md`, the password
in `password.md`, objects in `objects.md`. Where the remake differs, see `docs/deviations.md`.

## 1. How a frame runs

### 1.1 Everything happens in the vertical-blank interrupt

The game has no main loop. After a scene is set up, the CPU sits in an idle loop (bank 0 $9570, a jump to
itself). Each NMI (once per frame, 60.0988 Hz) does the following: **[C]**, structure **[V]**

1. Select bank 0 and jump to the scene's NMI handler, whose address is held in RAM $050A-$050B.
2. The handler does the frame's video work (the sprite table, RAM $0700-$07FF, goes to the PPU; whatever the
   scene queued for video memory and the palette is uploaded; the scroll is set), reads the controllers, and,
   in every scene except pause, runs one step of the sound driver (bank 4 entry $800F). NMIs are enabled again
   before the logic starts.
3. The handler then **resets the stack and jumps to the scene's logic routine**. This pass updates the game and
   ends by jumping back into an idle loop.

So every NMI starts a fresh pass from the top, whatever the CPU was doing. If a pass is still running when the
next NMI arrives, the rest of it is abandoned (section 1.4).

Every scene's pass begins by servicing one of the two sound-effect request slots (RAM $F3/$F4 and $F5/$F6; slot A
on frames where $5D is even, slot B where it is odd). The same service also runs whenever a scene's loading code
waits for a frame (default handler, below). During an attract demo the requests are dropped without being
played. The request lock RAM $F7 (one request let through, then 128 frames of discarded requests) is in
`sound.md` 8.2. **[C]**

Each scene has one handler/logic pair. The pairs found (bank 0 addresses):

| Handler | Logic | Scene (section 7) |
|---|---|---|
| $FF91 | none | default, while a scene loads: sound step and frame counter only |
| $B696 | $817F | title screen and attract timing |
| $B539 | $8587 | attract story (Jetman cartoon) |
| $B74A | $86CF | password entry |
| $B76E | $88D9 | interplanetary flight |
| $B7EB | $8E06 | shop list / equip-pod list |
| $B78A | $884E | approach screen ("APPROACHING PLANET") |
| $B6D5 | $8B34 | planet surface picture (landing, planet menu, blast-off) |
| $B4B2 | $8F51 | launch scene (pod leaves the mothership) |
| $B0B6 | $94F5 | gameplay |
| $B4CB | $91FD | pause |
| $B740 | $9263 | planet map (`scenes.md` 13) |
| $B60B | $92C4 | message screen (items delivered, analysis, warpship pieces) |
| $B33B | $9148 | intermission with the Space Loonies badge (transition 6) |
| $B50C | $CFC4 | interlude on a separate backdrop screen (transition 7) |
| $B634 | $941A | game-over animation |
| $B5CD | $94DE | "YOUR LAST PASSWORD WAS" |
| $B466 | $D03A | ending sequence (`scenes.md` 14.2) |
| $B393 | $D0F7 | scrolling credits (`scenes.md` 14.3) |

**Default handler.** While a scene is being built (rendering off, data being decoded into video memory) the
handler is $FF91: it runs the sound step, increments RAM $5D and returns to the interrupted code. The building
code runs outside the NMI and waits whole frames by watching $5D change. These loading frames are not passes.
Their number depends on how much data the change decodes **[V]** (measured, in frames: title to approach 5,
approach to surface 22, surface to launch 13, launch to gameplay 19, dock to surface 13, password to flight 5,
flight to shop 10, shop to approach 6, gameplay to message 3, message back to gameplay 18, transition 3 28,
respawn 19, transition 5 to the approach 15, pause and resume 0).

### 1.2 Gameplay: two passes per logic step

Gameplay is the only scene whose logic is split over two frames: an **A pass** on frames where RAM $5D is even
when the NMI arrives, a **B pass** where it is odd. **[C]**, parity behaviour **[V]**

Gameplay handler, on every frame:

- the sprite table upload;
- one rotating video job, chosen by RAM $60 (table below);
- HUD and map streaming: a queued buffer of tiles (RAM $0102-, count $52, flag $53, PPU address $55/$56), and a
  30-tile map row or column when the scroll has crossed a tile boundary;
- the scroll;
- **the controllers, read only before an A pass;**
- one sound driver step.

| $60 | Job | Status |
|---|---|---|
| 0 | palette upload | C |
| 1, 4, 6 | fuel gauge (the "F" bar: a display copy of the fuel, RAM $05CE, `economy.md` 3.1) | C |
| 2, 5 | weapon energy gauge (the "W" bar, RAM $C9) | C |
| 3 | lives and jetpod icons ($3E, $3D) | C |
| 7 | score digits | C |
| 8 | palette upload (only on the first frame of a level; set at level start) | C |
| 9 | blanks a 3 × 3 tile area of the HUD (first frame of the final level) | C |

$60 advances by one per frame and wraps 7 to 0 (8 and 9 therefore occur once each, then become 1 and 2).
So the fuel gauge is refreshed on 3 frames in 8, the weapon energy gauge on 2, the rest on 1. **[C]**

**Pass contents.** In each pass the work runs in the order listed, top to bottom. **[C]**, pass assignment
**[V]**

| A pass ($5D even) | B pass ($5D odd) |
|---|---|
| 1. attract demo only: a real button press ends the demo; the autopilot writes the input bytes (7.1) | 1. camera and map streaming (bank 3 entry $8009) |
| 2. tractor-beam towing effects | 2. object sprites into the sprite table, in an order that alternates every pass, and removal of objects far off-screen (bank 1 entry $8003) |
| 3. player handling: input, thrust, rotation, gravity, terrain test (`levels.md` 6) | 3. **status-bar split** (as in A) |
| 4. object behaviours: every object slot runs the behaviour of its state (bank 7 entry $800F; `objects.md`) | 4. the player's sprite (bank 1 entry $8000) |
| 5. camera and map streaming (bank 3 entry $8009) | 5. new objects spawned from the level's placement list as the camera reaches them (bank 4 entry $8003; `objects.md` 3) |
| 6. two further level updates, depending on the level flags and the tractor beam | 6. per-level routine (a table by level index), or the alternative-mode update when RAM $9B bit 7 is set |
| 7. **status-bar split**: wait for the sprite-0 hit, change the scroll for the HUD; within it the random generator steps once, then $5D (carrying into $5E, $5F) increments | 7. act on a transition request in $3F (7.4) |
| 8. Start: pause, if the player state allows it (section 4). Pausing ends the pass here | |
| 9. periodic events: the transition guard, fuel use, timed enemy arrivals | |
| 10. collisions of the player with enemies and their shots (bank 1 entry $8033) | |
| 11. collisions of the player and the tractor beam with items (bank 0 $A14D) | |
| 12. act on a transition request in $3F (7.4) | |
| 13. attract demo only: end the demo when the player state is 2 | |

**Which pass changes what [V].** Measured in Mesen over 1,200-1,500 gameplay frames of random play, recording for
every write the parity of $5D at the start of its pass:

| Data | A pass | B pass |
|---|---|---|
| player position $0200/$021F/$025D/$027C slot 0, speeds $029B/$02BA/$02D9/$02F8 | all writes | none |
| other objects' positions (slots 1-30) | all writes but one | only when an object is spawned |
| object state $0317-, frame $0374-, list index $0393- | almost all | spawning and removal only |
| camera $4C-$4F | yes | yes (the second half of the step) |
| sprite table $0700-$07FF | only the HUD sprites | all object sprites |

So the game logic proper (input, player, enemies, items, collisions, scoring) advances **once per logic step of
two frames, 30 steps per second**. The B pass redraws: it finishes the camera move, streams the map, rebuilds the
sprites from the new positions and spawns objects that came into view. The camera moving in both passes spreads
each step's scroll over two frames (`levels.md` 5.2).

- **Every gameplay level and state** uses this schedule: the launch flight, the planets, the second and third
  levels, the final level.
- A speed of "n px per step" means n px per two frames.
- The remake keeps the schedule: it decides when input is seen, how often objects move, and the camera's half
  steps. A remake running a 60 Hz tick does the A work on even ticks and the B work on odd ticks.

### 1.3 Other scenes

Every other scene's logic runs completely on every frame, so its logic step is one frame. Its handler reads the
controllers on every frame. **[C]**; input on every frame **[V]** for the title, password, approach, pause and
message screens. Each scene logic increments $5D itself, at a point of its own (section 5, "Where it steps").

### 1.4 Overruns (the original's "lag")

If a pass has not reached its idle loop when the next NMI fires, it is cut off there: the new NMI resets the stack
and starts the next pass. The game does not slow down. Instead, part of the work for that frame is lost.
**[V]**

Measured with random input on each of the 42 levels for 7,000-7,400 gameplay frames each: most levels never
overran; level 4 overran on 343 frames, 17 on 205, 6 on 194, 35 on 36, 19 on 10, 9 on 6, 29 on 2 and 15 on 1.

- **Every overrun seen** was on an A pass, after its status-bar split. So the frame counter had already advanced,
  the next pass was a normal B pass, and the end of the A pass (A-pass items 8-13 of 1.2) was skipped for that
  step. **[V]**
- **An overrun before the split** would leave $5D unchanged, so the next pass would run the A pass again. Not
  seen. **[C]**

**For replay validation:**

- No RAM byte records completion. Use an execution callback in Mesen.
- A pass is complete when the CPU reaches its idle loop after the NMI. The idle loops are bank 0 $9570 (with RAM
  $75 AND 7 = 0), and, for some scenes, $949B (game-over animation), $94F0 (last password), $95C4/$95CF
  (transition code) and $F38B/$F38D (the approach screen's pass ends in the frame wait). **[C]**
- In gameplay, compare game state once per logic step, at the completion of each **B pass**: the first arrival
  at the idle loop after an NMI at which $5D was odd (at that point $5D has just become even). Camera-only
  comparisons can be made every frame.
- A step whose A pass was cut is marked diverged, not compared.
- Loading frames (handler $FF91) are not passes.

### 1.5 Frame counter

- RAM $5D (low) and $5E (high) count frames. During loading the default handler increments $5D. **[V]**
- **Set to 0 when the scene starts:** title (and to 1 when the badge sub-state starts), attract story,
  interplanetary flight, surface picture, gameplay (every level start and reload), message screens, game over.
  The credits set it to 1. The final message (kind 7, reached by the end-of-final-level routine) and the ending do
  not reset it: the ending starts with the value the message screen left. **[V]**
- **Not reset (it carries on):** password entry, approach screen, shop and equip list, launch scene, pause, planet
  map, backdrop interlude, last password. The badge intermission carries on until the badge lands, then the scene
  resets it to 0 (`economy.md` 5.4). **[C]**
- In gameplay, pause, the map, the approach screen, the message screens, the game-over and last-password screens,
  the badge intermission and the backdrop interlude, the increment happens inside the status-bar split. That split also carries into RAM $5F when $5E wraps. $5F is otherwise only cleared
  by the RAM clear at power-on and reset (section 2). The credits' own split carries into $5E but not $5F. **[C]**
- Scenes use $5D and $5E as their clock: animation phases, timeouts, the 4-frame text typing rate, gameplay's
  A/B parity.

## 2. Power-on and reset

1. Clear the PPU registers, wait two vertical blanks, initialise the APU. **[C]**
2. Clear RAM, except the stack page $0100-$01FF and $06FA-$06FF. **[C]**
3. Check for a warm reset: $06FA-$06FF are compared with the 6-byte signature at bank 0 $8066 (`5A 49 70 70 4F 21`,
   "ZIppO!"). **[C]**
   - **Cold:** write the signature, clear CHR RAM, show the licence screen (text message 24) for 512 frames.
     **[V]**
   - **Warm** (signature present, e.g. after a game over or the end of the credits, which restart the program):
     skip the licence screen. **[C]**
4. Start the title music, clear CHR RAM and build the title screen (`graphics.md` 5.2). **[V]**

## 3. Input

### 3.1 Controller bytes **[V]**

| RAM | Meaning |
|---|---|
| $18 | controller 1 buttons held |
| $19 | controller 1 buttons newly pressed: held now and not held at the previous read |
| $1A | controller 2 held |
| $1B | controller 2 newly pressed |

Bits: 7 A, 6 B, 5 Select, 4 Start, 3 Up, 2 Down, 1 Left, 0 Right.

- **Edge rule:** `pressed = (held_now XOR held_before) AND held_now`, computed at each read.
- **Between reads** both bytes keep their values, so in gameplay a press is visible for both passes of a step. A
  press that starts and ends between two reads is never seen: in gameplay, a press lasting one frame that falls
  before a B pass. **[V]** (a Start press before a B pass was acted on one frame later, at the next A pass)
- **Controller 2** is read together with controller 1, at the same reads, so $1A/$1B always hold its state at the
  last read. Nothing but the random generator uses them (section 5). **[C]**
- **Attract demo:** the autopilot writes $18/$19 itself (7.1).

### 3.2 Repeat

Only the password screen repeats a held direction (`password.md` 9). No other screen repeats input. **[C]**

### 3.3 Buttons per scene **[C]** ([V] where marked)

| Scene | Buttons |
|---|---|
| Licence screen | none |
| Title | Select toggles START GAME / CONTINUE; Start chooses **[V]**; in sub-state 0, Start skips ahead (7.1) |
| Attract story, demo | any newly pressed button returns to the title |
| Password entry | `password.md` 9 **[V]** |
| Interplanetary flight | A skips the rest of the flight |
| Approach screen | any newly pressed button lands (the screen says START) **[V]** |
| Surface, mothership descending | A skips the descent |
| Planet menu | Up/Down move the cursor between the options shown, with no wrap; A chooses |
| Shop / equip list | Left/Right scroll the list, A selects, B leaves |
| Launch scene | any newly pressed button ends it |
| Gameplay | Start pauses, if the player state allows it (9.1) |
| Pause | Start resumes; Select opens the planet map when allowed (section 4) |
| Planet map | Start returns to the game |
| Message screens | any newly pressed button ends the message once its timer is below 64 (7.4) |
| Game over, last password | any newly pressed button restarts the program (warm reset) |

## 4. Pause

- **Entering:** Start pressed in gameplay. It is seen only in an A pass, after the status-bar split, and only if
  the player state allows it (9.1). **[V]**
  - The game silences the APU (all channels off), installs the pause handler, and ends the pass there. The rest of
    that A pass (A-pass items 9-13 of 1.2) is not done for that step and is not done later. **[C]**
  - The picture freezes; no text is shown. **[V]**
- **During pause:**
  - The sound driver does not run, so music and effects stay frozen where they were (`sound.md`). **[V]**
  - The logic only watches the buttons, but it still runs the status-bar split, so **the frame counter and the
    random generator advance on every paused frame**. **[V]**
  - The controllers and the rotating job counter $60 advance every frame; no rotating video job runs. **[C]**
- **Start** turns the sound channels back on and reinstalls the gameplay handler. The next pass is A or B by the
  parity of $5D at that moment. **[V]**
- **Select** opens the planet map when both of these hold: on a planet surface (level index $3B < 13), and RAM
  $060C or $060D (the mapping device items, `password.md`) is nonzero. Otherwise Select does nothing **[V]** (the
  "does nothing" case).
  - The map screen (handler $B740) draws the whole planet, one tile per chunk, with markers (not an "explored" map)
    and cycles the colours of the markers (`scenes.md` 13.2). **[V]**
  - On Start (the only key it reads) it starts the level music again and reloads the level (7.4, "Reload"). **[V]**

## 5. Random number generator **[V]**

**State:** RAM $5A (the output byte), plus a 16-bit shift register in $5B (high) and $5C (low). All three are
cleared only by the RAM clear at power-on and reset (section 2), never by a scene.

**One step:**

```
feedback  = bit 3 of $5A XOR bit 6 of $5A
carry     = bit 7 of $5B                   # the only carry entering the sum
($5B,$5C) = ($5B,$5C) shifted left by one, feedback entering bit 0 of $5C, old bit 7 of $5B dropped
sum       = ($5A + 1) AND $FF              # this increment produces no carry
for each v in [$18, $19, $1A, $1B, $5D, $5E, $5F, $60, 13]:
    sum   = sum + v + carry                # 8-bit add
    carry = 1 if that addition exceeded 255 else 0
$5A       = sum                            # the last carry is dropped
```

The values of $5D, $5E, $5F and $60 used are those at the moment of the step (see "Where it steps"): $5D-$5F from before the pass's increment, $60 after the frame's advance. The model
reproduced 2,376 consecutive calls traced in Mesen (title and gameplay with random input) with no difference.

**Inputs outside gameplay.** **[C]**

- **$60** advances by one per frame ($60 becomes ($60 + 1) AND 7, wrapping 7 to 0). The video job of that frame
  uses the value from before the advance, but the generator step (inside the status-bar split, after the
  advance) sees the **already advanced** value **[V]** in gameplay (a frame that enters with $60 = 3 steps the
  generator with 4); in the other scenes the advance is a call at the start of the scene's handler, so the same
  order is expected **[H]**. The advance happens in every scene's handler except the launch scene, the ending,
  the credits and loading frames, where $60 keeps its value.
- **$60 is set** to 0 when the interplanetary flight starts, when the landing starts and when the shop or equip
  list starts; to 8 at each gameplay level start, or 9 on the final level.
- **$18-$1B:** section 3.1.

**Where it steps.** Exactly one step per pass, in these scenes:

| Scene | Order within the pass |
|---|---|
| gameplay (A and B passes), pause, planet map, approach, message screens, game-over animation, last password, backdrop interlude, badge intermission | inside the status-bar split, before that split increments $5D: the step sees the frame counter of the pass's start |
| credits | inside the credits' own split, before its $5D increment |
| ending | before the ending's $5D increment |
| title, interplanetary flight | after the scene's own $5D/$5E increment: the step sees the incremented counter |

- The controllers are always read before the step (in the handler), except that gameplay B passes keep the bytes
  from the A pass.
- **Exception:** the message screen of kind 6 (7.4) increments $5D itself and does not step the generator.
- **No step:** loading frames, password entry, attract story, surface picture, launch scene, shop and equip list.
- **Cut passes:** a pass cut before its step does not step it; the rerun does.

**[C]**; gameplay, pause and title **[V]**

**Other writers.** Some behaviours change the state directly, so it is not a pure function of the step count.
Seen a few times in 1,400 gameplay frames. **[V]** that they happen, **[C]** the rules. The object-side writers
(domes, generators, pod missiles, impact sparks and the like) and readers are listed in `behaviour.md` 8 and
`player.md` 5.5; they are not repeated here. Non-object writers:

- **A particle spawn** (bank 1 $90B1): after one generator step, $5A = $5A + $47 (8-bit); the spawn goes on only if
  the result is below 6 (about 6 in 256 calls), and then uses $5B and $5C for the particle's position and kind.
  The caller was not found. **[C]**
- **Backdrop bars** (bank 7 $83C2, the 15 bars of the backdrop interlude, `scenes.md` 11.1.1): for each idle bar,
  $5A = $5A + $37 (the incoming carry is always 0), and the slot starts when the result AND $7F is below 23. **[C]**

Impact sparks (`player.md` 5.5) rotate $5A twice per spark. Their incoming carry, which fills one of the new bits,
is bit 4 of (8 − player's whole horizontal speed − whole vertical speed) for the first spark and 1 for the others.
**[C]**

**Readers, by purpose.** **[C]**

| Reader | Purpose |
|---|---|
| title, bank 0 $81A8 | every 16 frames, $5A (raised to at least 32, rounded down to a multiple of 4) picks one of the star sprites (sprite offset 32-252, so sprites 8-63) and switches it to palette 3 (`scenes.md` 2.1) |
| bank 0 $A65F, $D49F | random arrivals of enemies on planets: only when bit 7 of $5A is clear, $3B < 13 and one of slots 23-25 is free; $5C bits choose the screen edge and offset |
| bank 0 $AD70 | a random one of 12 offsets for an object spawn relative to the camera (the alternative HUD mode's level objects) |
| bank 0 $A0CA | scrambling digits shown on a message screen (the analysis, `text.md` 4.4) |
| bank 6 $AB62 | a timed spawner of objects at a random position (active while $5E < 5) |
| bank 7 $C5EA, $C62C | object timers (4-19 and 8-55 steps), `behaviour.md` 8 |
| objects | `behaviour.md` 8 |

## 6. Level index and game structure

RAM $3B is the level index (0-41), used everywhere to select the level header, map, objects and music
(`rom-map.md`).

| $3B | What | How it is reached **[C]** |
|---|---|---|
| 0-12 | the 13 planets ("1 PRELUDON" ... "13 UROWND") | new game (0); blast-off (+1); password; from 13-25 and 29-41 by transition 3 |
| 13-25 | a second level belonging to planet $3B − 13 | transition 3 from the planet |
| 26 | the Golden Warpship flight (final level) | after the last warpship piece is delivered |
| 27, 28 | special levels reached by transition 5 | transition 5 with destination 27 |
| 29-41 | a third level belonging to planet $3B − 29 | transition 3 from level 13-25 when that planet's flag is set |

- The planet name table has a 14th name, "14 DRYN KUP LEEZ". With $3B = 13 the approach screen shows it, but in
  play 13 is a second level, not a planet. **[V]** (approach screen drawn with $3B poked to 13)
- **Systems.** Planets are grouped by three into five systems: system = $3B / 3 (RAM $EA), named ZANDOR, FROBOF,
  MUGRIM, SHOBAS, XEXORM. The shop's stock depends on the system. **[C]**
- **Planet flags** (RAM $DE-$DF, one bit per planet). **[C]**
  - Bits: bit 7 of $DE is planet 0 ... bit 0 is planet 7; bit 7 of $DF is planet 8 ... bit 4 is planet 11.
  - Planet 12 has no bit of its own: it uses planet 10's.
  - When the level is 13 or more, the flag used is that of the previous level index $3C (12 again counting as
    10).
  - A flag is set when that planet's warpship piece is delivered.
  - Every approach screen rebuilds the flags as "all planets before the current one".
  - The warpship is complete when bits for planets 0-11 are all set ($DE = $FF, upper nibble of $DF = $F). Since
    12 shares 10's bit, a game needs 12 pieces.
- **Blast-off order:** the next planet is $3B + 1. Planet 12 is the exception: blasting off from it leads to 11
  (as if it were 10). **[C]**
- **Planet 12** is reached from planet 7 by a transition that sets $3B = 12 (bank 0 $A365). The route 7 → 12 → 11
  skips planets 8-10; arriving at 11 marks them done. **[C]**
  - Blasting off from 11 to 12 needs 11's piece delivered first (BLAST OFF is offered only then, 7.3). With all
    earlier flags set, that delivery completes the warpship, so the game goes to the final level instead. **[C]**
    (reasoning from the rules above)

## 7. Scene state machine

```
power-on ──cold──> licence (512 fr) ─┐
         ──warm──────────────────────┴─> TITLE ─Start on START GAME──> new game ─────┐
                                           │   └Start on CONTINUE─> PASSWORD ──valid─┤
                                           │                          └Start, invalid┤
                                           └─timeout─> attract (badge, story or demo) ─> TITLE
 START LEVEL (below) <────────────────────────────────────────────────────────────────┘
 START LEVEL: $3B = 0 ──────────────────────────> APPROACH
              $3B ≠ 0 ─> FLIGHT ─end─> ($3B odd) SHOP ─B─> APPROACH
                                    └> ($3B even) ─────────> APPROACH
 APPROACH ─any button─> SURFACE (mothership lands) ─> LAUNCH ─> GAMEPLAY
 GAMEPLAY ─dock (2)─> SURFACE with menu: EXPLORE ─> LAUNCH ─> GAMEPLAY
                                         EQUIP POD ─> EQUIP LIST ─B─> LAUNCH
                                         BLAST OFF ─> mothership leaves ─> $3B+1 ─> START LEVEL
 GAMEPLAY ─item delivered (1)─> MESSAGE ─> GAMEPLAY | dock | final level | ending
 GAMEPLAY ─Jetman lost (4)─> respawn | GAME OVER ─> LAST PASSWORD ─button─> warm reset
 GAMEPLAY ─(3)/(5)─> another level ;  (6)/(7) intermissions ─> GAMEPLAY
 GAMEPLAY ─Start─> PAUSE ─Start─> GAMEPLAY ;  PAUSE ─Select─> MAP ─Start─> GAMEPLAY
 final level complete ─> MESSAGE (congratulations) ─> ENDING ─> CREDITS ─> warm reset
```

### 7.1 Title and attract **[V]** timings, **[C]** rules

The title logic steps through sub-states (RAM $0C, timer $0D):

| Sub-state | What happens | Ends |
|---|---|---|
| 0 | stars and sun | when $5E reaches 2 (512 frames). Start ends it early; the fade of sub-state 1 then starts at its seventh (last) step, so the logo is at full brightness after 8 frames |
| 1 | palette fade-in of the logo, 7 steps, one every 8 frames | after the steps (64 frames) |
| 2 | logo slides up, one pixel per frame (sprites every other frame) | after 48 frames |
| 3 | "START GAME" and "CONTINUE" written, one per frame (`text.md` 4.1) | 3 frames |
| 4 | menu box frame drawn, one tile or tile pair per frame (26 updates); then the copyright line, in one update (`text.md` 4.1) | 27 frames |
| 5 | menu active (START GAME / CONTINUE; Select toggles, Start chooses); a small Jetman sprite walks across | when he has left the screen (about 990 frames) |
| 6 | "Federation of Space Loonies" badge | after 256 frames: attract |

**Attract cycle.** RAM $0512 counts timeouts. It is incremented first, then tested.

- **Odd count:** the attract story (handler $B539). So the first timeout after power-on gives the story.
  - It is timed by the story clock c = $5D + 256 × $5E: the scroll slows at c = 240, the face loop starts at 320,
    the scroll stops at 600 and the closing rise starts at 680 (observed frames and the whole scene: `scenes.md` 3).
    It shows 754 frames (c = 4-757), then 14 blank frames before the title.
  - Any newly pressed button ends it at once.
- **Even count:** a gameplay demo on level 6, then 4, then 10 (a three-entry table), with RAM $D1 (an equipment
  item fitted to the pod) set to 1, 8, 6. After the third demo the count restarts at 0.
- **Demo setup (what is reset)** **[V]** (full RAM compared at the first play frame of four demos, power-on to the
  second round): RAM $050F = $80 (demo flag), the new-game state (7.2: 4 Jetman lives, 4 jetpods, score 001000,
  $3C = $FF, no equipment), $DC = $C0 (pod type 0, probe shields and boosters), $D1 = 1, 8, 6 for the three demos
  (the demo weapons, below), fuel and weapon energy both full ($C6/$C7 and $C8/$C9 = $E000), the waypoint index RAM
  $0510 = 0, the frame counter $5D/$5E = 0, every object slot cleared (the pod in slot 0 starts as after a launch),
  the pod's position fractions and speed fractions, the hit-pause $99 = 0, the window size $F1/$F2 = 6/6, the
  parallax stars' dead mask (all dead) and their y bytes ($FF). There is no flight, approach, landing or launch: the
  demo goes straight into gameplay at the level start (player position and camera from the level header,
  `levels.md` 3). **[V]**
  - Start position seen: level 6 and level 4 x = 1915, y = 191; level 10 x = 2299, y = 191 (RAM $0200/$021F and
    $025D/$027C). Facing 0, vertical speed 1.5 px per step upward, horizontal speed 0, as after any launch
    (`player.md` 4.1a).
- **Demo carry-over (what is not reset)** **[V]** (the values at the first play frame equal the values left by the
  previous demo, and differ from the power-on values, which are 0). The attract demos of one power-on run are
  not independent; a remake that starts each demo from a clean state differs from the original in these bytes:
  - **The pod's thrust accumulators RAM $C4 (horizontal) and $C5 (vertical)** (`player.md` 4.6). These are the only
    pod fields that matter. Demo 2 started with $C4 = $45, $C5 = $A5 (the values the pod had when demo 1 ended),
    demo 3 with $20/$15, the second round's demo 1 with $7E/$47. A test that zeroed just these two bytes at the
    start of the second round's demo 1 gave exactly the pod fields (position, speeds with fractions, facing) of the
    first demo 1 for the next 700 frames, while the unchanged run first differs in a speed fraction after 9-11
    frames. **The demo-end rule must therefore leave $C4/$C5 alone and the next demo must start with them.** (After
    power-on they are 0.)
  - **The shield timer RAM $0511** (7.1 below): $15 at the start of every later demo (it was 0 at power-on), so the
    first A passes of demos 2, 3 and the second round do not behave like those of the first demo.
  - **The invulnerability timer RAM $97** (`player.md` 5.4): it holds 96 at the start of every later demo (left by
    the Jetman who emerged from the last pod) and counts down by 1 per logic step, so the pod of demos 2, 3 and
    later is invulnerable for the first 96 logic steps (192 frames); the very first demo (power-on, 0) is not.
    Zeroing it changed nothing else in 700 frames of the pod's fields. **[V]**
  - The autofire countdown $EF (left at 6), the random generator ($5A-$5C, with $60; `game-flow.md` 5, never reset,
    which is why a demo's length changes from round to round: demo 1 lasted 4,459 frames the first time and 3,261
    the second), and the sound driver. The terrain list table and the star job's state follow.
  - **The terrain table RAM $A8-$B7 and $D4** (section "Shield" below) keep the previous demo's last values until the
    first logic step recomputes them. The autopilot's first A pass therefore reads a leftover table (0 at power-on,
    which is "not 0 to 15" and so starts the shield timer at once; the empty one after demo 1; a non-empty one after
    demos 2 and 3), but the non-zero timer makes that harmless in every demo except the first.
  - **The parallax-star job** (`graphics.md` 5.7.3): its mode RAM $D5 (0 at the first demo's start, 1 at the second's,
    $FF at the third's, 0 at the second round's), the stars' x bytes ($04B7-$04BE) and their x and y
    fraction bytes ($04C7-$04CE, $04CF-$04D6) survive; the dead mask is set to all dead and the y bytes to $FF. A non-zero mode is cleared by the
    job's first call (all stars dead). **This has no effect on the pod in a demo**, because the thrust never sees the star
    job's carry-in byte there (below): clearing the mode and the terrain chunk byte at the start changed one
    scratch byte in one frame and no pod field in 700 frames. The stars are cosmetic in a demo.
  - Slots of the object table that are not alive keep their old speed, direction and frame bytes; they are written
    when the next object is made.
- **During a demo:**
  - Transition requests are dropped (7.4).
  - Sound-effect requests are dropped (1.1).
  - The autopilot (below) supplies the input.
- **End of a demo:** when the player state (RAM $0317) becomes 2, i.e. the pod was destroyed and Jetman is out
  on his own, or on a newly pressed real button (checked at the start of each A pass, before the autopilot writes
  its bytes). **[V]**
  - There is no time limit and no end of the route: the autopilot never stops by itself. The demos seen all ended
    by the pod being destroyed (state 4, then state 2 22 frames later, then the title 16 frames after that).
  - Then the object tables, $DC and $D1 are cleared, and the title starts again from sub-state 0.
  - The length varies, because the pod's fate depends on the random generator and the frame counter (the enemies'
    behaviour does). From power-on with no input, Mesen: demo 1 (level 6) set up at frame 5120, gameplay from
    5147, state 4 at 9584, state 2 at 9606, title at 9622; demo 2 (level 4) 14199, 14223, 21500, 21522, 21538;
    demo 3 (level 10) 26115, 26142, 32835, 32857, 32873; demo 1 again (second round) 37450, 37477, state 2 at
    43418. **[V]**
- **After the story** the title also restarts from sub-state 0. **[V]**


**Demo autopilot** (an input generator in the demo's own code bank, plus a part of the pod's step) **[V]**. The rules below were written as a
model and compared with Mesen at every A pass of four complete demos (10,869 passes, including the second round, no
differences; a second implementation of exactly the rules and tables below, run over the same recorded passes,
also gives no differences). They are exact, including the places marked **quirk**, which are how the original steers
and which the demo needs to repeat its recorded paths (a demo-only deviation from "no bugs", `PLAN.md` Fidelity; to
be listed in `docs/deviations.md`).

*When it runs.* Once at the start of every A pass while the demo flag RAM $050F is set, after the real controller
was found idle (a newly pressed real button ends the demo instead). It builds the pass's $18 (held, **H** below)
and $19 (newly pressed, **P**) from zero, using the bit layout of 3.1: A $80, B $40, Select $20, Up $08, Down $04,
Left $02, Right $01. The bytes are used only by that A pass. The thrust bits B + Select ($60) mean "boosters"
because the demo pod has them ($DC = $C0).

*Inputs it reads.* The level $3B; the towed slot $66 (0 = nothing); the pod's position (x $0200/$021F, y
$025D/$027C, 16-bit pixels), horizontal speed ($02BA whole . $029B fraction), vertical speed ($02F8 . $02D9),
direction bits ($0336 bit 0 = moving left, $0355 bit 0 = moving down), facing $0374 (0-31, 1/32 turn clockwise
from up, `player.md` 2); the frame counter $5D; the states $0317+slot of slots 23-30; and its own memory, the
waypoint index $0510 and the timer $0511.

*Waypoint lists.* A small table maps each of the three demo levels (6, 4, 10) to the start of its list. Each entry is 5 bytes: x (16-bit, little endian), y (16-bit), radius
(byte), in pixels. There is no end marker.

| Level | List at | Entries | Counted by |
|---|---|---|---|
| 6 | ROM 0x30036 | 27 (indices 0-26) | first: x 1928, y 86, radius 64 |
| 4 | ROM 0x300BD | 25 (0-24) | first: x 1787, y 112, radius 64 |
| 10 | ROM 0x3013A | 36 (0-35) | first: x 2180, y 129, radius 64 |

The lists are stored one after the other, so an index past the end reads the next list (the level 4 demo ended
while on index 25, the first entry of the level 10 list); no demo went further. The waypoints are not the level's
placement data, only coordinates in its map. The index only ever grows, within a demo (it is set to 0 at set-up,
7.1).

*Quantities.* For the current waypoint (wx, wy, radius): dx = wx - x and dy = wy - y (signed); ax = |dx|, ay = |dy|;
D = ax + ay (the "distance"). `S` is the 16-bit sum of the two speeds as 8.8 numbers (horizontal + vertical,
magnitudes, wrapping at 16 bits). dx = 0 counts as "right", dy = 0 as "down" wherever a sign is tested.

*Rules.* They apply in this order in every A pass; H and P start at 0.

1. **Shield switch** (only while nothing is towed). The Down and Up bits are the pod's shield switch (`player.md`
   4.7). If the timer $0511 is not 0: its old value `t` falls by 1, and P gets Down if `t` >= 30. If the timer is 0:
   P gets Up.
2. **Waypoint reached.** If ax < 256, ay < 256, D < 256 and D < the waypoint's radius, the index goes up by 1 and
   rules 1 and 2 are run again at once for the new waypoint, in the same pass (so the timer falls again and Down or Up
   is added again, possibly several times if several waypoints are passed). Otherwise go on.
3. **Mode** (table below), from which way the pod is moving relative to the waypoint. The pod moves "toward" it on x
   when (dx >= 0) equals (not moving left), and on y when (dy < 0) equals (not moving down).

   | Condition | Mode |
   |---|---|
   | toward on both axes, S >= $0010, and S / 4 (rounded down) >= D | BRAKE |
   | toward on both axes, S >= $0010, and S / 4 < D | CRUISE |
   | anything else (including S < $0010) | FREE |

4. **Sector** of the waypoint, 0 = up, 1 = up-right, 2 = right, 3 = down-right, 4 = down, 5 = down-left, 6 = left,
   7 = up-left (clockwise). Write ax and ay as (high, low) bytes. The sector is found by the test below; "diagonal"
   takes the corner from the signs of dx and dy (down-right if dx >= 0 and dy >= 0; up-right if dx >= 0 and dy < 0;
   down-left if dx < 0 and dy >= 0; up-left if dx < 0 and dy < 0).

   | Case | Result |
   |---|---|
   | ax >= ay, and the high bytes are equal, and (low byte of ax) < 2 x (low byte of ay) | diagonal |
   | ax >= ay, any other case | horizontal: left if dx < 0, else right |
   | ax < ay, and ay >= ax2 | diagonal |
   | ax < ay, and ay < ax2 | vertical: up if dy < 0, else down |

   where ax2 is 2 x ax worked out one byte at a time: the high byte doubled modulo 256, the low byte doubled modulo
   256, with no carry from the low byte into the high byte. **Quirks:** the horizontal test only compares the low
   bytes and only when the high bytes are equal (so ax = 300, ay = 250 is "horizontal"); the vertical case is the
   reverse of what one would expect (a target nearly straight up or down, ay >= 2 ax, is classed as diagonal;
   dx = 0 gives diagonal up-right or down-right; vertical needs ay between ax and about 2 ax); and doubling ax
   loses its carry (ax = 200, ay = 250 gives ax2 = 144 and so "diagonal"). In effect: horizontal when ax is at least
   about 2 ay; vertical when ay is between ax and 2 ax (for ax < 128); diagonal otherwise. Verified outputs (dx, dy to
   result): (300, 250) right; (200, 120) down-right; (250, 100) right; (100, 150) down; (100, 200) down-right;
   (130, 200) down-right; (200, 300) down-right; (0, -50) up-right; (256, 255) right; (255, 256) down-right; (-200,
   -250) up-left.
5. **Wanted facing** from the sector and the mode (facing units of 1/32 turn clockwise from up):

   | Sector | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
   |---|---|---|---|---|---|---|---|---|
   | FREE or CRUISE | 0 | 4 | 8 | 12 | 16 | 20 | 24 | 28 |
   | BRAKE | 0 | 28 | 24 | 20 | 16 | 12 | 8 | 4 |

   (In BRAKE the wanted facing is the mirror image, about the vertical axis, of the facing in the first row, not the
   opposite direction: for sector 1 it is 28, not 20. Quirk.)
6. **Steering and thrust.** If the facing is not the wanted facing: H gets Right if (wanted - facing) mod 32 is below
   16, else Left; and thrust by case: $60 if something is towed; otherwise by level: none on level 4, $40 on level
   10, $60 on level 6. If the facing is the wanted facing:

   | Mode | H |
   |---|---|
   | FREE | $60 |
   | CRUISE | $40, unless `faster(1)` is true, then nothing |
   | BRAKE | $40, unless `faster(3)` is true, then nothing |

   `faster(t)` is true when **either** the horizontal whole speed is >= `t` and the facing points to the side the pod
   is moving toward on x (facing 1-7 or 9-15 when not moving left; 17-23 or 25-31 when moving left; **quirk:** never
   when the facing is 0, 8, 16 or 24), **or** the vertical whole speed is >= `t` (no direction test on the vertical
   side: another quirk). It makes the demo stop thrusting once it moves at 1 px per step (cruise) or 3 px per step
   (brake).
7. **Weapon pulse.** When $5D mod 32 = 0 (at that A pass), look at slot 23 + ($5D / 32) (slots 23-30 in turn). If its
   state is 6 or more: `f` = $80 (A) when ($5D AND $20) is set, else $A0 (A + Select). P is **replaced** by `f`
   (so any Up or Down from rule 1 is lost) and H gets `f` added.

*The pod's horizontal carry-in in a demo ($0D)* **[V]**. The autopilot uses the scratch bytes RAM $0C-$0F: at the start
of every A pass it loads the current waypoint (after any index increments of rule 2) as x low into $0C and x high
into $0D (and its y into the next two). The pod's thrust runs later in the same A pass (`player.md` 4.6) and reads
**$0D = the high byte of the current waypoint's x** (3 to 13 in the lists above; level 6 waypoint 0 has x = 1928,
so $07), not the star job's value and not 0. ($0C is overwritten by the gravity step before the thrust, so only $0D
matters.) Checked on all 6,728 thrust steps of the four demos in which nothing but the autopilot had written $0D
since the pass began: the byte equalled the high byte of the waypoint at the index in $0510 every time. Two
exceptions, both ahead of the thrust and described in `player.md`: a **sloped-terrain response** in the same step
(`player.md` 4.6 rule 1: 89 steps) and **towing** (`player.md` 8.3: 1,521 steps; where towing leaves the byte
untouched it is still the autopilot's). The star job's rule (`player.md` 4.6 rule 2, `graphics.md` 5.7.3) therefore
does **not** apply during a demo: the job still runs and writes $0D at the end of its passes, but the autopilot
overwrites it before the next thrust. In practice the thrust adds this constant to the horizontal accumulator $C4
each step ($07 per step on level 6 waypoint 0, $06 on level 4's) whenever the horizontal acceleration is not zero.

*Demo weapons.* The demo pod carries one special weapon, chosen by the demo's number and fitted by the set-up (RAM
$D1; shop numbers `economy.md` 6): demo 1 **homing missiles** ($D1 = 1; shot object state $0F; costs 4 gauge
units per shot, `economy.md` 6), demo 2 **star bullets** ($D1 = 8; parent state $12 with children of state $13;
32 units per shot), demo 3 **multi warhead** ($D1 = 6; state $14; 16 units per shot). The weapon energy gauge
starts full ($E000, 224 units) in every demo and is not carried over (it ended at $A0, $00 and $7F in the three
demos). The autopilot's only way to fire is rule 7: one press every 32 frames of $5D, when the slot in turn holds
a live object (state 6 or more). With Select held a fitted weapon replaces the plain bullet (`economy.md` 6), and
the autopilot holds Select (bit $20) in every boost ($60, $E0 ...), so **a plain pulse ($80) also fires the special
weapon whenever the pass also holds the boost bits**; the two kinds of pulse differ only in whether Select is
already held (rule 7 adds Select for slots 23, 25, 27, 29 regardless). Counts seen (new shot objects in
slots 3-10; a few charged shots show no new object in the frames sampled): demo 1 (4,459 frames) 15 homing
missiles seen and the gauge $E0 to $A0 in 16 steps of 4; demo 2 (7,299 frames) 6 star-bullet parents (68
children) seen, the gauge empty at frame 4,417 of the demo after 7 charges of 32, and plain bullets (state $08, 78
seen) between and after; demo 3 (6,715 frames) 8 multi warheads, with the gauge $E0 to $A0 over
the first four, a single rise by $1F at frame 2,027 of the demo (cause not identified, **[H]** a pick-up) and the
end value $7F. With the gauge at or below the cost the press fires a plain bullet (`economy.md` 6), as in
demo 2's last half. No other weapon (smart bombs, time bombs, anti-gravity) is used by any demo.

*Shield (timer $0511).* The Down and Up bytes above are the pod's shield switch (`player.md` 4.7: Down newly
pressed raises the probe shield, Up newly pressed lowers it). The timer is started in the pod's step (after the autopilot
in the same A pass), only while it is 0 and the demo flag is set, and then when either:

- while the 16 bytes RAM $A8-$B7 are **not** 0, 1, 2, ..., 15 in order **[V]**. They are the index table of the
  pod's terrain test (`levels.md` 6.4, window; `graphics.md` 5.7.3 calls its total length Z): one running offset
  per direction group of the segment lists the test collects from the 3 x 3 blocks around the pod. They read
  0, 1, ..., 15 exactly when the window holds no terrain segment at all (every list empty, Z = $10); a pod within
  about 16-32 pixels of a wall, floor or ceiling gives other values (e.g. `00 0A 0B 0C ...` for nine segments in
  group 1). The test rebuilds the table on every logic step in which the pod's terrain test runs (about once per
  2 frames in play); it is never built before the first gameplay, so it is all 0 at power-on, and it keeps its
  last values through the title and the story. In the four demos this branch fired 59 times in 44,000 frames
  (e.g. at frames 5147, 6565, 9041, 9107, 9259 of demo 1 and 14263, 14327, 15197 of demo 2: the pod beside a wall
  with the timer at 0), not only at the first A pass; the enemy test below fired for the other 128 starts (187
  starts in all, out of 3,020 passes with the timer at 0). So the shield goes up whenever the pod is near
  terrain or near an enemy, and stays up 32 A passes after the last such pass. **[V]** (counts from the program's
  own branches, 44,000 frames from power-on)
- when the object in slot 15 + (($5D >> 1) and 15) (slots 15-30, chosen by the frame counter) has a nonzero
  state and is near the pod: |dx| < 256, |dy| < 256 and |dx| + |dy| < 256, in pixels. **[V]** (80 samples, no
  exceptions).

It is set to 32. So after an enemy comes within reach the pod raises its shield for three A passes (Down at timer
32, 31, 30), the timer counts down by one per A pass while nothing is towed (it stands still while an item is
towed, and no Up or Down is sent then), and from timer 0 on Up is sent every A pass (lowering the shield again).

*Checks.* Demo 1 (level 6), first A pass (frame 5147): timer 0 and nothing towed, so pressed = Up; the waypoint
is up and slightly right, the pod is cruising upward (S = 1.5, D = 118, so S >> 2 < D) and wants facing 4 while it
faces 0, so held = Right + $60. Output $18/$19 of the first passes (frames 5147, 5149, ...): $61/$08, $61/$04,
$62/$04, $62/$04, $62/$00, $62/$00, $62/$00, $61/$00; the pod's step sets the timer to 32 after the first pass,
so it reads 32, 31, 30, 29, ... at the following passes. The waypoint index reaches 1 at frame 5190, 5 at 6106 and
10 at 7096 (demo 1); 5 at 15108, 10 at 16140, 15 at 17358, 20 at 18270 (demo 2), counting from power-on with no
input.

### 7.2 New game, continue, password

- **New-game state** (set by START GAME and on entering the password screen) **[C]**:
  - $3B = 0, Jetman lives $3E = 4, jetpods $3D = 4;
  - score 001000 (digits $CB-$D0 = 0,0,1,0,0,0);
  - $3C = $FF, $060E = $0626 = $0632 = $FF (empty markers of the level-memory records, section 9).
  - Equipment ($0600-$060D), $D1 and the fuel and crystal counts are not touched: they are zero because the RAM
    clear at power-on and reset clears them, and nothing in the title or attract leaves them set.
- **START GAME:** new-game state, then START LEVEL. **[V]**
- **CONTINUE:** the new-game state is set, then the password entry screen opens (`password.md`).
  - A valid password overwrites the fields it holds and goes to START LEVEL. **[V]**
  - An invalid one sets the new-game state again. With Start this begins a new game; with A or B the screen
    stays. **[V]**

**START LEVEL** **[C]**:

1. Set $060E, $0100 and $0101 to $FF.
2. Clear the demo flag.
3. System $EA = $3B / 3.
4. $3B = 0 goes straight to the approach screen. Any other $3B shows the interplanetary flight first.

### 7.3 Between planets

- **Interplanetary flight** (handler $B76E): the mothership crosses space (`scenes.md` 5, which gives the whole
  scene). A newly pressed A ends it at once **[V]**; otherwise it ends after 1,343 passes ($5E = 5, $5D = 64, after
  a four-step fade) **[V]**. Then:
  - odd $3B: the shop;
  - even $3B: the approach screen. **[V]**
- **Shop** (handler $B7EB, list mode):
  - The goods come from the system's list: bank 0 $8CEE, zero-separated lists of item numbers, one list per
    system.
  - Which items are offered, prices, buying and the money (the score, shown as "IN YOUR ACCOUNT"): `economy.md` 6.
    The screen's composition: `scenes.md` 6.
  - B leaves to the approach screen. **[V]**
- **Approach screen** (handler $B78A): planet number and name, gravity, diameter, the password for the current
  state, "PRESS START TO LAND", and the status panel (system number and name, lives, score). **[V]**
  - The password is computed here, and kept in $06EE-$06F9 for the game-over screen.
  - Any button lands.
- **Surface picture** (handler $B6D5, mode in RAM $A4; menu layout $A1, cursor $A5) **[V]**, durations **[V]**:
  - **The five modes** (the one handler serves all; every row **[V]** by watching $A4 in Mesen unless marked **[C]**):

    | $A4 | Name | Set by | Does |
    |---|---|---|---|
    | 0 | descent | the landing from the approach screen | the mothership descends onto the picture (below); music 3 starts with the screen. At the end: nothing owned or fitted: layout 2, cursor 1, straight to the launch scene; otherwise mode 2 |
    | 1 | leaving | the end of mode 4, or mode 3 with nothing left to do (below) | the mothership flies straight up with its exhaust (`scenes.md` 8.3); music 5. It ends in the first pass in which the mothership's y has gone below 0 (RAM $0287 bit 7 set): $3B + 1 (from 12: 10 + 1), then START LEVEL |
    | 2 | menu after the first arrival | the end of the descent, when something is owned or fitted | the planet menu, layout 0 (EXPLORE, EQUIP POD), cursor on option 2 (EQUIP POD) |
    | 3 | menu after docking | the dock transition (code 2) | the planet menu with the layout of 7.3 (below), cursor on option 1; the picture is set up again from the stored mothership position; music is not changed |
    | 4 | blast-off start | choosing BLAST OFF ($3C = $3B, and the picture is wiped) | a 16-bit counter (RAM $08/$09) starts at $0080 and rises by $20 per pass; it is the nametable address, offset from $2000, of the picture row redrawn in that pass, so rows 4 to 18 are redrawn one per pass from top to bottom (15 passes), erasing the menu box; when it equals $0260 music 5 and mode 1 start **[V]**. The 480 steps are address units, not frames |

  - **Modes 2 and 3 are the same menu**: both run the menu cursor and option code (bank 2 entry $39) and accept
    A. They differ only in (a) the way in, (b) mode 3 not calling the mothership motion routine, which in mode 2
    still runs but finds the mothership at its resting position, and (c) mode 3 able to turn into mode 1. After
    the menu (EXPLORE, EQUIP POD, BLAST OFF) both behave as described below.
  - **Mode 3 to mode 1 (automatic blast-off).** On entering the surface after docking, when the layout has bit 0
    set (this planet's flag is set: BLAST OFF would be offered) and none of the planet's items list entries 1 to
    count - 1 (RAM $0513 + index, count in $BA; entry 0 is not looked at) still has a collectable kind
    (kinds 3-9, 12, 15, 16: a table at bank 0 $D321 holds $FF for them and 0 for the rest; delivered or destroyed
    entries have kind 0), the mode is set to 1 at once and no menu is shown: the mothership leaves. **[V]**
    (flag set and all kinds cleared: mode 3 became 1 in the first pass of the surface handler, then $3B 0 → 1; flag
    set with the six items still present: stayed 3 with layout 3.)
  - **First arrival** (mode 0): the mothership flies in and lands (`scenes.md` 8.2: start point, speeds, braking,
    and the arrival pass per picture; the path is fully determined by the planet's picture number, RAM $05FF).
    A newly pressed A in any pass ends the descent at once (the mothership is put at its resting place; the launch
    scene or the planet menu follows) **[V]**. When nothing is owned or fitted the descent also includes 128 passes
    in which a small object rises from the mothership after it has stopped; when something is owned the
    descent ends one pass before the mothership's arrival pass (`scenes.md` 8.2, corrected in the ninth round). Then the launch scene and gameplay follow by themselves.
    **[V]** Frames from the landing button to the launch scene's handler being
    installed, including 35-39 frames of loading (22 + 13, section 1.1, plus a few for the picture's data), so the
    descent itself is that much fewer (arrival pass + 128 = 561, 757, 433, 595, 469, 720 passes for pictures 0, 1, 2,
    4, 5, 6, the planet 0 value agreeing with its 631 frames less 36 loading frames):

    | Planet | Frames | Planet | Frames | Planet | Frames |
    |---|---|---|---|---|---|
    | 0 | 631 | 5 | 468 | 10 | 467 |
    | 1 | 759 | 6 | 634 | 11 | 601 |
    | 2 | 504 | 7 | 794 | 12 | 598 |
    | 3 | 598 | 8 | 469 | | |
    | 4 | 794 | 9 | 504 | | |

    If anything is owned or fitted (a password can give that), the first arrival ends in the planet menu
    instead of the launch scene (layout 0, so no BLAST OFF). **[C]**
  - **After docking** (mode 3): the planet menu box is shown over the picture (its drawing: `text.md` 4.6). Cursor in RAM
    $A5 (1 = first option); layout in RAM $A1:

    | $A1 | Options |
    |---|---|
    | 0 | EXPLORE, EQUIP POD |
    | 1 | EXPLORE, EQUIP POD, BLAST OFF |
    | 2 | EXPLORE |
    | 3 | EXPLORE, BLAST OFF |

    Bit 0 of the layout is set when this planet's flag is set (BLAST OFF offered, `economy.md` 5.1). Bit 1 is set
    when nothing is owned or fitted: none of $0600-$060B, $D1, $D3 and $050E is non-zero (no EQUIP POD,
    `economy.md` 5.1).
  - **EXPLORE:** launch scene, then gameplay.
  - **EQUIP POD:** the equip list (handler $B7EB, equip mode) of items owned (RAM $0600-$060B nonzero); B
    continues to the launch.
  - **BLAST OFF:** the mothership takes off. When it has left the screen, $3B goes to the next planet (6), then
    START LEVEL. **[V]** Frames from choosing BLAST OFF to the start of loading (planets 0-12): 643, 731, 616,
    720, 291, 708, 643, 291, 708, 616, 708, 720, 720 (15 passes of mode 4, the passes of mode 1 and one more;
    `scenes.md` 8.3; planets 0, 1, 2, 3, 7, 8 measured, the rest from the verified model). An earlier list (468,
    505, 396, ...) was measured with the menu state poked from the first arrival, which leaves the mothership its
    landing speed; it is not the path a player sees.
- **Launch scene** (handler $B4B2): the pilot walks to the pod, the hatch closes and the pod rises out of the
  mothership (`scenes.md` 9: object timeline in passes). Every launch fills the fuel and weapon energy
  (`economy.md` 3.2). It lasts 358 frames **[V]**, or ends on any newly pressed button **[V]**.
- **Level start** (after the launch) **[C]**:
  - Set the camera and the player position from the level header (`levels.md` 3).
  - Load the level (header, terrain, palettes, objects).
  - Then continue as a reload (7.4, "Reload").
- **The last thing every level start does before the first pass [V]** (2026-10-20): the frame counter RAM $5D and $5E are set to 0, then the **drawing stage of
  the step is run once** (the routine of A-pass item 9 that eases the F gauge, ticks the guard $41, does the hit-pause bookkeeping and the bay test; on warp levels it builds the mesh tile; `economy.md` 3.1,
  "The extra step"), and only then is the gameplay handler installed. Consequences seen at the first gameplay pass of every replay: the F gauge's display copy RAM $05CE is 3
  after a launch (0 + one easing step), the guard $41 is 63 (a reload sets 64), RAM $98 already has the bay bit. After the first logic step: $05CE = 6, $41 = 62. Slot 1's flag bytes (RAM $0337, $0356) are not touched by a level start or a reload: 1 and 1 at the first gameplay pass after a launch from a normal start (the title's helper object left them), unchanged from the previous play after a level change or a reload (`player.md` 7.4).

### 7.4 Transitions out of gameplay **[C]** (each forced in Mesen by writing $3F: **[V]** for the scene reached)

Code anywhere in gameplay requests a change by writing a code to RAM $3F. At the end of the pass (A pass item 12,
B pass item 7), if $3F ≠ 0:

- **Guard:** if $3F equals the last transition ($40) and the guard $41 is not 0, the request is dropped and $3F
  cleared.
- **Demo:** all requests are dropped and $3F cleared.
- **Otherwise:** the default handler is installed, $40 = $3F, $41 = $FF (except for code 1, which leaves $41), $3F
  = 0, and the code is acted on (one table entry per code).

**The guard [V].** Every reload sets $41 = 64 and, on planets ($3B < 13), $40 = 2. In each A pass $41 decreases
by one until 0 (item 9 of 1.2; the level start runs that item once before the first pass, so the first A pass finds 63 and step n leaves 63 - n, 7.3), so docking (code 2) is refused for the first 64 logic steps (128 frames) after the
pod leaves the mothership. The value $FF set at a transition is overwritten by the reload that follows. The same
guard also refuses a **code 3** (the wormhole, `player.md` 9.2.1) in the first 64 steps after a reload that followed a
code 3, because $40 is then 3; on a planet the reload writes $40 = 2, so a wormhole is never refused there.
Who changes $41 besides the reload and the countdown **[C]** (found by listing every writer of the byte; none of the three
was run): Jetman alone touching a transporter ring on a planet clears it (an immediate dock request, `objects.md` 10); the
follow-on of a message screen clears it when the delivery left the pod in the bay area and the planet's flag is
set (the dock of item 4 in the list below); a kind-17 ring touched by an Italian-type pod (`economy.md` 2.1) sets it
back to 64 instead of teleporting while it is not 0. Nothing else writes it, so **on levels 13 and above nothing
clears the guard early**: a wormhole on a second level can be entered 64 steps after arrival, never sooner.

| Code | Meaning | Result |
|---|---|---|
| 1 | an object was pulled into the mothership | message screen for that object, see below |
| 2 | dock: the pod, or Jetman alone, back in the mothership | if the player state is not 1 (pod) or 5 (Jetman being killed), jetpods $3D + 1, the refund for the pod Jetman lost (`economy.md` 4) **[V]** (state 2: +1; state 1: no change); then the surface menu (7.3). Who may dock: `player.md` 6.4 |
| 3 | change level within a planet | see below; reload, the player placed at the new level's home point (not kept) |
| 4 | Jetman lost | final level: game over. Else if $3E = 0: game over. Else $3E − 1; 0 → game over, else respawn **[V]** |
| 5 | leave to level $DB | new $3B = $DB, except the pairs in the table below; a planet (< 13) → START LEVEL, else reload |
| 6 | badge intermission (handler $B33B) | then: back to the level as saved if $DB ≠ 0, else code 3. It ends when $5D reaches 255; the scene resets $5D itself when the badge lands, so the length is 255 frames from the landing whatever $5D was on entry (`economy.md` 5.4) **[V]** |
| 7 | backdrop interlude (handler $B50C) | saves the player object (RAM $05A6-$05AB), plays the bonus flight of `scenes.md` 11.1 (2,054 frames from the request), then code 6 with $DB = 1 |

**Code 3:**

- $3B < 13 → $3B + 13.
- 13-25 → $3B + 16 if the planet flag of $3C is set, else $3B − 13.
- 26-28 → no flag is tested and $3B + 16 is taken, giving 42-44: there are no such levels, so code 3 is never
  requested there **[C]** (reasoning).
- ≥ 29 → $3B − 29, **never testing a flag**: the return from a warp level (code 6 with $DB = 0, `economy.md` 5.4) always
  lands on the planet that the warp level belongs to **[V]** (level 29 to planet 0).
- Then $3C = the old $3B. On leaving a planet the mothership's fuel count RAM $0513 is saved in RAM $0636, and put back
  when the level is a planet again.
- **Where the player appears [V]** (Mesen, 2026-10-17: planet 0 to level 13 and back, level 13 to 29 and back to
  planet 0, planet 5 to level 18 and back; earlier text said "the position is kept", which is wrong for this
  transition). The new level is set up as a fresh start (`levels.md` 5.2, Appendix A), so:
  - **Second level (13-25) and warp level (29-41):** the player is at the new level's home point from its header
    (x = bytes 4-5, y = bytes 6-7 minus 3: level 13 (127, 41), level 18 (192, 109), warp levels (320, 253)); the camera is placed from
    the player position (`levels.md` 5.2: level 13 (0, 0), warp level (192, 160)).
  - **A planet (below 13), from the second level or from the warp level:** the same home point, then **the player's y and the camera's y
    are lowered by 128** (floored at 0; the x is unchanged). The pod therefore appears **128 pixels above the mothership's home
    point and falls onto it** under the planet's gravity: planet 0 (635, 63), planet 5 (1659, 447), planet 9 (home y 66 → 0, **[C]**, not run). The camera is placed from that
    position (planet 0: (480, 0); planet 5: (1504, 352)).
  - In all three cases the speeds are cleared (the vertical fraction is set to $40), the facing is 0, the fuel is
    **unchanged** (no refill), the two direction bits are whatever the previous level left (harmless at speed 0), the previous player
    state is kept (Jetman alone arrives as Jetman) and the pod's jetpod count and lives are unchanged. RAM $97 = 64 (a gate
    that ended directly in a code 3: the second-level and planet arrivals; **0** after the kind-6 screen and the warp-level arrival, and on the planet after the badge screen). The guard: $41 = 64 and $40 = 3 on levels 13 and above, $40 = 2 on a planet.
  - Everything in slots 1-30 is cleared and the level's own objects are placed again by the scan; the wormhole's list
    entry is back (the list is copied afresh).

Remap for code 5 (three parallel lists: destination, current level, resulting level) **[C]**:

| Destination $DB | From level $3B | Goes to |
|---|---|---|
| 0 | 27 | 4 |
| 1 | 27 | 5 |
| 2 | 27 | 6 |
| 0 | 28 | 8 |
| 1 | 28 | 9 |
| 21 | 3 | 10 |
| 5 | 0 | 7 |

Level 27 is reached by code 5 from planet index 2, level 28 from planet index 5; the triggers (the pod at a fixed
spot, for level 27 while four or more parts of a moving formation are present) are in `economy.md` 5.2. Levels 27
and 28 are junctions whose exits lead to later planets. **[C]**

**Reload** (after codes 3, 4, 5 to a non-planet, 6, the message screen and the map; the end of a
level start) **[C]**:

| Kept | Changed |
|---|---|
| player position (**not after a delivery**, code 1: rounded, see "Return from the message screen" below; kept exactly after code 4 [V]; **not kept after code 3: the home point of the new level, see "Code 3" above**); camera position (placed again from the player position and rounded down to a multiple of 32 for the full redraw: `levels.md` 5.2, the one owner of the rule) | objects of state 6 in slots 1-30 are removed |
| | codes 3 and 5 clear every object slot except 0 (the placement scan starts again) |
| | code 4 (respawn) clears slots 3-10 (shots), keeps the other objects, and puts the player in state 7 (new pod arriving) |
| | the video buffers are emptied and the transition guard set (above) |
| | the screen is redrawn, the level music restarts, $5D/$5E are set to 0, the rotating job starts at 8 |

The player's "home" (where state 7 brings the new pod) is for the physics spec.

**Message screens** (handler $B60B, after code 1; the warp-level introduction and the final message use the same
handler). RAM $DB holds the kind and $DA the timer. One table for all kinds (pictures: `scenes.md` 10; the item
kinds that give kinds 1-5 are in the next table; "follow-on" is the list "What follows a message screen" below):

| $DB | Shown for | $DA at the start | Text and picture | Next |
|---|---|---|---|---|
| 1 | a delivery: shields, boosters, alien items, navigators, fuel and everything else | 240 | the item's message; a tile picture or the item's own sprites (`scenes.md` 10) | follow-on |
| 2 | a crystal | 240 | message 25, or 50 for the seventh | follow-on |
| 3 | analysis of a sample | 240 | message 27 + the analysis text (28-44) | follow-on |
| 4 | radioactives | 240 | message 46 | follow-on |
| 5 | a warpship piece | 240 | message 47, or 48 for the last | follow-on (a complete ship goes to the final level) |
| 6 | the warp-level introduction (not a delivery; section below) | 192 | one full-screen picture with sprite lettering, no text | code 3 |
| 7 | completion of the final level | 255 (plus the 64 idle frames: **319 frames in all; the remake always shows it for 319**, the original varies, `behaviour.md` Q33, `docs/deviations.md`) | message 23, **shown whole on its first frame, not typed** (`text.md` 3, `scenes.md` 14.1) | the ending |

The kind of a delivery depends on the delivered object's item kind (RAM $03B2 of that slot; `economy.md` 2.1 and
2.3 for the full effects) **[C]**:

| Item kind | Effect | Text id | $DB |
|---|---|---|---|
| 3 | probe shields: $DC bit 7 set | 7 | 1 |
| 4 | probe boosters: $DC bit 6 set | 8 | 1 |
| 5 | alien items (value 1000) | 45 | 1 |
| 6 | navigators: $060C = 1 | 9 | 1 |
| 9 | analysis of a sample (list and cargo, `text.md` 3, 4.4) | 27 + 28..44 | 3 |
| 10 | fuel for the mothership (`economy.md` 3.4) | 21 or 22 | 1 |
| 12 | radioactives (value 750) | 46 | 4 |
| 15, 16 | crystal (`economy.md` 2.4): 500 points, or at the seventh a bonus life, jetpod and 2000 points | 25, or 50 | 2 |
| 19 | warpship piece: this planet's flag set | 47, or 48 if it was the last | 5 |
| others | | 6, then 6 + type | 1 |

The final level's completion sets $DB = 7 and shows text 23 (below).

**Return from the message screen (after code 1) [V].** The pod is not placed at a fixed offset from the camera. Before
the level is redrawn the original rounds the player's position **down to a multiple of 8 on both axes and clears the
x and y sub-pixel fractions**: x = x AND $FFF8, y = y AND $FFF8 (only the low bytes change), and the camera's low
bytes are rounded down to a multiple of 8 as well; the camera is then placed from the new position (`levels.md`
5.2: x − 128 and y − 80, then down to 32). The speeds, directions, fuel and facing are kept. Checked in Mesen on
`tools/mesen/replays/delivery.replay` (pod x $29F.B1, y $7C.5A before; $2A0.00, $78.00 after, camera $220, $20)
and on three more runs with the pod moved by (+13, −21), (−30, +9) and (+5, +30) at step 2010 (it drifted a few
pixels before the transition): after the reload x = $2A8, $280, $2A0 and y = $60, $80, $98, always multiples of 8
with fractions 0. The "camera + (128, 88)" that fits the
first run is a coincidence of rounding (the first moved run gives x $2A8 against camera $220 + 128 = $2A0). Respawn (code
4) does not round (checked on `respawn.replay`: $32C.97, $E3.A6 before and after). Other transitions: not checked.

**Kind 6, the warp-level introduction** **[V]** (the screen, its timing and what follows, by forcing the end of the
gate state in Mesen; the trigger **[C]**). It is not a delivery. It is shown when the level-change gate (player
state 9, `player.md` 9.2; the wormhole of `economy.md` 5.3) ends on a level 13 or above and the current planet's
flag is set (`economy.md` 5.1; on a second level that is the flag of $3C):

- With the flag clear, or on a planet (level below 13), the end of the gate state requests code 3 at once and no
  screen is shown.
- Otherwise the gate's end clears the gate's list entry and the player's speeds (`player.md` 9.2), rebuilds the
  screen and installs the message handler with $DB = 6 and $DA = 192. The picture is one full-screen tile picture
  (listed in `graphics.md` 4.5; palette preset 21 ($15)): no printer text, no
  status panel. Its lettering is drawn by sprites, the layout of an object in slot 11 (x = 128, y = 160, frame $9E;
  30 sprites, `scenes.md` 10.1). No item is awarded and nothing is released. See `scenes.md` 10.
- **Shake:** the screen's vertical scroll (RAM $03) alternates between two values 2 pixels apart on every frame.
  **[V]** (screenshots in successive frames).
- **Timing:** the kind-6 handler adds 1 to $5D itself each frame and does not run the sprite-0 split or the random
  generator (3). $DA counts down only on frames where the low byte of $5D is 64 or more, and the button test is
  made only then too. Other message screens start with $5D = 0, so their first 64 frames are the idle ones; kind 6
  does not reset $5D, so its idle 64 frames come at the next wrap of $5D (0 to 63), if the screen is still up.
  So $DA falls on a frame when $5D, after the handler's own increment, is 64 or more. The screen ends when $DA
  reaches 0 (192 counted frames), or on a newly pressed button once $DA is below 64 **[C]**.
  The remake's length is decided: always 256 frames (`docs/deviations.md`, "Warp-level intro length"); the dependence on
  the gameplay frame counter is a bug of the original, and the remake has no load frames to reproduce it anyway. With $5D = `s` before
  the first pass it lasts 256 frames when `s` >= 64 (192 counted and 64 idle) and 255 - `s` frames when `s` < 64.
  **[V]** for `s` = 166: 256 frames ($DA = 192 at the first frame, 103 at the wrap of $5D, 0 at the end; the earlier
  figure of 254 was a measuring slip).
- **Then:** code 3 (level swap, 7.4): level + 16, a warp level (29-41), as `economy.md` 5.3 says. **[V]** (level
  13 to 29).

**Timing and exits, kinds 1-5 and 7** **[V]** for the typing, **[C]** for the rest (kind 6 has its own rule, above):

- **Timing:** typed text appears at one character per 4 frames (`text.md` 2.3). Nothing else happens for the first
  64 frames. Then $DA counts down one per frame. (Exactly: it falls on frames where the low byte of $5D is 64 or more, as for
  kind 6 above; the screens that start with $5D = 0 therefore idle 64 frames first. Kind 7 does not reset $5D, so its length
  depends on the start value: `scenes.md` 14.1, Q33. **[V]** 2026-10-17, DA read every frame.) The screen ends when $DA reaches 0, or on a newly pressed button
  once $DA is below 64. **[V]** for the typing, **[C]** for the rest.
- **What follows a message screen.** The object table is restored from the copy made when the screen began (so the
  player state is the one from before the screen). Then, in this order:
  1. kind 6: code 3 (level swap, 7.4);
  2. kind 7: the ending;
  3. the warpship is complete (all twelve planet flags set, RAM $DE = $FF and $DF = $F0; **this is tested after every
     message screen of kinds 1-5, not only after a warpship piece**, so a game restored by a password with all flags set
     goes to the final level after its next delivery of anything **[V]** (a crystal delivered with the flags poked:
     message kind 2, then the final level): the final level, "Entry to the final level" below;
  4. RAM $98 bit 5 is set (the pod was inside the mothership's bay area when the object was delivered,
     `economy.md` 2.3): **dock** (code 2) if the restored player state is not 1 (Jetman on his own), and, if it
     is 1 (the pod), only if the current planet's flag is set; with the pod and the flag clear nothing docks and
     play resumes. **[V]** (all three combinations forced in Mesen: pod with flag clear: reload; pod with flag set:
     dock; Jetman alone with flag clear: dock);
  5. otherwise: reload, back to gameplay.

**Final level:** $3B = 26 with RAM $9A bit 7 set. Lives and jetpods are set to 0, so losing Jetman is game over. How the
player gets there: "Entry to the final level" below.
When it is completed (the camera route has ended and none of the object slots 23-27 holds an object in state 6 or
more, and RAM $5D AND $1E is 0 **[V]**, played naturally 2026-10-17: `behaviour.md` 4.16), message 23 is shown (kind 7, 319 frames, song 32 requested), then the ending (handler $B466, 902
frames, no input), then the credits (handler $B393, with the Space Loonies badge, 2,274 visible frames (2,271 in the first run, `scenes.md` 14.3), no input; text in
`text.md` 4.7). The end of the credits restarts the program (warm): 14 blank frames, then the title. **[V]** by
forcing the end routine on planet 6 (`scenes.md` 14); the trigger was also played through naturally (`scenes.md` 14.1).

**Entry to the final level [V]** (Mesen, 2026-10-17: the planet flags poked to all set and a crystal delivered on planet 0;
a warpship piece delivered on planet 12 with only planet 10's flag missing; the follow-on code entered directly).
Reached from the follow-on of a message screen (list above, item 3). The game does, in this order:

1. **Scenes:** none. There is no flight, no approach screen, no surface picture, no launch scene, no mothership and no
   shop: the level is built at once and play starts. (The routine for a level start skips the launch when RAM $9A bit 7 is
   set.) Between the end of the message screen and the first gameplay frame there were **29 frames of loading** with the
   screen blank (the same kind of gap as every level start; the remake needs none).
2. **State set:** lives RAM $3E = 0 and jetpods $3D = 0; the fitted special weapon $D1 = 0; $E0 = 0; $9A = $80; $3B = 26;
   the video job counter $60 = 9. **Not touched by the entry:** score, the pod type and its upgrades (RAM $DC), the bullet upgrade, the fitted
   engine/shield item $D3, the planet flags. The probe shield is off ($65 = 0).
3. **The player:** state 6 (`player.md` 9.1) at the header home (x 96, y 1024 − 3 = 1021), facing 0 (frame 0), horizontal
   speed 0, **vertical speed 1.50 up** (the direction bit up), the invulnerability counter $97 = 0. The camera is the header's (0, 896), not a pod-based one; the route camera takes over (`levels.md` 5.2). The fuel high
   byte is **full ($E0)** whatever it was (the same refill as every level start); the gauge fills up on screen as on any level.
4. **HUD:** the lives and jetpod icons are not drawn (both counts are 0; video job 9 blanks the 3 × 3 tile area of the
   icons on the first frame); the fuel gauge fills up, the weapon gauge and the score stay.
5. **Music:** song 33 (`sound.md` 8.1) starts at the end of the loading, one frame before the first gameplay frame
   (the current-song byte went from 8 to $21 there).
6. The wormhole, the shop and docking do not exist on this level; the guard and all later rules are those of level 26
   (`levels.md` 5.2, `behaviour.md` 4.16, `player.md` 9.1).

**Game over** (handler $B634) **[V]**:

1. An animation of about 240 frames (the pass waits until $5D ≥ 240).
2. "YOUR LAST PASSWORD WAS" and the password last shown on an approach screen (handler $B5CD).
3. Any newly pressed button restarts the program; this is a warm reset, so straight to the title.

With 0 lives (possible from a password) the first loss of Jetman is game over. **[V]**

## 8. Fidelity notes for the remake

Keep, as game logic:

- the A/B gameplay schedule and the order within each pass;
- input sampling per scene;
- the random generator's inputs and step points, including pause;
- the guard on repeated transitions;
- the scene order above.

Drop (`docs/deviations.md`) the hardware timing:

- overrun losses (1.4);
- loading frames (1.1; the row "Loading frames" of `docs/deviations.md`);
- the rotating video jobs as a bandwidth measure;
- the busy-waits that place work relative to the raster: the status-bar split waits for the sprite-0 hit; many
  scenes start their pass with a fixed delay of up to about 9,000 CPU cycles (about 80 scanlines); the gameplay
  handler pads with a short delay when no map row is streamed.

## 9. RAM map (globals found)

Scene-local scratch ($04-$17, $0C-$0F as sub-states and timers) is reused by every scene and is not listed as
state. [V] where the meaning was confirmed by watching RAM, else [C].

| RAM | Meaning | Status |
|---|---|---|
| $00, $01 | PPUCTRL and PPUMASK shadows | C |
| $02, $03 | horizontal and vertical scroll | C |
| $0C-$0F | scene sub-state and timers (title sub-state, password cursor $0D and repeat timer $0E, ...) | V |
| $18, $19 | controller 1 held / newly pressed | V |
| $1A, $1B | controller 2 held / newly pressed | C |
| $37 | planet diameter shown on the approach screen | C |
| $3B | level index 0-41 | V |
| $3C | previous level index, $FF after a new game | C |
| $3D | jetpods (pod icons in the HUD, at most 4 drawn). Read only by the HUD; it never limits play (`economy.md` 4) | V |
| $3E | Jetman lives (person icons in the HUD); a loss with 0 or 1 left is game over | V |
| $3F | transition request (7.4) | V |
| $40 | last transition code | V |
| $41 | transition guard (7.4) | V |
| $4C-$4F | camera position: x low, x high, y low, y high | C |
| $52, $53, $55, $56 | video buffer: byte count, flag, PPU address high, low | V |
| $5A, $5B, $5C | random generator | V |
| $5D, $5E, $5F | frame counter, low to high | V |
| $60 | rotating video job 0-7 (8, 9 on a level's first frame); also mixed into the random generator; advances as ($60 + 1) AND 7 after the frame's job has used the old value; the random generator step of that frame sees the advanced value (5) | V |
| $63 | bit 7: shop list mode (clear: equip-pod mode) | C |
| $66 | slot of the object held by the tractor beam (`objects.md`) | C |
| $6F | planet gravity shown on the approach screen | C |
| $75 | value last written to select the bank | C |
| $76-$95 | palette shadow (`graphics.md`) | V |
| $98 | sprite attribute bits ORed into the pod's sprites: bits 0-1 palette (flash), bit 5 behind the background = the pod is inside the mothership's bay area (`economy.md` 2.2); readers and writers in 9.2; cleared at every reload | C |
| $9A | level flags: bit 7 final (Golden Warpship) level, set together with the forced scroll and other final-level effects; bit 6 the camera's route is finished (`levels.md` 5.2) | C |
| $9B | level flags: bit 7 selects an alternative HUD/scene mode | C |
| $A1, $A4, $A5 | planet menu layout, surface mode (0-4, 7.3), menu cursor | V |
| $C6, $C7 | fuel, 16-bit (high byte $C7 shown by the "F" gauge through $05CE); full $E000, refilled at every launch (`economy.md` 3.2) | C |
| $C8, $C9 | weapon energy, 16-bit ($C9 shown by the "W" gauge); refilled at every launch | C |
| $CB-$D0 | score, six decimal digits, most significant first (also spent in the shop) | V |
| $9A | level flags: bit 7 final level (set on entering level 26; one effect is the forced scroll along the route), bit 6 route finished (`levels.md` 5.2) | C |
| $D1 | an equipment item fitted to the pod | C |
| $DA | message screen timer | V |
| $DB | message kind, or destination level for transition 5 | V |
| $DC | pod: bits 0-1 pod type (0-2), bit 6 boosters, bit 7 shields | V |
| $DE, $DF | planet flags (warpship pieces delivered), section 6 | C |
| $E0 | crystals delivered toward the next bonus (0-6; the seventh gives it, `economy.md` 2.4). On the final level the same byte is the camera's route step (`levels.md` 5.2) | C |
| $EA | system number 0-4 | C |
| $F3-$F6 | two queued sound effect requests; $F7 a countdown that discards them | C |
| $0100, $0101 | list indices of two placed objects remembered by an object behaviour; $FF = none. Set to $FF by START LEVEL and level start (these bytes are in the stack page, so the RAM clear does not touch them) | C |
| $0102-$0141 | video buffer data | V |
| $0200-$0441 | the object table: 31 slots × 18 fields (`objects.md` 2.1) | V |
| $0317-$0335 | object **state** (`objects.md` 2.1, 2.4); for slot 0 the player state, 9.1 | V |
| $0393-$03B1 | object **list index** (`objects.md` 2.1; a work byte for objects not from the placement list) | V |
| $050A, $050B | NMI handler address (identifies the current scene, 1.1) | V |
| $050F | demo flag ($80 during an attract demo; stays set back at the title until a game starts) | V |
| $0510, $0511 | demo waypoint index and tractor-beam timer | C |
| $0512 | attract counter | V |
| $0513 | the mothership's fuel count on this planet (`economy.md` 3.4); the first byte of the copy of the items list (`economy.md` 2.1) | C |
| $0596-$059A | text printer resume state (`text.md`) | V |
| $05A6-$05AB | saved player object during transition 7 | C |
| $05CE | display copy of the fuel for the "F" gauge, easing toward $C7 (`economy.md` 3.1) | C |
| $05FF | surface picture index (`graphics.md`) | C |
| $0600-$060D | shop items 0-13 owned, one count each (up to 9; `economy.md` 6); $060C, $060D the Mapping Device and Super Mapping Device. Jetpods are not counted: buying one sets the pod type in $DC | C |
| $060E; $0626-$0631, $0632-$0635 | records of which placed objects of a level were collected or destroyed, kept between visits; $FF = empty. $060E: the revisit record; $0626-$0631: placed-object presence of the last level left among 0-12 (12 bytes), $0632-$0635: of 13-25 (4 bytes; `objects.md` 3.6) | C (H for the meaning) |
| $0637 | current music number | C |
| $06EE-$06F9 | the 12 password letters (ASCII) | V |
| $06FA-$06FF | warm-reset signature | C |
| $0700-$07FF | sprite table copied by DMA each frame | V |

### 9.1 Player states (RAM $0317, object slot 0)

Slot 0's state field (`objects.md` 2.4) holds the player state, values 0-10. The full table, with what each
state does, the transitions between them and the pause rule, is `player.md` 3 (6 final-level flight, 7 respawn,
8 Jetman assembling a spare pod, 9 level-change gate, 10 the backdrop interlude). Seen in Mesen with screenshots at
every change during random play on planet 1: 1, 2, 4, 5, 7 **[V]**. Points that matter to the scene flow:

- State 1 is also the state a new pod starts in; the demo ends on state 2 (7.1).
- State 5 lasts 96 logic steps (192 frames, `player.md` 7.2) before transition 4. An earlier measurement of about
  210 frames included the loading frames of the respawn (section 1.1).
- State 9 is entered when a level-change gate (wormhole) is touched and the guard (7.4) lets the request pass; the
  request itself is then withdrawn (RAM $3F cleared) and the gate state does the waiting. If the guard refuses (a
  request for code 3 within 64 steps of the last reload after a code 3), the touch is ignored and nothing happens.
  **[V]** (`player.md` 9.2.1; the same two sentences were once written the other way round in `player.md` 3) The previous state is kept in RAM $69 and restored when the gate animation ends (`player.md` 9.2, and
  section 9.2 below).

- **Hit box:** the half-height used by the terrain test is a variable of its own (RAM $F2), not a property of
  the state. How it is set and grows (6 for a pod, growing to 9 while the pod explodes and kept by Jetman, 12 with
  the shield on) is in `levels.md` 6.4, step 2. **[C]** (it is set to 9 at level
  start only when the state is 2-4, else 6, and grows by one per logic step up to 9 while nothing is touched)
- **Pause:** allowed in states 1, 2, 3 and 6 (table bank 0 $BE0E; `player.md` 3). **[C]**
- **Dock (transition 2):** adds a jetpod unless the state is 1 or 5. **[V]** for states 1 and 2
- **Launch scene:** the launch is a separate scene (handler $B4B2) and uses slot 0 for its own animation. The
  value 2 seen there is that scene's own use, not a gameplay state. **[V]** (2 during the launch scene, 1 from
  the first gameplay frame)

**For state replays:**

- **Scene:** $050A-$050B.
- **In gameplay:** $3B, $3D, $3E, $CB-$D0, $DC, $DE-$DF, the object table ($0200-$0441), the camera $4C-$4F, the
  random generator $5A-$5C, and $5D-$5F, $60.
- **Sampling:** once per completed logic step (1.4).

### 9.2 Two shared bytes: RAM $98 and RAM $69 **[C]** (functions [V] where marked)

**RAM $98: the pod's sprite attribute bits.** The pod's sprite drawer ORs it into the pod's sprite attribute bytes
(the second drawing path of the pod masks bit 5; `objects.md` 6.4).

| Bits | Meaning | Written | Read |
|---|---|---|---|
| 5 | the pod is behind the background, because it is inside the mothership's bay area | by the bay test in the pod's step (A-pass item 9), every logic step: set while the pod's position is inside the bay rectangle (`economy.md` 2.2), cleared otherwise; not touched while the pod holds an item in the strip where the delivery test runs instead | by the sprite drawer, and at the end of a message screen: bit 5 set means "dock" under the rule of 7.4, "What follows a message screen" (item 4) **[V]** for the effect, **[C]** for the code |
| 0-1 | palette bits of the pod: the colour flash of the momentum killer, of anti-gravity and of the gate state, and the hit pause's palette 1 | five writers, listed in "Bits 0-1: every writer and their order" below; cleared when an effect ends | by the sprite drawer (ORed into the pod's and Jetman's attribute bytes; not into the shield ring, the beam or the flame) |

The whole byte is cleared at every level start and reload and in a few other set-up paths. Bit 5 means "in the bay
area" and nothing else.

**Bits 0-1: every writer and their order [V]** (2026-10-20: `antigravity`, `momentum-killer`, `tow`, `wormhole` and eight more replays, every step; the original's OAM attribute bytes of the pod read back from the step pictures, and the writers confirmed in the program). The palette of the pod's four sprites in a step's picture is the layout's own palette (0) ORed with these two bits, as they stand at the end of the step's A pass (the B pass draws from them). Within one A pass the order is:

1. **Start of the player's step:** if anti-gravity is on (RAM $D2 bit 7) and the weapon gauge runs out, bits 0-1 are cleared (`player.md` 4.1, gauge drain).
2. **The player state's own step** (`player.md` 3), in the order of the pod's step (`player.md` 4.2):
   - *Momentum killer fitted* (RAM $D3 = 9), pod state 1: bits 0-1 are **cleared when B is held**, otherwise set to 0 or 2: bit 1 = bit 2 of the frame counter RAM $5D as the A pass starts (the counter is even in A passes), bit 0 = 0. So palette 0 on two steps, then palette 2 on two steps, and so on, with the counter reset to 0 at every level start: the step k since the level start (k = 1, 2, ...; the counter is 2 (k - 1)) has palette 2 when (k - 1) mod 4 is 2 or 3, else 0. **[V]** 599 of 599 steps in `momentum-killer.replay` (palette 2 on steps 3, 4, 7, 8, 11, 12, ...; 0 on steps 1, 2, 5, 6, ... and during every step with B held).
   - *Anti-gravity on* (RAM $D2 bit 7 set) and the fitted special weapon is anti-gravity ($D1 = 2): bits 0-1 := bits 1-2 of the counter, i.e. (RAM $5D >> 1) AND 3 = (k - 1) mod 4: **all four palettes in turn, one per logic step** (0, 1, 2, 3, 0, ...), written after the momentum killer's bits (so it wins if both are fitted). **[V]** `antigravity.replay`: palette 1 on steps 6, 10, 14, ..., 2 on 7, 11, ..., 3 on 8, 12, ..., 0 on 9, 13, ... (the switch-on press was in step 4, which clears the bits; step 5 gives (5 - 1) mod 4 = 0). If the weapon is not anti-gravity the on-bit is cleared instead and the bits are not written. The orange pod of step 6 is only one of the four phases.
   - *Gate state* (player state 9): both bits flip (XOR 3) every step, from the value they had: 0, 3, 0, 3, ... **[V]** `wormhole.replay` (`player.md` 9.2.3). On the step that ends the gate state the bits keep the value the flip left (invulnerability starts in that step, so the drawing stage below clears them from the next step on).
   - *Switching anti-gravity by the A + Select press* (`player.md` 10.1, any of the weapon values 2, 3, 7): bits 0-1 are cleared on the step of the press (the press is handled last in the pod's step, so the step's picture shows palette 0; the flash starts on the next step).
   - *Interlude state* (10): the whole byte is cleared every step. *End of the pod's explosion* (state 4 to 2, `player.md` 7.1): the whole byte is cleared. Level start and reload: cleared (`game-flow.md` 7.4).
3. **The drawing stage of the step** (the per-step routine after the objects' step, the one that also eases the F gauge (`economy.md` 3.1) and tests the bay (bit 5); it runs in every player state, the gate state included; in state 9 only its first part, the object placement and the random spawn test, is skipped **[V]**: the gate state's bits stayed 0 while $99 was 4, 3, 2, 1 in `wormhole.replay`): **if the hit-pause counter RAM $99 is not 0**, bits 0-1 are replaced: the counter is decremented first, and bit 0 := 1 when the *new* value is 1F or more (equivalently: it was $20 or more and does not become 0), bit 1 := 0 in every case; bits 0-1 are therefore 0 on every step of the invulnerability timer's `$99 = ($5D AND 7) + 1` mode (`objects.md` 6.4) and while $99 is $1E or less. If $99 is 0 at that point the flash bits of step 2 are left alone. Bit 5 is then set or cleared by the bay test (bit 0-1 untouched).

So in one step the palette flashes of steps 2 are visible only when $99 was 0 at the start of the drawing stage; a hit pause (`player.md` 5.4) or an invulnerability period (RAM $97 non-zero) overrides them completely.

**What a step's picture shows for a hit pause [V]** (`tow.replay`, steps 1098, 1231, 1649 and 1,800 others): with $99 as logged at the end of the step (after this step's decrement), palette 1 for $99 of $1F to $2B and palette 0 below; at $1F palette 1 still shows (the bit was written from the new value $1F), at $1E not. In terms of the value *before* the step's decrement: palette 1 when it was $20 or more. A hit that sets $99 = $30 (48) in step n therefore shows palette 1 from step n on (the decrement of the same step leaves 47 = $2F) down to the step that leaves $1F (step n + 16), and palette 0 from the step that leaves $1E.

**RAM $69: two uses that never overlap.**

| Use | While | Written | Read |
|---|---|---|---|
| rope angle, 0-127 (`player.md` 8.2), with the fraction in $68 | an item hangs on the tractor beam ($66 ≠ 0) | by the catch (when the item is caught) and by the swing (every step) | by the push that releases the item when the pod is destroyed, the rope's sprites, the crystal-spin counter (`economy.md` 2.6) and the tow-rope sound effect (`sound.md`) |
| the pod's or Jetman's previous player state, restored when the gate state ends (`player.md` 9.2) | player state 9 | by the gate touch | by the gate step (while it is 1, the pod's facing frame cycles 0, 2, 4, 6 with $5D; otherwise the facing is left alone) and by its end (copied back into $0317; then $69, $67, $68 and the speeds are cleared) |

They cannot meet: the wormhole does nothing for a pod that is towing (`economy.md` 5.3), so $66 = 0 in the gate state.
$68 is shared the same way (rope fraction, gate counter, cargo number).

## Open questions

- What exactly triggers transitions 1, 5, 6 and 7 in play: the object and level specs name the objects or places
  that request them (`economy.md` 2.2, 5.2-5.4). The remaps for levels 27-28 are in 7.4.
- Whether an overrun can ever hit before the status-bar split (would rerun an A pass).
