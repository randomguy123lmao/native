//! Checks the host conventions (delirium::host): each joint's angles, as its muscle measures them, rebuild the part's
//! actual rotation in its parent; and shows the muscles a balancing Homer hands to a host engine.
//! usage: hostcheck [GTA IV folder]

use delirium::collision::Collision;
use delirium::host;
use delirium::msg::{self, Value};
use delirium::nm::Agent;
use delirium::prelude::*;
use delirium::ragdoll::Ragdoll;
use gmsv_delirium::game::Game;
use std::path::Path;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(r"J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV".into());
    let g = Game::open(Path::new(&dir)).expect("open");
    g.load_character().expect("character");
    let homer = g.homer().expect("Homer");
    let b = &homer.build;
    let mut world = Collision::default();
    world.add_floor(0.0, 0.0, 0.0, 50.0, 0);
    let mut agent = Agent::new(b, Ragdoll::new(b, &homer.frames, Vec3::ZERO), Quat::IDENTITY, 0.3);
    let mut st = std::mem::take(&mut agent.msgs);
    msg::send(&mut agent, b, &world, &mut st, "bodyBalance", &[("start", Value::Bool(true))]).expect("bodyBalance");
    agent.msgs = st;
    for _ in 0..45 {
        agent.step(b, &world, 1.0 / 60.0, &[], &[]);
    }
    // a shove, so the joints are off their rest
    let chest = agent.doll.frames().nth(10).unwrap_or_default().translation;
    agent.doll.push(b, chest, Vec3::new(25.0, 10.0, 0.0));
    for _ in 0..12 {
        agent.step(b, &world, 1.0 / 60.0, &[], &[]);
    }
    let kept = agent.step_host(b, &world, 1.0 / 60.0, &[]).expect("a drive");
    let frames: Vec<Transform> = agent.doll.frames().collect();
    let mut worst = 0.0f32;
    println!("part  angles now (lean lean twist)     rebuilt vs actual   muscle: off target  stiffness  damping");
    for k in 0..b.parts.len() {
        let Some(parent) = b.parts[k].parent else { continue };
        let r = kept.rigid.get(k);
        let Some(at) = agent.doll.joint_angles(b, k, r) else { continue };
        let rebuilt = host::joint_rotation(b, k, r, at).unwrap();
        let actual = (frames[parent].rotation.inverse() * frames[k].rotation).normalize();
        let err = rebuilt.angle_between(actual);
        worst = worst.max(err);
        let m = host::muscles(b, &kept)[k];
        let (off, s, d) = m.map_or((f32::NAN, 0.0, 0.0), |m| (m.target.angle_between(actual), m.stiffness, m.damping));
        println!("{k:4}  {:6.3} {:6.3} {:6.3}          {:7.4} rad         {:6.3} rad  {:6.2}     {:5.2}", at[0], at[1], at[2], err, off, s, d);
    }
    println!("worst rebuilt error: {worst:.5} rad ({})", if worst < 0.01 { "conventions agree" } else { "CONVENTIONS DISAGREE" });
}
