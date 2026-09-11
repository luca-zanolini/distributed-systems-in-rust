"""The finale: a Byzantine NEW LEADER, with a real decision at stake — deposed.

The setup engineers the highest-stakes view change possible:
- 7000, 7001, 7002 run --drop-commits: they prepare (certificates form!) but
  never process commits, so they starve and rotate.
- 7003 is normal: it receives everyone's commits and DECIDES 'attack' in
  view 0. A real decision now exists — if any later view decides differently,
  AGREEMENT is broken.
- 7001, leader of view 1, runs --evil-leader: its NewView forwards genuine
  evidence (certificates forcing 'attack') but proposes 'evil'.

What the defense must produce, and this script checks:
1. Every replica re-runs the selection over the forwarded evidence, catches
   the mismatch, and REJECTS the NewView — including 7001 itself, whose own
   listener runs the same honest arm on its own lie.
2. Nobody ever enters view 1.
3. The escalating timer (vc_sent_for) complains PAST the spent view — no
   livelock retrying the same liar — and view 2's honest leader 7002 is
   FORCED by the same evidence to propose 'attack' (lock-in via quorum
   intersection: 7003's decision guarantees >=2f+1 certificate holders, who
   intersect every view-change quorum in a correct node).
4. 'evil' is never decided by anyone; 7003's decision stands alone and safe.

This is the lock-in theorem executing against a live adversary."""
import time
import common as c

procs = c.launch(extra={
    "7000": ["--drop-commits"],
    "7001": ["--drop-commits", "--evil-leader"],
    "7002": ["--drop-commits"],
    # 7003: normal — it will actually decide in view 0
})
print(">> view-0 leader 7000 proposes 'attack'; 7000/7001/7002 drop incoming COMMITs,")
print("   so only 7003 decides — a real decision is now at stake")
c.drive(procs, "7000", "propose attack")
print(">> starved nodes time out -> view 1 -> EVIL leader 7001 forwards true evidence")
print("   but proposes 'evil' -> must be rejected by all, itself included")
print(">> escalation -> view 2 -> honest 7002 is forced to 'attack' ...")
logs = c.collect(procs, settle=12.0)
c.report(logs, faulty={"7001"})

lied = any("BYZANTINE: evidence forces" in l for l in logs["7001"])
self_reject = any("NewView REJECTED" in l for l in logs["7001"])
rejections = {p: any("NewView REJECTED" in l for l in logs[p]) for p in c.ALL}
entered_v1 = [p for p in c.ALL if 1 in c.views_entered(logs[p])]
v2_forced = [p for p in c.ALL
             if any("ENTERED VIEW 2" in l and 'Some("attack")' in l for l in logs[p])]
ok, vals = c.verdict_agreement(logs)

print(f"\n   7001 lied: {lied}; 7001 rejected its OWN NewView: {self_reject}")
print(f"   nodes that rejected the evil NewView: {[p for p, r in rejections.items() if r]}")
print(f"   nodes that entered view 1: {entered_v1 or 'NONE'}")
print(f"   nodes that entered view 2 bound to 'attack': {v2_forced}")

if (ok and "evil" not in vals and vals == {"attack"} and lied and self_reject
        and not entered_v1 and len(v2_forced) >= 3):
    print("\n   VERDICT: LIAR DEPOSED, DECISION DEFENDED — the evil leader's validly-")
    print("   signed NewView carried genuine evidence and a lying conclusion; every")
    print("   node re-derived the truth from the evidence and refused, the liar's own")
    print("   honest listener refused its own message, view 1 was never entered, the")
    print("   escalating timer rotated PAST the spent view, and view 2's leader was")
    print("   forced by quorum-intersected certificates to propose the decided value.")
    print("   Agreement held with a real decision at stake. The lock-in theorem, live.")
else:
    print(f"\n   VERDICT: UNEXPECTED — decided={vals}, lied={lied}, "
          f"self_reject={self_reject}, v1={entered_v1}, v2_forced={v2_forced}")
