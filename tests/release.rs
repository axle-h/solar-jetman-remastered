//! The Europe release against the USA release: both ROMs (`vendor/rom/*.nes` or `vendor/*.nes`,
//! found by checksum) import into the same asset pack but for the differences listed here,
//! which checks every location of the catalogue and the extraction in the Europe release
//! (`src/rom/release.rs`). Each test skips (passes with a message) unless both ROMs are there.
//! Nothing read from the ROMs is written anywhere.

use solar_jetman::rom::{
    AssetPack, MANIFEST_PATH, Release, Rom, import, note_periods, sound_data, world,
};
use solar_jetman::tools::{repo_root, vendor_rom};
use std::sync::OnceLock;

fn rom(release: Release) -> Option<Rom> {
    let path = vendor_rom(&repo_root(), release)?;
    Some(Rom::from_nes_file(&std::fs::read(path).ok()?).expect("a checked ROM"))
}

/// The USA and the Europe release's packs, imported once, or `None` (after saying so).
fn packs() -> Option<&'static (AssetPack, AssetPack)> {
    static PACKS: OnceLock<Option<(AssetPack, AssetPack)>> = OnceLock::new();
    PACKS
        .get_or_init(|| {
            let (Some(usa), Some(europe)) = (rom(Release::Usa), rom(Release::Europe)) else {
                eprintln!("skipped: needs both the USA and the Europe ROM in vendor/");
                return None;
            };
            let import = |rom: &Rom| import(rom).unwrap_or_else(|error| panic!("{error}"));
            Some((import(&usa), import(&europe)))
        })
        .as_ref()
}

/// Files that differ between the releases, checked by the tests below.
fn differs(path: &str) -> bool {
    path == MANIFEST_PATH
        || path == "game/objects.ron"
        || path == "game/blocks.ron"
        || path.starts_with("sound/song-")
}

#[test]
fn every_other_file_is_the_same() {
    let Some((usa, europe)) = packs() else { return };
    assert_eq!(
        usa.files.keys().collect::<Vec<_>>(),
        europe.files.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in usa.files.iter().filter(|(path, _)| !differs(path)) {
        assert!(*bytes == europe.files[path], "{path} differs");
    }
}

/// The manifests differ in the checksum and in where each source was read.
#[test]
fn manifests_differ_in_checksum_and_offsets() {
    let Some((usa, europe)) = packs() else { return };
    let (mut a, mut b) = (usa.manifest.clone(), europe.manifest.clone());
    assert_eq!(a.rom_sha1, Release::Usa.prg_sha1());
    assert_eq!(b.rom_sha1, Release::Europe.prg_sha1());
    for manifest in [&mut a, &mut b] {
        manifest.rom_sha1.clear();
        for asset in &mut manifest.assets {
            for source in &mut asset.sources {
                source.rom_offset.clear();
            }
        }
    }
    assert_eq!(a, b);
}

/// The songs are the same once the Europe tempos are brought to 60 Hz, but for song 14's.
#[test]
fn songs_differ_only_in_one_tempo() {
    let Some((usa, europe)) = packs() else { return };
    let read =
        |pack: &AssetPack| sound_data(&pack.files, note_periods(&pack.files).unwrap()).unwrap();
    let (a, b) = (read(usa), read(europe));
    assert_eq!(a.effects, b.effects);
    for (number, (a, b)) in a.songs.iter().zip(&b.songs).enumerate() {
        assert_eq!(a.tracks, b.tracks, "song {number}");
        let expected = if number == 14 { 75 } else { a.tempo };
        assert_eq!(b.tempo, expected, "song {number}");
    }
}

/// The level data is the same but for one object placed 16 pixels further right, and blocks
/// 122-147 of block set 1 and 132-147 of block set 2, which levels 10 and 26 place only in
/// the map rows below where the camera goes.
#[test]
fn level_data_differs_in_one_placement_and_unseen_blocks() {
    let Some((usa, europe)) = packs() else { return };
    let (a, b) = (world(&usa.files).unwrap(), world(&europe.files).unwrap());
    assert_eq!(a.levels, b.levels);
    assert_eq!(a.maps, b.maps);
    assert_eq!(a.collision, b.collision);
    assert_eq!(a.blocks.chunks, b.blocks.chunks);
    let differing = |set: usize| -> Vec<usize> {
        let (x, y) = (&a.blocks.block_sets[set].0, &b.blocks.block_sets[set].0);
        (0..x.len() / 4)
            .filter(|i| x[4 * i..4 * i + 4] != y[4 * i..4 * i + 4])
            .collect()
    };
    assert_eq!(differing(0), Vec::<usize>::new());
    assert_eq!(differing(1), (122..148).collect::<Vec<_>>());
    assert_eq!(differing(2), (132..148).collect::<Vec<_>>());

    let mut moved = Vec::new();
    for (level, (x, y)) in a
        .objects
        .placements
        .iter()
        .zip(&b.objects.placements)
        .enumerate()
    {
        assert_eq!(x.len(), y.len(), "level {level}");
        for (index, (p, q)) in x.iter().zip(y).enumerate() {
            if p != q {
                moved.push((level, index, (p.x, p.y, p.kind), (q.x, q.y, q.kind)));
            }
        }
    }
    // Level 1, object 6 (type 4).
    assert_eq!(moved, [(1, 6, (2645, 1924, 4), (2661, 1924, 4))]);
    let mut objects = b.objects.clone();
    objects.placements = a.objects.placements.clone();
    assert_eq!(a.objects, objects);
}
