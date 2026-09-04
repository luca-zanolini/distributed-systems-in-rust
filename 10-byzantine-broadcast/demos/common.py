"""Shared helpers for the Byzantine reliable broadcast demos.

Launches a cluster of the compiled binary, drives the designated sender over its
stdin, and reports what each node DELIVERED. Uses subprocess pipes (not a shell)
because a backgrounded shell pipeline does not reliably deliver stdin to the
sender process."""
import os, subprocess, time

HERE = os.path.dirname(__file__)
CRATE = os.path.abspath(os.path.join(HERE, ".."))
BIN = os.path.join(CRATE, "target", "debug", "byzantine-broadcast")

ALL = ["6000", "6001", "6002", "6003"]      # n = 4, f = 1
SENDER = "127.0.0.1:6000"


def peers_of(port):
    return [f"127.0.0.1:{q}" for q in ALL if q != port]


def launch(ports):
    """Start the given nodes; return {port: Popen}. Nodes not listed are 'down'."""
    procs = {}
    for p in ports:
        procs[p] = subprocess.Popen(
            [BIN, p, *peers_of(p), "--sender", SENDER],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, text=True,
        )
    time.sleep(1.0)   # let listeners bind
    return procs


def drive(procs, port, line):
    """Feed one command line to a node's stdin (the sender reads 'bcast …')."""
    procs[port].stdin.write(line + "\n")
    procs[port].stdin.flush()


def collect(procs, settle=2.0):
    """Wait, stop all nodes, and return {port: [delivered values]}."""
    time.sleep(settle)
    for p in procs.values():
        p.terminate()
    out = {}
    for port, p in procs.items():
        text, _ = p.communicate(timeout=5)
        out[port] = [
            l.split("Delivered message:")[1].strip()
            for l in text.splitlines() if "Delivered message:" in l
        ]
    return out


def report(delivered, faulty=()):
    for port in ALL:
        if port not in delivered:
            print(f"   node {port}: DOWN")
            continue
        tag = "BYZANTINE" if port in faulty else "correct  "
        vals = delivered[port]
        print(f"   {tag} {port}: delivered {vals if vals else 'NOTHING'}")
