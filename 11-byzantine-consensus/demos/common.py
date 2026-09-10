"""Shared helpers for the Byzantine consensus demos.

Launches a cluster of the compiled binary, drives nodes over their stdin, and
reports what each node DECIDED plus the view-change events it logged. Uses
subprocess pipes (not a shell) because a backgrounded shell pipeline does not
reliably deliver stdin to a child process."""
import os, subprocess, time

HERE = os.path.dirname(__file__)
CRATE = os.path.abspath(os.path.join(HERE, ".."))
BIN = os.path.join(CRATE, "target", "debug", "byzantine-consensus")

ALL = ["7000", "7001", "7002", "7003"]      # n = 4, f = 1; leader of view 0 = 7000


def addr(port):
    return f"127.0.0.1:{port}"


def peers_of(port):
    return [addr(q) for q in ALL if q != port]


def keygen():
    """Generate the trusted-setup keys (keys/<port>.sk/.pk) in the crate dir."""
    subprocess.run([BIN, "keygen", *ALL], cwd=CRATE, check=True,
                   capture_output=True)


def launch(ports=ALL):
    """Start the given nodes; return {port: Popen}. Nodes not listed are 'down'."""
    keygen()
    procs = {}
    for p in ports:
        procs[p] = subprocess.Popen(
            [BIN, p, *peers_of(p)],
            cwd=CRATE,   # nodes read keys/ relative to the crate dir
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, text=True,
        )
    time.sleep(0.6)   # let listeners bind
    return procs


def drive(procs, port, line):
    """Feed one command line to a node's stdin."""
    procs[port].stdin.write(line + "\n")
    procs[port].stdin.flush()


def collect(procs, settle=2.0):
    """Wait, stop all nodes, and return {port: [log lines]}."""
    time.sleep(settle)
    for p in procs.values():
        p.terminate()
    out = {}
    for port, p in procs.items():
        text, _ = p.communicate(timeout=5)
        out[port] = text.splitlines()
    return out


def decisions(lines):
    """Extract the decided values from one node's log."""
    return [l.split("DECIDED on message:")[1].strip()
            for l in lines if "DECIDED on message:" in l]


def views_entered(lines):
    """Extract the view numbers this node entered."""
    return [int(l.split("ENTERED VIEW")[1].strip())
            for l in lines if "ENTERED VIEW" in l]


def report(logs, faulty=()):
    """Print per-node: decided values and views entered."""
    for port in ALL:
        if port not in logs:
            print(f"   node {port}: DOWN")
            continue
        tag = "BYZANTINE" if port in faulty else "correct  "
        d = decisions(logs[port])
        v = views_entered(logs[port])
        extra = f", entered views {v}" if v else ""
        print(f"   {tag} {port}: decided {d if d else 'NOTHING'}{extra}")


def verdict_agreement(logs, faulty=()):
    """Return (ok, distinct-values-decided-by-correct-nodes)."""
    vals = {v for p in ALL if p in logs and p not in faulty
            for v in decisions(logs[p])}
    return len(vals) <= 1, vals
