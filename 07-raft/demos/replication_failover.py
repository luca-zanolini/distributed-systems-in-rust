"""Replication + failover: write to the leader, kill it, and the data survives on the new leader
(committed on a majority → safe on the survivors)."""
import os, socket, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "raft"))

def launch(port, peers):
    return subprocess.Popen([BIN, port] + peers, stdout=subprocess.DEVNULL, stderr=subprocess.STDOUT)

A = launch("6000", ["127.0.0.1:6001", "127.0.0.1:6002"])
B = launch("6001", ["127.0.0.1:6000", "127.0.0.1:6002"])
C = launch("6002", ["127.0.0.1:6000", "127.0.0.1:6001"])

def cmd(port, line):
    try:
        with socket.create_connection(("127.0.0.1", int(port)), timeout=3) as s:
            s.sendall((line + "\n").encode()); s.settimeout(3)
            data = b""
            while not data.endswith(b"\n"):
                ch = s.recv(1024)
                if not ch: break
                data += ch
            return data.decode().strip()
    except Exception as e:
        return f"(err {e})"

procs = {"6000": A, "6001": B, "6002": C}

def find_leader(ports):
    # randomized election timeouts: ANY node may lead — probe for it.
    for _ in range(20):
        for p in ports:
            if cmd(p, "get __probe") != "NOT LEADER":
                return p
        time.sleep(0.5)
    return ports[0]

try:
    time.sleep(2.5)
    leader = find_leader(list(procs))
    print(f"1) write to leader {leader}:")
    print(f"   set x 1        ->", cmd(leader, "set x 1"))
    print(f"   set name luca  ->", cmd(leader, "set name luca"))
    time.sleep(0.6)  # replicate + commit on a majority
    print(f"   get x @{leader}    ->", cmd(leader, "get x"))

    print(f"2) KILL leader {leader}, wait for failover:")
    procs[leader].terminate(); procs[leader].wait()
    survivors = [p for p in procs if p != leader]
    time.sleep(4.0)
    new_leader = find_leader(survivors)
    print(f"   does the data survive on the new leader ({new_leader})?")
    for p in survivors:
        print(f"   get x @{p}     ->", cmd(p, "get x"))
    for p in survivors:
        print(f"   get name @{p}  ->", cmd(p, "get name"))
finally:
    for x in (A, B, C):
        x.terminate()
