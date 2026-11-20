//! Trains a coached get-up network (the "learned" way of dlGetUp) on the engine's own bodies.
//!
//! The network rides on the muscles' get-up: GTA IV's get_up_fast as the muscles' targets, helped at the pelvis. It
//! sees the body and how far along the clip it is, and turns each joint a little from the clip's (and sets how stiff the
//! muscles are). It starts out saying nothing (all zeros: the muscles' get-up itself, which gets up with the help); the
//! help is taken away as it learns to get up without it. Each try runs dlGetUp itself (getup.rs), as the game does.
//!
//! Evolution strategies (as Delirium's own trainer): pairs of nudges to the weights, tried on bodies knocked down
//! every which way (sizes and weights varied, strength too), ranked, and the weights moved the way the better ones
//! went (Adam).
//!
//! usage: getuptrain [generations] [out.net] [GTA IV folder]

use delirium::collision::Collision;
use delirium::nm::Agent;
use delirium::prelude::*;
use delirium::ragdoll::{Build, Ragdoll};
use gmsv_delirium::custom::Custom;
use gmsv_delirium::getup::{self, GetUp, Net};
use std::sync::Arc;

const DT: f32 = 1.0 / 60.0;
const EPISODE: usize = 450;
const PAIRS: usize = 48;
const TRIES: usize = 2;
const SIGMA: f32 = 0.02;
const LR: f32 = 0.01;
const HIDDEN: usize = 64;
const HEAD: usize = 12;
const SPEED: f32 = 0.7;

struct Rng(u64);
impl Rng {
    fn u(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn f(&mut self) -> f32 {
        (self.u() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn n(&mut self) -> f32 {
        let (a, b) = (self.f().max(1e-7), self.f());
        (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }
}

/// A body knocked down: its build (a size and weight of its own), how it lies, its size to Homer's.
struct Start {
    build: Build,
    lay: Vec<Transform>,
    scale: f32,
}

fn starts(homer: &gmsv_delirium::game::Homer, world: &Collision, n: usize, rng: &mut Rng) -> Vec<Start> {
    let homer_leg = gmsv_delirium::game::leg_length(&homer.build);
    let mut out = vec![];
    while out.len() < n {
        let size = 0.9 + 0.2 * rng.f();
        let heavy = 0.85 + 0.3 * rng.f();
        let (build, mut frames) = homer.build.scaled(&homer.frames, &vec![size; homer.build.parts.len()], heavy);
        let floor = build.parts.iter().zip(&frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
        let shift = -frames[0].translation.with_z(floor);
        for f in &mut frames {
            f.translation += shift;
        }
        let mut a = Agent::new(&build, Ragdoll::new(&build, &frames, Vec3::ZERO), Quat::IDENTITY, 0.3);
        let dir = Quat::from_rotation_z(rng.f() * std::f32::consts::TAU) * Vec3::X;
        a.doll.kick(dir * (1.5 + 2.5 * rng.f()));
        a.set_stiffness(2.0 + 3.0 * rng.f(), 1.0);
        for _ in 0..240 {
            a.step(&build, world, DT, &[], &[]);
        }
        let lay: Vec<Transform> = a.doll.frames().collect();
        if lay[HEAD].translation.z < 0.5 && lay.iter().all(|f| f.translation.is_finite()) {
            let scale = gmsv_delirium::game::leg_length(&build) / homer_leg;
            out.push(Start { build, lay, scale });
        }
    }
    out
}

/// One try: the get-up from how it lies, then what it does for the rest of the 7.5 s. Its score: how high its head and
/// pelvis were along the way (up sooner, and staying up, score more), and 1 more for standing still, balancing, at the
/// end. And whether it did.
fn episode(net: &Arc<Net>, s: &Start, world: &Collision, seed: u32, strength: f32) -> (f32, bool) {
    let b = &s.build;
    let mut a = Agent::new(b, Ragdoll::new(b, &s.lay, Vec3::ZERO), Quat::IDENTITY, 0.3);
    a.strength = strength;
    let mut g = GetUp::coached(net.clone(), seed);
    let mut going = true;
    let mut score = 0.0;
    for _ in 0..EPISODE {
        if going {
            g.before(&mut a, b, world, DT);
        }
        a.step(b, world, DT, &[], &[]);
        if going {
            going = g.after(&mut a, b, world, DT);
        }
        let fr: Vec<Transform> = a.doll.frames().collect();
        let (h, p) = (fr[HEAD].translation.z, fr[0].translation.z);
        if !(h.is_finite() && p.is_finite()) {
            return (-10.0, false);
        }
        score += 0.5 * (h / (1.6 * s.scale)).clamp(0.0, 1.0) + 0.5 * (p / (0.9 * s.scale)).clamp(0.0, 1.0);
    }
    let ok = getup::up(&a, s.scale) && a.balancing() && a.doll.com().1.length() < 0.6;
    (score / EPISODE as f32 + if ok { 1.0 } else { 0.0 }, ok)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let gens: usize = args.get(1).and_then(|g| g.parse().ok()).unwrap_or(200);
    // usage: getuptrain [generations] [output .net] [GTA IV folder]
    let out = args.get(2).cloned().unwrap_or("getup/coach.net".into());
    let dir = gmsv_delirium::game::dir_or_find(args.get(3).cloned());
    if let Some(d) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let sim = gmsv_delirium::sim::Sim::start(&dir).expect("start");
    let set = getup::clip_set().expect("GTA IV's get-up clips");
    let mut world = Collision::default();
    world.add_floor(0.0, 0.0, 0.0, 50.0, 0);
    let homer = &sim.homer;
    let parts = homer.build.parts.len();
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let t0 = std::time::Instant::now();
    let pool = starts(homer, &world, 64, &mut rng);
    let evals = starts(homer, &world, 16, &mut Rng(0xD1B54A32D192ED03));
    println!("getuptrain: {} starts and {} to judge by in {:.1} s", pool.len(), evals.len(), t0.elapsed().as_secs_f32());

    let sizes = [getup::observe_len(parts) + 3, HIDDEN, HIDDEN, 3 * (parts - 1) + 2];
    let mut net = match Net::load(&out).filter(|n| n.sizes == sizes) {
        Some(n) => {
            println!("getuptrain: carrying on from {out} (help {:.3})", n.help);
            n
        }
        None => {
            // (the hidden layers random, the last zero: it says nothing yet - the muscles' get-up itself)
            let mut w: Vec<f32> = (0..Net::len(sizes)).map(|_| rng.n() * 0.08).collect();
            let last = w.len() - (sizes[2] + 1) * sizes[3];
            w[last..].fill(0.0);
            Net::new(sizes, w, 0.5, SPEED)
        }
    };
    let np = net.w.len();
    let (mut m1, mut m2) = (vec![0.0f32; np], vec![0.0f32; np]);
    let threads = std::thread::available_parallelism().map_or(8, |n| n.get()).saturating_sub(2).max(1);
    println!("getuptrain: {np} weights, {threads} threads, {PAIRS} pairs x {TRIES} tries a generation, {:.1} s tries", EPISODE as f32 * DT);

    let judge = |net: &Net, evals: &[Start]| -> (f32, usize) {
        let net = Arc::new(Net::new(net.sizes, net.w.clone(), net.help, net.speed));
        getup::use_clip_set(Some(set.clone()));
        let r: Vec<(f32, bool)> = evals.iter().enumerate().map(|(i, s)| episode(&net, s, &world, 11 + i as u32, 1.0)).collect();
        (r.iter().map(|x| x.0).sum::<f32>() / r.len() as f32, r.iter().filter(|x| x.1).count())
    };
    let (score, up) = judge(&net, &evals);
    println!("start: judged {score:.3}, up {up}/{} with help {:.3}", evals.len(), net.help);

    for g in 0..gens {
        let tg = std::time::Instant::now();
        let seeds: Vec<u64> = (0..PAIRS).map(|_| rng.u() | 1).collect();
        let tries: Vec<Vec<(usize, f32)>> = (0..PAIRS).map(|_| (0..TRIES).map(|_| ((rng.u() % pool.len() as u64) as usize, 0.85 + 0.25 * rng.f())).collect()).collect();
        let noise = |s: u64| {
            let mut r = Rng(s);
            (0..np).map(|_| r.n()).collect::<Vec<f32>>()
        };
        let mut ret = vec![[0.0f32; 2]; PAIRS];
        let mut ups = vec![[0usize; 2]; PAIRS];
        std::thread::scope(|sc| {
            let chunk = PAIRS.div_ceil(threads);
            for (c, (out, ups)) in ret.chunks_mut(chunk).zip(ups.chunks_mut(chunk)).enumerate() {
                let (net, pool, seeds, tries, world, set) = (&net, &pool, &seeds, &tries, &world, &set);
                sc.spawn(move || {
                    getup::use_clip_set(Some(set.clone()));
                    for (i, (o, u)) in out.iter_mut().zip(ups.iter_mut()).enumerate() {
                        let k = c * chunk + i;
                        let e = noise(seeds[k]);
                        for (side, sign) in [1.0f32, -1.0].iter().enumerate() {
                            let w: Vec<f32> = net.w.iter().zip(&e).map(|(a, b)| a + sign * SIGMA * b).collect();
                            let cand = Arc::new(Net::new(net.sizes, w, net.help, net.speed));
                            let mut sum = 0.0;
                            for (n, &(t, strength)) in tries[k].iter().enumerate() {
                                let (sc, ok) = episode(&cand, &pool[t], world, (seeds[k] as u32).wrapping_add(n as u32), strength);
                                sum += sc;
                                u[side] += ok as usize;
                            }
                            o[side] = sum / TRIES as f32;
                        }
                    }
                });
            }
        });
        // ranked, the weights moved the way the better nudges went
        let mut all: Vec<(f32, usize)> = ret.iter().flatten().copied().zip(0..).collect();
        all.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut rank = vec![0.0f32; all.len()];
        for (r, &(_, i)) in all.iter().enumerate() {
            rank[i] = r as f32 / (all.len() - 1) as f32 - 0.5;
        }
        let mut grad = vec![0.0f32; np];
        for k in 0..PAIRS {
            let (d, e) = (rank[2 * k] - rank[2 * k + 1], noise(seeds[k]));
            for (gi, ei) in grad.iter_mut().zip(e) {
                *gi += d * ei / (2.0 * PAIRS as f32 * SIGMA);
            }
        }
        let t = (g + 1) as i32;
        for i in 0..np {
            let gi = grad[i] - 0.005 * net.w[i];
            m1[i] = 0.9 * m1[i] + 0.1 * gi;
            m2[i] = 0.999 * m2[i] + 0.001 * gi * gi;
            net.w[i] += LR * (m1[i] / (1.0 - 0.9f32.powi(t))) / ((m2[i] / (1.0 - 0.999f32.powi(t))).sqrt() + 1e-8);
        }
        // the help taken away as it gets up without needing it (given back a little when it can't)
        let tried = (2 * PAIRS * TRIES) as f32;
        let success = ups.iter().flatten().sum::<usize>() as f32 / tried;
        let before = net.help;
        if success >= 0.8 {
            net.help = (net.help - 0.02).max(0.0);
        } else if success < 0.5 {
            net.help = (net.help + 0.01).min(0.5);
        }
        let mean = ret.iter().flatten().sum::<f32>() / (2 * PAIRS) as f32;
        println!("generation {g:4}  mean {mean:.3}  up {:3.0}%  help {before:.3}  {:.1} s", success * 100.0, tg.elapsed().as_secs_f32());
        if g % 5 == 4 || g + 1 == gens {
            let (score, up) = judge(&net, &evals);
            println!("        judged {score:.3}, up {up}/{} (help {:.3}); saved {out}", evals.len(), net.help);
            if let Err(e) = net.save(&out) {
                println!("        couldn't save: {e}");
            }
        }
    }
}
