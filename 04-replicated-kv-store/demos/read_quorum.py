"""Read quorum: an empty/stale node still answers a read with the latest value (polls a majority),
and refuses when no majority is reachable (the CP choice)."""
import os, socket, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "replicated-kv-store"))
A, B, C = "4600", "4601", "4602"  # A = write target; every node knows the other two

def launch(port, peers):
    return subprocess.Popen([BIN, port] + peers, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def wait_port(port, t=5.0):
    end = time.time() + t
    while time.time() < end:
        try:
            with socket.create_connection(("127.0.0.1", int(port)), timeout=0.2):
                return True
        except OSError:
            time.sleep(0.05)
    return False

def cmd(port, line):
    with socket.create_connection(("127.0.0.1", int(port)), timeout=3) as s:
        s.sendall((line + "\n").encode()); s.settimeout(3)
        data = b""
        while not data.endswith(b"\n"):
            ch = s.recv(1024)
            if not ch:
                break
            data += ch
        return data.decode().strip()

def peers(*ps): return [f"127.0.0.1:{p}" for p in ps]

procs = []
def track(p): procs.append(p); return p
try:
    a = track(launch(A, peers(B, C)))
    b = track(launch(B, peers(A, C)))
    c = track(launch(C, peers(A, B)))
    assert wait_port(A) and wait_port(B) and wait_port(C), "a node didn't start"

    print("1) all up — write via A:")
    print("     set name luca      ->", cmd(A, "set name luca"))

    print("2) kill C, then overwrite (2/3 quorum):")
    c.terminate(); c.wait(); time.sleep(0.3)
    print("     set name antonio   ->", cmd(A, "set name antonio"), " (A,B now (2,antonio); C is down)")

    print("3) restart C with peers but NO catch-up  → C is EMPTY (amnesia):")
    c = track(launch(C, peers(A, B)))
    assert wait_port(C); time.sleep(0.3)
    print("     [C local ] readts name ->", cmd(C, "readts name"), " (C's OWN copy: nothing)")
    print("     [C quorum] get name    ->", cmd(C, "get name"), " <- read quorum MASKS the stale node!")
    print("     [A quorum] get name    ->", cmd(A, "get name"))

    print("4) kill A and B too — only C (empty) left, no majority reachable:")
    a.terminate(); a.wait(); b.terminate(); b.wait(); time.sleep(0.3)
    print("     [C] get name ->", cmd(C, "get name"), " <- refuses (CP: consistency over availability)")
finally:
    for p in procs:
        try:
            p.terminate()
        except Exception:
            pass
