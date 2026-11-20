//! dlGetUp on the engine's own bodies (a floor at z = 0): knocked down on the front and on the back, lying still, then
//! up each way. The animation's targets are checked (they start where it lies, end standing on the floor) and played
//! onto the body as the game would; the muscles' and the learned get-ups are left to the body (they can fail).
//! usage: getupsim [GTA IV folder] [getup.net]

use delirium::prelude::*;
use gmsv_delirium::lua::Val;
use gmsv_delirium::sim::Sim;

const DT: f32 = 1.0 / 60.0;
const HEAD: usize = 12;

fn frames(sim: &Sim, id: i32) -> Vec<Transform> {
    sim.bodies[&id].agent.doll.frames().collect()
}

fn heights(f: &[Transform]) -> (f32, f32, f32) {
    let feet = f[3].translation.z.min(f[6].translation.z);
    (f[0].translation.z, f[HEAD].translation.z, feet)
}

fn knock_down(sim: &mut Sim, back: bool) -> i32 {
    let id = sim.create("", Vec3::ZERO, 0.0, 0.3);
    let _ = sim.send(id, "bodyRelax", vec![("start".into(), Val::Bool(true)), ("relaxation".into(), Val::Num(100.0))]);
    let fr = frames(sim, id);
    let fwd = (fr[0].rotation * Vec3::X).with_z(0.0).normalize_or(Vec3::X);
    // which way the body faces: from its hips (left to right, turned up)
    let left = fr[1].translation - fr[4].translation;
    let facing = left.cross(Vec3::Z).with_z(0.0).normalize_or(fwd);
    let dir = if back { -facing } else { facing };
    let at = fr[10].translation;
    sim.bodies.get_mut(&id).unwrap().agent.doll.impulse(10, at, dir * 60.0);
    for _ in 0..240 {
        sim.step(DT);
    }
    id
}

fn run(sim: &mut Sim, back: bool, mode: &str, net: &str) -> bool {
    let id = knock_down(sim, back);
    let lay = frames(sim, id);
    let (p, h, _) = heights(&lay);
    let chest_up = (lay[10].rotation * Vec3::Z).z;
    println!("-- {} on its {} (pelvis {p:.2} m, head {h:.2} m)", mode, if back { "back" } else { "front" });
    let _ = chest_up;
    let mut params = vec![("start".into(), Val::Bool(true)), ("mode".into(), Val::Str(mode.into()))];
    if !net.is_empty() {
        params.push(("net".into(), Val::Str(net.into())));
    }
    if let Some(h) = std::env::var("HELP").ok().and_then(|v| v.parse::<f64>().ok()) {
        params.push(("help".into(), Val::Num(h)));
    }
    if std::env::var("FRESH").is_ok() {
        let b = sim.bodies.get_mut(&id).unwrap();
        b.agent = delirium::nm::Agent::new(&b.build, delirium::ragdoll::Ragdoll::new(&b.build, &lay, Vec3::ZERO), Quat::IDENTITY, 0.3);
    }
    sim.send(id, "dlGetUp", params).expect("dlGetUp");
    let mut first: Option<f32> = None;
    let mut last_targets: Vec<Transform> = vec![];
    let mut t = 0.0;
    let mut ended_at = None;
    let mut ok = true;
    while t < 12.0 {
        sim.step(DT);
        t += DT;
        let b = sim.bodies.get_mut(&id).unwrap();
        let g = b.custom.iter().find_map(|c| c.as_get_up()).map(|g| g.targets().to_vec());
        match g {
            Some(tg) if !tg.is_empty() && mode != "animation" => {
                let fr: Vec<Transform> = b.agent.doll.frames().collect();
                if (t * 60.0).round() as i32 % 15 == 0 {
                    let errs: Vec<String> = [0usize, 1, 2, 4, 5, 9, 10, 12, 14, 15].iter().map(|&k| {
                        let pa = b.build.parts[k].parent;
                        let rel = |f: &[Transform]| pa.map_or(f[k].rotation, |j| f[j].rotation.inverse() * f[k].rotation);
                        format!("{k}:{:.2}", rel(&fr).angle_between(rel(&tg)))
                    }).collect();
                    println!("   t {t:.2} pelvis {:.2}/{:.2} joint errors {}", fr[0].translation.z, tg[0].translation.z, errs.join(" "));
                }
            }
            Some(tg) if !tg.is_empty() => {
                if first.is_none() {
                    let off = tg.iter().zip(&lay).map(|(a, b)| a.translation.distance(b.translation)).fold(0.0f32, f32::max);
                    first = Some(off);
                }
                // the game moves the ragdoll there (as its bones are driven to them)
                let fr: Vec<Transform> = b.agent.doll.frames().collect();
                for (k, f) in tg.iter().enumerate() {
                    let v = (f.translation - fr[k].translation) / DT;
                    b.agent.doll.set_body(k, f.translation, f.rotation, v, Vec3::ZERO);
                }
                last_targets = tg;
            }
            Some(_) => {}
            None => {
                if ended_at.is_none() {
                    ended_at = Some(t);
                    let (p, h, f) = heights(&frames(sim, id));
                    println!("   ended at {t:.2} s: pelvis {p:.2}, head {h:.2}, feet {f:.2}; balancing {}", sim.bodies[&id].agent.balancing());
                }
                if t - ended_at.unwrap() > 3.0 {
                    break;
                }
            }
        }
    }
    if mode == "animation" {
        let start_ok = first.is_some_and(|d| d < 0.12);
        println!("   {}  the animation starts where it lies [{:.3} m]", if start_ok { "PASS" } else { "FAIL" }, first.unwrap_or(f32::NAN));
        let (p, h, f) = if last_targets.is_empty() { (0.0, 0.0, 0.0) } else { heights(&last_targets) };
        let end_ok = h > 1.45 && p > 0.85 && f.abs() < 0.2;
        println!("   {}  ... and ends standing on the floor [pelvis {p:.2}, head {h:.2}, feet {f:.2}]", if end_ok { "PASS" } else { "FAIL" });
        ok &= start_ok && end_ok;
    }
    let (p, h, _) = heights(&frames(sim, id));
    let up = h > 1.35 && p > 0.8;
    println!("   {}  standing 3 s after (its own muscles) [pelvis {p:.2}, head {h:.2}; ended {}]", if up { "up" } else { "DOWN" },
             ended_at.map_or("never".into(), |t| format!("{t:.2} s")));
    if mode == "animation" {
        ok &= up && ended_at.is_some();
    }
    sim.bodies.remove(&id);
    ok
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(r"J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV".into());
    let net = std::env::args().nth(2).unwrap_or(r"J:\SteamLibrary\steamapps\common\GarrysMod\garrysmod\addons\delirium-physics-main\getup.net".into());
    let mut sim = Sim::start(&dir).expect("start");
    if std::env::var("BIGFLOOR").is_ok() {
        let p = [[-30.0, -30.0, 0.0], [30.0, -30.0, 0.0], [30.0, 30.0, 0.0], [-30.0, 30.0, 0.0]];
        sim.world.add_chunk(1, &p, &[[0, 1, 2], [0, 2, 3]], &[0, 0]);
    } else {
        sim.world.add_floor(0.0, 0.0, 0.0, 50.0, 0);
    }
    println!("clips: {}, getup.net: {}", gmsv_delirium::getup::available(), gmsv_delirium::getup::has_net(&net));
    let mut failed = 0;
    {
        let h = &sim.homer;
        let b = &h.build;
        let mut worst = (0.0f32, 0.0f32);
        for (i, p) in b.parts.iter().enumerate() {
            let Some(j) = p.parent else { continue };
            let zb = b.parts[j].bind.inverse() * p.bind;
            let zf = h.frames[j].rotation.inverse() * h.frames[i].rotation;
            worst.0 = worst.0.max(zb.angle_between(zf));
            worst.1 = worst.1.max(p.rest.angle_between(zf));
        }
        println!("zero from bind vs standing frames: {:.3} rad; rest vs frames: {:.3} rad", worst.0, worst.1);
    }
    if std::env::var("ROOMSTEP").is_ok() {
        // the network as the viewer runs it: a fresh body from how it lies, its own step
        let net = gmsv_delirium::getup::Net::load(&net).expect("net");
        for back in [false, true] {
            let id = knock_down(&mut sim, back);
            let lay = frames(&sim, id);
            let b = &sim.bodies[&id];
            let build = b.build.clone();
            let zero: Vec<Quat> = build.parts.iter().map(|p| p.parent.map_or(p.bind, |j| build.parts[j].bind.inverse() * p.bind)).collect();
            let mut a = delirium::nm::Agent::new(&build, delirium::ragdoll::Ragdoll::new(&build, &lay, Vec3::ZERO), Quat::IDENTITY, 0.3);
            let mut best = 0.0f32;
            for _ in 0..480 {
                let act = net.forward(&gmsv_delirium::getup::observe(&a, 0.0));
                gmsv_delirium::getup::drive(&mut a, &act, &zero, &build);
                a.doll.kick(Vec3::ZERO);
                a.step(&build, &sim.world, 1.0 / 60.0, &[], &[]);
                best = best.max(a.doll.frames().nth(HEAD).unwrap().translation.z);
            }
            let (p, h, f) = heights(&a.doll.frames().collect::<Vec<_>>());
            println!("roomstep {}: pelvis {p:.2} head {h:.2} feet {f:.2} (best head {best:.2})", if back { "back" } else { "front" });
            sim.bodies.remove(&id);
        }
        return;
    }
    let only = std::env::var("MODE").unwrap_or_default();
    for back in [false, true] {
        if (only.is_empty() || only == "animation") && !run(&mut sim, back, "animation", "") {
            failed += 1;
        }
        if only.is_empty() || only == "muscles" {
            run(&mut sim, back, "muscles", "");
        }
        if only.is_empty() || only == "learned" {
            run(&mut sim, back, "learned", &net);
        }
    }
    let h = &sim.homer;
    let low = h.build.parts.iter().zip(&h.frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
    println!("homer standing: lowest {low:.3}, feet {:.3} {:.3}, pelvis {:.3}", h.frames[3].translation.z, h.frames[6].translation.z, h.frames[0].translation.z);
    println!("{}", if failed == 0 { "animation: all passed".to_string() } else { format!("animation: {failed} FAILED") });
}
