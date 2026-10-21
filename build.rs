//! Writes `embedded_rom.rs` to `$OUT_DIR`, with `EMBEDDED_ROM: Option<&[u8]>`: debug builds
//! embed the player's ROM from `vendor/rom/*.nes` when it is there, so the game starts without
//! an import step. Release builds never embed it: this script writes an empty slot for them and
//! refuses outright if asked to embed.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    embed_rom();
}

fn first_rom(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("nes"))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

fn embed_rom() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("set by cargo"));
    let roms = manifest.join("vendor/rom");
    println!("cargo:rerun-if-changed={}", roms.display());
    println!("cargo:rerun-if-env-changed=SJ_NO_EMBED");

    // "debug" for the dev and test profiles, "release" for release and bench.
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let debug_assertions = std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some();
    let wanted = profile == "debug" && std::env::var_os("SJ_NO_EMBED").is_none();
    let rom = if wanted { first_rom(&roms) } else { None };

    let code = match &rom {
        Some(path) => {
            // A ROM may only go into a build with debug assertions, which a release never has.
            if !debug_assertions {
                panic!(
                    "refusing to embed {} in a build without debug assertions (profile {profile}); \
                     only debug builds may contain the ROM",
                    path.display()
                );
            }
            println!("cargo:rerun-if-changed={}", path.display());
            let path = path.canonicalize().unwrap_or_else(|_| path.clone());
            format!(
                "/// The ROM embedded by this debug build.\npub const EMBEDDED_ROM: Option<&[u8]> = Some(include_bytes!({:?}));\n",
                path.display().to_string()
            )
        }
        None => {
            "/// No ROM is embedded in this build.\npub const EMBEDDED_ROM: Option<&[u8]> = None;\n"
                .to_owned()
        }
    };
    fs::write(out.join("embedded_rom.rs"), code).expect("writing embedded_rom.rs");
}
