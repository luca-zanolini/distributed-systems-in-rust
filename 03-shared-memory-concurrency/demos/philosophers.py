"""M2 demo: dining philosophers — deadlock, and both cures.

Claims reproduced:
  1. Naive left-then-right FREEZES: all 5 pick up a first fork, 0 meals, no exit.
  2. Ordered acquisition (min/max on fork indices): 5 meals, clean exit.
  3. Waiter (Semaphore(4) bouncer) over the SAME naive protocol: 5 meals, clean exit.
All three run with the rigging sleep — the fixes must survive hostile scheduling.
"""

from common import build, run, warm, verdict

build()
warm("philosophers_ordered", "philosophers_waiter")  # naive one gets no warmup: it never exits anyway

DEADLOCK_PATIENCE = 4  # seconds — generous; a healthy run finishes in well under 1s

done, out, _ = run("philosophers", timeout=DEADLOCK_PATIENCE)
lefts = out.count("picked up the left fork")
meals = out.count("is eating")
assert not done, "naive version EXITED — expected deadlock (rare lucky schedule? rerun)"
assert meals == 0, f"naive version ate {meals} times before freezing"
print(f"naive: froze with {lefts} first-forks held, {meals} meals — the ring, photographed")

for name in ("philosophers_ordered", "philosophers_waiter"):
    done, out, secs = run(name, timeout=DEADLOCK_PATIENCE)
    meals = out.count("is eating")
    assert done, f"{name} FROZE — fix failed"
    assert meals == 5, f"{name}: {meals} meals, expected 5"
    print(f"{name}: 5 meals, exited in {secs*1000:.0f}ms")

verdict(True, "naive freezes (0 meals); ordered and waiter both serve 5 meals under the rigged schedule")
