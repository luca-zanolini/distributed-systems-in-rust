"""Safety under a Byzantine leader (M2): equivocation cannot split the decision.

The view-0 leader sends 'attack' to one peer and 'retreat' to the other two,
and withholds its own vote (it never feeds its own tally). Prepare tallies
stick at attack:1 / retreat:2 — the quorum of 3 is unreachable for both values,
so no certificate forms and nobody decides. Agreement holds vacuously;
Termination is taken hostage.

We collect BEFORE the 4-second progress timeout so the wedge itself is visible;
left running, the view change would fire — that is the next demo."""
import common as c

procs = c.launch()
print(">> BYZANTINE leader 7000 equivocates: 'attack' to one peer, 'retreat' to two,")
print("   and withholds its own vote — the split is frozen")
c.drive(procs, "7000", "bcast equiv attack retreat")
logs = c.collect(procs, settle=3.0)   # < 4s timeout: observe the wedge, not the rescue
c.report(logs, faulty={"7000"})

ok, vals = c.verdict_agreement(logs, faulty={"7000"})
n_decided = sum(1 for p in c.ALL if c.decisions(logs[p]))
if ok and n_decided == 0:
    print("\n   VERDICT: WEDGED SAFELY — no decision anywhere, and no split:")
    print("   two prepare certificates for different values would need a common")
    print("   correct testifier (quorums of 3 intersect in >= 2 nodes, >= 1 correct),")
    print("   and first-testimony-wins lets each node vouch once. Compare Module 10 M1,")
    print("   where the same attack split the naive broadcast. Termination, however,")
    print("   is lost — the debt the view change pays off.")
elif not ok:
    print(f"\n   VERDICT: SAFETY VIOLATED — correct nodes split: {vals}")
else:
    print(f"\n   VERDICT: UNEXPECTED — {n_decided} node(s) decided {vals}")
