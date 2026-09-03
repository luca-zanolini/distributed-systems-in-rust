"""Shared helpers for the 2PC demos: launch a cluster, drive the coordinator, read state."""
import os, subprocess, time, glob

HERE = os.path.dirname(__file__)
CRATE = os.path.abspath(os.path.join(HERE, ".."))          # participants' cwd → 2pc-*.state land here
BIN = os.path.join(CRATE, "target", "debug", "two-phase-commit")

# A 2-node cluster: two bank branches on 6000 and 6001.
PARTICIPANTS = ["127.0.0.1:6000", "127.0.0.1:6001"]
PORTS = ["6000", "6001"]
_procs = {}


def clean_state():
    for f in glob.glob(os.path.join(CRATE, "2pc-*.state")):
        os.remove(f)


def launch(port):
    log = os.path.join(CRATE, f"demo-{port}.log")
    _procs[port] = subprocess.Popen(
        [BIN, "participant", port], cwd=CRATE,
        stdout=open(log, "w"), stderr=subprocess.STDOUT,
    )


def start_all():
    for p in PORTS:
        launch(p)


def stop_all(hard=False):
    for p in list(_procs):
        _procs[p].kill() if hard else _procs[p].terminate()
        _procs[p].wait()
    _procs.clear()


def coordinator(cmds):
    """Run a coordinator session, feeding it newline-separated commands. Returns its stdout."""
    r = subprocess.run(
        [BIN, "coordinator", *PARTICIPANTS], cwd=CRATE,
        input="\n".join(cmds) + "\n", text=True, capture_output=True, timeout=30,
    )
    return r.stdout


def state(port):
    """Read a participant's on-disk state file as a dict, or {} if none yet."""
    path = os.path.join(CRATE, f"2pc-{port}.state")
    if not os.path.exists(path):
        return {}
    out = {}
    for line in open(path).read().splitlines():
        k, _, v = line.partition(" ")
        out[k] = v
    return out


def show_state(label):
    print(f"   {label}")
    for p in PORTS:
        s = state(p)
        if not s:
            print(f"     p{p}: (no state file — never persisted)")
            continue
        bal = s.get("balance", "?")
        prep = s.get("prepared", "-")
        lock = "UNLOCKED" if prep == "-" else f"LOCKED in-doubt on tx {prep}"
        print(f"     p{p}: balance {bal}   [{lock}]")


def show_logs():
    for p in PORTS:
        path = os.path.join(CRATE, f"demo-{p}.log")
        print(f"   --- p{p} log ---")
        try:
            for l in open(path).read().splitlines():
                print(f"     {l}")
        except FileNotFoundError:
            pass
