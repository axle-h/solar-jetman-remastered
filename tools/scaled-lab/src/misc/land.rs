//! Jetman soft landing (levels.md 6.4: state 2/3 and whole vertical speed < 2 on a flat floor).
use super::common::*;
use crate::contact::{Init, M, Rules, Sim, Scen, rules_from, scenarios};
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::{Gameplay, Seed};
use solar_jetman::game::level::World;

#[derive(Clone, Copy, Debug)]
pub struct Drop {
    pub level: usize,
    pub init: Init,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Outcome {
    pub step: usize,
    pub soft: bool,
    pub speed: i64, // whole+fraction, 8.8, speed before the contact step ended
}

fn gravity12(level: &solar_jetman::game::level::Level, y: i32) -> i64 {
    // signed (down positive) level pull in 8.8 per step^2, as the contact model's grav()
    if (level.number as usize) < solar_jetman::game::level::PLANETS && y < 64 {
        let value = ((65 - y) * 2) as u8;
        return i64::from(value / 4);
    }
    let g = i16::from(level.gravity as u8 as i8);
    if g >= 0 { i64::from(g.unsigned_abs() / 4) } else { -i64::from(g.unsigned_abs() / 4) }
}

/// The real engine: Jetman in the air above a flat floor site.
pub fn real(world: &World, d: &Drop) -> Option<Outcome> {
    let sd = Seed { rng: solar_jetman::game::gameplay::rng::Rng { out: 0x78, high: 0xC9, low: 0x03 }, ..Default::default() };
    let mut g = Gameplay::new(world, d.level, sd);
    g.warp(d.init.x, d.init.y, 0, 0);
    let p = &mut g.pod;
    p.state = 2;
    p.half_height = 9;
    p.width = 6;
    p.body.x_fraction = d.init.xf;
    p.body.y_fraction = d.init.yf;
    p.body.h_speed = 0;
    p.body.v_speed = d.init.vs;
    p.body.v_down = true;
    p.fuel = 0xE000;
    p.invulnerable = 0;
    for n in 1..=150 {
        let before = g.pod.body.v_speed;
        g.pod.fuel = 0xE000;
        g.step(world, Buttons(0));
        let p = &g.pod;
        if p.state == 3 {
            return Some(Outcome { step: n, soft: true, speed: i64::from(before) + 12 });
        }
        if p.state == 2 && !p.body.v_down {
            return if p.surface & 1 != 0 { Some(Outcome { step: n, soft: false, speed: i64::from(before) + 12 }) } else { None };
        }
        if p.state != 2 {
            return None;
        }
    }
    None
}

/// The contact model (Jetman: half-height 9, pull fixed at a = 12 per step^2).
pub fn model(world: &World, d: &Drop, rate: u32, rules: Rules) -> Option<(f64, Outcome)> {
    let level = &world.levels[d.level];
    let sim = Sim { world, level, rules };
    let mut m = M::from_init(d.init);
    m.h = 9;
    m.ow = 6;
    let f = f16(rate) as i64;
    let mut t = 0i64;
    for _ in 0..(150 * rate / 30 + 2) {
        // replace the level's pull by 12 for this tick
        let lv = gravity12(level, m.yw());
        m.vy += (12 - lv) * f;
        let vy_before = m.vy;
        let nev = m.ev.len();
        sim.tick(&mut m, f);
        t += f;
        if m.ev.len() > nev {
            let e = m.ev[nev];
            if e.kind == 1 {
                let _ = vy_before;
                let sp = e.speed;
                // time of the contact: end of this tick (the model detects at the tick's position)
                // the original would have sensed this contact at the next 30 Hz grid line, with the pull of the rest of that step
                let to_grid = (65536 - (t % 65536)) % 65536;
                let adj = if grid_adjust() { 12 * to_grid / 65536 } else { 0 };
                return Some((t as f64 / 65536.0, Outcome { step: 0, soft: ((sp + adj) >> 8) < 2, speed: sp }));
            }
            return None;
        }
        if m.surface != 0 {
            return None;
        }
    }
    None
}

fn grid_adjust() -> bool {
    std::env::var("GRID").is_ok()
}
fn dense() -> bool {
    std::env::var("DENSE").is_ok()
}

pub fn drops(world: &World) -> Vec<Drop> {
    let scens: Vec<Scen> = scenarios(world).into_iter().filter(|s| s.class == "F").collect();
    let mut out = Vec::new();
    let mut r = Xs(99);
    for s in &scens {
        for k in 0..if dense() { 30 } else { 12 } {
            let mut init = s.init;
            init.hs = 0;
            init.vd = true;
            init.vs = if dense() { 0x1A0 + 4 * k as u16 } else { [0u16, 0x40, 0x80, 0x100, 0x140, 0x180, 0x1A0, 0x1C0, 0x1E0, 0x200, 0x240, 0x2C0][k] };
            init.yf = r.below(256) as u8;
            init.xf = 0;
            out.push(Drop { level: s.level, init });
        }
    }
    out
}

pub fn report(world: &World) {
    let ds = drops(world);
    let rules = rules_from("corr+tp1+appr+acd+push+wfull+fxcd");
    // validate the model at R=30 against the real engine
    let (mut n, mut agree_soft, mut agree_step, mut both) = (0, 0, 0, 0);
    let mut reals = Vec::new();
    for d in &ds {
        let rl = real(world, d);
        reals.push(rl);
        if let Some(o) = rl {
            n += 1;
            if let Some((t, mo)) = model(world, d, 30, rules) {
                both += 1;
                if mo.soft == o.soft {
                    agree_soft += 1;
                }
                if (t.round() as usize) == o.step {
                    agree_step += 1;
                }
            }
        }
    }
    let soft = reals.iter().flatten().filter(|o| o.soft).count();
    println!("drops {} real outcomes {} (soft {} / bounce {}); model at R=30 has an outcome in {both}: soft/bounce agrees {agree_soft}, contact step agrees {agree_step}", ds.len(), n, soft, n - soft);
    // classification and timing at the other rates vs the real engine
    println!("rate | soft/bounce classification differs | contact time minus real (ms) mean / p90 / min(max lead) | impact speed minus real (1/256) mean / |max| | near threshold only (|v-2.0|<0.1): differs");
    for &r in &RATES {
        let (mut diff, mut cnt, mut dt, mut dv) = (0, 0, Vec::new(), Vec::new());
        let (mut ndiff, mut ncnt) = (0, 0);
        for (d, rl) in ds.iter().zip(&reals) {
            let (Some(o), Some((t, mo))) = (rl, model(world, d, r, rules)) else { continue };
            cnt += 1;
            if mo.soft != o.soft {
                diff += 1;
            }
            if (o.speed - 512).abs() < 26 {
                ncnt += 1;
                if mo.soft != o.soft {
                    ndiff += 1;
                }
            }
            dt.push((t - o.step as f64) * 1000.0 / 30.0);
            dv.push((mo.speed - o.speed) as f64);
        }
        let ab: Vec<f64> = dt.iter().map(|x| x.abs()).collect();
        println!("R={r:3} | {diff}/{cnt} | {:+.1} / {:.1} / {:.1} | {:+.1} / {:.0} | {ndiff}/{ncnt}", mean(&dt), pct(&ab, 0.9), dt.iter().cloned().fold(f64::MAX, f64::min), mean(&dv), absmax(&dv));
    }
}

// ---------------- pod explosion (state 4): half-height growth and the drift up ----------------

pub struct Expl {
    pub level: usize,
    pub init: Init,
    pub hh0: u8,
    /// y (whole, fraction) after explosion steps 1..=10 and after the emerging step 11
    pub y: Vec<(u16, u8)>,
}

pub fn explosions(runs: &[super::foot::Run]) -> Vec<Expl> {
    let mut out = Vec::new();
    for r in runs {
        let k = r.kill;
        if k == usize::MAX || k + 12 > r.recs.len() {
            continue;
        }
        let a = &r.recs[k];
        if a.state != 4 || a.counter != 0 {
            continue;
        }
        if r.recs[k + 1..=k + 10].iter().any(|x| x.state != 4) || r.recs[k + 11].state != 2 {
            continue;
        }
        let init = Init { x: a.x, y: a.y, xf: a.xf, yf: a.yf, hs: 0, hl: false, vs: 0, vd: true };
        out.push(Expl { level: r.level, init, hh0: a.hh, y: r.recs[k + 1..=k + 11].iter().map(|x| (x.y, x.yf)).collect() });
    }
    out
}

/// y (Q16 px) at the end of each whole step 1..=11 for tick rate `rate`; and the time the emerging happens (steps).
const EXPL_FULL: bool = true;

pub fn explosion_model(world: &World, e: &Expl, rate: u32, rules: Rules) -> Vec<i64> {
    let mut rules = rules;
    rules.motion_corr = false; // nothing moves under gravity here
    let level = &world.levels[e.level];
    let sim = Sim { world, level, rules };
    let mut m = M::from_init(e.init);
    m.ow = 6;
    let f = f16(rate) as i64;
    let mut hh_q: i64 = i64::from(e.hh0) << 16;
    let mut t = 0i64;
    let mut out = Vec::new();
    let mut prev_y = m.y;
    let mut next = 65536i64;
    let mut jitter = 0i64;
    let mut pause_until = 0i64;
    while out.len() < 11 {
        let ft = f;
        if t < 10 * 65536 {
            m.h = (hh_q >> 16) as i32;
            let lv = gravity12(level, m.yw());
            m.vy = -(lv * ft);
            m.vx = 0;
            sim.tick(&mut m, ft);
            m.vy = 0;
            if m.surface == 0 {
                if t >= pause_until {
                    hh_q = (hh_q + ft).min(9 << 16);
                }
            } else if m.surface & 0x1F != 0 {
                m.y -= if EXPL_FULL { 65536 } else { ft };
                pause_until = t + 65536;
            }
        }
        let t_new = t + ft;
        let _ = &mut jitter;
        while t_new >= next && out.len() < 11 {
            // linear interpolation of y at the sample time
            let frac = (next - t) as i128 * 65536 / ft as i128;
            let y = prev_y + ((m.y - prev_y) as i128 * frac / 65536) as i64;
            out.push(if out.len() == 10 { y - (2 << 16) } else { y });
            next += 65536;
        }
        prev_y = m.y;
        t = t_new;
    }
    out
}

pub fn report_explosion(world: &World, runs: &[super::foot::Run]) {
    let es = explosions(runs);
    let rules = rules_from("corr+tp1+appr+acd+push+wfull+fxcd");
    println!("pod explosions: {} runs (11 steps each)", es.len());
    let mut kinds = std::collections::BTreeMap::new();
    for e in &es {
        let rise = i32::from(e.init.y) - i32::from(e.y[9].0);
        *kinds.entry(rise).or_insert(0usize) += 1;
    }
    println!("rise of y during steps 1..10 (px) histogram: {:?}", kinds);
    for &r in &RATES {
        let (mut err_all, mut err_fin) = (Vec::new(), Vec::new());
        for e in &es {
            let m = explosion_model(world, e, r, rules);
            for (j, (y, yf)) in e.y.iter().enumerate() {
                let real = (i64::from(*y) << 16) | (i64::from(*yf) << 8);
                let d = (m[j] - real) as f64 / 65536.0;
                err_all.push(d);
                if j == 9 || j == 10 {
                    err_fin.push(d);
                }
            }
        }
        let ab: Vec<f64> = err_all.iter().map(|x| x.abs()).collect();
        println!("R={r:3} y error at the 11 samples: rms {:.3} p99 {:.3} max {:.3} px; at steps 10/11: rms {:.3} max {:.3}", rms(&err_all), pct(&ab, 0.99), absmax(&err_all), rms(&err_fin), absmax(&err_fin));
    }
}

pub fn debug_explosion(world: &World, runs: &[super::foot::Run]) {
    let es = explosions(runs);
    let rules = rules_from("corr+tp1+appr+acd+push+wfull+fxcd");
    for e in es.iter().take(3) {
        let y0 = i64::from(e.init.y) << 16 | i64::from(e.init.yf) << 8;
        println!("L{} y0 {} hh0 {} real rises: {:?}", e.level, e.init.y, e.hh0, e.y.iter().map(|(y, yf)| ((i64::from(*y) << 16 | i64::from(*yf) << 8) - y0) as f64 / 65536.0).collect::<Vec<_>>());
        for r in [60u32, 144] {
            let m = explosion_model(world, e, r, rules);
            println!("   R{r}: {:?}", m.iter().map(|y| (y - y0) as f64 / 65536.0).collect::<Vec<_>>());
        }
    }
}
