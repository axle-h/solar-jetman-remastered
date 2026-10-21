use exp::tow::rec::*;
use exp::tow::scaled::*;
fn main() {
    let world = exp::world();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/mesen/replays");
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path().file_stem().unwrap().to_string_lossy().to_string()).collect();
    names.sort();
    let (mut ok, mut bad) = (0, 0);
    for n in names {
        let Some(recs) = replay_recs(&world, &n) else { continue };
        for r in &recs {
            let pre = r.pre_t;
            let moved_before = pre.speed != 0 || pre.angle != 0;
            let _ = moved_before;
            let mut ro = Rope::from_real(pre.angle, pre.speed, pre.sense, pre.mass);
            let f = 65536;
            let thrust = r.held & 0x40 != 0 && r.post_p.fuel < r.pre_p.fuel && r.state == 1;
            if pre.active && !r.anti { ro.pull(r.g_a, !r.g_down, f, Cap::Glitch); }
            if thrust { ro.torque(r.post_p.facing, pre.active, 20 + (r.post_p.y & 15) as u8, f, Cap::Glitch); }
            if pre.slot != 0 && !pre.taken_in { ro.swing(f); }
            let post = r.post_t;
            let want = (post.angle, post.speed, post.sense);
            let got = (ro.angle16(), ro.speed24(), ro.sense);
            // only compare when the slot is the same (catch/drop steps differ)
            if pre.slot != post.slot { continue; }
            if pre.slot == 0 && !thrust { continue; }
            if want == got { ok += 1 } else {
                bad += 1;
                if bad <= 15 { println!("{n} step {} pre {:?} thrust {thrust} anti {} want {:x?} got {:x?} held {:02x} state {}", r.step, pre, r.anti, want, got, r.held, r.state); }
            }
        }
    }
    println!("ok {ok} bad {bad}");
}
