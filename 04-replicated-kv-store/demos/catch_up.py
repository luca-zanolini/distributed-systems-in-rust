"""Catch-up / anti-entropy: a crashed node restarts empty, then --catch-up makes it converge."""
import os, socket, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "replicated-kv-store"))
PRI, B1, B2 = "4500", "4501", "4502"

def launch(args):
    return subprocess.Popen([BIN] + args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

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
    with socket.create_connection(("127.0.0.1", int(port)), timeout=2) as s:
        s.sendall((line + "\n").encode()); s.settimeout(2)
        data = b""
        while not data.endswith(b"\n"):
            chunk = s.recv(1024)
            if not chunk:
                break
            data += chunk
        return data.decode().strip()

procs = []
try:
    b1 = launch([B1]); procs.append(b1)
    b2 = launch([B2]); procs.append(b2)
    assert wait_port(B1) and wait_port(B2), "a backup didn't start"
    pri = launch([PRI, f"127.0.0.1:{B1}", f"127.0.0.1:{B2}"]); procs.append(pri)
    assert wait_port(PRI), "primary didn't start"

    print("1) all 3 up — write a, b via primary:")
    print("     set a 1            ->", cmd(PRI, "set a 1"))
    print("     set b 2            ->", cmd(PRI, "set b 2"))
    print("     [backup2] get b    ->", cmd(B2, "get b"), " (backup2 has it)")

    print("2) kill backup2, then write c (quorum 2/3):")
    b2.terminate(); b2.wait(); time.sleep(0.3)
    print("     set c 3            ->", cmd(PRI, "set c 3"), " (OK — backup2 is DOWN, never saw c)")

    print("3) restart backup2 WITH --catch-up:")
    b2 = launch([B2, "--catch-up", f"127.0.0.1:{PRI}"]); procs.append(b2)
    assert wait_port(B2), "backup2 didn't restart"
    time.sleep(0.5)
    print("     [backup2] get a    ->", cmd(B2, "get a"))
    print("     [backup2] get b    ->", cmd(B2, "get b"))
    print("     [backup2] get c    ->", cmd(B2, "get c"), " <- the key it MISSED — present = caught up!")
finally:
    for p in procs:
        try:
            p.terminate()
        except Exception:
            pass
