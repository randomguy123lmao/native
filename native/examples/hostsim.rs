//! The module's host step (host.rs) against a stand-in for the game's physics: a "ragdoll" of 15 bones made of
//! Homer's own parts (a bone's frame is its part's), as a ValveBiped ragdoll has them. Checks the body is put together
//! from the bones as it stands, the muscles handed back (a composite joint's too) and the forces.
//! usage: hostsim [GTA IV folder]

use delirium::prelude::*;
use gmsv_delirium::host::{Between, BoneDef, Physics};
use gmsv_delirium::lua::Val;
use gmsv_delirium::sim::Sim;
use std::cell::RefCell;
use std::ffi::c_void;

const M: f32 = 0.0254;

#[derive(Default)]
struct Fake {
    bones: Vec<[f32; 13]>,
    mass: Vec<f32>,
    muscles: Vec<(usize, usize, Quat, f32, f32)>,
    pushes: Vec<(usize, Vec3, Vec3)>,
    /// the world's gravity, units/s/s (none: 0)
    gravity: Option<f32>,
}

thread_local! {
    static FAKE: RefCell<Fake> = RefCell::new(Fake::default());
}

fn idx(p: *mut c_void) -> Option<usize> {
    let i = p as usize;
    (i >= 1).then(|| i - 1)
}

unsafe extern "C" fn state(p: *mut c_void, out: *mut f32) -> i32 {
    FAKE.with(|f| {
        let f = f.borrow();
        let Some(s) = idx(p).and_then(|i| f.bones.get(i)) else { return 0 };
        for (k, x) in s.iter().enumerate() {
            unsafe { *out.add(k) = *x };
        }
        1
    })
}

unsafe extern "C" fn mass(p: *mut c_void, m: *mut f32, g: *mut f32) -> i32 {
    FAKE.with(|f| {
        let f = f.borrow();
        let Some(x) = idx(p).and_then(|i| f.mass.get(i)) else { return 0 };
        unsafe {
            *m = *x;
            *g = 0.0;
            *g.add(1) = 0.0;
            *g.add(2) = -f.gravity.unwrap_or(600.0);
        }
        1
    })
}

unsafe extern "C" fn muscle(a: *mut c_void, b: *mut c_void, q: *const f32, hz: f32, d: f32) -> i32 {
    let q = unsafe { Quat::from_xyzw(*q, *q.add(1), *q.add(2), *q.add(3)) };
    FAKE.with(|f| f.borrow_mut().muscles.push((idx(a).unwrap_or(0), idx(b).unwrap_or(0), q, hz, d)));
    1
}

unsafe extern "C" fn push(p: *mut c_void, fo: *const f32, to: *const f32) -> i32 {
    let (f, t) = unsafe { (Vec3::new(*fo, *fo.add(1), *fo.add(2)), Vec3::new(*to, *to.add(1), *to.add(2))) };
    FAKE.with(|x| x.borrow_mut().pushes.push((idx(p).unwrap_or(0), f, t)));
    1
}

fn rot(i: usize) -> Quat {
    FAKE.with(|f| {
        let b = f.borrow().bones[i];
        Quat::from_xyzw(b[3], b[4], b[5], b[6])
    })
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(r"J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV".into());
    let mut sim = Sim::start(&dir).expect("start");
    sim.physics = Some(Ok(Physics::custom(state, mass, muscle, push)));
    sim.world.add_floor(0.0, 0.0, 0.0, 50.0, 0);
    let mut failed = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        if !ok {
            failed += 1;
        }
        println!("{}  {name}   [{detail}]", if ok { "PASS" } else { "FAIL" });
    };
    // a body standing in its rest pose; its "ragdoll": the parts a ValveBiped ragdoll has bones for
    let id = sim.create("", Vec3::ZERO, 0.0, 0.3);
    let frames: Vec<Transform> = sim.bodies[&id].agent.doll.frames().collect();
    let parents: Vec<Option<usize>> = sim.bodies[&id].build.parts.iter().map(|p| p.parent).collect();
    let carried = [0usize, 1, 2, 3, 4, 5, 6, 10, 12, 14, 15, 16, 18, 19, 20];
    let parent_of = |k: usize| -> Option<usize> {
        let mut p = parents[k];
        while let Some(x) = p {
            if carried.contains(&x) {
                return Some(x);
            }
            p = parents[x];
        }
        None
    };
    let bones: Vec<Option<BoneDef>> = carried
        .iter()
        .map(|&k| Some(BoneDef { part: k, parent: parent_of(k).map(|p| carried.iter().position(|&c| c == p).unwrap()), offset: Quat::IDENTITY }))
        .collect();
    let between = vec![
        Between { part: 7, a: 0, d: 10, t: 0.25 },
        Between { part: 8, a: 0, d: 10, t: 0.5 },
        Between { part: 9, a: 0, d: 10, t: 0.75 },
        Between { part: 11, a: 10, d: 12, t: 0.5 },
        Between { part: 13, a: 10, d: 14, t: 0.0 },
        Between { part: 17, a: 10, d: 18, t: 0.0 },
    ];
    let masses: Vec<f32> = carried.iter().map(|&k| sim.bodies[&id].agent.doll.part_mass(k) * 1.2).collect();
    // bone origins: each part's centre (the pelvis's too: no root offset)
    FAKE.with(|f| {
        let mut f = f.borrow_mut();
        f.bones = carried
            .iter()
            .map(|&k| {
                let t = frames[k];
                let p = t.translation / M;
                [p.x, p.y, p.z, t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
            })
            .collect();
        f.mass = masses.clone();
    });
    let phys: Vec<*mut c_void> = (1..=carried.len()).map(|i| i as *mut c_void).collect();
    let r = sim.host(id, phys, bones, between, Vec3::ZERO);
    check("hosted", r.is_ok(), format!("{r:?}"));
    let sent = sim.send(id, "bodyBalance", vec![("start".into(), Val::Bool(true))]);
    check("balance started", sent.is_ok(), String::new());
    sim.step(1.0 / 60.0);
    // the body was put together from the bones: as it stood (its parts read back before its own step moved them)
    let put: Vec<Transform> = sim.bodies[&id].agent.doll.frames().collect();
    let worst_put = put.iter().zip(&frames).map(|(a, b)| a.translation.distance(b.translation)).fold(0.0f32, f32::max);
    check("put together from the bones where it stood (after one step of its own)", worst_put < 0.05, format!("{worst_put:.3} m"));
    FAKE.with(|f| {
        let mut f = f.borrow_mut();
        f.muscles.clear();
        f.pushes.clear();
    });
    sim.step(1.0 / 60.0);
    let (muscles, pushes) = FAKE.with(|f| {
        let f = f.borrow();
        (f.muscles.clone(), f.pushes.clone())
    });
    check("a muscle for each of the ragdoll's 14 joints", muscles.len() == 14, format!("{}", muscles.len()));
    // standing at rest, balancing: each muscle's target close to the joint as it is (bones = parts here)
    let mut worst = (0.0f32, 0usize);
    for &(a, b, q, _, _) in &muscles {
        let now = rot(a).inverse() * rot(b);
        let off = q.angle_between(now);
        if off > worst.0 {
            worst = (off, carried[b]);
        }
    }
    check("the muscles' targets near the joints as they stand (rest pose, balancing)", worst.0 < 0.35, format!("worst {:.3} rad (part {})", worst.0, worst.1));
    let spine = muscles.iter().find(|m| carried[m.1] == 10).copied();
    check("the spine's four joints as one, pelvis to chest", spine.is_some_and(|s| carried[s.0] == 0 && s.3 > 0.5),
          format!("{:?}", spine.map(|s| (carried[s.0], s.3, s.4))));
    let hz: Vec<f32> = muscles.iter().map(|m| m.3).collect();
    check("stiffness as a frequency, 0.3 to 5 Hz", hz.iter().all(|&h| (0.3..5.0).contains(&h)), format!("{hz:.2?}"));
    // a hurt leg (the engine's injury): its muscles weaker on the game's ragdoll too - an injury of 0.75 leaves a
    // quarter of the strength, half the stiffness
    let _ = sim.send(id, "injury", vec![("mask".into(), Val::Str("ll".into())), ("amount".into(), Val::Num(0.75))]);
    FAKE.with(|f| f.borrow_mut().muscles.clear());
    sim.step(1.0 / 60.0);
    let ms = FAKE.with(|f| f.borrow().muscles.clone());
    let hz_of = |part: usize| ms.iter().find(|m| carried[m.1] == part).map_or(0.0, |m| m.3);
    let (hurt, good) = (hz_of(2), hz_of(5));
    check("a hurt leg: its knee half as stiff on the game's ragdoll", good > 0.0 && (0.4..0.6).contains(&(hurt / good)),
          format!("{hurt:.2} Hz against {good:.2}"));
    let _ = sim.send(id, "injury", vec![("mask".into(), Val::Str("ll".into())), ("amount".into(), Val::Num(0.0))]);
    FAKE.with(|f| f.borrow_mut().muscles.clear());
    sim.step(1.0 / 60.0);
    let ms = FAKE.with(|f| f.borrow().muscles.clone());
    let healed = ms.iter().find(|m| carried[m.1] == 2).map_or(0.0, |m| m.3);
    check("... healed (0): as strong as the other again", (healed - good).abs() < good * 0.1, format!("{healed:.2} Hz"));
    let up: f32 = pushes.iter().map(|p| p.1.z).sum();
    let total: f32 = masses.iter().sum();
    check("the heavier gravity made up (with the behaviours' forces): pushed up about m (g - 9.81)", up > total * (600.0 * M - 9.81) * 0.5,
          format!("{up:.0} N up, {total:.0} kg"));
    // lying on its back, aiming a gun (pointArm): its arm held up against gravity off the feet - the joint springs alone
    // can't hold up an arm's whole weight - as a muscle's torque, the pair on the upper arm and the chest
    let pelvis = frames[0].translation;
    let left = (frames[1].translation - frames[4].translation).normalize();
    let tip = Quat::from_axis_angle(left, -std::f32::consts::FRAC_PI_2);
    FAKE.with(|f| {
        let mut f = f.borrow_mut();
        f.bones = carried
            .iter()
            .map(|&k| {
                let t = frames[k];
                let p = (tip * (t.translation - pelvis) + Vec3::new(pelvis.x, pelvis.y, 0.16)) / M;
                let q = tip * t.rotation;
                [p.x, p.y, p.z, q.x, q.y, q.z, q.w, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
            })
            .collect();
    });
    let _ = sim.send(id, "stopAllBehaviours", vec![]);
    for _ in 0..20 {
        sim.step(1.0 / 60.0);
    }
    let sh = frames[18].translation;
    let target = tip * (sh - pelvis) + Vec3::new(pelvis.x, pelvis.y, 0.16) + Vec3::new(0.0, 0.0, 1.0) + (tip * Vec3::Y) * 2.0;
    let aim = sim.send(id, "pointArm", vec![("start".into(), Val::Bool(true)), ("target".into(), Val::Vec(target.to_array())), ("useLeftArm".into(), Val::Bool(false)),
                                             ("armStiffness".into(), Val::Num(14.0)), ("armStraightness".into(), Val::Num(2.0))]);
    check("pointArm taken (lying)", aim.is_ok(), format!("{aim:?}"));
    let mut arm_t = 0.0f32;
    let mut chest_t = Vec3::ZERO;
    let mut arm_v = Vec3::ZERO;
    for _ in 0..10 {
        FAKE.with(|f| f.borrow_mut().pushes.clear());
        sim.step(1.0 / 60.0);
        FAKE.with(|f| {
            for &(b, _, t) in &f.borrow().pushes {
                if carried[b] == 18 {
                    arm_t = arm_t.max(t.length());
                    arm_v = t;
                }
                if carried[b] == 10 {
                    chest_t = t;
                }
            }
        });
    }
    let arm_mass: f32 = [18usize, 19, 20].iter().map(|&k| masses[carried.iter().position(|&c| c == k).unwrap()]).sum();
    let need = arm_mass * 9.81 * 0.15;
    check("lying, aiming: the upper arm held up against gravity (a torque about its weight times a lever)", arm_t > need,
          format!("{arm_t:.1} N m on the upper arm, its arm's weight {:.0} N; chest {:.1} N m, against it {:.2}", arm_mass * 9.81, chest_t.length(),
                  if arm_v.length() > 0.0 && chest_t.length() > 0.0 { arm_v.normalize().dot(chest_t.normalize()) } else { 0.0 }));
    // no gravity at all (sv_gravity 0, changed while it's up): standing, balancing, it isn't pushed up - the support it
    // gets is against the world's gravity as it is, none
    FAKE.with(|f| {
        let mut f = f.borrow_mut();
        f.bones = carried
            .iter()
            .map(|&k| {
                let t = frames[k];
                let p = t.translation / M;
                [p.x, p.y, p.z, t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
            })
            .collect();
        f.gravity = Some(0.0);
    });
    let _ = sim.send(id, "stopAllBehaviours", vec![]);
    let _ = sim.send(id, "bodyBalance", vec![("start".into(), Val::Bool(true))]);
    let mut up0 = 0.0f32;
    for _ in 0..10 {
        FAKE.with(|f| f.borrow_mut().pushes.clear());
        sim.step(1.0 / 60.0);
        up0 = FAKE.with(|f| f.borrow().pushes.iter().map(|p| p.1.z).sum());
    }
    check("no gravity: a balancing body isn't pushed up (no lift, no make-up against gravity there isn't)", up0.abs() < total * 0.5,
          format!("{up0:.0} N up, {total:.0} kg (with gravity: {up:.0} N)"));
    FAKE.with(|f| f.borrow_mut().gravity = None);
    // the world's gravity as ever, the ragdoll's own turned off (the physgun, "no gravity"): not pushed up either
    sim.bodies.get_mut(&id).unwrap().host.as_mut().unwrap().settings.gravity = 0.0;
    let mut up1 = 0.0f32;
    for _ in 0..10 {
        FAKE.with(|f| f.borrow_mut().pushes.clear());
        sim.step(1.0 / 60.0);
        up1 = FAKE.with(|f| f.borrow().pushes.iter().map(|p| p.1.z).sum());
    }
    check("its own gravity off (the world's as ever): a balancing body isn't pushed up", up1.abs() < total * 0.5,
          format!("{up1:.0} N up, {total:.0} kg (with gravity: {up:.0} N)"));
    sim.bodies.get_mut(&id).unwrap().host.as_mut().unwrap().settings.gravity = 1.0;
    // falling: what the module adds to a body as it falls - in the air (GTA IV's high fall), and knocked over from
    // standing (catching the fall) - against its weight: nothing much holding it up, or it falls slower than anything
    let weight = total * 600.0 * M;
    let place = |lift: f32, vz: f32| {
        FAKE.with(|f| {
            let mut f = f.borrow_mut();
            f.bones = carried
                .iter()
                .map(|&k| {
                    let t = frames[k];
                    let p = (t.translation + Vec3::Z * lift) / M;
                    [p.x, p.y, p.z, t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w, 0.0, 0.0, vz / M, 0.0, 0.0, 0.0]
                })
                .collect();
        });
    };
    let mut measure = |name: &str, start: &dyn Fn(&mut Sim)| -> f32 {
        let _ = sim.send(id, "stopAllBehaviours", vec![]);
        start(&mut sim);
        let mut most = 0.0f32;
        let mut sum = 0.0f32;
        for _ in 0..30 {
            FAKE.with(|f| f.borrow_mut().pushes.clear());
            sim.step(1.0 / 60.0);
            let up: f32 = FAKE.with(|f| f.borrow().pushes.iter().map(|p| p.1.z).sum());
            most = most.max(up);
            sum += up;
        }
        println!("  {name}: up {:.0} N on average, at most {:.0} N, against its weight {:.0} N ({:.0}%)", sum / 30.0, most, weight, 100.0 * sum / 30.0 / weight);
        sum / 30.0
    };
    place(3.0, -3.0);
    let air = measure("in the air, the high fall", &|s: &mut Sim| {
        let Sim { bodies, world, tp, .. } = s;
        let b = bodies.get_mut(&id).unwrap();
        delirium::nm::task_high_fall(&mut b.agent, &b.build, world, tp, "*", false);
    });
    place(0.0, -3.0);
    let over = measure("knocked over standing, catching the fall", &|s: &mut Sim| {
        let Sim { bodies, tp, .. } = s;
        let b = bodies.get_mut(&id).unwrap();
        delirium::nm::task_catch_fall(&mut b.agent, tp, "*");
    });
    place(3.0, -3.0);
    let limp = measure("in the air, limp (out cold)", &|s: &mut Sim| {
        let _ = s.send(id, "bodyRelax", vec![("relaxation".into(), Val::Num(100.0))]);
    });
    // falling fast already (well into a long fall): nothing working against the speed either
    let mut fast = vec![];
    for v in [10.0f32, 20.0, 30.0] {
        place(10.0, -v);
        let got = measure(&format!("in the air at {v} m/s, the high fall"), &|s: &mut Sim| {
            let Sim { bodies, world, tp, .. } = s;
            let b = bodies.get_mut(&id).unwrap();
            delirium::nm::task_high_fall(&mut b.agent, &b.build, world, tp, "*", false);
        });
        fast.push(got);
    }
    check("falling fast (10 to 30 m/s): the module doesn't hold it up either", fast.iter().all(|&u| u < weight * 0.2), format!("{fast:.0?} N; weight {weight:.0} N"));
    // balancing when it left the ground (walked off a roof, shot in a jump): the support while it still balances
    place(0.0, -3.0);
    let _ = sim.send(id, "stopAllBehaviours", vec![]);
    let _ = sim.send(id, "bodyBalance", vec![("start".into(), Val::Bool(true))]);
    for _ in 0..30 {
        sim.step(1.0 / 60.0);
    }
    place(3.0, -3.0);
    let mut held = vec![];
    for i in 0..60 {
        FAKE.with(|f| f.borrow_mut().pushes.clear());
        sim.step(1.0 / 60.0);
        let up: f32 = FAKE.with(|f| f.borrow().pushes.iter().map(|p| p.1.z).sum());
        if i % 6 == 0 {
            held.push(format!("{:.0}", 100.0 * up / weight));
        }
    }
    println!("  balancing, then 3 m up in the air: held up (% of its weight, every 0.1 s): {}  balancing: {}", held.join(" "), sim.bodies[&id].agent.balancing());
    check("falling, the module doesn't hold it up (under a fifth of its weight)", air < weight * 0.2 && over < weight * 0.2 && limp < weight * 0.2,
          format!("high fall {air:.0} N, catching the fall {over:.0} N, limp {limp:.0} N; weight {weight:.0} N"));
    // the ragdoll gone: lost
    FAKE.with(|f| f.borrow_mut().bones.clear());
    sim.step(1.0 / 60.0);
    check("its ragdoll gone: lost", sim.bodies[&id].host.as_ref().is_some_and(|h| h.lost), String::new());
    println!("{}", if failed == 0 { "all passed".to_string() } else { format!("{failed} FAILED") });
}
