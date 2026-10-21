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
    for &r in &[60.0, 144.0] {
    for &halves in &[false, true] {
    for &lead in &[0.0, 0.25, 0.5] {
        for &bias in &[-0.5, 0.0, 0.5] {
            let c = Cand { halves, whole: true, cap: true, bias: (bias * 65536.0) as i64, lead, qpod: true };
            let mut e = Errs::default();
            for &phi in &[0.0, 0.37, 0.71] { for s in &segs { e.merge(&evaluate(s, &run_cand(s, r, phi, c))); } }
            println!("R{r} halves {halves} lead {lead} bias {bias:+.1} | x rms {:.3} mean {:+.3} max {:.1} | y rms {:.3} mean {:+.3} max {:.1}| disp interp %>=1: {:.1} {:.1}", e.cam_x.rms(), e.cam_x.mean(), e.cam_x.max, e.cam_y.rms(), e.cam_y.mean(), e.cam_y.max, 100.0*e.di_x.ge1 as f64/e.di_x.n as f64, 100.0*e.di_y.ge1 as f64/e.di_y.n as f64);
        }
    }}}
}
