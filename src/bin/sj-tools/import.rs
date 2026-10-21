//! `sj-tools import`: imports the ROM's graphics and writes the asset pack to a folder, so the
//! sheets and pictures can be looked at.

use std::fs;
use std::path::{Path, PathBuf};

/// Runs the import; `args` are the arguments after `import`.
pub fn run(repo: &Path, args: &[String]) -> Result<(), String> {
    let repo = repo.canonicalize().unwrap_or_else(|_| repo.to_owned());
    let repo = repo.as_path();
    let mut out = repo.join("vendor/import");
    let mut rom = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                let dir = args.next().ok_or("--out needs a folder")?;
                out = PathBuf::from(dir);
            }
            _ if rom.is_none() && !arg.starts_with('-') => rom = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    let (rom, checked) = solar_jetman::tools::load_rom(repo, rom)?;
    let pack = solar_jetman::rom::import(&checked).map_err(|error| error.to_string())?;
    for (path, bytes) in &pack.files {
        let target = out.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::write(&target, bytes).map_err(|error| format!("{}: {error}", target.display()))?;
    }
    let tiles: u32 = pack.manifest.assets.iter().filter_map(|a| a.tiles).sum();
    println!(
        "Imported {} assets ({tiles} tiles) from {} into {}",
        pack.manifest.assets.len(),
        rom.display(),
        out.display()
    );
    Ok(())
}
