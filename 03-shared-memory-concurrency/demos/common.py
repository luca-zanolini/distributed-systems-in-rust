"""Shared harness for module 03 demos.

Unlike modules 04-11 (multi-process TCP clusters), module 03's subjects are
single processes whose failures are temporal (slowness, freezes) — so the
harness's job is: build once, warm each binary (macOS Gatekeeper assesses a
fresh binary on first exec, ~300-500ms once — measuring cold runs lies),
run with timeouts, and time honestly.
"""

import subprocess
import sys
import time
from pathlib import Path

CRATE = Path(__file__).resolve().parent.parent
BIN = CRATE / "target" / "debug"


def build():
    r = subprocess.run(["cargo", "build"], cwd=CRATE, capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stderr)
        sys.exit("BUILD FAILED")


def run(name, timeout=None):
    """Run a binary; returns (completed, output, seconds). completed=False on timeout."""
    t0 = time.monotonic()
    try:
        r = subprocess.run([str(BIN / name)], capture_output=True, text=True, timeout=timeout)
        return True, r.stdout, time.monotonic() - t0
    except subprocess.TimeoutExpired as e:
        out = e.stdout.decode() if e.stdout else ""
        return False, out, time.monotonic() - t0


def warm(*names):
    """First-exec each binary once so Gatekeeper's tax never pollutes a measurement."""
    for n in names:
        run(n, timeout=30)


def verdict(ok, msg):
    print(("VERDICT: PASS — " if ok else "VERDICT: FAIL — ") + msg)
    if not ok:
        sys.exit(1)
