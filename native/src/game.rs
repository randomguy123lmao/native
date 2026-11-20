//! The engine's numbers and Homer's body, read from the player's own GTA IV (the PC game, the folder with GTAIV.exe),
//! the way the Delirium Playroom reads them (crates/delirium-room/src/getup.rs and main.rs). Nothing of the game is
//! stored or shipped: every server running this needs its own copy.

use delirium::nmrs::HOMER;
use delirium::prelude::*;
use delirium::ragdoll::Build;
use delirium_data::{anim, archive_key::ArchiveKey, img, model, resource, rpf, skeleton};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;

/// The Playroom's Homer: the ped model whose ragdoll the body is made from.
pub const HOMER_MODEL: &str = "m_y_pmanhat_01";

/// The console paths the engine's readers use, as the PC game names them.
fn pc(s: &str) -> String {
    [("xbox360", "pc"), (".xrt", ".art"), (".xft", ".wft"), (".xad", ".wad"), (".xtd", ".wtd"), (".xdr", ".wdr")]
        .iter()
        .fold(s.to_string(), |s, (a, b)| s.replace(a, b))
}

pub struct Game {
    dir: PathBuf,
    key: ArchiveKey,
}

/// Where GTA IV is installed, looked for in Steam's libraries: the one Garry's Mod runs from first, then every library
/// Steam lists (its libraryfolders.vdf). The folder with GTAIV.exe, or None.
pub fn find() -> Option<PathBuf> {
    find_in_libraries()
}

/// The GTA IV folder for a tool run outside Garry's Mod: the one given (a command-line argument), else the
/// DELIRIUM_GTA environment variable, else the one found in Steam's libraries (see find). Exits with a message if none.
pub fn dir_or_find(given: Option<String>) -> String {
    given
        .or_else(|| std::env::var("DELIRIUM_GTA").ok())
        .or_else(|| find().map(|p| p.display().to_string()))
        .unwrap_or_else(|| {
            eprintln!("GTA IV not found: pass the folder with GTAIV.exe, or set DELIRIUM_GTA to it");
            std::process::exit(2)
        })
}

fn find_in_libraries() -> Option<PathBuf> {
    fn add(libs: &mut Vec<PathBuf>, p: PathBuf) {
        if !libs.iter().any(|l| l == &p) {
            libs.push(p);
        }
    }
    let mut libs: Vec<PathBuf> = vec![];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(steamapps) = exe.ancestors().find(|a| a.file_name().is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))) {
            if let Some(lib) = steamapps.parent() {
                add(&mut libs, lib.to_path_buf());
            }
        }
    }
    let steam_dirs = [std::env::var_os("ProgramFiles(x86)").map(|p| PathBuf::from(p).join("Steam")), Some(PathBuf::from(r"C:\Program Files (x86)\Steam"))];
    let mut vdfs: Vec<PathBuf> = steam_dirs.into_iter().flatten().map(|s| s.join("steamapps").join("libraryfolders.vdf")).collect();
    vdfs.extend(libs.iter().map(|l| l.join("steamapps").join("libraryfolders.vdf")));
    for vdf in vdfs {
        let Ok(t) = std::fs::read_to_string(&vdf) else { continue };
        for line in t.lines() {
            let mut q = line.split('"').filter(|s| !s.trim().is_empty());
            if let (Some(k), Some(v)) = (q.next(), q.next()) {
                if k.trim().eq_ignore_ascii_case("path") {
                    add(&mut libs, PathBuf::from(v.replace("\\\\", "\\")));
                }
            }
        }
    }
    libs.into_iter()
        .map(|l| l.join("steamapps").join("common").join("Grand Theft Auto IV").join("GTAIV"))
        .find(|d| d.join("GTAIV.exe").is_file())
}

/// Homer's body at rest: standing in the game's idle pose, feet on z = 0, the pelvis over the origin, facing +X.
#[derive(Clone)]
pub struct Homer {
    pub build: Build,
    pub frames: Vec<Transform>,
}

/// A material row of the game's materials.dat: its name, friction and elasticity.
pub struct Surface {
    pub name: String,
    pub friction: f32,
    pub elasticity: f32,
}

impl Game {
    /// Opens the game in `dir` (the folder with GTAIV.exe, or GTAIV.exe itself): unlocks the engine's numbers from its
    /// program and finds the archives' key in it.
    pub fn open(dir: &Path) -> Result<Game, String> {
        let dir = if dir.is_file() { dir.parent().unwrap_or(dir) } else { dir };
        let exe = dir.join("GTAIV.exe");
        if !exe.is_file() {
            return Err(format!("no GTAIV.exe in {} (Delirium needs your own PC GTA IV)", dir.display()));
        }
        let prog = delirium_data::xex::program(&exe)?;
        if !delirium_data::kv::open() && !delirium_data::kv::unlock(&prog) {
            return Err("this GTAIV.exe isn't a version Delirium reads its numbers from (it needs the Steam GTA IV's)".into());
        }
        // the PC game's files are little-endian
        delirium_data::LE.store(true, Relaxed);
        let mut head = vec![0; 0x810];
        File::open(dir.join("pc/models/cdimages/playerped.rpf"))
            .and_then(|mut f| f.read_exact(&mut head))
            .map_err(|e| format!("playerped.rpf: {e}"))?;
        let (key, _) = ArchiveKey::find_by_trial(&prog, &head).ok_or("no archive key in GTAIV.exe")?;
        Ok(Game { dir: dir.to_path_buf(), key })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// NM's character (fred: its joints, masses, effector limits, self-collision), once for every agent.
    pub fn load_character(&self) -> Result<(), String> {
        if delirium::nmrs::ASSET[HOMER].get().is_some() {
            return Ok(());
        }
        let p = self.dir.join(pc("pc/data/naturalmotion/fred.xrt"));
        let d = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        delirium::nmrs::load(HOMER, &d);
        Ok(())
    }

    /// The game's surfaces (friction and elasticity per material), for every agent; their rows by name.
    pub fn load_surfaces(&self) -> Vec<Surface> {
        let Ok(t) = std::fs::read_to_string(self.dir.join("common/data/materials/materials.dat")) else { return vec![] };
        let f = delirium_data::vehicles::read_friction(&t);
        *delirium::ragdoll::SURFACES.write().unwrap_or_else(|e| e.into_inner()) = f.clone();
        t.lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic()))
            .zip(f)
            .map(|(l, [friction, elasticity])| Surface { name: l.split_whitespace().next().unwrap_or("").to_string(), friction, elasticity })
            .collect()
    }

    /// The tasks' tuning (taskparams.txt: the shot reaction's sets, on fire, falls...): loose, or in the PC game's
    /// pc/data/game.rpf.
    pub fn task_params(&self) -> Option<delirium_data::taskparams::TaskParams> {
        if let Ok(t) = std::fs::read_to_string(self.dir.join("common/data/taskparams.txt")) {
            return Some(delirium_data::taskparams::read(&t));
        }
        for archive in ["pc/data/game.rpf", "common.rpf"] {
            if let Ok(t) = self.archive_text(archive, "taskparams.txt") {
                if t.contains('{') {
                    return Some(delirium_data::taskparams::read(&t));
                }
            }
        }
        None
    }

    /// A text file in one of the game's archives (the first whose path ends with `name`): as stored, inflated,
    /// or decrypted with the archives' key first - whichever reads as text.
    pub fn archive_text(&self, archive: &str, name: &str) -> Result<String, String> {
        let bytes = std::fs::read(self.dir.join(archive)).map_err(|e| format!("{archive}: {e}"))?;
        let toc = rpf::read_toc(&bytes, &self.key)?;
        let e = toc.iter().find(|e| e.path.to_ascii_lowercase().ends_with(name)).ok_or_else(|| format!("no {name} in {archive}"))?;
        let stored = bytes.get(e.offset as usize..e.offset as usize + e.stored as usize).ok_or("entry out of range")?.to_vec();
        let texty = |d: &[u8]| {
            !d.is_empty() && d.iter().take(512).filter(|&&b| matches!(b, 9 | 10 | 13) || (32..127).contains(&b)).count() * 10 >= d.len().min(512) * 9
        };
        let mut tries: Vec<Vec<u8>> = vec![];
        if let Ok(d) = rpf::read(&bytes, e) {
            tries.push(d);
        }
        tries.push(stored.clone());
        let mut dec = stored.clone();
        self.key.decrypt(&mut dec);
        tries.push(dec.clone());
        for d in [stored, dec] {
            if let Ok(x) = miniz_oxide::inflate::decompress_to_vec_zlib(&d) {
                tries.push(x);
            }
            if let Ok(x) = miniz_oxide::inflate::decompress_to_vec(&d) {
                tries.push(x);
            }
        }
        tries.into_iter().find(|d| texty(d)).map(|d| String::from_utf8_lossy(&d).into_owned()).ok_or_else(|| format!("{name} in {archive}: not readable"))
    }

    /// Every file in one of the game's archives (for finding things in it).
    pub fn archive_files(&self, archive: &str) -> Result<Vec<String>, String> {
        let bytes = std::fs::read(self.dir.join(archive)).map_err(|e| format!("{archive}: {e}"))?;
        Ok(rpf::read_toc(&bytes, &self.key)?.into_iter().map(|e| e.path).collect())
    }

    /// The game's weapons (weaponinfo.xml).
    pub fn weapons(&self) -> Vec<delirium_data::weapons::Weapon> {
        std::fs::read_to_string(self.dir.join("common/data/WeaponInfo.xml")).map(|t| delirium_data::weapons::read(&t)).unwrap_or_default()
    }

    fn img(&self, path: &str) -> Result<(Vec<img::Entry>, File), String> {
        let path = self.dir.join(pc(path));
        let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        let mut f = File::open(&path).map_err(|e| format!("{name}: {e}"))?;
        let mut head = vec![];
        (&mut f)
            .take(delirium_data::kv!(1 int) << delirium_data::kv!(127 int))
            .read_to_end(&mut head)
            .map_err(|e| e.to_string())?;
        Ok((img::read_table(&head, Some(&self.key))?, f))
    }

    /// Homer's body, from the game's ped model and its idle pose (load_character first).
    /// GTA IV's clips for a body built as `build` (Homer's, or one refitted from it: the parts are the same).
    pub fn clips(&self, build: &Build) -> Result<Clips, String> {
        Clips::load(self, build)
    }

    pub fn homer(&self) -> Result<Homer, String> {
        let mut build = self.homer_build(delirium::nmdbm::asset_links(HOMER))?;
        let clips = Clips::load(self, &build)?;
        if build.links.is_empty() {
            let idle: Vec<Quat> = clips.place(&clips.idle_pose(), Self::facing()).iter().map(|f| f.rotation).collect();
            if let Some(links) = delirium::nmrs::derive_links(&build, &idle) {
                build = self.homer_build(links)?;
            }
        }
        let clips = Clips::load(self, &build)?;
        let mut frames = clips.stand_up(&mut build);
        // feet on the floor, the pelvis over the origin
        let floor = build.parts.iter().zip(&frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
        let shift = -frames[0].translation.with_z(floor);
        for f in &mut frames {
            f.translation += shift;
        }
        Ok(Homer { build, frames })
    }

    /// The Playroom stands Homer up turned a quarter left: facing +X.
    fn facing() -> Transform {
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
    }

    fn homer_build(&self, links: Vec<Quat>) -> Result<Build, String> {
        let (table, mut f) = self.img("xbox360/models/cdimages/componentpeds.img")?;
        let frag = img_read(&table, &mut f, &format!("{HOMER_MODEL}.xft"))?;
        let bones = skeleton::read(&frag, model::fragment_root(&frag)?)?;
        let parts = delirium_data::ragdoll::read(&frag)?;
        let parents: Vec<Option<usize>> = bones.iter().map(|b| b.parent).collect();
        let bind: Vec<(Vec3, Quat)> = bones.iter().map(|b| (Vec3::from(b.translation), Quat::from_array(b.rotation).normalize())).collect();
        delirium::ragdoll::Rig::linked(&parts, &parents, &bind, &bones, HOMER, links)
            .map(|r| r.build)
            .ok_or(format!("{HOMER_MODEL}: no ragdoll"))
    }
}

fn img_read(table: &[img::Entry], f: &mut File, name: &str) -> Result<resource::Resource, String> {
    let name = &pc(name);
    let e = table.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or(format!("{name} not found"))?;
    let mut raw = vec![];
    f.seek(SeekFrom::Start(e.offset)).map_err(|e| e.to_string())?;
    f.take(e.used_blocks as u64 * delirium_data::kv!(708 int)).read_to_end(&mut raw).map_err(|e| e.to_string())?;
    resource::parse(&raw)
}

/// GTA IV's animation clips the body uses, on the game's player skeleton: the idle (standing it up) and the get-ups
/// (get_up_fast, from lying on the front and on the back).
pub struct Clips {
    parents: Vec<Option<usize>>,
    bind: Vec<(Vec3, Quat)>,
    idle: (anim::Animation, Vec<Option<usize>>),
    getup: Vec<(anim::Animation, Vec<Option<usize>>)>,
    parts: Vec<(usize, Transform)>,
    chest: usize,
}

/// A get-up under way: the clip, where it plays (the body's place and heading), the pose it began from, how far in.
#[derive(Clone)]
pub struct Rise {
    pub clip: usize,
    pub body: Transform,
    pub lay: Vec<Transform>,
    pub t: f32,
    /// The body's size to Homer's (its legs'): the clip's movement scaled to it.
    pub scale: f32,
    /// How far the clip is raised by its end (its feet on the floor), a little more each frame.
    pub lift: f32,
}

impl Clips {
    fn load(g: &Game, build: &Build) -> Result<Clips, String> {
        let peds = std::fs::read(g.dir.join("pc/models/cdimages/playerped.rpf")).map_err(|e| format!("playerped.rpf: {e}"))?;
        let inner = rpf::read_toc(&peds, &g.key)?;
        let e = inner.iter().find(|e| e.path.eq_ignore_ascii_case(&pc("player.xft"))).ok_or("player.wft not in playerped.rpf")?;
        let frag = resource::parse(&rpf::read(&peds, e)?)?;
        let bones = skeleton::read(&frag, model::fragment_root(&frag)?)?;
        let (table, mut f) = g.img("xbox360/anim/anim.img")?;
        let mut clip = |dict: &str, name: &str| -> Result<(anim::Animation, Vec<Option<usize>>), String> {
            let a = anim::read_dictionary(&img_read(&table, &mut f, dict)?)?
                .into_iter()
                .find(|a| a.name.eq_ignore_ascii_case(name))
                .ok_or(format!("no {name} in {dict}"))?;
            let map = a.tracks.iter().map(|t| bones.iter().position(|b| b.id == t.bone).filter(|&b| b != 0)).collect();
            Ok((a, map))
        };
        let idle = clip("move_player.xad", "idle")?;
        // (the get-ups are optional: without them, nothing gets up but by other means)
        let getup = [clip("get_up.xad", "get_up_fast"), clip("get_up_back.xad", "get_up_fast")].into_iter().filter_map(Result::ok).collect();
        let parts = build
            .parts
            .iter()
            .map(|p| bones.iter().position(|b| b.name == p.name).map(|b| (b, p.offset)).ok_or(format!("no bone {}", p.name)))
            .collect::<Result<_, _>>()?;
        let chest = build.parts.iter().position(|d| d.parent == Some(0) && !d.lower).unwrap_or(0);
        Ok(Clips {
            parents: bones.iter().map(|b| b.parent).collect(),
            bind: bones.iter().map(|b| (Vec3::from(b.translation), Quat::from_array(b.rotation).normalize())).collect(),
            idle,
            getup,
            parts,
            chest,
        })
    }

    fn pose_of(&self, clip: &(anim::Animation, Vec<Option<usize>>), phase: f32) -> Vec<(Vec3, Quat)> {
        let (a, map) = clip;
        let mut bind = self.bind.clone();
        bind[0] = (Vec3::ZERO, Quat::IDENTITY);
        let mut out = delirium::ragdoll::sample(&bind, a, map, phase);
        out[0] = delirium::ragdoll::root(a, phase);
        out
    }

    fn idle_pose(&self) -> Vec<(Vec3, Quat)> {
        self.pose_of(&self.idle, 0.0)
    }

    fn place(&self, pose: &[(Vec3, Quat)], body: Transform) -> Vec<Transform> {
        let mut bones: Vec<(Vec3, Quat)> = Vec::with_capacity(pose.len());
        for (i, &(t, r)) in pose.iter().enumerate() {
            let (pt, pr) = self.parents[i].map_or((body.translation, body.rotation), |p| bones[p]);
            bones.push((pt + pr * t, pr * r));
        }
        self.parts
            .iter()
            .map(|&(b, o)| {
                let (bt, br) = bones[b];
                Transform::from_translation(bt + br * o.translation).with_rotation(br * o.rotation)
            })
            .collect()
    }

    fn stand_up(&self, build: &mut Build) -> Vec<Transform> {
        let mut frames = self.place(&self.idle_pose(), Game::facing());
        for (p, f) in build.parts.iter_mut().zip(&frames) {
            p.bind = f.rotation;
        }
        delirium::nmrs::asset_layout(build, &mut frames);
        frames
    }

    /// The standing pose (the idle's first frame), facing +X, the pelvis over the origin.
    pub fn standing(&self) -> Vec<Transform> {
        self.place(&self.idle_pose(), Game::facing())
    }

    pub fn has_getup(&self) -> bool {
        !self.getup.is_empty()
    }

    /// A get-up for a body lying as `lay` (its parts), `scale` times Homer's size: the clip that starts nearest how it
    /// lies (on its front, on its back), turned to its heading, from where its pelvis is; ending with its feet's centres
    /// at height `feet` (metres) when that's known.
    pub fn rise(&self, lay: Vec<Transform>, scale: f32, feet: Option<f32>) -> Option<Rise> {
        let yaw = |q: Quat| {
            let a = q * Vec3::Y;
            a.y.atan2(a.x)
        };
        let ch = self.chest;
        let scale = if scale.is_finite() { scale.clamp(0.5, 2.0) } else { 1.0 };
        let (_, clip, body) = (0..self.getup.len())
            .map(|c| {
                let first = self.place(&self.pose_of(&self.getup[c], 0.0), Transform::IDENTITY);
                let turn = Quat::from_rotation_z(yaw(lay[ch].rotation) - yaw(first[ch].rotation));
                let body = Transform::from_translation(lay[0].translation - turn * first[0].translation * scale).with_rotation(turn);
                ((turn * first[ch].rotation).angle_between(lay[ch].rotation), c, body)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        let mut r = Rise { clip, body, lay, t: 0.0, scale, lift: 0.0 };
        // (begun where the pelvis lies, the clip's end can stand off the floor or in it: raised or lowered by then)
        if let Some(f) = feet {
            let end = self.clip_frames(&r, 1.0);
            if end.len() > 6 {
                let lift = f - end[3].translation.z.min(end[6].translation.z);
                if lift.abs() < 0.6 * scale {
                    r.lift = lift;
                }
            }
        }
        Some(r)
    }

    pub fn duration(&self, r: &Rise) -> f32 {
        self.getup[r.clip].0.duration
    }

    fn clip_frames(&self, r: &Rise, phase: f32) -> Vec<Transform> {
        self.place(&self.pose_of(&self.getup[r.clip], phase), Transform::from_rotation(r.body.rotation))
            .into_iter()
            .map(|f| f.with_translation(r.body.translation + f.translation * r.scale + Vec3::Z * r.lift * phase * phase * (3.0 - 2.0 * phase)))
            .collect()
    }

    /// The parts now (`blend`: s to blend from how it lay into the clip), or None once the clip is over.
    pub fn rise_frames(&self, r: &Rise, blend: f32) -> Option<Vec<Transform>> {
        let d = self.duration(r);
        (r.t <= d).then(|| {
            let k = (r.t / blend.max(1e-3)).min(1.0);
            self.clip_frames(r, r.t / d.max(1e-3))
                .into_iter()
                .zip(&r.lay)
                .map(|(n, l)| Transform::from_translation(l.translation.lerp(n.translation, k)).with_rotation(l.rotation.slerp(n.rotation, k)))
                .collect()
        })
    }

    /// Where the get-up ends: standing.
    pub fn end_frames(&self, r: &Rise) -> Vec<Transform> {
        self.clip_frames(r, 1.0)
    }
}

/// A body's legs' length (thigh and shin, hip to ankle), for its size to Homer's.
pub fn leg_length(build: &Build) -> f32 {
    let child = |k: usize| build.parts.iter().position(|p| p.parent == Some(k));
    let mut len = 0.0;
    let mut k = 1;
    for _ in 0..2 {
        let Some(c) = child(k) else { break };
        len += (build.parts[c].anchor_parent - build.parts[k].anchor).length();
        k = c;
    }
    len
}

