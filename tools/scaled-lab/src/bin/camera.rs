use exp::camera::*;
fn real_segs(world: &solar_jetman::game::level::World) -> Vec<Seg> {
    let mut names: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".replay").to_string()).collect();
    names.sort();
    let mut segs = vec![];
    for n in names {
        if n == "level26" { continue; }
        let text = std::fs::read_to_string(format!("{}/../../tools/mesen/replays/{n}.replay", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
        if r.demo.is_some() { continue; }
        segs.extend(to_segs(&run_replay(world, &n, &r)));
    }
    segs
}
fn main() {
    let world = exp::world();
    let real = real_segs(&world);
    let synth: Vec<Seg> = synthetic(&world).iter().flat_map(to_segs).collect();
    // quirk census
    println!("real: {} segs {} steps; synth: {} segs {} steps", real.len(), real.iter().map(|s| s.p.len()).sum::<usize>(), synth.len(), synth.iter().map(|s| s.p.len()).sum::<usize>());
    let base = Cand { halves: false, whole: false, cap: true, bias: 0, lead: 0.0, qpod: false };
    let cands = [("F: whole-px pod displacement per tick, two half-moves, whole-px cam", Cand { halves: true, whole: true, qpod: true, ..base })];
    for (set, segs) in [("REAL", &real), ("SYNTH", &synth)] {
        for (name, c) in cands {
            println!("== {set} {name}");
            println!("rate | cam/pod-screen err px: x rms/max  y rms/max | displayed (hold) x,y rms/max/%>=1px | displayed (interp)");
            for &r in &RATES {
                let mut e = Errs::default();
                for &phi in &[0.0, 0.37, 0.71] { for s in segs.iter() { e.merge(&evaluate(s, &run_cand(s, r, phi, c))); } }
                let p = |s: &St| format!("{:.2}/{:.0}/{:.1}%/{:.2}%", s.rms(), s.max, 100.0 * s.ge1 as f64 / s.n as f64, 100.0 * s.ge2 as f64 / s.n as f64);
                println!("{r:5} | {:.2}/{:.1}  {:.2}/{:.1} | {} {} | {} {}", e.cam_x.rms(), e.cam_x.max, e.cam_y.rms(), e.cam_y.max, p(&e.dh_x), p(&e.dh_y), p(&e.di_x), p(&e.di_y));
            }
        }
    }
}
