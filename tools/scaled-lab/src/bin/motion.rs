use exp::motion::integ::*;
use exp::motion::rec::*;
use solar_jetman::game::gameplay::Gameplay;
use solar_jetman::game::gameplay::input::Buttons;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn synth_inputs(seed: u64, n: usize, boost: bool) -> Vec<Buttons> {
    let mut r = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
    let mut v = Vec::new();
    while v.len() < n {
        let len = 3 + r.below(40) as usize;
        let mut b = 0u8;
        if r.below(100) < 70 {
            b |= Buttons::B;
        }
        match r.below(4) {
            0 => b |= Buttons::LEFT,
            1 => b |= Buttons::RIGHT,
            _ => {}
        }
        if boost && r.below(100) < 30 {
            b |= Buttons::SELECT;
        }
        for _ in 0..len {
            v.push(Buttons(b));
        }
    }
    v.truncate(n);
    v
}

fn collect(world: &solar_jetman::game::level::World) -> (Vec<Seg>, Vec<Seg>) {
    let mut from_replays = Vec::new();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays");
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    names.sort();
    for p in names {
        let text = std::fs::read_to_string(&p).unwrap();
        let Ok(r) = solar_jetman::tools::replay::parse_replay(&text) else { continue };
        if r.demo.is_some() || r.level >= 26 {
            continue;
        }
        let mut seed = exp::game(world, 0);
        let _ = &mut seed;
        let sd = _seed(r.equipment, r.device);
        let g = Gameplay::new(world, r.level, sd);
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let mut g = g;
        g.pod.equipment.owned = r.equipment;
        let (fp, wp, kf) = (r.fuel_pokes.clone(), r.warps.clone(), r.keep_fuel);
        let segs = run(world, g, &r.steps, &name, 12, &mut |g, step| {
            if kf.is_some_and(|f| step >= f) {
                g.pod.fuel = 0xE000;
            }
            if let Some(&f) = fp.get(&step) {
                g.pod.fuel = f;
            }
            if let Some(&[x, y, cx, cy]) = wp.get(&step) {
                g.warp(x, y, cx, cy);
            }
        });
        from_replays.extend(segs);
    }
    let mut synth = Vec::new();
    for level in 0..=12usize {
        for (ei, (eq, dev)) in [(0u8, 0u8), (0xC0, 0), (0x00, 9), (0x00, 10), (0xC0, 11)].into_iter().enumerate() {
            for seed in 0..10u64 {
                let sd = _seed(eq, dev);
                let mut g = Gameplay::new(world, level, sd);
                g.pod.equipment.owned = eq;
                let inputs = synth_inputs(seed * 100 + level as u64 * 7 + ei as u64, 700, eq != 0);
                let name = format!("syn-L{level}-e{ei}-s{seed}");
                synth.extend(run(world, g, &inputs, &name, 12, &mut |_, _| {}));
            }
        }
    }
    (from_replays, synth)
}


#[derive(Clone, Copy, Debug)]
struct Ref {
    x: i64,
    y: i64,
    vx: i64,
    vy: i64,
}
fn ref_of_s(s: &S) -> Ref {
    let (vx, vy) = s.signed();
    Ref { x: s.x, y: s.y, vx, vy: -(-vy) }
}
fn ref_of_rec(r: &Rec) -> Ref {
    ref_of_s(&S::from_rec(r))
}

fn tickin(seg: &Seg, o: &Opts, k: usize) -> TickIn {
    let n = seg.steps.len();
    let k = k.clamp(1, n);
    let r = &seg.steps[k - 1];
    let thrust = match o.order {
        Order::After | Order::BeforeNow => thrust_in(r),
        Order::BeforeLag => {
            if k >= 2 {
                thrust_in(&seg.steps[k - 2])
            } else {
                None
            }
        }
    };
    TickIn { kill: r.device == 9 && r.held & Buttons::B == 0, g_down: r.g_down, g_a: r.g_a, thrust, phase: 0 }
}

/// Simulate; returns states at sample times 1..=n (positions as the integrator holds them at that instant, velocity with
/// pending thrust applied for BeforeLag).
fn sim(seg: &Seg, o: &Opts, sched: &[(u64, u32)], mode: u8) -> Vec<Ref> {
    let n = seg.steps.len();
    let mut tk = Ticker::new(sched);
    let mut s = S::from_rec(&seg.start);
    let mut out = Vec::with_capacity(n);
    let finish = |s: &S, k: usize| -> Ref {
        let mut c = *s;
        if o.order == Order::BeforeLag {
            if let Some(t) = thrust_in(&seg.steps[k - 1]) {
                let _ = apply_thrust(&mut c, ONE, &t, o, 0, 0);
            }
        }
        ref_of_s(&c)
    };
    while out.len() < n {
        let t0 = tk.t;
        let end = tk.next_end();
        let k_int = if mode == 2 { ((t0 + end) / 2 / u64::from(ONE)) as usize + 1 } else { (t0 / u64::from(ONE)) as usize + 1 };
        let mut inp = tickin(seg, o, k_int);
        inp.phase = (t0 % u64::from(ONE)) as u32;
        let k_next = out.len() + 1;
        let sample_t = k_next as u64 * u64::from(ONE);
        if mode == 0 {
            let b = k_int as u64 * u64::from(ONE);
            let eff = end.min(b);
            let _ = tick(&mut s, (eff - t0) as u32, &inp, o);
            if eff == end {
                tk.advance(end);
            } else {
                tk.t = eff;
            }
            if tk.t == sample_t {
                out.push(finish(&s, k_next));
            }
        } else {
            if end > sample_t && t0 < sample_t {
                let mut c = s;
                let _ = tick(&mut c, (sample_t - t0) as u32, &inp, o);
                out.push(finish(&c, k_next));
            }
            let _ = tick(&mut s, (end - t0) as u32, &inp, o);
            tk.advance(end);
            if end == sample_t {
                out.push(finish(&s, k_next));
            }
        }
    }
    out
}

/// Number of vertical kicks in a free-mode simulation.
fn kicks_at(seg: &Seg, o: &Opts, rate: u32) -> usize {
    let n = seg.steps.len();
    let mut tk = Ticker::new(&[(0, rate)]);
    let mut s = S::from_rec(&seg.start);
    let mut kicks = 0;
    while tk.t < n as u64 * u64::from(ONE) {
        let t0 = tk.t;
        let end = tk.next_end();
        let k_int = (t0 / u64::from(ONE)) as usize + 1;
        let mut inp = tickin(seg, o, k_int);
        inp.phase = (t0 % u64::from(ONE)) as u32;
        let e = tick(&mut s, (end - t0) as u32, &inp, o);
        kicks += e.kick_v as usize;
        tk.advance(end);
    }
    kicks
}

fn q(v: i64) -> f64 {
    v as f64 / (1u64 << 24) as f64
}

#[derive(Default, Clone)]
struct Stat {
    pe: Vec<f64>,
    ve: Vec<f64>,
    drift: f64,
    segs: usize,
}
impl Stat {
    fn add(&mut self, a: &[Ref], b: &[Ref]) {
        let mut last = 0.0;
        for (x, y) in a.iter().zip(b) {
            let pe = (q(x.x - y.x).powi(2) + q(x.y - y.y).powi(2)).sqrt();
            let ve = (q(x.vx - y.vx).powi(2) + q(x.vy - y.vy).powi(2)).sqrt();
            self.pe.push(pe);
            self.ve.push(ve);
            last = pe;
        }
        self.drift += last / a.len() as f64;
        self.segs += 1;
    }
    fn fmt(&mut self) -> String {
        let rms = |v: &Vec<f64>| (v.iter().map(|x| x * x).sum::<f64>() / v.len().max(1) as f64).sqrt();
        let pct = |v: &mut Vec<f64>, p: f64| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[((v.len() as f64 - 1.0) * p) as usize]
        };
        let pmax = self.pe.iter().cloned().fold(0.0, f64::max);
        let vmax = self.ve.iter().cloned().fold(0.0, f64::max);
        let (p50, p99) = (pct(&mut self.pe, 0.5), pct(&mut self.pe, 0.99));
        let p90 = pct(&mut self.pe, 0.9);
        format!(
            "pos max {:8.4} p99 {:7.4} p90 {:7.4} p50 {:7.5} rms {:7.4} | vel max {:7.4} rms {:7.5} | drift {:8.6}/step",
            pmax,
            p99,
            p90,
            p50,
            rms(&self.pe),
            vmax,
            rms(&self.ve),
            self.drift / self.segs.max(1) as f64
        )
    }
}

const RATES: [u32; 10] = [30, 60, 75, 90, 100, 120, 137, 144, 165, 200];

/// Cuts segments at steps the exact f=1 model does not reproduce (object contacts and the like).
fn clean(segs: Vec<Seg>, rep: &mut Vec<(String, f64, f64)>) -> Vec<Seg> {
    let o = Opts::exact(Order::After);
    let mut out = Vec::new();
    for seg in segs {
        let mut cur_start = seg.start;
        let mut cur: Vec<Rec> = Vec::new();
        let mut s = S::from_rec(&cur_start);
        for r in &seg.steps {
            let inp = TickIn { kill: r.device == 9 && r.held & Buttons::B == 0, g_down: r.g_down, g_a: r.g_a, thrust: thrust_in(r), phase: 0 };
            let _ = tick(&mut s, ONE, &inp, &o);
            let want = ref_of_rec(r);
            let got = ref_of_s(&s);
            if got.x != want.x || got.y != want.y || got.vx != want.vx || got.vy != want.vy {
                let pe = (q(got.x - want.x).powi(2) + q(got.y - want.y).powi(2)).sqrt();
                let ve = (q(got.vx - want.vx).powi(2) + q(got.vy - want.vy).powi(2)).sqrt();
                rep.push((seg.name.clone(), pe, ve));
                if cur.len() >= 12 {
                    out.push(Seg { name: seg.name.clone(), start: cur_start, steps: std::mem::take(&mut cur) });
                }
                cur.clear();
                cur_start = *r;
                s = S::from_rec(r);
            } else {
                cur.push(*r);
            }
        }
        if cur.len() >= 12 {
            out.push(Seg { name: seg.name.clone(), start: cur_start, steps: cur });
        }
    }
    out
}

fn main() {
    let world = exp::world();
    if std::env::var("JET").is_ok() {
        exp::motion::jet::report(&world);
        return;
    }
    let (rep, syn) = collect(&world);
    let nraw: usize = rep.iter().chain(syn.iter()).map(|s| s.steps.len()).sum();
    let mut cut = Vec::new();
    let rep = clean(rep, &mut cut);
    let syn = clean(syn, &mut cut);
    let kept: usize = rep.iter().chain(syn.iter()).map(|s| s.steps.len()).sum();
    let small = cut.iter().filter(|c| c.1 < 0.05 && c.2 < 0.05).count();
    println!("raw free steps {nraw}, model mismatches (cut) {}, of which tiny (<0.05 px, <0.05 px/step) {small}, kept steps {kept}", cut.len());
    let mut by: std::collections::BTreeMap<String, (usize, f64, f64)> = Default::default();
    for c in &cut {
        let k = c.0.split("-s").next().unwrap().to_string();
        let e = by.entry(k).or_default();
        e.0 += 1;
        e.1 = e.1.max(c.1);
        e.2 = e.2.max(c.2);
    }
    for (k, v) in by.iter().filter(|(_, v)| v.0 > 20) {
        println!("  cuts {k}: {} maxpe {:.3} maxve {:.3}", v.0, v.1, v.2);
    }
    let all: Vec<Seg> = rep.iter().chain(syn.iter()).cloned().collect();
    let steps: usize = all.iter().map(|s| s.steps.len()).sum();
    println!("segments: replays {} synthetic {} total steps {}", rep.len(), syn.len(), steps);
    let thr = all.iter().map(|s| s.steps.iter().filter(|r| r.held & 0x40 != 0).count()).sum::<usize>();
    println!("steps with B held: {thr}");

    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    for seg in &all {
        *by.entry(seg.name.split("-s").next().unwrap().split("-e").next().unwrap().to_string()).or_default() += seg.steps.len();
    }
    // 1. exactness at R=30 against the recording
    let one = [(0u64, 30u32)];
    for (name, o) in [("After", Opts::exact(Order::After)), ("BeforeLag", Opts::exact(Order::BeforeLag))] {
        let (mut pm, mut vm, mut tot) = (0, 0, 0);
        for seg in &all {
            let out = sim(seg, &o, &one, 0);
            for (a, r) in out.iter().zip(&seg.steps) {
                let w = ref_of_rec(r);
                tot += 1;
                pm += (a.x != w.x || a.y != w.y) as usize;
                vm += (a.vx != w.vx || a.vy != w.vy) as usize;
            }
        }
        println!("exact@30 {name}: samples {tot} pos mismatches {pm} vel mismatches {vm}");
    }
    let l_ref: Vec<Vec<Ref>> = all.iter().map(|sg| sim(sg, &Opts::exact(Order::BeforeNow), &one, 0)).collect();
    let rec_ref: Vec<Vec<Ref>> = all.iter().map(|sg| sg.steps.iter().map(ref_of_rec).collect()).collect();

    // classify segments by the original's nonlinear events (counted at f=1)
    let mut classes: Vec<usize> = Vec::new(); // 0 = no events, 1 = gravity reflection only, 2 = kick/horizontal crossing
    let (mut nk, mut nh, mut ng) = (0, 0, 0);
    for seg in &all {
        let o = Opts::exact(Order::After);
        let mut st = S::from_rec(&seg.start);
        let (mut k, mut h, mut g, mut c) = (0, 0, 0, 0);
        for r in &seg.steps {
            let inp = TickIn { kill: r.device == 9 && r.held & Buttons::B == 0, g_down: r.g_down, g_a: r.g_a, thrust: thrust_in(r), phase: 0 };
            let e = tick(&mut st, ONE, &inp, &o);
            k += e.kick_v as usize;
            h += e.cross_h as usize;
            g += e.grav as usize;
            c += (e.cap || e.kill_sat) as usize;
        }
        nk += k;
        nh += h;
        ng += g;
        classes.push(if k + h == 0 && g + c == 0 { 0 } else if k + h == 0 { 1 } else { 2 });
    }
    println!("original events in the kept steps: vertical thrust reversals {nk}, horizontal thrust reversals {nh}, gravity reflections {ng}");
    println!("segments: {} with no events, {} with gravity reflection / cap hits / killer floor only, {} with thrust reversals", classes.iter().filter(|&&c| c == 0).count(), classes.iter().filter(|&&c| c == 1).count(), classes.iter().filter(|&&c| c == 2).count());

    if std::env::var("KICKS").is_ok() {
        for &rate in &[60u32, 144] {
            for (nm, o) in [("cross", Opts::exact(Order::BeforeLag)), ("cooldown", Opts { cooldown: true, ..Opts::exact(Order::BeforeLag) })] {
                let mut hist: std::collections::BTreeMap<i64, usize> = Default::default();
                let (mut tot_o, mut tot_s) = (0usize, 0usize);
                let mut errs_eq = Vec::new();
                let mut errs_ne = Vec::new();
                for (i, seg) in all.iter().enumerate() {
                    let ko = {
                        let mut st = S::from_rec(&seg.start);
                        let oo = Opts::exact(Order::After);
                        let mut k = 0;
                        for r in &seg.steps {
                            let inp = TickIn { kill: r.device == 9 && r.held & Buttons::B == 0, g_down: r.g_down, g_a: r.g_a, thrust: thrust_in(r), phase: 0 };
                            k += tick(&mut st, ONE, &inp, &oo).kick_v as usize;
                        }
                        k
                    };
                    let ks = kicks_at(seg, &o, rate);
                    tot_o += ko;
                    tot_s += ks;
                    *hist.entry(ks as i64 - ko as i64).or_default() += 1;
                    let out = sim(seg, &o, &[(0, rate)], 1);
                    let e = out.iter().zip(&rec_ref[i]).map(|(a, b)| (q(a.x - b.x).powi(2) + q(a.y - b.y).powi(2)).sqrt()).fold(0.0, f64::max);
                    if ks == ko { errs_eq.push(e) } else { errs_ne.push(e) }
                }
                let med = |v: &mut Vec<f64>| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); if v.is_empty() { 0.0 } else { v[v.len() / 2] } };
                println!("R={rate} {nm}: kicks orig {tot_o} ours {tot_s}; (ours-orig) per segment histogram {:?}; max-pos-err median where equal {:.3} (n={}), where different {:.3} (n={})", hist, med(&mut errs_eq), errs_eq.len(), med(&mut errs_ne), errs_ne.len());
            }
        }
        return;
    }
    if std::env::var("FINAL2").is_ok() { final2(&all, &classes, &rec_ref, &l_ref); return; }
    if std::env::var("FINAL").is_ok() { final_report(&all, &classes, &rec_ref, &l_ref); return; }
    if std::env::var("DEBUGCAP").is_ok() {
        let mut worst = (0.0, 0usize);
        for (i, seg) in all.iter().enumerate() {
            let o = Opts::exact(Order::After);
            let mut st = S::from_rec(&seg.start);
            let mut m = 0u8;
            for r in &seg.steps {
                let inp = TickIn { kill: r.device == 9 && r.held & Buttons::B == 0, g_down: r.g_down, g_a: r.g_a, thrust: thrust_in(r), phase: 0 };
                let e = tick(&mut st, ONE, &inp, &o);
                m |= (e.grav as u8) | (e.cap as u8) << 1 | (e.kill_sat as u8) << 2 | (e.kick_v as u8) << 3;
            }
            if m != 1 { continue; }
            let out = sim(seg, &Opts { cap_bias: true, ..Opts::exact(Order::BeforeLag) }, &[(0, 60)], 0);
            let e = out.iter().zip(&rec_ref[i]).map(|(a, b)| (q(a.x - b.x).powi(2) + q(a.y - b.y).powi(2)).sqrt()).fold(0.0, f64::max);
            if e > worst.0 { worst = (e, i); }
        }
        let seg = &all[worst.1];
        println!("worst cap-only seg {} len {} pe {:.3}", seg.name, seg.steps.len(), worst.0);
        let out = sim(seg, &Opts { cap_bias: true, ..Opts::exact(Order::BeforeLag) }, &[(0, 60)], 0);
        let mut st = S::from_rec(&seg.start);
        let mut prev = 0.0;
        for (k, r) in seg.steps.iter().enumerate() {
            let inp = TickIn { kill: false, g_down: r.g_down, g_a: r.g_a, thrust: thrust_in(r), phase: 0 };
            let e = tick(&mut st, ONE, &inp, &Opts::exact(Order::After));
            let a = &out[k]; let b = &rec_ref[worst.1][k];
            let ve = (q(a.vx - b.vx).powi(2) + q(a.vy - b.vy).powi(2)).sqrt();
            if (102..=109).contains(&k) {
                println!("k {k:3} held {:02x} facing {:2} g {} {} v {}{} acc {} | grav {} | ours vy {:.4} orig vy {:.4}  ve {:.4} pe {:.3}", r.held, r.facing, r.g_a, r.g_down as u8, if r.vd {'D'} else {'U'}, r.v, r.av, e.grav as u8, q(a.vy), q(b.vy), ve, (q(a.x-b.x).powi(2)+q(a.y-b.y).powi(2)).sqrt());
            }
            prev = ve;
        }
        return;
    }
    if std::env::var("RATESCAN").is_ok() {
        let o = Opts { cap_bias: true, phase_kick: true, ..Opts::exact(Order::BeforeLag) };
        for &r in &RATES {
            let mut a = run_stat(&all, &classes, &|_| true, &rec_ref, &o, &[(0, r)], 0);
            let mut b = run_stat(&all, &classes, &|c| c == 0, &rec_ref, &o, &[(0, r)], 0);
            let rms = |v: &Vec<f64>| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
            println!("R={r:3} all rms {:.3} p99 {:.3} p50 {:.4} | {}", rms(&a.pe), { a.pe.sort_by(|x, y| x.partial_cmp(y).unwrap()); a.pe[(a.pe.len() as f64 * 0.99) as usize] }, a.pe[a.pe.len() / 2], b.fmt());
        }
        return;
    }
    if std::env::var("CAM").is_ok() {
        let mut hist = [0usize; 8];
        let (mut sum, mut n) = (0u64, 0u64);
        for seg in &all {
            for r in &seg.steps {
                if let Some(t) = thrust_of(r) {
                    if t.ah != 0 {
                        hist[(r.cam >> 5) as usize] += 1;
                        sum += u64::from(r.cam);
                        n += 1;
                    }
                }
            }
        }
        println!("camera carry byte over horizontal-thrust steps: n {n} mean {:.1} hist(32-wide bins) {:?}; zero share {:.2}", sum as f64 / n as f64, hist, all.iter().flat_map(|s| s.steps.iter()).filter(|r| thrust_of(r).is_some_and(|t| t.ah != 0) && r.cam == 0).count() as f64 / n as f64);
        return;
    }
    if std::env::var("SUB").is_ok() {
        // subclasses by event kind, segments without thrust reversals
        let mut masks = Vec::new();
        for seg in &all {
            let o = Opts::exact(Order::After);
            let mut st = S::from_rec(&seg.start);
            let mut m = 0u8;
            for r in &seg.steps {
                let inp = TickIn { kill: r.device == 9 && r.held & Buttons::B == 0, g_down: r.g_down, g_a: r.g_a, thrust: thrust_in(r), phase: 0 };
                let e = tick(&mut st, ONE, &inp, &o);
                m |= (e.grav as u8) | (e.cap as u8) << 1 | (e.kill_sat as u8) << 2 | (e.kick_v as u8) << 3 | (e.cross_h as u8) << 4;
            }
            masks.push(m);
        }
        for (name, sel) in [("none", 0u8), ("grav only", 1), ("cap only", 2), ("killer floor only", 4), ("grav+cap", 3), ("hcross only", 16), ("kick only", 8)] {
            for &rate in &[60u32, 144] {
                let mut st = Stat::default();
                for (i, seg) in all.iter().enumerate() {
                    if masks[i] != sel { continue; }
                    let out = sim(seg, &Opts { cap_bias: true, ..Opts::exact(Order::BeforeLag) }, &[(0, rate)], 0);
                    st.add(&out, &rec_ref[i]);
                }
                if st.segs > 0 { let n = st.segs; println!("{name:20} R={rate:3} segs {:4} {}", n, st.fmt()); }
            }
        }
        return;
    }
    if std::env::var("DEBUG").is_ok() { debug_worst(&all, &rec_ref, &Opts::exact(Order::BeforeLag), 60, &classes); return; }
    let base = |order| Opts::exact(order);
    let cands: Vec<(&str, Opts)> = vec![
        ("N1 naive: thrust after move, no corr", Opts { corr: false, ..base(Order::After) }),
        ("N2 thrust after move, gravity corr", base(Order::After)),
        ("M0 lag, thrust before move, no corr", Opts { corr: false, ..base(Order::BeforeLag) }),
        ("M1 lag, thrust before move, corr, kick on crossing", base(Order::BeforeLag)),
        ("M1p M1 with phase-locked reversal decision", Opts { phase_kick: true, ..base(Order::BeforeLag) }),
        ("M1d M1 kick dropped", Opts { bump: Bump::Drop, ..base(Order::BeforeLag) }),
        ("M1s M1 kick scaled by f", Opts { bump: Bump::Scale, ..base(Order::BeforeLag) }),
        ("L1 thrust now, corr, kick on crossing", base(Order::BeforeNow)),
    ];
    let which: Vec<(&str, Box<dyn Fn(usize) -> bool>)> = vec![
        ("segments without nonlinear events", Box::new(|c| c == 0)),
        ("segments without thrust reversals", Box::new(|c| c <= 1)),
        ("all segments", Box::new(|_| true)),
    ];
    for &split in &[true, false] {
        for (wname, sel) in &which {
            println!("\n=== {} | {} ===", if split { "split at step boundaries" } else { "tick-start input, peek at samples" }, wname);
            for (name, o) in &cands {
                println!("{name}");
                for &r in &RATES {
                    let mut st = Stat::default();
                    for (i, seg) in all.iter().enumerate() {
                        if !sel(classes[i]) {
                            continue;
                        }
                        let out = sim(seg, o, &[(0, r)], if split { 0 } else { 1 });
                        let rf = if o.order == Order::BeforeNow { &l_ref[i] } else { &rec_ref[i] };
                        st.add(&out, rf);
                    }
                    println!("   R={r:3} {}", st.fmt());
                }
            }
        }
    }
}

#[allow(dead_code)]
pub fn debug_worst(all: &[Seg], rec_ref: &[Vec<Ref>], o: &Opts, rate: u32, classes: &[usize]) {
    let mut worst = (0.0, 0usize);
    for (i, seg) in all.iter().enumerate() {
        if classes[i] != 0 { continue; }
        let out = sim(seg, o, &[(0, rate)], 0);
        for (a, b) in out.iter().zip(&rec_ref[i]) {
            let pe = (q(a.x - b.x).powi(2) + q(a.y - b.y).powi(2)).sqrt();
            if pe > worst.0 {
                worst = (pe, i);
            }
        }
    }
    let seg = &all[worst.1];
    let out = sim(seg, o, &[(0, rate)], 0);
    println!("worst seg {} len {} pe {:.2}", seg.name, seg.steps.len(), worst.0);
    let mut prev_ve = 0.0;
    for (k, (a, b)) in out.iter().zip(&rec_ref[worst.1]).enumerate() {
        let pe = (q(a.x - b.x).powi(2) + q(a.y - b.y).powi(2)).sqrt();
        let ve = (q(a.vx - b.vx).powi(2) + q(a.vy - b.vy).powi(2)).sqrt();
        let r = &seg.steps[k];
        if (ve - prev_ve).abs() > 0.004 || k % 40 == 0 || false {
            println!("k {k:3} g {} {} v {}{} acc {} units ({},{}) want ({},{}) pe {pe:7.3} ve {ve:7.4} held {:02x} facing {:2} v ({:.3},{:.3}) want v ({:.3},{:.3}) y {}", r.g_a, r.g_down as u8, if r.vd {"D"} else {"U"}, r.v, r.av, a.vx as f64/65536.0, a.vy as f64/65536.0, b.vx as f64/65536.0, b.vy as f64/65536.0, r.held, r.facing, q(a.vx), q(a.vy), q(b.vx), q(b.vy), r.y);
        }
        prev_ve = ve;
    }
}

fn run_stat(all: &[Seg], classes: &[usize], sel: &dyn Fn(usize) -> bool, refs: &[Vec<Ref>], o: &Opts, sched: &[(u64, u32)], mode: u8) -> Stat {
    let mut st = Stat::default();
    for (i, seg) in all.iter().enumerate() {
        if !sel(classes[i]) {
            continue;
        }
        let out = sim(seg, o, sched, mode);
        st.add(&out, &refs[i]);
    }
    st
}

fn final_report(all: &[Seg], classes: &[usize], rec_ref: &[Vec<Ref>], l_ref: &[Vec<Ref>]) {
    let none = |c: usize| c == 0;
    let any = |_: usize| true;
    let m1 = Opts::exact(Order::BeforeLag);
    println!("\n##### A. integrator comparison, split sampling (pure integrator error), class: no nonlinear events");
    for (name, o, refs) in [
        ("N1 naive (a*f, x+=v*f, thrust after move)", Opts { corr: false, ..Opts::exact(Order::After) }, rec_ref),
        ("N2 gravity exact-form, thrust after move", Opts::exact(Order::After), rec_ref),
        ("M0 thrust before move, lagged, no correction", Opts { corr: false, ..m1 }, rec_ref),
        ("M1 thrust before move, lagged, exact form", m1, rec_ref),
        ("L1 thrust before move, no lag (vs original with thrust one step earlier)", Opts::exact(Order::BeforeNow), l_ref),
    ] {
        println!("{name}");
        for &r in &RATES {
            let mut st = run_stat(all, classes, &none, refs, &o, &[(0, r)], 0);
            println!("   R={r:3} {}", st.fmt());
        }
    }
    println!("\n##### B. same, all segments (cap hits, reflections, reversals included)");
    for (name, o, refs) in [("N1 naive", Opts { corr: false, ..Opts::exact(Order::After) }, rec_ref), ("M1 exact form lagged", m1, rec_ref), ("L1 no lag (vs advanced reference)", Opts::exact(Order::BeforeNow), l_ref)] {
        println!("{name}");
        for &r in &RATES {
            let mut st = run_stat(all, classes, &any, refs, &o, &[(0, r)], 0);
            println!("   R={r:3} {}", st.fmt());
        }
    }
    println!("\n##### C. M1 with inputs sampled per tick instead of split at step edges (class none / all)");
    for (nm, mode) in [("tick start", 1u8), ("tick middle", 2u8)] {
        println!("{nm}");
        for &r in &[60u32, 90, 100, 137, 144, 165, 200] {
            let mut a = run_stat(all, classes, &none, rec_ref, &m1, &[(0, r)], mode);
            let mut b = run_stat(all, classes, &any, rec_ref, &m1, &[(0, r)], mode);
            println!("   R={r:3} none: {} | all: rms {:.3}", a.fmt(), (b.pe.iter().map(|x| x * x).sum::<f64>() / b.pe.len() as f64).sqrt());
            let _ = &mut b;
        }
    }
    println!("\n##### D. decisions (M1, split): reversal kick, carry bytes; rows R=60 and R=144; classes none/all");
    let variants: Vec<(&str, Opts)> = vec![
        ("kick full size on crossing (chosen)", m1),
        ("kick scaled by f", Opts { bump: Bump::Scale, ..m1 }),
        ("kick dropped", Opts { bump: Bump::Drop, ..m1 }),
        ("kick, at most one per 1/30 s", Opts { cooldown: true, ..m1 }),
        ("kick, phase-locked decision", Opts { phase_kick: true, ..m1 }),
        ("thrust cap lowered by opposing gravity", Opts { cap_bias: true, ..m1 }),
        ("cap lowered + phase-locked kick", Opts { cap_bias: true, phase_kick: true, ..m1 }),
        ("cap lowered + phase-locked kick + 1/30 s cooldown", Opts { cap_bias: true, phase_kick: true, cooldown: true, ..m1 }),
        ("cap lowered + cooldown", Opts { cap_bias: true, cooldown: true, ..m1 }),
        ("vertical carry byte dropped", Opts { carry_v: false, ..m1 }),
        ("horizontal carry byte = 0", Opts { carry_h: CarryH::Zero, ..m1 }),
        ("horizontal carry byte = 69 (its mean)", Opts { carry_h: CarryH::Mean, ..m1 }),
        ("both carry bytes dropped (h=0)", Opts { carry_v: false, carry_h: CarryH::Zero, ..m1 }),
    ];
    for (name, o) in &variants {
        for &r in &[60u32, 144] {
            let mut a = run_stat(all, classes, &none, rec_ref, o, &[(0, r)], 0);
            let mut b = run_stat(all, classes, &any, rec_ref, o, &[(0, r)], 0);
            let (ra, rb) = ((a.pe.iter().map(|x| x * x).sum::<f64>() / a.pe.len() as f64).sqrt(), (b.pe.iter().map(|x| x * x).sum::<f64>() / b.pe.len() as f64).sqrt());
            let drift_a = a.drift / a.segs as f64;
            let drift_b = b.drift / b.segs as f64;
            let _ = (a.fmt(), b.fmt());
            println!("{name:42} R={r:3} none: pos rms {ra:.4} drift {drift_a:.5}/step | all: pos rms {rb:.3} drift {drift_b:.5}/step");
        }
    }
    println!("   (carry bytes at R=30, vs recording:)");
    for (name, o) in &variants[5..] {
        let a = run_stat(all, classes, &any, rec_ref, o, &[(0, 30)], 0);
        let rms = (a.pe.iter().map(|x| x * x).sum::<f64>() / a.pe.len() as f64).sqrt();
        println!("{name:42} R= 30 all: pos rms {rms:.4} drift {:.5}/step", a.drift / a.segs as f64);
    }
    println!("\n##### E. fixed-point formats: error against the Q.24/Q.24 integrator itself (quantisation only), class none, split");
    for &r in &[60u32, 200] {
        let base_out: Vec<Vec<Ref>> = all.iter().enumerate().map(|(i, s)| if none(classes[i]) { sim(s, &m1, &[(0, r)], 0) } else { Vec::new() }).collect();
        for &pb in &[12u32, 16, 20, 24] {
            for &sb in &[16u32, 20, 24] {
                let o = Opts { pos_bits: pb, spd_bits: sb, ..m1 };
                let mut st = Stat::default();
                let mut vs = Stat::default();
                for (i, seg) in all.iter().enumerate() {
                    if !none(classes[i]) {
                        continue;
                    }
                    let out = sim(seg, &o, &[(0, r)], 0);
                    st.add(&out, &base_out[i]);
                    vs.add(&out, &rec_ref[i]);
                }
                println!("   R={r:3} pos Q.{pb:2} speed Q.{sb:2}: vs Q.24: {} | vs original: {}", st.fmt(), vs.fmt());
            }
        }
    }
    println!("\n##### F. rate changes in flight, M1, split, all segments");
    let t = u64::from(ONE);
    for (name, sched) in [
        ("144 -> 60 at step 20 -> 120 at step 40", vec![(0u64, 144u32), (20 * t, 60), (40 * t, 120)]),
        ("200 -> 75 at step 15 -> 137 at step 33 -> 90 at step 50", vec![(0, 200), (15 * t, 75), (33 * t, 137), (50 * t, 90)]),
        ("constant 144", vec![(0, 144)]),
        ("constant 60", vec![(0, 60)]),
    ] {
        let mut st = run_stat(all, classes, &none, rec_ref, &m1, &sched, 0);
        let mut sa = run_stat(all, classes, &any, rec_ref, &m1, &sched, 0);
        println!("{name}: none: {} | all rms {:.3}", st.fmt(), (sa.pe.iter().map(|x| x * x).sum::<f64>() / sa.pe.len() as f64).sqrt());
        let _ = sa.fmt();
    }
    // jump test: the largest change of position error between consecutive samples, around the switch and elsewhere
    let sched = vec![(0u64, 144u32), (20 * t, 60), (40 * t, 120)];
    let (mut around, mut elsewhere) = (0.0f64, 0.0f64);
    for (i, seg) in all.iter().enumerate() {
        if !none(classes[i]) || seg.steps.len() < 60 {
            continue;
        }
        let out = sim(seg, &m1, &sched, 0);
        for k in 1..out.len() {
            let e = |j: usize| (q(out[j].x - rec_ref[i][j].x).powi(2) + q(out[j].y - rec_ref[i][j].y).powi(2)).sqrt();
            let d = (e(k) - e(k - 1)).abs();
            if (19..=21).contains(&k) || (39..=41).contains(&k) {
                around = around.max(d);
            } else {
                elsewhere = elsewhere.max(d);
            }
        }
    }
    println!("largest step-to-step change of position error: at the switches {around:.5} px, elsewhere {elsewhere:.5} px");
    println!("\n##### G. the intentional lead of L1 (thrust acts one step earlier) against the original, R=30 itself");
    for (nm, sel) in [("none", &none as &dyn Fn(usize) -> bool), ("all", &any)] {
        let mut st = run_stat(all, classes, sel, rec_ref, &Opts::exact(Order::BeforeNow), &[(0, 30)], 0);
        println!("   class {nm}: {}", st.fmt());
    }
}

fn final2(all: &[Seg], classes: &[usize], rec_ref: &[Vec<Ref>], l_ref: &[Vec<Ref>]) {
    let m1 = Opts::exact(Order::BeforeLag);
    let m2 = Opts { cap_bias: true, ..m1 };
    let m3 = Opts { cap_bias: true, phase_kick: true, ..m1 };
    let l2 = Opts { cap_bias: true, ..Opts::exact(Order::BeforeNow) };
    let cl: [(&str, Box<dyn Fn(usize) -> bool>); 4] = [
        ("class 0: no nonlinear event", Box::new(|c| c == 0)),
        ("class 1: gravity reflection / cap hit / killer floor, no thrust reversal", Box::new(|c| c == 1)),
        ("class 2: vertical thrust reversal (kick) present", Box::new(|c| c == 2)),
        ("all segments", Box::new(|_| true)),
    ];
    for (cn, sel) in &cl {
        let nseg = classes.iter().filter(|&&c| sel(c)).count();
        println!("\n##### {cn} ({nseg} segments), split sampling");
        for (name, o, refs) in [("M2 = exact form + cap lowered", m2, rec_ref), ("M3 = M2 + phase-locked kick decision", m3, rec_ref), ("L2 = M2 with no input lag (vs original-with-thrust-one-step-earlier)", l2, l_ref)] {
            println!("{name}");
            for &r in &RATES {
                let mut st = run_stat(all, classes, sel.as_ref(), refs, &o, &[(0, r)], 0);
                println!("   R={r:3} {}", st.fmt());
            }
        }
    }
    println!("\n##### M2 with per-tick input sampling (not split at step edges), class 0 / all");
    for (nm, mode) in [("tick start", 1u8), ("tick middle", 2u8)] {
        println!("{nm}");
        for &r in &RATES {
            let mut a = run_stat(all, classes, &|c| c == 0, rec_ref, &m2, &[(0, r)], mode);
            let mut b = run_stat(all, classes, &|_| true, rec_ref, &m2, &[(0, r)], mode);
            println!("   R={r:3} class0: {} | all: p50 {:.4} p90 {:.3}", a.fmt(), { b.pe.sort_by(|x, y| x.partial_cmp(y).unwrap()); b.pe[b.pe.len() / 2] }, b.pe[(b.pe.len() as f64 * 0.9) as usize]);
        }
    }
    println!("\n##### M2 rate changes, split, class 0 / all (p50, p90)");
    let t = u64::from(ONE);
    for (name, sched) in [
        ("144 -> 60 -> 120 (switch at steps 20, 40)", vec![(0u64, 144u32), (20 * t, 60), (40 * t, 120)]),
        ("200 -> 75 -> 137 -> 90", vec![(0, 200), (15 * t, 75), (33 * t, 137), (50 * t, 90)]),
        ("constant 144", vec![(0, 144)]),
        ("constant 60", vec![(0, 60)]),
    ] {
        let mut a = run_stat(all, classes, &|c| c == 0, rec_ref, &m2, &sched, 0);
        let mut b = run_stat(all, classes, &|_| true, rec_ref, &m2, &sched, 0);
        println!("{name}: class0: {} | all: p50 {:.4} p90 {:.3}", a.fmt(), { b.pe.sort_by(|x, y| x.partial_cmp(y).unwrap()); b.pe[b.pe.len() / 2] }, b.pe[(b.pe.len() as f64 * 0.9) as usize]);
    }
    println!("\n##### formats for M2, class 0, split: quantisation only (vs Q.24/Q.24) and vs original");
    for &r in &[60u32, 200] {
        let base_out: Vec<Vec<Ref>> = all.iter().enumerate().map(|(i, s)| if classes[i] == 0 { sim(s, &m2, &[(0, r)], 0) } else { Vec::new() }).collect();
        for &(pb, sb) in &[(12u32, 24u32), (16, 24), (16, 20), (16, 16), (20, 20), (24, 24)] {
            let o = Opts { pos_bits: pb, spd_bits: sb, ..m2 };
            let mut st = Stat::default();
            let mut vs = Stat::default();
            for (i, seg) in all.iter().enumerate() {
                if classes[i] != 0 {
                    continue;
                }
                let out = sim(seg, &o, &[(0, r)], 0);
                st.add(&out, &base_out[i]);
                vs.add(&out, &rec_ref[i]);
            }
            println!("   R={r:3} pos Q.{pb:2} speed Q.{sb:2}: vs Q.24: {} | vs original: p50 {:.5} rms {:.4} drift {:.6}", st.fmt(), { vs.pe.sort_by(|x, y| x.partial_cmp(y).unwrap()); vs.pe[vs.pe.len() / 2] }, (vs.pe.iter().map(|x| x * x).sum::<f64>() / vs.pe.len() as f64).sqrt(), vs.drift / vs.segs as f64);
        }
    }
    println!("\n##### carry bytes for M2 at R=30 (vs recording) and R=144, class 0");
    for (nm, o) in [("both kept (chosen: vertical kept)", m2), ("vertical dropped", Opts { carry_v: false, ..m2 }), ("horizontal 0", Opts { carry_h: CarryH::Zero, ..m2 }), ("horizontal 69", Opts { carry_h: CarryH::Mean, ..m2 }), ("both dropped", Opts { carry_v: false, carry_h: CarryH::Zero, ..m2 })] {
        for &r in &[30u32, 144] {
            let mut st = run_stat(all, classes, &|c| c == 0, rec_ref, &o, &[(0, r)], 0);
            println!("{nm:36} R={r:3} {}", st.fmt());
        }
    }
}
