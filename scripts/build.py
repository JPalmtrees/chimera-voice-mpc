#!/usr/bin/env python3
"""Build Chimera Voice: the Rust engine for ARM32, ARM64 and this computer, then the MPC plugin
folder (and, with --package, the installable zip).

    python scripts/build.py [--package]

Runs scripts/bootstrap.py first, then the plugin's own build (plugin/build_engine.py) inside the
pinned SDK, exactly as the release was built:
  external/mpc-plugin-builder-sdk/plugins/Chimera Voice/build/Gator Audio - VST - Chimera Voice/
  external/mpc-plugin-builder-sdk/plugins/Chimera Voice/dist/chimera-voice-<version>-mpc.zip  (--package)

Needs: Python 3.10+, Git, Rust (rustup) with the armv7-unknown-linux-gnueabihf and
aarch64-unknown-linux-gnu targets, and on Windows the x86_64-pc-windows-gnu target for the native
test build. The SDK fetches its C++ toolchain (zig, via pip) on first use. See docs/building.md.
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SDK = os.path.join(ROOT, "external", "mpc-plugin-builder-sdk")
PLUGIN = os.path.join(SDK, "plugins", "Chimera Voice")


def run(cmd, cwd):
    print("+", " ".join('"%s"' % c if " " in c else c for c in cmd), flush=True)
    code = subprocess.call(cmd, cwd=cwd)
    if code:
        sys.exit(code)


def main():
    package = "--package" in sys.argv[1:]
    run([sys.executable, os.path.join(ROOT, "scripts", "bootstrap.py")], ROOT)
    # (build_engine.py finds the engine beside the SDK: external/chimera-vocal-synth)
    run([sys.executable, os.path.join(PLUGIN, "build_engine.py")], SDK)
    if package:
        run([sys.executable, os.path.join(SDK, "tools", "package.py"), PLUGIN], SDK)


if __name__ == "__main__":
    main()
