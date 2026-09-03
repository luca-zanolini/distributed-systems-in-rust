"""Quorum writes: a write survives on a majority of replicas, and fails without one."""
import os, socket, subprocess, time

BIN = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "replicated-kv-store"))
P, B1, B2 = "4300", "4301", "4302"

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
    pri = launch([P, f"127.0.0.1:{B1}", f"127.0.0.1:{B2}"]); procs.append(pri)
    assert wait_port(P), "primary didn't start"

    print("all 3 up            | set x 1 ->", repr(cmd(P, "set x 1")), "  (expect OK, 3/3)")
    b2.terminate(); b2.wait(); time.sleep(0.2)
    print("backup2 killed      | set y 2 ->", repr(cmd(P, "set y 2")), "  (expect OK, 2/3 quorum met)")
    b1.terminate(); b1.wait(); time.sleep(0.2)
    print("backup1 killed too  | set z 3 ->", repr(cmd(P, "set z 3")), "  (expect ERR no quorum, 1/3)")
finally:
    for p in procs:
        try:
            p.terminate()
        except Exception:
            pass
