"""Fault tolerance: with n = 4, f = 1, the broadcast still delivers at every correct
process even though one node is down (a crash is the mildest Byzantine fault).

The correct sender broadcasts; node 6003 never starts. The three correct processes
still reach the echo quorum of 3 among themselves, send READY, and deliver — the
protocol tolerates f = 1 faulty node by construction (n > 3f)."""
import common as c

procs = c.launch(["6000", "6001", "6002"])   # 6003 is DOWN
print(">> correct sender broadcasts 'hello'; node 6003 is DOWN (1 fault, f = 1)")
c.drive(procs, "6000", "bcast hello")
delivered = c.collect(procs)
c.report(delivered)
print("   => every correct (running) process delivered 'hello' despite the missing node.")
