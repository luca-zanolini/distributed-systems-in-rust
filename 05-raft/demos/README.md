# Raft demos

Each script launches a real 3-node cluster (the compiled binary) and drives it over TCP. Build
first, then run any script:

```bash
cargo build
python3 demos/election.py
python3 demos/replication_failover.py
```

| Script | What it demonstrates |
|---|---|
| `election.py` | The cluster elects **one leader per term**; kill the leader → a new one is elected in a **higher term** (leadership fails over automatically). |
| `replication_failover.py` | Write to the leader, then **kill it**. Because the write was committed on a **majority**, the data is already safe on the survivors, and the **newly-elected leader serves it** — consensus + durability across failover. |
| `persistence.py` | Write, then **kill the *whole* cluster**, restart it, and the data **survives** — each node reloads `currentTerm`/`votedFor`/`log`/`commitIndex` from disk (fsync'd before every reply). Without persistence a full-cluster restart would come back empty. |

The scripts find the binary at `../target/debug/raft` relative to this folder.
