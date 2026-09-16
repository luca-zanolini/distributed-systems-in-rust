"""Shared harness for module 12 demos.

Module 12's exhibits are fully deterministic — no threads, no timing, no
rigging sleeps: a replica is a value and gossip is a function call. The
harness only builds, runs, and asserts on the printed evidence.
"""

import re
import subprocess
import sys
from pathlib import Path

CRATE = Path(__file__).resolve().parent.parent
BIN = CRATE / "target" / "debug"


def build():
    r = subprocess.run(["cargo", "build"], cwd=CRATE, capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stderr)
        sys.exit("BUILD FAILED")


def run(name, timeout=30):
    r = subprocess.run([str(BIN / name)], capture_output=True, text=True, timeout=timeout)
    return r.stdout


def grab(pattern, text):
    """Return the first regex capture group in text, or None."""
    m = re.search(pattern, text)
    return m.group(1) if m else None


def verdict(ok, msg):
    print(("VERDICT: PASS — " if ok else "VERDICT: FAIL — ") + msg)
    if not ok:
        sys.exit(1)
