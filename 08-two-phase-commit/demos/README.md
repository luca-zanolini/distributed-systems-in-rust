# 2PC demos

Each script launches a real 2-node cluster (two "bank branches" — the compiled binary) and drives it
over TCP through the coordinator. Build first, then run any script:

```bash
cargo build
python3 demos/happy.py
python3 demos/abort.py
python3 demos/persistence.py
python3 demos/blocking.py
```

| Script | What it demonstrates |
|---|---|
| `happy.py` | **Commit.** All participants can afford their delta → all vote YES → the coordinator **COMMITs**, both apply (100→70, 100→130; money conserved). |
| `abort.py` | **Atomicity.** p0 can't afford its side → votes **NO** → the coordinator **ABORTs**. p1 *was* willing (voted YES, locked) but **does not apply** — a single NO vetoes everyone. Both stay at 100. |
| `persistence.py` | **Durability + in-doubt recovery.** (A) commit, kill the *whole* cluster, restart → balances reload from disk. (B) crash a participant *while in-doubt* → it comes back **still locked**, and a reboot does **not** free it. Persistence buys safety, not liveness. |
| `blocking.py` | **THE BLOCKING FLAW.** The coordinator dies after PREPARE (`transfer-crash`) → both participants are stranded **in-doubt, locked forever**; a later transaction can't make progress. The cluster is wedged. This is why *consensus ≠ atomic commit*. |

The scripts share `common.py` (launch/stop cluster, drive the coordinator, read on-disk state). They
find the binary at `../target/debug/two-phase-commit` and run participants with their `cwd` set to the
crate dir, so `2pc-<port>.state` files land there (and are cleaned up at start/end). Node logs go to
`demo-<port>.log`.
