//! Everything the module keeps: the game's data, a body per model, the world, and the characters (agents), stepped
//! together so they collide with each other. Metres, kilograms, seconds, Z up (as Source's axes, scaled).

use crate::custom::Custom;
use crate::game::{Game, Homer, Surface};
use delirium::collision::Collision;
use delirium::msg::{self, Kind, Value};
use delirium::nm::Agent;
use delirium::prelude::*;
use delirium::ragdoll::{Build, Group, Obstacle, Ragdoll};
use delirium_data::taskparams::TaskParams;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

pub struct Body {
    pub agent: Agent,
    pub build: Build,
    /// Behaviours of this module's own (see custom.rs), run alongside the engine's.
    pub custom: Vec<Box<dyn Custom>>,
    /// The game's ragdoll it is the brain of (host.rs), or None: a body of the engine's own.
    pub host: Option<crate::host::Host>,
    /// A gun in its hands (the right; both: a long gun held two-handed): those hands don't reach for wounds.
    pub gun: (bool, bool),
}

pub struct Sim {
    pub game: Game,
    pub homer: Homer,
    pub surfaces: Vec<Surface>,
    pub tp: TaskParams,
    pub tp_found: bool,
    pub rand: delirium_data::rng::Rand,
    pub world: Collision,
    pub boxes: Vec<Obstacle>,
    /// Homer refitted to other skeletons, by the key the caller gave (a model's name).
    pub builds: HashMap<String, Homer>,
    pub bodies: BTreeMap<i32, Body>,
    pub next: i32,
    pub group: Group,
    /// The rest pose's heading (radians about Z): agents are turned from it.
    pub rest_yaw: f32,
    /// The game's physics, for characters on its ragdolls (found on first use).
    pub physics: Option<Result<crate::host::Physics, String>>,
    /// The body each character was made from.
    pub keys: HashMap<i32, String>,
}

thread_local! {
    // (Garry's Mod runs the server's Lua on one thread)
    pub static SIM: RefCell<Option<Sim>> = const { RefCell::new(None) };
}

/// Where a body made from `frames` faces (radians about Z): its left hip to its right, turned up.
pub fn facing(frames: &[Transform]) -> f32 {
    let left = frames[1].translation - frames[4].translation;
    let fwd = left.cross(Vec3::Z);
    fwd.y.atan2(fwd.x)
}

impl Sim {
    pub fn start(dir: &str) -> Result<Sim, String> {
        let game = Game::open(std::path::Path::new(dir))?;
        game.load_character()?;
        let surfaces = game.load_surfaces();
        let tp = game.task_params();
        crate::custom::set_task_params(tp.clone().unwrap_or_default());
        let homer = game.homer()?;
        // (GTA IV's get-ups, for dlGetUp: without them nothing gets up but by other means)
        crate::getup::set_clips(game.clips(&homer.build).ok(), &homer.build, &homer.frames);
        let rest_yaw = facing(&homer.frames);
        let world = Collision::default();
        Ok(Sim {
            game,
            homer,
            surfaces,
            tp_found: tp.is_some(),
            tp: tp.unwrap_or_default(),
            rand: delirium_data::rng::Rand { seed: 1 },
            world,
            boxes: vec![],
            builds: HashMap::new(),
            bodies: BTreeMap::new(),
            next: 1,
            group: Group::default(),
            rest_yaw,
            physics: None,
            keys: HashMap::new(),
        })
    }

    pub fn body_def(&self, key: &str) -> &Homer {
        self.builds.get(key).unwrap_or(&self.homer)
    }

    /// Homer refitted for another skeleton: each part's segment `len[k]` times as long (from its joint to its
    /// children's), `mass` times as heavy, standing in Homer's pose.
    pub fn fit(&mut self, key: &str, len: &[f32], mass: f32) -> Result<(), String> {
        if len.len() != self.homer.build.parts.len() || len.iter().any(|l| !(l.is_finite() && *l > 0.2 && *l < 5.0)) {
            return Err("delirium.Fit: 21 lengths, each 0.2 to 5 times Homer's".into());
        }
        if !(mass.is_finite() && mass > 0.1 && mass < 10.0) {
            return Err("delirium.Fit: a mass scale of 0.1 to 10".into());
        }
        // (the same behaviours and tuning as Homer's: only the parts' sizes and weights change)
        let (build, mut frames) = self.homer.build.scaled(&self.homer.frames, len, mass);
        let floor = build.parts.iter().zip(&frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
        let shift = -frames[0].translation.with_z(floor);
        for f in &mut frames {
            f.translation += shift;
        }
        self.builds.insert(key.to_string(), Homer { build, frames });
        Ok(())
    }

    /// A new character of body `key`, standing with its feet at `pos`, facing `yaw` (radians about Z).
    pub fn create(&mut self, key: &str, pos: Vec3, yaw: f32, seed: f32) -> i32 {
        let def = self.body_def(key).clone();
        let turn = Quat::from_rotation_z(yaw - self.rest_yaw);
        let frames: Vec<Transform> =
            def.frames.iter().map(|f| Transform::from_translation(pos + turn * f.translation).with_rotation((turn * f.rotation).normalize())).collect();
        let agent = Agent::new(&def.build, Ragdoll::new(&def.build, &frames, Vec3::ZERO), turn, seed);
        let id = self.next;
        self.next += 1;
        self.bodies.insert(id, Body { agent, build: def.build, custom: vec![], host: None, gun: (false, false) });
        self.keys.insert(id, key.to_string());
        id
    }

    /// The game's physics (VPhysics-Box3D with Delirium's functions), looked for once.
    pub fn physics(&mut self) -> Result<crate::host::Physics, String> {
        self.physics.get_or_insert_with(crate::host::Physics::find).clone()
    }

    /// Makes character `id` the brain of the game's ragdoll (see host.rs).
    pub fn host(&mut self, id: i32, phys: Vec<*mut std::ffi::c_void>, bones: Vec<Option<crate::host::BoneDef>>, between: Vec<crate::host::Between>,
                root_offset: Vec3) -> Result<(), String> {
        let physics = self.physics()?;
        let rest: Vec<Quat> = self.rest_of(id).ok_or("delirium.Host: the body's rest pose isn't known")?;
        let body = self.bodies.get(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        let host = crate::host::Host::new(&physics, body, rest, phys, bones, between, root_offset)?;
        let body = self.bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        if let Some(old) = body.host.replace(host) {
            old.release(&physics);
        }
        Ok(())
    }

    /// Lets go of the game's ragdoll (its muscles off): the character is the engine's own body again.
    pub fn unhost(&mut self, id: i32) {
        let physics = self.physics.as_ref().and_then(|p| p.as_ref().ok()).copied();
        if let Some(b) = self.bodies.get_mut(&id) {
            if let (Some(h), Some(p)) = (b.host.take(), physics) {
                if !h.lost {
                    h.release(&p);
                }
            }
        }
    }

    /// A character's body's rest rotations (by the key it was made from).
    fn rest_of(&self, id: i32) -> Option<Vec<Quat>> {
        let key = self.keys.get(&id)?;
        Some(self.body_def(key).frames.iter().map(|f| f.rotation).collect())
    }

    pub fn body(&mut self, id: i32) -> Result<&mut Body, String> {
        self.bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))
    }

    /// Every character `dt` seconds on, together (they meet each other), then their own behaviours.
    pub fn step(&mut self, dt: f32) {
        let Sim { bodies, world, boxes, group, physics, .. } = self;
        // characters on the game's ragdolls: each on its own (the game's physics has them meet)
        if let Some(Ok(physics)) = physics.as_ref() {
            for b in bodies.values_mut().filter(|b| b.host.as_ref().is_some_and(|h| !h.lost)) {
                crate::host::step(physics, b, world, boxes, dt);
            }
        }
        for b in bodies.values_mut().filter(|b| b.host.is_none()) {
            for c in &mut b.custom {
                c.before(&mut b.agent, &b.build, world, dt);
            }
        }
        {
            let mut list: Vec<(&mut Agent, usize, &Build, &[Obstacle], &[delirium::ragdoll::Capsule])> = bodies
                .iter_mut()
                .filter(|(_, b)| b.host.is_none())
                .map(|(id, b)| (&mut b.agent, *id as usize, &b.build, &boxes[..], &[][..]))
                .collect();
            if !list.is_empty() {
                Agent::step_group(&mut list, world, dt, group, &mut |_, _, _| {});
            }
        }
        for b in bodies.values_mut().filter(|b| b.host.is_none()) {
            let (mut kept, mut posed) = (vec![], false);
            for mut c in std::mem::take(&mut b.custom) {
                if c.after(&mut b.agent, &b.build, world, dt) {
                    kept.push(c);
                } else {
                    posed |= c.poses();
                    c.stop(&mut b.agent, &b.build, world);
                }
            }
            crate::custom::reposed(&mut kept, posed);
            b.custom = kept;
        }
    }

    /// Sends an engine message (or one of this module's own behaviours) to a character. Returns how many of its
    /// parameters were taken but have no effect.
    pub fn send(&mut self, id: i32, name: &str, params: Vec<(String, crate::lua::Val)>) -> Result<usize, String> {
        if crate::custom::is_custom(name) {
            let Sim { bodies, world, .. } = self;
            let b = bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
            return crate::custom::send(b, world, name, &params);
        }
        let m = msg::find(name).ok_or_else(|| format!("delirium: no message {name}"))?;
        let mut ps: Vec<(&str, Value)> = Vec::with_capacity(params.len());
        for (k, v) in &params {
            let kind = if k == "start" {
                Kind::Bool
            } else {
                m.params.iter().find(|p| p.name == k).map(|p| p.kind).ok_or_else(|| format!("delirium: {name} has no parameter {k}"))?
            };
            ps.push((k.as_str(), value(kind, v).ok_or_else(|| format!("delirium: {name}.{k}: wrong kind of value"))?));
        }
        let Sim { bodies, world, .. } = self;
        let b = bodies.get_mut(&id).ok_or_else(|| format!("delirium: no character {id}"))?;
        // (a balance started afresh: the engine keeps a body that has fallen fallen for good, dropping any balance it
        // starts - Delirium's own viewer makes a new body once one is up again)
        if name == "bodyBalance" && ps.iter().any(|(k, v)| *k == "start" && matches!(v, Value::Bool(true))) {
            b.agent.fallen = false;
        }
        let mut st = std::mem::take(&mut b.agent.msgs);
        let r = msg::send(&mut b.agent, &b.build, world, &mut st, name, &ps);
        b.agent.msgs = st;
        r.map_err(|e| format!("delirium: {name}: {e:?}"))
    }
}

fn value(kind: Kind, v: &crate::lua::Val) -> Option<Value> {
    use crate::lua::Val;
    Some(match (kind, v) {
        (Kind::Float, Val::Num(n)) => Value::Float(*n as f32),
        (Kind::Int, Val::Num(n)) => Value::Int(*n as i32),
        (Kind::Int, Val::Bool(b)) => Value::Int(*b as i32),
        (Kind::Bool, Val::Bool(b)) => Value::Bool(*b),
        (Kind::Bool, Val::Num(n)) => Value::Bool(*n != 0.0),
        (Kind::Vec3, Val::Vec(v)) => Value::Vec3(Vec3::from_array(*v)),
        (Kind::Str, Val::Str(s)) => Value::Str(s.clone()),
        (Kind::Float, Val::Bool(b)) => Value::Float(*b as i32 as f32),
        _ => return None,
    })
}
