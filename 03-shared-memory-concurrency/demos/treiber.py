"""M4 demo: lock-free Treiber stack — conservation under contention.

Claims reproduced:
  1. 4 lock-free producers x 100k pushes; drain audits per-producer and total counts.
  2. CONSERVATION EXACT: CAS cannot lose a push (two successes on one
     photograph are impossible) — contrast counters.rs's `broken`, which
     loses ~75% via unconditional store.
Run several times: lock-free bugs are schedule-dependent; one green run is
an anecdote, N green runs are evidence (still not proof — that's Quint's job).
"""

from common import build, run, warm, verdict

build()
warm("treiber")

RUNS = 5
for i in range(RUNS):
    done, out, secs = run("treiber", timeout=60)
    assert done, "treiber did not finish"
    assert "CONSERVATION EXACT" in out, f"run {i}: {(out.splitlines() or ['<no output>'])[-1]}"
    print(f"run {i}: exact (400000/400000) in {secs*1000:.0f}ms")

verdict(True, f"{RUNS}/{RUNS} runs conserve exactly — CAS refuses stale evidence")
