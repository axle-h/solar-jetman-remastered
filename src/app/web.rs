//! The browser build's glue (`wasm32-unknown-emscripten`): where the player's folder is, how
//! the main loop yields to the page, and syncing the folder to IndexedDB. The page
//! (`web/shell.html`) mounts IDBFS at [`FOLDER`] before the program starts, and puts a ROM the
//! player picks or drops at [`IMPORT_FILE`]. Off the web these are plain std calls.

use std::time::Duration;

/// The player's folder in the browser's file system.
#[cfg(target_os = "emscripten")]
pub const FOLDER: &str = "/solar-jetman";

/// A ROM file the page has received, waiting to be imported.
#[cfg(target_os = "emscripten")]
pub const IMPORT_FILE: &str = "/solar-jetman/import.nes";

#[cfg(target_os = "emscripten")]
mod imp {
    use std::ffi::{CString, c_char};
    use std::time::Duration;

    unsafe extern "C" {
        fn emscripten_sleep(ms: u32);
        fn emscripten_run_script(script: *const c_char);
    }

    /// Gives the browser the thread (ASYNCIFY) for about `time`, so the page can draw, take
    /// input and finish its IndexedDB work.
    pub fn pause(time: Duration) {
        // SAFETY: plain call into the emscripten runtime, no pointers.
        unsafe { emscripten_sleep(time.as_millis() as u32) };
    }

    /// Writes the folder back to IndexedDB (the page's `sjSync`, which batches calls).
    pub fn sync_folder() {
        if let Ok(script) = CString::new("if (window.sjSync) sjSync();") {
            // SAFETY: a valid NUL-terminated string that outlives the call.
            unsafe { emscripten_run_script(script.as_ptr()) };
        }
    }
}

#[cfg(target_os = "emscripten")]
pub use imp::sync_folder;

#[cfg(not(target_os = "emscripten"))]
pub fn sync_folder() {}

/// Waits for `time`; in the browser, hands the thread back to the page meanwhile.
pub fn pause(time: Duration) {
    #[cfg(target_os = "emscripten")]
    imp::pause(time);
    #[cfg(not(target_os = "emscripten"))]
    std::thread::sleep(time);
}
