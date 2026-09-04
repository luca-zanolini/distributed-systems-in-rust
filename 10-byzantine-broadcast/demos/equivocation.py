"""Consistency (BCB4/BRB4): a Byzantine sender equivocates — telling one half of the
cluster 'attack' and the other half 'retreat' — yet no two correct processes deliver
different values.

Contrast with the naive M1 broadcast, where the same attack split the cluster
(attack / retreat / retreat). Here the echo quorum (2*count > n+f, i.e. 3 of 4) is
unreachable by both values at once: a value needs echoes from a quorum, quorums
intersect in a correct process, and a correct process echoes only one value. So the
correct processes either all deliver the SAME value or all deliver nothing — never a
split."""
import common as c

procs = c.launch(c.ALL)
print(">> BYZANTINE sender 6000 equivocates: 'attack' to one half, 'retreat' to the other")
c.drive(procs, "6000", "bcast equiv attack retreat")
delivered = c.collect(procs)
c.report(delivered, faulty={"6000"})

correct = [delivered[p] for p in c.ALL if p != "6000" and p in delivered]
values = {v for vals in correct for v in vals}
print(f"\n   distinct values delivered by correct processes: {values or 'none'}")
if len(values) <= 1:
    outcome = "all agreed on one value" if values else "nobody delivered"
    print(f"   VERDICT: CONSISTENCY HELD — no split ({outcome}).")
    print("   Both outcomes are correct: the equivocation could not make two correct")
    print("   processes deliver different values. Compare M1, where it split the cluster.")
else:
    print(f"   VERDICT: CONSISTENCY VIOLATED — the cluster split: {values}")
