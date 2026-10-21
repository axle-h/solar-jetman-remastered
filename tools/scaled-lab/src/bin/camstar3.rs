use exp::camera::*;
fn wrap(a: u8, b: u8) -> i32 { (a.wrapping_sub(b) as i8) as i32 }
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let world = exp::world();
    let name = &args[1];
    let (u0, u1): (usize, usize) = (args[2].parse().unwrap(), args[3].parse().unwrap());
    let segname = args.get(4).cloned();
    let star: usize = args.get(5).map(|x| x.parse().unwrap()).unwrap_or(0);
    let mut segs = vec![];
    if name.starts_with("synth") { for r in synthetic(&world) { if r.name == *name { segs = to_segs(&r); } } }
    else { let text = std::fs::read_to_string(format!("{}/../../tools/mesen/replays/{name}.replay", env!("CARGO_MANIFEST_DIR"))).unwrap(); let r = solar_jetman::tools::replay::parse_replay(&text).unwrap(); segs = to_segs(&run_replay(&world, name, &r)); }
    let _ = segname;
    for s in &segs {
        if u1 >= s.p.len() * 2 || s.sky.iter().filter(|&&x| x).count() <= 100 { continue; }
        let (o, _) = orig_stars(s);
        let c = Cand { halves: true, whole: true, cap: true, bias: 0, lead: 0.0, qpod: true };
        let t = run_cand(s, 60.0, 0.0, c);
        let sc = scaled_stars(s, &t, false);
        let mut j = 0;
        let mut bad = false;
        for u in 4..o.len() { while j + 1 < t.len() && t[j + 1].u <= u as f64 - 1.0 { j += 1; } if j+1>=t.len() {break;} let (a,b)=(&o[u].stars[star],&sc[j].stars[star]); if !a.dead && !b.dead && (wrap(a.x,b.x).abs()>8 || wrap(a.y,b.y).abs()>8) { bad = true; } }
        println!("seg len {} bad {}", s.p.len(), bad);
        if !bad { continue; }
        let mut j = 0;
        for u in u0..u1.min(o.len()) {
            while j + 1 < t.len() && t[j + 1].u <= u as f64 - 1.0 { j += 1; }
            let (a, b) = (&o[u].stars[star], &sc[j].stars[star]);
            if u % 2 == 0 { println!("u{u} mode o{} s{} sky {} acall {} star o({},{}){} s({},{}){}  cam {:?}", o[u].mode, sc[j].mode, s.sky[u/2], s.acall[u/2], a.x, a.y, if a.dead {"D"} else {""}, b.x, b.y, if b.dead {"D"} else {""}, s.cam_b[u/2]); }
        }
        break;
    }
}
