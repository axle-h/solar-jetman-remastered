use exp::tow::rec::*;
fn main() {
    let world = exp::world();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays");
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path().file_stem().unwrap().to_string_lossy().to_string()).collect();
    names.sort();
    for n in names {
        let Some(recs) = replay_recs(&world, &n) else { continue };
        let tow = recs.iter().filter(|r| r.pre_t.slot != 0).count();
        let act = recs.iter().filter(|r| r.pre_t.active).count();
        let thr = recs.iter().filter(|r| r.pre_t.active && r.held & 0x40 != 0).count();
        // segments
        let mut segs = vec![];
        let mut start = None;
        for (i, r) in recs.iter().enumerate() {
            if r.pre_t.slot != 0 { if start.is_none() { start = Some(i); } } else if let Some(s) = start.take() { segs.push((s, i)); }
        }
        if let Some(s) = start { segs.push((s, recs.len())); }
        println!("{n}: steps {} holding {tow} active {act} thrust {thr} segs {:?}", recs.len(), segs);
    }
}
