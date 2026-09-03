"""Abort path (atomicity): one NO vote vetoes the whole transaction — NOBODY applies.

Transfer $150 from p0, but p0 only has $100, so p0 votes NO. Even though p1 would
gladly accept (it votes YES and locks), the coordinator must ABORT — and p1 discards
its reservation WITHOUT applying. Both stay at 100. That is atomicity / Commit-Validity."""
import common as c
import time

c.clean_state()
c.start_all()
time.sleep(1.0)

print("1) transfer -150 150  (p0 has only $100 → can't afford → votes NO)")
out = c.coordinator(["transfer -150 150"])
print("   coordinator:", out.strip().splitlines()[-1])

time.sleep(0.3)
print("2) result — a single NO aborted it; p1 was WILLING (voted YES, locked) but did NOT apply:")
c.show_logs()
print("   → both balances unchanged at 100 (atomic all-or-nothing).")

c.stop_all()
c.clean_state()
