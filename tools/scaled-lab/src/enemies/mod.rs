//! Experiment `enemies`: object traces from the real engine and models of the scaled rules.
pub mod ctl;
use solar_jetman::game::gameplay::Gameplay;
use solar_jetman::game::gameplay::objects::{Object, SLOTS};
use solar_jetman::game::level::World;

#[derive(Clone)]
pub struct Frame {
    pub slots: [Object; SLOTS],
    pub px: u16, pub pxf: u8, pub py: u16, pub pyf: u8,
    pub rng: u8, pub r1: u8, pub r2: u8,
    pub counter: u32,
}
pub struct Run { pub name: String, pub level: usize, pub frames: Vec<Frame> }

pub fn capture(g: &Gameplay) -> Frame {
    Frame { slots: g.objects.slots, px: g.pod.body.x, pxf: g.pod.body.x_fraction, py: g.pod.body.y, pyf: g.pod.body.y_fraction, rng: g.rng.out, r1: g.rng.high, r2: g.rng.low, counter: g.counter() }
}

/// All replays driven through the real engine; frame i = state after step i (frame 0 = before step 1).
pub fn trace_all(world: &World) -> Vec<Run> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays");
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    names.sort();
    let mut out = Vec::new();
    for p in names {
        let text = std::fs::read_to_string(&p).unwrap();
        let Ok(r) = solar_jetman::tools::replay::parse_replay(&text) else { continue };
        if r.demo.is_some() || r.level >= 26 { continue; }
        let sd = crate::motion::rec::_seed(r.equipment, r.device);
        let mut g = Gameplay::new(world, r.level, sd);
        g.pod.equipment.owned = r.equipment;
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let mut frames = vec![capture(&g)];
        for (i, &b) in r.steps.iter().enumerate() {
            let step = i + 1;
            if r.keep_fuel.is_some_and(|f| step >= f) { g.pod.fuel = 0xE000; }
            if let Some(&f) = r.fuel_pokes.get(&step) { g.pod.fuel = f; }
            if let Some(&[x, y, cx, cy]) = r.warps.get(&step) { g.warp(x, y, cx, cy); }
            if !g.next_is_a_pass() { break; }
            let lvl = g.level();
            g.step(world, b);
            if g.level() != lvl { break; }
            frames.push(capture(&g));
        }
        out.push(Run { name, level: r.level, frames });
    }
    out
}
