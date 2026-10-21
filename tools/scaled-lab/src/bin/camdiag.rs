use exp::camera::*;
fn main() {
    let world = exp::world();
    let mut worst = (0.0, String::new(), 0usize, 0usize);
    let mut names: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".replay").to_string()).collect();
    names.sort();
    let c = Cand { halves: false, whole: false, cap: true, bias: 0, lead: 0.0, qpod: false };
    let mut all = vec![];
    for n in names {
        if n == "level26" { continue; }
        let text = std::fs::read_to_string(format!("{}/../../tools/mesen/replays/{n}.replay", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
        if r.demo.is_some() { continue; }
        for s in to_segs(&run_replay(&world, &n, &r)) { all.push(s); }
    }
    for (si, s) in all.iter().enumerate() {
        let t = run_cand(s, 120.0, 0.0, c);
        let e = evaluate(s, &t);
        let m = e.cam_x.max.max(e.cam_y.max);
        if m > worst.0 { worst = (m, s.name.clone(), si, 0); }
    }
    println!("worst {:?}", worst);
    let s = &all[worst.2];
    let t = run_cand(s, 120.0, 0.0, c);
    let refs = ref_frames(s);
    let mut best = (0.0, 0usize);
    for (i, tk) in t.iter().enumerate().skip(1) {
        let u = tk.u.floor() as usize; if u + 1 >= refs.len() { break; }
        let ey = (tk.cam.1 as f64 / 65536.0 - refs[u].2 .1 as f64).abs();
        if ey > best.0 { best = (ey, i); }
    }
    let i0 = best.1.saturating_sub(40);
    for tk in &t[i0..(best.1 + 40).min(t.len())] {
        let u = tk.u.floor() as usize;
        println!("u {:.2} cam {:.2},{:.2} ref cam {:?} podscr {:.1},{:.1} refpodscr {:.1},{:.1}", tk.u, tk.cam.0 as f64/65536.0, tk.cam.1 as f64/65536.0, refs[u].2, (tk.pod.0-tk.cam.0) as f64/65536.0, (tk.pod.1-tk.cam.1) as f64/65536.0, (refs[u].1.0 as f64/65536.0 - refs[u].2.0 as f64), (refs[u].1.1 as f64/65536.0 - refs[u].2.1 as f64));
    }
}
