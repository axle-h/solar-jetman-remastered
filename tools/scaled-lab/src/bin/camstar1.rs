use exp::camera::*;
fn wrap(a: u8, b: u8) -> i32 { (a.wrapping_sub(b) as i8) as i32 }
fn main() {
    let world = exp::world();
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays/free-flight.replay")).unwrap();
    let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
    let segs = to_segs(&run_replay(&world, "ff", &r));
    let s = &segs[0];
    let (o, sp) = orig_stars(s);
    println!("spawns {:?}", sp);
    let c = Cand { halves: true, whole: true, cap: true, bias: 0, lead: 0.0, qpod: true };
    let t = run_cand(s, 60.0, 0.0, c);
    let sc = scaled_stars(s, &t, false);
    let mut j = 0;
    for u in 4..o.len() {
        while j + 1 < t.len() && t[j + 1].u <= u as f64 - 1.0 { j += 1; }
        let mut line = String::new();
        for i in 0..8 {
            let (a, b) = (&o[u].stars[i], &sc[j].stars[i]);
            if !a.dead || !b.dead { line += &format!(" s{i}: o({},{}){} s({},{}){} d{}", a.x, a.y, if a.dead {"D"} else {""}, b.x, b.y, if b.dead {"D"} else {""}, wrap(a.x,b.x)); }
        }
        if u % 20 == 0 || u < 140 && u % 4 == 0 { println!("u{u} mode o{} s{} sky {} cam {:?}{}", o[u].mode, sc[j].mode, s.sky[u/2], s.cam_b[u/2], line); }
        if u > 300 { break; }
    }
}
