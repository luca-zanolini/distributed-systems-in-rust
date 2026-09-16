"""M4 — MVCC/snapshot isolation: readers never block, retry, or lie — and the
one anomaly that survives.

mvcc: scene 1, racing deposits — first-committer-wins still polices writers
(one CONFLICT). Scene 2, an auditor straddles a transfer that commits inside
its read gap — and reports a consistent TOTAL on its first and only attempt:
read-only transactions cannot fail under MVCC.

write_skew: two withdrawals each read BOTH accounts, write DISJOINT keys.
First-committer-wins checks only write sets — both commit, x+y >= 0 dies,
and no CONFLICT is ever printed. The anomaly separating snapshot isolation
from serializability.
"""

from common import build, run, verdict

build()

print("== mvcc: writers policed, readers untearable ==")
done, out, _ = run("mvcc", timeout=10)
print(out)
audit_lines = [l for l in out.splitlines() if l.startswith("audit:")]
verdict(done and "CONFLICT" in out and "TOTAL 230" in out and len(audit_lines) == 1,
        "deposit race caught (CONFLICT) AND the straddled auditor reported TOTAL 230 "
        "in exactly one attempt — no retry line exists")

print()
print("== write_skew: the crack in the armor ==")
done, out, _ = run("write_skew", timeout=10)
print(out)
verdict(done and "INVARIANT BROKEN" in out and "CONFLICT" not in out,
        "both withdrawals committed with disjoint write sets, no conflict was ever "
        "raised, and x + y >= 0 is dead — snapshot isolation is not serializability")
