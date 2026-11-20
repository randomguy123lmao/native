//! gmsv_delirium: Delirium Physics (the GTA IV Euphoria port) inside Garry's Mod's server, as the Lua table
//! `delirium` (see api.rs). Positions are metres with Z up (Source's axes, x 0.0254), rotations quaternions
//! (x, y, z, w), parts the engine's 21 in its order (0 pelvis, 10 chest, 12 head, 16 and 20 hands, 3 and 6 feet).
//!
//! Every function returns its values, or nil and why not: none raises a Lua error.

pub mod custom;
pub mod game;
pub mod getup;
pub mod host;
pub mod lua;
pub mod sim;

#[cfg(feature = "gmod")]
pub mod api;
