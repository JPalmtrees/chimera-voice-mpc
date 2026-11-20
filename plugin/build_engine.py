#!/usr/bin/env python3
"""Build Chimera Voice end to end: the CHIMERA engine (Rust) for every target, then the plugin.

    python "plugins/Chimera Voice/build_engine.py" [build.py options...]

1. cargo builds crates/chimera-mpc of the chimera-vocal-synth fork as a static library for
   arm32 (MPC One, Live, X, Force...), arm64 (Live III, XL) and this computer (host simulator).
2. The libraries go to lib/<target>/ (link.json tells tools/build.py to link them), and the
   engine's data (pronouncing dictionary, Mandarin lexicon, their licences) to Content/.
3. tools/build.py builds, packages and validates the plugin folder as usual.

The fork is found at ../../../chimera-vocal-synth (beside the SDK), or set CHIMERA_DIR.
Needs Rust (rustup) with the armv7-unknown-linux-gnueabihf and aarch64-unknown-linux-gnu targets.
"""
import os
import platform
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SDK = os.path.normpath(os.path.join(HERE, "..", ".."))
CHIMERA = os.environ.get("CHIMERA_DIR") or os.path.normpath(os.path.join(SDK, "..", "chimera-vocal-synth"))

ARM = {
    "arm32": ("armv7-unknown-linux-gnueabihf", "cortex-a17"),
    "arm64": ("aarch64-unknown-linux-gnu", "cortex-a55"),
}


def native_triple():
    m = platform.machine().lower()
    arm = m in ("arm64", "aarch64")
    if sys.platform == "win32":
        return ("aarch64" if arm else "x86_64") + "-pc-windows-gnu"
    if sys.platform == "darwin":
        return ("aarch64" if arm else "x86_64") + "-apple-darwin"
    return ("aarch64" if arm else "x86_64") + "-unknown-linux-gnu"


def find_cargo():
    c = shutil.which("cargo")
    if c:
        return c
    home = os.path.join(os.path.expanduser("~"), ".cargo", "bin", "cargo" + (".exe" if sys.platform == "win32" else ""))
    if os.path.isfile(home):
        return home
    sys.exit("cargo not found: install Rust from https://rustup.rs")


def main():
    if not os.path.isfile(os.path.join(CHIMERA, "crates", "chimera-mpc", "Cargo.toml")):
        sys.exit("CHIMERA fork not found at %s (set CHIMERA_DIR)" % CHIMERA)
    cargo = find_cargo()
    native = native_triple()
    triples = [t for t, _ in ARM.values()] + [native]
    env = dict(os.environ)
    for t, cpu in ARM.values():
        env["CARGO_TARGET_%s_RUSTFLAGS" % t.upper().replace("-", "_")] = "-C target-cpu=" + cpu
    cmd = [cargo, "build", "--release", "-p", "chimera-mpc"]
    for t in triples:
        cmd += ["--target", t]
    print("+", " ".join(cmd), flush=True)
    subprocess.check_call(cmd, cwd=CHIMERA, env=env)
    for name, t in [("arm32", ARM["arm32"][0]), ("arm64", ARM["arm64"][0]), ("native", native)]:
        src = os.path.join(CHIMERA, "target", t, "release", "libchimera_mpc.a")
        dst = os.path.join(HERE, "lib", name)
        os.makedirs(dst, exist_ok=True)
        shutil.copy2(src, os.path.join(dst, "libchimera_mpc.a"))
    content = os.path.join(HERE, "Content")
    os.makedirs(content, exist_ok=True)
    data = os.path.join(CHIMERA, "public", "data")
    for f in ("cmudict.txt.gz", "CMUDICT-LICENSE.txt", "mandarin.txt.gz", "MANDARIN-PINYIN-LICENSE.txt"):
        shutil.copy2(os.path.join(data, f), os.path.join(content, f))
    shutil.copy2(os.path.join(CHIMERA, "LICENSE"), os.path.join(content, "CHIMERA-LICENSE.txt"))
    shutil.copy2(os.path.join(SDK, "LICENSE"), os.path.join(content, "MPC-SDK-LICENSE.txt"))
    build = [sys.executable, os.path.join(SDK, "tools", "build.py"), HERE] + sys.argv[1:]
    print("+", " ".join(build), flush=True)
    sys.exit(subprocess.call(build, cwd=SDK))


if __name__ == "__main__":
    main()
