//! Area 3: the gate state (L56) and the warp level routine.
use super::common::*;
use crate::motion::integ::{ONE, Ticker};
use solar_jetman::game::level::World;

#[derive(Clone, Debug, Default)]
pub struct G {
    pub n: usize, // call number within the gate state (1 = first)
    pub counter: u32,
    pub k: u8,
    pub fades: u8,
    pub flash: u8,
    pub facing: u8,
    pub x: u16,
    pub xf: u8,
    pub y: u16,
    pub yf: u8,
    pub h: u16,
    pub hl: bool,
    pub v: u16,
    pub vd: bool,
    pub at: (u16, u16),
    pub grp: [bool; 2],
    pub parts: Vec<(u16, u16)>, // positions of group A (15..22) and B (23..30), 16 entries
}

pub fn record(world: &World, name: &str) -> Vec<Vec<G>> {
    let (mut g, r) = replay_game(world, name);
    let mut out: Vec<Vec<G>> = Vec::new();
    let mut cur: Vec<G> = Vec::new();
    for (i, &b) in r.steps.iter().enumerate() {
        setups(&mut g, &r, i + 1);
        if g.transition().is_some() {
            break;
        }
        if g.paused() {
            continue;
        }
        g.step(world, b);
        if g.pod.state == 9 {
            let p = &g.pod;
            let slot = p.timer as usize;
            let at = (g.objects.slots[slot].x, g.objects.slots[slot].y);
            cur.push(G {
                n: cur.len() + 1,
                counter: g.counter(),
                k: (g.tow.angle & 0xFF) as u8,
                fades: g.gate_fades(),
                flash: g.gate_flash().unwrap_or(0),
                facing: p.facing,
                x: p.body.x,
                xf: p.body.x_fraction,
                y: p.body.y,
                yf: p.body.y_fraction,
                h: p.body.h_speed,
                hl: p.body.h_left,
                v: p.body.v_speed,
                vd: p.body.v_down,
                at,
                grp: [g.objects.slots[15].state != 0, g.objects.slots[23].state != 0],
                parts: (15..31).map(|s| (g.objects.slots[s].x, g.objects.slots[s].y)).collect(),
            });
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn cadences(world: &World) {
    for name in ["wormhole", "wormhole-jetman", "wormhole-warp"] {
        let gs = record(world, name);
        for (i, g) in gs.iter().enumerate() {
            // calls at which k increments, fades increments, group appearances
            let (mut kc, mut fc) = (Vec::new(), Vec::new());
            for w in g.windows(2) {
                if w[1].k != w[0].k {
                    kc.push(w[1].n);
                }
                if w[1].fades != w[0].fades {
                    fc.push(w[1].n);
                }
            }
            let kd: Vec<usize> = kc.windows(2).map(|w| w[1] - w[0]).collect();
            let fd: Vec<usize> = fc.windows(2).map(|w| w[1] - w[0]).collect();
            let flash_alt = g.windows(2).all(|w| w[0].flash != w[1].flash);
            println!("{name}[{i}]: {} calls; k increments: first at call {:?}, last at {:?}, spacing set {:?}, {} total; fades at calls {:?}.. spacing {:?}; flash alternates every call: {flash_alt}; counter at call 1: {:#x}, ends with k={}",
                g.len(), kc.first(), kc.last(), kd.iter().collect::<std::collections::BTreeSet<_>>(), kc.len(), &fc[..fc.len().min(3)], fd.iter().collect::<std::collections::BTreeSet<_>>(), g[0].counter, g.last().unwrap().k);
        }
    }
}

// ---- pod homing on the gate ----
#[derive(Clone, Copy)]
struct H {
    x: i64, // Q.24 px (wraps like the u16)
    y: i64,
    hm: i64,
    hl: bool,
    vm: i64,
    vd: bool,
}
const U: i64 = 0x10000;

fn from_g(g: &G) -> H {
    H {
        x: (i64::from(g.x) << 24) | (i64::from(g.xf) << 16),
        y: (i64::from(g.y) << 24) | (i64::from(g.yf) << 16),
        hm: i64::from(g.h) << 16,
        hl: g.hl,
        vm: i64::from(g.v) << 16,
        vd: g.vd,
    }
}
fn sg(m: i64, neg: bool) -> i64 {
    if neg { -m } else { m }
}

/// One tick of the homing (player.md 9.2.3 step 2) at fraction f.
fn tick(h: &mut H, f: i64, at: (u16, u16)) {
    let (vx0, vy0) = (sg(h.hm, h.hl), sg(h.vm, !h.vd));
    let right = (at.0.wrapping_sub((h.x >> 24) as u16) as i16) >= 0;
    let toward_x = if right { !h.hl } else { h.hl };
    let down = (at.1.wrapping_sub((h.y >> 24) as u16) as i16) >= 0;
    let toward_y = if down { h.vd } else { !h.vd };
    if toward_x {
        h.hm = (h.hm + 0x80 * f).min(0x380 * U);
    } else {
        h.hm = (h.hm - 0x80 * f).max(0);
        if h.hm == 0 {
            h.hl = !h.hl;
        }
    }
    if toward_y {
        h.vm = (h.vm + 0x80 * f).min(0x300 * U);
    } else {
        h.vm = (h.vm - 0x80 * f).max(0);
        if h.vm == 0 {
            h.vd = !h.vd;
        }
    }
    let (vx1, vy1) = (sg(h.hm, h.hl), sg(h.vm, !h.vd));
    let k = ONE as i64 - f;
    h.x += ((vx1 * f) >> 16) + (((vx1 - vx0) * k) >> 17);
    h.y += ((vy1 * f) >> 16) + (((vy1 - vy0) * k) >> 17);
}

/// Records reproduced exactly by the f = 1 model (a replay's test setup can teleport the pod in the middle of a gate).
fn valid_len(g: &[G]) -> usize {
    let at = g[0].at;
    let mut s = from_g(&g[0]);
    for i in 1..g.len() {
        tick(&mut s, 65536, at);
        let real = from_g(&g[i]);
        if (s.x, s.y, s.hm >> 16, s.vm >> 16, s.hl, s.vd) != (real.x, real.y, real.hm >> 16, real.vm >> 16, real.hl, real.vd) {
            return i;
        }
    }
    g.len()
}

pub fn homing(world: &World) {
    let mut gates: Vec<Vec<G>> = Vec::new();
    for name in ["wormhole", "wormhole-jetman", "wormhole-warp"] {
        gates.extend(record(world, name));
    }
    let _ = &mut gates;
    // reconstruct: the engine state after call n; the model starts from the state after call 1 (call 1 includes the first homing step)
    println!("gate homing: {} gate states, calls {:?}", gates.len(), gates.iter().map(|g| g.len()).collect::<Vec<_>>());
    for (label, flip_hold) in [("plain", false)] {
        let _ = flip_hold;
        println!("--- pod homing, split sampling and free ticks ({label}); position error at the 30 Hz samples (px): rms / max, speed rms (1/256)");
        for &mode in &[0u8, 1] {
            for &r in &RATES {
                let (mut pe, mut ve) = (Vec::new(), Vec::new());
                for g in &gates {
                    if g.len() < 10 {
                        continue;
                    }
                    let g = &g[1..]; // the first record holds two calls (see above): start after it
                    let g = &g[..valid_len(g)];
                    let at = g[0].at;
                    let mut tk = Ticker::new(&[(0, r)]);
                    let mut s = from_g(&g[0]);
                    let mut idx = 1usize;
                    while idx < g.len() {
                        let t0 = tk.t;
                        let end = tk.next_end();
                        let sample_t = idx as u64 * u64::from(ONE);
                        let b = if mode == 0 { sample_t.min(end) } else { end };
                        let eff = b;
                        if mode == 1 && end > sample_t && t0 < sample_t {
                            let mut c = s;
                            tick(&mut c, (sample_t - t0) as i64, at);
                            let real = from_g(&g[idx]);
                            pe.push(((c.x - real.x) as f64 / 16777216.0).hypot((c.y - real.y) as f64 / 16777216.0));
                            ve.push(((sg(c.hm, c.hl) - sg(real.hm, real.hl)) as f64 / 65536.0).hypot((sg(c.vm, !c.vd) - sg(real.vm, !real.vd)) as f64 / 65536.0));
                            idx += 1;
                        }
                        tick(&mut s, (eff - t0) as i64, at);
                        if eff == end {
                            tk.advance(end);
                        } else {
                            tk.t = eff;
                        }
                        if mode == 0 && tk.t == sample_t {
                            let real = from_g(&g[idx]);
                            pe.push(((s.x - real.x) as f64 / 16777216.0).hypot((s.y - real.y) as f64 / 16777216.0));
                            ve.push(((sg(s.hm, s.hl) - sg(real.hm, real.hl)) as f64 / 65536.0).hypot((sg(s.vm, !s.vd) - sg(real.vm, !real.vd)) as f64 / 65536.0));
                            idx += 1;
                        } else if mode == 1 && end == sample_t && idx < g.len() {
                            let real = from_g(&g[idx]);
                            pe.push(((s.x - real.x) as f64 / 16777216.0).hypot((s.y - real.y) as f64 / 16777216.0));
                            ve.push(((sg(s.hm, s.hl) - sg(real.hm, real.hl)) as f64 / 65536.0).hypot((sg(s.vm, !s.vd) - sg(real.vm, !real.vd)) as f64 / 65536.0));
                            idx += 1;
                        }
                    }
                }
                println!("  mode {mode} R={r:3}: n {:4} pos rms {:7.4} max {:7.4} | v rms {:7.3}", pe.len(), rms(&pe), absmax(&pe), rms(&ve));
            }
        }
    }
}

pub fn debug_homing(world: &World) {
  for name in ["wormhole", "wormhole-jetman", "wormhole-warp"] {
    println!("== {name}");
    let gs = record(world, name);
    let g = &gs[0];
    let at = g[0].at;
    println!("at {:?}", at);
    for i in 0..3 { println!("rec {i}: n {} counter {} x {} xf {} y {} yf {} h {:#x} {} v {:#x} {} k {} flash {} facing {}", g[i].n, g[i].counter, g[i].x, g[i].xf, g[i].y, g[i].yf, g[i].h, g[i].hl, g[i].v, g[i].vd, g[i].k, g[i].flash, g[i].facing); }
    let g = &g[1..];
    let mut s = from_g(&g[0]);
    for i in 1..10 {
        tick(&mut s, 65536, at);
        let real = from_g(&g[i]);
        if i < 5 { println!("call {}: real x {:.3} y {:.3} h {} {} v {} {} | model x {:.3} y {:.3} h {} {} v {} {}", g[i].n, real.x as f64 / 16777216.0, real.y as f64 / 16777216.0, real.hm >> 16, real.hl, real.vm >> 16, real.vd, s.x as f64 / 16777216.0, s.y as f64 / 16777216.0, s.hm >> 16, s.hl, s.vm >> 16, s.vd); }
    }
  }
}

pub fn debug_first_mismatch(world: &World) {
    for name in ["wormhole", "wormhole-jetman", "wormhole-warp"] {
        let gs = record(world, name);
        let g = &gs[0][1..];
        let at = g[0].at;
        let mut s = from_g(&g[0]);
        for i in 1..g.len() {
            tick(&mut s, 65536, at);
            let real = from_g(&g[i]);
            if (s.x, s.y, s.hm >> 16, s.vm >> 16, s.hl, s.vd) != (real.x, real.y, real.hm >> 16, real.vm >> 16, real.hl, real.vd) {
                println!("{name}: first mismatch at record {} (call {}): real x {:.3} y {:.3} h {} {} v {} {}; model x {:.3} y {:.3} h {} {} v {} {}; at {:?}", i, g[i].n, real.x as f64 / 16777216.0, real.y as f64 / 16777216.0, real.hm >> 16, real.hl, real.vm >> 16, real.vd, s.x as f64 / 16777216.0, s.y as f64 / 16777216.0, s.hm >> 16, s.hl, s.vm >> 16, s.vd, g[i].at);
                break;
            }
            if i == g.len() - 1 { println!("{name}: exact for all {} records", g.len()); }
        }
    }
}

pub fn groups(world: &World) {
    for name in ["wormhole", "wormhole-warp"] {
        let gs = record(world, name);
        let g = &gs[0];
        let mut lives: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
        let mut starts: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
        for gi in 0..2 {
            let mut n = 0;
            for (i, w) in g.iter().enumerate() {
                if w.grp[gi] {
                    if n == 0 {
                        starts[gi].push(i + 1);
                    }
                    n += 1;
                } else if n > 0 {
                    lives[gi].push(n);
                    n = 0;
                }
            }
        }
        let uniq = |v: &Vec<usize>| v.iter().cloned().collect::<std::collections::BTreeSet<_>>();
        println!("{name}: group A starts {} (first at {:?}), lives {:?}; group B starts {} (first at {:?}), lives {:?}", starts[0].len(), starts[0].first(), uniq(&lives[0]), starts[1].len(), starts[1].first(), uniq(&lives[1]));
        // max displacement of a particle per call
        let mut mx = 0f64;
        let mut disp = Vec::new();
        for w in g.windows(2) {
            for i in 0..16 {
                if w[0].grp[i / 8] && w[1].grp[i / 8] {
                    let d = (f64::from(w[1].parts[i].0) - f64::from(w[0].parts[i].0)).hypot(f64::from(w[1].parts[i].1) - f64::from(w[0].parts[i].1));
                    disp.push(d);
                    mx = mx.max(d);
                }
            }
        }
        println!("   particle displacement per step (px): median {:.1} p90 {:.1} max {:.1}", pct(&disp, 0.5), pct(&disp, 0.9), mx);
    }
}

// ---- warp level routine: the mesh helpers follow the pod once per 60 Hz frame ----
use solar_jetman::game::gameplay::objects::Object;

pub fn warp_levels(world: &World) {
    for name in ["warp-level", "warp-level-37", "warp-level-40"] {
        let (mut g, r) = replay_game(world, name);
        // per pass: pod position after the pass, helper (slot 23, 24) bytes
        let mut pod: Vec<(u16, u16)> = Vec::new(); // after each step
        let mut helper_orig: Vec<[u8; 4]> = Vec::new(); // after each step (second pass)
        for (i, &b) in r.steps.iter().enumerate() {
            setups(&mut g, &r, i + 1);
            if g.transition().is_some() {
                break;
            }
            if g.paused() || !g.next_is_a_pass() {
                continue;
            }
            g.step(world, b);
            pod.push((g.pod.body.x, g.pod.body.y));
            helper_orig.push(g.helper_bytes());
        }
        if pod.len() < 20 {
            println!("{name}: too short");
            continue;
        }
        // the helpers: slot 23 homes with (2, 0x50), slot 24 with (3, 0x40); start at (0, 16). Level flags unknown here:
        // follow both as homing helpers and compare only between the two samplings.
        let run = |sample: &dyn Fn(usize) -> (u16, u16)| -> Vec<[u16; 4]> {
            let mut a = Object { x: 0, y: 16, ..Object::default() };
            let mut b = Object { x: 0, y: 16, ..Object::default() };
            let mut out = Vec::new();
            for j in 0..pod.len() * 2 {
                let (px, py) = sample(j);
                a.home_x(px, 2, 0x50);
                a.home_y(py, 2, 0x50);
                b.home_x(px, 3, 0x40);
                b.home_y(py, 3, 0x40);
                out.push([a.x, a.y, b.x, b.y]);
            }
            out
        };
        // original: both frames of step n see the pod after step n
        let orig = run(&|j| pod[j / 2]);
        // Scaled: frame j happens at time j/2 steps (j even = integer step n = pod sample n); the pod is interpolated
        // between the samples of steps n-1 (time n-1... ) -> position at time t = lerp(pod[floor(t)-1], pod[floor(t)])
        let lerp = |t2: usize| -> (u16, u16) {
            // t2 in half steps; pod[n] is the pod at the end of step n+1, i.e. at time n+1
            let n = t2 / 2; // frame in step n+1 (0-based)
            let frac = (t2 % 2) as i32; // 0 or 1 (half step)
            let a = if n == 0 { pod[0] } else { pod[n - 1] };
            let b = pod[n.min(pod.len() - 1)];
            // frame at the start of step n+1 sees the pod at time n+... use the end-of-step position progress: frac/2 of the way
            let l = |p: u16, q: u16| -> u16 { ((i32::from(p) * (2 - frac) + i32::from(q) * frac) / 2) as u16 };
            (l(a.0, b.0), l(a.1, b.1))
        };
        let scaled = run(&lerp);
        let mut dmax = [0u32; 4];
        let mut off_diff = 0usize;
        for (a, b) in orig.iter().zip(&scaled) {
            for k in 0..4 {
                let d = u32::from(a[k].wrapping_sub(b[k]) as i16 as i32 as u32);
                let d = (a[k] as i32 - b[k] as i32).unsigned_abs().max(d.min(0));
                dmax[k] = dmax[k].max(d);
            }
            if (a[0] & 7, a[1] & 7) != (b[0] & 7, b[1] & 7) {
                off_diff += 1;
            }
        }
        println!("{name}: {} steps; helper position difference between the pod seen at the engine's frame and the interpolated pod at the frame time: max (a.x,a.y,b.x,b.y) {:?} px; frames with a different mesh offset (x&7,y&7) {:.1}%", pod.len(), dmax, 100.0 * off_diff as f64 / orig.len() as f64);
        let _ = helper_orig;
    }
}
