# Password

The password the original shows on every approach screen and after a game over, and accepts from the CONTINUE
option of the title menu: the alphabet, how the 12 letters encode 48 bits, which game state they hold, the
checksum, the validation rules, and what entering one restores. The password encoding is a mechanic, so it may be
committed (`docs/CLEAN_ROOM.md`, rule 5).

Status: **[V]** verified in Mesen (passwords typed into the original with scripted input and the resulting RAM
read back), **[C]** read from the program, **[H]** hypothesis. RAM names are in `game-flow.md` section 9.

## 1. Alphabet **[V]**

Sixteen letters, each standing for a 4-bit value (table at bank 1 $98E1, ROM 0x098E1):

| Letter | B | D | G | H | K | L | M | N | P | Q | R | T | V | W | X | Z |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Value | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |

The entry screen lets the player choose any of A-Z for each place. A letter outside this alphabet makes the
password invalid.

## 2. Layout: 12 letters, 6 bytes **[V]**

The password carries six bytes, P0 to P5. **Letters 1-6 are the high nibbles of P0..P5, letters 7-12 the low
nibbles of P0..P5:**

```
letter  1  2  3  4  5  6     7  8  9 10 11 12
nibble P0 P1 P2 P3 P4 P5    P0 P1 P2 P3 P4 P5
       high nibbles          low nibbles
```

## 3. What the bytes hold **[V]**

The score is six decimal digits S1..S6, most significant first (RAM $CB..$D0; the new-game score 001000 has S3 =
1). Each nibble below holds one digit or one field.

| Byte | High nibble | Low nibble |
|---|---|---|
| P0 | Jetman lives (RAM $3E), 0-15; values above 15 are saved as 15 | score digit S3 |
| P1 | score digit S6 | score digit S2 |
| P2 | (whole byte) level index × 4 + mapping device × 2 + super mapping device (see below) | |
| P3 | (whole byte) checksum, section 4 | |
| P4 | score digit S4 | pod field (below) |
| P5 | score digit S5 | score digit S1 |

**P2:** level index RAM $3B in bits 2-7 (0-63), RAM $060C (owns the MAPPING DEVICE) in bit 1, RAM $060D (owns
the SUPER MAPPING DEVICE) in bit 0. Both bytes are used as stored, with no masking; in practice they are always 0
or 1 (a device can be bought only when none is owned, `economy.md` 6.3; the navigators set 1).

- **[C]:** a value of 2 or more would spill into the neighbouring bits; the game never produces one.
- They are the Mapping Device and Super Mapping Device owned flags (`economy.md` 6.5). Delivering the navigators
  also sets RAM $060C to 1 (`text.md`, message 9).

**Pod field** (the low nibble of P4) holds four bits of RAM $DC:

| Pod field bit | From RAM $DC |
|---|---|
| 3-2 | bits 1-0: pod type, 0-2 |
| 1 | bit 7: probe shields |
| 0 | bit 6: probe boosters |

So `pod field = (pod type × 4) + (shields × 2) + boosters`, and decoding gives back
`$DC = (shields × $80) + (boosters × $40) + pod type`. Bits 2-5 of $DC are not stored and come back as 0.

## 4. Checksum **[V]**

All arithmetic is on bytes, keeping the low 8 bits:

```
t     = (P0 XOR P1) + P2          # 9-bit result: c = 1 if it exceeded 255
P3    = ((low 8 bits of t) XOR P4) + P5 + c
```

## 5. Encoding: when and from what **[V]**

The password is computed on every approach screen from the state at that moment ($3B already set to the planet
being approached). The 12 letters are written to RAM $06EE-$06F9 (ASCII codes) and printed after "PASSWORD". They
stay in RAM until the next approach screen. The game-over screen shows them as "YOUR LAST PASSWORD WAS". **[C]** that
nothing else rewrites them.

## 6. Decoding and validation **[V]**

When A, B or Start is pressed on the entry screen:

1. Each of the 12 letters is looked up in the alphabet; a letter not found makes the password invalid.
2. P0..P5 are rebuilt (section 2).
3. The checksum must match the formula of section 4 (including its carry `c` from the first addition).
4. Every score nibble must be a decimal digit 0-9: both nibbles of P1, of P5, the low nibble of P0, and the high
   nibble of P4.
5. The pod type (bits 3-2 of the pod field) must not be 3.

**Not checked:**

- the lives nibble: 0-15 all accepted; 12 was verified;
- the level index: 0-63 accepted **[V]**, although only 0-12 make sense as a starting planet (see below);
- the two mapping-device bits.

**If valid,** the decoder stores:

| RAM | Value |
|---|---|
| $3E (Jetman lives) and $3D (jetpods) | both = the lives nibble |
| $CB..$D0 | the six score digits |
| $DC | rebuilt from the pod field |
| $3B | level index |
| $060C, $060D | the two device flags |

Nothing else is written by the decoder. The other fields already hold the new-game state, because opening the
entry screen set it (`game-flow.md` 7.2): $3C = $FF, $060E/$0626/$0632 = $FF. Equipment $0600-$060B, the fitted
item $D1, fuel and crystal counts are zero from the RAM clear at power-on or reset. The planet flags are rebuilt at
the next approach screen. **[C]**

The game then starts the level (`game-flow.md` 7.2): the interplanetary flight for any level but 0, then the shop
(odd $3B) or the approach screen. **[V]**

**Lives 0.** Accepted: lives and jetpods are 0 and the HUD shows no icons. The first loss of Jetman is then game
over. **[V]** (password BBBBBBDBKLBB, level 1; Jetman's loss forced in gameplay went straight to the game-over
screen)

**Levels 13-63.** The decoder accepts them and the game tries to play them (the remake rejects them,
`docs/deviations.md`). **[V]**, observed with levels 20, 50 and 63:

- **13-41:** the game runs the flight, shop or approach by the parity of $3B. The approach screen shows the
  14th planet name for 13, and whatever follows the name table for higher values. The system number shown is
  $3B / 3 + 1, and above 5 it reads past the system names. Then the surface picture, the launch and that level are
  played (level 20, a planet's second level, was playable).
- **42-63:** the tables indexed by level have no such entries, so data beyond them is used. Level 50 showed a
  corrupt surface picture, launched, then hung while loading. Level 63 showed an approach screen with garbage
  ("SYSTEM F") and crashed on landing.

**If invalid,** the new-game state is set ($3B = 0, lives and jetpods 4, score 001000, and the other fields of a
new game).

- **Start:** a new game begins. **[V]**
- **A or B:** the entry screen stays, with the letters as typed. **[V]**

## 7. What a password does not keep

Everything not listed in sections 3 and 6 is lost:

- the shop equipment owned (RAM $0600-$060B, all items except the two mapping devices);
- the item fitted to the pod (RAM $D1);
- crystals and fuel collected;
- which warpship pieces were delivered. When the next approach screen is shown, the planet flags are rebuilt as
  "every planet before this one done" (`game-flow.md` 6). **[C]**
- the separate jetpod count: on decoding it is set equal to the lives.

## 8. Worked examples (all **[V]**: accepted by the original, RAM read back)

**New game** (level 0, lives 4, score 001000, $DC = 0, no devices) shows **KBBKBBDBBDBB**:

| | P0 | P1 | P2 | P3 | P4 | P5 |
|---|---|---|---|---|---|---|
| value | $41 | $00 | $00 | $41 | $00 | $00 |
| letters (high / low) | K / D | B / B | B / B | K / D | B / B | B / B |

Checksum: ($41 XOR $00) + $00 = $41, no carry. Then ($41 XOR $00) + $00 + 0 = $41.

**Level 5, lives 7, score 012345, pod type 1 with boosters ($DC = $41), mapping device**
gives **NLDKHKGDMVLB**:

| | P0 | P1 | P2 | P3 | P4 | P5 |
|---|---|---|---|---|---|---|
| value | $72 | $51 | $16 | $4C | $35 | $40 |
| meaning | lives 7, S3 = 2 | S6 = 5, S2 = 1 | 5 × 4 + 2 + 0 | checksum | S4 = 3, pod field 1 × 4 + 0 + 1 | S5 = 4, S1 = 0 |

Checksum: $72 XOR $51 = $23; $23 + $16 = $39, carry 0; $39 XOR $35 = $0C; $0C + $40 + 0 = $4C.

Entered on the CONTINUE screen, the original set $3B = 5, $3D = $3E = 7, $CB..$D0 = 0,1,2,3,4,5, $DC = $41,
$060C = 1, $060D = 0. It flew to the shop (planet 6 is odd), then showed the same password, NLDKHKGDMVLB, on the
approach screen of "6 LEMONTE".

**Level 3, lives 12, score 999999, pod type 2, super mapping device** gives **VQBLQQQQWXPQ**: P0..P5 = $C9, $99,
$0D, $5E, $98, $99. Checksum: $C9 XOR $99 = $50; $50 + $0D = $5D, carry 0; $5D XOR $98 = $C5; $C5 + $99 + 0 = $15E,
low byte $5E. It was accepted: lives and jetpods 12, all digits 9, $DC = 2, $060D = 1. Changing its last letter to
B gave an invalid password, rejected as described in section 6.

**Level 12, lives 15, score 000000, $DC = 0, no devices** gives **ZBHGBBBBBDBB**: P0..P5 = $F0, $00, $30, $21,
$00, $00. Here the first addition carries: $F0 XOR $00 = $F0; $F0 + $30 = $120, so t = $20 and c = 1; $20 XOR
$00 = $20; $20 + $00 + 1 = $21. Accepted: $3B = 12, lives and jetpods 15, score 0, $DC = 0, no devices; the game
flew to the approach screen of planet 12 (even, no shop).

## 9. Entry screen **[V]**

- **Layout:** "ENTER PASSWORD" and "OR PRESS START TO PLAY" (`text.md`, message 4). The 12 letters, all A at
  first, are on row 12 from column 10. The cursor starts on the first place. It is a sprite (tile $5C, a dash)
  at x = 80 + 8 × place, sprite table y byte = 100, so the sprite is drawn from screen line 101, just under the
  letter (the sprite is not affected by the screen's vertical scroll of 2; `scenes.md` 4). **[V]** y, **[C]** steps of 8 px
- **No sounds:** moving the cursor or changing a letter plays no sound effect. **[C]**
- **Select:** does nothing. **[C]**
- **No timeout:** the screen stays until a valid password is entered or Start is pressed; there is no way back to
  the title. **[C]**
- **Left/Right:** move the cursor (0-11, no wrap).
- **Down / Up:** step the letter forward / backward through A-Z, wrapping Z→A / A→Z.
- **Repeat:** a held Up or Down repeats after 32 frames, then every 14 frames. **[V]**
  - The repeat timer runs while Up or Down is held. It restarts at 0 whenever either direction is newly pressed,
    so switching from Up to Down while still holding gives an immediate step and a new 32-frame wait. **[C]**
  - With both held, Up wins (the letter steps backward). **[C]**
- **A, B or Start:** validate (section 6). If one of them is newly pressed on the same frame as a direction, only
  the validation happens: that frame's direction, repeat and cursor movement are skipped. **[C]**

## Open questions

- Whether the second and third levels (13-41) entered from a password behave fully like a normal visit (only
  level 20 was tried, briefly).
