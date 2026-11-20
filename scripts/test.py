#!/usr/bin/env python3
"""Run the project's automated tests against a build made by scripts/build.py.

    python scripts/test.py [--skip-rust]

1. Rust: the engine bridge's unit and integration tests (cargo test -p chimera-mpc, release).
2. Generated skin: tests/skin_test.py on the built Plugin Skins folder (layout rules learned on
   the MPC: visibility, captions, touch order, Quality buttons, keyboard, Q-Links, images).
3. Compiled plugin (needs the native test build, made on Windows): tests/keyboard_test.cpp
   (parameters, typing, Done/Cancel, state compatibility, long lyrics) and tests/realtime_test.cpp
   (sung syllables, host calls from the audio thread, refresh rates, block timing).
4. The SDK's host simulator (VST2 conformance, state round trip, MIDI, silence) on 3 seeds.

None of this replaces a test on the MPC itself: see docs/testing.md.
"""
import glob
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ENGINE = os.path.join(ROOT, "external", "chimera-vocal-synth")
SDK = os.path.join(ROOT, "external", "mpc-plugin-builder-sdk")
PLUGIN = os.path.join(SDK, "plugins", "Chimera Voice")
BUILD = os.path.join(PLUGIN, "build")
FOLDER = os.path.join(BUILD, "Gator Audio - VST - Chimera Voice")
failures = []


def step(name, cmd, cwd, env=None):
    print("\n== %s\n+ %s" % (name, " ".join(cmd)), flush=True)
    code = subprocess.call(cmd, cwd=cwd, env=env)
    print("-> %s" % ("ok" if code == 0 else "FAILED (%d)" % code))
    if code:
        failures.append(name)


def cargo():
    home = os.path.join(os.path.expanduser("~"), ".cargo", "bin", "cargo" + (".exe" if os.name == "nt" else ""))
    return home if os.path.isfile(home) else "cargo"


def main():
    if not os.path.isdir(FOLDER):
        sys.exit("no build found: run  python scripts/build.py  first")
    if "--skip-rust" not in sys.argv:
        step("Rust tests (chimera-mpc)", [cargo(), "test", "--release", "-p", "chimera-mpc"], ENGINE)
    step("generated skin", [sys.executable, os.path.join(PLUGIN, "tests", "skin_test.py"),
                            os.path.join(FOLDER, "Plugin Skins"), os.path.join(BUILD, "params.json")], PLUGIN)
    native = (glob.glob(os.path.join(BUILD, "native", "chimera-voice.*")) or [None])[0]
    zigpy = [p for p in (os.path.join(SDK, ".sdk-tools", "zigenv", "Scripts", "python.exe"),
                         os.path.join(SDK, ".sdk-tools", "zigenv", "bin", "python")) if os.path.isfile(p)]
    if native and zigpy:
        env = dict(os.environ, MPC_PLUGIN_FOLDER=FOLDER)
        for t in ("keyboard_test", "realtime_test"):
            exe = os.path.join(BUILD, t + (".exe" if os.name == "nt" else ""))
            step("compile %s" % t, [zigpy[0], "-m", "ziglang", "c++", "-std=c++17", "-O2", "-I", os.path.join(SDK, "framework"),
                                    os.path.join(PLUGIN, "tests", t + ".cpp"), "-o", exe], PLUGIN)
            if os.path.isfile(exe):
                step(t, [exe, native], PLUGIN, env)
        sim = os.path.join(SDK, ".sdk-tools", "host_sim" + (".exe" if os.name == "nt" else ""))
        if os.path.isfile(sim):
            for seed in ("1", "2", "3"):
                step("host simulator, seed %s" % seed, [sim, native, "test", "--seed", seed], SDK)
    else:
        print("\n(no native test build or zig toolchain: compiled-plugin tests and host simulator skipped)")
    print("\n%s" % ("FAILED: " + ", ".join(failures) if failures else "all tests passed"))
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
