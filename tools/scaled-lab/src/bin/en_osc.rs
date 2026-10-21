use exp::enemies::ctl::*;
// Stationary target at whole px 1000 (+0.0); object starts d px away, v=0. 1-D. Reports peaks (amplitude about target) and crossing period.
fn run(p: Par, d: f64, rate: i64, turn: Option<Turn>, steps: f64, latch: bool) -> Vec<(f64, f64)> {
    // returns (time, pos-target) samples at every tick
    let mut ax = Ax { pos: ((1000.0 + d) * 16777216.0) as i64, mag: 0, neg: false };
    let fq = (65536 * 30 + rate / 2) / rate;
    let mut t = 0.0; let mut out = vec![(0.0, d)];
    let tgt = 1000; let mut ev = 0usize; let mut side = true;
    while t < steps {
        if latch {
            while (ev as f64) <= t + 1e-9 { side = i32::from((tgt - ax.whole()) as i16) >= 0; ev += 1; }
            latch_axis(&mut ax, side, p, fq); t += fq as f64 / 65536.0;
        } else {
        match turn { None => orig_axis(&mut ax, tgt, p), Some(tn) => tick_axis(&mut ax, tgt, p, fq, tn) }
        t += if turn.is_none() { 1.0 } else { fq as f64 / 65536.0 };}
        out.push((t, ax.pos as f64 / 16777216.0 - 1000.0));
    }
    out
}
fn analyse(s: &[(f64, f64)]) -> (Vec<f64>, f64) {
    // peaks: local extrema of |pos|; period: mean time between same-direction crossings of 0.5 (target + 0.5 ~ centre of the whole-px decision)
    let mut peaks = vec![];
    for i in 1..s.len()-1 { if (s[i].1 > s[i-1].1 && s[i].1 >= s[i+1].1) || (s[i].1 < s[i-1].1 && s[i].1 <= s[i+1].1) { peaks.push(s[i].1); } }
    let mut cr = vec![];
    for i in 1..s.len() { if s[i-1].1 > 0.0 && s[i].1 <= 0.0 { cr.push(s[i].0); } }
    let per = if cr.len() >= 3 { (cr[cr.len()-1]-cr[0]) / (cr.len()-1) as f64 } else { f64::NAN };
    (peaks, per)
}
fn main() {
    let sets = [("mine a=8 m=1.0", Par{a:8,max:0x100}), ("mine a=8 m=1.875", Par{a:8,max:0x1E0}), ("orb a=2 m=.5", Par{a:2,max:0x80}), ("sphere x a=8 m=3", Par{a:8,max:0x400}), ("sphere y a=24 m=3", Par{a:0x18,max:0x400}), ("sat y a=16 m=2.75", Par{a:0x10,max:0x2C0}), ("sat x a=3 m=.75", Par{a:3,max:0xC0}), ("companion a=240 m=3", Par{a:0xF0,max:0x380}), ("creature a=1 m=.5", Par{a:1,max:0x80})];
    for (name, p) in sets {
        for &d in &[6.0, 30.0, 100.0] {
            let o = run(p, d, 30, None, 1200.0, false);
            let (pk, per) = analyse(&o);
            let n = pk.len();
            let amp = |pk: &Vec<f64>, a: usize, b: usize| { let s: f64 = pk[a..b.min(pk.len())].iter().map(|x| x.abs()).sum(); s / (b.min(pk.len()) - a).max(1) as f64 };
            let late = |pk: &Vec<f64>| { let m = pk.len(); amp(pk, m.saturating_sub(8), m) };
            println!("{name} d={d}: ORIG peaks={n} first {:.2} last8 {:.2} period {:.2} steps", pk.first().copied().unwrap_or(0.0).abs(), late(&pk), per);
            for &rate in &[60i64, 144, 200] {
                for (tn, lt) in [(Turn::Drop, false), (Turn::Reflect, true)] {
                    let o = run(p, d, rate, Some(tn), 1200.0, lt);
                    let (pk, per) = analyse(&o);
                    println!("     R={rate} {:?}/{}: peaks={} first {:.2} last8 {:.2} period {:.2}", tn, lt, pk.len(), pk.first().copied().unwrap_or(0.0).abs(), late(&pk), per);
                }
            }
        }
    }
}
