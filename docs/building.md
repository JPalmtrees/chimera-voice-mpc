# Building from source

## Requirements

| Tool | Version used for 1.3.3-rc1 | Notes |
|---|---|---|
| Git | any recent | submodules |
| Python | 3.14 (3.10+ expected to work) | build scripts, skin generator |
| Rust (rustup) | rustc 1.100.0 | edition 2024; targets below |
| Rust targets | `armv7-unknown-linux-gnueabihf`, `aarch64-unknown-linux-gnu` | the MPC builds |
| Rust target (Windows) | `x86_64-pc-windows-gnu` | the native test build (host simulator, VST tests) |
| Zig C++ | fetched by the SDK from PyPI (`ziglang`) into `external/mpc-plugin-builder-sdk/.sdk-tools/` on first use | cross-compiles the C++ shell for ARM; needs network access once |
| Pillow | only to regenerate skin images | `pip install pillow`; the generated PNGs are committed |

```sh
rustup target add armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu      # Windows only
```

## Build

```sh
git clone --recurse-submodules https://github.com/JPalmtrees/chimera-voice-mpc.git
cd chimera-voice-mpc
python scripts/build.py --package
```

`scripts/build.py` runs, in order:

1. **`scripts/bootstrap.py`**
   - Checks that `external/chimera-vocal-synth` is at `eeb0c19` and
     `external/mpc-plugin-builder-sdk` is at `a5949cf`.
   - Applies `patches/*/*.patch`. It does this once, and recognises patches already applied.
   - Copies `engine/chimera-mpc` to `external/chimera-vocal-synth/crates/chimera-mpc`, and
     `plugin/` to `external/mpc-plugin-builder-sdk/plugins/Chimera Voice`.
2. **`plugin/build_engine.py`** (inside the SDK)
   - Runs `cargo build --release -p chimera-mpc` for ARM32 (`target-cpu=cortex-a17`), ARM64
     (`cortex-a55`) and the native target.
   - Copies the static libraries into `lib/`, and the engine's dictionary data and license files
     into `Content/`.
   - Runs the SDK's `tools/build.py`. That covers the native build, a parameter dump through the
     host simulator, skin generation from `ui.json` and `skins_post.py`, the ARM32 and ARM64
     builds, and validation.
3. With `--package`, the SDK's `tools/package.py` makes the installable zip.

Outputs, under `external/mpc-plugin-builder-sdk/plugins/Chimera Voice/`:

| Path | What |
|---|---|
| `build/Gator Audio - VST - Chimera Voice/` | the plugin folder to copy into `VST2 Plugins` |
| `build/native/chimera-voice.dll` | native test build (Windows) |
| `build/params.json` | parameter list (used by the skin generator and tests) |
| `dist/chimera-voice-1.3.3-mpc.zip` | installable zip (`--package`) |

The plugin and engine sources inside `external/` are copies. Edit `plugin/` and `engine/`, then
build again.

## Verified reproducibility

On 2026-11-20, a fresh clone of this repository with submodules at the pinned commits was built
with `python scripts/build.py --package` on Windows 11 (x86-64), using rustc 1.100.0 and the
SDK's zig. The build took 1 min 31 s. **Every file in the resulting zip is byte-identical to the
hardware-tested release zip**, including both ARM shared libraries and every skin image. The one
exception is `README.md`, a later documentation revision.

In that test the submodules came from local mirrors at the same commit hashes, because the
published `.gitmodules` points at GitHub. Fetching them from GitHub has not been tested from this
workspace.

## Platform notes

- **Windows**: the tested platform for the full build, the native test build and all tests.
- **Linux / macOS**: the ARM builds should work, since cargo and zig are cross-platform, but they
  are untested. The native link line in `plugin/link.json` lists Windows system libraries. A
  native build on another OS needs that line adapted, so the host simulator and VST tests would
  not run as-is.
- The released binaries embed a Rust registry path from the build machine
  (`C:\Users\Administrator\.cargo\registry\...`) in panic-location strings. This is harmless,
  but a future build could add `--remap-path-prefix`.

## Regenerating generated files

| File | Regenerate with |
|---|---|
| `engine/chimera-mpc/src/anim_poses.rs`, `plugin/skin/head_anim.json` | after bootstrap, in `external/chimera-vocal-synth`: `cargo run --release -p chimera-mpc --example anim_poses -- "<sdk>/plugins/Chimera Voice" 32 "<sdk>/plugins/Chimera Voice/skin/head_anim.json"`, then copy both back into `engine/` and `plugin/` |
| `plugin/skin/head_*_base.png`, `head_*_anim.png`, `anim_layout.json` | `python plugin/skin/make_assets.py --anim` |
| other skin PNGs | `python plugin/skin/make_assets.py --buttons` (page and buttons) or without options (everything) |
| `plugin/skin/head_outlines.json` | `cargo run --release -p chimera-mpc --example head_outlines > head_outlines.json` |

Regenerated images change the package. Hardware-test them again before release.
