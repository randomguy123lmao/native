//! dlGetUp: up off the floor, the three ways Delirium's own viewer has (its get-up setting):
//!
//! - animation: GTA IV's own get-up (get_up_fast, from the front or the back, whichever it lies nearest) played on
//!   the body. The parts' targets are handed to the game (delirium.GetUpTarget), which moves the ragdoll's bones to
//!   them; the muscles follow the same pose. It always gets up.
//! - muscles: the clip's poses are only the muscles' targets (as the game's NMPose task has them), with a little help
//!   at the pelvis - the body does the work, and can fail (slip, topple, be knocked down).
//! - learned: a trained network works the muscles - it can fail too. Delirium's own getup.net (its joints from
//!   standing, nothing else) never stood Homer up; a coached one (trained by examples/getuptrain.rs) follows the
//!   muscles' get-up and corrects each joint as it goes, with as little of their help as it learned to need.
//!
//! Once up it balances (bodyBalance). Parameters: mode ("animation", "muscles", "learned", or 0 1 2), net (getup.net's
//! path, for learned), help (the pelvis help, times the body's weight: 0.5), blend (s from how it lies into the clip).

use crate::custom::{Custom, flag, message, num, push};
use crate::game::{Clips, Rise};
use crate::lua::Val;
use delirium::collision::Collision;
use delirium::msg::Value;
use delirium::nm::Agent;
use delirium::prelude::*;
use delirium::ragdoll::Build;
use std::cell::RefCell;
use std::sync::Arc;

/// GTA IV's get-up clips, with Homer's leg length and how high his feet's centres stand off the floor.
#[derive(Clone)]
pub struct ClipSet {
    pub clips: Arc<Clips>,
    pub leg: f32,
    pub feet: f32,
}

thread_local! {
    // (set when the module starts)
    static CLIPS: RefCell<Option<ClipSet>> = const { RefCell::new(None) };
    // (the trained network, by its path: loaded once)
    static NETS: RefCell<Vec<(String, Option<Arc<Net>>)>> = const { RefCell::new(vec![]) };
}

/// The clips this thread's get-ups use.
pub fn clip_set() -> Option<ClipSet> {
    CLIPS.with(|c| c.borrow().clone())
}

/// The clips for this thread's get-ups (a trainer's threads: the module's set, shared).
pub fn use_clip_set(set: Option<ClipSet>) {
    CLIPS.with(|c| *c.borrow_mut() = set);
}

/// The clips, for Homer (`build`, standing as `frames`).
pub fn set_clips(clips: Option<Clips>, build: &Build, frames: &[Transform]) {
    let lowest = build.parts.iter().zip(frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
    let feet = FEET.iter().filter_map(|&k| frames.get(k)).map(|f| f.translation.z).fold(f32::MAX, f32::min) - lowest;
    let set = clips.filter(Clips::has_getup).map(|c| ClipSet { clips: Arc::new(c), leg: crate::game::leg_length(build), feet: feet.clamp(0.0, 0.2) });
    CLIPS.with(|c| *c.borrow_mut() = set);
}

pub fn available() -> bool {
    CLIPS.with(|c| c.borrow().is_some())
}

const HEAD: usize = 12;
const FEET: [usize; 2] = [3, 6];
const MUSCLE_STIFFNESS: (f32, f32) = (11.0, 12.0);
// (the muscles' get-up: the clip played slower, the body's own dynamics need the time; the trunk's reach for the help's
// turn, metres)
const MUSCLE_SPEED: f32 = 0.7;
const TRUNK_REACH: f32 = 0.5;
const MUSCLE_HOLD: f32 = 1.2;
// (up: the help stays on a moment while the balance takes over, towards standing)
const STEADY: f32 = 2.5;
// (the learned get-up: the network's step, 60 a second, and how long it has)
const NET_DT: f32 = 1.0 / 60.0;
const NET_STEPS: usize = 480;
// (a coached network: how far it may turn a joint from the clip's, radians)
const COACH_TURN: f32 = 0.6;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Animation,
    Muscles,
    Learned,
}

/// A trained get-up (getup.net): a small network, the body's state in, the muscles' targets out. Delirium's own sees
/// the body alone; a coached one (see coach_drive) sees how far along the clip it is too, and comes with the help at the
/// pelvis and the clip's speed it learned with (getup.net.cfg beside it).
pub struct Net {
    pub sizes: [usize; 4],
    pub w: Vec<f32>,
    pub help: f32,
    pub speed: f32,
}

impl Net {
    pub fn len(sizes: [usize; 4]) -> usize {
        (0..3).map(|l| sizes[l] * sizes[l + 1] + sizes[l + 1]).sum()
    }

    pub fn new(sizes: [usize; 4], w: Vec<f32>, help: f32, speed: f32) -> Net {
        Net { sizes, w, help, speed }
    }

    /// Coached (it sees the clip): its inputs are the body's and three more.
    pub fn coached(&self, parts: usize) -> bool {
        self.sizes[0] == observe_len(parts) + 3
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let mut b: Vec<u8> = self.sizes.iter().flat_map(|&s| (s as u32).to_le_bytes()).collect();
        b.extend(self.w.iter().flat_map(|x| x.to_le_bytes()));
        let tmp = format!("{path}.tmp");
        std::fs::write(&tmp, b)?;
        std::fs::rename(&tmp, path)?;
        std::fs::write(format!("{path}.cfg"), format!("help {}\nspeed {}\n", self.help, self.speed))
    }

    pub fn load(path: &str) -> Option<Net> {
        let b = std::fs::read(path).ok()?;
        if b.len() < 16 || (b.len() - 16) % 4 != 0 {
            return None;
        }
        let u = |i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
        let sizes = [u(0), u(1), u(2), u(3)];
        let count = (0..3).try_fold(0usize, |n, l| n.checked_add(sizes[l].checked_mul(sizes[l + 1])?.checked_add(sizes[l + 1])?))?;
        if sizes.contains(&0) || count != (b.len() - 16) / 4 {
            return None;
        }
        let w: Vec<f32> = b[16..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        if !w.iter().all(|x| x.is_finite()) {
            return None;
        }
        let (mut help, mut speed) = (0.5, MUSCLE_SPEED);
        for line in std::fs::read_to_string(format!("{path}.cfg")).unwrap_or_default().lines() {
            let mut it = line.split_whitespace();
            match (it.next(), it.next().and_then(|v| v.parse::<f32>().ok())) {
                (Some("help"), Some(v)) => help = v.clamp(0.0, 2.0),
                (Some("speed"), Some(v)) => speed = v.clamp(0.2, 2.0),
                _ => {}
            }
        }
        Some(Net { sizes, w, help, speed })
    }

    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        let mut a = x.to_vec();
        a.resize(a.len().max(self.sizes[0]), 0.0);
        let mut o = 0;
        for l in 0..3 {
            let (n, m) = (self.sizes[l], self.sizes[l + 1]);
            let (wl, b) = (&self.w[o..o + n * m], &self.w[o + n * m..o + n * m + m]);
            a = (0..m).map(|j| (b[j] + (0..n).map(|i| wl[j * n + i] * a[i]).sum::<f32>()).tanh()).collect();
            o += n * m + m;
        }
        a
    }

    pub fn outputs(&self) -> usize {
        self.sizes[3]
    }
}

/// The network at `path` (or found beside the game), loaded once.
fn net(path: &str) -> Option<Arc<Net>> {
    let candidates: Vec<std::path::PathBuf> = {
        let mut v = vec![];
        if !path.is_empty() {
            v.push(path.into());
            v.push(std::path::Path::new("garrysmod").join(path));
        }
        for rel in ["garrysmod/addons/delirium/getup.net", "addons/delirium/getup.net"] {
            v.push(rel.into());
            if let Ok(e) = std::env::current_exe() {
                v.extend(e.ancestors().skip(1).take(4).map(|d| d.join(rel)));
            }
        }
        v
    };
    let key = path.to_string();
    if let Some(n) = NETS.with(|n| n.borrow().iter().find(|(k, _)| *k == key).map(|(_, n)| n.clone())) {
        return n;
    }
    let found = candidates.iter().find_map(|p| Net::load(&p.to_string_lossy())).map(Arc::new);
    NETS.with(|n| n.borrow_mut().push((key, found.clone())));
    found
}

/// getup.net is there (at `path`, or beside the game).
pub fn has_net(path: &str) -> bool {
    net(path).is_some()
}

/// What the network sees (as Delirium trained it): the pelvis' height off the floor and which way is up for it, then
/// each part's place, speed and up from the pelvis, turned to the body's heading; the pelvis' own speed.
pub fn observe(a: &Agent, floor: f32) -> Vec<f32> {
    let fr: Vec<Transform> = a.doll.frames().collect();
    let root = fr[0];
    let f = (root.rotation * Vec3::X).with_z(0.0).normalize_or(Vec3::X);
    let inv = Quat::from_rotation_z(-f.y.atan2(f.x));
    let mut o = vec![root.translation.z - floor];
    o.extend((inv * (root.rotation * Vec3::Z)).to_array());
    let v0 = a.doll.motion(0).0;
    for (k, t) in fr.iter().enumerate() {
        o.extend((inv * (t.translation - root.translation)).to_array());
        o.extend((inv * (a.doll.motion(k).0 - v0) * 0.2).to_array());
        o.extend((inv * (t.rotation * Vec3::Z)).to_array());
    }
    o.extend((inv * v0 * 0.2).to_array());
    o
}

/// How many numbers observe gives for a body of `parts` parts.
pub fn observe_len(parts: usize) -> usize {
    4 + 9 * parts + 3
}

/// What a coached network sees: the body, and how far along the clip it is (from the front, or the back).
pub fn coach_obs(a: &Agent, floor: f32, phase: f32, back: bool) -> Vec<f32> {
    let mut o = observe(a, floor);
    let phase = phase.clamp(0.0, 1.5);
    o.extend([phase, 1.0 - phase, if back { 1.0 } else { -1.0 }]);
    o
}

/// A coached network's muscles: the clip's pose (its joints, as the muscles' get-up has them), each joint turned a
/// little more by what the network says, and how stiff. All zeros: the muscles' get-up itself.
pub fn coach_drive(a: &mut Agent, act: &[f32], clip: &[Transform], build: &Build) {
    let n = 3 * (build.parts.len() - 1);
    if act.len() < n + 2 || clip.len() != build.parts.len() {
        delirium::nm::task_pose_control(a, clip.to_vec());
        return;
    }
    let mut rot: Vec<Quat> = Vec::with_capacity(clip.len());
    for (i, p) in build.parts.iter().enumerate() {
        let r = match p.parent {
            None => clip[0].rotation,
            Some(j) => {
                let turn = Vec3::new(act[3 * (i - 1)], act[3 * (i - 1) + 1], act[3 * (i - 1) + 2]) * COACH_TURN;
                rot[j] * (clip[j].rotation.inverse() * clip[i].rotation) * Quat::from_scaled_axis(turn)
            }
        };
        rot.push(r.normalize());
    }
    a.set_stiffness(0.5 * (MUSCLE_STIFFNESS.0 + MUSCLE_STIFFNESS.1) + 3.0 * act[n], 1.0);
    a.muscle_stiffness_x(2f32.powf(act[n + 1]));
    delirium::nm::task_pose_control(a, rot.into_iter().zip(clip).map(|(r, c)| c.with_rotation(r)).collect());
}

/// The network's muscles: each joint turned from standing by what it says, and how stiff.
pub fn drive(a: &mut Agent, act: &[f32], zero: &[Quat], build: &Build) {
    let n = 3 * (build.parts.len() - 1);
    if act.len() < n + 1 {
        return;
    }
    let root = a.doll.frames().next().unwrap_or_default();
    let mut rot: Vec<Quat> = Vec::with_capacity(zero.len());
    for (i, p) in build.parts.iter().enumerate() {
        let r = match p.parent {
            None => root.rotation,
            Some(j) => rot[j] * zero[i] * Quat::from_scaled_axis(Vec3::new(act[3 * (i - 1)], act[3 * (i - 1) + 1], act[3 * (i - 1) + 2]) * 1.2),
        };
        rot.push(r);
    }
    a.set_stiffness(8.0 + 4.0 * act[n], 1.0);
    a.muscle_stiffness_x(act.get(n + 1).map_or(1.0, |&m| 4f32.powf(m)));
    delirium::nm::task_pose_control(a, rot.into_iter().map(Transform::from_rotation).collect());
}

/// An angular impulse on a part, as a couple of equal and opposite pushes about `at`.
fn push_torque(a: &mut Agent, k: usize, at: Vec3, l: Vec3) {
    let n = l.length();
    if n < 1e-6 {
        return;
    }
    let axis = l / n;
    let u = axis.any_orthonormal_vector();
    let d = u.cross(axis);
    let h = 0.25;
    let j = u * (n / (2.0 * h));
    push(a, k, at + d * h, j);
    push(a, k, at - d * h, -j);
}

/// Up on its feet: the head well over them, the pelvis at a standing height (its own size).
pub fn up(a: &Agent, scale: f32) -> bool {
    let fr: Vec<Transform> = a.doll.frames().collect();
    let feet = FEET.iter().filter_map(|&k| fr.get(k)).map(|f| f.translation.z).fold(f32::MAX, f32::min);
    fr.len() > HEAD && fr[HEAD].translation.z - feet > 1.15 * scale && fr[0].translation.z - feet > 0.7 * scale
}

pub struct GetUp {
    mode: Mode,
    net_path: String,
    help: f32,
    speed: f32,
    blend: f32,
    started: bool,
    done: bool,
    clips: Option<ClipSet>,
    rise: Option<Rise>,
    steady: Option<(f32, Transform)>,
    best: f32,
    net: Option<Arc<Net>>,
    /// A coached network: the muscles' get-up, its joints corrected by it.
    coach: Option<Arc<Net>>,
    zero: Vec<Quat>,
    floor: f32,
    scale: f32,
    steps: usize,
    acc: f32,
    act: Vec<f32>,
    targets: Vec<Transform>,
    rand: delirium_data::rng::Rand,
    /// How it went: None under way, Some(true) up, Some(false) it didn't make it.
    pub result: Option<bool>,
}

fn mode_of(p: &[(String, Val)], or: Mode) -> Mode {
    match p.iter().find(|(k, _)| k == "mode").map(|(_, v)| v) {
        Some(Val::Str(s)) => match s.to_ascii_lowercase().as_str() {
            "muscles" | "muscle" => Mode::Muscles,
            "learned" | "learnt" => Mode::Learned,
            _ => Mode::Animation,
        },
        Some(Val::Num(n)) => match *n as i32 {
            1 => Mode::Muscles,
            2 => Mode::Learned,
            _ => Mode::Animation,
        },
        _ => or,
    }
}

impl GetUp {
    pub fn make(p: &[(String, Val)]) -> Box<dyn Custom> {
        let net_path = p.iter().find(|(k, _)| k == "net").and_then(|(_, v)| if let Val::Str(s) = v { Some(s.clone()) } else { None }).unwrap_or_default();
        Box::new(GetUp {
            mode: mode_of(p, Mode::Animation),
            net_path,
            help: num(p, "help", 0.5).clamp(0.0, 2.0),
            speed: num(p, "speed", if mode_of(p, Mode::Animation) == Mode::Muscles { MUSCLE_SPEED } else { 1.0 }).clamp(0.2, 2.0),
            blend: num(p, "blend", delirium_data::kv!(17)).clamp(0.05, 2.0),
            started: false,
            done: false,
            clips: CLIPS.with(|c| c.borrow().clone()),
            rise: None,
            steady: None,
            best: 0.0,
            net: None,
            coach: None,
            zero: vec![],
            floor: 0.0,
            scale: 1.0,
            steps: 0,
            acc: 0.0,
            act: vec![],
            targets: vec![],
            rand: delirium_data::rng::Rand { seed: num(p, "seed", 7.0) as u32 | 1 },
            result: None,
        })
    }

    /// A learned get-up by a coached network given here (a trainer's candidate), with the clips this thread uses.
    pub fn coached(net: Arc<Net>, seed: u32) -> GetUp {
        GetUp {
            mode: Mode::Learned,
            net_path: String::new(),
            help: net.help,
            speed: net.speed,
            blend: delirium_data::kv!(17),
            started: false,
            done: false,
            clips: CLIPS.with(|c| c.borrow().clone()),
            rise: None,
            steady: None,
            best: 0.0,
            net: None,
            coach: Some(net),
            zero: vec![],
            floor: 0.0,
            scale: 1.0,
            steps: 0,
            acc: 0.0,
            act: vec![],
            targets: vec![],
            rand: delirium_data::rng::Rand { seed: seed | 1 },
            result: None,
        }
    }

    /// The muscles do it, the clip's poses their targets (the muscles' get-up, or a coached network's).
    fn muscular(&self) -> bool {
        self.mode == Mode::Muscles || self.coach.is_some()
    }

    /// The animation's parts now (animation only; empty otherwise or once over).
    pub fn targets(&self) -> &[Transform] {
        &self.targets
    }

    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            Mode::Animation => "animation",
            Mode::Muscles => "muscles",
            Mode::Learned => "learned",
        }
    }

    /// Up on its feet and steadying (the muscles' or a coached get-up, once it's up: balancing, still helped a moment).
    pub fn standing_up(&self) -> bool {
        self.steady.is_some()
    }

    /// How far it is (0-1).
    pub fn progress(&self) -> f32 {
        match (self.mode, &self.rise, &self.clips) {
            (Mode::Learned, _, _) if self.coach.is_none() => self.steps as f32 / NET_STEPS as f32,
            (_, Some(r), Some(c)) => r.t / c.clips.duration(r).max(1e-3),
            _ => 0.0,
        }
    }

    fn start(&mut self, a: &mut Agent, b: &Build, w: &Collision) {
        self.started = true;
        let lay: Vec<Transform> = a.doll.frames().collect();
        let homer_leg = self.clips.as_ref().map_or(0.0, |c| c.leg);
        let leg = crate::game::leg_length(b);
        self.scale = if homer_leg > 0.05 && leg > 0.05 { (leg / homer_leg).clamp(0.5, 2.0) } else { 1.0 };
        // the floor under it: the world's, or its lowest part's underside
        let lowest = lay.iter().zip(&b.parts).map(|(f, p)| f.translation.z - p.radius).fold(f32::MAX, f32::min);
        let pelvis = lay[0].translation;
        let below = w.segment(pelvis + Vec3::Z * 0.3, pelvis - Vec3::Z * 1.5).map(|(h, _)| h.z);
        self.floor = below.filter(|z| (z - lowest).abs() < 0.35).unwrap_or(lowest);
        message(a, b, w, "stopAllBehaviours", &[]);
        if self.mode == Mode::Learned && self.coach.is_none() {
            let found = net(&self.net_path);
            match found {
                Some(n) if n.coached(b.parts.len()) => {
                    (self.help, self.speed) = (n.help, n.speed);
                    self.coach = Some(n);
                }
                f => self.net = f,
            }
        }
        match self.mode {
            _ if self.coach.is_some() || self.mode != Mode::Learned => {
                let Some(set) = self.clips.clone() else {
                    self.finish(a, b, w, false);
                    return;
                };
                // (its feet put down on the floor by the end, its own size)
                self.rise = set.clips.rise(lay, self.scale, Some(self.floor + set.feet * self.scale));
                if self.rise.is_none() {
                    self.finish(a, b, w, false);
                    return;
                }
                delirium::nm::task_pose(a, b, w, &mut self.rand, MUSCLE_STIFFNESS.0, MUSCLE_STIFFNESS.1);
            }
            _ => {
                self.net = self.net.take().filter(|n| n.outputs() > 3 * (b.parts.len() - 1));
                if self.net.is_none() {
                    self.finish(a, b, w, false);
                    return;
                }
                // (the network's joints are turned from the body standing)
                self.zero = b.parts.iter().map(|p| p.parent.map_or(p.bind, |j| b.parts[j].bind.inverse() * p.bind)).collect();
            }
        }
    }

    fn finish(&mut self, a: &mut Agent, b: &Build, w: &Collision, ok: bool) {
        self.done = true;
        self.result = Some(ok);
        self.targets.clear();
        if self.steady.is_none() {
            a.incoming.clear();
            a.muscle_stiffness_x(1.0);
            if ok {
                Self::balance(a, b, w);
            }
        }
    }

    fn balance(a: &mut Agent, b: &Build, w: &Collision) {
        a.incoming.clear();
        a.muscle_stiffness_x(1.0);
        // (on its feet again: no longer the body that fell - the engine drops any balance a fallen body starts)
        a.fallen = false;
        message(a, b, w, "stopAllBehaviours", &[]);
        message(a, b, w, "bodyBalance", &[("start", Value::Bool(true))]);
    }

    /// The muscles' help: the pelvis towards `to` (its place and way), at most `help` of the body's weight and a limited
    /// turn.
    fn help_pelvis(&self, a: &mut Agent, to: Transform, k: f32, dt: f32) {
        let help = self.help * k.clamp(0.0, 1.0);
        let now = a.doll.frames().next().unwrap_or_default();
        let (at, v, w) = (now.translation, a.doll.com().1, a.doll.motion(0).1);
        let m = a.doll.mass();
        let f = (m * (40.0 * (to.translation - at) - 8.0 * v)).clamp_length_max(help * m * 9.81);
        push(a, 0, at, f * dt);
        let mut turn = (to.rotation * now.rotation.inverse()).normalize();
        if turn.w < 0.0 {
            turn = -turn;
        }
        let inertia = m * TRUNK_REACH * TRUNK_REACH;
        let tau = (inertia * (30.0 * turn.to_scaled_axis() - 7.0 * w)).clamp_length_max(help * m * 9.81 * TRUNK_REACH);
        push_torque(a, 0, at, tau * dt);
    }
}

impl Custom for GetUp {
    fn name(&self) -> &'static str {
        "dlGetUp"
    }

    fn set(&mut self, p: &[(String, Val)]) {
        self.help = num(p, "help", self.help).clamp(0.0, 2.0);
        if flag(p, "abort", false) {
            self.done = true;
            self.result = Some(false);
        }
    }

    fn before(&mut self, a: &mut Agent, b: &Build, w: &Collision, dt: f32) {
        // up, balancing: the help a moment longer, less and less, towards standing still where it is (over once it's
        // steady)
        if let Some((left, to)) = self.steady {
            let here = a.doll.frames().next().unwrap_or_default().translation;
            self.help_pelvis(a, to.with_translation(Vec3::new(here.x, here.y, to.translation.z)), left / STEADY, dt);
            let calm = STEADY - left > 0.8 && a.doll.com().1.length() < 0.25;
            self.steady = (left > dt && !calm).then_some((left - dt, to));
            if self.steady.is_none() {
                self.done = true;
            }
            return;
        }
        if self.done {
            return;
        }
        if !self.started {
            self.start(a, b, w);
            if self.done {
                return;
            }
        }
        match self.mode {
            _ if self.coach.is_some() || self.mode != Mode::Learned => {
                let Some(clips) = self.clips.as_ref().map(|c| c.clips.clone()) else { return };
                let muscular = self.muscular();
                let frames = match self.rise.as_mut() {
                    Some(r) => {
                        r.t += dt * self.speed;
                        let f = clips.rise_frames(r, self.blend);
                        // (the muscles' get-up, the clip over but not quite up: it holds the end a moment longer)
                        let late = r.t - clips.duration(r);
                        match f {
                            None if muscular && late < MUSCLE_HOLD && !up(a, self.scale) => Some(clips.end_frames(r)),
                            f => f,
                        }
                    }
                    None => return,
                };
                match frames {
                    Some(p) => {
                        if self.muscular() {
                            // (the muscles work the joints; the body as a whole - up off the floor, the trunk turned up -
                            // is helped at the pelvis: a hard enough knock still has it down)
                            self.help_pelvis(a, p[0], 1.0, dt);
                        }
                        self.targets.clone_from(&p);
                        match self.coach.clone() {
                            Some(net) => {
                                // (the network steps at 60 a second, as it learned: its muscles held between)
                                self.acc += dt;
                                while self.acc >= NET_DT || self.act.is_empty() {
                                    self.acc = (self.acc - NET_DT).max(0.0);
                                    let (phase, back) = self.rise.as_ref().map_or((1.0, false), |r| (r.t / clips.duration(r).max(1e-3), r.clip == 1));
                                    self.act = net.forward(&coach_obs(a, self.floor, phase, back));
                                    self.steps += 1;
                                }
                                let act = self.act.clone();
                                coach_drive(a, &act, &p, b);
                            }
                            None => delirium::nm::task_pose_control(a, p),
                        }
                    }
                    None => {
                        // over: the animation always ends standing; the muscles' might not
                        let ok = self.mode == Mode::Animation || up(a, self.scale);
                        if ok && self.muscular() {
                            if let Some(r) = self.rise.as_ref() {
                                self.steady = Some((STEADY, clips.end_frames(r)[0]));
                                Self::balance(a, b, w);
                            }
                        }
                        self.finish(a, b, w, ok);
                        // (steadying: carries on a moment)
                        self.done = self.steady.is_none();
                    }
                }
            }
            _ => {
                let Some(net) = self.net.clone() else { return };
                // (the network steps at 60 a second, as it learned: its muscles held between)
                self.acc += dt;
                while self.acc >= NET_DT && self.steps < NET_STEPS {
                    self.acc -= NET_DT;
                    self.act = net.forward(&observe(a, self.floor));
                    self.steps += 1;
                }
                if !self.act.is_empty() {
                    let act = self.act.clone();
                    drive(a, &act, &self.zero, b);
                }
                let head = a.doll.frames().nth(HEAD).map_or(0.0, |f| f.translation.z) - self.floor;
                self.best = self.best.max(head);
                // (up and steady; or out of time - or getting nowhere, its head never off the floor after 3 s)
                let nowhere = self.steps > 180 && self.best < 0.7 * self.scale;
                if self.steps >= NET_STEPS || nowhere || (self.steps > 120 && up(a, self.scale) && a.doll.com().1.length() < 0.6) {
                    let ok = up(a, self.scale);
                    self.finish(a, b, w, ok);
                }
            }
        }
    }

    fn after(&mut self, _a: &mut Agent, _b: &Build, _w: &Collision, _dt: f32) -> bool {
        !self.done
    }

    fn stop(&mut self, a: &mut Agent, _b: &Build, _w: &Collision) {
        if !self.done {
            a.incoming.clear();
            a.muscle_stiffness_x(1.0);
        }
    }

    fn as_get_up(&self) -> Option<&GetUp> {
        Some(self)
    }
}
