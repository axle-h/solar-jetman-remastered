//! Jetman flying (player.md 6.1, J1-J2): the same exercise as the pod, with friction and steering.
use super::integ::{ONE, S, Ticker};
use super::rec::{Rec, Seg, run_as, _seed};
use solar_jetman::game::gameplay::Gameplay;
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::level::World;

#[derive(Clone, Copy, Debug)]
pub struct JIn {
    pub l: bool,
    pub r: bool,
    pub down: bool,
    pub b: bool,
}
pub fn jin(r: &Rec) -> JIn {
    JIn { l: r.held & Buttons::LEFT != 0, r: r.held & Buttons::RIGHT != 0, down: r.held & Buttons::DOWN != 0, b: r.held & Buttons::B != 0 }
}

const U: i64 = 0x10000;
#[derive(Clone, Copy)]
pub struct JOpts {
    pub corr: bool,
    pub lag: bool,
    /// kick (extra 0x30 after an upward reversal): 0 keep, 1 scale by f, 2 drop
    pub kick: u8,
    pub cap_bias: bool,
}

fn cap3(m: &mut i64) {
    if *m >= 3 << 24 {
        *m = 3 << 24;
    }
}

fn steer(s: &mut S, f: u32, i: &JIn, o: &JOpts) -> u8 {
    let mut fl = 0u8;
    let f = i64::from(f);
    if i.l || i.r {
        let toward_left = i.l;
        if s.hl != toward_left {
            let amt = 0x38 * f;
            if s.hm > amt {
                s.hm -= amt;
            } else {
                s.hm = amt - s.hm;
                s.hl = !s.hl;
                fl |= 1;
            }
        } else {
            s.hm = (s.hm + 0x28 * f).min(7 << 24);
        }
    }
    if i.down {
        if !s.vd {
            let amt = 0x1C * f;
            if s.vm > amt {
                s.vm -= amt;
            } else {
                s.vm = 0;
                s.vd = true;
                fl |= 2;
            }
        } else {
            s.vm = (s.vm + 0x1A * f).min(3 << 24);
        }
    } else if i.b {
        if s.vd {
            let amt = 0x20 * f;
            if s.vm > amt {
                s.vm -= amt;
            } else {
                s.vm = amt - s.vm;
                s.vd = false;
                fl |= 4;
                s.vm += match o.kick {
                    0 => 0x30 * U,
                    1 => 0x30 * f,
                    _ => 0,
                };
            }
        } else {
            s.vm += 0x30 * f;
        }
        if o.cap_bias && !s.vd {
            // the original's displacement at the cap is the cap minus one full step of gravity
            let lim = (3 << 24) - ((12 * f * (i64::from(ONE) - f)) >> 16);
            if s.vm > lim {
                s.vm = lim;
            }
        } else {
            cap3(&mut s.vm);
        }
    }
    fl
}

pub fn jtick(s: &mut S, f: u32, steer_in: Option<&JIn>, o: &JOpts) -> u8 {
    let mut fl = 0u8;
    let fi = i64::from(f);
    let disp = |s: &S| (if s.hl { -s.hm } else { s.hm }, if s.vd { s.vm } else { -s.vm });
    let (vx0, vy0) = disp(s);
    // friction, then steering (both after the move in the original)
    if let Some(i) = steer_in {
        if s.hm <= 2 * fi {
            fl |= 8;
        }
        s.hm = (s.hm - 2 * fi).max(0);
        fl |= steer(s, f, i, o);
    }
    // gravity 12 down
    let da = 12 * fi;
    if s.vd {
        s.vm += da;
    } else if s.vm > da {
        s.vm -= da;
    } else {
        s.vm = da - s.vm;
        s.vd = true;
        fl |= 16;
    }
    if s.hm >= 3 << 24 || s.vm >= 3 << 24 {
        fl |= 32;
    }
    cap3(&mut s.hm);
    cap3(&mut s.vm);
    let (vx1, vy1) = disp(s);
    let k = i64::from(ONE - f);
    let mut dx = (vx1 * fi) >> 16;
    let mut dy = (vy1 * fi) >> 16;
    if o.corr {
        dx += ((vx1 - vx0) * k) >> 17;
        dy += ((vy1 - vy0) * k) >> 17;
    }
    s.x += dx;
    s.y += dy;
    fl
}

fn from_rec(r: &Rec) -> S {
    let mut s = S::from_rec(r);
    s.hacc = 0;
    s.vacc = 0;
    s
}

/// States at sample times 1..=n. mode 0 split at step edges, 1 tick-start input.
fn sim(seg: &Seg, o: &JOpts, sched: &[(u64, u32)], mode: u8) -> Vec<(i64, i64, i64, i64)> {
    let n = seg.steps.len();
    let mut tk = Ticker::new(sched);
    let mut s = from_rec(&seg.start);
    let mut out = Vec::new();
    let sig = |k: usize| -> Option<JIn> {
        let k = k.clamp(1, n);
        if o.lag {
            if k >= 2 { Some(jin(&seg.steps[k - 2])) } else { None }
        } else {
            Some(jin(&seg.steps[k - 1]))
        }
    };
    let fin = |s: &S, k: usize| {
        let mut c = *s;
        if o.lag {
            // pending friction and steering of step k at full size
            c.hm = (c.hm - 2 * i64::from(ONE)).max(0);
            let _ = steer(&mut c, ONE, &jin(&seg.steps[k - 1]), o);
        }
        let h = if c.hl { -c.hm } else { c.hm };
        let v = if c.vd { -c.vm } else { c.vm };
        (c.x, c.y, h, v)
    };
    while out.len() < n {
        let t0 = tk.t;
        let end = tk.next_end();
        let k_int = (t0 / u64::from(ONE)) as usize + 1;
        let inp = sig(k_int);
        let k_next = out.len() + 1;
        let sample_t = k_next as u64 * u64::from(ONE);
        if mode == 0 {
            let b = k_int as u64 * u64::from(ONE);
            let eff = end.min(b);
            let _ = jtick(&mut s, (eff - t0) as u32, inp.as_ref(), o);
            if eff == end {
                tk.advance(end);
            } else {
                tk.t = eff;
            }
            if tk.t == sample_t {
                out.push(fin(&s, k_next));
            }
        } else {
            if end > sample_t && t0 < sample_t {
                let mut c = s;
                let _ = jtick(&mut c, (sample_t - t0) as u32, inp.as_ref(), o);
                out.push(fin(&c, k_next));
            }
            let _ = jtick(&mut s, (end - t0) as u32, inp.as_ref(), o);
            tk.advance(end);
            if end == sample_t {
                out.push(fin(&s, k_next));
            }
        }
    }
    out
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

pub fn report(world: &World) {
    // Jetman runs: fly the pod, empty the tank, then steer Jetman with random holds.
    let mut segs = Vec::new();
    for level in 0..=12usize {
        for seed in 0..40u64 {
            let mut g = Gameplay::new(world, level, _seed(0, 0));
            g.pod.equipment.owned = 0;
            let mut r = Rng((seed * 131 + level as u64 * 17 + 5) | 1);
            let mut inputs = Vec::new();
            let mut cur = 0u8;
            for i in 0..500 {
                if i % (3 + (r.next() % 25) as usize) == 0 {
                    cur = 0;
                    if r.next() % 100 < 70 {
                        cur |= Buttons::B;
                    }
                    match r.next() % 4 {
                        0 => cur |= Buttons::LEFT,
                        1 => cur |= Buttons::RIGHT,
                        _ => {}
                    }
                    if r.next() % 100 < 15 {
                        cur |= Buttons::DOWN;
                    }
                }
                inputs.push(Buttons(cur));
            }
            let kill_at = 5 + (r.next() % 40) as usize;
            let name = format!("jet-L{level}-s{seed}");
            segs.extend(run_as(world, g, &inputs, &name, 8, &mut |g, step| {
                if step == kill_at {
                    g.pod.fuel = 0;
                }
                if g.pod.state == 2 && g.pod.fuel < 0x4000 {
                    g.pod.fuel = 0xE000;
                }
            }, true));
        }
    }
    println!("jetman: {} segments, {} steps", segs.len(), segs.iter().map(|s| s.steps.len()).sum::<usize>());
    // exactness at f=1 against the engine; segments are cut at steps the model does not reproduce
    let one = [(0u64, 30u32)];
    let o1 = JOpts { corr: true, lag: true, kick: 0, cap_bias: false };
    let mut clean = Vec::new();
    let (mut tot, mut cuts) = (0usize, 0usize);
    let matches = |a: &(i64, i64, i64, i64), r: &Rec| {
        let w = S::from_rec(r);
        let wh = if w.hl { -w.hm } else { w.hm };
        let wv = if w.vd { -w.vm } else { w.vm };
        (a.0, a.1, a.2 >> 16, a.3 >> 16) == (w.x, w.y, wh >> 16, wv >> 16)
    };
    let mut work: Vec<Seg> = segs.clone();
    while let Some(seg) = work.pop() {
        let out = sim(&seg, &o1, &one, 0);
        tot += out.len();
        match out.iter().zip(&seg.steps).position(|(a, r)| !matches(a, r)) {
            None => clean.push(seg),
            Some(i) => {
                cuts += 1;
                if i >= 8 {
                    work.push(Seg { name: seg.name.clone(), start: seg.start, steps: seg.steps[..i].to_vec() });
                }
                if seg.steps.len() - i - 1 >= 8 {
                    work.push(Seg { name: seg.name.clone(), start: seg.steps[i], steps: seg.steps[i + 1..].to_vec() });
                }
            }
        }
    }
    let segs = clean;
    println!("jetman exact@30: model mismatches cut {cuts}; kept {} segments, {} steps, all bit-exact in position and whole-unit speed", segs.len(), segs.iter().map(|s| s.steps.len()).sum::<usize>());
    let _ = tot;
    // event classes from a run at f=1 (non-lagged order identical at f=1 for flags)
    let mut cls: Vec<u8> = Vec::new();
    for seg in &segs {
        let mut st = from_rec(&seg.start);
        let o = JOpts { corr: true, lag: false, kick: 0, cap_bias: false };
        let mut fl = 0u8;
        for r in &seg.steps {
            fl |= jtick(&mut st, ONE, Some(&jin(r)), &o);
        }
        cls.push(fl);
    }
    let count = |m: u8| cls.iter().filter(|&&c| c & m != 0).count();
    println!("jetman segments with: vertical flip by Up {} / by Down {} / horizontal brake flip {} / friction floor {} / gravity flip {} / cap {}", count(4), count(2), count(1), count(8), count(16), count(32));
    if std::env::var("JDEBUG").is_ok() {
        let o = JOpts { corr: true, lag: true, kick: 0, cap_bias: false };
        let mut worst = (0.0f64, 0usize);
        for (si, seg) in segs.iter().enumerate() {
            if cls[si] != 0 { continue; }
            let out = sim(seg, &o, &[(0, 60)], 0);
            for (a, rec) in out.iter().zip(&seg.steps) {
                let w = S::from_rec(rec);
                let q = |v: i64| v as f64 / (1u64 << 24) as f64;
                let p = (q(a.0 - w.x).powi(2) + q(a.1 - w.y).powi(2)).sqrt();
                if p > worst.0 { worst = (p, si); }
            }
        }
        let seg = &segs[worst.1];
        println!("worst jet no-event {} len {} pe {:.3}", seg.name, seg.steps.len(), worst.0);
        let out = sim(seg, &o, &[(0, 60)], 0);
        for (k, (a, rec)) in out.iter().zip(&seg.steps).enumerate() {
            let w = S::from_rec(rec);
            let q = |v: i64| v as f64 / (1u64 << 24) as f64;
            let (wh, wv) = (if w.hl { -w.hm } else { w.hm }, if w.vd { -w.vm } else { w.vm });
            println!("k {k:3} held {:02x} orig v ({:.4},{:.4}) ours ({:.4},{:.4}) pos err ({:.4},{:.4})", rec.held, q(wh), q(wv), q(a.2), q(a.3), q(a.0 - w.x), q(a.1 - w.y));
        }
        return;
    }
    let rates = [30u32, 60, 75, 90, 100, 120, 137, 144, 165, 200];
    let classes: [(&str, Box<dyn Fn(u8) -> bool>); 3] = [
        ("no events", Box::new(|c| c == 0)),
        ("no flips/kicks (caps and friction floor allowed)", Box::new(|c| c & (1 | 2 | 4 | 16) == 0)),
        ("all", Box::new(|_| true)),
    ];
    for (cname, sel) in &classes {
        println!("--- Jetman class: {cname}");
    for (name, o) in [
        ("lagged, exact form", JOpts { corr: true, lag: true, kick: 0, cap_bias: false }),
        ("lagged, no correction", JOpts { corr: false, lag: true, kick: 0, cap_bias: false }),
        ("lagged, exact form, cap lowered", JOpts { corr: true, lag: true, kick: 0, cap_bias: true }),
    ] {
        println!("{name}");
        for &r in &rates {
            let mut pe = Vec::new();
            let mut ve = Vec::new();
            let mut drift = 0.0;
            let mut nseg = 0;
            for (si, seg) in segs.iter().enumerate() {
                if !sel(cls[si]) {
                    continue;
                }
                nseg += 1;
                let out = sim(seg, &o, &[(0, r)], 0);
                let mut last = 0.0;
                for (a, rec) in out.iter().zip(&seg.steps) {
                    let w = S::from_rec(rec);
                    let (wh, wv) = (if w.hl { -w.hm } else { w.hm }, if w.vd { -w.vm } else { w.vm });
                    let q = |v: i64| v as f64 / (1u64 << 24) as f64;
                    let p = (q(a.0 - w.x).powi(2) + q(a.1 - w.y).powi(2)).sqrt();
                    let v = (q(a.2 - wh).powi(2) + q(a.3 - wv).powi(2)).sqrt();
                    pe.push(p);
                    ve.push(v);
                    last = p;
                }
                drift += last / out.len() as f64;
            }
            if pe.is_empty() { continue; }
            let rms = |v: &Vec<f64>| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
            let mut sorted = pe.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("   R={r:3} segs {nseg:4} pos max {:7.4} p99 {:7.4} p50 {:7.5} rms {:7.4} | vel rms {:7.5} | drift {:8.6}/step", sorted.last().unwrap(), sorted[(sorted.len() as f64 * 0.99) as usize], sorted[sorted.len() / 2], rms(&pe), rms(&ve), drift / nseg as f64);
        }
    }
    }
}
