//! The game: deterministic, platform-free, stepped one logic tick at a time.
#![forbid(unsafe_code)]

pub mod gameplay;
pub mod graphics;
pub mod hex;
pub mod hud;
pub mod level;
pub mod ntsc;
pub mod password;
pub mod picture;
pub mod pilot;
pub mod playfield;
pub mod scaled;
pub mod scene;
pub mod sound;
pub mod state;
pub mod view;
pub mod viewer;

/// The NES's frame rate, which the game's logic tick follows.
pub const TICK_HZ: f64 = 60.0988;

/// Width and height of the framebuffer of NES colour indices.
pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;
