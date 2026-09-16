"""M1 — the anomalies, exhibited under per-operation locking.

lost_update: two deposits (+10, +20) on x=100. Every individual lock op is
flawless; the interleaving loses one deposit — final < 130.

read_skew: a transfer x->y straddled by an auditor. The auditor certifies a
TOTAL the bank never durably held (70): a state that never existed.
"""

from common import build, run, grab, verdict

build()

print("== lost_update: two deposits, one vanishes ==")
done, out, _ = run("lost_update", timeout=10)
print(out)
final = grab(r"final balance: (\d+)", out)
verdict(done and final is not None and int(final) < 130,
        f"deposited 30, recorded {int(final) - 100 if final else '?'} — a deposit was lost")

print()
print("== read_skew: the auditor certifies a state that never existed ==")
done, out, _ = run("read_skew", timeout=10)
print(out)
total = grab(r"TOTAL (\d+)", out)
fx = grab(r"final balance: x = (\d+)", out)
fy = grab(r"y = (\d+)\s*$", out)
verdict(done and total is not None and int(total) != 100,
        f"audit reported TOTAL {total}; the bank's durable total was always 100 "
        f"(final x={fx}, y={fy})")
