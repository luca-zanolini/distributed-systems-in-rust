"""Normal case (M1): a correct leader proposes; every node decides in view 0.

PRE-PREPARE from the leader, all-to-all PREPARE, all-to-all COMMIT, with both
quorums at 2*count > n+f (3 of 4). No view change should occur: the progress
timer stands down once a node has decided."""
import common as c

procs = c.launch()
print(">> leader 7000 proposes 'attack' in view 0")
c.drive(procs, "7000", "propose attack")
logs = c.collect(procs, settle=2.0)
c.report(logs)

ok, vals = c.verdict_agreement(logs)
n_decided = sum(1 for p in c.ALL if c.decisions(logs[p]))
no_vc = all(not c.views_entered(logs[p]) for p in c.ALL)
if ok and n_decided == len(c.ALL) and no_vc:
    print(f"\n   VERDICT: NORMAL CASE OK — {n_decided}/4 decided {vals}, no view change.")
else:
    print(f"\n   VERDICT: UNEXPECTED — decided={n_decided}/4, values={vals}, "
          f"view changes={'none' if no_vc else 'PRESENT'}")
