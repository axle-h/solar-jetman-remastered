//! The Android build: the program in `main.rs` as a library, which SDL's Java activity
//! (`android/`) loads and runs through `SDL_main`. Built only by `tools/package/android.sh`
//! (`docs/android.md`).

use std::ffi::{c_char, c_int};

#[path = "main.rs"]
mod app;

/// Called by SDL's activity on its own thread, once the Java side is ready.
#[unsafe(no_mangle)]
pub extern "C" fn SDL_main(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
    // stderr goes nowhere on Android; SDL's log reaches logcat.
    std::panic::set_hook(Box::new(|info| {
        sdl3::log::log_error(sdl3::log::Category::Application, &info.to_string());
    }));
    let code = match app::run() {
        Ok(()) => 0,
        Err(message) => {
            sdl3::log::log_error(sdl3::log::Category::Application, &message);
            let _ = sdl3::messagebox::show_simple_message_box(
                sdl3::messagebox::MessageBoxFlag::ERROR,
                "Solar Jetman remastered",
                &message,
                None,
            );
            1
        }
    };
    // Android may keep the process and hand it the next launch with every static still set
    // (SDL's among them), so end it for a clean start.
    std::process::exit(code)
}
