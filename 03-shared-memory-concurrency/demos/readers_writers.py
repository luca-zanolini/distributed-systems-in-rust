"""M3 demo: readers-writers — the cost of role-blindness, measured.

Claims reproduced:
  1. Same workload (100 reads x 5ms + 5 writes x 5ms), both versions correct.
  2. Mutex serializes all 105 holds; RwLock lets readers stack 4-abreast.
     Expected ~3x; verdict requires >=2x to stay robust on noisy machines.
Timing is read from the programs' own Instant output (stopwatch inside the lab).
"""

import re

from common import build, run, warm, verdict

build()
warm("readers_writers_mutex", "readers_writers")


def elapsed_ms(name):
    done, out, _ = run(name, timeout=30)
    assert done, f"{name} did not finish"
    assert out.count("read #") == 100, f"{name}: reads missing"
    m = re.search(r"Elapsed time: ([0-9.]+)m?s", out)
    assert m, f"{name}: no elapsed line"
    val = float(m.group(1))
    return val if "ms" in m.group(0) else val * 1000


t_mutex = min(elapsed_ms("readers_writers_mutex") for _ in range(2))
t_rw = min(elapsed_ms("readers_writers") for _ in range(2))

print(f"mutex: {t_mutex:.0f}ms   rwlock: {t_rw:.0f}ms   speedup: {t_mutex/t_rw:.1f}x")
verdict(
    t_mutex / t_rw >= 2.0,
    f"role-aware locking: {t_mutex/t_rw:.1f}x faster on a 20:1 read-heavy workload "
    f"({t_mutex:.0f}ms -> {t_rw:.0f}ms)",
)
