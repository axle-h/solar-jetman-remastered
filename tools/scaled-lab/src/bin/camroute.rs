use exp::camera::*;
fn main() {
    let world = exp::world();
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays/level26.replay")).unwrap();
    let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
    let run = run_replay(&world, "level26", &r);
    let ps = &run.passes;
    println!("passes {} states {:?}", ps.len(), { let mut m = std::collections::BTreeMap::new(); for p in ps { *m.entry(p.state).or_insert(0) += 1; } m });
    // per-frame camera moves histogram
    let mut hist = std::collections::BTreeMap::new();
    for w in ps.windows(2) { *hist.entry((w[1].cam.0 - w[0].cam.0, w[1].cam.1 - w[0].cam.1)).or_insert(0u32) += 1; }
    println!("camera move per frame (dx,dy): count {:?}", hist);
    // clamp analysis, A pass k pod vs camera at end of previous step (B pass before) and vs current
    let (mut n, mut at_x_prev, mut at_y_prev, mut at_x_cur, mut at_y_cur) = (0, 0, 0, 0, 0);
    let (mut minx, mut maxx, mut miny, mut maxy) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    let mut i = 2;
    while i + 1 < ps.len() {
        if ps[i].a && ps[i].state == 6 {
            n += 1;
            let prev = ps[i - 1].cam; // end of previous step
            let podx = ps[i].pod.0 >> 8; let pody = ps[i].pod.1 >> 8;
            let sx = podx - prev.0 as i64; let sy = pody - prev.1 as i64;
            if sx == 24 || sx == 232 { at_x_prev += 1; } if sy == 16 || sy == 176 { at_y_prev += 1; }
            let cur = ps[i + 1].cam;
            let cx = podx - cur.0 as i64; let cy = pody - cur.1 as i64;
            if cx == 24 || cx == 232 { at_x_cur += 1; } if cy == 16 || cy == 176 { at_y_cur += 1; }
            minx = minx.min(sx); maxx = maxx.max(sx); miny = miny.min(sy); maxy = maxy.max(sy);
        }
        i += 1;
    }
    // Scaled route camera: Q32 game clock (steps), frames = game_time >> 31, frac Q16 of a frame.
    let path: Vec<(i64, i64)> = ps.iter().map(|p| (p.cam.0 as i64, p.cam.1 as i64)).collect();
    for &rate in &RATES {
        let f32q = ((1u64 << 32) as f64 * 30.0 / rate).round() as u64;
        let mut gt: u64 = 0;
        let (mut maxe, mut maxd) = (0f64, 0i64);
        let mut n = 0u64;
        loop {
            n += 1;
            gt += f32q;
            let frame = (gt >> 31) as usize;   // frames elapsed (60 Hz)
            if frame + 2 >= path.len() { break; }
            let frac = ((gt & ((1u64 << 31) - 1)) >> 15) as i64; // Q16
            let cam = |a: (i64, i64), b: (i64, i64)| ((a.0 << 16) + (b.0 - a.0) * frac, (a.1 << 16) + (b.1 - a.1) * frac);
            let c = cam(path[frame], path[frame + 1]);
            // exact: true time in frames
            let tau = n as f64 * 60.0 / rate;
            let fi = tau.floor() as usize; let ff = tau - fi as f64;
            let ex = path[fi].0 as f64 + (path[fi + 1].0 - path[fi].0) as f64 * ff;
            let ey = path[fi].1 as f64 + (path[fi + 1].1 - path[fi].1) as f64 * ff;
            maxe = maxe.max((c.0 as f64 / 65536.0 - ex).abs()).max((c.1 as f64 / 65536.0 - ey).abs());
            // displayed whole pixel at the tick vs the original's integer frame camera (hold = floor)
            let disp = (c.0 >> 16, c.1 >> 16);
            let orig_hold = path[fi];
            maxd = maxd.max((disp.0 - orig_hold.0).abs()).max((disp.1 - orig_hold.1).abs());
        }
        println!("route camera R={rate}: max |Q-clock camera - exact lerp| = {maxe:.6} px over {n} ticks; floor(cam) vs original frame camera (hold) max {maxd} px");
    }
    println!("steps {n}; pod on clamp (prev-step camera): x {at_x_prev}, y {at_y_prev}; (end-of-step camera): x {at_x_cur}, y {at_y_cur}; screen range x {minx}..{maxx} y {miny}..{maxy}");
}

#[allow(dead_code)]
fn unused() {}
