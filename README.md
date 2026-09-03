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
| 05 | [`05-raft`](05-raft) | **Raft consensus**: terms, leader election, a replicated **log**, majority commit, and the safety rules (leader completeness + Figure-8) — data survives leader failover. Fuses `03`+`04` into one algorithm. Includes [**CONSENSUS.md**](05-raft/CONSENSUS.md), a full theory map (FLP, timing, crash vs BFT, quorums, 2PC). ✅ |
| 06 | [`06-two-phase-commit`](06-two-phase-commit) | **Two-phase commit / atomic commit**: coordinator-driven PREPARE→vote→COMMIT/ABORT across **partitioned** accounts; atomicity (unanimity), durable in-doubt state (Strict 2PL), and the **blocking flaw** (coordinator crash strands participants) — making *consensus ≠ atomic commit* concrete. ✅ |

## Theory maps

Cross-cutting reference documents that span multiple projects:

- [**CONSENSUS.md**](05-raft/CONSENSUS.md) — consensus & impossibility: FLP, timing models (sync/partial-sync/async), failure detectors, crash vs Byzantine quorums, protocol families, why 2PC ≠ consensus, CAP.
- [**CONSISTENCY_AND_CONCURRENCY.md**](CONSISTENCY_AND_CONCURRENCY.md) — concurrency & consistency: sequential/concurrent/parallel, linearizability vs serializability vs strict serializability, the mechanisms (mutex/2PL/MVCC/quorums), ACID, replication vs partitioning.

## Author

**Luca Zanolini**
[Website](https://lucazanolini.com) · [GitHub](https://github.com/luca-zanolini) · [LinkedIn](https://www.linkedin.com/in/luca-zanolini) · [X](https://x.com/luca_zanolini)
