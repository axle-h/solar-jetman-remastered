use exp::camera::*;
fn wrap(a: u8, b: u8) -> f64 { (a.wrapping_sub(b) as i8) as f64 }
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
    segs.retain(|s| s.sky.iter().filter(|&&x| x).count() > 100);
    println!("{} segs with sky", segs.len());
    let mut tot_spawns = [0u32; 8];
    let mut alive_steps = [0u64; 8];
    for s in &segs { let (o, sp) = orig_stars(s); for i in 0..8 { tot_spawns[i] += sp[i]; } for st in &o { for i in 0..8 { if !st.stars[i].dead { alive_steps[i] += 1; } } } }
    println!("original: spawns per star {:?}; frames alive {:?}", tot_spawns, alive_steps);
    let c = Cand { halves: true, whole: true, cap: true, bias: 0, lead: 0.0, qpod: true };
    println!("rate | star pos err x,y rms/max/%>=1px | alive-mismatch % | n");
    for &r in &RATES {
        let (mut sx, mut sy) = (St::default(), St::default());
        let (mut mism, mut tot, mut div, mut pairs) = (0u64, 0u64, 0u64, 0u64);
        for &phi in &[0.0, 0.37, 0.71] {
            for s in &segs {
                let (o, _) = orig_stars(s);
                let t = run_cand(s, r, phi, c);
                let sc = scaled_stars(s, &t, false);
                let mut j = 0;
                for u in 4..o.len() {
                    while j + 1 < t.len() && t[j + 1].u <= u as f64 - 1.0 { j += 1; }
                    if j + 1 >= t.len() { break; }
                    let sj = &sc[j];
                    for i in 0..8 {
                        let (a, b) = (&o[u].stars[i], &sj.stars[i]);
                        if a.dead != b.dead { mism += 1; }
                        if !a.dead || !b.dead { tot += 1; }
                        if !a.dead && !b.dead && o[u].mode == 1 && sj.mode == 1 {
                            pairs += 1;
                            let (ex, ey) = (wrap(a.x, b.x), wrap(a.y, b.y));
                            if ex.abs() > 16.0 || ey.abs() > 16.0 { div += 1; } else { sx.add(ex); sy.add(ey); }
                        }
                    }
                }
            }
        }
        println!("{r:5} | {:.2}/{:.0}/{:.1}%  {:.2}/{:.0}/{:.1}% | diverged {:.2}% | alive-mismatch {:.2}% | n {}", sx.rms(), sx.max, 100.0*sx.ge1 as f64/sx.n as f64, sy.rms(), sy.max, 100.0*sy.ge1 as f64/sy.n as f64, 100.0*div as f64/pairs as f64, 100.0*mism as f64/tot as f64, sx.n);
    }
}
