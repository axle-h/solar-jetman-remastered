# Widescreen

A port-menu option, VIDEO > ASPECT, that widens the field of view: the frame grows sideways and the game shows more of the world. Nothing is stretched or cropped. On ORIGINAL (the default) everything is exactly as on the original's 256 pixels; replays, Mesen comparisons and `sj-tools` renders run that way unless `--aspect` is given.

## Decisions (Alex, 2026-10-25)

- **ASPECT: ORIGINAL / NATIVE.** ORIGINAL is the 256-wide frame of square pixels (16:15, not 4:3). NATIVE takes the shape of the display the window is on, so windowed and fullscreen match.
- **The logic widens with the view.** Spawning, despawning and every other rule measured against the screen use the wider view, so things are live wherever they can be seen. Enemies wake earlier, and more can be active at once within the original's fixed slots.
- **The camera is the original's** (follow band, limits, the level 26 route). The view is centred on it.
- **Status panel centred**, the sides in the backdrop colour.
- **Scenes** show their 256-wide picture centred on the backdrop colour; sprites may go anywhere across the frame. Where a star field is left short, stars are invented to fill it.
- **Sprites showing earlier** at the wider edges is intended.

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
- The HUD is drawn at x (W − 256) / 2.
- Scenes (`ppu.rs`) draw the 256-wide screen at (W − 256) / 2 with the backdrop either side, sprites anywhere across the frame, and the left-column mask on the background only. `Screen::side_sprites` holds stars for the sides: the title and the story copy their star fields into the bands either side, each star with another star's x (`ppu::side_stars`).

## Tools

`sj-tools render` and `sj-tools scene` take `--aspect W:H` (16:9, 21:9, ...). `scene --compare` refuses it.

## Open

- **Approach screen stars** are background tiles of its picture, so its sides have none.
- **Flight scene stars** are scene objects that vanish when they leave x 0-255, in the middle of a wide view. Letting them fly on would change when new ones start, and the scene's random numbers carry into the level.
