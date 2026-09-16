"""M2 demo: counting semaphore — the 3-permit invariant.

Claims reproduced:
  1. All 8 workers complete.
  2. At no instant are more than 3 workers between ENTER and EXIT
     (replayed from the log: ENTER/EXIT lines are emitted under the stdout
     lock in true order, so the log's running count is the real concurrency).
"""

from common import build, run, warm, verdict

build()
warm("semaphore")

done, out, secs = run("semaphore", timeout=30)
assert done, "semaphore demo did not finish"

inside = peak = enters = 0
for line in out.splitlines():
    if "ENTER" in line:
        inside += 1
        enters += 1
        peak = max(peak, inside)
    elif "EXIT" in line:
        inside -= 1

assert enters == 8, f"{enters} workers entered, expected 8"
assert inside == 0, "ENTER/EXIT imbalance — someone never left"
print(f"8 workers, peak concurrency {peak}, total {secs*1000:.0f}ms")
verdict(peak == 3, f"peak concurrency exactly {peak} of 3 permits; all 8 workers served")
