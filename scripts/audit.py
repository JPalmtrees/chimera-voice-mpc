#!/usr/bin/env python3
"""Pre-publication audit of this repository's own files (not the pinned upstream submodules).

    python scripts/audit.py

Flags likely secrets, e-mail addresses, machine-specific paths, AI-session artefacts and files
over 2 MB. Every hit is reported for a human to judge; exit code 1 if anything is found outside
the allow-list below (expected, reviewed matches).
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PATTERNS = {
    "secret": re.compile(r"(?i)(api[_-]?key|client[_-]?secret|passw(or)?d\s*[:=]|bearer\s+[a-z0-9]|BEGIN [A-Z ]*PRIVATE KEY"
                         r"|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|xox[bp]-[A-Za-z0-9-]+)"),
    "email": re.compile(r"[\w.+-]+@[\w-]+\.[a-z]{2,}(?:\.[a-z]{2,})?", re.I),
    "path": re.compile(r"(?i)(c:[\\/]+users|/users/[a-z]|/home/[a-z]|\badministrator\b|mpc-development)"),
    "ai-log": re.compile(r"(?i)(\.jsonl\b|conversation log|session transcript)"),
}
# reviewed, expected matches: (pattern kind, file suffix, matched text)
ALLOW = {
    ("ai-log", ".jsonl"),          # the .gitignore entry and this script
    ("path", "Administrator"),     # docs explaining the Rust registry path inside the released binaries
    ("path", "C:\\Users"),
    ("path", "c:[\\/]+users"),
}
SKIP_DIRS = {".git", "external", "__pycache__"}


def main():
    found = 0
    for root, dirs, files in os.walk(ROOT):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            p = os.path.join(root, f)
            rel = os.path.relpath(p, ROOT)
            size = os.path.getsize(p)
            if size > 2_000_000:
                print("large   %s (%.1f MB)" % (rel, size / 1e6))
            if f.endswith((".png", ".gz")) or rel == os.path.join("scripts", "audit.py"):
                continue
            text = open(p, encoding="utf-8", errors="replace").read()
            for kind, rx in PATTERNS.items():
                for m in sorted(set(x.group(0) for x in rx.finditer(text))):
                    ok = (kind, m) in ALLOW
                    print("%s %-7s %s: %s" % ("  " if ok else "!!", kind, rel, m))
                    found += not ok
    print("audit: %d unreviewed finding(s)" % found)
    sys.exit(1 if found else 0)


if __name__ == "__main__":
    main()
