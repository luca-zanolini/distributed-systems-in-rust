"""Failure detection: kill a node -> survivors SUSPECT it; restart it -> ALIVE again."""
import os, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "leader-election"))

def launch(port, peers):
    return subprocess.Popen([BIN, port] + peers, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

A = launch("5000", ["127.0.0.1:5001", "127.0.0.1:5002"])
B = launch("5001", ["127.0.0.1:5000", "127.0.0.1:5002"])
C = launch("5002", ["127.0.0.1:5000", "127.0.0.1:5001"])

time.sleep(2.5)
print(">> killing node 5002")
C.terminate(); C.wait()
time.sleep(4.5)                       # past the 3s timeout -> SUSPECT
print(">> restarting node 5002")
C = launch("5002", ["127.0.0.1:5000", "127.0.0.1:5001"])
time.sleep(3.0)                       # heard again -> ALIVE again

for p in (A, B, C):
    p.terminate()
for name, p in [("5000", A), ("5001", B)]:
    out, _ = p.communicate(timeout=2)
    about = [l for l in out.splitlines() if "5002" in l]
    print(f"node {name} — what it saw about 5002:")
    for l in about:
        print("     ", l)
C.communicate(timeout=2)
