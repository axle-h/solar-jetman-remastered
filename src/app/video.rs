//! The window and input: shows the framebuffer scaled by whole pixels with no smoothing,
//! centred with black bars, and turns keyboard and gamepad events into the controls bound to
//! them (`controls`).

use crate::controls::{Control, Device, Keys, Pad, Pressed};
use crate::font::Surface;
use crate::menu::Nav;
use crate::settings::Settings;
use sdl3::event::Event;
use sdl3::gamepad::Gamepad;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::{FRect, ScaleMode, Texture, TextureCreator, WindowCanvas};
use sdl3::video::WindowContext;
use sdl3::{EventPump, GamepadSubsystem, Sdl};
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::playfield::Frame;
use solar_jetman::game::viewer::ViewerInput;
use solar_jetman::game::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::path::PathBuf;

fn sdl_error(error: impl std::fmt::Display) -> String {
    format!("SDL: {error}")
}

/// The window and its renderer.
pub struct Screen {
    sdl: Sdl,
    canvas: WindowCanvas,
    rgb: [[u8; 3]; 64],
    pixels: Vec<u8>,
    /// The part of the picture shown: left, top, width, height.
    visible: (u32, u32, u32, u32),
}

/// The part of the picture shown: all of it, or without the edges a television's overscan
/// hides (8 lines at the top and bottom) and the 8-pixel column at the left that the game
/// blanks (`Settings::crop_overscan`).
fn visible_area(settings: &Settings) -> (u32, u32, u32, u32) {
    let (width, height) = (SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    if settings.crop_overscan {
        (8, 8, width - 8, height - 16)
    } else {
        (0, 0, width, height)
    }
}

impl Screen {
    pub fn open(settings: &Settings) -> Result<Screen, String> {
        let sdl = sdl3::init().map_err(sdl_error)?;
        let video = sdl.video().map_err(sdl_error)?;
        // The bindings have no setter for the 2D renderer's vsync; SDL reads this hint when
        // the renderer is created.
        sdl3::hint::set("SDL_RENDER_VSYNC", if settings.vsync { "1" } else { "0" });
        let scale = settings.scale.max(3);
        let visible = visible_area(settings);
        let window = video
            .window(
                "Solar Jetman remastered",
                visible.2 * scale,
                visible.3 * scale,
            )
            .resizable()
            .high_pixel_density()
            .build()
            .map_err(sdl_error)?;
        let canvas = window.into_canvas();
        let mut screen = Screen {
            sdl,
            canvas,
            rgb: solar_jetman::game::ntsc::master_palette(),
            pixels: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 3],
            visible,
        };
        screen.apply(settings, None)?;
        Ok(screen)
    }

    /// Puts the video settings into effect. `before` is the settings in force until now, so
    /// that only what changed is touched.
    pub fn apply(&mut self, settings: &Settings, before: Option<&Settings>) -> Result<(), String> {
        let changed = |f: fn(&Settings) -> bool| before.is_none_or(|b| f(b) != f(settings));
        if changed(|s| s.vsync) {
            // SAFETY: the renderer pointer is valid as long as the canvas is alive.
            unsafe {
                sdl3::sys::render::SDL_SetRenderVSync(self.canvas.raw(), i32::from(settings.vsync));
            }
        }
        if changed(|s| s.fullscreen) {
            self.canvas
                .window_mut()
                .set_fullscreen(settings.fullscreen)
                .map_err(sdl_error)?;
        }
        self.visible = visible_area(settings);
        if !settings.fullscreen
            && settings.scale > 0
            && before.is_none_or(|b| {
                b.scale != settings.scale
                    || b.fullscreen
                    || b.crop_overscan != settings.crop_overscan
            })
        {
            let scale = settings.scale;
            self.canvas
                .window_mut()
                .set_size(self.visible.2 * scale, self.visible.3 * scale)
                .map_err(sdl_error)?;
        }
        Ok(())
    }

    pub fn window(&self) -> &sdl3::video::Window {
        self.canvas.window()
    }

    /// The last presented picture as packed RGB, [`SCREEN_WIDTH`] by [`SCREEN_HEIGHT`].
    pub fn picture(&self) -> &[u8] {
        &self.pixels
    }

    /// The SDL context, for other subsystems (audio).
    pub fn sdl(&self) -> &Sdl {
        &self.sdl
    }

    pub fn set_title(&mut self, title: &str) -> Result<(), String> {
        self.canvas.window_mut().set_title(title).map_err(sdl_error)
    }

    /// Makes textures for this window's renderer.
    pub fn texture_creator(&self) -> TextureCreator<WindowContext> {
        self.canvas.texture_creator()
    }

    /// Shows a frame: NES colours to RGB, then scaled by the largest whole factor that fits.
    /// `overlay` draws over the converted picture (the menu, a message).
    pub fn present(
        &mut self,
        texture: &mut Texture,
        frame: &Frame,
        overlay: impl FnOnce(&mut Surface),
    ) -> Result<(), String> {
        for (rgb, &colour) in self
            .pixels
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(&frame.pixels)
        {
            rgb.copy_from_slice(&self.rgb[usize::from(colour & 0x3F)]);
        }
        overlay(&mut Surface {
            pixels: &mut self.pixels,
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
        });
        texture
            .update(None, &self.pixels, SCREEN_WIDTH * 3)
            .map_err(sdl_error)?;
        let (width, height) = self.canvas.output_size().map_err(sdl_error)?;
        let (left, top, shown_w, shown_h) = self.visible;
        let scale = (width / shown_w).min(height / shown_h).max(1);
        let (w, h) = (shown_w * scale, shown_h * scale);
        let x = (width.saturating_sub(w) / 2) as f32;
        let y = (height.saturating_sub(h) / 2) as f32;
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();
        let source = FRect::new(left as f32, top as f32, shown_w as f32, shown_h as f32);
        self.canvas
            .copy(texture, source, FRect::new(x, y, w as f32, h as f32))
            .map_err(sdl_error)?;
        self.canvas.present();
        Ok(())
    }
}

/// A streaming texture the size of the screen, sampled without smoothing.
pub fn frame_texture(creator: &TextureCreator<WindowContext>) -> Result<Texture<'_>, String> {
    let mut texture = creator
        .create_texture_streaming(
            PixelFormat::RGB24,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
        )
        .map_err(sdl_error)?;
    texture.set_scale_mode(ScaleMode::Nearest);
    Ok(texture)
}

/// What happened since the last poll.
#[derive(Debug, Default)]
pub struct Events {
    /// The window was closed, or the quit control pressed.
    pub quit: bool,
    pub menu_toggle: bool,
    pub screenshot: bool,
    pub quick_save: bool,
    pub quick_load: bool,
    /// Menu controls, in order, from the game's buttons.
    pub nav: Vec<Nav>,
    /// Files dropped on the window.
    pub dropped: Vec<PathBuf>,
    /// The key or button the port menu was waiting for ([`Input::set_listening`]).
    pub bound: Option<Pressed>,
}

/// Keyboard and gamepads, through the bindings in the settings.
pub struct Input {
    pump: EventPump,
    gamepads: GamepadSubsystem,
    open: Vec<Gamepad>,
    keys: Keys,
    pad: Pad,
    /// The device whose next key or button goes to the port menu as a binding.
    listen: Option<Device>,
    menu_open: bool,
    /// The game's buttons pressed since [`Input::take_pressed`] last took them, so that a press
    /// shorter than a frame still reaches the game.
    pressed: Buttons,
    /// Buttons held while the menu was open, kept from the game until released, so that the
    /// press that closes the menu does not also act in the game.
    blocked: u8,
}

/// What a control does besides being the game's button: as the controller drives the port
/// menu, A and Start accept, B goes back and Select moves down, as in the game's own menus.
fn press(control: Control, repeat: bool, events: &mut Events) {
    let nav = match control {
        Control::Up => Some(Nav::Up),
        Control::Down | Control::Select => Some(Nav::Down),
        Control::Left => Some(Nav::Left),
        Control::Right => Some(Nav::Right),
        Control::A | Control::Start if !repeat => Some(Nav::Accept),
        Control::B if !repeat => Some(Nav::Back),
        _ => None,
    };
    events.nav.extend(nav);
    if repeat {
        return;
    }
    match control {
        Control::Menu => events.menu_toggle = true,
        Control::QuickSave => events.quick_save = true,
        Control::QuickLoad => events.quick_load = true,
        Control::Screenshot => events.screenshot = true,
        Control::Quit => events.quit = true,
        _ => {}
    }
}

impl Input {
    pub fn new(screen: &Screen, settings: &Settings) -> Result<Input, String> {
        let pump = screen.sdl.event_pump().map_err(sdl_error)?;
        let gamepads = screen.sdl.gamepad().map_err(sdl_error)?;
        Ok(Input {
            pump,
            gamepads,
            open: Vec::new(),
            keys: settings.keys.clone(),
            pad: settings.pad.clone(),
            listen: None,
            menu_open: false,
            pressed: Buttons(0),
            blocked: 0,
        })
    }

    /// Takes up changed bindings.
    pub fn set_controls(&mut self, settings: &Settings) {
        self.keys = settings.keys.clone();
        self.pad = settings.pad.clone();
    }

    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    /// Sends the next key or button of `device` to the menu instead of acting on it.
    pub fn set_listening(&mut self, device: Option<Device>) {
        self.listen = device;
    }

    pub fn set_menu_open(&mut self, open: bool) {
        self.menu_open = open;
        let held = self.held_mask();
        self.blocked = if open { held } else { self.blocked & held };
    }

    pub fn poll(&mut self) -> Events {
        let mut events = Events::default();
        for event in self.pump.poll_iter() {
            let mut control = None;
            match event {
                Event::Quit { .. } => events.quit = true,
                Event::DropFile { filename, .. } => events.dropped.push(filename.into()),
                Event::KeyDown {
                    scancode: Some(key),
                    repeat,
                    ..
                } => {
                    if self.listen == Some(Device::Keyboard) {
                        if !repeat {
                            events.bound = Some(Pressed::Key(key));
                            self.listen = None;
                        }
                    } else if let Some(bound) = self.keys.control(key) {
                        control = Some((bound, repeat));
                    }
                }
                Event::GamepadButtonDown { button, .. } => {
                    let held = |control| {
                        self.pad
                            .get(control)
                            .is_some_and(|button| self.open.iter().any(|p| p.button(button)))
                    };
                    if self.listen == Some(Device::Gamepad) {
                        events.bound = Some(Pressed::Button(button));
                        self.listen = None;
                    } else if self.pad.get(Control::Start) == Some(button) && held(Control::Select)
                    {
                        // For pads whose guide button the system keeps: Select held with Start.
                        events.menu_toggle = true;
                    } else if let Some(bound) = self.pad.control(button) {
                        control = Some((bound, false));
                    }
                }
                Event::GamepadAdded { which, .. } => {
                    // A pad that cannot be opened is simply not used.
                    if let Ok(pad) = self.gamepads.open(which) {
                        self.open.push(pad);
                    }
                }
                Event::GamepadRemoved { which, .. } => {
                    self.open.retain(|pad| pad.id().ok() != Some(which));
                }
                _ => {}
            }
            if let Some((control, repeat)) = control {
                if !repeat && !self.menu_open {
                    self.pressed.0 |= control.mask();
                }
                press(control, repeat, &mut events);
            }
        }
        events
    }

    /// The game's buttons held now on the keyboard and any gamepad.
    fn held_mask(&self) -> u8 {
        let keys = self.pump.keyboard_state();
        let mut held = 0;
        for control in Control::ALL {
            if self
                .keys
                .get(control)
                .is_some_and(|key| keys.is_scancode_pressed(key))
                || self
                    .pad
                    .get(control)
                    .is_some_and(|button| self.open.iter().any(|p| p.button(button)))
            {
                held |= control.mask();
            }
        }
        held
    }

    /// The game's controller as it is held now.
    pub fn buttons(&self) -> Buttons {
        Buttons(self.held_mask() & !self.blocked)
    }

    /// The game's buttons pressed since the last call, held now or not.
    pub fn take_pressed(&mut self) -> Buttons {
        std::mem::take(&mut self.pressed)
    }

    /// The directions held now, for the viewer.
    pub fn held(&self) -> ViewerInput {
        let held = self.buttons().0;
        ViewerInput {
            left: held & Buttons::LEFT != 0,
            right: held & Buttons::RIGHT != 0,
            up: held & Buttons::UP != 0,
            down: held & Buttons::DOWN != 0,
        }
    }
}
