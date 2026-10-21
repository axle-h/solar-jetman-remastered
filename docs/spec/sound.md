# Sound: driver, music and sound-effect data

How the original makes its sound: the data formats of the music and sound effects in bank 4, the sound driver's
per-frame behaviour as rules, how the game asks for songs and effects, and which songs and effects exist. Written so
that `src/game` can reimplement the driver (driving `src/apu`) and `src/rom` can import the data, without seeing the
original program.

Addresses follow `README.md`: `ROM 0x22A17` is a PRG ROM offset, `bank 4:$AA17` a CPU address with bank 4 mapped
(`ROM offset = 0x20000 + address − $8000` for bank 4). `RAM $0638` is internal RAM. Status: **[V]** verified in
Mesen, **[C]** read from the program, **[H]** hypothesis.

**How this was verified.** An independent model of the driver, written from the rules in this file, was fed the
song and effect start events logged from the original running in Mesen, and its APU register writes ($4000-$400F)
were compared with the original's, write by write, after every driver update. They were identical in every update
of: all 34 songs (each played past its loop point or end), every sound-effect id (each started at least once, in
play or through the request slots), about 400 000 frames of random play on 20 levels, the title screen, pause, and
the travel and approach screens of several planets. Rules marked [V] below are covered by that comparison.

## 1. Hardware use

- **Channels:** the driver uses the two pulse channels, the triangle and the noise channel. **No DPCM**: no sample is
  ever played. **[V]**
- **Power-on writes** (once, before the title): $10 to every register from $4017 down to $4000, then $4010 = $C0,
  $4017 = $80 (5-step frame counter sequence) and $4015 = $0F (the four channels enabled). The DMC output level
  therefore stays at 16 ($4011 = $10) for the whole game, which slightly lowers the triangle and noise levels in the
  APU's non-linear mixer. **[V]**
- **The APU does part of the work.** The data relies on hardware length counters, hardware volume envelopes
  (constant-volume bit clear), sweep units, the triangle's linear counter and the reload side effects of writing
  $4003/$4007/$400B/$400F. `src/apu` must implement these from the NESdev documentation, and the driver must issue the
  same register writes, in the same order, as listed in sections 6 and 7. **[V]**
- **Channel-enable writes outside the driver** (all in bank 0, [C]; pause verified [V]):

  | Value to $4015 | Where (bank 0) | When |
  |---|---|---|
  | $00 | $95A2 | the game is paused (Start during flight) |
  | $0F | $920E, $9231 | the game is resumed |
  | $00 | $DEF9 | silences everything; context not identified |
  | $0F | $E0FC | just after song 6 is started |

  While the game is paused the driver is **not run at all**: the music and effects freeze and continue from the same
  point on resume. **[V]** (Disabling a channel in $4015 clears its length counter, so a held note stays silent after
  resume until the next write to its fourth register; this is APU behaviour.)
- **Rate:** the driver runs once per video frame, from the end of the NMI handler **[V]**, so it does not slow down
  when the game lags **[C]**. In the remake it also runs **once per frame** (not once per logic step: in gameplay a
  logic step is two frames, `game-flow.md`). (A handful of frames during scene loads run it zero or two times;
  ignore.)

## 2. Where the data is, and what it is

All sound data and the driver are in bank 4. Categories follow `docs/CLEAN_ROOM.md` rule 5 and `rom-map.md`: the
music and effect data are ripped; the tables that only say where they are (pointers) and the tuning constants are
commit.

| ROM | Bank 4 | Size | What | Category |
|---|---|---|---|---|
| 0x22320-0x22341 | $A320-$A341 | 34 | tempo of each song (section 3.1) | **ripped** (per-song music data) |
| 0x22342-0x22846 | $A342-$A846 | | driver code, including a 23-entry command dispatch table at $A663-$A690 | code |
| 0x22847-0x228C0 | $A847-$A8C0 | 122 | note period table, 61 entries (section 3.3) | **commit** (an equal-temperament tuning table), see note |
| 0x228C1-0x22906 | $A8C1-$A906 | 70 | sound-effect pointer table (section 3.2) | **commit** (location table) |
| 0x22907-0x22A16 | $A907-$AA16 | 272 | track pointer tables (section 3.1) | **commit** (location table) |
| 0x22A17 | $AA17 | 1 | the shared "end" track of song 0 | **ripped** |
| 0x22A18-0x22C1A | $AA18-$AC1A | 515 | sound-effect data | **ripped** |
| 0x22C1B-0x27E4B | $AC1B-$FE4B | 21 041 | music track data | **ripped** |
| 0x27E4C-0x27E54 | $FE4C-$FE54 | 9 | stray bytes, never read (a copy of bytes from bank 0's code) | code (not sound) |
| 0x07273-0x07278 | bank 0 $F273-$F278 | 6 | template of the one sound effect built in RAM (section 4.3) | **ripped** |

Every byte of 0x22A17-0x27E4B is reached by decoding all songs and effects from the pointer tables, and nothing
outside it is. **[V]** There are **no instrument, envelope or vibrato tables**: all of that is inline in the track data
as commands.

Note on the period table: its values are close to, but not exactly, what a formula gives (the lowest octave differs by
1-2 from rounding 1 789 773 / (16 f) − 1 with A4 = 440 Hz), so it cannot be regenerated; if it is committed, extract
it from the ROM with our tools rather than recomputing it.

## 3. Tables

### 3.1 Songs **[V]**

There are **34 songs**, numbered 0-33.

- **Tempo:** byte `song` of the table at ROM 0x22320.
- **Tracks:** each song has four tracks, one per channel (0 = pulse 1, 1 = pulse 2, 2 = triangle, 3 = noise).
  Index `i = song × 4 + channel`; the track's start address is `byte[0x22907 + i] + 256 × byte[0x2298F + i]`, a bank 4
  CPU address. Tracks of different songs and channels may share bytes (song 0 uses $AA17 for all four channels;
  subroutines are shared between channels of a song).

### 3.2 Sound effects **[V]**

Sound effects are identified by an **even id** $02-$44 (34 effects; odd ids are reserved, section 7.3). The
effect's start address is the 16-bit little-endian word at ROM `0x228C1 + (id AND $FE)`. Entry 0 (id 0) is $0000
and unused: id 0 means "no effect". The entry for id $1A is **$06E8, a RAM address**: that effect is built in RAM by
the game (section 4.3).

### 3.3 Note periods **[V]**

61 entries of 16 bits (low byte first) at ROM 0x22847. Entry 0 is 0 (a rest). Entries 1-60 are the 11-bit pulse /
triangle timer periods of the 60 semitones from C2 (entry 1, period $06AE, 65.4 Hz on a pulse channel) to B6 (entry
60, period $0038): entry `n` is the semitone `n − 1` above C2. On the triangle, which sounds an octave lower than a
pulse channel with the same period, the same entry is an octave lower.

## 4. Data formats

### 4.1 Music tracks **[V]**

A track is a byte stream read by the driver one event at a time. A byte **$80-$FF is a note**, a byte **$00-$16 is a
command** followed by its operands. The track pointer moves past whatever was read.

**Note.** `n` = the note byte.

- If **fixed duration** is off, the next byte is the duration `d` (in ticks, 1-255; 0 means 256). If fixed duration is
  on, there is no duration byte and `d` = the fixed duration.
- **Note index:** if `n = $80` the note is a **rest**: index 0. Otherwise `index = (n + transpose) AND $7F` (8-bit
  addition, transpose from commands $12-$14). So without transposition $81 is C2, $8C is B2, $BC is B6.
- **Pulse and triangle:** the period is entry `index` of the period table. The data never produces an index above 60.
- **Noise:** the value for the noise period register is `(index − 65) AND $7F` (so index 65 gives 0), and 0 for
  index 0 (and for index 64, which the data never produces). Bits 0-3 of the value are the noise period index; bits
  4-6 can be set and are ignored by the hardware; bit 7 (mode) is never set by music. The data uses both ranges: index
  65-82 (values $00-$11) and index 1-12 (values $40-$4B, e.g. song 14's noise notes $81-$8C). **[V]** (the original
  writes $40-$4B to $400E in song 14.)
- **Index 0 is a rest whatever the note byte:** besides $80, a note whose transposed index is 0 plays as a rest
  (period 0). This happens in song 22, where note $83 is played with transpose −3 on both pulse channels. **[V]**
  Importers must compute the index before deciding that a note is a rest.
- **Rests** write period 0. On a pulse channel that is silent (the hardware mutes periods below 8). On the triangle
  it runs the waveform at an ultrasonic rate (inaudible; `src/apu` may hold the output instead). On the noise channel a
  rest writes 0 to the period register and is silent only if the volume is 0.
- What a note does to the channel at its start is in section 6.2.

**Commands.** Operand bytes are listed in order; "signed" means two's complement.

| Byte | Operands | Effect |
|---|---|---|
| $00 | | **End of track.** The channel's music stops: music-active flag cleared and $10 written to the channel's first register (constant volume 0 for pulse and noise; for the triangle a linear-counter reload of 16 with the control flag clear, which stops it shortly after). This is written even if a sound effect is playing on that channel. |
| $01 | lo, hi | **Jump** to the bank 4 address `lo + 256 × hi` and keep reading there (loops). |
| $02 | | Clears bit 0 of the id of the sound effect on this channel, then reads the next byte **under sound-effect rules** (section 4.4). Not meant for music; reached only by a data slip in song 4 (section 9), where it behaves as "do nothing". |
| $03 | c, s, h | **Channel setup.** Control byte := `c` (its bits 5-7 go to the first register: duty and length-counter halt / triangle control); volume := `c AND $1F` (bit 4 = constant-volume flag, bits 0-3 = volume or envelope period); sweep byte := `s` (written to the second register); period-high byte := `h` (bits 3-7 = length-counter index, kept; bits 0-2 are replaced by each note). If the retrigger is on (bit 7 set), the retrigger byte := `(c AND $1F) OR $80`: bit 7 stays set, bit 4 is copied from `c` (not forced), bits 5-6 are cleared. |
| $04 | count, lo, hi | **Call** the subroutine at `lo + 256 × hi`, `count` times in a row (0 = 256). One level only: a call inside a subroutine overwrites the saved return point and count. |
| $05 | | **Return** from the subroutine: decrement the count; if not zero, jump to the subroutine's start again; if zero, continue after the call command. |
| $06 | d | **Fixed duration on:** every following note lasts `d` ticks and has no duration byte. |
| $07 | | **Fixed duration off:** notes carry their own duration byte again. |
| $08 | len, rate, depth | **Vibrato on** (section 6.3): steps per half cycle := `len`, step interval := `rate + 1` effect frames, delta := −`depth`; the half-cycle counter := `len / 2` (rounded down). |
| $09 | | **Vibrato off.** |
| $0A | delay, rate, steps, amount, back | **Pitch slide up** (section 6.3): start delay := `delay`, step interval := `rate + 1`, number of steps := `steps`, delta := −`amount` (period decreases), number of initial steps in the opposite direction := `back`. |
| $0B | delay, rate, steps, amount, back | **Pitch slide down:** as $0A with delta := +`amount` (period increases). |
| $0C | v | **Set volume:** volume effect off; volume := `v OR $10`. If the retrigger is on, the retrigger byte := `v OR $90`. |
| $0D | delay, rate, target | **Volume ramp** (section 6.3): mode := 1; delay := `delay + 1`, also saved as the initial delay; step interval := `rate + 1`; target := `target OR $10`; volume := volume `OR $10`. If the retrigger is on, the retrigger byte := the new volume `OR $80`. |
| $0E | half, rate, delta | **Volume oscillation** (section 6.3): mode := 2; half period := `half`, reversal counter := `half / 2` (rounded down), step interval := `rate + 1`, delta := `delta` (signed); volume := volume `OR $10`. If the retrigger is on, the retrigger byte := the new volume `OR $80`. |
| $0F | | **Volume effect off.** |
| $10 | t | **Tempo** := `t` (global, all channels). |
| $11 | t | **Tempo** := tempo + `t` (8-bit). |
| $12 | t | **Transpose** this channel := `t`. |
| $13 | t | **Transpose** this channel := transpose + `t` (8-bit). |
| $14 | t | **Transpose all four channels** := `t`. |
| $15 | | **Retrigger off.** |
| $16 | delay, rate, target | **Retrigger on**, then as $0D: the retrigger start volume := the current volume `OR $90` (bit 7 = on; bits 0-6 = start volume, constant-volume flag set), so that every note restarts the volume ramp from that volume (section 6.2). |

Wherever a command sets an interval ("step interval := ..."), the interval's counter is set to the same value.
Commands are processed one after another, in the same frame, until a note is read. Command bytes above $16 do not
occur. Commands used by the data: $00-$08, $0A-$0C, $13, $14, $16; never used: $09, $0D-$12, $15 ([C] for these).

### 4.2 Sound effects **[V]**

An effect is a 3-byte header, then 2-byte steps, then a $00 terminator:

| Offset | Meaning |
|---|---|
| 0 | bits 4-7: the upper four bits of the first register (duty, halt / triangle control, constant-volume flag); bits 0-3: `r`, each step lasts `r + 1` frames |
| 1 | sweep byte, written to the second register at every step |
| 2 | initial shadow of the fourth register (length-counter index in bits 3-7, period bits 8-10 in bits 0-2) |
| 3.. | steps, then $00 |

A **step** is two bytes `a b`:
- `a` (≥ $10): volume = `a / 16` (1-15), period bits 8-10 = `a AND 7` (bit 3 is ignored);
- `b`: period bits 0-7 (for noise: the value of the period register, bit 7 = mode).

A byte $01-$0F where a step is expected is a command: only $02 occurs (once, in effect $3A): it clears bit 0 of the
playing effect's id and the next byte is read in its place. $00 ends the effect.

### 4.3 The effect built in RAM **[V]**

Effect $1A (the tractor-beam hum, on the triangle) lives at RAM $06E8-$06ED. At game start the six bytes at bank 0
ROM 0x07273 are copied there: a header, one step and the terminator. While the beam holds an object the game rewrites
the step's period-low byte (RAM $06EC) every 4 frames (section 8.3). An importer copies those six bytes; the remake
keeps the effect in its own state and patches its pitch byte.

### 4.4 Command $02 inside music (data slip) **[V]**

When music reads $02: bit 0 of the channel's sound-effect id is cleared, then the next byte `x` is read with the
sound-effect step rules of section 7.2, on this channel, at the music position:
- `x` = $01-$0F: it is executed as the music command `x` (with its operands), and music reading continues normally.
  This is the only case in the data (always $0C or $05 follows), so the importer may treat $02 as "no operation".
- `x` = $00 or ≥ $10 never happens; the original would end or play a sound-effect step and lose its music position.

## 5. Driver state

Per channel `k` (0 = pulse 1, 1 = pulse 2, 2 = triangle, 3 = noise), the original keeps its state in RAM at
`base + 4k`. Useful for state comparisons with the original; the remake may hold the same values its own way.

| RAM (k = 0) | Meaning | Reset by song start |
|---|---|---|
| $0638 | music active (non-zero) | := 1 |
| $0639 | ticks left on the current note | := 1 |
| $063A-$063B | music read position | := track start |
| $0648 | sound-effect id playing (0 = none) | |
| $0649 | effect frame counter: bits 4-7 frames left, bits 0-3 frames per step − 1 | |
| $064A-$064B | effect read position | |
| $0658 | music control byte (command $03) | |
| $0659 | music sweep byte | |
| $065A | music period, low byte | |
| $065B | music period high byte (length index in bits 3-7) | |
| $0668, $0669, $066A, $066B | effect's shadows of the four registers | |
| $0678-$0679 | subroutine return point (address of the call's address operand) | |
| $067A | subroutine repeat count | |
| $067B | vibrato delta (signed) | |
| $0688 | fixed duration (0 = off) | := 0 |
| $0689 | volume byte (bit 4 constant-volume flag, bits 0-3 volume) | |
| $068A | volume ramp delay counter (mode 1) / half period (mode 2) | |
| $068B | transpose | := 0 |
| $0698 | vibrato steps per half cycle (0 = vibrato off) | := 0 |
| $0699 | vibrato steps left in this half cycle | |
| $069A, $069B | vibrato interval, interval counter | |
| $06A8 | retrigger (bit 7 = on, bits 0-6 = start volume) | |
| $06AA | volume effect interval counter | |
| $06AB | slide interval counter | |
| $06B8 | slide steps left | |
| $06B9 | slide start delay | |
| $06BA | slide delta (signed) | |
| $06BB | slide steps left in the opposite direction | |
| $06C8 | volume target (mode 1) / delta (mode 2) | |
| $06C9 | initial ramp delay (mode 1) / reversal counter (mode 2) | |
| $06CA | volume effect mode: 0 none, 1 ramp, 2 oscillation | |
| $06CB | volume effect interval | |
| $06D8 | slide interval (0 = slide off) | := 0 |

Global: RAM $F8 tempo accumulator (:= 0 by song start), $F9 tempo (:= the song's tempo), $0637 current song (section
8.1). Everything not reset by a song start **carries over from the previous song** (control, sweep, volume,
retrigger, volume effect, vibrato and slide parameters, the vibrato delta's sign, interval counters). At power-on and
whenever the game restarts (game over) the game clears RAM, so all of it starts at 0. **[V]**

## 6. Per-frame update: music

Each update first advances the tempo, then processes channels 0, 1, 2, 3 in order; for each channel, the music part
(if music is active on it) and then the sound-effect part (if an effect is playing on it, section 7.2). **[V]**

### 6.1 Tempo

A **tick** in this file is the driver's own music tick, unrelated to the game's logic step.
`accumulator := accumulator + tempo`; if the sum reached 256 (it is kept modulo 256), this frame is a **tick frame**.
So a song advances `tempo / 256` ticks per frame: tempo $80 gives a tick every second frame (30.05 ticks per second).
All four channels share the tempo. Right after a song start the accumulator is 0, so the first tick, which reads the
first event of every track, comes on update `ceil(256 / tempo)`. **[V]**

### 6.2 Tick frames

On a tick frame each active music channel decrements its ticks-left counter. If it is still non-zero, **nothing else
happens on that channel this frame**: no effects, no register writes. If it reached zero, the channel reads commands
until a note, and the note starts:

1. If the retrigger is on: volume := its start volume (bits 0-6 of the retrigger byte), volume effect mode := 1
   (ramp), ramp delay := the saved initial delay.
2. Vibrato half-cycle counter := `ceil(len / 2)` (the vibrato delta keeps its current sign).
3. Period := the note's period (section 4.1); the period-high byte keeps its bits 3-7 and takes bits 0-2 from the table
   (noise: bits 0-2 are cleared and the noise value goes to the low byte).
4. If no sound effect is playing on the channel, all four registers are written (section 6.5, "full write").
5. Ticks left := the note's duration.

### 6.3 Other frames: effects, in this order

On a frame that is not a tick frame, each active music channel runs its effects (all counters below count only these
frames), then writes its registers ("frame write", section 6.5) unless a sound effect is playing on the channel.

**Pitch slide** (while the slide interval is non-zero). Only commands $0A and $0B arm a slide (start delay, interval,
steps, delta, opposite-direction count). A new note never re-arms or resets it: the note sets the period, and a slide
still running continues from the new period with its remaining steps; a slide that has finished stays off until the
next $0A/$0B. A song start turns it off (interval := 0). **[V]**
- if the start delay is non-zero, decrement it; while it is still non-zero, skip the slide this frame;
- decrement the interval counter; when it reaches 0, reload it with the interval and do one step: decrement the
  steps left (when they reach 0 the slide turns off after this step); the step's delta is the slide delta, or its
  negation while the opposite-direction count is non-zero (that count is decremented); add the delta, sign-extended,
  to the 16-bit period (low byte + high byte together, so a carry can change the length-index bits).

**Vibrato** (while the steps per half cycle are non-zero):
- decrement the interval counter; when it reaches 0, reload it, and if the period's 11 bits are not zero (not a rest):
  add the delta (sign-extended) to the 16-bit period, decrement the half-cycle counter, and when it reaches 0 reload it
  with the steps per half cycle and negate the delta.
- Example: command `08 02 01 01` (len 2, rate 1, depth 1), delta −1: every 2nd effect frame the period changes by
  −1 once (`ceil(2/2)` = 1 step), then +1, +1, −1, −1, +1, +1 and so on. The period is reset to the note's value at
  each note; the delta's sign and the interval counter are not.

**Volume effect** (while the mode is non-zero):
- Mode 1 only: while the delay counter is not 1, it is decremented (8-bit) and the volume effect does nothing else
  this frame. (So the ramp starts after `delay` effect frames, and the counter then stays at 1.)
- Decrement the interval counter; when it reaches 0, reload it and do one step:
  - **Mode 1 (ramp):** if volume < target (comparing the whole bytes) volume := volume + 1, else volume := volume − 1;
    then, after this step, if the low four bits of volume and target are equal, mode := 0. The test is never made
    before a step, so a ramp that starts at its target first moves one away (down) and comes back on the next step.
    The target always has bit 4 set. If the volume has bit 4 clear (only possible when the retrigger byte was set by a
    command $03 without the constant-volume bit, which the data never does [C]), the whole-byte comparison makes the
    volume count up through the values with bit 4 clear, where the first register runs the hardware envelope with the
    low four bits as its period, until it reaches the target's value with bit 4 set; it stops there, not at the first
    value whose low four bits match after the start.
  - **Mode 2 (oscillation):** `v := volume + delta` (8-bit); if bit 4 of `v` is set, volume := `v` (otherwise the
    step is lost); decrement the reversal counter; when it reaches 0 reload it with the half period and negate the
    delta.

### 6.4 What the volume byte means

The volume byte goes into bits 0-4 of the first register: with bit 4 set, bits 0-3 are a constant volume; with bit 4
clear (possible after command $03), the hardware envelope runs with bits 0-3 as its period. On the triangle the value
written, `(control AND $E0) OR volume`, is the linear-counter setup: bit 7 the control flag, bits 0-6 the reload
value. The commands $0C, $0D, $0E and $16 always set bit 4.

### 6.5 Register writes for music

Registers of channel `k`: `R0 = $4000 + 4k` (first), `R1` (sweep), `R2` (period low), `R3` (period high / length).

| | Writes, in order |
|---|---|
| **Full write** (note start; also when an effect ends, section 7.2) | `R1 := sweep byte`; `R0 := (control AND $E0) OR volume`; `R2 := period low`; `R3 := period high` (always) |
| **Frame write** (frames that are not tick frames) | `R0 := (control AND $E0) OR volume`; `R2 := period low`; `R3 := period high` **only if the period-high byte changed during this frame's effects** |

R3 is written at every note start, so every note restarts the length counter, the envelope and (on pulses) the phase.
For the noise channel R1 ($400D) is written although the hardware ignores it. **[V]**

**Triangle exception** [C]: while object slot 2 (one of the two slots attached to the pod, `objects.md` 2.2) is in
state 8 (its state byte RAM $0317 + 2 = $0319) and the tractor beam holds an object (RAM $66, the held object's slot,
is non-zero), the triangle's music writes stop after the R1 write (no R0, R2, R3). Slot 2 was never seen in state 8:
in about 370 000 logged driver updates it was in state 0, 9 (only while the beam held an object) or $0C. The game sets
slot 2's state to 9 when the beam catches an object (with effects $12/$14) and to 0 or $0C elsewhere; state 8 is
written only by the code that creates shots and sparks, which `objects.md` places in slots 3-10. So the condition
probably never occurs [H], and the remake may omit it. Purpose [H]: to keep the music off the triangle while the
beam hum uses it.

## 7. Sound effects

### 7.1 Starting an effect **[V]**

The game starts effect `id` on a chosen channel (section 8.2). If `id` equals the id already playing on that channel,
**nothing happens** (the effect is not restarted). Otherwise:
- effect id := `id`; read position := the header + 3;
- frame counter := header byte 0 AND $0F (bits 4-7 zero, so the first step is played at the next update);
- R0 shadow := header byte 0 AND $F0; sweep shadow := header byte 1; R3 shadow := header byte 2;
- the music vibrato of that channel is switched off (steps per half cycle := 0). It stays off until the track's next
  command $08.

Starting an effect writes no register; music on the channel keeps running silently underneath (section 7.3).

### 7.2 Effect update **[V]**

Every update (tick frame or not), for a channel with an effect playing, after its music part:
- If the frame counter's bits 4-7 are non-zero, decrement them and do nothing else.
- Otherwise set both nibbles to the frames-per-step value `r` (the step lasts `r + 1` frames) and read:
  - **$00 (end):** effect id := 0; if music is active on the channel, do a music full write (section 6.5) so the
    music takes the channel back at once; otherwise write $10 to R0.
  - **$02:** clear bit 0 of the effect id and read the next byte instead.
  - **step `a b`:** R0 shadow := (R0 shadow AND $F0) OR `a / 16`, write it to R0; write the sweep shadow to R1; if
    `a AND 7` differs from the R3 shadow's bits 0-2, put it there and write the R3 shadow to R3 (otherwise R3 is not
    written, so the length counter is not reloaded); R2 := `b`, write it. Advance by 2.

### 7.3 Priority between music and effects **[V]**

- An effect owns its channel's registers while it plays: the music on that channel keeps reading notes and running
  its effects (its state advances), but writes nothing (except the end-of-track write of command $00).
- When the effect ends, the music's full state is written at once if the music is active.
- A new request for a **different** id replaces the playing effect immediately; a request for the **same** id is
  ignored. Bit 0 of an id is a lock: an odd id would be ignored while it plays until its data reaches a $02, which
  clears the bit so that the same odd request starts it again. The game only uses even ids, so this never applies.
- The game decides the channel for each request; many effects are requested on both pulse channels at once.

## 8. What the game requests

### 8.1 Songs

Two entry points in bank 0: **start if different** ($D480: does nothing if the song equals RAM $0637, the last song
started this way; else stores it there and starts it) and **start always** ($D488, does not update $0637). Starting a
song resets the tempo accumulator and each channel as in the table of section 5. **To stop the music, the game starts
song 0**, whose four tracks end at once (each channel writes $10 to its first register). **[V]**

| Song | Kind | Plays | Status |
|---|---|---|---|
| 0 | silence | stops the music (e.g. on the Space Loonies bonus screen after a warp level) | [V] |
| 1 | once, 2662 frames | title (after the opening, and after a game over) | [V] |
| 2 | once, 389 frames | **the level-change gate**: started the moment the pod or Jetman touches a wormhole and the gate state (state 9, `player.md` 9.2) begins; it replaces the level song for the gate's 385 frames (193 logic steps, `player.md` 9.2.3), and the next level then starts its own song (bank 0 0x0243D, the only place that starts song 2; checked in Mesen on level 13: song 2 at the touch, song 8 about 410 frames later). It is **not** started by the pod's or Jetman's death nor by a game over (checked: pod and Jetman destroyed to game over, no song start). The earlier label "the pod is destroyed" was wrong | [V] |
| 3 | once, 1077 frames | landing on a planet (surface picture), when RAM $A4 = 0 | [V] |
| 4 | loops | the mothership travelling between planets (star field), before the shop | [V] |
| 5 | loops | warp levels (instead of the planet song when RAM $9B bit 7 is set); also when the mothership leaves a planet (surface mode 1, bank 0 $8AD7 and $9EEB: `scenes.md` 8.3) | [V] |
| 6 | loops | the planet map (pause, then Select): started always, at bank 0 $E0F2, after a silence ($4015 = 0), **at the end of the map's build** (15-58 video frames after the Select and 1-3 frames before the first picture: the driver does not run while the build has the frame interrupt off, `scenes.md` 13.2); the level song is started again at once when Start leaves (`scenes.md` 15) | [V] |
| 7, 9, ..., 29 (odd) | loop | **approach screen** ("APPROACHING PLANET"): song `2 × L + 7`, where L = level index (RAM $3B), with L taken as 8 if L ≥ 12 | [V] for 7, 9, 17, 23 |
| 8, 10, ..., 30 (even) | loop | **planet music**, from the level setup: song `2 × m + 8` (rule below) | [V] for 8-30 |
| 31 | loops | the backdrop interlude (transition 7), at entry (`scenes.md` 15) | [V] |
| 32 | loops | the final congratulations message (message 23) and, with no further request, the ending and credits (bank 0 $AC40, `scenes.md` 15) | [V] (request seen when the end-of-level routine was run in Mesen, 2026-10-17; it plays to the reset, `scenes.md` 15) |
| 33 | loops | instead of the planet song when RAM $9A bit 7 is set (the final level, `game-flow.md`): the current-song byte changed from 8 to $21 on the last loading frame of the final level, one frame before gameplay | [V] 2026-10-17 |

Planet song rule (L = RAM $3B; [V] for L = 0-14, 20 and the warp levels 29, 33, 35, 41; [C] otherwise):
m = L if L < 13; m = L − 13 if 13 ≤ L < 26; m = 1 if L ≥ 26; then if m ≥ 12, m = 8. Song = 2m + 8, except: RAM $9A
bit 7 set → song 33; else RAM $9B bit 7 set → song 5. If RAM $050F bit 7 is set (the demo), no planet song is
started.

Tempos are ripped data and are not listed here: the importer reads them from the table (section 3.1).

"Once" songs end by themselves (their tracks reach $00); the channels then stay silent until the next song. All
other songs loop forever through $01 jumps; some of their tracks are a single $00 (that channel is silent in that
song). **[V]**

### 8.2 Sound-effect requests **[V]**

Most requests go through **two request slots**, A (RAM $F3 channel, $F4 id) and B ($F5 channel, $F6 id). The channel
is stored as $15 (pulse 1), $18 (pulse 2), $1B (triangle) or $1E (noise); 0 = slot empty. Game code fills a slot (both
slots to play an effect on both pulses).

- **Servicing:** once per frame, in every scene, **one** slot is serviced: slot A when the frame counter RAM $5D is
  even, slot B when it is odd [V]. It happens at the very start of each scene's logic pass (`game-flow.md` 1.1),
  before anything else in the pass, so it is done even when the rest of the pass is cut off; in gameplay that is every
  frame (both the A and the B frame of a logic step). While a scene is being built (no logic pass), the loader's
  wait-for-next-frame services a slot once per frame it waits. The pause scene services slots too, but the driver
  does not run while paused, so an effect started then plays on resume. [C]; servicing seen in gameplay, the
  title screen and the Space Loonies intermission [V]. The slot is emptied and, unless RAM $050F bit 7 is set
  (demo: requests are dropped), the effect is started (section 7.1). A request therefore starts at the next update or
  the one after. A slot written again before it is serviced keeps only the last request.
- **Request lock:** RAM $F7. While it is non-zero, each servicing first decrements it; if the result is negative
  (bit 7 set) servicing proceeds normally, otherwise both slots are emptied and nothing starts. The game sets it to
  $81 when it requests effect $32 (bank 0 $9D5E): the next servicing starts $32 (on one pulse only, the slot of that
  frame's parity), then requests are discarded for 128 frames.
- **Direct starts** (no slot, immediate): effect $06 on pulse 2 from bank 0 $CC80 (skipped when $050F bit 7 is set),
  and effect $3A on the noise channel from the text printer (bank 1 $9B47: for each printed character that is not a
  space; this one ignores $050F).

### 8.3 Sound effects and when they play

"Both" = requested on pulse 1 and pulse 2 together. Frames = steps × (`r` + 1). Requester = where the game asks for it
(ROM offset of the code). Descriptions marked [H] come from screenshots of the moment the effect started.

| Id | Channel(s) | Steps × frames | Requester | When |
|---|---|---|---|---|
| $02, $04 | $02 pulse 1 + $04 pulse 2 | 7 × 5 | bank 2 0x10133 | an explosion is spawned (shots hitting things) [H] |
| $04 alone | **noise** (channel byte $1E, slot A) | as $04 | bank 1 0x08BEF (called from 0x0856E) | a shot destroys a fuel pod (item kind 11, `behaviour.md` 3.3): the id $04 is requested on the noise channel, in the step the fuel is set; no other sound **[V]** (Mesen, shot poked onto a kind-11 item) |
| $06 | pulse 2 (also both) | 1 × 7 | bank 0 0x04C80 (direct), 0x05B90, bank 1 0x0950B (both pulses) | the gun: started directly (not in a demo) **only when the gun found a shot slot**, so a press that makes a shot makes the sound and a press with all eight slots 3-10 in use (none free, none a spark, `player.md` 10) makes neither [V] (Mesen: 10 presses with slots free, 10 sounds; 10 presses with the slots filled, 0 sounds); the same for a special weapon, and for Jetman's shot [C]. The two both-pulse requests are menu code, not the gun: 0x05B90 (bank 0) is the shop's, one request at every payment step of a purchase, and 0x0950B (bank 1; the equip list's fit and un-fit) is the equip list's **[V]** 2026-10-17 (`scenes.md` 6) |
| $08 | both | 1 × 7 | bank 7 0x3B08F | an enemy fires a shot (`behaviour.md`), from eleven call sites (turrets, dome, orbs, formation parts and others). **Not requested** by the large creature (type 21, both its shots) nor by the large machine (type 22: the four-shot volley and the extra forward shot): those shots are silent **[V]** (Mesen, objects placed in a slot, both types fired, 0 requests; the handlers contain no request, [C]) |
| $0A | noise | 6 × 4 | bank 1 0x08665, 0x089B1 | an object is hit / destroyed [H] |
| $0C | noise | 9 × 4 | bank 1 0x08983 | an object is destroyed [H] |
| $0E | pulse 1 | 1 × 7 | bank 1 0x08A20 | a pod shot is deflected by an object that cannot be hurt (`behaviour.md` 3.3) [C] |
| $10 | noise | 1 × 2 | bank 0 0x04BFE | engine thrust, requested while B is held (`player.md` 4.8) [C]; also repeatedly on the title screen |
| $12, $14 | $12 pulse 1 + $14 pulse 2 | 3 × 3 | bank 0 0x0251B | the tractor beam catches an object [H] |
| $16 | pulse 1 | 7 × 4 | bank 1 0x083C4 | the pod bumps an object that survives the collision [C] |
| $18 | pulse 2 | 12 × 4 | bank 7 0x3CDA3 | a drone is launched [C] |
| $1A | triangle | 1 × 5 | bank 0 0x025C2 | tractor-beam hum (RAM effect, below) [V] |
| $1C | both | 13 × 3 | bank 0 0x02484, bank 7 0x381FB | a gem is collected: a crystal touched on a warp level (0x02484) **[V]**, and a gem touched in the backdrop interlude (0x381FB; `scenes.md` 11.1.2) **[V]**. Both add 1 to the crystal count RAM $67. (The earlier guess "warp level start" was wrong) |
| $1E, $20, ..., $30 | both | 1 × 7 | bank 0 0x01174 | a rising series: id = $1E + 2 × counter (RAM $0C, set to 0 when the intermission starts and incremented each time), one per counted crystal, one every 16 frames, on the Space Loonies badge screen (`economy.md` 5.4). With the ten crystals of a warp level or of the interlude the ids run $1E, $20, ..., $30 **[V]** (Mesen, count 10: ten requests $1E-$30, the first about 120 frames after the badge starts, then one every 16 frames) |
| $30 | a pulse channel (its header has a pulse sweep byte, $8B; which pulse is not known) | 8 × 12 | bank 0 0x01174 (the series above) | the tenth step of the badge screen's series, requested on both pulses **[V]**; there is no other requester |
| $32 | pulse 1 (see 8.2) | 38 × 4 | bank 0 0x01D4E | sets the request lock; screen black at the time [H: leaving a level] |
| $34, $38 | (noise) | 10 × 5, 10 × 2 | none found | [H] unused |
| $36 | noise | 10 × 2 | bank 0 0x0230F, 0x0233F | a ring moves the pod: the special ring (item kind 26, planet index 1, 0x0233F) **[V]** (Mesen: touched, $36 requested each time, the pod restarted at the ring's fixed position) and a transporter ring (kind 17, the jump to the next ring, 0x0230F) [C] |
| $3A | noise | 3 × 1 | bank 1 0x09B47 (direct) | each printed character of a text (not spaces) [C] |
| $3C | both | 2 × 16 | bank 0 0x01C46, 0x027C5 | a fuel cell (item kind 10) is delivered, in either of its two paths **[V]** (Mesen, kind 10 put on the beam above the mothership): delivered into the bay while the fuel count is below 24, no message, the request is made at that moment (0x027C5); and the cell that fills the tank (count 24 to 25) or any fuel cell delivered through a ring, where the message screen of kind 10 opens and the request is made as it starts (0x01C46; the ring route [C]). Nothing else delivered asks for a sound (below) |
| $3E | both | 2 × 14 | bank 0 0x04754 | the pod is hit [H] |
| $40 | both | 16 × 3 | bank 0 0x01329 (only if $40 is not already in slot A) | a warpship piece is delivered (message kind 5, `scenes.md` 10): the sound is requested on the frame the delivered piece appears in the picture of the warpship, with the lower pair of cells (RAM $DA odd), once **[V]** (Mesen, kind 19 delivered) |
| $42 | pulse 1 | 5 × 3 | bank 0 0x022CA, bank 1 0x0853F | a bonus (small pickup) is collected, also by shooting it [C] |
| $44 | both | 7 × 5 | bank 0 0x023FA | the wormhole bounces the pod **or Jetman alone** (planet with the fuel count below 25, `player.md` 9.2.2), together with the sparkle object, in the same step as the touch **[V]** (Mesen, planet 0, touch; both) |

**Deliveries and sound [V]** (Mesen, an item of each kind put on the beam above the mothership): kinds 3, 5, 9, 12 and 15 request no sound at all, neither at the delivery nor in their message screen; kind 10 (fuel cell) requests $3C as above; kind 19 (warpship piece) requests $40 as above. Pick-ups: the small pickup (box, star) requests $42 (0x022CA, bank 1 0x0853F); warp-level crystals and interlude gems request $1C.

**Shop, equip list and planet map [V]** (every request-slot write, effect start and song start read back in Mesen,
2026-10-17; details in `scenes.md` 6 and 13.2):

| Event | Sound |
|---|---|
| Left or Right in the shop or the equip list (also refused at the ends), B, a refused A | none |
| Shop purchase | effect $06 on both pulses at each payment step (every 8 frames, one per unit of the price's digits: 400 = 4 sounds, 1500 = 6); none when it starts or ends |
| Shop refusal (score too low, 9 owned, device already owned) | none |
| Equip list: A on an item with a count of 1 or more (fit, or un-fit of the fitted one) | effect $06 on both pulses, once |
| Equip list: A on a listed item whose count is 0 | none |
| Planet map: Select (entering) | channels silenced in the Select's own frame; song 6 when the build ends |
| Planet map: A, B, Select, the pad | none |
| Planet map: Start (leaving) | the level's song started again at once; the map itself requests no effect |

**Tractor-beam hum [C], verified as sound [V]:** every 4 frames (frame counter RAM $5D AND 3 = 0), while RAM $66
(the object held by the beam) is non-zero and that object's byte at RAM $0422 + $66 is 0, the game sets the step's
period-low byte to `2 × p`, where `p = RAM $69` if it is below $40, else `$80 − RAM $69`, and requests effect $1A on
the triangle (slot A). The effect lasts 5 frames, so it restarts every 8 frames with a 3-frame gap.

## 9. Importer notes

- **What to import:** ripped: the 34 tempos, the bytes reachable from the pointer tables in 0x22A17-0x27E4B, and
  the 6-byte RAM effect template at 0x07273; commit (location tables, which our tools may extract into `data/`):
  the 136 track start addresses and the 34 effect start addresses.
- **Decoding** needs the fixed-duration state to know whether a note has a duration byte. That state is set by
  commands $06/$07 and carries through calls and returns, so decode by walking each track with that state, following
  calls (the state at a return is passed back to the caller).
- **One block is reached in both states:** the subroutine at bank 4 $AC46-$ACC6 (ROM 0x22C46) is called from song 4's
  pulse 1 with fixed duration off (its notes carry duration bytes $02) and from song 4's pulse 2 with fixed duration 2,
  where each $02 is then read as command $02 (section 4.4) and acts as "no operation". A static decoder must decode
  this block once per state, or the importer can keep the raw command stream and let the driver interpret it at run
  time. Everywhere else, every byte has one meaning. Related: the block at bank 4 $ADE8, which song 4's
  pulse 2 calls repeatedly, begins with the fixed-duration command (duration 2) and returns with it still on, so
  the state after the call differs from the state before it; this is the reason for passing the state back from a
  return (above). **[V]**
- Loops are `$01` jumps backwards; nothing else marks a loop. Subroutines are shared between tracks.
- Worked example (real data, effect $10 at ROM 0x22A74, bytes `91 08 C0 64 2C 00`), started on the noise channel
  with the noise music active:

  | Update | What happens | Writes |
  |---|---|---|
  | 1 | frame counter 1 → step: volume 6, period-high 4 ≠ shadow 0 | $400C = $96, $400D = $08, $400F = $C4, $400E = $2C |
  | 2 | counter $11 → $01 | (none for noise) |
  | 3 | end: id := 0, music full write | $400D, $400C, $400E, $400F from the music state |

- Worked example (made-up track, tempo $80, from silence): `03 B7 43 9A 0C 08 98 04 80 02 00`.
  Update 1 (no tick) writes `$4000 = $00, $4002 = $00` from the all-zero state. At the first tick (update 2): setup (control $B7, volume $17, sweep $43, high byte $9A), set volume → $18, note $98
  (index 24, B3, period $01C4) for 4 ticks → `$4001 = $43, $4000 = $B8, $4002 = $C4, $4003 = $99`. Updates 3, 5, 7, 9:
  `$4000 = $B8, $4002 = $C4`. Updates 4, 6, 8: nothing. Update 10 (4th tick): rest for 2 ticks → `$4001 = $43,
  $4000 = $B8, $4002 = $00, $4003 = $98`; update 11 and 13 `$4000 = $B8, $4002 = $00`; update 14: end of track,
  `$4000 = $10`.

## Open questions

- Contexts of songs 6, 31 and 33 are in `scenes.md` 15; of the effects marked [C] or [H], and of the shop's requests (another
  note); whether effects $34 and $38 are used (no requester found; $30 is the tenth step of the badge series, song 2 is the
  level-change gate, 8.1, 8.3).
- The triangle exception (section 6.5) was never seen happening in our runs; the meaning of object slot 2 = type 8 is
  for the objects spec.
- Song 1 contains its own end; whether the title screen restarts it on a long wait was not seen (20 000 frames idle:
  it played once).
