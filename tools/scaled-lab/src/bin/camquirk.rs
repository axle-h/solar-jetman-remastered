use exp::camera::*;
fn main() {
    let world = exp::world();
    let mut names: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".replay").to_string()).collect();
    names.sort();
    let mut hist = std::collections::BTreeMap::new();
    let (mut steps, mut big) = (0u64, 0u64);
    for n in names {
        if n == "level26" { continue; }
        let text = std::fs::read_to_string(format!("{}/../../tools/mesen/replays/{n}.replay", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
        if r.demo.is_some() { continue; }
        let run = run_replay(&world, &n, &r);
        for p in run.passes.iter().filter(|p| p.a) {
            steps += 1;
            for d in [p.delta.0, p.delta.1] {
                if d.unsigned_abs() >= 9 { big += 1; *hist.entry(d).or_insert(0u32) += 1; }
            }
        }
    }
    println!("steps {steps}, axis-steps with |d|>=9: {big}");
    println!("{:?}", hist);
}
