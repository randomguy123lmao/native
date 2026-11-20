## Requirements

- Windows, 64-bit Garry's Mod (default branch)
- A CPU with AVX2 (Intel 4th gen / 2013 or newer, or any AMD Ryzen)
- GTA IV for PC (Steam)

## Building

**gmsv_delirium_win64.dll** (Rust): from `native/`, `cargo build --release --lib` → `target/release/gmsv_delirium.dll`.

**vphysics.dll** (Visual Studio, CMake): put [Box3D](https://github.com/erincatto/box3d) @ `33626332` and
[source-sdk-2013](https://github.com/ValveSoftware/source-sdk-2013) @ `be57e3d0` in `VPhysics-Box3D-main/external/`,
then from `VPhysics-Box3D-main/`:

```
cmake --preset windows-x64
cmake --build --preset windows-x64-release
```
