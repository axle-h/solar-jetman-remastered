//! Camera experiment: record the real engine's camera/pod per pass, then model candidates.
use solar_jetman::game::gameplay::{Gameplay, Seed, input::Buttons, rng::Rng, pod};
use solar_jetman::game::level::World;
use solar_jetman::tools::replay::Replay;

#[derive(Clone, Debug)]
pub struct Pass {
    pub a: bool,
    pub counter: u32,
    pub cam: (i32, i32),
    pub pod: (i64, i64), // Q8 world px
    pub state: u8,
    pub in_sky: bool,
    pub list_size: u16,
    pub towing: bool,
    pub warp: bool,
    pub delta: (i8, i8),
}

#[derive(Clone, Debug, Default)]
pub struct Run {
    pub name: String,
    pub limit: (i32, i32),
    pub passes: Vec<Pass>,
    /// index into passes where a discontinuity begins (before that pass)
    pub breaks: Vec<usize>,
}

fn apply_setups(game: &mut Gameplay, replay: &Replay, step: usize) -> bool {
    let mut warped = false;
    if replay.keep_fuel.is_some_and(|from| step >= from) { game.pod.fuel = 0xE000; }
    if let Some(&fuel) = replay.fuel_pokes.get(&step) { game.pod.fuel = fuel; }
    if let Some(&[x, y, cx, cy]) = replay.warps.get(&step) { game.warp(x, y, cx, cy); warped = true; }
    for &(slot, state, x, y, frame, index) in replay.objs.get(&step).into_iter().flatten() {
        game.set_object(slot, state, x, y, frame, index);
    }
    if let Some(&spin) = replay.spins.get(&step) { game.tow.spin = spin; }
    for &(index, kind, x, y) in replay.items.get(&step).into_iter().flatten() { game.set_item(index, kind, x, y); }
    warped
}

pub fn run_replay(world: &World, name: &str, replay: &Replay) -> Run {
    let seed = Seed {
        rng: Rng { out: 0x78, high: 0xC9, low: 0x03 },
        equipment: 0, weapon: replay.weapon, device: replay.device, bullet_upgrade: replay.bullet_upgrade,
        weapon_gauge: replay.weapon_gauge, planet_flags: replay.flags,
        ..Default::default()
    };
    let level = replay.level;
    let mut game = Gameplay::new(world, level, seed);
    game.pod.equipment.owned = replay.equipment;
    if level != 26 && level < 13 {
        (game.objects.slots[1].flags_x, game.objects.slots[1].flags_y) = (1, 1);
    }
    let mut run = Run { name: name.into(), limit: { let l = world.levels[level].camera_limit(); (l.x as i32, l.y as i32) }, ..Default::default() };
    let mut warped = apply_setups(&mut game, replay, 1);
    let n = replay.steps.len();
    for index in 0..n {
        let step = index + 1;
        if step > 1 { warped = apply_setups(&mut game, replay, step); }
        let buttons: Buttons = replay.steps[index];
        for _ in 0..2 {
            if game.transition().is_some() { return run; }
            if game.paused() { break; }
            let a = game.next_is_a_pass();
            let counter = game.counter();
            game.pass(world, buttons);
            let towing = game.tow.holding();
            if warped && a { run.breaks.push(run.passes.len()); warped = false; }
            run.passes.push(Pass {
                a, counter,
                cam: (game.camera.position.x as i32, game.camera.position.y as i32),
                pod: (((game.pod.body.x as i64) << 8) | game.pod.body.x_fraction as i64, ((game.pod.body.y as i64) << 8) | game.pod.body.y_fraction as i64),
                state: game.pod.state, in_sky: game.pod.in_sky, list_size: game.pod.list_size, towing,
                warp: world.levels[level].is_warp_level(), delta: game.camera.delta,
            });
            if a && game.next_is_a_pass() { /* gate state quirk */ }
        }
    }
    run
}

/// Continuous segments of whole steps: Vec of (start pass index of an A pass, number of steps)
pub fn segments(run: &Run) -> Vec<(usize, usize)> {
    let p = &run.passes;
    let mut out = vec![];
    let mut i = 0;
    let ok_state = |s: u8| matches!(s, pod::STATE_POD | pod::STATE_JETMAN_FLYING | pod::STATE_JETMAN_WALKING | pod::STATE_FINAL);
    while i + 1 < p.len() {
        if !p[i].a { i += 1; continue; }
        // start a segment at A pass i
        let mut k = 0;
        let from_warp = run.breaks.contains(&i);
        let mut j = i;
        while j + 1 < p.len() && p[j].a && !p[j + 1].a && ok_state(p[j].state) && ok_state(p[j + 1].state) {
            if k > 0 {
                let dx = (p[j].pod.0 - p[j - 2].pod.0).abs() >> 8;
                let dy = (p[j].pod.1 - p[j - 2].pod.1).abs() >> 8;
                let cx = (p[j + 1].cam.0 - p[j - 1].cam.0).abs();
                let cy = (p[j + 1].cam.1 - p[j - 1].cam.1).abs();
                if dx > 12 || dy > 12 || cx > 16 || cy > 16 || run.breaks.contains(&j) { break; }
            }
            k += 1;
            j += 2;
        }
        if !from_warp && k >= 20 && (p[i + 1].cam.0 as i32) <= run.limit.0 && (p[i + 1].cam.1 as i32) <= run.limit.1 { out.push((i, k)); }
        i = j.max(i + 2);
    }
    out
}

// ---------------------------------------------------------------- model
pub const ONE: i64 = 1 << 16;

#[derive(Clone)]
pub struct Seg {
    pub name: String,
    pub limit: (i32, i32),
    pub p: Vec<(i64, i64)>,      // Q8 per step
    pub cam_a: Vec<(i32, i32)>,
    pub cam_b: Vec<(i32, i32)>,
    pub sky: Vec<bool>,
    pub acall: Vec<bool>,
    pub ctr0: u32,
}

pub fn to_segs(run: &Run) -> Vec<Seg> {
    segments(run).into_iter().map(|(i, k)| {
        let ps = &run.passes;
        Seg {
            name: run.name.clone(), limit: run.limit,
            p: (0..k).map(|s| ps[i + 2 * s].pod).collect(),
            cam_a: (0..k).map(|s| ps[i + 2 * s].cam).collect(),
            cam_b: (0..k).map(|s| ps[i + 2 * s + 1].cam).collect(),
            sky: (0..k).map(|s| ps[i + 2 * s].in_sky).collect(),
            acall: (0..k).map(|s| { let q = &ps[i + 2 * s]; !q.warp && (q.list_size == 0x10 || (q.list_size < 0x20 && !q.towing)) }).collect(),
            ctr0: ps[i].counter,
        }
    }).collect()
}

/// Pod position (Q16 world px) at time u (units of 1/60 s), from per-step positions.
pub fn pod_at(seg: &Seg, u: f64) -> (i64, i64) {
    let s = u / 2.0;
    let k = (s.floor() as usize).min(seg.p.len() - 2);
    let fr = s - k as f64;
    let l = |a: i64, b: i64| ((a as f64 + (b - a) as f64 * fr) * 256.0).round() as i64;
    (l(seg.p[k].0, seg.p[k + 1].0), l(seg.p[k].1, seg.p[k + 1].1))
}

/// Original frames: (u, pod Q16 as displayed, cam)
pub fn ref_frames(seg: &Seg) -> Vec<(i64, (i64, i64), (i32, i32))> {
    let mut v = vec![];
    for k in 0..seg.p.len() {
        if k >= 1 {
            let m = |a: i64, b: i64| (a + b) * 128; // (a+b)/2 in Q8 -> Q16
            v.push((2 * k as i64 - 1, (m(seg.p[k - 1].0, seg.p[k].0), m(seg.p[k - 1].1, seg.p[k].1)), seg.cam_a[k]));
        }
        v.push((2 * k as i64, (seg.p[k].0 * 256, seg.p[k].1 * 256), seg.cam_b[k]));
    }
    v
}

#[derive(Clone, Copy, Debug)]
pub struct Cand {
    pub halves: bool,       // two half-moves per tick
    pub whole: bool,        // camera state in whole pixels (carry accumulator)
    pub cap: bool,          // 8 px per 1/60 s cap
    pub bias: i64,          // Q16 px: shift of the band edges outward (+) 
    pub lead: f64,          // pod look-ahead in steps (0 none)
    pub qpod: bool,         // use whole-pixel pod positions for the camera (floor)
}

#[derive(Clone, Copy, Debug)]
pub struct Tick { pub u: f64, pub cam: (i64, i64), pub pod: (i64, i64) }

fn clampv(v: i64, cap: i64, on: bool) -> i64 { if on { v.clamp(-cap, cap) } else { v } }

/// Run a candidate over a segment at rate r, phase phi (fraction of a tick).
pub fn run_cand(seg: &Seg, r: f64, phi: f64, c: Cand) -> Vec<Tick> {
    let du = 60.0 / r;
    let nsteps = seg.p.len();
    let end = (2 * (nsteps - 1)) as f64;
    let mut cam = ((seg.cam_b[0].0 as i64) * ONE, (seg.cam_b[0].1 as i64) * ONE);
    let mut pend = (0i64, 0i64);
    let la = |u: f64| pod_at(seg, (u + c.lead * 2.0).min(end));
    let mut prevp = { let q0 = la(0.0); if c.qpod { ((q0.0 >> 16) << 16, (q0.1 >> 16) << 16) } else { q0 } };
    let mut out = vec![Tick { u: 0.0, cam, pod: pod_at(seg, 0.0) }];
    let capq = ((8.0 * 30.0 / r).ceil() as i64) * ONE; // per half-move: 8 px at R=30
    let lim = ((seg.limit.0 as i64) * ONE, (seg.limit.1 as i64) * ONE);
    let mut j = 0;
    loop {
        let u = (phi + j as f64) * du;
        j += 1;
        if u <= 0.0 { continue; }
        if u > end { break; }
        let q0 = la(u);
        let q = if c.qpod { ((q0.0 >> 16) << 16, (q0.1 >> 16) << 16) } else { q0 };
        let d = (q.0 - prevp.0, q.1 - prevp.1);
        prevp = q;
        let h = (d.0 / 2, d.1 / 2);
        let two = [h, (d.0 - h.0, d.1 - h.1)];
        let one = [d];
        let steps: &[(i64, i64)] = if c.halves { &two } else { &one };
        for &(dx, dy) in steps {
            let dx = clampv(dx, capq, c.cap);
            let dy = clampv(dy, capq, c.cap);
            let sx = q.0 - cam.0;
            let sy = q.1 - cam.1;
            let mut mx = 0; let mut my = 0;
            if (dx > 0 && sx >= 144 * ONE + c.bias) || (dx < 0 && sx < 112 * ONE - c.bias) { mx = dx; }
            if (dy > 0 && sy >= 96 * ONE + c.bias) || (dy < 0 && sy < 64 * ONE - c.bias) { my = dy; }
            if c.whole {
                pend.0 += mx; pend.1 += my;
                let wx = pend.0 >> 16; let wy = pend.1 >> 16; // floor
                // apply whole pixels; keep remainder
                cam.0 += wx * ONE; cam.1 += wy * ONE;
                pend.0 -= wx * ONE; pend.1 -= wy * ONE;
            } else { cam.0 += mx; cam.1 += my; }
            cam.0 = cam.0.clamp(0, lim.0); cam.1 = cam.1.clamp(0, lim.1);
        }
        out.push(Tick { u, cam, pod: pod_at(seg, u) });
    }
    out
}

#[derive(Clone, Copy, Default, Debug)]
pub struct St { pub n: u64, pub sum: f64, pub sq: f64, pub max: f64, pub ge1: u64, pub ge2: u64 }
impl St {
    pub fn add(&mut self, e: f64) {
        self.n += 1; self.sum += e; self.sq += e * e; self.max = self.max.max(e.abs());
        if e.abs() >= 1.0 { self.ge1 += 1 } if e.abs() >= 2.0 { self.ge2 += 1 }
    }
    pub fn rms(&self) -> f64 { (self.sq / self.n.max(1) as f64).sqrt() }
    pub fn mean(&self) -> f64 { self.sum / self.n.max(1) as f64 }
    pub fn merge(&mut self, o: &St) { self.n += o.n; self.sum += o.sum; self.sq += o.sq; self.max = self.max.max(o.max); self.ge1 += o.ge1; self.ge2 += o.ge2; }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Errs { pub pod_x: St, pub pod_y: St, pub cam_x: St, pub cam_y: St, pub dh_x: St, pub dh_y: St, pub di_x: St, pub di_y: St }
impl Errs {
    pub fn merge(&mut self, o: &Errs) {
        self.pod_x.merge(&o.pod_x); self.pod_y.merge(&o.pod_y); self.cam_x.merge(&o.cam_x); self.cam_y.merge(&o.cam_y);
        self.dh_x.merge(&o.dh_x); self.dh_y.merge(&o.dh_y); self.di_x.merge(&o.di_x); self.di_y.merge(&o.di_y);
    }
}

fn fl(v: i64) -> i64 { v >> 16 }

pub fn evaluate(seg: &Seg, ticks: &[Tick]) -> Errs {
    let refs = ref_frames(seg);
    let mut e = Errs::default();
    let scr = |pod: (i64, i64), cam: (i64, i64)| (pod.0 - cam.0, pod.1 - cam.1);
    let lerp = |a: i64, b: i64, f: f64| a as f64 + (b - a) as f64 * f;
    // continuous, at ticks
    for t in ticks.iter().skip(1) {
        let i = t.u.floor() as usize;
        if i + 1 >= refs.len() || i == 0 { continue; }
        let f = t.u - i as f64;
        let (r0, r1) = (&refs[i], &refs[i + 1]);
        let s0 = scr(r0.1, (r0.2 .0 as i64 * ONE, r0.2 .1 as i64 * ONE));
        let s1 = scr(r1.1, (r1.2 .0 as i64 * ONE, r1.2 .1 as i64 * ONE));
        let sref = (lerp(s0.0, s1.0, f), lerp(s0.1, s1.1, f));
        let cref = (lerp(r0.2 .0 as i64, r1.2 .0 as i64, f), lerp(r0.2 .1 as i64, r1.2 .1 as i64, f));
        let s = scr(t.pod, t.cam);
        e.pod_x.add((s.0 as f64 - sref.0) / ONE as f64);
        e.pod_y.add((s.1 as f64 - sref.1) / ONE as f64);
        e.cam_x.add(t.cam.0 as f64 / ONE as f64 - cref.0);
        e.cam_y.add(t.cam.1 as f64 / ONE as f64 - cref.1);
    }
    // displayed (whole pixel) at original frame instants
    let mut j = 0;
    for r in refs.iter().skip(1) {
        let u = r.0 as f64;
        while j + 1 < ticks.len() && ticks[j + 1].u <= u { j += 1; }
        if j + 1 >= ticks.len() { break; }
        let sr = scr(r.1, (r.2 .0 as i64 * ONE, r.2 .1 as i64 * ONE));
        let sref = (fl(sr.0), fl(sr.1));
        let h = scr(ticks[j].pod, ticks[j].cam);
        e.dh_x.add((fl(h.0) - sref.0) as f64); e.dh_y.add((fl(h.1) - sref.1) as f64);
        let (a, b) = (&ticks[j], &ticks[j + 1]);
        let f = (u - a.u) / (b.u - a.u);
        let ip = |x: i64, y: i64| (x as f64 + (y - x) as f64 * f).round() as i64;
        let pod = (ip(a.pod.0, b.pod.0), ip(a.pod.1, b.pod.1));
        let cam = (ip(a.cam.0, b.cam.0), ip(a.cam.1, b.cam.1));
        let h = scr(pod, cam);
        e.di_x.add((fl(h.0) - sref.0) as f64); e.di_y.add((fl(h.1) - sref.1) as f64);
    }
    e
}

pub const RATES: [f64; 10] = [30.0, 60.0, 75.0, 90.0, 100.0, 120.0, 144.0, 165.0, 200.0, 137.0];

/// Synthetic fast-flight replays: thrust held, random rotation runs, fuel kept full.
pub fn synthetic(world: &World) -> Vec<Run> {
    let mut out = vec![];
    let mut seed: u64 = 12345;
    let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    for (li, &level) in [0usize, 1, 3, 5, 6, 9, 10, 11].iter().enumerate() {
        for rep in 0..3 {
            let mut steps = vec![];
            while steps.len() < 2500 {
                let run = 5 + (rnd() % 30) as usize;
                let dir = match rnd() % 4 { 0 => 0, 1 => Buttons::LEFT, 2 => Buttons::RIGHT, _ => if rnd() % 2 == 0 { Buttons::LEFT } else { Buttons::RIGHT } };
                let thrust = if rnd() % 5 == 0 { 0 } else { Buttons::B };
                for _ in 0..run { steps.push(Buttons(dir | thrust)); }
            }
            let r = Replay { steps, equipment: [0x80u8, 0xC0, 0xFF][rep], level, keep_fuel: Some(1), ..Default::default() };
            out.push(run_replay(world, &format!("synth-l{level}-{rep}-{li}"), &r));
        }
    }
    out
}

// ---------------------------------------------------------------- stars
use solar_jetman::game::gameplay::stars::Stars;

/// Original star model on the recorded passes: state after every pass u (u = 2k-1 for A of step k, 2k for B).
pub fn orig_stars(seg: &Seg) -> (Vec<Stars>, [u32; 8]) {
    let mut st = Stars::new(0);
    let mut out = vec![st.clone()];
    let mut spawns = [0u32; 8];
    let mut prev_dead = [true; 8];
    for k in 1..seg.p.len() {
        let ctr = seg.ctr0.wrapping_add(2 * k as u32);
        let ma = (seg.cam_a[k].0 - seg.cam_b[k - 1].0, seg.cam_a[k].1 - seg.cam_b[k - 1].1);
        let mb = (seg.cam_b[k].0 - seg.cam_a[k].0, seg.cam_b[k].1 - seg.cam_a[k].1);
        if seg.acall[k] { st.call(ma, seg.sky[k], ctr, false); }
        out.push(st.clone());
        st.call(mb, seg.sky[k], ctr.wrapping_add(2), false);
        out.push(st.clone());
        for i in 0..8 { if prev_dead[i] && !st.stars[i].dead { spawns[i] += 1; } prev_dead[i] = st.stars[i].dead; }
    }
    // out index: [0]=state at u=0; then for k>=1: [2k-1] after A (u=2k-1), [2k] after B (u=2k)
    (out, spawns)
}

/// Scaled star model: per tick the camera's whole-pixel move drives the star job; spawn on crossed frames.
pub fn scaled_stars(seg: &Seg, ticks: &[Tick], dummy_all_phases: bool) -> Vec<Stars> {
    let mut st = Stars::new(0);
    let mut out = vec![st.clone()];
    let mut prev_frame: i64 = 1;
    for w in ticks.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let k = (((b.u.floor() as usize) + 2) / 2).min(seg.p.len() - 1);
        let sky = seg.sky[k];
        let m = (((b.cam.0 >> 16) - (a.cam.0 >> 16)) as i32, ((b.cam.1 >> 16) - (a.cam.1 >> 16)) as i32);
        // move first (a star spawned by this tick's call is not moved by it, as in the original)
        st.call(m, sky, 127 + 128 * 1000 - 0, false); // counter whose low 7 bits (127) match no phase
        // frames crossed: pass index F = frame number; spawn test as the original's call counters
        let now = b.u.floor() as i64 + 2;
        let mut f = prev_frame + 1;
        while f <= now {
            if f >= 2 {
                let c = if f % 2 == 0 { f } else { f + 1 };
                let c = if dummy_all_phases { f } else { c };
                st.call((0, 0), sky, seg.ctr0.wrapping_add(c as u32), false);
            }
            f += 1;
        }
        prev_frame = now;
        out.push(st.clone());
    }
    out
}
