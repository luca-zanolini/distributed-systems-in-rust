"""M4 — the epidemic: 5 replicas, a 2|3 partition, pairwise pull gossip on a
fixed rotation. During the partition the islands hold different truths; after
the heal every replica computes its way to [1,2,3,4,5] = 15. No coordinator,
no ordering, no luck — only joins.
"""

from common import build, run, verdict

build()

out = run("gossip")
print(out)

finals = [l for l in out.splitlines() if "[1, 2, 3, 4, 5] = 15" in l]
verdict(len(finals) == 5 and "identical = 15" in out,
        "all 5 replicas at [1,2,3,4,5]=15 after the heal — epidemic convergence")
