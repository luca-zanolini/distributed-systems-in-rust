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

try:
    time.sleep(2.5)  # 6000 becomes leader
    print("1) write to leader 6000:")
    print("   set x 1        ->", cmd("6000", "set x 1"))
    print("   set name luca  ->", cmd("6000", "set name luca"))
    time.sleep(0.6)  # replicate + commit on a majority
    print("   get x @6000    ->", cmd("6000", "get x"))

    print("2) KILL leader 6000, wait for failover:")
    A.terminate(); A.wait()
    time.sleep(4.0)
    print("   does the data survive on the new leader?")
    for p in ("6001", "6002"):
        print(f"   get x @{p}     ->", cmd(p, "get x"))
    for p in ("6001", "6002"):
        print(f"   get name @{p}  ->", cmd(p, "get name"))
finally:
    for x in (A, B, C):
        x.terminate()
