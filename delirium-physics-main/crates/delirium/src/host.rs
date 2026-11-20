//! A body another physics engine simulates (a game's own ragdoll), with Delirium as its brain: the behaviours read the
//! state the game sets (Ragdoll::set_body, FOLLOW) and decide, and the game's engine carries it out with its own
//! joints. A muscle is handed over as what it is - a spring towards a rotation of the part in its parent's frame, of a
//! stiffness (its natural frequency) and a damping ratio - for the engine to solve with its joints, implicitly, every
//! substep (no explicit torques at the game's tick to blow up). What the behaviours add beyond the muscles (holding up
//! against gravity, the balancer's help, a shot's push) comes as forces and torques per part.

use crate::prelude::*;
use crate::ragdoll::{Build, Kept};

/// A part's muscle: the spring on the joint to its parent.
#[derive(Clone, Copy, Debug)]
pub struct HostMuscle {
    pub parent: usize,
    /// The part's rotation in its parent's frame the muscle pulls to (the parts' frames as Ragdoll::frames has them).
    pub target: Quat,
    /// Natural frequency, radians a second (the engine's strength is its square).
    pub stiffness: f32,
    /// Damping ratio (1: critical).
    pub damping: f32,
    pub hinge: bool,
}

/// The rotation a joint's angles stand for, in its own frames: the swing (the lean, theta about an axis in the
/// frame's xy plane, taking z to the child's z) after the twist about z - as Ragdoll::joint_angles measures them.
pub fn swing_twist(a: [f32; 3]) -> Quat {
    let twist = Quat::from_rotation_z(a[2]);
    let theta = a[0].hypot(a[1]);
    if theta < 1e-7 {
        return twist;
    }
    let (cs, sn) = (a[0] / theta, a[1] / theta);
    Quat::from_axis_angle(Vec3::new(-sn, cs, 0.0), theta) * twist
}

/// The rotation of part k in its parent's frame for the joint's angles (a hinge: its angle in a[0]).
pub fn joint_rotation(build: &Build, k: usize, rigid: Option<&crate::ragdoll::Rigid>, a: [f32; 3]) -> Option<Quat> {
    let (_, pf, cf, hinge) = crate::ragdoll::joint_frames(build, k, rigid)?;
    let turn = if hinge { Quat::from_rotation_z(a[0]) } else { swing_twist(a) };
    Some((pf * turn * cf.inverse()).normalize())
}

/// Every part's muscle from a step's drive (step_host): None for the root, a part lost, or a muscle with no strength.
pub fn muscles(build: &Build, kept: &Kept) -> Vec<Option<HostMuscle>> {
    (0..build.parts.len())
        .map(|k| {
            let r = kept.rigid.get(k)?;
            let [strength, damping] = r.gain;
            if !(strength > 0.0) || !strength.is_finite() {
                return None;
            }
            let (parent, _, _, hinge) = crate::ragdoll::joint_frames(build, k, Some(r))?;
            let target = joint_rotation(build, k, Some(r), r.target)?;
            let s = strength.sqrt();
            Some(HostMuscle { parent, target, stiffness: s, damping: (damping / (2.0 * s)).max(0.0), hinge })
        })
        .collect()
}

/// What a step's drive adds beyond the muscles, per part: a force at its centre (newtons) and a torque (newton
/// metres), world - the behaviours' own forces and torques, and gravity's share they take off (lift) or add (accel).
pub fn extra(build: &Build, kept: &Kept, masses: &[f32], gravity: f32) -> Vec<(Vec3, Vec3)> {
    (0..build.parts.len())
        .map(|k| {
            let m = masses.get(k).copied().unwrap_or(0.0);
            let f = kept.force.get(k).copied().unwrap_or(Vec3::ZERO) + (Vec3::Z * (kept.lift * gravity) + kept.accel) * m;
            (f, kept.torque.get(k).copied().unwrap_or(Vec3::ZERO))
        })
        .collect()
}
