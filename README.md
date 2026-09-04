# Concurrent and Distributed Systems in Rust

A course-in-a-repository: the theory of concurrent and distributed systems, developed through a
sequence of small, working Rust implementations. Each numbered directory is a self-contained
**module** — lecture-style notes (its `README.md`), a runnable implementation, scripted failure
demonstrations, and exercises — and each module's notes are written to be readable on their own,
introducing the terminology and formal properties they need.

The theoretical spine is Cachin, Guerraoui & Rodrigues, *Introduction to Reliable and Secure
Distributed Programming*, 2nd ed., Springer, 2011 (**CCGR**): modules adopt its vocabulary
(processes and links, failure and timing models, safety/liveness, module specifications with
named properties) and cite it alongside the primary literature. The systems perspective —
*why the theory looks the way it does* — is carried by the implementations: every abstraction
is built, then broken by injected failures, and the failure is what motivates the next module.

## Design of the course

Three commitments distinguish the material:

1. **Theory with its hands dirty.** Every impossibility, property, and trade-off that can be
   demonstrated *is* demonstrated on a running cluster: the CAP trade-off is a killed replica
   refusing a synchronous write (Module 04); the eventual accuracy of ◇P is a `SUSPECT` line
   retracted on recovery (Module 05); the blocking of 2PC is a cluster of processes wedged
   in-doubt on a terminal (Module 08).
2. **One staircase, one artifact.** A single key-value store is carried from a local `HashMap`
   to a consensus-replicated, transaction-capable system; each module adds exactly one
   distribution concern, and each module's closing limitations are the next module's opening
   problem.
3. **Failure first.** Modules are organized around what goes *wrong* — the deficiency tables in
   each module's notes record the deliberate sequence design → exposed defect → repair.

## Modules

Module numbers give the **teaching order**. (The repository was not built in this order;
placeholders mark modules whose notes and code are still to come.)

| # | Module | Abstraction | Status |
|---|---|---|---|
| 01 | [The Key-Value Store](01-kv-store/) — state, durability, the register | registers (CCGR Ch. 4); stable storage (§2.2.4) | ✅ |
| 02 | [The Networked Store](02-networked-kv-store/) — processes, links, local concurrency | processes & perfect links (§2.1, §2.4); crash-stop; safety/liveness | ✅ |
| 03 | [Shared-Memory Concurrency](03-shared-memory-concurrency/) — semaphores, monitors, deadlock, lock-free | mutual exclusion; progress properties | 🔲 planned |
| 04 | [Replication and Quorums](04-replicated-kv-store/) — the (1, N) regular register | majority voting (§4.2.3); quorums (§2.7.3) | ✅ |
| 05 | [Failure Detection and Leader Election](05-leader-election/) — ◇P and Ω from heartbeats | failure detectors & Ω (§2.6); timing (§2.5) | ✅ |
| 06 | [Logical Time and Broadcast](06-logical-time-broadcast/) — Lamport/vector clocks; FIFO/causal/total order | broadcast hierarchy (Ch. 3); TOB ⟺ consensus | 🔲 planned |
| 07 | [Consensus: Raft](07-raft/) — terms, replicated log, majority commit, safety, persistence | uniform consensus, leader-driven (Ch. 5) | ✅ |
| 08 | [Atomic Commitment: 2PC](08-two-phase-commit/) — transactions, unanimity, blocking, strict 2PL | NBAC (§6.1); P vs. ◇P | ✅ |
| 09 | [Concurrency Control](09-concurrency-control/) — 2PL, OCC, MVCC; anomalies as tests | serializability mechanisms | 🔲 planned |
| 10 | [Byzantine Reliable Broadcast](10-byzantine-broadcast/) — Bracha: `3f+1`, echo/ready | Byzantine broadcast (Ch. 3) | ✅ |
| 11 | [Byzantine Consensus](11-byzantine-consensus/) — PBFT-style: certificates, view-change | Byzantine consensus (Ch. 5) | 🔲 planned |
| 12 | [Eventual Consistency, CRDTs, Gossip](12-crdts-eventual-consistency/) — SEC, semilattices, anti-entropy | the AP regime | 🔲 planned |

**Why Byzantine broadcast (10) precedes Byzantine consensus (11).** Bracha's reliable
broadcast is the correct first Byzantine primitive: it introduces the `n > 3f` bound,
supermajority quorums, and echo/ready amplification — the consistency mechanics of the
Byzantine world — without view changes. CCGR follows the same order (broadcast in Ch. 3 before
consensus in Ch. 5), and PBFT's prepare/commit certificates are structurally Bracha's
echo/ready phases put in service of agreement; Module 11 then isolates what is genuinely new:
the view-change.

## Theory notes (cross-module lecture notes)

- [**CONSENSUS.md**](07-raft/CONSENSUS.md) — agreement and impossibility: the consensus
  specification, FLP and its two circumventions, timing models and failure detectors
  (P, ◇P, Ω; CHT), quorum arithmetic (majority vs. `3f+1`), protocol families
  (Paxos/Raft/PBFT/HotStuff), consensus vs. atomic commitment, CAP.
- [**CONSISTENCY_AND_CONCURRENCY.md**](CONSISTENCY_AND_CONCURRENCY.md) — consistency
  conditions and concurrency control: histories and linearizability (Herlihy–Wing),
  sequential consistency and composability, transactions and ACID formally,
  schedules/serializability (conflict vs. view), strict serializability, 2PL and its variants,
  MVCC/OCC, replication vs. partitioning.

Planned notes-level additions (deliberately not modules): **physical time** — clock
synchronization, NTP, bounded-uncertainty clocks and commit-wait, with **Spanner** as the
closing case study tying together Modules 04, 07, 08 and strict serializability;
**Chandy–Lamport snapshots** appear as an exercise in Module 06.

## Using this material

Each built module directory contains: `README.md` (the module notes: objectives, system model,
formal specifications, algorithm and correctness argument, implementation correspondence,
limitations, exercises, references), `src/` (the Rust implementation), and `demos/`
(scripted failure experiments that reproduce the notes' claims against real processes over
TCP). Planned modules carry a syllabus-level README stating scope, deliverables, and sources.
Exercises range from proofs and counterexample constructions to implementation extensions and
are suitable as homework.

Build any implemented module with `cargo build` in its directory; the demos require only
Python 3 and the compiled binary.

## Author

**Luca Zanolini**
[Website](https://lucazanolini.com) · [GitHub](https://github.com/luca-zanolini) ·
[LinkedIn](https://www.linkedin.com/in/luca-zanolini) · [X](https://x.com/luca_zanolini)
