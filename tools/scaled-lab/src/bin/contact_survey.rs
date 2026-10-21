use solar_jetman::game::gameplay::input::Buttons;
fn main() {
    let world = exp::world();
    let names: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays")).unwrap().filter_map(|e| { let n = e.unwrap().file_name().to_string_lossy().to_string(); n.strip_suffix(".replay").map(String::from) }).collect();
    let mut names = names; names.sort();
    let (mut tot_steps, mut tot_contact) = (0usize, 0usize);
    let mut kinds = [0usize; 6]; let mut runs: Vec<usize> = vec![];
    println!("{:22} {:>5} {:>6} {:>6}  flat slope wall ceil  maxrun", "replay", "lvl", "steps", "contact");
    for name in &names {
        let r = exp::contact::replay_full(name);
        if r.level >= 13 && r.level != 26 && r.level < 29 { }
        if r.level >= 29 { continue; }
        let mut g = exp::game(&world, r.level);
        let mut cnt = 0usize; let mut k = [0usize; 4]; let mut run = 0usize; let mut maxrun = 0usize; let mut n = 0usize;
        for (i, &b) in r.steps.iter().enumerate() {
            if let Some(w) = r.warps.get(&(i + 1)) { g.pod.body.x = w[0]; g.pod.body.y = w[1]; g.pod.body.h_speed = 0; g.pod.body.v_speed = 0; }
            if let Some(f) = r.fuel_pokes.get(&(i + 1)) { g.pod.fuel = *f; }
            if !g.next_is_a_pass() || g.level() != r.level { break; }
            g.step(&world, b);
            n += 1;
            if g.pod.state != 1 { continue; }
            let s = g.pod.surface;
            if s != 0 { cnt += 1; run += 1; maxrun = maxrun.max(run);
                if s & 1 != 0 { k[0] += 1; } if s & 0x1E != 0 { k[1] += 1; } if s & 0x20 != 0 { k[2] += 1; } }
            else { if run > 0 { runs.push(run); } run = 0; }
        }
        tot_steps += n; tot_contact += cnt;
        for i in 0..3 { kinds[i] += k[i]; }
        println!("{name:22} {:>5} {:>6} {:>6}  {:>4} {:>5} {:>4} {:>4}  {maxrun}", r.level, n, cnt, k[0], k[1], k[2], k[3]);
    }
    runs.sort();
    let med = runs.get(runs.len() / 2).copied().unwrap_or(0);
    let longer = runs.iter().filter(|&&x| x >= 3).count();
    println!("total steps {tot_steps}, contact steps {tot_contact} ({:.1}%); contact runs {} (median length {med}, >=3 steps: {longer})", 100.0 * tot_contact as f64 / tot_steps as f64, runs.len());
    println!("flat {} slope {} wall/ceil-slope {}", kinds[0], kinds[1], kinds[2]);
}
