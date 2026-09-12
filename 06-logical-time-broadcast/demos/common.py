"""Shared helpers for the logical-time & broadcast demos.

Launches a 4-node cluster of the compiled binary and drives any node's stdin over
subprocess pipes (a backgrounded shell pipeline does not reliably deliver stdin)."""
import atexit, os, subprocess, time

HERE = os.path.dirname(__file__)
CRATE = os.path.abspath(os.path.join(HERE, ".."))
BIN = os.path.join(CRATE, "target", "debug", "logical-time-broadcast")

PORTS = ["6000", "6001", "6002", "6003"]


def peers_of(port):
    return [f"127.0.0.1:{q}" for q in PORTS if q != port]


def launch(extra_args=None):
    """Start all nodes; extra_args maps port -> list of extra CLI args.
    Registers an atexit kill (a failed demo must not leak a cluster that
    poisons the next run's ports) and fails loudly if any node died during
    startup (e.g. its port was still held by a leaked process)."""
    extra_args = extra_args or {}
    procs = {}
    for p in PORTS:
        procs[p] = subprocess.Popen(
            [BIN, p, *peers_of(p), *extra_args.get(p, [])],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, text=True,
        )
    atexit.register(lambda: [pr.kill() for pr in procs.values() if pr.poll() is None])
    time.sleep(1.0)
    dead = [p for p, pr in procs.items() if pr.poll() is not None]
    if dead:
        raise SystemExit(f"nodes failed to start (port in use?): {dead}")
    return procs


def drive(procs, port, line):
    procs[port].stdin.write(line + "\n")
    procs[port].stdin.flush()


def report(procs, settle=1.5, dead=()):
    """Stop all and print each node's Holding/Delivered lines in arrival order."""
    time.sleep(settle)
    for p in PORTS:
        if p in dead:
            print(f"--- node {p} --- KILLED")
            continue
        procs[p].terminate()
        out, _ = procs[p].communicate(timeout=5)
        print(f"--- node {p} ---")
        for l in out.splitlines():
            if "Deliver" in l or "Holding" in l:
                print(f"    {l}")
