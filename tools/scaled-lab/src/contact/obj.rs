//! 1-D model of pod-against-object contact (collide.rs / behaviour.md 3.1) at tick fraction f.
use super::ONE;

/// Penetration as in the original (whole-pixel coordinates).
pub fn penetration(d: i32, w: i32, p: i32) -> Option<i32> {
    if d < 0 { let c = i32::from(-d <= w); let depth = w + p + d + c; (depth >= 0).then_some(depth) }
    else { let c = i32::from(d < w); let depth = w + p - d + c; (depth > 0).then_some(depth) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Policy { PerTick, Cooldown, Grid, CooldownPush }

#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub w: i32,            // object half width
    pub pw: i32,           // pod half width (3)
    pub pod_x: i64, pub pod_v: i64,   // Q16 px, speed Q16 px/step signed
    pub pod_acc: i64,      // pod accel Q16 px/step^2 signed (thrust)
    pub obj_x: i64, pub obj_v: i64,   // object own velocity
    pub home_acc: i64, pub home_max: i64, // homing accel/max speed Q16 (0 acc = drift)
}

#[derive(Clone, Debug, Default)]
pub struct Out { pub hits: u32, pub dmg_units: i64, pub hit_times: Vec<i64>, pub first_ms: f64 }

pub fn run(c: Case, rate: i64, pol: Policy, secs: f64) -> Out {
    let f = (ONE * 30 + rate / 2) / rate;
    let mut px = c.pod_x; let mut pv = c.pod_v; let mut ox = c.obj_x; let mut ov = c.obj_v;
    let mut t: i64 = 0; let mut next_ok: i64 = i64::MIN / 2; let mut grid_next = ONE;
    let mut out = Out::default();
    let end = (secs * 30.0 * ONE as f64) as i64;
    while t < end {
        // object homing then motion; pod motion (sample-exact: ignore, constant velocity)
        if c.home_acc != 0 {
            let toward = if px > ox { 1 } else { -1 };
            ov += toward * c.home_acc * f / ONE;
            if ov.abs() > c.home_max { ov = c.home_max * ov.signum(); }
        }
        ox += ov * f / ONE; px += pv * f / ONE;
        pv += c.pod_acc * f / ONE;
        t += f;
        let is_grid = t >= grid_next;
        if is_grid { grid_next += ONE; }
        let allowed = match pol { Policy::PerTick => true, Policy::Cooldown | Policy::CooldownPush => t >= next_ok, Policy::Grid => is_grid };
        if !allowed {
            if pol == Policy::CooldownPush {
                let d = ((px >> 16) as i32) - ((ox >> 16) as i32);
                if let Some(pen) = penetration(d, c.w, c.pw) { if d < 0 { ox += (pen as i64) << 16; } else { ox -= (pen as i64) << 16; } }
            }
            continue;
        }
        let (pxw, oxw) = ((px >> 16) as i32, (ox >> 16) as i32);
        let d = pxw - oxw;
        if let Some(pen) = penetration(d, c.w, c.pw) {
            // push object away, pod direction away, pod speed += object speed / 2
            if d < 0 { ox += (pen as i64) << 16; } else { ox -= (pen as i64) << 16; }
            let mag = (pv.abs() + ov.abs() / 2).min(0x3FF * 256);
            pv = if d < 0 { -mag } else { mag };
            out.hits += 1; out.dmg_units += 0x4000; out.hit_times.push(t); next_ok = if t - next_ok >= ONE { t + ONE } else { next_ok + ONE };
            if out.hits == 1 { out.first_ms = t as f64 / 65536.0 * 1000.0 / 30.0; }
        }
    }
    out
}
