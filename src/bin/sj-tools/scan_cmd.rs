//! `sj-tools scan-release`.

use solar_jetman::tools::scan::{
    Category, Hit, NOT_COMMITTABLE, Region, allowed, byte_categories, parse_rom_map, region_at,
    scan,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn report(
    name: &str,
    data: &[u8],
    prg: &[u8],
    cats: &[Category],
    regions: &[Region],
    wanted: &[Category],
) -> usize {
    let (excused, hits): (Vec<Hit>, Vec<Hit>) = scan(data, prg, cats, wanted)
        .into_iter()
        .partition(|hit| allowed(hit, data).is_some());
    for hit in &excused {
        println!(
            "  allowed {name} @ 0x{:X}: {} bytes = ROM 0x{:05X} ({})",
            hit.found_at,
            hit.len,
            hit.rom_offset,
            allowed(hit, data).unwrap_or_default(),
        );
    }
    for hit in &hits {
        let region = region_at(regions, hit.rom_offset);
        println!(
            "  HIT {name} @ 0x{:X}: {} bytes = ROM 0x{:05X} ({}: {})",
            hit.found_at,
            hit.len,
            hit.rom_offset,
            cats[hit.rom_offset].name(),
            region.map_or("no region", |r| r.label.as_str()),
        );
    }
    hits.len()
}

/// `sj-tools scan-release [--rom ROM] [BINARY]`: checks that the release binary (built if not
/// given) holds no run of 16 or more consecutive bytes of the ROM outside the commit and fill
/// regions of `docs/spec/rom-map.md`.
pub fn run(repo: &Path, args: &[String]) -> Result<(), String> {
    let (mut binary, mut rom) = (None, None);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rom" => rom = Some(PathBuf::from(args.next().ok_or("--rom needs a path")?)),
            _ if binary.is_none() && !arg.starts_with('-') => binary = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
    }
    let binary = match binary {
        Some(path) => path,
        None => {
            println!("building the release binary");
            let status = Command::new("cargo")
                .args(["build", "--release", "--bin", "solar-jetman"])
                .current_dir(repo)
                .status()
                .map_err(|error| format!("cargo: {error}"))?;
            if !status.success() {
                return Err("the release build failed".into());
            }
            let target = std::env::var_os("CARGO_TARGET_DIR")
                .map_or_else(|| repo.join("target"), PathBuf::from);
            target.join("release/solar-jetman")
        }
    };
    let (rom_path, rom) = solar_jetman::tools::load_rom(repo, rom)?;
    let prg = rom.prg();
    let map_path = repo.join("docs/spec/rom-map.md");
    let map = fs::read_to_string(&map_path).map_err(|e| format!("{}: {e}", map_path.display()))?;
    let regions = parse_rom_map(&map);
    let cats = byte_categories(&regions, prg.len());
    let count = |c: Category| cats.iter().filter(|&&x| x == c).count();
    println!(
        "ROM {}: {} regions; scanning {} ripped, {} code and {} unknown bytes",
        rom_path.display(),
        regions.len(),
        count(Category::Ripped),
        count(Category::Code),
        count(Category::Unknown),
    );

    let bytes = fs::read(&binary).map_err(|e| format!("{}: {e}", binary.display()))?;
    println!("{}: {} bytes", binary.display(), bytes.len());
    let total = report("binary", &bytes, prg, &cats, &regions, NOT_COMMITTABLE);

    if total == 0 {
        println!("scan-release: no hits");
        Ok(())
    } else {
        Err(format!("{total} hit(s)"))
    }
}
