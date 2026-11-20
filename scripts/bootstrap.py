#!/usr/bin/env python3
"""Prepare the two pinned upstream trees so the plugin builds exactly as it was released.

    python scripts/bootstrap.py

The released plugin was built from two development checkouts: a branch of CHIMERA (the engine)
with one new crate (crates/chimera-mpc) and a few engine changes, and a branch of the MPC Plugin
Builder SDK with two small build hooks and the plugin project in plugins/Chimera Voice. This
repository keeps only what is ours and pins the upstream commits those branches started from:

  external/chimera-vocal-synth     submodule @ UPSTREAM["chimera-vocal-synth"]
  external/mpc-plugin-builder-sdk  submodule @ UPSTREAM["mpc-plugin-builder-sdk"]

This script:
  1. checks that both submodules are checked out at their pinned commits;
  2. applies patches/<upstream>/*.patch to them (once; it detects patches already applied);
  3. copies engine/chimera-mpc to external/chimera-vocal-synth/crates/chimera-mpc and plugin/ to
     external/mpc-plugin-builder-sdk/plugins/Chimera Voice, the places they had in the release
     build (so relative paths, the cargo workspace and its release profile are the same).

The copies inside external/ are build inputs: edit engine/ and plugin/, then run this again.
Nothing is downloaded here; `git submodule update --init` fetches the upstreams.
"""
import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UPSTREAM = {
    # the commits the released branches were based on (both in each project's public main)
    "chimera-vocal-synth": "eeb0c1940d2d52f5f5effdecf3507fe79157f389",
    "mpc-plugin-builder-sdk": "a5949cf6fe2cbfb95bcbe0ddf1f7ef555e71f2be",
}
COPIES = [
    (os.path.join("engine", "chimera-mpc"), os.path.join("external", "chimera-vocal-synth", "crates", "chimera-mpc")),
    ("plugin", os.path.join("external", "mpc-plugin-builder-sdk", "plugins", "Chimera Voice")),
]
# build products the copy must leave alone in the plugin folder (they are recreated by builds)
KEEP = {"build", "lib", "dist", "__pycache__"}


def git(cwd, *args, check=True):
    return subprocess.run(["git", *args], cwd=cwd, check=check, capture_output=True, text=True)


def main():
    for name, sha in UPSTREAM.items():
        sub = os.path.join(ROOT, "external", name)
        if not os.path.exists(os.path.join(sub, ".git")):
            sys.exit("external/%s is missing: run  git submodule update --init" % name)
        head = git(sub, "rev-parse", "HEAD").stdout.strip()
        if head != sha:
            sys.exit("external/%s is at %s, expected %s: run  git submodule update --init" % (name, head[:12], sha[:12]))
        pdir = os.path.join(ROOT, "patches", name)
        for patch in sorted(p for p in os.listdir(pdir) if p.endswith(".patch")):
            path = os.path.join(pdir, patch)
            if git(sub, "apply", "--check", path, check=False).returncode == 0:
                git(sub, "apply", "--whitespace=nowarn", path)
                print("applied  %s/%s" % (name, patch))
            elif git(sub, "apply", "--reverse", "--check", path, check=False).returncode == 0:
                print("already  %s/%s" % (name, patch))
            else:
                sys.exit("patches/%s/%s neither applies nor is applied: external/%s has local changes "
                         "(inspect with  git -C external/%s status)" % (name, patch, name, name))
    for src, dst in COPIES:
        s, d = os.path.join(ROOT, src), os.path.join(ROOT, dst)
        os.makedirs(d, exist_ok=True)
        for entry in os.listdir(s):
            a, b = os.path.join(s, entry), os.path.join(d, entry)
            if os.path.isdir(a):
                shutil.copytree(a, b, dirs_exist_ok=True, ignore=shutil.ignore_patterns(*KEEP))
            else:
                shutil.copy2(a, b)
        print("copied   %s -> %s" % (src, dst))
    print("ready: python scripts/build.py")


if __name__ == "__main__":
    main()
