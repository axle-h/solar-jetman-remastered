use exp::enemies::ctl::*;
use solar_jetman::game::gameplay::objects::Object;
const XA: [i64; 8] = [0, 0x14, 0x20, 0x14, 0, 0x14, 0x20, 0x14];
const YA: [i64; 8] = [0x20, 0x14, 0, 0x14, 0x20, 0x14, 0, 0x14];
fn same(a: &Object, b: &Object) -> bool { (a.x,a.x_fraction,a.y,a.y_fraction,a.speed_x,a.speed_y,a.flags_x&1,a.flags_y&1)==(b.x,b.x_fraction,b.y,b.y_fraction,b.speed_x,b.speed_y,b.flags_x&1,b.flags_y&1) }
// drive: accel a toward sign (want_neg), or slow by slow_a when a == 0. step of fraction f (Q16); f = 65536 is the original.
fn drive(ax: &mut Ax, want_neg: bool, a: i64, f: i64) {
    let v0 = ax.vel();
    if a == 0 { ax.mag = (ax.mag - 0x12 * U * f / 65536).max(0); }
    else if ax.neg != want_neg { let m = ax.mag - a * U * f / 65536; if m > 0 { ax.mag = m } else { ax.mag = 0; ax.neg = !ax.neg } }
    else { ax.mag += a * U * f / 65536; if ax.mag >> 24 >= 3 { ax.mag = CAP3; } }
    let v1 = ax.vel();
    ax.pos += (f * v1 >> 16) + ((v1 - v0) * (65536 - f) >> 17);
}
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    const W: usize = 24;
    let mut eps: Vec<(Object, Vec<Object>)> = vec![]; let (mut ok, mut bad) = (0, 0);
    for r in &runs { for s in 15..23 { let n = r.frames.len(); let mut i = 0;
        while i + W < n {
            let mut good = true;
            for k in 0..W { let a = &r.frames[i+k].slots[s]; let b = &r.frames[i+k+1].slots[s];
                if a.state != 0x0F || b.state != 0x0F || a.work6 != 0 || b.work6 != 0 { good = false; break; }
                let d = usize::from(b.kind);
                let mut x = ax_x(a); let mut y = ax_y(a);
                drive(&mut x, (5..=7).contains(&d), XA[d], 65536); drive(&mut y, !(3..=5).contains(&d), YA[d], 65536);
                let mut o = *a; o.x = (x.pos >> 24) as u16; o.x_fraction = (x.pos >> 16) as u8; o.y = (y.pos >> 24) as u16; o.y_fraction = (y.pos >> 16) as u8;
                o.speed_x = (x.mag / U) as u16; o.speed_y = (y.mag / U) as u16; o.flags_x = (o.flags_x & !1) | u8::from(x.neg); o.flags_y = (o.flags_y & !1) | u8::from(!y.neg);
                if same(&o, b) { ok += 1 } else { bad += 1; good = false; break; } }
            if good { eps.push((r.frames[i].slots[s], (0..=W).map(|k| r.frames[i+k].slots[s]).collect())); i += W } else { i += 1 }
        } } }
    println!("missile f=1 model: ok {ok} bad {bad}; {} windows of {W} steps", eps.len());
    for &rate in &[30i64, 60, 100, 144, 200, 137] {
        let fq = (65536 * 30 + rate / 2) / rate;
        let mut sq = 0.0; let mut cnt = 0.0; let mut mx: f64 = 0.0;
        for (o0, tr) in &eps { for ph in [1.0f64, 0.3, 0.7] { if rate == 30 && ph != 1.0 { continue }
            let mut x = ax_x(o0); let mut y = ax_y(o0);
            let mut t = 0.0f64; let mut first = true; let (mut px, mut py) = (x.pos as f64 / 16777216.0, y.pos as f64 / 16777216.0); let mut tp = 0.0; let mut next = 1usize;
            while next <= W {
                let ff = if first { ((fq as f64 * ph) as i64).max(1) } else { fq }; first = false;
                let stepk = (t.floor() as usize).min(W - 1); let d = usize::from(tr[stepk + 1].kind);
                drive(&mut x, (5..=7).contains(&d), XA[d], ff); drive(&mut y, !(3..=5).contains(&d), YA[d], ff);
                t += ff as f64 / 65536.0;
                let (cx, cy) = (x.pos as f64 / 16777216.0, y.pos as f64 / 16777216.0);
                while next <= W && next as f64 <= t + 1e-9 { let u = (next as f64 - tp) / (t - tp); let (sx, sy) = (px + (cx - px) * u, py + (cy - py) * u);
                    let tt = &tr[next]; let e = ((sx - (f64::from(tt.x) + f64::from(tt.x_fraction) / 256.0)).powi(2) + (sy - (f64::from(tt.y) + f64::from(tt.y_fraction) / 256.0)).powi(2)).sqrt();
                    sq += e * e; cnt += 1.0; mx = mx.max(e); next += 1; }
                px = cx; py = cy; tp = t;
            } } }
        println!("R={rate:3}: per-tick S-motion, direction (decision) taken from the original's 8-step events: rms {:.4} px, max {:.3} px", (sq / cnt).sqrt(), mx);
    }
}
