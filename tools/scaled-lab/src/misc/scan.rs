//! Area 6: the item scan (L19), the placement scan (L20) and the green-orb spawner (L18) on the game-time clock.
use super::common::*;
use solar_jetman::game::gameplay::input::Buttons;
use solar_jetman::game::gameplay::objects::{ObjectTable, state};
use solar_jetman::game::level::{Point, World};

/// Camera position after each step of a replay (A and B pass views are the same here).
pub fn camera_track(world: &World, name: &str) -> (usize, Vec<(u16, u16)>) {
    let (mut g, r) = replay_game(world, name);
    let mut out = vec![(g.camera.position.x, g.camera.position.y)];
    for (i, &b) in r.steps.iter().enumerate() {
        setups(&mut g, &r, i + 1);
        if g.transition().is_some() {
            break;
        }
        if g.paused() || !g.next_is_a_pass() {
            continue;
        }
        g.step(world, b);
        out.push((g.camera.position.x, g.camera.position.y));
    }
    (r.level, out)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// the original: one A scan and one B scan per logic step
    Original,
    /// scans run once per crossed logic step of the game clock, camera at the tick
    Clock,
    /// the naive port: both scans on every physics tick
    PerTick,
}

fn cam_at(track: &[(u16, u16)], t: f64) -> Point {
    let i = (t.floor() as usize).min(track.len() - 1);
    let j = (i + 1).min(track.len() - 1);
    let w = t - i as f64;
    let l = |a: u16, b: u16| -> u16 { (f64::from(a) + (f64::from(b) - f64::from(a)) * w).round() as u16 };
    Point { x: l(track[i].0, track[j].0), y: l(track[i].1, track[j].1) }
}

/// Spawn events (time in steps, kind of scan 'i' item / 'p' placed, entry index) over the track.
pub fn run(world: &World, level: usize, track: &[(u16, u16)], mode: Mode, rate: u32) -> Vec<(f64, char, u8)> {
    let lv = &world.levels[level];
    let mut tbl = ObjectTable::new(&world.objects, lv);
    let c0 = Point { x: track[0].0, y: track[0].1 };
    tbl.start_scan(&world.objects, lv, c0);
    tbl.item_cursor = 0;
    let mut ev = Vec::new();
    let n = track.len() - 1;
    let mut call = |tbl: &mut ObjectTable, t: f64, cam: Point, ev: &mut Vec<(f64, char, u8)>| {
        let before: Vec<u8> = tbl.slots.iter().map(|s| s.state).collect();
        tbl.item_scan(&world.objects, cam);
        for (s, o) in tbl.slots.iter().enumerate() {
            if before[s] == 0 && o.state != 0 {
                ev.push((t, 'i', o.index));
            }
        }
        let before: Vec<u8> = tbl.slots.iter().map(|s| s.state).collect();
        tbl.despawn(lv, cam);
        let before2: Vec<u8> = tbl.slots.iter().map(|s| s.state).collect();
        let _ = (before, before2);
        let before: Vec<u8> = tbl.slots.iter().map(|s| s.state).collect();
        tbl.step_scan(&world.objects, lv, cam, true);
        for (s, o) in tbl.slots.iter().enumerate() {
            if before[s] == 0 && o.state != 0 {
                ev.push((t, 'p', o.index));
            }
        }
    };
    match mode {
        Mode::Original => {
            for k in 1..=n {
                let cam = Point { x: track[k].0, y: track[k].1 };
                call(&mut tbl, k as f64, cam, &mut ev);
            }
        }
        Mode::Clock | Mode::PerTick => {
            let f = f32q(rate);
            let mut gt: u64 = 0;
            let mut done_steps = 0u64;
            loop {
                gt += f;
                let t = gt as f64 / 4294967296.0;
                if t >= n as f64 {
                    break;
                }
                let cam = cam_at(track, t);
                if mode == Mode::PerTick {
                    call(&mut tbl, t, cam, &mut ev);
                } else {
                    let steps = gt >> 32;
                    while done_steps < steps {
                        done_steps += 1;
                        call(&mut tbl, t, cam, &mut ev);
                    }
                }
            }
        }
    }
    ev
}

pub fn report(world: &World) {
    let names = ["level1", "level2", "level3", "level5", "level9", "level10", "level13", "free-flight", "tow"];
    println!("scans on a replay's camera track: spawns of items ('i') and placed objects ('p'); timing of each entry's first spawn vs the original");
    println!("mode | R | spawns | matched first spawns | time diff ms: mean / p90 / max (positive = later than original) | spawns per second: max in any 1 s window");
    for name in names {
        let (level, track) = camera_track(world, name);
        if track.len() < 60 {
            continue;
        }
        let orig = run(world, level, &track, Mode::Original, 30);
        let first = |ev: &Vec<(f64, char, u8)>| {
            let mut m = std::collections::BTreeMap::new();
            for &(t, k, i) in ev {
                m.entry((k, i)).or_insert(t);
            }
            m
        };
        let fo = first(&orig);
        let maxwin = |ev: &Vec<(f64, char, u8)>| {
            let mut best = 0;
            for &(t, _, _) in ev {
                let c = ev.iter().filter(|e| e.0 >= t && e.0 < t + 30.0).count();
                best = best.max(c);
            }
            best
        };
        println!("== {name}: level {level}, {} steps; original: {} spawns ({} items, {} placed), max {} in a second", track.len() - 1, orig.len(), orig.iter().filter(|e| e.1 == 'i').count(), orig.iter().filter(|e| e.1 == 'p').count(), maxwin(&orig));
        for (mode, rates) in [(Mode::Clock, vec![60u32, 100, 144, 200]), (Mode::PerTick, vec![60, 144, 200])] {
            for r in rates {
                let ev = run(world, level, &track, mode, r);
                let fe = first(&ev);
                let mut d = Vec::new();
                for (k, t) in &fo {
                    if let Some(t2) = fe.get(k) {
                        d.push((t2 - t) * 1000.0 / 30.0);
                    }
                }
                let ab: Vec<f64> = d.iter().map(|x| x.abs()).collect();
                let big = ab.iter().filter(|&&x| x > 100.0).count();
                let med = pct(&ab, 0.5);
                println!("  {:?} R={r:3}: {} spawns, matched {}/{}, |diff| median {:.1} p90 {:.1} ms, >100 ms: {}, max {:.0} ms, max per second {}", mode, ev.len(), d.len(), fo.len(), med, pct(&ab, 0.9), big, absmax(&d), maxwin(&ev));
            }
        }
    }
}

pub fn orb(world: &World) {
    // real engine: planet level, pod resting (tank kept full), count green orbs created
    let mut g = crate::game(world, 1);
    g.pod.equipment.owned = 0;
    let steps = 60000;
    let y0 = g.pod.body.y;
    let mut created = Vec::new();
    let mut prev = [false; 3];
    let mut checks = 0;
    for n in 1..=steps {
        g.pod.fuel = 0xE000;
        g.pod.invulnerable = 255;
        let c = g.counter();
        g.step(world, Buttons(0));
        for (i, s) in (23..=25).enumerate() {
            let is = g.objects.slots[s].state == state::GREEN_ORB;
            if is && !prev[i] {
                created.push((n, c, i));
            }
            prev[i] = is;
        }
        // an A pass whose counter after the split is 1 mod 256 is a check
        let after = c.wrapping_add(1);
        if after & 0xFF == 1 {
            checks += 1;
        }
        // hover (no landing on the mothership)
        g.pod.body.v_speed = 0;
        g.pod.body.y = y0;
        if g.transition().is_some() {
            println!("transition at step {n}");
            break;
        }
    }
    let by_phase: Vec<u32> = created.iter().map(|&(_, c, _)| (c.wrapping_add(1) >> 8) & 3).collect();
    println!("orb spawner, real engine, {steps} steps ({:.0} s): {checks} checks (every 128 steps), {} orbs created; counter>>8 &3 of the creations {:?}", steps as f64 / 30.0, created.len(), by_phase);
    // clock model: the event count per game time
    for &r in &RATES {
        let f = f32q(r);
        let frames_total = (steps as u64) * 2;
        let mut gt = 0u64;
        let mut ev = 0u64;
        let mut last = 0u64;
        while (gt >> 31) < frames_total {
            gt += f;
            let fr = gt >> 31;
            // frames crossed; count the frames with (frame & 0xFF) == 1 after the split: frame f' = f+1 ... checks happen for even frames 255 mod 256
            for x in last + 1..=fr {
                if x % 256 == 0 {
                    ev += 1;
                }
            }
            last = fr;
        }
        println!("  R={r:3}: {ev} check events in {steps} steps ({} expected)", frames_total / 256);
    }
}

pub fn orb_debug(world: &World) {
    let mut g = crate::game(world, 1);
    let mut last = g.counter();
    for n in 1..=700 {
        g.pod.fuel = 0xE000;
        g.step(world, Buttons(0));
        let c = g.counter();
        if n < 3 || (n >= 126 && n <= 134) { println!("step {n}: counter {c} (+{}) a_pass {} transition {:?} paused {} state {}", c.wrapping_sub(last), g.next_is_a_pass(), g.transition().is_some(), g.paused(), g.pod.state); }
        last = c;
    }
}
