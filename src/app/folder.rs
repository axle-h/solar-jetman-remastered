//! The player's folder: `settings.ini`, `saves/`, `screenshots/`, `data/` (the imported asset
//! pack, with `manifest.toml`) and `rom.nes`, a copy of the ROM the pack came from, so that a
//! newer importer can run again without asking.

use super::settings::Settings;
use solar_jetman::rom::{IMPORTER_VERSION, MANIFEST_PATH, Manifest, PRG_SHA1};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub type Files = BTreeMap<String, Vec<u8>>;

/// What `data/` holds.
#[derive(Debug)]
pub enum PackState {
    Missing,
    /// Imported by another importer version or from another ROM.
    Stale,
    Current(Files),
}

pub struct Folder {
    root: PathBuf,
}

fn io(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

impl Folder {
    /// The platform's per-user data folder, or `$SJ_FOLDER` when set.
    pub fn locate() -> Result<Folder, String> {
        #[cfg(target_os = "emscripten")]
        return Ok(Folder::at(super::web::FOLDER.into()));
        #[cfg(target_os = "android")]
        return android_folder().map(Folder::at);
        #[cfg(not(any(target_os = "emscripten", target_os = "android")))]
        if let Some(path) = std::env::var_os("SJ_FOLDER") {
            return Ok(Folder::at(path.into()));
        }
        #[cfg(not(any(target_os = "emscripten", target_os = "android")))]
        {
            let dirs = directories::ProjectDirs::from("", "", "solar-jetman-remastered")
                .ok_or("cannot find the player's folder (no home directory)")?;
            Ok(Folder::at(dirs.data_dir().to_owned()))
        }
    }

    pub fn at(root: PathBuf) -> Folder {
        Folder { root }
    }

    #[cfg(test)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn screenshots(&self) -> PathBuf {
        self.root.join("screenshots")
    }

    fn settings_path(&self) -> PathBuf {
        self.root.join("settings.ini")
    }

    fn data(&self) -> PathBuf {
        self.root.join("data")
    }

    fn rom_path(&self) -> PathBuf {
        self.root.join("rom.nes")
    }

    fn save_path(&self, slot: usize) -> PathBuf {
        self.root
            .join("saves")
            .join(format!("slot-{}.sav", slot + 1))
    }

    /// Writes a save state (compressed) to a slot, through a temporary file so a failed write
    /// leaves the old state.
    pub fn write_save(&self, slot: usize, state: &[u8]) -> Result<(), String> {
        let path = self.save_path(slot);
        let temporary = path.with_extension("tmp");
        let packed = miniz_oxide::deflate::compress_to_vec(state, 6);
        fs::write(&temporary, packed).map_err(|e| io(&temporary, e))?;
        fs::rename(&temporary, &path).map_err(|e| io(&path, e))?;
        super::web::sync_folder();
        Ok(())
    }

    /// The state in a slot, `None` when the slot is empty.
    pub fn read_save(&self, slot: usize) -> Result<Option<Vec<u8>>, String> {
        let path = self.save_path(slot);
        let packed = match fs::read(&path) {
            Ok(packed) => packed,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(&path, e)),
        };
        miniz_oxide::inflate::decompress_to_vec(&packed)
            .map(Some)
            .map_err(|e| format!("{}: damaged ({e:?})", path.display()))
    }

    /// Which slots hold a state.
    pub fn filled_slots(&self) -> [bool; super::settings::SLOTS] {
        std::array::from_fn(|slot| self.save_path(slot).is_file())
    }

    pub fn create(&self) -> Result<(), String> {
        for dir in [
            self.root.clone(),
            self.root.join("saves"),
            self.screenshots(),
            self.data(),
        ] {
            fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
        }
        Ok(())
    }

    pub fn load_settings(&self) -> Settings {
        fs::read_to_string(self.settings_path())
            .map(|text| Settings::parse(&text))
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        let path = self.settings_path();
        fs::write(&path, settings.to_ini()).map_err(|e| io(&path, e))?;
        super::web::sync_folder();
        Ok(())
    }

    /// Replaces `data/` with the pack's files, the manifest last, so an interrupted write
    /// reads back as missing. Also keeps a copy of the ROM.
    pub fn write_pack(&self, files: &Files, rom_file: &[u8]) -> Result<(), String> {
        let data = self.data();
        if data.exists() {
            fs::remove_dir_all(&data).map_err(|e| io(&data, e))?;
        }
        let ordered = files
            .iter()
            .filter(|(path, _)| path.as_str() != MANIFEST_PATH)
            .chain(files.get_key_value(MANIFEST_PATH));
        for (path, bytes) in ordered {
            let target = data.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
            }
            fs::write(&target, bytes).map_err(|e| io(&target, e))?;
        }
        let rom = self.rom_path();
        fs::write(&rom, rom_file).map_err(|e| io(&rom, e))?;
        super::web::sync_folder();
        Ok(())
    }

    pub fn read_pack(&self) -> PackState {
        let data = self.data();
        let Ok(text) = fs::read_to_string(data.join(MANIFEST_PATH)) else {
            return PackState::Missing;
        };
        match toml::from_str::<Manifest>(&text) {
            Ok(m) if m.importer_version == IMPORTER_VERSION && m.rom_sha1 == PRG_SHA1 => {}
            _ => return PackState::Stale,
        }
        let mut files = Files::new();
        match read_tree(&data, "", &mut files) {
            Ok(()) => PackState::Current(files),
            Err(_) => PackState::Stale,
        }
    }

    /// The ROM file kept from the last import, if any.
    pub fn stored_rom(&self) -> Option<Vec<u8>> {
        fs::read(self.rom_path()).ok()
    }
}

/// The app's folder on shared storage, `Android/data/<package>/files`, where a file manager
/// or a USB cable reaches the saves and screenshots; the app's internal storage when shared
/// storage cannot be written. Android deletes both with the app.
#[cfg(target_os = "android")]
fn android_folder() -> Result<PathBuf, String> {
    use sdl3::sys::system::{
        SDL_ANDROID_EXTERNAL_STORAGE_WRITE, SDL_GetAndroidExternalStoragePath,
        SDL_GetAndroidExternalStorageState, SDL_GetAndroidInternalStoragePath,
    };
    use std::ffi::CStr;
    // SAFETY: SDL's Java side is up before `SDL_main` runs, and the paths it returns are
    // NUL-terminated strings that SDL keeps.
    unsafe {
        let mut path = std::ptr::null();
        if SDL_GetAndroidExternalStorageState() & SDL_ANDROID_EXTERNAL_STORAGE_WRITE != 0 {
            path = SDL_GetAndroidExternalStoragePath();
        }
        if path.is_null() {
            path = SDL_GetAndroidInternalStoragePath();
        }
        if path.is_null() {
            return Err(format!("no folder for the game: {}", sdl3::get_error()));
        }
        Ok(PathBuf::from(
            CStr::from_ptr(path).to_string_lossy().into_owned(),
        ))
    }
}

fn read_tree(dir: &Path, prefix: &str, files: &mut Files) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let entry = entry.map_err(|e| io(dir, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let key = format!("{prefix}{name}");
        if path.is_dir() {
            read_tree(&path, &format!("{key}/"), files)?;
        } else {
            files.insert(key, fs::read(&path).map_err(|e| io(&path, e))?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> Folder {
        let root =
            std::env::temp_dir().join(format!("sj-folder-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        Folder::at(root)
    }

    fn manifest(version: u32, sha1: &str) -> Vec<u8> {
        format!("importer_version = {version}\nrom_sha1 = \"{sha1}\"\n").into_bytes()
    }

    #[test]
    fn save_slots_round_trip_and_start_empty() {
        let folder = temp("saves");
        folder.create().unwrap();
        assert_eq!(
            folder.filled_slots(),
            [false; super::super::settings::SLOTS]
        );
        assert_eq!(folder.read_save(1).unwrap(), None);
        let state: Vec<u8> = (0..5000u32).map(|n| (n % 7) as u8).collect();
        folder.write_save(1, &state).unwrap();
        assert_eq!(folder.read_save(1).unwrap(), Some(state));
        assert_eq!(folder.filled_slots(), [false, true, false]);
        std::fs::write(folder.root().join("saves/slot-1.sav"), b"not deflate").unwrap();
        assert!(folder.read_save(0).is_err());
        let _ = fs::remove_dir_all(folder.root());
    }

    #[test]
    fn pack_round_trip_and_staleness() {
        let folder = temp("pack");
        folder.create().unwrap();
        assert!(matches!(folder.read_pack(), PackState::Missing));

        let mut files = Files::new();
        files.insert(MANIFEST_PATH.into(), manifest(IMPORTER_VERSION, PRG_SHA1));
        files.insert("tiles/a.png".into(), vec![1, 2, 3]);
        folder.write_pack(&files, b"rom").unwrap();
        match folder.read_pack() {
            PackState::Current(read) => assert_eq!(read, files),
            other => panic!("{other:?}"),
        }
        assert_eq!(folder.stored_rom().unwrap(), b"rom");

        files.insert(
            MANIFEST_PATH.into(),
            manifest(IMPORTER_VERSION + 1, PRG_SHA1),
        );
        folder.write_pack(&files, b"rom").unwrap();
        assert!(matches!(folder.read_pack(), PackState::Stale));
        files.insert(MANIFEST_PATH.into(), manifest(IMPORTER_VERSION, "00"));
        folder.write_pack(&files, b"rom").unwrap();
        assert!(matches!(folder.read_pack(), PackState::Stale));
        let _ = fs::remove_dir_all(folder.root());
    }

    #[test]
    fn settings_persist() {
        let folder = temp("settings");
        folder.create().unwrap();
        let mut settings = folder.load_settings();
        assert_eq!(settings, Settings::default());
        settings.volume = 3;
        folder.save_settings(&settings).unwrap();
        assert_eq!(folder.load_settings(), settings);
        let _ = fs::remove_dir_all(folder.root());
    }

    #[test]
    fn a_switch_in_the_menu_reaches_settings_ini() {
        let folder = temp("menu-switch");
        folder.create().unwrap();
        let mut settings = folder.load_settings();
        assert!(settings.remastered);
        let mut menu = super::super::menu::Menu::default();
        menu.toggle();
        // The first row is the physics.
        let out = menu.handle(super::super::menu::Nav::Right, &mut settings);
        assert!(out.settings_changed);
        folder.save_settings(&settings).unwrap();
        let text = fs::read_to_string(folder.root().join("settings.ini")).unwrap();
        assert!(text.contains("physics = original"), "{text}");
        assert!(!folder.load_settings().remastered);
        let _ = fs::remove_dir_all(folder.root());
    }
}
