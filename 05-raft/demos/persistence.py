"""Persistence: write, KILL THE WHOLE CLUSTER, restart, and the data survives — each node
reloads currentTerm/votedFor/log/commitIndex from disk (fsync'd before every reply)."""
import os, socket, subprocess, time, glob

HERE = os.path.dirname(__file__)
BIN = os.path.abspath(os.path.join(HERE, "..", "target", "debug", "raft"))
STATE_DIR = os.path.abspath(os.path.join(HERE, ".."))  # where raft-*.state files live (nodes' cwd)

PEERS = {
    "6000": ["127.0.0.1:6001", "127.0.0.1:6002"],
    "6001": ["127.0.0.1:6000", "127.0.0.1:6002"],
    "6002": ["127.0.0.1:6000", "127.0.0.1:6001"],
}
procs = {}

def launch(port):
    return subprocess.Popen([BIN, port] + PEERS[port], cwd=STATE_DIR,
                            stdout=subprocess.DEVNULL, stderr=subprocess.STDOUT)

def start_all():
    for p in PEERS: procs[p] = launch(p)

def stop_all():
    for p in list(procs): procs[p].terminate(); procs[p].wait()
    procs.clear()

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
    except Exception:
        return "(err)"

def statefiles():
    return sorted(os.path.basename(f) for f in glob.glob(os.path.join(STATE_DIR, "raft-*.state")))

for f in glob.glob(os.path.join(STATE_DIR, "raft-*.state")):  # fresh start
    os.remove(f)

try:
    start_all()
    time.sleep(2.5)
    print("1) write to the cluster:")
    print("   set x 1        ->", cmd("6000", "set x 1"))
    print("   set name luca  ->", cmd("6000", "set name luca"))
    time.sleep(0.6)
    print("   get x @6000    ->", cmd("6000", "get x"))

    print("2) KILL THE WHOLE CLUSTER (all 3 nodes crash):")
    stop_all()
    time.sleep(0.5)
    print("   on-disk state:", statefiles())

    print("3) restart all 3 (each reloads from disk), wait for re-election:")
    start_all()
    time.sleep(3.5)
    print("   did the data survive a FULL-cluster crash?")
    for p in ("6000", "6001", "6002"):
        print(f"   get x    @{p} -> {cmd(p, 'get x')}")
    for p in ("6000", "6001", "6002"):
        print(f"   get name @{p} -> {cmd(p, 'get name')}")
finally:
    stop_all()
    for f in glob.glob(os.path.join(STATE_DIR, "raft-*.state")):
        try: os.remove(f)
        except Exception: pass
