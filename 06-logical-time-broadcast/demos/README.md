# Logical time & broadcast — demos

Each script launches a 4-node cluster of the compiled binary and drives nodes' stdin over
`subprocess` pipes. Build first, then run any script:

```bash
cargo build
python3 demos/crash_relay.py
python3 demos/causal_order.py
python3 demos/concurrent_no_total_order.py
```

| Script | What it demonstrates |
|---|---|
| `crash_relay.py` | **RB4 agreement.** The origin is killed ~50 ms after broadcasting; eager relaying delivers to every survivor anyway. |
| `causal_order.py` | **CRB5 causal delivery.** Node 6003 is slow on messages *from* 6000; the causally-later `answer` physically arrives first, is visibly **held** (`Holding …`), and is delivered only after its cause (`question`). All nodes end in the same causal order. |
| `concurrent_no_total_order.py` | **What causal order does *not* give.** Two concurrent broadcasts are delivered in *different orders* by different nodes — legitimately; agreeing on one order is total-order broadcast (= consensus; Part II). |

Historical demos: M1's equivocator-vs-eager-RB experiment (every RB property holds, outcome
still nonsense — the model-gap lesson) requires the `equiv` command, present at commit
`863b7da`; M2's Lamport-clock run (equal stamps for concurrent broadcasts) is commit
`e480823`. Both are documented with logs in the module README.
