//! Area 4: timers and cadences on the game-time clock, against the real engine.
use super::common::*;
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::level::World;

#[derive(Clone, Copy, Debug, Default)]
pub struct T {
    pub step: usize,
    pub counter: u32, // frame counter before the step (A pass sees it)
    pub held: u8,
    pub state: u8,
    pub width: u8,
    pub hh: u8,
    pub shield: bool,
    pub surface: u8,
    pub fuel: u16,
    pub shown: u8,
    pub flame: Option<u8>,
    pub pal: u8,
    pub hit_pause: u8,
    pub inv: u8,
    pub facing: u8,
    pub timer: u8,
    pub device: u8,
    pub antigrav: bool,
    pub y: u16,
    pub fire: bool,
}

pub fn trace(world: &World, name: &str) -> Vec<T> {
    let (mut g, r) = replay_game(world, name);
    let mut out = Vec::new();
    for (i, &b) in r.steps.iter().enumerate() {
        let step = i + 1;
        setups(&mut g, &r, step);
        if g.transition().is_some() {
            break;
        }
        if g.paused() || !g.next_is_a_pass() {
            continue;
        }
        let c = g.counter();
        g.step(world, b);
        let p = &g.pod;
        out.push(T {
            step,
            counter: c,
            held: b.0,
            state: p.state,
            width: p.width,
            hh: p.half_height,
            shield: p.shield,
            surface: p.surface,
            fuel: p.fuel,
            shown: p.fuel_shown,
            flame: p.flame,
            pal: p.palette_bits,
            hit_pause: p.hit_pause,
            inv: p.invulnerable,
            facing: p.facing,
            timer: p.timer,
            device: p.equipment.device,
            antigrav: p.antigravity,
            y: p.body.y,
            fire: p.fire,
        });
    }
    out
}

// ---------- generic deadline lateness ----------

/// A duration of `n` steps armed at a tick (gt = 0): the firing time minus the ideal deadline, ms, for the plain rule (first tick
/// with gt >= deadline) and the centred rule (gt + f/2 >= deadline).
pub fn lateness(rate: u32, n: u64) -> (f64, f64) {
    let f = f32q(rate);
    let deadline = n << 32;
    let mut gt = 0u64;
    while gt < deadline {
        gt += f;
    }
    let a = (gt - deadline) as f64 / 4294967296.0 * 1000.0 / 30.0;
    let mut gt = 0u64;
    while gt + f / 2 < deadline {
        gt += f;
    }
    let b = (gt as f64 - deadline as f64) / 4294967296.0 * 1000.0 / 30.0;
    (a, b)
}

pub fn report_lateness() {
    let ns: [u64; 9] = [8, 10, 11, 48, 64, 96, 128, 192, 1000];
    println!("deadline timers: firing time minus the ideal deadline in ms, plain rule (fire on the first tick with gt >= deadline), f rounded UP");
    println!("   N steps: {:?}", ns);
    for &r in &RATES {
        let v: Vec<String> = ns.iter().map(|&n| format!("{:5.2}", lateness(r, n).0)).collect();
        println!("  R={r:3} (1 tick {:5.2} ms): {}", 1000.0 / r as f64, v.join(" "));
    }
    println!("centred rule (gt + f/2 >= deadline):");
    for &r in &RATES {
        let v: Vec<String> = ns.iter().map(|&n| format!("{:5.2}", lateness(r, n).1)).collect();
        println!("  R={r:3}: {}", v.join(" "));
    }
}

fn runs_of<F: Fn(&T) -> bool>(ts: &[T], f: F) -> Vec<usize> {
    let mut out = Vec::new();
    let mut n = 0;
    for t in ts {
        if f(t) {
            n += 1;
        } else if n > 0 {
            out.push(n);
            n = 0;
        }
    }
    if n > 0 {
        out.push(n);
    }
    out
}

fn hist(v: &[usize]) -> std::collections::BTreeMap<usize, usize> {
    let mut m = std::collections::BTreeMap::new();
    for &x in v {
        *m.entry(x).or_insert(0) += 1;
    }
    m
}

pub fn report_engine(world: &World) {
    // 1. shooting pose: runs of facing 0x28 in Jetman states from the foot runs
    let rs = super::foot::runs(world, 4, 600);
    let mut poses: Vec<usize> = Vec::new();
    let mut pose_press: Vec<usize> = Vec::new();
    for r in &rs {
        let mut n = 0;
        let mut again = false;
        for w in r.recs.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if (b.state == 2 || b.state == 3) && b.facing == 0x28 {
                if a.facing != 0x28 {
                    n = 1;
                    again = false;
                } else {
                    n += 1;
                }
                if b.held & Buttons::A != 0 && a.facing == 0x28 {
                    again = true;
                }
            } else if n > 0 {
                if !again {
                    poses.push(n);
                }
                n = 0;
                pose_press.push(0);
            }
        }
    }
    println!("shoot pose: run lengths (steps) of facing=shoot after a press with no repeat: {:?}", hist(&poses));
    // 2. invulnerability after Jetman emerges (96), hit pause (48): from the foot runs
    let mut inv: Vec<(u8, usize)> = Vec::new();
    let mut dying: Vec<usize> = Vec::new();
    for r in &rs {
        let mut start = None;
        for (i, w) in r.recs.windows(2).enumerate() {
            if w[0].state == 4 && w[1].state == 2 {
                start = Some(i + 1);
            }
            let _ = i;
        }
        if let Some(s) = start {
            let n = r.recs[s..].iter().take_while(|x| x.inv > 0).count();
            inv.push((r.recs[s].inv, n));
        }
        let _ = &mut dying;
    }
    println!("invulnerability after Jetman emerges: (initial value at first step, steps it stays > 0): {:?}", hist(&inv.iter().map(|x| x.1).collect::<Vec<_>>()));
    println!("   initial values {:?}", hist(&inv.iter().map(|x| x.0 as usize).collect::<Vec<_>>()));
    // hit pause: from pod runs; replay with bounces
    for name in ["bounce", "floors-walls", "terrain"] {
        let ts = trace(world, name);
        let mut lens = Vec::new();
        let mut i = 0;
        while i < ts.len() {
            if ts[i].state == 1 && ts[i].hit_pause >= 47 && (i == 0 || ts[i - 1].hit_pause == 0) {
                let n = ts[i..].iter().take_while(|x| x.hit_pause > 0).count();
                lens.push((ts[i].hit_pause, n));
            }
            i += 1;
        }
        println!("hit pause in {name}: (value at the impact step, steps > 0) {:?}", hist(&lens.iter().map(|x| (x.1 * 1000 + x.0 as usize)).collect::<Vec<_>>()));
    }
    let _ = runs_of(&[], |_| true);
}

// ---------- cadences driven by the frame counter ----------

pub fn report_cadences(world: &World) {
    // flame linger: advance when counter&7==0 with B released (A-pass counter), checked on the engine
    for name in ["rotate-thrust", "boosters", "free-flight", "boosters-double"] {
        let ts = trace(world, name);
        let (mut mism, mut checked) = (0, 0);
        let mut d: Option<u8> = None;
        for t in &ts {
            if t.state != 1 {
                d = t.flame;
                continue;
            }
            if t.held & Buttons::B != 0 {
                d = t.flame; // pattern while held: function of the counter (checked below)
            } else if t.counter as u8 & 7 == 0 {
                d = match d {
                    Some(x) if x < 2 => Some(x + 1),
                    _ => None,
                };
            }
            if d != t.flame {
                mism += 1;
            }
            checked += 1;
        }
        println!("flame {name}: {} steps, linger rule mismatches vs engine {mism}/{checked}", ts.len());
    }
    // flame pattern while B held (counter bits 1-3), non-boosted and boosted
    for name in ["rotate-thrust", "boosters", "boosters-double"] {
        let ts = trace(world, name);
        let (mut bad, mut n) = (0, 0);
        for t in &ts {
            if t.state == 1 && t.held & Buttons::B != 0 && t.fuel > 0 {
                let c = t.counter as u8;
                let boosted = t.held & Buttons::SELECT != 0 && name != "rotate-thrust";
                let want = if boosted { [0, 1, 2, 1, 0, 1, 2, 1][usize::from((c & 0x0E) / 2)] } else if c & 2 == 0 { 2 } else { 1 };
                n += 1;
                if t.flame != Some(want) && !(boosted && false) {
                    bad += 1;
                }
            }
        }
        println!("flame pattern {name}: {bad}/{n} steps differ from the counter formula (boosted formula assumed when Select held)");
    }
    // palette flicker of the momentum killer and anti-gravity
    for (name, mode) in [("momentum-killer", 0u8), ("antigravity", 1)] {
        let ts = trace(world, name);
        let (mut bad, mut n) = (0, 0);
        for t in &ts {
            if t.state != 1 || t.hit_pause != 0 || t.inv != 0 {
                continue;
            }
            let c = t.counter as u8;
            let want = if mode == 0 {
                if t.device != 9 { continue } else if t.held & Buttons::B != 0 { 0 } else { (c >> 2 & 1) << 1 }
            } else if t.antigrav {
                c >> 1 & 3
            } else {
                continue;
            };
            n += 1;
            if t.pal != want {
                bad += 1;
            }
        }
        println!("palette {name}: {bad}/{n} steps differ from the counter formula");
    }
}

// ---------- fuel gauge easing ----------

pub fn ease_step(shown: u8, target: u8) -> u8 {
    if shown < target {
        shown + ((target - shown) / 64).clamp(1, 3)
    } else if shown > target {
        shown - if shown - target <= 16 { 1 } else { 3 }
    } else {
        shown
    }
}

/// Continuous version: shown in Q16 gauge units, moved by s*f per tick, s from the current whole gap.
pub fn ease_tick(shown_q: &mut i64, target: u8, f: i64) {
    let shown = (*shown_q >> 16) as u8;
    let t = i64::from(target) << 16;
    if *shown_q < t {
        let gap = target - shown.min(target);
        let s = i64::from((gap / 64).clamp(1, 3));
        *shown_q = (*shown_q + s * f).min(t);
    } else if *shown_q > t {
        let gap = shown.saturating_sub(target);
        let s = if gap <= 16 { 1 } else { 3 };
        *shown_q = (*shown_q - s * f).max(t);
    }
}

pub fn report_gauge(world: &World) {
    let mut series: Vec<(String, Vec<T>)> = Vec::new();
    for name in ["free-flight", "delivery-thrust", "boosters", "delivery", "level1", "tow", "shield", "respawn"] {
        series.push((name.to_string(), trace(world, name)));
    }
    // which target does the engine's easing use?
    for (name, ts) in &series {
        let (mut a, mut b, mut n) = (0, 0, 0);
        for w in ts.windows(2) {
            if w[0].state == 0 {
                continue;
            }
            n += 1;
            if ease_step(w[0].shown, (w[1].fuel >> 8) as u8) == w[1].shown {
                a += 1;
            }
            if ease_step(w[0].shown, (w[0].fuel >> 8) as u8) == w[1].shown {
                b += 1;
            }
        }
        println!("gauge {name}: {n} steps; shown_n = ease(shown_n-1, fuel_n): {a}; with fuel_n-1: {b}");
    }
    println!("continuous easing vs the engine's gauge (units of 1/256 of the tank), per rate: rms / max error at the 30 Hz samples; steps with |gauge| error > 1 unit %");
    for &r in &RATES {
        let (mut e, mut tot) = (Vec::new(), 0usize);
        let mut big = 0usize;
        for (_, ts) in &series {
            if ts.is_empty() {
                continue;
            }
            let f = f16(r) as i64;
            let mut shown_q: i64 = i64::from(ts[0].shown) << 16;
            let mut t_q: i64 = 0; // Q16 steps
            let mut k = 0usize; // number of completed samples
            while k < ts.len() {
                // target in force: fuel of the last completed step (aligned with the engine's sample k+1)
                let target = (ts[k.saturating_sub(0)].fuel >> 8) as u8;
                ease_tick(&mut shown_q, target, f);
                t_q += f;
                while k < ts.len() && t_q >= (k as i64 + 1) * 65536 {
                    let model = (shown_q >> 16) as i64;
                    let exception = k == 0 || ease_step(ts[k - 1].shown, (ts[k].fuel >> 8) as u8) != ts[k].shown;
                    if exception {
                        // the engine did not ease in this step (11 of 3917 in level1): resynchronise, do not count
                        shown_q = i64::from(ts[k].shown) << 16;
                    } else {
                        let err = (model - i64::from(ts[k].shown)) as f64;
                        e.push(err);
                        if err.abs() > 1.0 {
                            big += 1;
                        }
                        tot += 1;
                    }
                    k += 1;
                }
            }
        }
        println!("  R={r:3}: rms {:.3} max {:.0} | >1 unit {:.2}% of {tot}", rms(&e), absmax(&e), 100.0 * big as f64 / tot as f64);
    }
}

pub fn debug_gauge(world: &World) {
    let ts = trace(world, "level1");
    let mut shown_q: i64 = i64::from(ts[0].shown) << 16;
    let mut n = 0;
    for k in 0..ts.len() {
        let target = (ts[k].fuel >> 8) as u8;
        ease_tick(&mut shown_q, target, 65536);
        let model = (shown_q >> 16) as i64;
        if model != i64::from(ts[k].shown) && n < 8 {
            println!("k {k} fuel>>8 {target} engine shown {} model {} (prev engine {})", ts[k].shown, model, if k > 0 { ts[k - 1].shown } else { 0 });
            n += 1;
        }
        shown_q = i64::from(ts[k].shown) << 16; // resync
    }
}

pub fn report_shield(world: &World) {
    for name in ["shield", "super-shields", "boosters"] {
        let ts = trace(world, name);
        let (mut n, mut okw, mut okh, mut grow_runs) = (0, 0, 0, Vec::new());
        let mut run = 0;
        let mut start = None;
        for w in ts.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if a.state != 1 || b.state != 1 || !b.shield {
                if run > 0 { grow_runs.push((run, start)); run = 0; }
                continue;
            }
            n += 1;
            let want_w = if b.surface == 0 && b.y >= 32 && a.width < 12 { a.width + 1 } else { a.width };
            if want_w == b.width { okw += 1; }
            if b.width == a.width + 1 { if run == 0 { start = Some(a.width); } run += 1; } else if run > 0 { grow_runs.push((run, start)); run = 0; }
            let inc_h = b.hh == a.hh + 1;
            if b.surface != 0 && b.hh != a.hh { /* growth with touched is impossible */ }
            if (b.surface == 0 && a.hh < 12 && inc_h) || (!inc_h && (b.surface != 0 || a.hh >= 12 || true)) { okh += 1; }
        }
        let mut h = std::collections::BTreeMap::new();
        for (r, s) in &grow_runs { *h.entry((*r, s.unwrap_or(0))).or_insert(0usize) += 1; }
        println!("shield {name}: {n} shielded steps; width rule (+1 when nothing touched, y>=32, below 12) matches {okw}/{n}; growth runs (length, start width) {:?}", h);
        let _ = okh;
    }
}
