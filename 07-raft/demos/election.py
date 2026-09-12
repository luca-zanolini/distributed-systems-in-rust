"""Leader election + failover: one leader per term; kill the leader → a new one in a higher term."""
import os, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "raft"))

def launch(port, peers):
    return subprocess.Popen([BIN, port] + peers, stdout=open(f"/tmp/raft_{port}.log", "w"), stderr=subprocess.STDOUT)

A = launch("6000", ["127.0.0.1:6001", "127.0.0.1:6002"])
B = launch("6001", ["127.0.0.1:6000", "127.0.0.1:6002"])
C = launch("6002", ["127.0.0.1:6000", "127.0.0.1:6001"])  # survives the whole run

procs = {"6000": A, "6001": B, "6002": C}

def current_leader():
    # timeouts are randomized (as in the paper), so ANY node may lead —
    # discover the leader from the logs instead of assuming one.
    for p in procs:
        try:
            for l in open(f"/tmp/raft_{p}.log").read().splitlines():
                if "LEADER" in l:
                    return p
        except Exception:
            pass
    return None

time.sleep(3.5)
leader = current_leader() or "6000"
print(f">> killing leader {leader}")
procs[leader].terminate(); procs[leader].wait()
time.sleep(5.0)  # a survivor times out → new election in a higher term
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
