use std::collections::BTreeMap;
fn main() {
    let w = exp::world();
    let runs = exp::enemies::trace_all(&w);
    let mut cnt: BTreeMap<(u8, u8), (u64, usize)> = BTreeMap::new();
    let mut tot = 0;
    for r in &runs {
        tot += r.frames.len();
        for f in &r.frames { for (i, o) in f.slots.iter().enumerate() { if o.state != 0 && i >= 3 { let e = cnt.entry((o.state, if o.state >= 0x80 { o.kind } else { 0 })).or_default(); e.0 += 1; e.1 = e.1.max(i); } } }
    }
    println!("{} runs {} frames", runs.len(), tot);
    for ((s, k), (n, _)) in cnt { println!("state {s:#04x} kind {k:#04x}: {n} object-steps"); }
}
