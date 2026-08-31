# distributed-systems-in-rust

A learning monorepo: building distributed-systems prototypes in Rust — one
self-contained project per folder — starting from a local key-value store and
growing toward persistence, networking, replication, and consensus.

Each folder is an independent Cargo project you can build and run on its own.

## Projects

| #  | Project | Focus |
|----|---------|-------|
| 01 | [`01-kv-store`](01-kv-store) | Persistent key-value store + REPL (JSON via serde). Rust fundamentals and the core of every distributed store. ✅ |
| 02 | [`02-networked-kv-store`](02-networked-kv-store) | Concurrent TCP client/server over the store — request/response, newline framing, fault isolation, `Arc<Mutex>` concurrency. ✅ |
| 03 | [`03-replicated-kv-store`](03-replicated-kv-store) | Replication across nodes → a fault-tolerant **quorum register**: sync/async replication, the CAP trade-off, quorum reads & writes, crash-recovery catch-up. The (1, N) Majority-Voting register; the rung before consensus. ✅ |
| 04 | [`04-leader-election`](04-leader-election) | Failure detection & leader election: heartbeats, timeouts (◇P), and **majority-vote** election so a cluster survives losing its leader — the failover `03` needs, and the building block under Raft. ✅ |

## Author

**Luca Zanolini**
[Website](https://lucazanolini.com) · [GitHub](https://github.com/luca-zanolini) · [LinkedIn](https://www.linkedin.com/in/luca-zanolini) · [X](https://x.com/luca_zanolini)
