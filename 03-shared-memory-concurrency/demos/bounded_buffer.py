"""M1 demo: polling vs Condvar bounded buffer.

Claims reproduced:
  1. Both versions are CORRECT: 50 items, FIFO order preserved.
  2. The polling version is nap-bound (latency floor ~ polling period);
     the Condvar version is event-driven and materially faster.
"""

from common import build, run, warm, verdict

build()
warm("bounded_buffer_polling", "bounded_buffer")


def check_correct(name):
    done, out, secs = run(name, timeout=30)
    consumed = [int(l.split()[-1]) for l in out.splitlines() if l.startswith("Consumed")]
    assert done, f"{name} did not finish"
    assert consumed == list(range(50)), f"{name}: consumed {len(consumed)} items, order broken"
    return secs


# best-of-3 to keep scheduler noise out of the comparison
t_poll = min(check_correct("bounded_buffer_polling") for _ in range(3))
t_cv = min(check_correct("bounded_buffer") for _ in range(3))

print(f"polling: {t_poll*1000:.0f}ms   condvar: {t_cv*1000:.0f}ms")
verdict(
    t_cv < t_poll,
    f"both correct (50 items FIFO); condvar ({t_cv*1000:.0f}ms) beats polling "
    f"({t_poll*1000:.0f}ms) — no clock in the program, no nap in the latency",
)
