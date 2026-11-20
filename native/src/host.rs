//! Characters whose body is the game's own ragdoll. The ragdoll stays the game's - simulated by its physics
//! (VPhysics-Box3D) with the world, touching the real floor, held by the physgun, hit by bullets - and Delirium is its
//! brain: each step its parts' state is read from the ragdoll's physics objects, the behaviours run on it, and their
//! decisions go back into the ragdoll's physics - every muscle as a spring on the ragdoll's own joint (solved inside
//! Box3D's step, every substep), what the behaviours add beyond the muscles as forces. (delirium::host, and
//! physics_delirium.cpp in VPhysics-Box3D.)
//!
//! A ValveBiped ragdoll has 15 physics bones to the body's 21 parts: a bone carries one part (the pelvis, the chest,
//! the head, the limbs), and the parts between (the lower spine, the neck, the clavicles) are put in between, turned
//! part of the way from one to the next. A joint of the ragdoll may stand for several of the body's (the spine's
//! four between the pelvis and the chest): its muscle pulls to all of theirs together.

use crate::custom;
use crate::sim::Body;
use delirium::collision::Collision;
use delirium::prelude::*;
use delirium::ragdoll::Obstacle;
use std::ffi::c_void;

const M: f32 = 0.0254;
/// The engine's gravity (its behaviours are tuned for it), m/s/s.
const G: f32 = 9.81;

pub type StateFn = unsafe extern "C" fn(*mut c_void, *mut f32) -> i32;
pub type MassFn = unsafe extern "C" fn(*mut c_void, *mut f32, *mut f32) -> i32;
pub type MuscleFn = unsafe extern "C" fn(*mut c_void, *mut c_void, *const f32, f32, f32) -> i32;
pub type PushFn = unsafe extern "C" fn(*mut c_void, *const f32, *const f32) -> i32;

/// VPhysics-Box3D's functions for Delirium, in the game's vphysics.dll.
#[derive(Clone, Copy)]
pub struct Physics {
    state: StateFn,
    mass: MassFn,
    muscle: MuscleFn,
    push: PushFn,
}

#[cfg(windows)]
mod sys {
    use std::ffi::{c_char, c_void};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetModuleHandleA(name: *const c_char) -> *mut c_void;
        pub fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    }
}

impl Physics {
    /// The physics the game runs, if it's VPhysics-Box3D with Delirium's functions.
    #[cfg(windows)]
    pub fn find() -> Result<Physics, String> {
        let module = unsafe { sys::GetModuleHandleA(c"vphysics.dll".as_ptr()) };
        if module.is_null() {
            return Err("vphysics.dll isn't loaded".into());
        }
        let get = |name: &std::ffi::CStr| {
            let f = unsafe { sys::GetProcAddress(module, name.as_ptr()) };
            if f.is_null() { Err(format!("the game's vphysics.dll has no {}: it needs VPhysics-Box3D with Delirium's functions", name.to_string_lossy())) } else { Ok(f) }
        };
        let version: unsafe extern "C" fn() -> i32 = unsafe { std::mem::transmute(get(c"VBox3D_Delirium_Version")?) };
        let v = unsafe { version() };
        if v < 1 {
            return Err(format!("the game's VPhysics-Box3D has Delirium's functions version {v}, not 1"));
        }
        unsafe {
            Ok(Physics {
                state: std::mem::transmute::<*mut c_void, StateFn>(get(c"VBox3D_Delirium_State")?),
                mass: std::mem::transmute::<*mut c_void, MassFn>(get(c"VBox3D_Delirium_Mass")?),
                muscle: std::mem::transmute::<*mut c_void, MuscleFn>(get(c"VBox3D_Delirium_Muscle")?),
                push: std::mem::transmute::<*mut c_void, PushFn>(get(c"VBox3D_Delirium_Push")?),
            })
        }
    }

    /// Physics of one's own (a test's stand-in, another engine): the same four functions.
    pub fn custom(state: StateFn, mass: MassFn, muscle: MuscleFn, push: PushFn) -> Physics {
        Physics { state, mass, muscle, push }
    }

    #[cfg(not(windows))]
    pub fn find() -> Result<Physics, String> {
        Err("ragdolls are Delirium's only on Windows for now".into())
    }

    /// A physics object's state (metres, m/s, rad/s), or None (gone).
    fn state(&self, p: *mut c_void) -> Option<(Vec3, Quat, Vec3, Vec3)> {
        let mut s = [0.0f32; 13];
        if unsafe { (self.state)(p, s.as_mut_ptr()) } == 0 || s.iter().any(|x| !x.is_finite()) {
            return None;
        }
        let q = Quat::from_xyzw(s[3], s[4], s[5], s[6]).normalize();
        Some((Vec3::new(s[0], s[1], s[2]) * M, q, Vec3::new(s[7], s[8], s[9]) * M, Vec3::new(s[10], s[11], s[12])))
    }

    /// Its mass (kg) and its world's gravity (m/s/s, down).
    fn mass(&self, p: *mut c_void) -> Option<(f32, f32)> {
        let (mut m, mut g) = (0.0f32, [0.0f32; 3]);
        if unsafe { (self.mass)(p, &mut m, g.as_mut_ptr()) } == 0 {
            return None;
        }
        Some((m, -g[2] * M))
    }

    fn muscle(&self, parent: *mut c_void, child: *mut c_void, rel: Quat, hertz: f32, damping: f32) -> bool {
        let q = [rel.x, rel.y, rel.z, rel.w];
        unsafe { (self.muscle)(parent, child, q.as_ptr(), hertz, damping) != 0 }
    }

    fn push(&self, p: *mut c_void, force: Vec3, torque: Vec3) {
        let (f, t) = (force.to_array(), torque.to_array());
        unsafe { (self.push)(p, f.as_ptr(), t.as_ptr()) };
    }
}

/// A physics bone of the ragdoll: the part it carries, its parent bone, and the part's rotation in the bone's frame
/// (the part's rotation = the bone's * this).
#[derive(Clone, Copy, Debug)]
pub struct BoneDef {
    pub part: usize,
    pub parent: Option<usize>,
    pub offset: Quat,
}

/// A part with no bone of its own, put between two that have: turned `t` of the way from part a's way to part d's.
#[derive(Clone, Copy, Debug)]
pub struct Between {
    pub part: usize,
    pub a: usize,
    pub d: usize,
    pub t: f32,
}

/// How the game wants it driven just now.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The muscles work (off: limp - the ragdoll's own joints only).
    pub muscles: bool,
    /// The muscles' strength (the engine's strength scale: 1 full).
    pub strength: f32,
    /// Bones another addon moves itself just now: their joints' muscles let go.
    pub loose: Vec<bool>,
    /// The game's heavier gravity made up to the engine's (its behaviours are tuned for 9.81 m/s/s): 0 never, 1 while
    /// it stands and balances (falling, it falls as everything else does), 2 always.
    pub gravity_fix: u8,
    /// How hard what the behaviours add beyond the muscles holds the body (holding limbs up against gravity, the
    /// balancer's help): 1 as the engine has it.
    pub hold: f32,
    /// The muscles' damping, times this.
    pub damping: f32,
    /// The ragdoll's own gravity, of the world's (0: its gravity turned off - the physgun's, a "no gravity" setting):
    /// it weighs that much, and is held up against that much.
    pub gravity: f32,
}

pub struct Host {
    pub phys: Vec<*mut c_void>,
    pub bones: Vec<Option<BoneDef>>,
    pub between: Vec<Between>,
    /// The pelvis part's centre from the pelvis bone, in the part's frame (centre = bone - q * this).
    pub root_offset: Vec3,
    /// The body's rest rotations (world, at rest).
    pub rest: Vec<Quat>,
    /// part -> the bone that moves it (its own, or the one it's put between from).
    carrier: Vec<usize>,
    bone_mass: Vec<f32>,
    /// What a bone holds out, for its torque's cap: an arm's bone, the arm beyond it too (the rest, its own).
    held_mass: Vec<f32>,
    part_mass: Vec<f32>,
    gravity: f32,
    pub settings: Settings,
    /// The ragdoll's physics objects are gone.
    pub lost: bool,
    /// Its joints' muscles as last set (bone pairs that took them).
    joints: Vec<(usize, usize)>,
}

impl Host {
    /// A host for `body` on the ragdoll's physics objects `phys` (by physics bone).
    pub fn new(physics: &Physics, body: &Body, rest: Vec<Quat>, phys: Vec<*mut c_void>, bones: Vec<Option<BoneDef>>, between: Vec<Between>,
               root_offset: Vec3) -> Result<Host, String> {
        let n = body.build.parts.len();
        if phys.len() != bones.len() {
            return Err("delirium.Host: one bone definition for each physics object".into());
        }
        if rest.len() != n {
            return Err("delirium.Host: the body's rest pose doesn't fit".into());
        }
        let mut carrier = vec![usize::MAX; n];
        for (i, b) in bones.iter().enumerate() {
            if let Some(b) = b {
                if b.part >= n || b.parent.is_some_and(|p| p >= bones.len()) {
                    return Err(format!("delirium.Host: bone {i} has a part or parent out of range"));
                }
                carrier[b.part] = i;
            }
        }
        if carrier[0] == usize::MAX {
            return Err("delirium.Host: no bone carries the pelvis".into());
        }
        for w in &between {
            if w.part >= n || w.a >= n || w.d >= n || carrier[w.a] == usize::MAX || carrier[w.d] == usize::MAX {
                return Err("delirium.Host: a part put between parts no bone carries".into());
            }
            carrier[w.part] = if w.t < 0.5 { carrier[w.a] } else { carrier[w.d] };
        }
        if let Some(k) = carrier.iter().position(|&c| c == usize::MAX) {
            return Err(format!("delirium.Host: part {k} is neither carried nor put between"));
        }
        let mut bone_mass = vec![0.0; phys.len()];
        let mut gravity = 600.0 * M;
        for (i, &p) in phys.iter().enumerate() {
            let (m, g) = physics.mass(p).ok_or_else(|| format!("delirium.Host: physics object {i} isn't one (not VPhysics-Box3D's, or gone)"))?;
            bone_mass[i] = m;
            gravity = g;
        }
        let joints = bones.iter().enumerate().filter_map(|(c, b)| b.and_then(|b| b.parent).map(|p| (p, c))).collect();
        // (an arm's bones - Homer's upper arm, forearm and hand parts - hold out the arm beyond them: the shoulder's
        // muscle holds the whole arm up, not just the upper arm)
        let arm = |i: usize| bones[i].is_some_and(|b| matches!(b.part, 14 | 15 | 16 | 18 | 19 | 20));
        let mut held_mass = bone_mass.clone();
        for i in (0..bones.len()).filter(|&i| arm(i)) {
            let mut p = bones[i].and_then(|b| b.parent);
            while let Some(j) = p.filter(|&j| arm(j)) {
                held_mass[j] += bone_mass[i];
                p = bones[j].and_then(|b| b.parent);
            }
        }
        Ok(Host {
            settings: Settings { muscles: true, strength: 1.0, loose: vec![false; phys.len()], gravity_fix: 1, hold: 1.0, damping: 1.0, gravity: 1.0 },
            phys,
            bones,
            between,
            root_offset,
            rest,
            carrier,
            bone_mass,
            held_mass,
            part_mass: body.build.parts.iter().enumerate().map(|(k, _)| body.agent.doll.part_mass(k)).collect(),
            gravity,
            lost: false,
            joints,
        })
    }

    /// Every muscle let go (the ragdoll the game's alone again).
    pub fn release(&self, physics: &Physics) {
        for &(p, c) in &self.joints {
            physics.muscle(self.phys[p], self.phys[c], Quat::IDENTITY, 0.0, 1.0);
        }
    }
}

/// The parts' rotations from the bones' (and their spins).
fn parts_from(host: &Host, bones: &[(Vec3, Quat, Vec3, Vec3)], n: usize) -> (Vec<Quat>, Vec<Vec3>) {
    let mut q = vec![Quat::IDENTITY; n];
    let mut w = vec![Vec3::ZERO; n];
    for (i, b) in host.bones.iter().enumerate() {
        if let Some(b) = b {
            q[b.part] = (bones[i].1 * b.offset).normalize();
            w[b.part] = bones[i].3;
        }
    }
    for t in &host.between {
        let (ra, rd, rk) = (host.rest[t.a], host.rest[t.d], host.rest[t.part]);
        let from_a = q[t.a] * ra.inverse() * rk;
        let from_d = q[t.d] * rd.inverse() * rk;
        q[t.part] = from_a.slerp(from_d, t.t.clamp(0.0, 1.0)).normalize();
        w[t.part] = w[t.a].lerp(w[t.d], t.t.clamp(0.0, 1.0));
    }
    (q, w)
}

/// One step of a character on the game's ragdoll. False if the ragdoll is gone.
pub fn step(physics: &Physics, body: &mut Body, world: &Collision, obstacles: &[Obstacle], dt: f32) -> bool {
    let Some(host) = body.host.as_mut() else { return false };
    let n = body.build.parts.len();
    // the ragdoll now
    let mut bones = Vec::with_capacity(host.phys.len());
    for &p in &host.phys {
        match physics.state(p) {
            Some(s) => bones.push(s),
            None => {
                host.lost = true;
                return false;
            }
        }
    }
    // (the gravity it's under now - the world's, changed while the body's up, or its own turned off: weightless, the
    // support it's given is none; given it, it would only fly up)
    if let Some((_, g)) = physics.mass(host.phys[host.carrier[0]]) {
        host.gravity = g.max(0.0) * host.settings.gravity.clamp(0.0, 1.0);
    }
    body.agent.gravity_scale = (host.gravity / G).clamp(0.0, 1.0);
    let (q, w) = parts_from(host, &bones, n);
    // the parts placed down the body from the pelvis, joint to joint, in the body's own proportions
    let mut x = vec![Vec3::ZERO; n];
    let pelvis_bone = host.carrier[0];
    x[0] = bones[pelvis_bone].0 - q[0] * host.root_offset;
    for k in 1..n {
        let part = &body.build.parts[k];
        let Some(parent) = part.parent else { continue };
        let pivot = x[parent] + q[parent] * part.anchor_parent;
        x[k] = pivot - q[k] * part.anchor;
    }
    for k in 0..n {
        let (bx, _, bv, bw) = bones[host.carrier[k]];
        let v = bv + bw.cross(x[k] - bx);
        body.agent.doll.set_body(k, x[k], q[k], v, w[k]);
    }
    // the brain
    body.agent.strength = host.settings.strength.clamp(0.0, 4.0);
    let pushes = custom::host_mode(|| {
        for c in &mut body.custom {
            c.before(&mut body.agent, &body.build, world, dt);
        }
    });
    let kept = body.agent.step_host(&body.build, world, dt, obstacles);
    let (mut kept_customs, mut posed) = (vec![], false);
    let more = custom::host_mode(|| {
        for mut c in std::mem::take(&mut body.custom) {
            if c.after(&mut body.agent, &body.build, world, dt) {
                kept_customs.push(c);
            } else {
                posed |= c.poses();
                c.stop(&mut body.agent, &body.build, world);
            }
        }
    });
    custom::reposed(&mut kept_customs, posed);
    body.custom = kept_customs;
    let Some(kept) = kept else { return true };
    // the muscles, onto the ragdoll's joints
    let muscles = delirium::host::muscles(&body.build, &kept);
    let on = host.settings.muscles;
    for &(pb, cb) in &host.joints {
        let (Some(pd), Some(cd)) = (host.bones[pb], host.bones[cb]) else { continue };
        // the body's joints between the two bones' parts (child-most first)
        let mut chain = vec![];
        let mut k = cd.part;
        while k != pd.part {
            chain.push(k);
            match body.build.parts[k].parent {
                Some(p) => k = p,
                None => break,
            }
        }
        if k != pd.part || chain.is_empty() {
            continue;
        }
        let mut rel = Quat::IDENTITY;
        let (mut stiffness, mut damping) = (0.0f32, 1.0f32);
        for &j in chain.iter().rev() {
            let parent = body.build.parts[j].parent.unwrap_or(0);
            match muscles[j] {
                Some(m) => {
                    rel = rel * m.target;
                    if m.stiffness > stiffness {
                        (stiffness, damping) = (m.stiffness, m.damping);
                    }
                }
                // (a limp joint stays as it is)
                None => rel = rel * (q[parent].inverse() * q[j]),
            }
        }
        let hertz = if on && !host.settings.loose[cb] && !host.settings.loose[pb] { stiffness / std::f32::consts::TAU } else { 0.0 };
        // in the bones' own frames: bone = part * offset^-1
        let rel_bones = (pd.offset * rel.normalize() * cd.offset.inverse()).normalize();
        physics.muscle(host.phys[pb], host.phys[cb], rel_bones, hertz, (damping * host.settings.damping).clamp(0.05, 4.0));
    }
    // what the behaviours add beyond the muscles: per bone, as a force at its centre and a torque
    // (the behaviours' support - the lift holding a body up - against the world's gravity as it is, at most the
    // engine's: in none, nothing to hold up, and it would only fling the body up)
    let extra = delirium::host::extra(&body.build, &kept, &host.part_mass, G.min(host.gravity));
    let mut force = vec![Vec3::ZERO; host.phys.len()];
    let mut torque = vec![Vec3::ZERO; host.phys.len()];
    let mut carried = vec![0.0f32; host.phys.len()];
    let mut add = |k: usize, f: Vec3, t: Vec3, at: Vec3| {
        let i = host.carrier[k];
        force[i] += f;
        torque[i] += t + (at - bones[i].0).cross(f);
    };
    if on {
        for (k, (f, t)) in extra.iter().enumerate() {
            add(k, *f, *t, x[k]);
        }
        // this module's own behaviours' pushes (impulses over the step)
        for (k, at, j) in pushes.into_iter().chain(more) {
            if k < n && dt > 0.0 {
                add(k, j / dt, Vec3::ZERO, at);
            }
        }
    }
    for k in 0..n {
        carried[host.carrier[k]] += host.part_mass[k];
    }
    // (getting up, the trunk is turned up harder than a limb is ever held)
    let rising = body.custom.iter().any(|c| c.as_get_up().is_some());
    for i in 0..host.phys.len() {
        // (the engine's masses to the ragdoll's: the same change of speed)
        // (and as strong as the muscles are: the muscles' strength scales it too - weaker, it holds itself up less)
        let scale = if carried[i] > 1e-3 { host.bone_mass[i] / carried[i] } else { 1.0 } * host.settings.hold.max(0.0) * host.settings.strength.clamp(0.0, 1.0);
        // (a bone let go of - another addon moves it, or it's limp: a cuffed or broken arm - gets none of it either)
        let limp = host.settings.loose.get(i).copied().unwrap_or(false);
        let mut f = if limp { Vec3::ZERO } else { force[i] * scale };
        // (never a wrench: at most what holding a limb this heavy out straight takes - an arm's, all of it beyond)
        let t = if limp { Vec3::ZERO } else { (torque[i] * scale).clamp_length_max((8.0 + 6.0 * host.held_mass[i]) * if rising { 3.0 } else { 1.0 }) };
        let fix = match host.settings.gravity_fix {
            0 => false,
            1 => body.agent.balancing(),
            _ => true,
        };
        if on && fix && host.gravity > G {
            f += Vec3::Z * (host.bone_mass[i] * (host.gravity - G));
        }
        if f != Vec3::ZERO || t != Vec3::ZERO {
            physics.push(host.phys[i], f, t);
        }
    }
    true
}
