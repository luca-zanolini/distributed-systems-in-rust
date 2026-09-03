"""Happy path: all participants vote YES → the transaction COMMITS atomically.

Transfer $30 from p0 to p1. Both can afford their side, so both vote YES and the
coordinator commits: p0 100→70, p1 100→130 (money conserved)."""
import common as c

c.clean_state()
c.start_all()
import time; time.sleep(1.0)

print("1) transfer -30 30  (p0 sends $30 to p1)")
out = c.coordinator(["transfer -30 30"])
print("   coordinator:", out.strip().splitlines()[-1])

time.sleep(0.3)
print("2) result — both applied their delta (70 + 130 = 200, conserved):")
c.show_state("final balances:")

c.stop_all()
c.clean_state()
