//! Behaviours of this module's own, beside the engine's 55 messages. Each is sent like an engine message (its name
//! starts with "dl"): `start=true` begins it, `start=false` ends it, any other parameters change a running one.
//! Every step it can steer the engine (send it messages, hold a hand somewhere on the body) before the physics, and
//! say afterwards whether it carries on.

use crate::lua::Val;
use crate::sim::Body;
use delirium::collision::Collision;
use delirium::msg::{self, Value};
use delirium::nm::Agent;
use delirium::prelude::*;
use delirium::ragdoll::Build;

pub trait Custom {
    fn name(&self) -> &'static str;
    /// New parameters for a running one.
    fn set(&mut self, _params: &[(String, Val)]) {}
    /// Before the engine's step.
    fn before(&mut self, _a: &mut Agent, _b: &Build, _w: &Collision, _dt: f32) {}
    /// After it: false once it's done.
    fn after(&mut self, _a: &mut Agent, _b: &Build, _w: &Collision, _dt: f32) -> bool {
        true
    }
    /// Ended (by start=false, or by itself): undo what it holds.
    fn stop(&mut self, _a: &mut Agent, _b: &Build, _w: &Collision) {}
    /// The get-up, if this is it (for its targets).
    fn as_get_up(&self) -> Option<&crate::getup::GetUp> {
        None
    }
    /// Holds a pose of its own (animPose, sent again every so often). The engine has one pose: another behaviour's
    /// ending lets go of it, so the others that pose send theirs again at once (see reposed).
    fn poses(&self) -> bool {
        false
    }
    /// Its pose sent again on its next step.
    fn repose(&mut self) {}
    /// Started again while it runs: true if it took the new start itself (else it's stopped and made anew).
    fn restart(&mut self, _params: &[(String, Val)]) -> bool {
        false
    }
}

/// After behaviours stopped: if one of them posed, the others that pose send their poses again at once (its stopping
/// let go of the one pose they share - else the pose would drop for a moment).
pub fn reposed(list: &mut [Box<dyn Custom>], stopped_posing: bool) {
    if stopped_posing {
        for c in list.iter_mut().filter(|c| c.poses()) {
            c.repose();
        }
    }
}

type Make = fn(&[(String, Val)]) -> Box<dyn Custom>;

thread_local! {
    // While a character on the game's ragdoll steps (host.rs): its behaviours' pushes, for the ragdoll (the body's own
    // state is the ragdoll's, read again each step - a push on it alone would be lost).
    static HOST_PUSHES: std::cell::RefCell<Option<Vec<(usize, Vec3, Vec3)>>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` for a character on the game's ragdoll; what it pushed (part, where, impulse).
pub fn host_mode(f: impl FnOnce()) -> Vec<(usize, Vec3, Vec3)> {
    HOST_PUSHES.with(|h| *h.borrow_mut() = Some(vec![]));
    f();
    HOST_PUSHES.with(|h| h.borrow_mut().take()).unwrap_or_default()
}

fn hosted() -> bool {
    HOST_PUSHES.with(|h| h.borrow().is_some())
}

/// An impulse on a part: on the body, or (on the game's ragdoll) for the ragdoll.
pub(crate) fn push(a: &mut Agent, k: usize, at: Vec3, j: Vec3) {
    let kept = HOST_PUSHES.with(|h| h.borrow_mut().as_mut().map(|v| v.push((k, at, j))).is_some());
    if !kept {
        a.doll.impulse(k, at, j);
    }
}

/// The behaviours by name.
fn table() -> &'static [(&'static str, Make)] {
    &[("dlHoldWound", HoldWound::make), ("dlFentanylFold", Fold::make), ("dlBrainDamage", Brain::make), ("dlGrip", Grip::make), ("dlOnFire", Burn::make), ("dlGetUp", crate::getup::GetUp::make), ("dlWalk", Walk::make), ("dlStagger", Stagger::make), ("dlShotReact", ShotReact::make), ("dlBrace", Brace::make)]
}

// The parts (0 pelvis, 10 chest, 11 neck, 12 head, 16 and 20 hands) and the joints' angles, as measured on a standing
// body (tests/poses.lua): spine and neck angle 1 bends forward, angle 2 sideways, 3 twists; a shoulder's angle 1 raises
// the arm forward and 3 out to the side (both sides alike); an elbow's angle 1 bends it.
const L_HAND: usize = 16;
const R_HAND: usize = 20;

fn pivot(a: &Agent, b: &Build, k: usize) -> Vec3 {
    let f = a.doll.frames().nth(k).unwrap_or_default();
    if b.parts[k].parent.is_some() { f.translation + f.rotation * b.parts[k].anchor } else { f.translation }
}

fn pose_text(entries: &[(usize, [f32; 3])]) -> String {
    entries.iter().map(|(j, a)| format!("{j}:{:.3},{:.3},{:.3}", a[0], a[1], a[2])).collect::<Vec<_>>().join(";")
}

/// A small random stream of its own (deterministic per body).
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

/// The body's own directions now: forward, left and up (from the shoulders and the spine, so lying or rolled too).
fn body_axes(a: &Agent, b: &Build) -> (Vec3, Vec3, Vec3) {
    let up = (pivot(a, b, 11) - pivot(a, b, 7)).normalize_or(Vec3::Z);
    let left = pivot(a, b, 14) - pivot(a, b, 18);
    let left = (left - up * left.dot(up)).normalize_or(Vec3::Y);
    (left.cross(up), left, up)
}

/// dlHoldWound: a hand goes to a point on the body and presses on it - a slit throat, a bleeding arm - and stays there
/// as the body moves, falls or rolls. part (the wounded part, 0-20), offset (where on it, metres from its centre in the
/// body's own directions: forward, left, up - {0.1,0,0} the front of the throat; without one, the side the hand comes
/// from), hand (16 left, 20 right, -1 the one that makes sense: the other arm's for an arm, else the right). The hand
/// rests on the skin there: a point inside the part is taken out to its surface.
struct HoldWound {
    part: usize,
    offset: Option<Vec3>,
    hand: Option<usize>,
    reach: f32,
    at: Option<Vec3>,
    /// (the cradle spring's pull for being off)
    held: Vec3,
    /// an arm wound: that arm's hand held in front of the belly (as a hurt arm is held in)
    cradle: Option<(usize, ())>,
}

impl HoldWound {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        let part = (num(p, "part", 11.0) as i32).clamp(0, 20) as usize;
        let hand = match num(p, "hand", -1.0) as i32 {
            16 => L_HAND,
            20 => R_HAND,
            _ => match part {
                13..=16 | 1..=3 => R_HAND,
                17..=20 | 4..=6 => L_HAND,
                _ => R_HAND,
            },
        };
        // (a hand can't hold itself)
        let hand = if hand == part { if hand == L_HAND { R_HAND } else { L_HAND } } else { hand };
        let cradle = match part {
            14..=16 => Some((L_HAND, ())),
            18..=20 => Some((R_HAND, ())),
            _ => None,
        }
        .filter(|&(h, _)| h != hand);
        Box::new(HoldWound { part, offset: vec(p, "offset"), hand: Some(hand), reach: num(p, "reachTime", 0.25).max(0.05), at: None, held: Vec3::ZERO, cradle })
    }
}

/// A hand pulled (hard, over `time` from where it was) towards a point, no further than its arm reaches: a hard pull
/// past it would yank the whole body along. (Hard: right where it should be - a wound pressed on. Only for a point
/// that keeps still on the body while the hand gets there: one moving with the arm itself, a gun's grip, sets the
/// body spinning - that's a spring's job.)
fn pull_hand(a: &mut Agent, b: &Build, hand: usize, target: Vec3, at: &mut Option<Vec3>, time: f32, speed: f32, dt: f32) {
    let hand_at = a.doll.frames().nth(hand).unwrap_or_default().translation;
    let (upper, fore) = if hand == L_HAND { (14, 15) } else { (18, 19) };
    let shoulder = pivot(a, b, upper);
    let reach = (pivot(a, b, fore) - shoulder).length() + (pivot(a, b, hand) - pivot(a, b, fore)).length() + (hand_at - pivot(a, b, hand)).length();
    let d = target - shoulder;
    let target = if d.length() > reach * 0.97 { shoulder + d.normalize_or_zero() * reach * 0.97 } else { target };
    // closing in fast from afar, and then keeping up with the point as it moves (up to `speed` m/s)
    let from = at.unwrap_or(hand_at);
    let d = target - from;
    let step = (d.length() * (1.0 - (-dt / time).exp())).max(speed * dt).min(d.length());
    let now = from + d.normalize_or_zero() * step;
    *at = Some(now);
    a.doll.constrain(hand, Some(now), None);
}

/// How stiff an arm's muscles are while a spring holds its hand somewhere (out of the engine's usual 5 to 16).
const ARM_GIVE: f32 = 2.0;

/// A hand drawn to a point the way the arm's muscles would do it: a damped spring between the hand and the part the
/// arm pushes off from (the chest; for a grip, the other hand, through the gun) - what pulls the hand pulls that part
/// back, so nothing is pushed or spun from outside (a hard constraint to a point on the body itself would drive the
/// whole body round) - carrying the arm's weight. `w` is how stiff (rad/s),
/// `most` the most force (N).
/// `held` is how long it has been off, and how far (it pulls the harder for it, so the muscles giving way to it - the
/// balancer's own hold on the arm - don't leave the hand short).
/// The arm's own muscles let go of it meanwhile (whatever behaviour runs holds its arms in a pose of its own, and the
/// spring would only fight them).
fn spring_pair(a: &mut Agent, b: &Build, wd: &Collision, hand: usize, target: Vec3, other: usize, w: f32, most: f32, held: &mut Vec3, dt: f32) {
    let mask = if hand == L_HAND { "ul" } else { "ur" };
    message(a, b, wd, "setStiffness", &[("bodyStiffness", Value::Float(ARM_GIVE)), ("damping", Value::Float(1.0)), ("mask", Value::Str(mask.into()))]);
    let (fore, upper) = (hand - 1, hand - 2);
    let m = a.doll.part_mass(hand) + a.doll.part_mass(fore) + 0.5 * a.doll.part_mass(upper);
    let hand_at = a.doll.frames().nth(hand).unwrap_or_default().translation;
    let (hv, ov) = (a.doll.motion(hand).0, a.doll.motion(other).0);
    let off = target - hand_at;
    // (a leaky sum: half a second's worth of being off, no more - a longer memory swings the hand round in circles)
    *held = (*held * (1.0 - 2.0 * dt) + off * dt).clamp_length_max(0.05);
    // (held up against its weight: as much as there is of it - none, weightless)
    let acc = off * (w * w) + *held * (2.0 * w * w) - (hv - ov) * (2.2 * w) + Vec3::Z * (9.81 * a.gravity_scale);
    let j = (acc * m * dt).clamp_length_max(most * dt);
    push(a, hand, hand_at, j);
    push(a, other, target, -j);
}

/// Where a hurt arm's hand is held: in front of the belly, at the middle.
const CRADLE: [f32; 2] = [0.24, -0.22];

impl Custom for HoldWound {
    fn name(&self) -> &'static str {
        "dlHoldWound"
    }

    fn set(&mut self, p: &[(String, Val)]) {
        if let Some(o) = vec(p, "offset") {
            self.offset = Some(o);
        }
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        let Some(hand) = self.hand else { return };
        let (fwd, left, up) = body_axes(a, b);
        if let Some((h, _)) = self.cradle {
            let chest = a.doll.frames().nth(10).unwrap_or_default().translation;
            spring_pair(a, b, w, h, chest + fwd * CRADLE[0] + up * CRADLE[1], 10, 12.0, 150.0, &mut self.held, dt);
        }
        let hand_at = a.doll.frames().nth(hand).unwrap_or_default().translation;
        let mut centre = a.doll.frames().nth(self.part).unwrap_or_default().translation;
        if self.offset.is_none() && matches!(self.part, 1 | 2 | 4 | 5 | 14 | 15 | 18 | 19) {
            // a limb's long bone: the hand takes it where it's nearest along it (not past its ends)
            let (from, to) = (pivot(a, b, self.part), pivot(a, b, self.part + 1));
            let along = to - from;
            let t = ((hand_at - from).dot(along) / along.length_squared().max(1e-6)).clamp(0.25, 0.75);
            centre = from + along * t;
        }
        let off = match self.offset {
            Some(o) => fwd * o.x + left * o.y + up * o.z,
            // a held-in arm is gripped from above (its own hand is in the way from the side); else on the near side
            None if self.cradle.is_some() => up * 0.001,
            None => (hand_at - centre).normalize_or(fwd) * 0.001,
        };
        // the hand's centre on the skin there, no deeper (the part's centre is inside it)
        let skin = b.parts[self.part].radius + b.parts[hand].radius;
        let target = centre + if off.length() < skin { off.normalize_or(fwd) * skin } else { off };
        if hosted() {
            // (the game's ragdoll: a hard hold on the body alone would be lost - the arm's own muscles press it there)
            let k = (5.0 / self.reach).clamp(8.0, 30.0);
            spring_pair(a, b, w, hand, target, 10, k, 300.0, &mut self.held, dt);
        } else {
            pull_hand(a, b, hand, target, &mut self.at, self.reach, 1.0, dt);
        }
    }

    fn stop(&mut self, a: &mut Agent, _b: &Build, _w: &Collision) {
        if let Some(h) = self.hand {
            a.doll.constrain(h, None, None);
        }
    }
}

/// dlGrip: the free hand holds the gun the other hand has - on the foregrip of a long gun, cupped under the gun hand
/// for a pistol - and stays on it as the gun hand aims, kicks and swings. hand (the holding hand: 16 left, 20 right;
/// the left by default), offset (where, metres, in the gun hand's own frame), reachTime (s to get there, 0.15).
/// The grip is let go of with start=false (the gun dropped, the hand needed for something else).
struct Grip {
    hand: usize,
    gun_hand: usize,
    offset: Vec3,
    reach: f32,
    held: Vec3,
}

impl Grip {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        let hand = if num(p, "hand", 16.0) as i32 == 20 { R_HAND } else { L_HAND };
        let gun_hand = if hand == L_HAND { R_HAND } else { L_HAND };
        Box::new(Grip { hand, gun_hand, offset: vec(p, "offset").unwrap_or(Vec3::ZERO), reach: num(p, "reachTime", 0.15).max(0.02), held: Vec3::ZERO })
    }
}

impl Custom for Grip {
    fn name(&self) -> &'static str {
        "dlGrip"
    }

    fn set(&mut self, p: &[(String, Val)]) {
        if let Some(o) = vec(p, "offset") {
            self.offset = o;
        }
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        let f = a.doll.frames().nth(self.gun_hand).unwrap_or_default();
        let mut target = f.translation + f.rotation * self.offset;
        // (not into the gun hand itself: the two would push each other about)
        let skin = b.parts[self.hand].radius + b.parts[self.gun_hand].radius;
        let d = target - f.translation;
        if d.length() < skin {
            target = f.translation + d.normalize_or(f.rotation * Vec3::NEG_Z) * skin;
        }
        // held on stiffly (a gun kicks and swings fast). The hand touches nothing meanwhile: on the gun (which the
        // body hasn't got), it would only bump the gun arm about.
        a.doll.contact_off |= 1 << self.hand;
        let k = (4.5 / self.reach).clamp(10.0, 40.0);
        spring_pair(a, b, w, self.hand, target, self.gun_hand, k, 500.0, &mut self.held, dt);
    }

    fn stop(&mut self, a: &mut Agent, _b: &Build, _w: &Collision) {
        a.doll.contact_off &= !(1 << self.hand);
    }
}

/// dlFentanylFold: an opioid overdose's fold - nodding off on one's feet: the spine folds slowly forward, the head
/// drops onto the chest and the arms hang, nodding in and out of it, still standing; deep enough for long enough (or
/// told to: buckle), the knees give and it slumps down in a heap. amount (0-1, how deep: change it as the dose acts),
/// collapseAfter (s at its fold - as deep as amount takes it - before it goes down, limp; 0 never), buckle (true: going
/// down now).
struct Fold {
    amount: f32,
    now: f32,
    t: f32,
    next: f32,
    full: f32,
    collapse: f32,
    started: bool,
    /// going down: how long since the knees gave
    buckled: Option<f32>,
    down: bool,
}

impl Fold {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        Box::new(Fold {
            amount: num(p, "amount", 0.6).clamp(0.0, 1.0),
            now: 0.0,
            t: 0.0,
            next: 0.0,
            full: 0.0,
            collapse: num(p, "collapseAfter", 6.0),
            started: false,
            buckled: flag(p, "buckle", false).then_some(0.0),
            down: false,
        })
    }
}

impl Custom for Fold {
    fn name(&self) -> &'static str {
        "dlFentanylFold"
    }

    fn poses(&self) -> bool {
        !self.down && self.buckled.is_none()
    }

    fn repose(&mut self) {
        self.next = 0.0;
    }

    fn set(&mut self, p: &[(String, Val)]) {
        self.amount = num(p, "amount", self.amount).clamp(0.0, 1.0);
        self.collapse = num(p, "collapseAfter", self.collapse);
        if flag(p, "buckle", false) && self.buckled.is_none() {
            self.buckled = Some(0.0);
        }
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        self.t += dt;
        // it comes on over seconds, and eases off as slowly
        let rate = if self.amount > self.now { 0.22 } else { 0.08 };
        self.now += (self.amount - self.now).clamp(-rate * dt, rate * dt);
        // (folded as deep as the dose takes it - counted from there to going down: a lighter dose folds less deep, and
        // still goes down - it never reached the full fold, so it stood there folded for good)
        if self.now >= self.amount.min(0.95) - 0.03 { self.full += dt } else { self.full = 0.0 }
        if self.down {
            return;
        }
        if self.buckled.is_none() && self.collapse > 0.0 && self.full > self.collapse {
            self.buckled = Some(0.0);
        }
        // going down: the legs give first (the balancer let go, the knees too weak to hold), then all of it
        if let Some(t) = self.buckled.as_mut() {
            if *t == 0.0 {
                message(a, b, w, "bodyBalance", &[("bodyStiffness", Value::Float(2.5)), ("damping", Value::Float(0.6))]);
                message(a, b, w, "setStiffness", &[("bodyStiffness", Value::Float(2.0)), ("damping", Value::Float(0.7)), ("mask", Value::Str("lb".into()))]);
            }
            *t += dt;
            if *t > 0.9 {
                // (down and out: limp, as out cold)
                self.down = true;
                message(a, b, w, "stopAllBehaviours", &[]);
                message(a, b, w, "bodyRelax", &[("mask", Value::Str("fb".into())), ("relaxation", Value::Float(100.0))]);
            }
            return;
        }
        self.next -= dt;
        if self.next > 0.0 {
            return;
        }
        self.next = 0.1;
        // the nod: sinking deeper, catching itself, sinking again
        let nod = (self.t * 0.5).sin() * 0.14 * self.now + (self.t * 1.3).sin() * 0.04 * self.now;
        let depth = (self.now + nod).clamp(0.0, 1.1);
        // standing on: the balancer holds the legs, a little softer as it goes on (start only once: "start" false would
        // stop it)
        let stiff = [("bodyStiffness", Value::Float(10.5 - 2.5 * self.now)), ("damping", Value::Float(1.0))];
        if self.started {
            message(a, b, w, "bodyBalance", &stiff);
        } else {
            self.started = true;
            message(a, b, w, "bodyBalance", &[("start", Value::Bool(true)), stiff[0].clone(), stiff[1].clone()]);
        }
        let (n, sp, sh, el) = (delirium::nmrs::NECK(), delirium::nmrs::SPINE(), delirium::nmrs::SHOULDER(), delirium::nmrs::ELBOW());
        // bent over at the waist, the head hanging, the arms dangling down in front
        let mut pose: Vec<(usize, [f32; 3])> = sp.iter().map(|&j| (j, [0.27 * depth, 0.0, 0.0])).collect();
        pose.extend(n.iter().map(|&j| (j, [0.6 * depth, 0.0, 0.0])));
        pose.extend(sh.iter().map(|&j| (j, [0.55 * depth, 0.0, -0.08 * depth])));
        pose.extend(el.iter().map(|&j| (j, [0.2 * depth, 0.0, 0.0])));
        // (held firmly enough to stay folded against the weight of the upper body)
        message(a, b, w, "animPose", &[
            ("start", Value::Bool(true)),
            ("pose", Value::Str(pose_text(&pose))),
            ("mask", Value::Str("ub".into())),
            ("stiffness", Value::Float(9.0 - 2.0 * self.now)),
            ("damping", Value::Float(1.0)),
        ]);
    }

    fn stop(&mut self, a: &mut Agent, b: &Build, w: &Collision) {
        message(a, b, w, "animPose", &[("start", Value::Bool(false))]);
    }
}

/// dlBrainDamage: a hurt brain, by how bad (severity 0-1):
///   from 0.1, ataxia - the balance unsteady: a slow drunken sway, the head lolling, steps to catch it;
///   from 0.35, one side weak (hemiparesis: that arm and leg give, it leans and falls towards them);
///   from 0.6 posturing (lying down: all of it), and at 0.85 out of it - limp but rigid in the posture: decorticate
///   (arms drawn up to the chest, legs straight) or, worse, decerebrate (arms straight and turned in, the head arched
///   back, legs straight);
///   a seizure (seizure=true): stiff all over, then jerking in rhythm, slowing, then limp.
/// posture (0 none, 1 decorticate, 2 decerebrate, -1 by severity), side (0 left, 1 right, -1 either).
struct Brain {
    severity: f32,
    posture: i32,
    side: usize,
    rng: Rng,
    t: f32,
    next: f32,
    sway: f32,
    /// the seizure: how long since it began
    seizure: Option<f32>,
    relaxed: bool,
}

impl Brain {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        let severity = num(p, "severity", 0.5).clamp(0.0, 1.0);
        let mut rng = Rng(num(p, "seed", 7.0) as u32 | 1);
        let side = match num(p, "side", -1.0) as i32 {
            0 => 0,
            1 => 1,
            _ => (rng.next() < 0.5) as usize,
        };
        let sway = rng.range(0.0, std::f32::consts::TAU);
        Box::new(Brain {
            severity,
            posture: num(p, "posture", -1.0) as i32,
            side,
            rng,
            t: 0.0,
            next: 0.0,
            sway,
            seizure: flag(p, "seizure", false).then_some(0.0),
            relaxed: false,
        })
    }

    fn posture(&self) -> i32 {
        if self.posture >= 0 {
            self.posture
        } else if self.severity >= 0.85 {
            2
        } else if self.severity >= 0.6 {
            1
        } else {
            0
        }
    }

    /// The posturing pose: the arms (and, lying, the neck and legs too).
    fn posture_pose(kind: i32, legs: bool) -> Vec<(usize, [f32; 3])> {
        let (sh, el, n) = (delirium::nmrs::SHOULDER(), delirium::nmrs::ELBOW(), delirium::nmrs::NECK());
        let (h, k) = (delirium::nmrs::HIP(), delirium::nmrs::KNEE());
        let (shoulder, elbow, neck) = if kind == 1 { ([0.35, 0.0, -0.25], [1.7, 0.0, 0.0], 0.1) } else { ([0.1, 0.7, 0.1], [0.0, 0.0, 0.0], -0.45) };
        let mut pose: Vec<(usize, [f32; 3])> = sh.iter().map(|&j| (j, shoulder)).collect();
        pose.extend(el.iter().map(|&j| (j, elbow)));
        if legs {
            pose.extend(n.iter().map(|&j| (j, [neck, 0.0, 0.0])));
            pose.extend(h.iter().map(|&j| (j, [0.0, 0.0, 0.0])));
            pose.extend(k.iter().map(|&j| (j, [0.0, 0.0, 0.0])));
        }
        pose
    }
}

impl Custom for Brain {
    fn name(&self) -> &'static str {
        "dlBrainDamage"
    }

    fn poses(&self) -> bool {
        self.seizure.is_some() || self.posture() > 0
    }

    fn repose(&mut self) {
        self.next = 0.0;
    }

    fn set(&mut self, p: &[(String, Val)]) {
        self.severity = num(p, "severity", self.severity).clamp(0.0, 1.0);
        self.posture = num(p, "posture", self.posture as f32) as i32;
        if flag(p, "seizure", false) && self.seizure.is_none() {
            self.seizure = Some(0.0);
        }
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        let s = self.severity;
        self.t += dt;
        // a seizure: tonic (rigid, all extended, about 6 s), clonic (rhythmic jerks, 3 a second slowing to 1, 14 s), limp
        if let Some(ts) = self.seizure.as_mut() {
            *ts += dt;
            let ts = *ts;
            self.next -= dt;
            if ts > 20.0 {
                self.seizure = None;
                self.relaxed = true;
                message(a, b, w, "stopAllBehaviours", &[]);
                message(a, b, w, "bodyRelax", &[("mask", Value::Str("fb".into())), ("relaxation", Value::Float(85.0))]);
                return;
            }
            if self.next > 0.0 {
                return;
            }
            if ts < 6.0 {
                self.next = 0.25;
                if ts < 0.3 {
                    message(a, b, w, "stopAllBehaviours", &[]);
                }
                let pose = Self::posture_pose(2, true);
                message(a, b, w, "animPose", &[
                    ("start", Value::Bool(true)),
                    ("pose", Value::Str(pose_text(&pose))),
                    ("mask", Value::Str("fb".into())),
                    ("stiffness", Value::Float(13.0)),
                    ("damping", Value::Float(0.8)),
                ]);
            } else {
                // each jerk: a flex, then let go - faster at first
                let rate = 3.0 - 2.0 * ((ts - 6.0) / 14.0);
                self.next = 0.5 / rate;
                let flexed = ((self.t * rate * 2.0) as i64) % 2 == 0;
                let (sh, el, sp, n) = (delirium::nmrs::SHOULDER(), delirium::nmrs::ELBOW(), delirium::nmrs::SPINE(), delirium::nmrs::NECK());
                let f = if flexed { 1.0 } else { 0.0 };
                // (the whole upper body in each jerk: arms, trunk and head together)
                let mut pose: Vec<(usize, [f32; 3])> = sh.iter().map(|&j| (j, [0.8 * f - 0.2, 0.0, 0.4 - 0.6 * f])).collect();
                pose.extend(el.iter().map(|&j| (j, [1.6 * f, 0.0, 0.0])));
                pose.extend(sp.iter().map(|&j| (j, [0.18 * f - 0.05, 0.0, 0.0])));
                pose.extend(n.iter().map(|&j| (j, [0.5 * f - 0.3, 0.0, 0.0])));
                message(a, b, w, "animPose", &[
                    ("start", Value::Bool(true)),
                    ("pose", Value::Str(pose_text(&pose))),
                    ("mask", Value::Str("ub".into())),
                    ("stiffness", Value::Float(15.0)),
                    ("damping", Value::Float(0.4)),
                ]);
            }
            return;
        }
        self.next -= dt;
        if self.next > 0.0 {
            return;
        }
        self.next = 0.1;
        let standing = a.balancing();
        // out of it: limp, but for the posture
        if s >= 0.85 && !self.relaxed {
            self.relaxed = true;
            message(a, b, w, "stopAllBehaviours", &[]);
            message(a, b, w, "bodyRelax", &[("mask", Value::Str("fb".into())), ("relaxation", Value::Float(80.0))]);
        }
        // ataxia: the balance unsteady - a slow sway in a wandering direction, the knees soft, the head lolling
        if standing && s > 0.1 {
            self.sway += self.rng.range(-0.35, 0.35) + 0.05;
            let swell = 0.5 + 0.5 * (self.t * 0.7).sin() * (self.t * 0.23).cos();
            let lean = (0.06 + 0.28 * s) * swell;
            message(a, b, w, "leanInDirection", &[("leanAmount", Value::Float(lean)), ("dir", Value::Vec3(Vec3::new(self.sway.cos(), self.sway.sin(), 0.0)))]);
            message(a, b, w, "bodyBalance", &[("bodyStiffness", Value::Float(11.0 - 6.0 * s)), ("damping", Value::Float(1.0 - 0.4 * s))]);
            message(a, b, w, "setStiffness", &[("bodyStiffness", Value::Float(6.0 - 3.0 * s)), ("damping", Value::Float(0.6)), ("mask", Value::Str("un".into()))]);
        }
        // one side weak: that arm and leg give
        if s > 0.35 {
            let give = (8.0 * (1.0 - (s - 0.35) / 0.5)).clamp(1.0, 8.0);
            let (arm, leg) = if self.side == 0 { ("ul", "ll") } else { ("ur", "lr") };
            for m in [arm, leg] {
                message(a, b, w, "setStiffness", &[("bodyStiffness", Value::Float(give)), ("damping", Value::Float(0.5)), ("mask", Value::Str(m.into()))]);
            }
        }
        // posturing: the arms while standing; lying, all of it (rigid: stiffer the worse it is)
        let kind = self.posture();
        if kind > 0 {
            let pose = Self::posture_pose(kind, !standing);
            let mask = if standing { "ua" } else { "fb" };
            message(a, b, w, "animPose", &[
                ("start", Value::Bool(true)),
                ("pose", Value::Str(pose_text(&pose))),
                ("mask", Value::Str(mask.into())),
                ("stiffness", Value::Float(7.0 + 7.0 * s)),
                ("damping", Value::Float(1.0)),
            ]);
        }
    }

    fn stop(&mut self, a: &mut Agent, b: &Build, w: &Collision) {
        message(a, b, w, "animPose", &[("start", Value::Bool(false))]);
    }
}

thread_local! {
    // (the game's task tuning, for this module's behaviours that run the game's tasks: set when the module starts)
    static TASK_PARAMS: std::cell::RefCell<Option<std::rc::Rc<delirium_data::taskparams::TaskParams>>> = const { std::cell::RefCell::new(None) };
}

/// The game's task tuning, for this module's behaviours that run the game's own tasks.
pub fn set_task_params(tp: delirium_data::taskparams::TaskParams) {
    TASK_PARAMS.with(|t| *t.borrow_mut() = Some(std::rc::Rc::new(tp)));
}

/// dlOnFire: on fire - GTA IV's own: staggering about beating at the flames, falling, rolling and writhing on the floor;
/// put out (burning=false), it carries on a little and lets go. burning (true), health (0-200: weaker, it drops sooner),
/// dead.
struct Burn {
    task: delirium::nm::OnFire,
    started: bool,
    burning: bool,
    health: f32,
    dead: bool,
    rand: delirium_data::rng::Rand,
    tp: std::rc::Rc<delirium_data::taskparams::TaskParams>,
    done: bool,
}

impl Burn {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        let tp = TASK_PARAMS.with(|t| t.borrow().clone()).unwrap_or_default();
        Box::new(Burn {
            task: delirium::nm::OnFire::new("*"),
            started: false,
            burning: flag(p, "burning", true),
            health: num(p, "health", 150.0),
            dead: flag(p, "dead", false),
            rand: delirium_data::rng::Rand { seed: num(p, "seed", 3.0) as u32 | 1 },
            tp,
            done: false,
        })
    }
}

impl Custom for Burn {
    fn name(&self) -> &'static str {
        "dlOnFire"
    }

    fn set(&mut self, p: &[(String, Val)]) {
        self.burning = flag(p, "burning", self.burning);
        self.health = num(p, "health", self.health);
        self.dead = flag(p, "dead", self.dead);
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        if !self.started {
            self.started = true;
            let tp = self.tp.clone();
            self.task.start(a, b, w, &tp, &mut self.rand);
            return;
        }
        if a.feedback.iter().any(|f| f.slot == 8 && f.name == "balance") {
            self.task.balance_failed();
        }
        if self.task.update(a, b, w, &mut self.rand, dt, self.burning, self.health, self.dead).is_some() {
            self.done = true;
        }
    }

    fn after(&mut self, _a: &mut Agent, _b: &Build, _w: &Collision, _dt: f32) -> bool {
        !self.done
    }

    fn stop(&mut self, a: &mut Agent, b: &Build, w: &Collision) {
        // (over - put out, or stopped: the writhing and the lean it started end with it; in the game the next task
        // would take the body on, here whoever started it does)
        message(a, b, w, "stopAllBehaviours", &[]);
    }
}

/// dlStagger: knocked off balance - a kick, a shove, a blunt blow: driven on the way it was hit for a moment, the
/// balancer stepping and stepping to catch it, the arms flailing and the trunk thrown back; then caught (it stands), or
/// - hit too hard to catch - down (catchFall). dir (the blow's way, world), strength (its push, N s), fall (0-1: how
/// likely it goes down; default by strength).
struct Stagger {
    dir: Vec3,
    strength: f32,
    t: f32,
    dur: f32,
    falls_at: Option<f32>,
    started: bool,
    done: bool,
}

// (how hard it's driven: its extra push over the stagger, times the blow's; the most, m/s/s)
const STAGGER_PUSH: f32 = 2.6;
const STAGGER_MOST: f32 = 7.0;

impl Stagger {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        let strength = num(p, "strength", 25.0).clamp(1.0, 120.0);
        let fall = num(p, "fall", ((strength - 30.0) / 30.0).clamp(0.0, 0.95)).clamp(0.0, 1.0);
        let seed = num(p, "seed", 3.0) as u32;
        // (a coin for the fall, from the seed: down or caught)
        let coin = ((seed.wrapping_mul(2654435761) >> 8) % 1000) as f32 / 1000.0;
        let dur = (0.35 + strength / 55.0).clamp(0.4, 1.6);
        Box::new(Stagger {
            dir: vec(p, "dir").unwrap_or(Vec3::X).with_z(0.0).normalize_or(Vec3::X),
            strength,
            t: 0.0,
            dur,
            falls_at: (coin < fall).then_some(dur * (0.45 + 0.4 * coin / fall.max(1e-3))),
            started: false,
            done: false,
        })
    }
}

impl Custom for Stagger {
    fn name(&self) -> &'static str {
        "dlStagger"
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        if self.done {
            return;
        }
        let at = a.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
        if !self.started {
            self.started = true;
            a.fallen = false;
            if !a.balancing() {
                message(a, b, w, "bodyBalance", &[("start", Value::Bool(true)), ("maxSteps", Value::Int(100))]);
            } else {
                message(a, b, w, "bodyBalance", &[("maxSteps", Value::Int(100))]);
            }
            message(a, b, w, "armsWindmillAdaptive", &[("start", Value::Bool(true)), ("disableOnImpact", Value::Bool(true))]);
            // (the trunk thrown back the way of the blow: a pair, chest one way and pelvis the other)
            let chest = 10.min(b.parts.len() - 1);
            push(a, chest, at + Vec3::Z * 0.6 + self.dir * 0.05, self.dir * (0.15 * self.strength));
            push(a, 0, at - self.dir * 0.05, -self.dir * (0.15 * self.strength));
        }
        self.t += dt;
        // driven on, less and less: it has to keep stepping to catch itself
        if self.t < self.dur {
            let m = a.doll.mass();
            let left = 1.0 - self.t / self.dur;
            let a0 = (3.0 * STAGGER_PUSH * self.strength / (m * self.dur)).min(STAGGER_MOST);
            push(a, 0, at, self.dir * (m * a0 * left * left * dt));
        }
        // too hard to catch: down
        if let Some(when) = self.falls_at {
            if self.t >= when {
                self.done = true;
                message(a, b, w, "armsWindmillAdaptive", &[("start", Value::Bool(false))]);
                message(a, b, w, "bodyBalance", &[("start", Value::Bool(false))]);
                message(a, b, w, "catchFall", &[("start", Value::Bool(true))]);
                return;
            }
        }
        if self.t >= self.dur + 0.4 || (self.t > 0.3 && !a.balancing()) {
            self.done = true;
            message(a, b, w, "armsWindmillAdaptive", &[("start", Value::Bool(false))]);
        }
    }

    fn after(&mut self, _a: &mut Agent, _b: &Build, _w: &Collision, _dt: f32) -> bool {
        !self.done
    }
}

/// dlWalk: walking as Delirium has it - GTA IV's own player control under Euphoria: the body pushed at the pelvis the way
/// it's steered, the balancer stepping under it and turning the body, the head looking where it's told. dir (the way to
/// go, world), speed (0 standing, 1 walking; up to 1.5), look (where the head looks; none: ahead). It keeps balancing
/// while it runs; standing still it only balances. Not on its feet (fallen, or a reaction has the body), it waits.
struct Walk {
    dir: Vec3,
    speed: f32,
    look: Option<Vec3>,
    started: bool,
    send: f32,
    wander: delirium::playerctl::Wander,
    rand: delirium_data::rng::Rand,
}

const WALK_STEPS: i32 = 10_000;

impl Walk {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        Box::new(Walk {
            dir: vec(p, "dir").unwrap_or(Vec3::ZERO).with_z(0.0),
            speed: num(p, "speed", 0.0).clamp(0.0, 1.5),
            look: vec(p, "look"),
            started: false,
            send: 0.0,
            wander: Default::default(),
            rand: delirium_data::rng::Rand { seed: num(p, "seed", 5.0) as u32 | 1 },
        })
    }
}

impl Custom for Walk {
    fn name(&self) -> &'static str {
        "dlWalk"
    }

    fn set(&mut self, p: &[(String, Val)]) {
        if let Some(d) = vec(p, "dir") {
            self.dir = d.with_z(0.0);
        }
        self.speed = num(p, "speed", self.speed).clamp(0.0, 1.5);
        if let Some(l) = vec(p, "look") {
            self.look = Some(l);
        }
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        let ped = a.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
        let dir = self.dir.normalize_or_zero();
        let look = self.look.unwrap_or(ped + if dir == Vec3::ZERO { Vec3::ZERO } else { dir * 5.0 });
        let turn = [("useBodyTurn", Value::Bool(true)), ("useHeadLook", Value::Bool(true)), ("headLookPos", Value::Vec3(look))];
        if !self.started {
            self.started = true;
            // (balancing afresh: a body that fell before isn't the engine's fallen one now)
            if !a.balancing() {
                a.fallen = false;
                let mut ps = vec![("start", Value::Bool(true)), ("maxSteps", Value::Int(WALK_STEPS))];
                ps.extend(turn.iter().cloned());
                message(a, b, w, "bodyBalance", &ps);
            }
        }
        if !a.balancing() {
            return;
        }
        if self.speed > 0.02 && dir != Vec3::ZERO {
            let camera = (-dir.x).atan2(dir.y);
            let stick = (0, -((127.0 * self.speed).round() as i32));
            let (f, _) = delirium::playerctl::frame(stick, camera, ped, a.doll.mass(), &mut self.wander, &mut self.rand);
            push(a, 0, ped, f * dt);
        }
        self.send -= dt;
        if self.send <= 0.0 {
            self.send = 0.1;
            let mut ps = vec![("maxSteps", Value::Int(WALK_STEPS))];
            ps.extend(turn.iter().cloned());
            message(a, b, w, "bodyBalance", &ps);
        }
    }
}

/// dlShotReact: what a bullet does by where it lands, over GTA IV's own shot (which reaches for the wound, spins with the
/// hit, staggers and lets a shot leg collapse): region - "gut" (doubled over, the trunk folding round the wound), "groin"
/// (doubled over lower), "chest" (knocked back: the trunk thrown back, the head snapping back), "shoulder" (spun round,
/// the hit shoulder thrown back), "arm" (the arm flung back), "leg" (nothing of its own: the leg giving is the injury's
/// and GTA IV's - a lean onto it set the balancer swaying for seconds), "head" (the head snapped
/// back, then stunned: the neck slack, the head lolling), "neck" (the chin tucked in). side (0 left, 1 right: the side
/// hit), dir (the bullet's way, world), power (0-2: 1 a pistol or rifle round), seed. Plays out in a second or two and
/// ends by itself. (A limb it leaves weak - a dead leg, a dropped arm - is the injury message's: the caller sends that.)
#[derive(Clone, Copy, PartialEq, Debug)]
enum Region {
    Gut,
    Groin,
    Chest,
    Shoulder,
    Arm,
    Leg,
    Head,
    Neck,
}

struct ShotReact {
    region: Region,
    side: usize,
    dir: Vec3,
    power: f32,
    t: f32,
    /// at its full until, then easing off until dur
    hold: f32,
    dur: f32,
    next: f32,
    kicked: bool,
    posed: bool,
    /// the spine's and the neck's joints as they were before it was hit: posed from, and handed back as they were -
    /// their angles, stiffness and gravity compensation (nothing else sets them afresh: the pose's left behind, the
    /// head would hang and the trunk stay bent). Hit again meanwhile, it carries on from these (see restart).
    base: Vec<(usize, delirium::nmrs::Effector)>,
}

impl ShotReact {
    fn new(p: &[(String, Val)]) -> ShotReact {
        let region = match p.iter().find(|(k, _)| k == "region").map(|(_, v)| v) {
            Some(Val::Str(s)) => match s.as_str() {
                "groin" => Region::Groin,
                "chest" => Region::Chest,
                "shoulder" => Region::Shoulder,
                "arm" => Region::Arm,
                "leg" => Region::Leg,
                "head" => Region::Head,
                "neck" => Region::Neck,
                _ => Region::Gut,
            },
            _ => Region::Gut,
        };
        let (hold, dur) = match region {
            Region::Gut => (0.9, 1.8),
            Region::Groin => (1.1, 2.1),
            Region::Chest => (0.15, 0.7),
            Region::Shoulder => (0.2, 0.8),
            Region::Arm => (0.1, 0.5),
            Region::Leg => (0.5, 0.9),
            Region::Head => (0.25, 1.8),
            Region::Neck => (0.6, 1.2),
        };
        ShotReact {
            region,
            side: if num(p, "side", 1.0) as i32 == 0 { 0 } else { 1 },
            dir: vec(p, "dir").unwrap_or(Vec3::X).normalize_or(Vec3::X),
            power: num(p, "power", 1.0).clamp(0.0, 2.0),
            t: 0.0,
            hold,
            dur,
            next: 0.0,
            kicked: false,
            posed: false,
            base: vec![],
        }
    }

    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        Box::new(ShotReact::new(p))
    }

    /// How much of it there is now: in fast, held, then easing off.
    fn envelope(&self) -> f32 {
        // (doubling over comes on over a moment: thrown forward at once, it would only go over on its face)
        let rise = (self.t / if matches!(self.region, Region::Gut | Region::Groin) { 0.3 } else { 0.12 }).min(1.0);
        let fall = if self.t <= self.hold { 1.0 } else { 1.0 - ((self.t - self.hold) / (self.dur - self.hold).max(1e-3)).clamp(0.0, 1.0) };
        rise * fall * fall * (3.0 - 2.0 * fall)
    }

    /// The trunk and neck it holds (bend forward, sideways, twist each), how stiffly, and the mask.
    fn pose(&self, e: f32, standing: bool) -> Option<(Vec<(usize, [f32; 3])>, f32, &'static str)> {
        let (sp, n) = (delirium::nmrs::SPINE(), delirium::nmrs::NECK());
        let k = if standing { 1.0 } else { 0.6 };
        let (spine, neck, stiff, mask) = match self.region {
            Region::Gut => (0.17, 0.4, 9.0, "uk"),
            Region::Groin => (0.2, 0.45, 9.0, "uk"),
            Region::Chest if standing => (-0.12, -0.45, 10.0, "uk"),
            Region::Neck => (0.05, 0.35, 8.0, "uk"),
            // (the head snapped back first, then slack: lolling, as stunned)
            Region::Head if self.t < 0.15 => (0.0, -0.35, 10.0, "un"),
            Region::Head => (0.0, 0.75, 4.0, "un"),
            _ => return None,
        };
        // (from where each joint was: bent that much further)
        let from = |j: usize, by: f32| -> (usize, [f32; 3]) {
            let b = self.base.iter().find(|(i, _)| *i == j).map_or([0.0; 3], |(_, e)| e.desired);
            (j, [b[0] + by, b[1], b[2]])
        };
        let mut pose: Vec<(usize, [f32; 3])> = if mask == "uk" { sp.iter().map(|&j| from(j, spine * e * k)).collect() } else { vec![] };
        // (stunned: limp, so not eased off - the head hangs until it ends)
        let ne = if self.region == Region::Head && self.t >= 0.15 { 1.0 } else { e };
        pose.extend(n.iter().map(|&j| from(j, neck * ne * k)));
        Some((pose, stiff, mask))
    }

    /// Its pose let go of, the spine and neck handed back as they were.
    fn unpose(&mut self, a: &mut Agent, b: &Build, w: &Collision) {
        if self.posed {
            self.posed = false;
            message(a, b, w, "animPose", &[("start", Value::Bool(false))]);
            for &(j, e) in &self.base {
                a.set_effector(j, e);
            }
        }
    }
}

impl Custom for ShotReact {
    fn name(&self) -> &'static str {
        "dlShotReact"
    }

    fn poses(&self) -> bool {
        self.posed
    }

    fn repose(&mut self) {
        self.next = 0.0;
    }

    // (ended as the body goes limp or dies - restore=false: its trunk and neck aren't handed back the tone they had
    // standing; the pose let go of, they're as the going limp left them)
    fn set(&mut self, p: &[(String, Val)]) {
        if !flag(p, "restore", true) {
            self.base.clear();
        }
    }

    // hit again while it plays out: the new hit's reaction, from the spine and neck as they were before the first
    // (stopped and begun anew, the first would hand them back over what the new hit's own shot has just set)
    fn restart(&mut self, p: &[(String, Val)]) -> bool {
        let fresh = ShotReact::new(p);
        *self = ShotReact { base: std::mem::take(&mut self.base), posed: self.posed, ..fresh };
        true
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        self.t += dt;
        let standing = a.balancing();
        let flat = self.dir.with_z(0.0).normalize_or(self.dir);
        if !self.kicked {
            self.kicked = true;
            if self.base.is_empty() {
                self.base = delirium::nmrs::SPINE().iter().chain(delirium::nmrs::NECK().iter()).filter_map(|&j| a.effector(j).map(|e| (j, e))).collect();
            }
            let (shoulder, other, fore) = if self.side == 0 { (14, 18, 15) } else { (18, 14, 19) };
            let j = 9.0 * self.power;
            match self.region {
                // spun round by it: the hit shoulder thrown back, the other one forward - a turn, no push
                Region::Shoulder => {
                    let (s, o) = (pivot(a, b, shoulder), pivot(a, b, other));
                    push(a, shoulder, s, flat * j);
                    push(a, other, o, -flat * j);
                }
                Region::Arm => {
                    let at = a.doll.frames().nth(fore).map_or(Vec3::ZERO, |f| f.translation);
                    push(a, fore, at, flat * (0.45 * j));
                }
                // knocked back: the chest the way of the bullet, the pelvis held where it was
                Region::Chest if standing => {
                    let c = a.doll.frames().nth(10).map_or(Vec3::ZERO, |f| f.translation);
                    let p0 = a.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
                    push(a, 10, c, flat * (0.7 * j));
                    push(a, 0, p0, -flat * (0.35 * j));
                }
                Region::Head => {
                    let h = a.doll.frames().nth(12).map_or(Vec3::ZERO, |f| f.translation);
                    push(a, 12, h, self.dir * (0.15 * j));
                }
                _ => {}
            }
        }
        self.next -= dt;
        if self.next > 0.0 || self.t > self.dur {
            return;
        }
        self.next = 0.05;
        match self.pose(self.envelope(), standing) {
            Some((pose, stiff, mask)) => {
                self.posed = true;
                // (held up against its weight as it was: the pose's own would let the trunk and head sag on their own)
                let held: Vec<f32> = pose.iter().filter_map(|(j, _)| self.base.iter().find(|(i, _)| i == j)).map(|(_, e)| e.oppose_gravity).collect();
                let og = if held.is_empty() { 0.0 } else { held.iter().sum::<f32>() / held.len() as f32 };
                // (stunned, the head is let go of: it hangs)
                let og = if self.region == Region::Head && self.t >= 0.15 { 0.0 } else { og };
                message(a, b, w, "animPose", &[
                    ("start", Value::Bool(true)),
                    ("pose", Value::Str(pose_text(&pose))),
                    ("mask", Value::Str(mask.into())),
                    ("stiffness", Value::Float(stiff)),
                    ("damping", Value::Float(if stiff < 4.0 { 0.6 } else { 1.0 })),
                    ("gravityCompensation", Value::Float(og)),
                ]);
            }
            // (hit again where there's no pose - an arm after the gut: the last one's let go of, not held frozen)
            None => self.unpose(a, b, w),
        }
    }

    fn after(&mut self, _a: &mut Agent, _b: &Build, _w: &Collision, _dt: f32) -> bool {
        self.t <= self.dur
    }

    fn stop(&mut self, a: &mut Agent, b: &Build, w: &Collision) {
        self.unpose(a, b, w);
    }
}

/// dlBrace: arms out against a wall it's thrown at - staggering, shoved or falling into it: both hands go out to the
/// wall in front of the shoulders, the elbows giving as they land on it. pos (a point on the wall, world), normal (the
/// wall's face, out towards the body), time (the most it goes on, s: 1.2), useLeft / useRight (the hands it may use:
/// not one holding a gun or held by something else). Over, the arms are put back as they were (the springs leave them
/// soft) - unless it's ended with restore=false (gone limp).
struct Brace {
    pos: Vec3,
    normal: Vec3,
    time: f32,
    t: f32,
    held: [Vec3; 2],
    touched: Option<f32>,
    hands: [bool; 2],
    /// the arms' joints as they were (angles, stiffness): put back when it's over
    arms: Vec<(usize, delirium::nmrs::Effector)>,
    restore: bool,
}

impl Brace {
    fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        Box::new(Brace {
            pos: vec(p, "pos").unwrap_or(Vec3::ZERO),
            normal: vec(p, "normal").unwrap_or(Vec3::NEG_X).normalize_or(Vec3::NEG_X),
            time: num(p, "time", 1.2).clamp(0.2, 4.0),
            t: 0.0,
            held: [Vec3::ZERO; 2],
            touched: None,
            hands: [flag(p, "useLeft", true), flag(p, "useRight", true)],
            arms: vec![],
            restore: true,
        })
    }

    /// Where a hand goes: on the wall in front of its shoulder (a little lower), or as far towards it as the arm reaches.
    fn target(&self, a: &Agent, b: &Build, hand: usize) -> (Vec3, f32) {
        let (upper, fore) = if hand == L_HAND { (14, 15) } else { (18, 19) };
        let shoulder = pivot(a, b, upper);
        let reach = (pivot(a, b, fore) - shoulder).length() + (pivot(a, b, hand) - pivot(a, b, fore)).length() + b.parts[hand].radius;
        let off = (shoulder - self.pos).dot(self.normal);
        let on = shoulder - self.normal * (off - b.parts[hand].radius - 0.01) - Vec3::Z * 0.06;
        let d = on - shoulder;
        let to = if d.length() > reach * 0.95 { shoulder + d.normalize_or_zero() * reach * 0.95 } else { on };
        let hand_at = a.doll.frames().nth(hand).map_or(Vec3::ZERO, |f| f.translation);
        (to, (hand_at - self.pos).dot(self.normal) - b.parts[hand].radius)
    }
}

impl Custom for Brace {
    fn name(&self) -> &'static str {
        "dlBrace"
    }

    fn set(&mut self, p: &[(String, Val)]) {
        if let Some(v) = vec(p, "pos") {
            self.pos = v;
        }
        if let Some(n) = vec(p, "normal") {
            self.normal = n.normalize_or(self.normal);
        }
        self.restore = flag(p, "restore", self.restore);
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        if self.t == 0.0 {
            use delirium::nmrs::{CLAVICLE, ELBOW, SHOULDER, WRIST};
            for (side, used) in self.hands.into_iter().enumerate() {
                if used {
                    for j in [CLAVICLE()[side], SHOULDER()[side], ELBOW()[side], WRIST()[side]] {
                        if let Some(e) = a.effector(j) {
                            self.arms.push((j, e));
                        }
                    }
                }
            }
        }
        self.t += dt;
        for (i, hand) in [L_HAND, R_HAND].into_iter().enumerate() {
            if !self.hands[i] {
                continue;
            }
            let (to, gap) = self.target(a, b, hand);
            if gap < 0.04 && self.touched.is_none() {
                self.touched = Some(self.t);
            }
            // (reaching out firmly; on the wall, the elbows give with the weight coming onto them)
            let k = if self.touched.is_some() { 8.0 } else { 15.0 };
            let mut held = self.held[i];
            spring_pair(a, b, w, hand, to, 10, k, 350.0, &mut held, dt);
            self.held[i] = held;
        }
    }

    fn after(&mut self, a: &mut Agent, _b: &Build, _w: &Collision, _dt: f32) -> bool {
        // over: its time up, or on the wall a while and no longer going into it (or no hand to brace with)
        let into = -a.doll.vel(0).0.dot(self.normal);
        !(self.t > self.time || self.touched.is_some_and(|t| self.t - t > 0.5 && into < 0.3) || self.hands == [false, false])
    }

    fn stop(&mut self, a: &mut Agent, _b: &Build, _w: &Collision) {
        if self.restore {
            for &(j, e) in &self.arms {
                a.set_effector(j, e);
            }
        }
    }
}

pub fn names() -> Vec<&'static str> {
    table().iter().map(|(n, _)| *n).collect()
}

pub fn is_custom(name: &str) -> bool {
    name.starts_with("dl")
}

pub fn send(b: &mut Body, w: &Collision, name: &str, params: &[(String, Val)]) -> Result<usize, String> {
    let Some(&(_, make)) = table().iter().find(|(n, _)| *n == name) else { return Err(format!("delirium: no behaviour {name}")) };
    let start = params.iter().find(|(k, _)| k == "start").map(|(_, v)| matches!(v, Val::Bool(true)) || matches!(v, Val::Num(n) if *n != 0.0));
    let rest: Vec<(String, Val)> = params.iter().filter(|(k, _)| k != "start").map(|(k, v)| (k.clone(), v.clone())).collect();
    let running = b.custom.iter().position(|c| c.name() == name);
    match (start, running) {
        (Some(false), Some(i)) => {
            let mut c = b.custom.remove(i);
            // (what it's told as it ends: restore=false - the body gone limp or dead - and the like)
            c.set(&rest);
            let posed = c.poses();
            c.stop(&mut b.agent, &b.build, w);
            reposed(&mut b.custom, posed);
        }
        (Some(false), None) => {}
        (Some(true), Some(i)) if b.custom[i].restart(&rest) => {}
        (Some(true), Some(i)) => {
            let mut old = b.custom.remove(i);
            let posed = old.poses();
            old.stop(&mut b.agent, &b.build, w);
            reposed(&mut b.custom, posed);
            b.custom.push(make(&rest));
        }
        (Some(true), None) => b.custom.push(make(&rest)),
        (None, Some(i)) => b.custom[i].set(&rest),
        (None, None) => {}
    }
    Ok(0)
}

/// An engine message from inside a behaviour (its errors ignored: the names are this file's own).
pub fn message(a: &mut Agent, b: &Build, w: &Collision, name: &str, ps: &[(&str, Value)]) {
    let mut st = std::mem::take(&mut a.msgs);
    let _ = msg::send(a, b, w, &mut st, name, ps);
    a.msgs = st;
}

/// A parameter's number, or `or`.
pub fn num(params: &[(String, Val)], k: &str, or: f32) -> f32 {
    params.iter().find(|(n, _)| n == k).and_then(|(_, v)| if let Val::Num(n) = v { Some(*n as f32) } else { None }).unwrap_or(or)
}

pub fn vec(params: &[(String, Val)], k: &str) -> Option<Vec3> {
    params.iter().find(|(n, _)| n == k).and_then(|(_, v)| if let Val::Vec(v) = v { Some(Vec3::from_array(*v)) } else { None })
}

pub fn flag(params: &[(String, Val)], k: &str, or: bool) -> bool {
    params.iter().find(|(n, _)| n == k).map_or(or, |(_, v)| matches!(v, Val::Bool(true)) || matches!(v, Val::Num(n) if *n != 0.0))
}
