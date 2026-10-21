use exp::camera::*;
fn wrap(a: u8, b: u8) -> i32 { (a.wrapping_sub(b) as i8) as i32 }
fn main() {
    let world = exp::world();
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays/pod-nippon.replay")).unwrap();
    let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
    let segs = to_segs(&run_replay(&world, "x", &r));
    let s = &segs[0];
    let (o, _) = orig_stars(s);
    let c = Cand { halves: true, whole: true, cap: true, bias: 0, lead: 0.0, qpod: true };
    let t = run_cand(s, 60.0, 0.0, c);
    let sc = scaled_stars(s, &t, false);
    let mut j = 0;
    for u in 120..300 {
        while j + 1 < t.len() && t[j + 1].u <= u as f64 - 1.0 { j += 1; }
        let (a, b) = (&o[u].stars[0], &sc[j].stars[0]);
        let k = u / 2;
        let camo = if u % 2 == 0 { s.cam_b[k] } else { s.cam_a[(u + 1) / 2] };
        if !a.dead && !b.dead && u % 4 == 0 { println!("u{u} o({},{}) s({},{}) d({},{}) camo {:?} cams ({:.1},{:.1}) sky {} a {}", a.x, a.y, b.x, b.y, wrap(a.x,b.x), wrap(a.y,b.y), camo, t[j].cam.0 as f64/65536.0, t[j].cam.1 as f64/65536.0, s.sky[k], s.acall[k]); }
    }
}
