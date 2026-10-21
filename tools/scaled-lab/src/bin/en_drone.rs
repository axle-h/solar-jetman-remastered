use std::collections::BTreeMap;
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    let mut dec: BTreeMap<u8, u64> = BTreeMap::new();
    let mut shots = vec![]; let mut tot_dec = 0u64; let mut steps = 0u64;
    for r in &runs { for s in 3..31 { let mut last_shot: Option<usize> = None;
        for i in 1..r.frames.len() {
            let a = &r.frames[i-1].slots[s]; let b = &r.frames[i].slots[s];
            if a.state == 0x19 && b.state == 0x19 && a.index != 0 && b.index != 0 {
                let d = a.work5.wrapping_sub(b.work5); *dec.entry(d).or_default() += 1; steps += 1; tot_dec += u64::from(d.min(8));
                // a shot fires when work5 crosses a multiple of 64 going down
                if (a.work5 >> 6) != (b.work5 >> 6) && d <= 8 { if let Some(l) = last_shot { shots.push(i - l); } last_shot = Some(i); }
            } else { last_shot = None; }
        }
    }}
    println!("drone steps {steps}; work5 decrement per step histogram: {:?}", dec.iter().filter(|(_, n)| **n > 20).collect::<Vec<_>>());
    println!("mean decrement/step {:.2}; shot intervals: n {} mean {:.1} steps ({:.2} s) min {} max {}", tot_dec as f64 / steps as f64, shots.len(), shots.iter().sum::<usize>() as f64 / shots.len().max(1) as f64, shots.iter().sum::<usize>() as f64 / shots.len().max(1) as f64 / 30.0, shots.iter().min().unwrap_or(&0), shots.iter().max().unwrap_or(&0));
}
