//! Our sound driver against captures of the original (`sj-tools sound-compare` makes them in
//! `vendor/sound/captures/`). Skips when there is no ROM or no capture.

use solar_jetman::tools::sound::{compare, parse_capture, song_starts, sound_data};

#[test]
fn captures_of_the_original_replay_exactly() {
    let repo = solar_jetman::tools::repo_root();
    let Some(rom) = solar_jetman::tools::test_rom() else {
        return;
    };
    let mut logs: Vec<_> = std::fs::read_dir(repo.join("vendor/sound/captures"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    logs.retain(|path| path.extension().is_some_and(|ext| ext == "log"));
    logs.sort();
    if logs.is_empty() {
        eprintln!("skipped: no captures (run `cargo run --bin sj-tools -- sound-compare`)");
        return;
    }
    let data = sound_data(rom).unwrap();
    let starts = song_starts(rom).unwrap();
    for log in logs {
        let capture = parse_capture(&std::fs::read_to_string(&log).unwrap()).unwrap();
        let result = compare(&data, &starts, &capture);
        assert!(
            result.matching == result.updates && result.unexplained.is_empty(),
            "{}: {}/{} updates match; first differences {:?}",
            log.display(),
            result.matching,
            result.updates,
            result.mismatches.first()
        );
    }
}
