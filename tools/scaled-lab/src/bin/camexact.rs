use exp::camera::*;
fn main() {
    let world = exp::world();
    let mut names: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".replay").to_string()).collect();
    names.sort();
    let mut segs = vec![];
    for n in names {
        if n == "level26" { continue; }
        let text = std::fs::read_to_string(format!("{}/../../tools/mesen/replays/{n}.replay", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
        if r.demo.is_some() { continue; }
        segs.extend(to_segs(&run_replay(&world, &n, &r)));
    }
    segs.extend(synthetic(&world).iter().flat_map(to_segs));
    let c = Cand { halves: true, whole: true, cap: true, bias: 0, lead: 0.0, qpod: true };
    let (mut n, mut bad) = (0, 0);
    let mut first = vec![];
    for s in &segs {
        let t = run_cand(s, 30.0, 0.0, c);
        for (j, tk) in t.iter().enumerate().skip(1) {
            if j >= s.cam_b.len() { break; }
            n += 1;
            let o = s.cam_b[j];
            if (tk.cam.0 >> 16) as i32 != o.0 || (tk.cam.1 >> 16) as i32 != o.1 { bad += 1; if first.len() < 8 { first.push((s.name.clone(), j, (tk.cam.0 >> 16, tk.cam.1>>16), o, s.limit, s.p[j].0>>8, s.p[j].1>>8)); } }
        }
    }
    println!("R=30 exactness: {bad} of {n} steps differ");
    for f in first { println!("{:?}", f); }
}
