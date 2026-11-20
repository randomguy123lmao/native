# gmsv_delirium

The Garry's Mod server module behind **Delirium Physics for Garry's Mod**: [Delirium Physics](https://github.com/PVCPC/delirium-physics)
(GTA IV's Euphoria, reimplemented) as the brain of the game's own ragdolls. Every tick it reads a ragdoll's bones, runs
the Euphoria behaviours (balancing, staggering, catching falls, holding wounds, getting up...) and drives the ragdoll's
joints with them as muscles, through a modified [VPhysics-Box3D](https://github.com/Asphaltian/VPhysics-Box3D).

The Lua side (the addons that use this module) is on the Steam Workshop; this folder is the module itself.

> **Needs your own PC copy of GTA IV (Steam).** The module reads Euphoria's numbers and the character from your
> installed game when it starts. No GTA IV files are included here, and nothing works without them.

## Layout

The module builds against Delirium Physics sitting beside it:

```
native/                  this module
delirium-physics-main/   Delirium Physics (with the changes this module needs)
VPhysics-Box3D-main/     VPhysics-Box3D, with the patch that exposes ragdoll joints and muscles
```

## Building

Windows, Rust (stable, edition 2024), from this folder:

```
cargo build --release --lib
```

The module is `target/release/gmsv_delirium.dll`. Install it as `GarrysMod/garrysmod/lua/bin/gmsv_delirium_win64.dll`
(64-bit Garry's Mod), with the patched `vphysics.dll` in `GarrysMod/bin/win64/` (back the original up first).

`.cargo/config.toml` builds for CPUs with AVX2 (Intel Haswell / 2013 or newer, any AMD Ryzen) - about a third faster.
Remove it for a module that runs on any 64-bit CPU.

## Tests

The tests run the real module and the addons' real Lua outside the game, in Garry's Mod's own LuaJIT:

```
cd tests
python host.py api.lua
python host.py behaviours.lua
python host.py gmod_stub.lua core.lua
python host.py gmod_stub.lua zcityload.lua
```

They find what they need themselves, or take it from these environment variables:

| variable | what | default |
| --- | --- | --- |
| `DELIRIUM_GTA` | the GTA IV folder (with GTAIV.exe) | found in your Steam libraries |
| `GMOD_DIR` | Garry's Mod's folder (its LuaJIT runs the tests) | a folder above this one, else Steam's default |
| `DELIRIUM_DLL` | the module to test | `target/release/gmsv_delirium.dll` (or `$CARGO_TARGET_DIR`'s) |
| `DELIRIUM_ADDONS` | where the `delirium` and `delirium_zcity` addons are | the folder above this one |

`examples/` has tools run outside the game - `hostsim` (the module driving a stand-in for the game's physics; run it
with `cargo run --release --no-default-features --example hostsim`), `probe`, `hostcheck`, `rpfls` (lists GTA IV's
archives), and `getuptrain` / `getupsim` (training and trying a learned get-up). Each takes the GTA IV folder as an
argument, or uses `DELIRIUM_GTA` / your Steam libraries.

## License

GPL-3.0 - it is built from Delirium Physics, which is GPL-3.0. VPhysics-Box3D is MIT.
