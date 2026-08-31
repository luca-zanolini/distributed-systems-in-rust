# Failure demos

Each script launches a real multi-node cluster (the compiled binary) and drives it to make a
property observable. Build first, then run any script:

```bash
cargo build
python3 demos/failure_detection.py
python3 demos/election.py
```

| Script | What it demonstrates |
|---|---|
| `failure_detection.py` | Kill a node → after ~3s the survivors print `SUSPECT`; restart it → `ALIVE again`. An **eventually-perfect** detector (◇P): it can suspect, and it retracts when wrong. |
| `election.py` | Leadership fails over by **majority vote** (`5000 → 5001`), then kill 2 of 3 and watch the lone survivor **stand down** — only `1/3` votes, no quorum (the split-brain safety). |

The scripts find the binary at `../target/debug/leader-election` relative to this folder.
