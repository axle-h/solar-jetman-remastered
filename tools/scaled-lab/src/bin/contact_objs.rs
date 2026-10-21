use exp::contact::obj::*;
use exp::contact::ONE;
fn q(px: f64) -> i64 { (px * 65536.0) as i64 }
fn main() {
    // scenario families; random sub-pixel phases
    let mut seed = 99u64;
    let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 33) as f64) / (1u64 << 31) as f64 };
    struct Fam { name: &'static str, mk: Box<dyn Fn(f64, i32) -> Case> }
    let fams: Vec<Fam> = vec![
        Fam { name: "stationary pod, homing object 0.25/step^2 max 1.0", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(100.0), pod_v: 0, pod_acc: 0, obj_x: q(140.0 + ph), obj_v: 0, home_acc: q(0.25), home_max: q(1.0) }) },
        Fam { name: "stationary pod, homing object max 3.0", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(100.0), pod_v: 0, pod_acc: 0, obj_x: q(140.0 + ph), obj_v: 0, home_acc: q(0.25), home_max: q(3.0) }) },
        Fam { name: "stationary pod, drifting object 0.4/step", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(100.0), pod_v: 0, pod_acc: 0, obj_x: q(130.0 + ph), obj_v: q(-0.4), home_acc: 0, home_max: 0 }) },
        Fam { name: "pod flying 2.0/step into stationary object", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(100.0 + ph), pod_v: q(2.0), pod_acc: 0, obj_x: q(140.0), obj_v: 0, home_acc: 0, home_max: 0 }) },
        Fam { name: "pod flying 4.0/step into stationary object", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(100.0 + ph), pod_v: q(4.0), pod_acc: 0, obj_x: q(140.0), obj_v: 0, home_acc: 0, home_max: 0 }) },
        Fam { name: "pod flying 0.5/step into stationary object", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(120.0 + ph), pod_v: q(0.5), pod_acc: 0, obj_x: q(140.0), obj_v: 0, home_acc: 0, home_max: 0 }) },
        Fam { name: "pod thrusting (0.0625/step^2) into stationary object", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(110.0 + ph), pod_v: 0, pod_acc: q(0.0625), obj_x: q(140.0), obj_v: 0, home_acc: 0, home_max: 0 }) },
        Fam { name: "pod 1.0/step into drifting object 1.0/step toward it", mk: Box::new(|ph, w| Case { w, pw: 3, pod_x: q(100.0 + ph), pod_v: q(1.0), pod_acc: 0, obj_x: q(140.0), obj_v: q(-1.0), home_acc: 0, home_max: 0 }) },
    ];
    let rates = [30i64, 60, 100, 144, 200, 137];
    for fam in &fams {
        println!("== {}", fam.name);
        for (pol, pname) in [(Policy::PerTick, "per-tick naive"), (Policy::Cooldown, "per-object 1/30 s cooldown"), (Policy::CooldownPush, "cooldown + push every tick"), (Policy::Grid, "30 Hz sub-clock")] {
            let mut line = format!("  {pname:28}");
            for &rate in &rates {
                let (mut oh, mut sh, mut tr) = (0.0, 0.0, vec![]);
                let mut rr = |_: ()| rnd();
                for k in 0..900 {
                    let ph = rr(()) * 3.0; let w = [4, 6, 8][k % 3];
                    let case = (fam.mk)(ph, w);
                    let a = run(case, 30, Policy::PerTick, 4.0); let b = run(case, rate, pol, 4.0);
                    oh += a.hits as f64; sh += b.hits as f64; if a.hits > 0 && b.hits > 0 { tr.push(b.first_ms - a.first_ms); }
                }
                let lead = tr.iter().sum::<f64>() / tr.len().max(1) as f64;
                line += &format!(" R{rate}: hits {:+6.1}% (lead {:+5.1}ms)", 100.0 * (sh / oh - 1.0), lead);
            }
            println!("{line}");
        }
        let _ = ONE;
    }
}
