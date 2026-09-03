"""Leader election + failover: one leader per term; kill the leader → a new one in a higher term."""
import os, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "raft"))

def launch(port, peers):
    return subprocess.Popen([BIN, port] + peers, stdout=open(f"/tmp/raft_{port}.log", "w"), stderr=subprocess.STDOUT)

A = launch("6000", ["127.0.0.1:6001", "127.0.0.1:6002"])
B = launch("6001", ["127.0.0.1:6000", "127.0.0.1:6002"])
C = launch("6002", ["127.0.0.1:6000", "127.0.0.1:6001"])  # survives the whole run

time.sleep(2.5)
print(">> killing leader 6000")
A.terminate(); A.wait()
time.sleep(4.0)  # a survivor times out → new election in a higher term
for x in (A, B, C):
    x.terminate()
time.sleep(0.3)

print("\nleadership over time (from node logs):")
for p in ("6000", "6001", "6002"):
    try:
        for l in open(f"/tmp/raft_{p}.log").read().splitlines():
            if "LEADER" in l:
                print(f"   {p}: {l}")
    except Exception:
        pass
