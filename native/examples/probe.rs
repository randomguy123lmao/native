//! Checks the engine runs on this machine's GTA IV: loads Homer, stands him on a floor, shoots him in the chest.
//! usage: probe [GTA IV folder]

use delirium::collision::Collision;
use delirium::msg::{self, Value};
use delirium::nm::{self, Agent};
use delirium::prelude::*;
use delirium::ragdoll::Ragdoll;
use gmsv_delirium::game::Game;
use std::path::Path;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(r"J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV".into());
    let t = std::time::Instant::now();
    let g = Game::open(Path::new(&dir)).unwrap_or_else(|e| fail("open", e));
    g.load_character().unwrap_or_else(|e| fail("character", e));
    let surfaces = g.load_surfaces();
    let tp = g.task_params();
    let weapons = g.weapons();
    let homer = g.homer().unwrap_or_else(|e| fail("Homer", e));
    limits();
    println!("loaded in {:.2} s: {} parts, {} surfaces, {} weapons, taskparams.txt {}", t.elapsed().as_secs_f32(), homer.build.parts.len(),
             surfaces.len(), weapons.len(), if tp.is_some() { "found" } else { "missing (defaults)" });
    for (i, (p, f)) in homer.build.parts.iter().zip(&homer.frames).enumerate() {
        println!("  {i:2} {:<22} parent {:>2} radius {:.3} half {:.3} at {:6.3} {:6.3} {:6.3}", p.name,
                 p.parent.map_or(-1, |x| x as i32), p.radius, p.half, f.translation.x, f.translation.y, f.translation.z);
    }

    let mut world = Collision::default();
    world.add_floor(0.0, 0.0, 0.0, 50.0, 0);
    let mut agent = Agent::new(&homer.build, Ragdoll::new(&homer.build, &homer.frames, Vec3::ZERO), Quat::IDENTITY, 0.3);
    let mut st = std::mem::take(&mut agent.msgs);
    msg::send(&mut agent, &homer.build, &world, &mut st, "bodyBalance", &[("start", Value::Bool(true)), ("bodyStiffness", Value::Float(10.0))])
        .unwrap_or_else(|e| fail("bodyBalance", format!("{e:?}")));
    agent.msgs = st;
    let step = |agent: &mut Agent| agent.step(&homer.build, &world, 1.0 / 60.0, &[], &[]);
    let t = std::time::Instant::now();
    for _ in 0..60 {
        step(&mut agent);
    }
    let pelvis = |a: &Agent| a.doll.frames().next().unwrap_or_default().translation;
    println!("stood 1 s ({:.3} ms a step): balancing {} fallen {} pelvis {:.2?}", t.elapsed().as_secs_f64() * 1e3 / 60.0, agent.balancing(),
             agent.fallen, pelvis(&agent));

    let chest = agent.doll.frames().nth(10).unwrap_or_default().translation;
    agent.doll.push(&homer.build, chest, Vec3::new(0.0, 40.0, 0.0));
    let hit = nm::Hit { health: 200.0, ..nm::Hit::ragdoll(&agent) };
    let mut rand = delirium_data::rng::Rand { seed: 1 };
    nm::task_shot(&mut agent, &homer.build, &world, &tp.unwrap_or_default(), &mut rand, "POWERFUL_WEAPON", hit, 10, chest, Vec3::Y * 400.0,
                  -Vec3::Y, false);
    for i in 0..180 {
        step(&mut agent);
        if i % 30 == 29 {
            let fb: Vec<&str> = agent.feedback.iter().map(|f| f.name).collect();
            println!("  {:.1} s after the shot: balancing {} fallen {} pelvis {:.2?} {fb:?}", (i + 1) as f32 / 60.0, agent.balancing(),
                     agent.fallen, pelvis(&agent));
        }
    }
}

fn fail(what: &str, e: impl std::fmt::Display) -> ! {
    eprintln!("{what}: {e}");
    std::process::exit(1)
}

#[allow(dead_code)]
pub fn limits() {
    println!("largest triangle across (kv 73): {} m", delirium_data::kv!(73));
}
