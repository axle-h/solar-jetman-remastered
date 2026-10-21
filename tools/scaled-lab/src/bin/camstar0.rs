use exp::camera::*;
fn main() {
    let world = exp::world();
    let mut names: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".replay").to_string()).collect();
    names.sort();
    for n in names {
        let text = std::fs::read_to_string(format!("{}/../../tools/mesen/replays/{n}.replay", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
        if r.demo.is_some() { continue; }
        let run = run_replay(&world, &n, &r);
        let (mut sky, mut acall, mut tot, mut moving_sky, mut moving_acall) = (0, 0, 0, 0, 0);
        let mut sizes = std::collections::BTreeMap::new();
        for w in run.passes.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if !a.a || b.a { continue; }
            tot += 1;
            let act = !a.warp && (a.list_size == 0x10 || (a.list_size < 0x20 && !a.towing));
            let mv = b.cam != a.cam;
            if a.in_sky { sky += 1; *sizes.entry(a.list_size).or_insert(0) += 1; if mv { moving_sky += 1; if act { moving_acall += 1; } } }
            if a.in_sky && act { acall += 1; }
        }
        if sky > 0 { println!("{n:22} steps {tot:6} in_sky {sky:6} A-call active in sky {acall:6} | camera moving in sky {moving_sky:5}, of which A-call {moving_acall:5} sizes {:?}", sizes.iter().take(6).collect::<Vec<_>>()); }
    }
}
