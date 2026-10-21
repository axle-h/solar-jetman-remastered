use std::collections::BTreeMap;
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    // 1. first-sight index of enemy shots, gun shots, head shots: did the creation sweep process it?
    let mut first: BTreeMap<(u8, u8, bool), u64> = BTreeMap::new(); // (state, index at first sight, slot>=...)
    let mut speeds: Vec<u16> = vec![];
    let mut tracks = 0; let mut life = vec![];
    for r in &runs { for i in 1..r.frames.len() { for s in 3..31 {
        let a = &r.frames[i-1].slots[s]; let b = &r.frames[i].slots[s];
        if (b.state == 0x15 || b.state == 0x1A || b.state == 0x1B) && a.state == 0 {
            *first.entry((b.state, b.index, s >= 15)).or_default() += 1;
        }
    }}}
    println!("first-sight index of new shots (state,index,slot>=15): count");
    for ((st, ix, hi), n) in &first { if *n > 20 { println!("  state {st:#x} index {ix:#x} slot15+={hi}: {n}"); } }
    // 2. plain mover tracks: enemy shots whose speed is constant for their life -> collect speeds and track lengths
    let mut vs: Vec<(i64, i64, usize)> = vec![];
    for r in &runs { for s in 15..23 { let mut i = 1; while i < r.frames.len() {
        let a = &r.frames[i-1].slots[s];
        if r.frames[i].slots[s].state == 0x15 && a.state == 0 {
            let mut j = i; while j + 1 < r.frames.len() && r.frames[j+1].slots[s].state == 0x15 { j += 1; }
            let o = &r.frames[i+1.min(j-i)].slots[s];
            let n = j - i;
            if n >= 10 { vs.push((i64::from(o.speed_x), i64::from(o.speed_y), n)); }
            tracks += 1; life.push(n);
            i = j + 1;
        } else { i += 1; }
    }}}
    println!("{tracks} shot tracks; life steps: max {} mean {:.1}", life.iter().max().unwrap_or(&0), life.iter().sum::<usize>() as f64 / life.len().max(1) as f64);
    let mut sp: Vec<i64> = vs.iter().flat_map(|v| [v.0, v.1]).collect(); sp.sort();
    println!("speed (1/256 px/step) p50 {} p90 {} max {}", sp[sp.len()/2], sp[sp.len()*9/10], sp[sp.len()-1]);
    // 3. integration error of per-tick x += v*f: truncating vs rounding vs phase form, life up to 256 steps, v up to 8 px/step
    for &rate in &[30i64, 60, 75, 90, 100, 120, 144, 165, 200, 137] {
        let f = (65536 * 30 + rate / 2) / rate;
        let (mut max_t, mut max_r, mut max_p) = (0f64, 0f64, 0f64);
        for &v in &[0x40i64, 0x155, 0x400, 0x7FF, 0x333, 0x0A5] {
            // exact: pos after n ticks at T = n*30/rate steps with v per step; Q16 px accumulators
            let (mut xt, mut xr) = (0i64, 0i64); // Q16 px
            let n_ticks = 256 * rate / 30;
            for n in 1..=n_ticks {
                let inc = v * f; // v 1/256 px * f Q16 -> Q24 px... v*f is (1/256 px) * 2^16 = Q24 px -> to Q16 px: >> 8
                xt += inc >> 8; xr += (inc + 128) >> 8;
                let ideal_q16 = v * 256 * n * 30 / rate / 256 * 1; // placeholder not used
                let _ = ideal_q16;
                // exact ideal using f64
                let ideal = v as f64 / 256.0 * (n as f64 * 30.0 / rate as f64);
                max_t = max_t.max((xt as f64 / 65536.0 - ideal).abs());
                max_r = max_r.max((xr as f64 / 65536.0 - ideal).abs());
                // cumulative-time form: pos = v * Tq where Tq = round(n*30*65536/rate)
                let tq = (n * 30 * 65536 + rate / 2) / rate;
                let xp = (v * tq + 128) >> 8;
                max_p = max_p.max((xp as f64 / 65536.0 - ideal).abs());
            }
        }
        println!("R={rate:3} f={f:5} max err over 256 steps (px): truncated acc {max_t:.4}  rounded acc {max_r:.4}  cumulative-time form {max_p:.5}");
    }
}
