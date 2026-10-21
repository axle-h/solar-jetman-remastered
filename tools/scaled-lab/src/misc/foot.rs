//! Area 1: Jetman on foot and in the air: recorder of the real engine.
use super::common::*;
use crate::contact::{Scen, scenarios};
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::{Gameplay, Seed};
use solar_jetman::game::level::World;

#[derive(Clone, Copy, Debug, Default)]
pub struct R {
    pub held: u8,
    pub state: u8,
    pub x: u16,
    pub xf: u8,
    pub y: u16,
    pub yf: u8,
    pub h: u16,
    pub hl: bool,
    pub v: u16,
    pub vd: bool,
    pub facing: u8,
    pub timer: u8,
    pub counter: u8,
    pub hh: u8,
    pub surface: u8,
    pub fuel: u16,
    pub inv: u8,
    pub hit_pause: u8,
    pub fire: bool,
    pub slope: bool,
}

pub fn snap(g: &Gameplay, held: u8) -> R {
    let p = &g.pod;
    let b = &p.body;
    R {
        held,
        state: p.state,
        x: b.x,
        xf: b.x_fraction,
        y: b.y,
        yf: b.y_fraction,
        h: b.h_speed,
        hl: b.h_left,
        v: b.v_speed,
        vd: b.v_down,
        facing: p.facing,
        timer: p.timer,
        counter: p.counter,
        hh: p.half_height,
        surface: p.surface,
        fuel: p.fuel,
        inv: p.invulnerable,
        hit_pause: p.hit_pause,
        fire: p.fire,
        slope: false,
    }
}

/// A run: starts a pod at a flat-floor site, lets it reach the floor, empties the tank on contact
/// (explosion, Jetman emerges, falls, lands, walks), random inputs after. Records every step.
pub struct Run {
    pub level: usize,
    pub pre: R,
    pub recs: Vec<R>,
    /// step index at which fuel was poked to 0
    pub kill: usize,
}

pub fn run_one(world: &World, s: &Scen, seed: u64, n: usize) -> Run {
    let sd = Seed { rng: solar_jetman::game::gameplay::rng::Rng { out: 0x78, high: 0xC9, low: 0x03 }, ..Default::default() };
    let mut g = Gameplay::new(world, s.level, sd);
    g.pod.equipment.owned = 0;
    let (i, _) = (s.init, 0);
    g.warp(i.x, i.y, 0, 0);
    let mut rng = Xs(seed * 7919 + 13);
    let mut recs = Vec::new();
    let mut cur = 0u8;
    let mut kill = usize::MAX;
    let mut touched = 0;
    let mut hold_left = 0;
    let pre = snap(&g, 0);
    for k in 0..n {
        if g.next_is_a_pass() == false {
            break;
        }
        if hold_left == 0 {
            hold_left = 2 + rng.below(40) as usize;
            cur = 0;
            if g.pod.state == 3 || g.pod.state == 2 {
                match rng.below(5) {
                    0 | 1 => cur |= Buttons::LEFT,
                    2 | 3 => cur |= Buttons::RIGHT,
                    _ => {}
                }
                if g.pod.state == 2 && rng.below(100) < 35 {
                    cur |= Buttons::B;
                }
                if g.pod.state == 3 && rng.below(100) < 6 {
                    cur |= Buttons::B;
                }
                if rng.below(100) < 15 {
                    cur |= Buttons::A;
                }
                if rng.below(100) < 6 {
                    cur |= Buttons::DOWN;
                }
            }
        } else if cur & Buttons::A != 0 && rng.below(3) == 0 {
            cur &= !Buttons::A;
        }
        hold_left -= 1;
        // kill the pod once it has touched the floor for a while
        if g.pod.state == 1 {
            if g.pod.surface != 0 {
                touched += 1;
            }
            if touched >= 1 + (seed as usize % 3) && kill == usize::MAX {
                g.pod.fuel = 0;
                kill = k;
            }
            cur = 0;
        }
        if g.pod.state == 2 || g.pod.state == 3 {
            if g.pod.fuel < 0x4000 {
                g.pod.fuel = 0xE000;
            }
        }
        g.step(world, Buttons(cur));
        recs.push(snap(&g, cur));
    }
    Run { level: s.level, pre, recs, kill }
}

pub fn runs(world: &World, per_site: usize, n: usize) -> Vec<Run> {
    let scens: Vec<Scen> = scenarios(world).into_iter().filter(|s| s.class == "F").collect();
    let mut out = Vec::new();
    for s in &scens {
        for seed in 0..per_site {
            out.push(run_one(world, s, seed as u64 + 1, n));
        }
    }
    out
}

pub fn summary(world: &World) {
    let rs = runs(world, 3, 500);
    let mut by_state = [0usize; 12];
    let mut transitions = std::collections::BTreeMap::new();
    for r in &rs {
        let mut prev = 0u8;
        for rec in &r.recs {
            by_state[rec.state as usize] += 1;
            if rec.state != prev {
                *transitions.entry((prev, rec.state)).or_insert(0usize) += 1;
            }
            prev = rec.state;
        }
    }
    println!("runs {} steps by state {:?}", rs.len(), by_state);
    println!("transitions {:?}", transitions);
}

pub fn stick_stats(runs: &[Run]) {
    use std::collections::BTreeMap;
    let mut dy: BTreeMap<i32, usize> = BTreeMap::new();
    let mut walkoff = 0;
    let mut takeoff = 0;
    let mut walkoff_h = vec![];
    let mut land: BTreeMap<(bool, u16), usize> = BTreeMap::new();
    let mut yfrac_change = 0usize;
    let mut n3 = 0usize;
    for r in runs {
        for w in r.recs.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if a.state == 3 && b.state == 3 {
                n3 += 1;
                *dy.entry(i32::from(b.y) - i32::from(a.y)).or_default() += 1;
                if b.yf != a.yf {
                    yfrac_change += 1;
                }
            }
            if a.state == 3 && b.state == 2 {
                if b.held & Buttons::B != 0 {
                    takeoff += 1;
                } else {
                    walkoff += 1;
                    walkoff_h.push(f64::from(a.h) / 256.0);
                }
            }
            if a.state == 2 && b.state == 3 {
                // landing: speed before the landing step (a.v is after gravity of the previous step)
                *land.entry((a.vd, a.v >> 8)).or_default() += 1;
            }
        }
    }
    println!("state-3 steps {n3}: dy histogram {:?}, y fraction changed in {yfrac_change}", dy);
    println!("3->2: take-off by B {takeoff}, walked off / slope {walkoff} (speed px/step at that moment: mean {:.2}, p90 {:.2})", mean(&walkoff_h), pct(&walkoff_h, 0.9));
    println!("2->3 landings by whole vertical speed before the landing step (down?, whole): {:?}", land);
}
