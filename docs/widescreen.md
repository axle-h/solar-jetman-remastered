# Widescreen

A port-menu option, VIDEO > ASPECT, that widens the field of view: the frame grows sideways and the game shows more of the world. Nothing is stretched or cropped. On ORIGINAL (the default) everything is exactly as on the original's 256 pixels; replays, Mesen comparisons and `sj-tools` renders run that way unless `--aspect` is given.

## Decisions (Alex, 2026-10-25)

- **ASPECT: ORIGINAL / NATIVE.** ORIGINAL is the 256-wide frame of square pixels (16:15, not 4:3). NATIVE takes the shape of the display the window is on, so windowed and fullscreen match.
- **The logic widens with the view.** Spawning, despawning and every other rule measured against the screen use the wider view, so things are live wherever they can be seen. Enemies wake earlier, and more can be active at once within the original's fixed slots.
- **The camera is the original's** (follow band, limits, the level 26 route). The view is centred on it.
- **Sprites showing earlier** at the wider edges is intended.
- **Scenes** show their 256-wide picture centred on the backdrop colour; sprites may go anywhere across the frame. Where a star field is left short, stars are invented to fill it.

## Decisions (Alex, 2026-11-06)

- **Status panel spread** (gameplay and every scene with the panel): the labels and gauges keep their place at the frame's left; pods, lives, score and weapon icon keep their distance from the right edge; a dark grey rule spans the frame above it. Replaces "centred".
- **Side stars of our own.** The title's and story's side stars get their own lines and columns, not the middle's (sharing lines made dashed rows at the edges of the middle). The approach screen's sides get the field's own star tiles in a generated pattern, not a repeat.
- **The title's Jetman crosses the whole frame** at the original's speed, so the menu lasts longer when wide. **The story's picture drifts round the whole frame.**
- **The launch scene** gets side art for 16:9, drawn for the remake from the picture's own elements and committed as our own derivative work; wider frames show black beyond it.

## Decisions (Alex, 2026-11-07)

- **Surface pictures** are widened one at a time from their own cells, as planned below. Picture 1: the floe pair moved from the picture's right to just beyond it, the small floe on the left (chosen over sparser layouts and over the pair on the left).
- **The mothership enters from the side of the widened picture**, not at screen x 255 over the right side's art.
- **The landing fits its song.** In a wider frame the descent is time-scaled per picture so that the landing and the launch after it last exactly as long as song 3, which then ends as play begins; the launch itself is not changed. The speed is in passes, so the same at any display rate. (Chosen over extra time before the descent, with the song starting at once or delayed; extending the song was considered.)

## The view (`src/game/view.rs`)

- **Width.** The height stays 240 lines (224 shown with `crop_overscan`). Width W = round(lines shown × display aspect), held to 256-854 (854 is 32:9 at 240 lines).
- **Rectangle.** A `Viewport` is the world rectangle (left, top, W): left = camera x − (W − 256) / 2, pushed back inside the columns the camera can show (0 to its limit + 256), centred on them where they are narrower than W, and always holding the camera's screen. Top is camera y. At W = 256 it is the camera's screen.
- **The engines take W at run time** (`set_view_width`, and `Seed::view_width` so a level starts with the wide windows). W is not saved: a state loads at any width.
- **Beyond the map.** The original's screen never passes a map's left or right edge (`levels.md` 5.2a). A view wider than a map shows empty sky beyond it.
- **Blank left column.** The 8-pixel blank column is a scroll artifact: blanked at 256, drawn when wider. With `crop_overscan`, a wide frame crops only the top and bottom.

## Rules measured against the view

The analyst audit of 2026-10-25 lists every rule the original measures against the screen's horizontal extent (`objects.md` 3.7). Each one marked "widen" there is restated against the `Viewport`, unchanged at W = 256, in both `src/game/gameplay` and `src/game/scaled`.

| Rule | Wide |
|---|---|
| Item activation, −48 ≤ rx, ry < 304 (`levels.md` 3.3) | rx from the view's left, up to W + 48 |
| Placed objects: in range, −margin ≤ rx < 256 + margin (`objects.md` 3.4) | up to W + margin |
| Placement scan spawns only off-screen, 0 ≤ rx < 256 (`objects.md` 3.4) | 0 ≤ rx < W |
| Despawn windows (`objects.md` 3.5) | the horizontal bounds widen by W − 256 |
| Enemies spawned just off-screen at cx − 32 / cx + 288, else the other side, else top or bottom (`behaviour.md` 2.10) | the view's left − 32 / right + 32, the map tests on that x |
| Objects drawn when −32 ≤ rx < 288, and each sprite only within the screen | up to W + 32; sprites within the frame |
| Level 26: pod kept within x 24-232 of the camera (`player.md` 9.1) | within 24 of the view's sides |
| Level 26 types 59-5D wait, invisible, until rx ≤ 0 (`behaviour.md` 4.16) | until they reach the view's left edge |
| Surface descent: the mothership starts at x 255 (`scenes.md` 8.2) | drawn coming in from beyond the frame's right edge, the descent time-scaled to its song (below) |

Vertical bounds are unchanged. Rules tied to the world or to a fixed picture stay: the camera's follow band and start, the level 26 camera-page triggers, the delivery's item place on the message screen (camera + 130, on the original's screen in the middle) and the backdrop interlude's 256-wide walls.

## Drawing

- `Frame` has a width. The playfield draws the view; objects, the pod and stars are placed on the camera's screen and moved by its offset in the view (`Viewport::shift`).
- The parallax stars wrap round the 256-pixel screen, so a wide view repeats them every 256 pixels: a star leaving the middle carries on in the next repeat. The eight stars and their job (which also gives the thrust carry-in) are unchanged.
- The panel is spread (`hud::panel_frame_x`): screen x below 132 (columns 0-15 at the panel's scroll) stays at that frame x, the rest moves right by W − 256, the gap is the backdrop. The rule is NES colour $00 on line 194 (the middle of the three blank lines above the panel; the planet map's lifted panel, 193).
- Scenes (`ppu.rs`) draw the 256-wide screen at (W − 256) / 2 with the backdrop either side, sprites anywhere across the frame, and the left-column mask on the background only. `Screen::side_sprites` holds stars for the sides: the title and the story copy each star into the four 256-wide bands either side, each copy moved by a fixed offset of its own (a hash of band and star) in x and y, wrapping in the band and the sprite lines (`ppu::side_stars`). A copy moves, flashes and hides with its star; a star hidden for good takes its copies with it.
- **Title Jetman.** With L and R columns beside the screen, his flight starts 4L frames earlier at the frame's left edge and his jetpack works throughout: the homing swing of `scenes.md` 2.3 runs from his start, with the high target's window moved 4L frames later so his long climb comes over the same part of the screen as in the original. He flies on to the right edge, still swinging. The menu lasts 994 + 4(L + R) frames.
- **Story.** `Screen::roam`: the page goes round a loop W + 128 wide, so the 128-wide picture leaves one side as it comes in at the other; no left-column mask. Its x is the scroll moved by whole pages so that from clock 600, when the scroll stays still, the picture rests where the original's screen has it (x 64-191 of the screen): the rocket blows up in the middle.
- **Launch.** `Screen::side_art`: the remake's own art for 16:9 (drawn 2026-11-06 after a reference Alex chose, in the picture's palettes 0 and 2), `src/game/scene/launch-sides.png`. It covers screen x −88 to 343, so all of a 16:9 frame (−85 to 341) and of narrower wide ones; beyond it, black. Machinery walls either side (one layout, mirrored; lit from the left like the picture's pillars); on the left the box carried on (its back edge continuing the picture's at x 0, parallel to its front edge, the front edge bent gently downward from the box's corner at x 56, a dithered rounded front and two curved grooves); on the right the monitor completed about its circle (centre (255, 175), radius 23.6, fitted to the picture's arc), a vented unit and a floor, and the tennis ball's seam carried on, bending only upward. It also repaints 269 of the picture's own pixels (the front edge from x 56, the seam from x 239) and unmasks the left column. The PNG is 8-bit indexed: index = palette × 4 + colour value, 16 where the picture shows; its colours are only for viewing. Sprites behind the background are hidden by the art as by the picture. At 256 nothing changes.
- **Surface.** `Screen::side_cells`, built by `scene/surface_sides.rs` from a recipe per picture (plan below): 11 columns either side, each cell a tile of the picture's nametable as loaded with its palette and pattern table (rows 0-15 table 1, 16-29 table 0, as the split gives them), mirrored or not. Beyond them the backdrop. Side cells hide sprites behind the background as the picture does, and a wider frame unmasks column 0. A recipe may also replace cells of the picture itself, in a wider frame only (`SideCells::inside`), away from the menu box (rows 4-15, columns 10-21), which the replaced cells would hide. Pictures without a recipe yet show the backdrop either side.
  - **Picture 1** (ice floes, planets 4 and 7): each row's most common cell as the ground. Left: the distant range of columns 0-7 again (rows 10-15; column 8 is much like column 0, so column 7 sits beside column 0 as beside column 8), without the tiny shard of columns 0-1 and the big left floe's top (column 4) in row 15; the small floe (columns 28-31, rows 18-22, without its neighbour's drips) two rows lower. Right: the picture's right end again (columns 21-31 at 32-42, rows 10-23): the distant mountain and ridges, the pair of floes in front and their drips, but not the big centre floe's edge (column 21 from row 15, column 22 from row 16). In the picture itself (`SideCells::inside`, a wider frame only), the pair gives way to each row's common cell, open ice, so it shows once, beyond the edge: columns 25-31 in rows 13-15, beside the distant mountain the tall floe stood in front of (kept whole in columns 22-24 with its shaded face), and columns 24-31 in rows 16-23.
  - **Picture 6** (rock towers, planet 1): each row's most common cell, the sky and the black ground. Above the horizon (rows 15-19), the spire groups of the picture's left, columns 0-4 (the big spire), 5-9 (the middle two) and 10-11 (the small one), mirrored or not, and plain hill (sky cells over ground cells, its horizon at line 142 or 150), joined only where their horizon lines meet, so each spire shows at most once more. Left, outwards: plain hill at line 150 (where column 0's starts), the big spire mirrored, the small spire. Right: column 11 (from 3 px below the tower's dark base, which ends at line 126, down to 141), plain hill at 142, the middle spires mirrored, plain hill. Black ground for the edge of the small spire beside the tower's base and for the top of the boulder with the hole where those were copied. Below the horizon the ground stays open, but for the end of the lower-left boulder, which the picture's edge cuts (rock at x 0 from line 197 to 221): its first column mirrored to column −1 and rounded off by our own art, `src/game/scene/surface-6-sides.png` (`SideArt::Surface6`: ground beyond a curve that carries the boulder's top edge on down and its lower edge up to meet at x −6, with an orange rim), drawn only in a wider frame. The same art puts black rock behind the lit faces of the two mirrored spires (the original's spires have it on their right, which mirroring moves to the left): from two lines under each one's top, a pixel wider every two lines, until it meets the hill, over sky only.
  - **Picture 5** (crystals, planets 2 and 9) — **in progress, not finished** (see the handoff note below the plan). Cells: each row's most common cell; on the left, sky down to row 12 (the cell at (18, 12)); the horizon's gold line and pink band (rows 13-15: (25, 13), (24-27, 14), (25, 15)) on both sides; on the left, the two columns by the edge mirrored about it in rows 16-19 (the middle crystal the edge cuts) and 20-23 (the box crystal). Our own art over them, `src/game/scene/surface-5-sides.png` (`SideArt::Surface5`), generated by `tools/art/surface-5-sides.py` (deterministic, standard library only; `--crystals 8 --bush 2`, the defaults). Left: only the top crystal's corner (its top edge from (0, 72) to (-3, 75), its doubled gold left edge down x -3 and -2 to line 125, the face's shading beside it) and a clump of small purple crystals at line 205-221. Right, below line 46: the bush made whole, its right half a crown of round clumps of leaves (each with a gold rim and an inner arc of leaves) from its peak by the edge (line 61 at x 252) curving under onto a trunk (design 2) that stands on the ground, the horizon's cells showing under the crown beside it; below, bands of purple grass, pink crystals (carrying on from line 143), gold grass and clumps of purple crystals rising to the right, scattered by a fixed hash. One palette per 8 × 8 cell of the picture's grid, in the picture's palettes, so the art takes each planet's colours.
- **Approach.** `Screen::side_field`: each cell beside the field's 24 rows holds one of the field's star tiles (from the picture before any text) with the field's density × 1.2, chosen by a hash of the cell, and none next to a star on its left or above.

## Surface pictures (plan, 2026-11-07)

The planet surface (`scenes.md` 8) shows one of six pictures (0, 1, 2, 4, 5, 6; picture 3 is empty) in the planet's own colours. The plan is to widen each one picture at a time, in the order below. Mock-ups of every picture were made first, from the picture's own cells.

**Method.** The sides are built mostly from the picture's own 8 × 8 cells: copied, or mirrored, from its nametable as loaded, before the menu box is drawn. A short recipe per picture, written by us, says which cell goes where. Because a cell keeps its tile and palette number, the sides take each planet's colours by themselves, and no graphics of the ROM go into git. Where copying leaves a seam that only drawing can hide, a small indexed PNG of our own goes over the cells, in the picture's palettes, as with the launch scene's art. The sides cover 16:9 (11 columns either side); wider frames show the backdrop beyond them. They take part in sprite priority like the picture, so a ship behind the background is hidden by them too. Column 0, masked on the original, is shown when the frame is wider. At 256 nothing changes.

| Order | Picture (planets) | What copies well | What needs drawing |
|---|---|---|---|
| 1 (done) | 1, ice floes (4, 7) | the horizontal bands of sky and distant ridges; the floe pair moved from the picture's right to just beyond it, the small floe again on the left | nothing |
| 2 (done) | 6, rock towers (1) | the flat sky and black ground; the spire groups, mirrored or not, with plain hill between, joined at the horizon line | the lower-left boulder's end beyond x 0 |
| 3 (in progress) | 5, crystals (2, 9) | the sky bands; the horizon on both sides; the middle and box crystals at the left edge mirrored about it | the top crystal's corner; the bush made whole with a trunk and the ground's bands on the right (both by a script) |
| 4 | 2, canyon and sun (5, 8, 10) | the sky bands | both canyon walls (repeated spires show a rhythm): a black silhouette with a lit edge, the left wall climbing toward the top, the right one rising off the top |
| 5 | 4, river and tree (0, 6) | the trunk and canopy mirrored into a whole tree; the left mesa mirrored; the grass and ledges; sky stars | the river's shoreline at the left, the grass seams on the right. The ship is behind the background from x 216, so behind the new trunk too |
| 6 | 0, alien trees (3, 11, 12) | the horizon and far dunes; the small tree again on the right; the plateau's end mirrored on the left | the right side's plateau rim and pink foliage, the left ridge's mirror seam, the cloned tree's trunk base |

**Handoff: picture 5 (2026-11-10).** Alex has approved the right side (the bush with trunk 2, the horizon, the ground's bands). The left side's crystals are not approved yet. What was tried and turned down, in order:
- the whole cluster mirrored about x 0 (a symmetric "crown"), also with its upper and lower halves shifted a row or two: the crystals don't line up;
- the sides pasted from Alex's concept image (now gone): it didn't line up either, and Alex wants art made from shapes by a script, not pasted;
- whole new crystals drawn by the script (designs 1-5 and 7 in `tools/art/surface-5-sides.py`): they don't look like the picture's crystals.

Alex's latest direction: draw only the three crystals the picture's left edge cuts. The top one needs only its corner filled in (done: design 8). The middle and bottom ones should be almost mirrors of the matching crystals on the other side of the edge. Now they are the picture's cells mirrored, two columns wide, in rows 16-19 and 20-23 (`CRYSTALS` in `src/game/scene/surface_sides.rs`). Still open:
- the middle mirror turns the picture's lines into chevrons, not a clear crystal, and it may not be the crystal Alex means (the lines leaving the edge at (0, 106), (0, 115), (0, 141) and (0, 154));
- the mirrored strips stop at x -16, which cuts some lines.

How to iterate:
- **Render:** `cargo build --release --bin sj-tools`, then `sj-tools scene --surface 2 --aspect 16:9 --frames 900 --out DIR` (planet 9 for the other palette), and compare with `--surface 2 --frames 900` at 4:3.
- **Regenerate the art:** `python3 tools/art/surface-5-sides.py` after changing it. The PNG must stay 8-bit indexed (index = palette × 4 + value, 16 = the cells show).
- **Shading:** the picture's crystals are shaded with `band_shade`: inside a lit edge, a gold and dark-gold check, then solid dark gold, then dark checks, then sparse specks.
- **Tests:** `surface_5_art_fills_the_sides_below_the_sky_bands_in_the_pictures_palettes` (`src/game/scene/ppu.rs`) pins where the art may draw; change it with the art.
- **Before finishing:** update this note and the picture 5 entry above, remove the turned-down designs from the script, and run the usual checks (`CLAUDE.md`).

**The mothership's landing** (`Surface::enter_from_the_edge`), in a frame with R > 0 columns right of the screen:

- **Entry.** The mothership moves as on the original, from x 255, but is drawn further right by a lead of R + 32 px (past its widest layout) that shrinks with the square of the passes left, to nothing at the pass the original's braking starts (the 8-bit distance before the move first below 37). It flies in faster and has slowed to the original's 0.75 px a pass by then.
- **Time scale.** The original's landing with nothing owned (descent, arrival, the small object's 128 passes) takes D passes, and the launch after it L = 358 passes, or 357 when it opens on an odd frame counter (`scenes.md` 9). The remake plays those D passes in 1,077 − L (song 3's length, `scenes.md` 15), so the level's song is asked for exactly 1,077 passes after song 3: D = 431 to 755 becomes 719 or 720. Each pass owes D / (1,077 − L) of the original's (an integer accumulator: none, one or two run in a pass, each with the frame counter's increment), so the frame counter at the launch is the original's and the launch is the original's pass for pass. Buttons pressed in a pass that runs none wait for the next (A still ends the descent at once). The mothership, its flame and the small object are drawn on their way between the original's last two passes, by the accumulator's fraction, so a slowed descent moves smoothly. With something owned the descent runs at the same speed and ends at its arrival as on the original.
- **Any display rate.** The program runs passes 60 a second on its own clock whatever the display's rate (30 Hz draws two passes a frame, 120 Hz each pass twice), and the scale counts passes, so the landing takes the same time at any rate.
- A width set after the landing has opened but before its first pass sets the lead and scale too (and moves the sprites already placed). The docked menu and the departure are unchanged; at 256 nothing changes.

## Tools

`sj-tools render` and `sj-tools scene` take `--aspect W:H` (16:9, 21:9, ...). `scene --compare` refuses it. `sj-tools scene --frames` takes ranges (`530-1420/2`), and `--animate FILE.png` writes the frames as one animated PNG to watch in a browser (`--scale N` draws each pixel N × N), e.g. `sj-tools scene --surface 4 --aspect 16:9 --frames 530-1420/2 --animate landing.png --scale 3`.

## Open

- **Flight scene stars** are scene objects that vanish when they leave x 0-255, in the middle of a wide view. Letting them fly on would change when new ones start, and the scene's random numbers carry into the level.
