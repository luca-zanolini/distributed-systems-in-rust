"""Stable storage: a decision survives the death of the ENTIRE cluster.

Every node persists its durable facts — view, the PREPARE it signed
(my_prepare), its prepare certificate, and the decided value — with an fsync
BEFORE the corresponding message leaves the machine (persist-before-
externalize, the Module 07 discipline). This demo decides a value, kills all
four processes, restarts them from disk, and checks that every node still
knows what was decided — the exact amnesia bug that Module 07 fixed for Raft,
now closed in the Byzantine setting.

Why it matters MORE here (README §; the self-equivocation argument): a
restarted node without its my_prepare record could be tricked into SIGNING a
second, conflicting PREPARE in the same view — with signatures, amnesia does
not merely weaken quorums, it turns a correct node into an equivocator and
silently breaks the ≤ f assumption."""
import time
import common as c

procs = c.launch()
print(">> leader 7000 proposes 'lunch'; the cluster decides")
c.drive(procs, "7000", "propose lunch")
logs = c.collect(procs, settle=2.0)
first = {p: c.decisions(logs[p]) for p in c.ALL}
print(f"   first life: decisions = {first}")

print(">> ENTIRE cluster killed. Restarting all four nodes from disk ...")
time.sleep(0.5)
procs = c.launch(fresh_keys=False, fresh_state=False)   # same keys, same disks
logs = c.collect(procs, settle=2.0)

recovered = {p: [l for l in logs[p] if l.startswith("RECOVERED")] for p in c.ALL}
for p in c.ALL:
    print(f"   node {p}: {recovered[p] or 'NO RECOVERY LINE'}")

ok = all(
    any('decided=Some("lunch")' in l for l in recovered[p]) for p in c.ALL
)
if ok and all(v == ["lunch"] for v in first.values()):
    print("\n   VERDICT: RESURRECTED — all four nodes died and came back still")
    print("   knowing view and decision. The write() alone would not have survived")
    print("   a power cut; sync_all() (fsync) is what made this durable. And the")
    print("   persisted my_prepare record is what stops a reborn node from signing")
    print("   a conflicting PREPARE — amnesia here is not vote-loss, it is")
    print("   self-equivocation.")
else:
    print(f"\n   VERDICT: AMNESIA — recovery lines: {recovered}")
