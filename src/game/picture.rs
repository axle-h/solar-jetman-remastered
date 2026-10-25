//! One picture of gameplay, and its drawing, shared by both engines: each engine works out what
//! is shown (where the camera, the objects and the pod are, which tiles and colours have
//! changed) and this module draws it.

use crate::game::gameplay::objects::{Object, state};
use crate::game::gameplay::sprites::{SpriteFile, Sprites};
use crate::game::graphics::LevelGraphics;
use crate::game::hud::Hud;
use crate::game::level::{Point, World};
use crate::game::playfield::{Frame, blank_left_columns, draw_playfield};
use crate::game::view::Viewport;
use std::borrow::Cow;

/// The pod's sprite: its facing, whether it is mirrored, the hit pause (whose flashes pick the
/// palette) and the palette bits, and its object point on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PodPicture {
    pub facing: u8,
    pub mirrored: bool,
    pub hit_pause: u8,
    pub palette_bits: u8,
    pub at: (i32, i32),
}

/// What one picture of gameplay shows.
pub struct Picture<'a> {
    pub level: usize,
    /// The level's tiles and colours with this moment's changes (the warp mesh, the terrain's
    /// colours, the gate's fades).
    pub graphics: &'a LevelGraphics,
    /// The sprite tiles with this moment's changes ([`dress_sprites`]).
    pub sprites: &'a Sprites,
    /// The top-left of the playfield, in whole pixels.
    pub camera: Point,
    /// The world the frame shows (`docs/widescreen.md`): the camera's screen, or wider around
    /// it. Object, star and pod places are on the camera's screen and move with it.
    pub view: Viewport,
    /// The objects in slots 1-30 to draw, back to front, with their places on the screen (the
    /// beam goes behind them all, [`Picture::draw`]).
    pub objects: Vec<(usize, Object, (i32, i32))>,
    /// In the mothership's bay: the pod and slots 1 and 2 are drawn behind the background
    /// (RAM $98 bit 5; objects.md 6.3, 6.4).
    pub behind: bool,
    /// The parallax stars: screen x, y and sprite palette.
    pub stars: &'a [(u8, u8, u8)],
    pub hud: Hud,
    /// The fitted weapon's picture in the status panel.
    pub panel_icon: Option<u8>,
    pub pod: PodPicture,
}

/// The sprite tiles changed for this moment: Jetman's figure in the pod's first tiles
/// (graphics.md 5.7.4), the beam's tile $CF holding the image of the rope's angle (player.md
/// 8.4) and the bonus sign's tiles $BC and $D0 holding the last points picture written
/// (graphics.md 3.1).
pub fn dress_sprites<'a>(
    sprites: &'a Sprites,
    jetman: bool,
    beam: Option<&[u8]>,
    sign: Option<&[u8]>,
) -> Cow<'a, Sprites> {
    let mut dressed = Cow::Borrowed(sprites);
    if jetman {
        dressed = Cow::Owned(dressed.with_jetman());
    }
    if let Some(pixels) = beam {
        dressed = Cow::Owned(dressed.with_tile(0xCF, pixels));
    }
    if let Some(pair) = sign {
        dressed = Cow::Owned(
            dressed
                .with_tile(0xBC, &pair[..16])
                .with_tile(0xD0, &pair[16..]),
        );
    }
    dressed
}

impl Picture<'_> {
    pub fn draw(&self, frame: &mut Frame, world: &World, layouts: &SpriteFile) {
        let graphics = self.graphics;
        draw_playfield(frame, world, &world.levels[self.level], graphics, self.view);
        let shift = self.view.shift(self.camera);
        let behind = if self.behind { 0x20 } else { 0 };
        // The tow beam is drawn under every object, so the rope meets the item it holds; the
        // original swaps it in front and behind the item step by step (docs/deviations.md).
        let is_beam = |slot: usize, object: &Object| slot == 2 && object.state == state::BEAM;
        let beam = self.objects.iter().filter(|(s, o, _)| is_beam(*s, o));
        let others = self.objects.iter().filter(|(s, o, _)| !is_beam(*s, o));
        for &(slot, object, (x, y)) in beam.chain(others) {
            let at = (x + shift, y);
            let mut object = object;
            if slot <= 2 && behind != 0 {
                // Bit 0 applies the mask, bit 1 clear mirrors, bit 6 is attribute bit 5.
                object.flags_x = if object.flags_x & 1 != 0 && object.flags_x & 2 == 0 {
                    object.flags_x
                } else {
                    3
                } | behind << 1;
            }
            self.sprites
                .draw_object(frame, layouts, &graphics.palette, slot, &object, at);
        }
        self.draw_stars(frame, world);
        self.hud.draw(frame, graphics);
        if let Some(item) = self.panel_icon {
            crate::game::hud::draw_weapon_icon(frame, graphics, item);
        }
        let pod = self.pod;
        self.sprites.draw_pod(
            frame,
            layouts,
            &graphics.palette,
            (pod.facing, pod.mirrored),
            (pod.hit_pause, pod.palette_bits | behind),
            (pod.at.0 + shift, pod.at.1),
        );
    }

    /// The parallax stars: one dot each at (x, y + 1), behind the background (hidden wherever
    /// the background's colour index is not 0, whatever its colour) and behind the objects,
    /// hidden in the blanked left columns and below the playfield (graphics.md 5.7.3). A wider
    /// view repeats them every 256 pixels either side: the stars wrap round the screen, so a
    /// star leaving the middle carries on in the next repeat (`docs/widescreen.md`).
    fn draw_stars(&self, frame: &mut Frame, world: &World) {
        let graphics = self.graphics;
        let palette = &graphics.palette;
        let level = &world.levels[self.level];
        let shift = self.view.shift(self.camera);
        let screen = crate::game::SCREEN_WIDTH as i32;
        let repeats = (frame.width as i32 - 1) / screen;
        let copies = (-repeats..=repeats).map(|k| k * screen);
        for (offset, &(x, y, sprite_palette)) in
            copies.flat_map(|offset| self.stars.iter().map(move |star| (offset, star)))
        {
            let line = usize::from(y) + 1;
            let x = i32::from(x) + shift + offset;
            if x < blank_left_columns(frame.width) as i32
                || x >= frame.width as i32
                || y >= 0xF0
                || line >= crate::game::playfield::PLAYFIELD_LINES
            {
                continue;
            }
            let x = x as usize;
            let at = line * frame.width + x;
            let index = crate::game::playfield::background_value(
                world,
                level,
                graphics,
                self.view,
                (x, line),
            );
            if index == 0 && frame.pixels[at] == palette.universal_background() {
                frame.pixels[at] = palette.0[16 + usize::from(sprite_palette) * 4 + 1];
            }
        }
    }
}
