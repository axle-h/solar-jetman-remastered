// Cost per logic step (two passes) of the real engine on recorded replays (release build).
use solar_jetman::game::gameplay::{Gameplay, Seed, rng::Rng};
use std::time::Instant;

fn main() {
    let world = exp::world();
    let mut names: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().trim_end_matches(".replay").to_string()).collect();
    names.sort();
    let mut all: Vec<f64> = vec![];
    println!("{:24} {:>6} {:>9} {:>9} {:>9}", "replay", "steps", "mean_us", "p99_us", "max_us");
    for n in &names {
        let text = std::fs::read_to_string(format!("{}/../../tools/mesen/replays/{n}.replay", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = solar_jetman::tools::replay::parse_replay(&text).unwrap();
        let mut best: Vec<f64> = vec![];
        for _rep in 0..3 {
            let seed = Seed { rng: Rng { out: 0x78, high: 0xC9, low: 0x03 }, equipment: 0, weapon: r.weapon, device: r.device,
                bullet_upgrade: r.bullet_upgrade, weapon_gauge: r.weapon_gauge, planet_flags: r.flags, ..Default::default() };
            let level = r.level;
            let mut g = Gameplay::new(&world, level, seed);
            g.pod.equipment.owned = r.equipment;
            if level != 26 && level < 13 { (g.objects.slots[1].flags_x, g.objects.slots[1].flags_y) = (1, 1); }
            let mut t: Vec<f64> = vec![];
            for (i, &b) in r.steps.iter().enumerate() {
                let step = i + 1;
                if r.keep_fuel.is_some_and(|from| step >= from) { g.pod.fuel = 0xE000; }
                if let Some(&fuel) = r.fuel_pokes.get(&step) { g.pod.fuel = fuel; }
                if let Some(&[x, y, cx, cy]) = r.warps.get(&step) { g.warp(x, y, cx, cy); }
                for &(slot, state, x, y, frame, index) in r.objs.get(&step).into_iter().flatten() { g.set_object(slot, state, x, y, frame, index); }
                if let Some(&spin) = r.spins.get(&step) { g.tow.spin = spin; }
                for &(index, kind, x, y) in r.items.get(&step).into_iter().flatten() { g.set_item(index, kind, x, y); }
                if g.transition().is_some() { break; }
                if g.paused() { continue; }
                if !g.next_is_a_pass() { continue; }
                let t0 = Instant::now();
                g.step(&world, b);
                t.push(t0.elapsed().as_secs_f64() * 1e6);
            }
            if best.is_empty() || t.iter().sum::<f64>() < best.iter().sum::<f64>() { best = t; }
        }
        if best.is_empty() { continue; }
        let mut s = best.clone(); s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = s.iter().sum::<f64>() / s.len() as f64;
        println!("{:24} {:6} {:9.2} {:9.2} {:9.2}", n, s.len(), mean, s[s.len() * 99 / 100], s[s.len() - 1]);
        all.extend(best);
    }
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| all[((all.len() as f64 - 1.0) * p) as usize];
    println!("ALL steps {} mean {:.2} p50 {:.2} p99 {:.2} p99.9 {:.2} max {:.2} us",
        all.len(), all.iter().sum::<f64>() / all.len() as f64, q(0.5), q(0.99), q(0.999), all[all.len() - 1]);
}
