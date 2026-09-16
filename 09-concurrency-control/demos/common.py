"""Shared harness for module 09 demos.

Module 09's subjects are single processes whose failures are *semantic*
(wrong balances, torn views, dead invariants) or *temporal* (a deadlock
freezes silently). The harness builds once, warms each binary (macOS
Gatekeeper assesses a fresh binary on first exec — irrelevant here for
correctness checks, but kept for uniformity with module 03), runs with
timeouts, and lets each demo assert on the printed evidence.
"""

import re
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


def grab(pattern, text):
    """Return the first regex capture group in text, or None."""
    m = re.search(pattern, text)
    return m.group(1) if m else None


def verdict(ok, msg):
    print(("VERDICT: PASS — " if ok else "VERDICT: FAIL — ") + msg)
    if not ok:
        sys.exit(1)
