"""Liveness restored (M3): the view change deposes the faulty leader and the
new leader's proposal decides.

The equivocation wedge from the previous demo, left to run: after the 4-second
progress timeout every node broadcasts VIEWCHANGE for view 1 (join on >f,
enter on >2f). Nobody holds a prepare certificate, so the new leader 7001 logs
'expected None' (nothing certified); we then have it propose 'c',
and the ordinary three-phase protocol decides it in view 1 — Byzantine
ex-leader included, since its malice was only ever in its scripted proposal."""
import common as c

procs = c.launch()
print(">> BYZANTINE leader 7000 equivocates — view 0 wedges")
c.drive(procs, "7000", "bcast equiv attack retreat")
print(">> waiting out the 4s progress timeout: complain -> join -> enter view 1 ...")
import time; time.sleep(6.0)
print(">> new leader 7001 proposes 'c' in view 1")
c.drive(procs, "7001", "propose c")
logs = c.collect(procs, settle=2.0)
c.report(logs, faulty={"7000"})

ok, vals = c.verdict_agreement(logs, faulty={"7000"})
n_decided = sum(1 for p in c.ALL if c.decisions(logs[p]))
in_v1 = sum(1 for p in c.ALL if 1 in c.views_entered(logs[p]))
fresh = any("expected None" in l for l in logs["7001"])   # entry with no forced value
if ok and n_decided == len(c.ALL) and in_v1 == len(c.ALL) and vals == {"c"}:
    print(f"\n   VERDICT: UNWEDGED — all {in_v1}/4 entered view 1"
          + (", entered with expected None (correct: nothing was certified)" if fresh else "")
          + f", and {n_decided}/4 decided {vals}.")
    print("   M2's permanent wedge is now a five-second detour: rotation finds an")
    print("   honest leader; the read phase tells it what it may propose.")
else:
    print(f"\n   VERDICT: UNEXPECTED — decided={n_decided}/4 values={vals}, "
          f"entered view 1: {in_v1}/4")
