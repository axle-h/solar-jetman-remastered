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

Vertical bounds are unchanged. Rules tied to the world or to a fixed picture stay: the camera's follow band and start, the level 26 camera-page triggers, the delivery's item place on the message screen (camera + 130, on the original's screen in the middle) and the backdrop interlude's 256-wide walls.

## Drawing

- `Frame` has a width. The playfield draws the view; objects, the pod and stars are placed on the camera's screen and moved by its offset in the view (`Viewport::shift`).
- The parallax stars wrap round the 256-pixel screen, so a wide view repeats them every 256 pixels: a star leaving the middle carries on in the next repeat. The eight stars and their job (which also gives the thrust carry-in) are unchanged.
- The panel is spread (`hud::panel_frame_x`): screen x below 132 (columns 0-15 at the panel's scroll) stays at that frame x, the rest moves right by W − 256, the gap is the backdrop. The rule is NES colour $00 on line 194 (the middle of the three blank lines above the panel; the planet map's lifted panel, 193).
- Scenes (`ppu.rs`) draw the 256-wide screen at (W − 256) / 2 with the backdrop either side, sprites anywhere across the frame, and the left-column mask on the background only. `Screen::side_sprites` holds stars for the sides: the title and the story copy each star into the four 256-wide bands either side, each copy moved by a fixed offset of its own (a hash of band and star) in x and y, wrapping in the band and the sprite lines (`ppu::side_stars`). A copy moves, flashes and hides with its star; a star hidden for good takes its copies with it.
- **Title Jetman.** With L and R columns beside the screen, his flight starts 4L frames earlier at the frame's left edge and his jetpack works throughout: the homing swing of `scenes.md` 2.3 runs from his start, with the high target's window moved 4L frames later so his long climb comes over the same part of the screen as in the original. He flies on to the right edge, still swinging. The menu lasts 994 + 4(L + R) frames.
- **Story.** `Screen::roam`: the page goes round a loop W + 128 wide, so the 128-wide picture leaves one side as it comes in at the other; no left-column mask. Its x is the scroll moved by whole pages so that from clock 600, when the scroll stays still, the picture rests where the original's screen has it (x 64-191 of the screen): the rocket blows up in the middle.
- **Launch.** `Screen::side_art`: the remake's own art for 16:9 (drawn 2026-11-06 after a reference Alex chose, in the picture's palettes 0 and 2), `src/game/scene/launch-sides.png`. It covers screen x −88 to 343, so all of a 16:9 frame (−85 to 341) and of narrower wide ones; beyond it, black. Machinery walls either side (one layout, mirrored; lit from the left like the picture's pillars); on the left the box carried on (its back edge continuing the picture's at x 0, parallel to its front edge, the front edge bent gently downward from the box's corner at x 56, a dithered rounded front and two curved grooves); on the right the monitor completed about its circle (centre (255, 175), radius 23.6, fitted to the picture's arc), a vented unit and a floor, and the tennis ball's seam carried on, bending only upward. It also repaints 269 of the picture's own pixels (the front edge from x 56, the seam from x 239) and unmasks the left column. The PNG is 8-bit indexed: index = palette × 4 + colour value, 16 where the picture shows; its colours are only for viewing. Sprites behind the background are hidden by the art as by the picture. At 256 nothing changes.
- **Approach.** `Screen::side_field`: each cell beside the field's 24 rows holds one of the field's star tiles (from the picture before any text) with the field's density × 1.2, chosen by a hash of the cell, and none next to a star on its left or above.

## Tools

`sj-tools render` and `sj-tools scene` take `--aspect W:H` (16:9, 21:9, ...). `scene --compare` refuses it.

## Open

- **Flight scene stars** are scene objects that vanish when they leave x 0-255, in the middle of a wide view. Letting them fly on would change when new ones start, and the scene's random numbers carry into the level.
