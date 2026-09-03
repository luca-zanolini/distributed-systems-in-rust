"""Durability + in-doubt recovery: committed state AND the in-doubt lock survive a crash.

Part A: commit a transfer, KILL the whole cluster, restart → balances reload from disk.
Part B: (the subtle one) crash a participant WHILE in-doubt (voted YES, no verdict) →
it comes back STILL in-doubt, still holding its lock. Persistence buys SAFETY (never
forget a promise) but it also makes the blocking permanent — you can't reboot to escape."""
import common as c
import time, socket

c.clean_state()
c.start_all()
time.sleep(1.0)

print("=== Part A: committed state survives a full-cluster crash ===")
print("1) transfer -30 30 → COMMIT (p0 70, p1 130)")
c.coordinator(["transfer -30 30"])
time.sleep(0.3)
c.show_state("on disk before crash:")
print("2) KILL -9 the whole cluster, then restart")
c.stop_all(hard=True)
time.sleep(0.5)
c.start_all()
time.sleep(1.0)
c.show_state("after restart — reloaded from disk (not reset to 100):")

c.stop_all(hard=True)
c.clean_state()

print("\n=== Part B: the IN-DOUBT lock survives a crash too ===")
c.start_all()
time.sleep(1.0)
print("1) send a raw PREPARE to p0 (it votes YES, enters in-doubt) — NO verdict follows")
s = socket.create_connection(("127.0.0.1", 6000), timeout=3)
s.sendall(b"PREPARE 7 -40\n")
print("   p0 reply:", s.recv(100).decode().strip())
s.close()
time.sleep(0.3)
c.show_state("on disk (p0 locked in-doubt on tx 7):")
print("2) KILL -9 p0 mid-transaction, restart")
c._procs["6000"].kill(); c._procs["6000"].wait()
time.sleep(0.5)
c.launch("6000")
time.sleep(1.0)
print("3) a NEW PREPARE 8 must be REFUSED — p0 is still locked by tx 7:")
s = socket.create_connection(("127.0.0.1", 6000), timeout=3)
s.sendall(b"PREPARE 8 -10\n")
print("   p0 reply:", s.recv(100).decode().strip(), " (NO = still in-doubt, reboot didn't help)")
s.close()

c.stop_all(hard=True)
c.clean_state()
