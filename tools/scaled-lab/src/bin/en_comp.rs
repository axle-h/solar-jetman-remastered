use exp::enemies::ctl::*;
use solar_jetman::game::gameplay::objects::Object;
use solar_jetman::game::gameplay::tables::SIN;
fn max_speed(m: u8) -> u16 { u16::from(m & 7) << 8 | u16::from(m & 0xF8) }
fn same(a: &Object, b: &Object) -> bool { (a.x,a.x_fraction,a.y,a.y_fraction,a.speed_x,a.speed_y,a.flags_x&1,a.flags_y&1)==(b.x,b.x_fraction,b.y,b.y_fraction,b.speed_x,b.speed_y,b.flags_x&1,b.flags_y&1) }
fn orbit_target(centre: (u16, u16), f: u32) -> (u16, u16) {
    let s = f as u8 as i8;
    let radius = |v: u8| -> i32 { let sine = if v >= 128 { 0 } else { i32::from(SIN[usize::from(v)]) }; (0x48 * sine) >> 8 };
    let ox = radius(s.unsigned_abs()) * if s < 0 { -1 } else { 1 };
    let b = s.unsigned_abs().wrapping_add(0x40) as i8;
    let oy = radius(b.unsigned_abs()) * if b < 0 { -1 } else { 1 };
    (centre.0.wrapping_add_signed(ox as i16), centre.1.wrapping_add_signed(oy as i16))
}
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { v.sort_by(|a,b| a.partial_cmp(b).unwrap()); v[((v.len()-1) as f64 * p) as usize] }
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    const W: usize = 60;
    let mut eps: Vec<Ep> = vec![]; let (mut ok, mut bad) = ([0u32;2], [0u32;2]);
    for r in &runs { for s in 3..31 {
        let n = r.frames.len(); let mut i = 0;
        while i + W < n {
            let mut good = true; let mut tg = vec![];
            for k in 0..W {
                let a = &r.frames[i+k].slots[s]; let b = &r.frames[i+k+1].slots[s];
                if a.state != 0xBA || b.state != 0xBA || a.work6 > 3 || a.work5 == 0 || b.work5 == 0 { good = false; break; }
                let sat = usize::from(a.work5);
                // the satellite position as the sweep saw it: before or after its own move
                let mut fitted = false;
                for sf in 0..2 { let satpos = &r.frames[i+k+sf].slots[sat]; let tgt = orbit_target((satpos.x, satpos.y), r.frames[i+k].counter);
                    let mut o = *a; o.home_x(tgt.0, 0xF0, max_speed(0x83)); o.home_y(tgt.1, 0xF0, max_speed(0x83));
                    if same(&o, b) { ok[sf] += 1; fitted = true; tg.push((tgt.0, tgt.1)); break; } else { bad[sf] += 1; }
                }
                if !fitted { good = false; break; }
            }
            if good {
                // target at grid times 0..W: target used for step k (index k-1) sits at time k
                let mut pod = vec![(0.0, 0.0)]; for t in &tg { pod.push((f64::from(t.0), f64::from(t.1))); }
                pod[0] = pod[1];
                let truth = (0..=W).map(|k| r.frames[i+k].slots[s]).collect();
                let par = Par { a: 0xF0, max: i64::from(max_speed(0x83)) };
                eps.push(Ep { obj0: r.frames[i].slots[s], pod, truth, px: vec![par; W], py: vec![par; W] });
                i += W;
            } else { i += 1; }
        }
    }}
    println!("fit sat-pos-before ok {} bad {} | after ok {} bad {} ; {} episodes", ok[0], bad[0], ok[1], bad[1], eps.len());
    if eps.is_empty() { return; }
    dbg(&eps);
    for &rate in &[30i64, 60, 100, 144, 200] {
        let mut line = format!("R={rate:3}");
        for var in [Var::Drop, Var::GridPred, Var::GridOracle] {
            let mut errs = vec![]; let mut sq = 0.0;
            for ep in &eps { for ph in [1.0, 0.5] { if rate == 30 && ph != 1.0 { continue }
                let o = run_ep(ep, rate, ph, var);
                for k in 1..=W { let t = &ep.truth[k]; let tx = f64::from(t.x) + f64::from(t.x_fraction)/256.0; let ty = f64::from(t.y) + f64::from(t.y_fraction)/256.0;
                    let e = ((o[k].0 - tx).powi(2) + (o[k].1 - ty).powi(2)).sqrt(); errs.push(e); sq += e*e; } } }
            let mx = errs.iter().cloned().fold(0.0, f64::max);
            line += &format!(" | {:?}: rms {:.2} p50 {:.2} p99 {:.2} max {:.2}", var, (sq/errs.len() as f64).sqrt(), pct(&mut errs, 0.5), pct(&mut errs, 0.99), mx);
        }
        println!("{line}");
    }
}
#[allow(dead_code)]
fn dbg(eps: &[Ep]) {
    for (n, ep) in eps.iter().enumerate() {
        let o = run_ep(ep, 30, 1.0, Var::Drop);
        let mut first = None;
        for k in 1..=60 { let t = &ep.truth[k]; let tx = f64::from(t.x) + f64::from(t.x_fraction)/256.0; let ty = f64::from(t.y) + f64::from(t.y_fraction)/256.0; if (o[k].0 - tx).abs() > 0.01 || (o[k].1 - ty).abs() > 0.01 { first = Some((k, o[k].0, tx, o[k].1, ty, ep.pod[k])); break; } }
        println!("ep {n}: {:?}", first);
    }
}
