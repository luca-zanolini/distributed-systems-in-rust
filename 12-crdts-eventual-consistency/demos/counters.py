"""M1-M3a — the merge puzzle and both counters.

naive_counter: max forgets (merged 5 of a true 8), add hallucinates under
duplicate gossip (13). g_counter: three replicas converge to [3,5,2]=10
through duplicated, reordered, relayed gossip. pn_counter: decrements as
grow-only events; both replicas land on 4.
"""

from common import build, run, verdict

build()

print("== naive_counter: both bad merges photographed ==")
out = run("naive_counter")
print(out)
verdict("merged_max=5" in out and "13" in out,
        "max lost 3 increments (5 of 8); add counted the same news twice (13)")

print()
print("== g_counter: convergence through hostile delivery ==")
out = run("g_counter")
print(out)
finals = [l for l in out.splitlines() if l.startswith("final")]
verdict(len(finals) == 3 and all("[3, 5, 2] = 10" in l for l in finals)
        and "duplicate harmless" in out,
        "all 3 replicas at [3,5,2]=10; the duplicated delivery changed nothing")

print()
print("== pn_counter: decrement as a grow-only event ==")
out = run("pn_counter")
print(out)
finals = [l for l in out.splitlines() if l.startswith("final")]
verdict(len(finals) == 2 and all("= 4" in l for l in finals) and "duplicate harmless" in out,
        "retraction recorded in the N pile; both replicas agree on 4")
