"""Majority-vote election: failover 5000 -> 5001 by quorum, then kill 2 of 3 and watch the
lone survivor stand down (only 1/3 votes — no majority)."""
import os, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "leader-election"))

def launch(port, peers):
    return subprocess.Popen([BIN, port] + peers, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

A = launch("5000", ["127.0.0.1:5001", "127.0.0.1:5002"])
B = launch("5001", ["127.0.0.1:5000", "127.0.0.1:5002"])
C = launch("5002", ["127.0.0.1:5000", "127.0.0.1:5001"])   # survives the whole run

time.sleep(2.0)
print(">> killing leader 5000")
A.terminate(); A.wait()
time.sleep(4.5)                       # -> 5001 wins a 2/3 majority
print(">> killing new leader 5001")
B.terminate(); B.wait()
time.sleep(4.5)                       # -> 5002 alone: only 1/3, must stand down

def show(name, p):
    p.terminate()
    out, _ = p.communicate(timeout=2)
    lines = [l for l in out.splitlines() if any(k in l for k in ("LEADER", "voting", "majority"))]
    print(f"\nnode {name}:")
    for l in lines:
        print("   ", l)

for name, p in [("5000", A), ("5001", B), ("5002", C)]:
    show(name, p)
