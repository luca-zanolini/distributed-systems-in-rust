"""M2 demo: dining philosophers — deadlock, both cures, and the control experiment.

Claims reproduced:
  1. Naive left-then-right FREEZES: all 5 pick up a first fork, 0 meals, no exit.
  2. Ordered acquisition (min/max on fork indices): 5 meals, clean exit.
  3. Waiter (Semaphore(4) bouncer) over the SAME naive protocol: 5 meals, clean exit.
  4. CONTROL: waiter with 5 seats (bouncer admits everyone) — the deadlock RETURNS.
All runs keep the rigging sleep — the fixes must survive hostile scheduling.
"""

from common import build, run, warm, verdict

build()
warm("philosophers_ordered", "philosophers_waiter")  # naive one gets no warmup: it never exits anyway

DEADLOCK_PATIENCE = 6  # seconds — generous (covers Gatekeeper's cold-exec tax on the unwarmed naive binary)

done, out, _ = run("philosophers", timeout=DEADLOCK_PATIENCE)
lefts = out.count("picked up the left fork")
meals = out.count("is eating")
verdict(not done and meals == 0,
        f"naive froze with {lefts} first-forks held, {meals} meals — the ring, photographed"
        if not done else "naive version EXITED — expected deadlock (rare lucky schedule? rerun)")

for name in ("philosophers_ordered", "philosophers_waiter"):
    done, out, secs = run(name, timeout=DEADLOCK_PATIENCE)
    meals = out.count("is eating")
    verdict(done and meals == 5,
            f"{name}: {meals} meals, exited in {secs*1000:.0f}ms"
            if done else f"{name} FROZE — fix failed")

done, out, _ = run("philosophers_waiter", timeout=DEADLOCK_PATIENCE, args=("5",))
meals = out.count("is eating")
verdict(not done,
        f"CONTROL (5 seats): deadlock RESTORED ({meals} meals before the freeze) — "
        "fix-off proves the bouncer was the cause"
        if not done else "CONTROL (5 seats) EXITED — expected the deadlock back (lucky schedule? rerun)")

verdict(True, "naive freezes; ordered and waiter both serve 5 meals; 5-seat control restores the freeze")
