// Oscillation statistics on real episodes: object-pod separation, and reversals of the object's velocity direction on the tick clock.
use exp::enemies::ctl::*;
use solar_jetman::game::gameplay::objects::Object;
fn max_speed(m: u8) -> u16 { u16::from(m & 7) << 8 | u16::from(m & 0xF8) }
fn same(a: &Object, b: &Object) -> bool { (a.x,a.x_fraction,a.y,a.y_fraction,a.speed_x,a.speed_y,a.flags_x&1,a.flags_y&1)==(b.x,b.x_fraction,b.y,b.y_fraction,b.speed_x,b.speed_y,b.flags_x&1,b.flags_y&1) }
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    const W: usize = 90;
    struct Ty { name: &'static str, st: u8, ok: fn(&Object) -> bool, par: fn(&Object) -> (Par, Par) }
    let tys = [
        Ty { name: "mine", st: 0x1E, ok: |_| true, par: |o| { let c = i64::from(o.index % 32); let m = 0x100 + (c%8)*0x20 + if c>=16 {0x100} else {0}; (Par{a:8,max:m}, Par{a:8,max:m}) } },
        Ty { name: "orb", st: 0x23, ok: |o| o.index < 16, par: |_| (Par{a:2,max:0x80}, Par{a:2,max:0x80}) },
        Ty { name: "sphere", st: 0xAF, ok: |o| o.work6 >= 63 && o.frame >= 0xA1, par: |_| (Par{a:8,max:i64::from(max_speed(4))}, Par{a:0x18,max:i64::from(max_speed(4))}) },
    ];
    for ty in &tys {
        let mut eps: Vec<Ep> = vec![];
        for r in &runs { for s in 3..31 { let n = r.frames.len(); let mut i = 0;
            while i + W < n {
                let mut good = true; let (mut px, mut py) = (vec![], vec![]);
                for k in 0..W { let a = &r.frames[i+k].slots[s]; let b = &r.frames[i+k+1].slots[s];
                    if a.state != ty.st || b.state != ty.st || !(ty.ok)(a) { good = false; break; }
                    let (pa, pb) = (ty.par)(a); px.push(pa); py.push(pb);
                    let p = &r.frames[i+k+1]; let mut o = *a; o.home_x(p.px, pa.a as u16, pa.max as u16); o.home_y(p.py, pb.a as u16, pb.max as u16);
                    if !same(&o, b) { good = false; break; } }
                if good { eps.push(Ep { obj0: r.frames[i].slots[s], pod: (0..=W).map(|k| { let f = &r.frames[i+k]; (f64::from(f.px) + f64::from(f.pxf)/256.0, f64::from(f.py) + f64::from(f.pyf)/256.0) }).collect(),
                    truth: (0..=W).map(|k| r.frames[i+k].slots[s]).collect(), px, py }); i += W } else { i += 1 }
            } } }
        // truth stats at grid
        let stat = |xs: &[(f64, f64, f64)], pod: &[(f64, f64)]| -> (f64, f64) { // (sum sq separation, count of x-velocity sign reversals)
            let mut sq = 0.0; let mut rev = 0.0; let mut lastv: f64 = 0.0;
            for i in 1..xs.len() { let t = xs[i].0; let k = (t.floor() as usize).min(pod.len() - 2); let u = t - k as f64; let pxx = pod[k].0 + (pod[k+1].0 - pod[k].0) * u; let pyy = pod[k].1 + (pod[k+1].1 - pod[k].1) * u;
                sq += (xs[i].1 - pxx).powi(2) + (xs[i].2 - pyy).powi(2);
                let v: f64 = xs[i].1 - xs[i-1].1; if v != 0.0 && lastv != 0.0 && v.signum() != lastv.signum() { rev += 1.0; } if v != 0.0 { lastv = v; } }
            (sq, rev) };
        println!("== {} ({} episodes x {} steps): separation rms (px) / x-velocity reversals per 90 steps", ty.name, eps.len(), W);
        // truth at grid
        let (mut sq, mut rev, mut n) = (0.0, 0.0, 0.0);
        for ep in &eps { let xs: Vec<(f64, f64, f64)> = (0..=W).map(|k| { let o = &ep.truth[k]; (k as f64, f64::from(o.x) + f64::from(o.x_fraction)/256.0, f64::from(o.y) + f64::from(o.y_fraction)/256.0) }).collect(); let (a, b) = stat(&xs, &ep.pod); sq += a; rev += b; n += W as f64; }
        println!("   original      : sep rms {:.2}  reversals {:.2}", (sq / n).sqrt(), rev / eps.len() as f64);
        for &rate in &[60i64, 144, 200] { let mut line = format!("   R={rate:3}"); for var in [Var::Drop, Var::GridPred] {
            let (mut sq, mut rev, mut n) = (0.0, 0.0, 0.0);
            for ep in &eps { for ph in [1.0, 0.4] { TICKS.with(|t| t.borrow_mut().clear()); let _ = run_ep(ep, rate, ph, var); let xs = TICKS.with(|t| t.borrow().clone()); let (a, b) = stat(&xs, &ep.pod); sq += a; rev += b; n += xs.len() as f64; } }
            // reversals on tick clock are per tick; normalise to per 90 steps: count / (episodes*phases)
            line += &format!(" | {:?}: sep rms {:.2} reversals {:.2}", var, (sq / n).sqrt(), rev / (eps.len() as f64 * 2.0)); }
            println!("{line}"); }
    }
}
