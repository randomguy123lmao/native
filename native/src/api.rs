//! The Lua table `delirium` (built with the "gmod" feature, the default).

use crate::lua::{self, Lua, State, Val};
use crate::sim::{self, SIM, Sim};
use crate::{custom, lua_fn};
use delirium::nm;
use delirium::prelude::*;
use delirium::ragdoll::Obstacle;
use std::ffi::c_int;

fn with<R>(f: impl FnOnce(&mut Sim) -> Result<R, String>) -> Result<R, String> {
    SIM.with(|s| match s.borrow_mut().as_mut() {
        Some(sim) => f(sim),
        None => Err("delirium: not started (delirium.Init with the GTA IV folder)".into()),
    })
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

fn frames7(frames: impl Iterator<Item = Transform>) -> Vec<f32> {
    frames.flat_map(|f| {
        let (t, r) = (f.translation, f.rotation);
        [t.x, t.y, t.z, r.x, r.y, r.z, r.w]
    }).collect()
}

lua_fn!(init, |l| {
    let dir = l.str_arg(1)?;
    let already = SIM.with(|s| s.borrow().is_some());
    if already {
        l.push_bool(true);
        return Ok(1);
    }
    let sim = Sim::start(&dir)?;
    SIM.with(|s| *s.borrow_mut() = Some(sim));
    l.push_bool(true);
    Ok(1)
});

lua_fn!(ready, |l| {
    l.push_bool(SIM.with(|s| s.borrow().is_some()));
    Ok(1)
});

lua_fn!(shutdown, |l| {
    SIM.with(|s| *s.borrow_mut() = None);
    l.push_bool(true);
    Ok(1)
});

lua_fn!(info, |l| {
    with(|s| {
        l.new_table();
        l.field_num(c"parts", s.homer.build.parts.len() as f64);
        l.field_num(c"surfaces", s.surfaces.len() as f64);
        l.field_bool(c"taskparams", s.tp_found);
        l.field_num(c"bodies", s.bodies.len() as f64);
        l.field_num(c"triangles", s.world.len() as f64);
        l.field_num(c"restYaw", s.rest_yaw as f64);
        l.field_str(c"game", &s.game.dir().display().to_string());
        l.push_strings(&custom::names());
        l.set_field(c"behaviours");
        Ok(1)
    })
});

lua_fn!(part_names, |l| {
    let names: Vec<&str> = delirium_data::nm_asset::PARTS.iter().copied().collect();
    l.push_strings(&names);
    Ok(1)
});

lua_fn!(surfaces, |l| {
    with(|s| {
        let names: Vec<&str> = s.surfaces.iter().map(|x| x.name.as_str()).collect();
        l.push_strings(&names);
        Ok(1)
    })
});

lua_fn!(rest_pose, |l| {
    let key = l.string(1).unwrap_or_default();
    with(|s| {
        l.push_floats(&frames7(s.body_def(&key).frames.iter().copied()));
        Ok(1)
    })
});

lua_fn!(part_info, |l| {
    let key = l.string(1).unwrap_or_default();
    with(|s| {
        let b = &s.body_def(&key).build;
        // mass, radius, half length, parent, then the joint's anchor in the part's frame (x, y, z)
        let f: Vec<f32> = b.parts.iter().flat_map(|p| {
            [p.mass * p.size.powi(3) * p.heft, p.radius, p.half, p.parent.map_or(-1.0, |x| x as f32), p.anchor.x, p.anchor.y, p.anchor.z]
        }).collect();
        l.push_floats(&f);
        Ok(1)
    })
});

lua_fn!(fit, |l| {
    let key = l.str_arg(1)?;
    let len = l.floats(2)?;
    let mass = l.f32_or(3, 1.0);
    with(|s| s.fit(&key, &len, mass))?;
    l.push_bool(true);
    Ok(1)
});

lua_fn!(has_build, |l| {
    let key = l.str_arg(1)?;
    with(|s| {
        l.push_bool(s.builds.contains_key(&key));
        Ok(1)
    })
});

lua_fn!(create, |l| {
    let key = l.string(1).unwrap_or_default();
    let pos = l.vec3(2)?;
    let yaw = l.f32(5)?;
    let seed = l.f32_or(6, 0.3);
    let id = with(|s| Ok(s.create(&key, v3(pos), yaw, seed)))?;
    l.push_num(id as f64);
    Ok(1)
});

lua_fn!(destroy, |l| {
    let id = l.int(1)?;
    let gone = with(|s| {
        s.unhost(id);
        s.keys.remove(&id);
        Ok(s.bodies.remove(&id).is_some())
    })?;
    l.push_bool(gone);
    Ok(1)
});

lua_fn!(count, |l| {
    let n = with(|s| Ok(s.bodies.len()))?;
    l.push_num(n as f64);
    Ok(1)
});

lua_fn!(step, |l| {
    let dt = l.f32(1)?;
    if !(dt > 0.0 && dt <= 0.25) {
        return Err("delirium.Step: dt in (0, 0.25]".into());
    }
    with(|s| {
        s.step(dt);
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(parts, |l| {
    let id = l.int(1)?;
    with(|s| {
        let b = s.body(id)?;
        l.push_floats(&frames7(b.agent.doll.frames()));
        Ok(1)
    })
});

lua_fn!(motion, |l| {
    let id = l.int(1)?;
    with(|s| {
        let b = s.body(id)?;
        let n = b.build.parts.len();
        let f: Vec<f32> = (0..n).flat_map(|k| {
            let (v, w) = b.agent.doll.motion(k);
            [v.x, v.y, v.z, w.x, w.y, w.z]
        }).collect();
        l.push_floats(&f);
        Ok(1)
    })
});

lua_fn!(set_parts, |l| {
    let id = l.int(1)?;
    let f = l.floats(2)?;
    with(|s| {
        let b = s.body(id)?;
        let n = b.build.parts.len();
        if f.len() != n * 13 {
            return Err(format!("delirium.SetParts: {} numbers, 13 a part", n * 13));
        }
        for (k, c) in f.chunks(13).enumerate() {
            let q = Quat::from_xyzw(c[3], c[4], c[5], c[6]);
            if !(c.iter().all(|x| x.is_finite()) && q.length() > 0.5) {
                return Err(format!("delirium.SetParts: part {k} isn't a usable pose"));
            }
            b.agent.doll.set_body(k, Vec3::new(c[0], c[1], c[2]), q, Vec3::new(c[7], c[8], c[9]), Vec3::new(c[10], c[11], c[12]));
        }
        Ok(())
    })?;
    l.push_bool(true);
    Ok(1)
});

lua_fn!(state_of, |l| {
    let id = l.int(1)?;
    with(|s| {
        let b = s.body(id)?;
        let a = &b.agent;
        l.new_table();
        l.field_bool(c"balancing", a.balancing());
        l.field_bool(c"fallen", a.fallen);
        l.field_bool(c"settled", a.settled());
        l.field_bool(c"relaxing", a.relaxing());
        l.field_bool(c"catching", a.catching());
        l.field_bool(c"shot", a.shot_running());
        l.field_bool(c"rolledUp", a.rolled_up());
        l.field_bool(c"grabbing", a.grabbing());
        let [grip_l, grip_r] = a.gripping();
        l.field_bool(c"gripLeft", grip_l);
        l.field_bool(c"gripRight", grip_r);
        l.field_bool(c"awake", a.doll.awake());
        l.field_bool(c"hosted", b.host.is_some());
        l.field_bool(c"lost", b.host.as_ref().is_some_and(|h| h.lost));
        let custom: Vec<&str> = b.custom.iter().map(|c| c.name()).collect();
        l.push_strings(&custom);
        l.set_field(c"behaviours");
        // (the engine's own extra behaviours running: injuredOnGround, clutchWound, learnedCrawl...)
        let extras: Vec<&str> = a.extras.on.iter().map(|(n, _)| *n).collect();
        l.push_strings(&extras);
        l.set_field(c"extras");
        if let Some(g) = b.custom.iter().find_map(|c| c.as_get_up()) {
            l.field_str(c"getUp", g.mode_name());
            l.field_num(c"getUpProgress", g.progress() as f64);
            l.field_bool(c"getUpStanding", g.standing_up());
        }
        if let Some(p) = a.stance() {
            l.push_floats(&[p.x, p.y, p.z]);
            l.set_field(c"stance");
        }
        Ok(1)
    })
});

// HostReady(): true, or false and why not - the game's physics is VPhysics-Box3D with Delirium's functions.
lua_fn!(host_ready, |l| {
    // (asked before Init too: the game's physics is there or not, started or not)
    let found = SIM.with(|s| s.borrow_mut().as_mut().map(|s| s.physics())).unwrap_or_else(crate::host::Physics::find);
    match found {
        Ok(_) => {
            l.push_bool(true);
            Ok(1)
        }
        Err(e) => {
            l.push_bool(false);
            l.push_str(&e);
            Ok(2)
        }
    }
});

// Host(id, physobjs, bones, between, rootOffset): character id is the brain of the game's ragdoll. physobjs: its
// physics objects (by physics bone, 1-based); bones: 6 numbers a physics bone - the part it carries (-1 none), its
// parent bone (-1 none), and the part's rotation in the bone's frame (x y z w); between: 4 numbers a part with no
// bone - the part, the parts it lies between (a, d) and how far from a (0-1); rootOffset: the pelvis part's centre
// from the pelvis bone, in the part's frame (metres). True, or nil and why not.
lua_fn!(host, |l| {
    let id = l.int(1)?;
    with(|s| s.physics().map(|_| ()))?;
    let phys = l.objects(2)?;
    let b = l.floats(3)?;
    let w = if l.kind(4) == lua::TTABLE { l.floats(4)? } else { vec![] };
    let root = v3(l.vec3(5).unwrap_or([0.0; 3]));
    if b.len() != phys.len() * 6 {
        return Err(format!("delirium.Host: 6 numbers a physics bone ({} objects, {} numbers)", phys.len(), b.len()));
    }
    if phys.iter().any(|p| p.is_null()) {
        return Err("delirium.Host: physics objects expected".into());
    }
    let bones = b.chunks(6).map(|c| {
        (c[0] >= 0.0).then(|| crate::host::BoneDef {
            part: c[0] as usize,
            parent: (c[1] >= 0.0).then_some(c[1] as usize),
            offset: Quat::from_xyzw(c[2], c[3], c[4], c[5]).normalize(),
        })
    }).collect();
    let between = w.chunks(4).filter(|c| c.len() == 4).map(|c| crate::host::Between { part: c[0] as usize, a: c[1] as usize, d: c[2] as usize, t: c[3] }).collect();
    with(|s| s.host(id, phys, bones, between, root))?;
    l.push_bool(true);
    Ok(1)
});

// Unhost(id): lets go of the game's ragdoll (its muscles off).
lua_fn!(unhost, |l| {
    let id = l.int(1)?;
    with(|s| {
        s.unhost(id);
        Ok(())
    })?;
    Ok(0)
});

// HostSet(id, {muscles = bool, strength = number, gravityFix = 0/1/2, hold = number, damping = number, gravity = 0-1}, loose?):
// how the
// ragdoll is driven now (see host::Settings). loose: the physics bones (0-based) another addon moves itself just now -
// their joints' muscles let go.
lua_fn!(host_set, |l| {
    let id = l.int(1)?;
    let f = l.fields(2)?;
    let loose = if l.kind(3) == lua::TTABLE { Some(l.floats(3)?) } else { None };
    with(|s| {
        let b = s.body(id)?;
        let h = b.host.as_mut().ok_or("delirium.HostSet: not on a ragdoll")?;
        for (k, v) in &f {
            match (k.as_str(), v) {
                ("muscles", Val::Bool(x)) => h.settings.muscles = *x,
                ("strength", Val::Num(x)) => h.settings.strength = *x as f32,
                ("gravityFix", Val::Bool(x)) => h.settings.gravity_fix = if *x { 1 } else { 0 },
                ("gravityFix", Val::Num(x)) => h.settings.gravity_fix = (*x as i32).clamp(0, 2) as u8,
                ("hold", Val::Num(x)) => h.settings.hold = *x as f32,
                ("damping", Val::Num(x)) => h.settings.damping = *x as f32,
                ("gravity", Val::Num(x)) => h.settings.gravity = (*x as f32).clamp(0.0, 1.0),
                ("gravity", Val::Bool(x)) => h.settings.gravity = if *x { 1.0 } else { 0.0 },
                _ => {}
            }
        }
        if let Some(loose) = loose {
            h.settings.loose.iter_mut().for_each(|x| *x = false);
            for i in loose {
                if let Some(x) = h.settings.loose.get_mut(i as usize) {
                    *x = true;
                }
            }
        }
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(feedback, |l| {
    let id = l.int(1)?;
    with(|s| {
        let b = s.body(id)?;
        l.new_table();
        for (k, f) in b.agent.feedback.iter().enumerate() {
            l.new_table();
            l.field_str(c"name", f.name);
            l.field_num(c"slot", f.slot as f64);
            l.field_num(c"value", f.value as f64);
            l.set_index(k as i32 + 1);
        }
        Ok(1)
    })
});

lua_fn!(message, |l| {
    let id = l.int(1)?;
    let name = l.str_arg(2)?;
    let params = l.fields(3)?;
    let unused = with(|s| s.send(id, &name, params))?;
    l.push_num(unused as f64);
    Ok(1)
});

fn part_of(b: &sim::Body, k: i32) -> Result<usize, String> {
    (0..b.build.parts.len() as i32).contains(&k).then_some(k as usize).ok_or_else(|| format!("delirium: no part {k}"))
}

lua_fn!(impulse, |l| {
    let id = l.int(1)?;
    let k = l.int(2)?;
    let at = l.vec3(3)?;
    let imp = l.vec3(6)?;
    with(|s| {
        let b = s.body(id)?;
        let k = part_of(b, k)?;
        b.agent.doll.impulse(k, v3(at), v3(imp));
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(push, |l| {
    let id = l.int(1)?;
    let at = l.vec3(2)?;
    let imp = l.vec3(5)?;
    with(|s| {
        let b = s.body(id)?;
        b.agent.doll.push(&b.build, v3(at), v3(imp));
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(kick, |l| {
    let id = l.int(1)?;
    let dv = l.vec3(2)?;
    with(|s| {
        s.body(id)?.agent.doll.kick(v3(dv));
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(shot, |l| {
    let id = l.int(1)?;
    let set = l.string(2).unwrap_or_else(|| "*".into());
    let k = l.int(3)?;
    let at = l.vec3(4)?;
    let dir = v3(l.vec3(7)?).normalize_or_zero();
    let health = l.f32_or(10, 200.0);
    let force = l.f32_or(11, 400.0);
    // (a killing shot: the game's death reaction)
    let dying = l.boolean(12);
    with(|s| {
        let Sim { bodies, world, tp, rand, .. } = s;
        let b = bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        let k = part_of(b, k)?;
        // (its gun hands stay on the gun: the hit doesn't forget it)
        let hit = nm::Hit { health, dying, gun: b.gun.0, two_handed: b.gun.1, ..nm::Hit::ragdoll(&b.agent) };
        nm::task_shot(&mut b.agent, &b.build, world, tp, rand, &set, hit, k, v3(at), dir * force.max(1.0), -dir, false);
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(flinch, |l| {
    let id = l.int(1)?;
    let set = l.string(2).unwrap_or_else(|| "*".into());
    let from = l.vec3(3)?;
    with(|s| {
        let Sim { bodies, world, tp, rand, .. } = s;
        let b = bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        nm::task_flinch(&mut b.agent, &b.build, world, tp, rand, &set, v3(from));
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(balance, |l| {
    let id = l.int(1)?;
    let set = l.string(2).unwrap_or_else(|| "*".into());
    let look = l.vec3(3).ok().map(v3);
    with(|s| {
        let Sim { bodies, world, tp, .. } = s;
        let b = bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        // (standing again: no longer the body that fell, see Sim::send)
        b.agent.fallen = false;
        nm::task_balance(&mut b.agent, &b.build, world, tp, &set, look, false);
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(catch_fall, |l| {
    let id = l.int(1)?;
    let set = l.string(2).unwrap_or_else(|| "*".into());
    with(|s| {
        let Sim { bodies, tp, .. } = s;
        let b = bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        nm::task_catch_fall(&mut b.agent, tp, &set);
        Ok(())
    })?;
    Ok(0)
});

// HighFall(id, set): GTA IV's fall from a height, as its task starts it (its tuning): the arms windmilling, the legs
// pedalling, turned for the landing and rolled out of it.
lua_fn!(high_fall, |l| {
    let id = l.int(1)?;
    let set = l.string(2).unwrap_or_else(|| "*".into());
    with(|s| {
        let Sim { bodies, world, tp, .. } = s;
        let b = bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        nm::task_high_fall(&mut b.agent, &b.build, world, tp, &set, false);
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(nearest, |l| {
    let id = l.int(1)?;
    let at = l.vec3(2)?;
    let k = with(|s| {
        let b = s.body(id)?;
        Ok(b.agent.doll.nearest(&b.build, v3(at)))
    })?;
    l.push_num(k as f64);
    Ok(1)
});

lua_fn!(raycast, |l| {
    let id = l.int(1)?;
    let from = l.vec3(2)?;
    let to = l.vec3(5)?;
    let hit = with(|s| {
        let b = s.body(id)?;
        Ok(b.agent.doll.probe(&b.build, v3(from), v3(to)))
    })?;
    match hit {
        Some((k, p, n)) => {
            l.push_num(k as f64);
            l.push_floats(&[p.x, p.y, p.z]);
            l.push_floats(&[n.x, n.y, n.z]);
            Ok(3)
        }
        None => {
            l.push_num(-1.0);
            Ok(1)
        }
    }
});

// A part pulled to a point (world, metres) - held by a hand, a physgun - or let go (no point).
// Constrain(id, part, x?, y?, z?, qx?, qy?, qz?, qw?): the part held hard at that point (and turned so, given a
// rotation); nothing after the part, let go.
lua_fn!(constrain, |l| {
    let id = l.int(1)?;
    let k = l.int(2)?;
    let at = l.vec3(3).ok().map(v3);
    let turn = match (l.num(6), l.num(7), l.num(8), l.num(9)) {
        (Some(x), Some(y), Some(z), Some(w)) => Some(Quat::from_xyzw(x as f32, y as f32, z as f32, w as f32).normalize()),
        _ => None,
    };
    with(|s| {
        let b = s.body(id)?;
        let k = part_of(b, k)?;
        b.agent.doll.constrain(k, at, turn.filter(|q| q.is_finite()));
        Ok(())
    })?;
    Ok(0)
});

// The engine's joint groups (effector indices, left then right), for building animPose's pose strings.
lua_fn!(joints, |l| {
    with(|_| {
        use delirium::nmrs;
        let groups: [(&std::ffi::CStr, Vec<usize>); 9] = [
            (c"hip", nmrs::HIP().to_vec()), (c"knee", nmrs::KNEE().to_vec()), (c"ankle", nmrs::ANKLE().to_vec()),
            (c"spine", nmrs::SPINE().to_vec()), (c"neck", nmrs::NECK().to_vec()), (c"clavicle", nmrs::CLAVICLE().to_vec()),
            (c"shoulder", nmrs::SHOULDER().to_vec()), (c"elbow", nmrs::ELBOW().to_vec()), (c"wrist", nmrs::WRIST().to_vec()),
        ];
        l.new_table();
        for (name, js) in groups {
            let f: Vec<f32> = js.iter().map(|&j| j as f32).collect();
            l.push_floats(&f);
            l.set_field(name);
        }
        Ok(1)
    })
});

lua_fn!(find_gta, |l| {
    match crate::game::find() {
        Some(p) => {
            l.push_str(&p.display().to_string());
            Ok(1)
        }
        None => Ok(0),
    }
});

lua_fn!(incoming, |l| {
    let id = l.int(1)?;
    let f = if l.kind(2) == lua::TTABLE { Some(l.floats(2)?) } else { None };
    with(|s| {
        let b = s.body(id)?;
        let n = b.build.parts.len();
        b.agent.incoming = match f {
            None => vec![],
            Some(f) if f.len() == n * 7 => f.chunks(7).map(|c| {
                Transform { translation: Vec3::new(c[0], c[1], c[2]), rotation: Quat::from_xyzw(c[3], c[4], c[5], c[6]).normalize(), ..Default::default() }
            }).collect(),
            Some(_) => return Err(format!("delirium.Incoming: {} numbers, 7 a part", n * 7)),
        };
        Ok(())
    })?;
    Ok(0)
});

// GetUpTarget(id): where the get-up's animation has the parts now (7 numbers a part, as Parts), or nil - none under way,
// or not the animation (dlGetUp mode=animation: the game moves the ragdoll's bones there).
lua_fn!(get_up_target, |l| {
    let id = l.int(1)?;
    let f = with(|s| {
        let b = s.body(id)?;
        Ok(b.custom.iter().find_map(|c| c.as_get_up()).map(|g| frames7(g.targets().iter().copied())).filter(|f| !f.is_empty()))
    })?;
    match f {
        Some(f) => {
            l.push_floats(&f);
            Ok(1)
        }
        None => Ok(0),
    }
});

// CanGetUp(): GTA IV's get-up clips were found (dlGetUp's animation and muscles), and getup.net (its learned way) at
// 'net' if given.
lua_fn!(can_get_up, |l| {
    l.push_bool(crate::getup::available());
    let net = if l.kind(1) == lua::TSTRING { l.str_arg(1)? } else { String::new() };
    l.push_bool(crate::getup::has_net(&net));
    Ok(2)
});

// HoldsGun(id, weapon, twoHanded): a gun in its right hand (both, two-handed) - as the game tells its Euphoria: those
// hands stay on it, a wound is reached for with a free one.
lua_fn!(holds_gun, |l| {
    let id = l.int(1)?;
    let (gun, two) = (l.boolean(2), l.boolean(3));
    with(|s| {
        let b = s.body(id)?;
        b.gun = (gun, gun && two);
        b.agent.hold(gun, gun && two);
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(com, |l| {
    let id = l.int(1)?;
    let (x, v, m) = with(|s| {
        let b = s.body(id)?;
        let (x, v) = b.agent.doll.com();
        Ok((x, v, b.agent.doll.mass()))
    })?;
    l.push_floats(&[x.x, x.y, x.z]);
    l.push_floats(&[v.x, v.y, v.z]);
    l.push_num(m as f64);
    Ok(3)
});

lua_fn!(contacts, |l| {
    let id = l.int(1)?;
    with(|s| {
        let b = s.body(id)?;
        let f: Vec<f32> = b.agent.doll.touch.iter().map(|&t| t as f32).collect();
        l.push_floats(&f);
        Ok(1)
    })
});

lua_fn!(mesh_add, |l| {
    let key = l.num(1).ok_or("delirium.MeshAdd: a number key")? as u64;
    let pos = l.floats(2)?;
    let tri = l.floats(3)?;
    let mat = l.f32_or(4, 0.0).clamp(0.0, 255.0) as u8;
    if pos.len() % 3 != 0 || tri.len() % 3 != 0 {
        return Err("delirium.MeshAdd: positions and triangles 3 numbers each".into());
    }
    let p: Vec<[f32; 3]> = pos.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let t: Vec<[u32; 3]> = tri.chunks(3).map(|c| [c[0] as u32, c[1] as u32, c[2] as u32]).collect();
    if t.iter().flatten().any(|&i| i as usize >= p.len()) {
        return Err("delirium.MeshAdd: a triangle's corner past the positions".into());
    }
    let n = t.len();
    with(|s| {
        s.world.add_chunk(key, &p, &t, &vec![mat; n]);
        Ok(())
    })?;
    l.push_num(n as f64);
    Ok(1)
});

lua_fn!(mesh_remove, |l| {
    let key = l.num(1).ok_or("delirium.MeshRemove: a number key")? as u64;
    with(|s| {
        s.world.remove_chunk(key);
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(boxes, |l| {
    let f = l.floats(1)?;
    if f.len() % 13 != 0 {
        return Err("delirium.Boxes: 13 numbers a box".into());
    }
    with(|s| {
        s.boxes = f.chunks(13).enumerate().map(|(k, b)| {
            let c = Vec3::new(b[0], b[1], b[2]);
            let (rot, half, vel) = (Quat::from_xyzw(b[3], b[4], b[5], b[6]).normalize(), Vec3::new(b[7], b[8], b[9]), Vec3::new(b[10], b[11], b[12]));
            Obstacle { frame_end: false, id: k as i32, at: c, rot, centre: c, half, vel, spin: Vec3::ZERO, com: c, parts: &[], last: None, entity: 0,
                       first: false, inv_mass: 0.0, inv_inertia: None, force: Vec3::ZERO, turn: None }
        }).collect();
        Ok(())
    })?;
    Ok(0)
});

lua_fn!(world_clear, |l| {
    with(|s| {
        s.world = delirium::collision::Collision::default();
        s.boxes.clear();
        Ok(())
    })?;
    let _ = l;
    Ok(0)
});

lua_fn!(world_raycast, |l| {
    let from = l.vec3(1)?;
    let dir = v3(l.vec3(4)?).normalize_or_zero();
    let len = l.f32(7)?;
    let hit = with(|s| Ok(s.world.cast(v3(from), dir, len)))?;
    match hit {
        Some((t, n)) => {
            l.push_num(t as f64);
            l.push_floats(&[n.x, n.y, n.z]);
            Ok(2)
        }
        None => Ok(0),
    }
});

lua_fn!(messages, |l| {
    let mut names: Vec<&str> = delirium::msg::MESSAGES.iter().chain(delirium::extras::messages()).map(|m| m.name).collect();
    names.extend(custom::names());
    l.push_strings(&names);
    Ok(1)
});

lua_fn!(params, |l| {
    let name = l.str_arg(1)?;
    let m = delirium::msg::find(&name).ok_or_else(|| format!("delirium: no message {name}"))?;
    l.new_table();
    for (k, p) in m.params.iter().enumerate() {
        l.new_table();
        l.field_str(c"name", p.name);
        l.field_str(c"kind", match p.kind {
            delirium::msg::Kind::Float => "float",
            delirium::msg::Kind::Int => "int",
            delirium::msg::Kind::Bool => "bool",
            delirium::msg::Kind::Vec3 => "vec3",
            delirium::msg::Kind::Str => "string",
        });
        l.field_bool(c"used", p.used);
        l.field_str(c"doc", p.doc);
        l.set_index(k as i32 + 1);
    }
    Ok(1)
});

lua_fn!(version, |l| {
    l.push_str(env!("CARGO_PKG_VERSION"));
    Ok(1)
});

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gmod13_open(state: State) -> c_int {
    let l = Lua(state);
    l.register(c"delirium", &[
        (c"Init", init),
        (c"Ready", ready),
        (c"Shutdown", shutdown),
        (c"Info", info),
        (c"Version", version),
        (c"HostReady", host_ready),
        (c"Host", host),
        (c"Unhost", unhost),
        (c"HostSet", host_set),
        (c"PartNames", part_names),
        (c"Surfaces", surfaces),
        (c"RestPose", rest_pose),
        (c"PartInfo", part_info),
        (c"Fit", fit),
        (c"HasBuild", has_build),
        (c"Create", create),
        (c"Destroy", destroy),
        (c"Count", count),
        (c"Step", step),
        (c"Parts", parts),
        (c"Motion", motion),
        (c"SetParts", set_parts),
        (c"State", state_of),
        (c"Feedback", feedback),
        (c"Message", message),
        (c"Impulse", impulse),
        (c"Push", push),
        (c"Kick", kick),
        (c"Shot", shot),
        (c"Flinch", flinch),
        (c"Balance", balance),
        (c"CatchFall", catch_fall),
        (c"HighFall", high_fall),
        (c"Nearest", nearest),
        (c"Raycast", raycast),
        (c"Incoming", incoming),
        (c"Constrain", constrain),
        (c"FindGTA", find_gta),
        (c"Joints", joints),
        (c"Com", com),
        (c"GetUpTarget", get_up_target),
        (c"HoldsGun", holds_gun),
        (c"CanGetUp", can_get_up),
        (c"Contacts", contacts),
        (c"MeshAdd", mesh_add),
        (c"MeshRemove", mesh_remove),
        (c"Boxes", boxes),
        (c"WorldClear", world_clear),
        (c"WorldRaycast", world_raycast),
        (c"Messages", messages),
        (c"Params", params),
    ]);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn gmod13_close(_: State) -> c_int {
    SIM.with(|s| *s.borrow_mut() = None);
    0
}
