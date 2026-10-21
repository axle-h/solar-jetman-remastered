use exp::enemies::ctl::*;
use solar_jetman::game::gameplay::objects::Object;
fn max_speed(m: u8) -> u16 { u16::from(m & 7) << 8 | u16::from(m & 0xF8) }
fn same(a: &Object, b: &Object) -> bool { (a.x,a.x_fraction,a.y,a.y_fraction,a.speed_x,a.speed_y,a.flags_x&1,a.flags_y&1)==(b.x,b.x_fraction,b.y,b.y_fraction,b.speed_x,b.speed_y,b.flags_x&1,b.flags_y&1) }
fn pct(v: &mut Vec<f64>, p: f64) -> f64 { v.sort_by(|a,b| a.partial_cmp(b).unwrap()); v[((v.len()-1) as f64 * p) as usize] }
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    const W: usize = 60;
    struct Ty { name: &'static str, st: u8, ok: fn(&Object) -> bool, par: fn(&Object) -> (Par, Par) }
    let tys = [
        Ty { name: "mine", st: 0x1E, ok: |_| true, par: |o| { let c = i64::from(o.index % 32); let m = 0x100 + (c%8)*0x20 + if c>=16 {0x100} else {0}; (Par{a:8,max:m}, Par{a:8,max:m}) } },
        Ty { name: "orb", st: 0x23, ok: |o| o.index < 16, par: |_| (Par{a:2,max:0x80}, Par{a:2,max:0x80}) },
        Ty { name: "sphere", st: 0xAF, ok: |o| o.work6 >= 63 && o.frame >= 0xA1, par: |_| (Par{a:8,max:i64::from(max_speed(4))}, Par{a:0x18,max:i64::from(max_speed(4))}) },
        Ty { name: "satellite", st: 0xB9, ok: |_| true, par: |_| (Par{a:3,max:i64::from(max_speed(0xC0))}, Par{a:0x10,max:i64::from(max_speed(0xC2))}) },
    ];
    for ty in &tys {
        // episodes
        let mut eps: Vec<Ep> = Vec::new();
        for r in &runs {
            for s in 3..31 {
                let n = r.frames.len();
                let mut i = 0;
                while i + 1 < n {
                    // find a window starting at i of W steps all fitting
                    let mut good = true;
                    if i + W >= n { break; }
                    let mut px = vec![]; let mut py = vec![];
                    for k in 0..W {
                        let a = &r.frames[i+k].slots[s]; let b = &r.frames[i+k+1].slots[s];
                        if a.state != ty.st || b.state != ty.st || !(ty.ok)(a) { good = false; break; }
                        let (pa, pb) = (ty.par)(a);
                        px.push(pa); py.push(pb);
                        // fit check with the real Object logic
                        let p = &r.frames[i+k+1];
                        let mut o = *a; o.home_x(p.px, pa.a as u16, pa.max as u16); o.home_y(p.py, pb.a as u16, pb.max as u16);
                        if !same(&o, b) { good = false; break; }
                    }
                    if good {
                        let pod = (0..=W).map(|k| { let f = &r.frames[i+k]; (f64::from(f.px) + f64::from(f.pxf)/256.0, f64::from(f.py) + f64::from(f.pyf)/256.0) }).collect();
                        let truth = (0..=W).map(|k| r.frames[i+k].slots[s]).collect();
                        eps.push(Ep { obj0: r.frames[i].slots[s], pod, truth, px, py });
                        i += W;
                    } else { i += 1; }
                }
            }
        }
        println!("== {} : {} episodes of {} steps", ty.name, eps.len(), W);
        if eps.is_empty() { continue; }
        { let mut sq = 0.0; let mut n = 0.0; let mut mx: f64 = 0.0; for ep in &eps { for k in 1..=W { let (a, b) = (&ep.truth[k-1], &ep.truth[k]); let dx = f64::from(b.x) + f64::from(b.x_fraction)/256.0 - f64::from(a.x) - f64::from(a.x_fraction)/256.0; let dy = f64::from(b.y) + f64::from(b.y_fraction)/256.0 - f64::from(a.y) - f64::from(a.y_fraction)/256.0; let d = (dx*dx+dy*dy).sqrt(); sq += d*d; n += 1.0; mx = mx.max(d); } }
          println!("   one-step lag (render x(t-1 step)) error = step length: rms {:.2} max {:.2} px", (sq/n).sqrt(), mx); }
        for &rate in &[30i64, 60, 100, 144, 200, 137] {
            let mut line = format!("R={rate:3}");
            for var in [Var::Drop, Var::GridPred, Var::GridOracle] {
                let mut errs = vec![]; let mut sq = 0.0; let mut cnt = 0.0;
                for ep in &eps { for ph in [1.0, 0.25, 0.5, 0.75] {
                    if rate == 30 && ph != 1.0 { continue }
                    let o = run_ep(ep, rate, ph, var);
                    for k in 1..=W { let t = &ep.truth[k]; let tx = f64::from(t.x) + f64::from(t.x_fraction)/256.0; let ty_ = f64::from(t.y) + f64::from(t.y_fraction)/256.0;
                        let e = ((o[k].0 - tx).powi(2) + (o[k].1 - ty_).powi(2)).sqrt(); errs.push(e); sq += e*e; cnt += 1.0; }
                } }
                let mx = errs.iter().cloned().fold(0.0, f64::max);
                line += &format!(" | {:?}: rms {:.3} p50 {:.3} p99 {:.3} max {:.2}", var, (sq/cnt).sqrt(), pct(&mut errs, 0.5), pct(&mut errs, 0.99), mx);
            }
            println!("{line}");
        }
    }
}
