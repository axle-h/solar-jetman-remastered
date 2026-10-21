use exp::tow::rec::*;
use exp::tow::seg::*;
fn main() {
    let world = exp::world();
    for level in 0..=12usize {
        for kind in [0u8, 1, 2] {
            for mode in [2u32] {
                let recs = synth_tow(&world, level, kind, kind as usize, mode, 1 + level as u64, 800, false);
                let hold = recs.iter().filter(|r| r.pre_t.slot != 0).count();
                let act = recs.iter().filter(|r| r.pre_t.active && r.state == 1).count();
                let free = recs.iter().filter(|r| r.pre_t.active && r.surface == 0).count();
                let thr = recs.iter().filter(|r| r.pre_t.active && thrust_of(r)).count();
                let segs = segments("s", &recs, 20);
                let lens: Vec<_> = segs.iter().map(|s| s.recs.len()).collect();
                println!("L{level} k{kind} m{mode}: recs {} hold {hold} active {act} free {free} thrust {thr} segs {lens:?} g_a {:?}", recs.len(), recs.get(20).map(|r| (r.g_a, r.g_down)));
            }
        }
    }
}
