"""M2 — strict two-phase locking: both anomalies cured, and the price paid.

lost_update_2pl: tx-scoped write locks serialize the deposits — 130, always.
read_skew_2pl: the auditor is delayed, not deceived — TOTAL 100, always.
deadlock_2pl: opposite transfers acquire in opposite orders — silent freeze
(the exhibit IS the timeout).
deadlock_2pl_fixed: sorted acquisition order — completes, balances restored.
"""

from common import build, run, grab, verdict

build()

print("== lost_update_2pl: deposits serialized by the lock table ==")
done, out, _ = run("lost_update_2pl", timeout=10)
print(out)
final = grab(r"final balance: (\d+)", out)
verdict(done and final == "130", f"final balance {final} — both deposits kept, deterministically")

print()
print("== read_skew_2pl: auditor delayed, not deceived ==")
done, out, _ = run("read_skew_2pl", timeout=10)
print(out)
total = grab(r"TOTAL (\d+)", out)
verdict(done and total == "100", f"audit TOTAL {total} — a state the bank actually held")

print()
print("== deadlock_2pl: the price of locking — expected to FREEZE ==")
done, out, _ = run("deadlock_2pl", timeout=3)
print(out if out else "(no output — frozen before the first print)")
verdict(not done, "froze past the 3s timeout: AB/BA hold-and-wait cycle, exactly as predicted")

print()
print("== deadlock_2pl_fixed: sorted acquisition breaks the cycle ==")
done, out, _ = run("deadlock_2pl_fixed", timeout=10)
print(out)
fx = grab(r"x = (\d+)", out)
fy = grab(r"y = (\d+)", out)
verdict(done and fx == "50" and fy == "50",
        f"both opposite transfers completed (x={fx}, y={fy}) — the monotone ladder cannot cycle")
