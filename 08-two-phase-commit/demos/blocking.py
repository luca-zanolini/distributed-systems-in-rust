"""THE BLOCKING FLAW (2PC's fatal weakness): a coordinator crash after PREPARE strands
every participant in-doubt — holding locks forever, unable to decide alone.

tx1 = `transfer-crash`: the coordinator collects YES votes, then dies WITHOUT sending a
verdict. Both participants are now in-doubt, locked. tx2 = a normal `transfer`: it is
REFUSED (both vote NO — they're locked by tx1), so tx2 aborts, but tx2's abort does NOT
free tx1's lock. The accounts are wedged forever.

Contrast with Raft (05): a majority-based protocol survives losing a node. 2PC, needing
unanimity through a single coordinator, does not. This is why *consensus ≠ atomic commit*."""
import common as c
import time

c.clean_state()
c.start_all()
time.sleep(1.0)

print("Both branches start at $100, unlocked.")
print("\n1) tx1 = transfer-crash -30 30 : coordinator collects votes, then DIES (no verdict)")
print("   tx2 = transfer -10 10        : a later transaction, in the SAME coordinator session")
out = c.coordinator(["transfer-crash -30 30", "transfer -10 10"])
for line in out.strip().splitlines():
    if line.startswith("tx "):
        print("   coordinator:", line)

time.sleep(0.3)
print("\n2) THE DAMAGE — tx1's lock is still held; tx2 could not make progress:")
c.show_state("current state:")
print("\n   Both accounts remain LOCKED on tx1. tx1's verdict will NEVER arrive")
print("   (its coordinator is gone), so the lock is held forever — across any restart.")
print("   Every future transaction on these accounts will be refused. The cluster is wedged.")
print("\n   Fix (out of scope here): make the coordinator itself fault-tolerant via consensus")
print("   → that is Paxos Commit = 2PC's decision run through Raft/Paxos (05).")

c.stop_all(hard=True)
c.clean_state()
