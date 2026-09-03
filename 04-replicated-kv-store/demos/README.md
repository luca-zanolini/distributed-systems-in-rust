# Failure demos

Each script launches a real multi-node cluster (the compiled binary) and drives it over TCP to
make a distributed-systems property **observable** by breaking things on purpose. Build first, then
run any script:

```bash
cargo build
python3 demos/quorum_writes.py
python3 demos/catch_up.py
python3 demos/read_quorum.py
```

| Script | What it demonstrates |
|---|---|
| `quorum_writes.py` | A write survives on a **majority** (kill one backup of two → still `OK`) but fails without one (kill both → `ERR no quorum`). |
| `catch_up.py` | A crashed node restarts **empty** (amnesia), then `--catch-up` pulls a snapshot from a peer and it **converges** — recovering the write it missed. |
| `read_quorum.py` | A **read quorum** masks a stale replica: a restarted-empty node still answers a `get` with the latest value (it polls a majority); with no majority reachable it refuses (`ERR no read quorum`, the CP choice). |

The scripts find the binary at `../target/debug/replicated-kv-store` relative to this folder.
