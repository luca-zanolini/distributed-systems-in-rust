"""M3 — optimistic concurrency control (Kung-Robinson): detect, don't prevent.

occ: two racing deposits; the loser's validation fails, it retries — CONFLICT
is the engine's signature sound; final 130 by redo, not by waiting.

read_skew_occ: the auditor's first view TEARS (sees 130) — but validation
makes torn views unreportable: it retries and reports TOTAL 100. Read-only
transactions must validate too.
"""

from common import build, run, verdict

build()

print("== occ: the loser redoes instead of waiting ==")
done, out, _ = run("occ", timeout=10)
print(out)
verdict(done and "CONFLICT" in out and "-> 130" in out,
        "one deposit conflicted, retried, and the bank still reached 130 — detection, not prevention")

print()
print("== read_skew_occ: torn views are unreportable ==")
done, out, _ = run("read_skew_occ", timeout=10)
print(out)
verdict(done and "TORE" in out and "audit: TOTAL 100" in out,
        "the auditor SAW a torn view (130) but validation refused it; only TOTAL 100 was ever reported")
