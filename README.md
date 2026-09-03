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
   refusing a synchronous write (Module 03); the eventual accuracy of ◇P is a `SUSPECT` line
   retracted on recovery (Module 04); the blocking of 2PC is a cluster of processes wedged
   in-doubt on a terminal (Module 06).
2. **One staircase, one artifact.** A single key-value store is carried from a local `HashMap`
   to a consensus-replicated, transaction-capable system; each module adds exactly one
   distribution concern, and each module's closing limitations are the next module's opening
   problem.
3. **Failure first.** Modules are organized around what goes *wrong* — the deficiency tables in
   each module's notes record the deliberate sequence design → exposed defect → repair.

## Modules

| # | Module | Abstraction (CCGR) | Status |
|---|---|---|---|
| 01 | [The Key-Value Store](01-kv-store/) — state, durability, the register | registers (Ch. 4); stable storage (§2.2.4) | ✅ |
| 02 | [The Networked Store](02-networked-kv-store/) — processes, links, local concurrency | processes & perfect links (§2.1, §2.4); crash-stop (§2.2.2); safety/liveness (§2.1.3) | ✅ |
| 03 | [Replication and Quorums](03-replicated-kv-store/) — the (1, N) regular register | majority voting (§4.2.3); quorums (§2.7.3); crash-recovery (§2.2.4) | ✅ |
| 04 | [Failure Detection and Leader Election](04-leader-election/) — ◇P and Ω from heartbeats | failure detectors & Ω (§2.6); timing models (§2.5) | ✅ |
| 05 | [Consensus: Raft](05-raft/) — terms, replicated log, majority commit, safety rules, persistence | (uniform) consensus, leader-driven (Ch. 5, §5.3) | ✅ |
| 06 | [Atomic Commitment: 2PC](06-two-phase-commit/) — transactions, unanimity, the blocking flaw, strict 2PL | NBAC (§6.1); P vs. ◇P separation | ✅ |
| 07 | Byzantine Reliable Broadcast — Bracha's protocol: `3f+1`, echo/ready amplification | Byzantine broadcast (Ch. 3) | planned |
| 08 | Byzantine Consensus — PBFT-style: two-phase voting, certificates, view-change | Byzantine consensus (Ch. 5) | planned |

**Why 07 before 08.** Byzantine *reliable broadcast* (Bracha 1987) is the correct first
Byzantine primitive: it introduces the `n > 3f` bound, supermajority quorums, and the
echo/ready amplification pattern — the consistency mechanics of the Byzantine world — without
view changes or leader replacement. CCGR follows the same order (Byzantine broadcast in Ch. 3
before Byzantine consensus in Ch. 5), and PBFT's prepare/commit certificates are structurally
Bracha's echo/ready phases put in service of agreement. Module 08 then isolates what is *new*
in Byzantine consensus: the view-change.

## Theory notes (cross-module lecture notes)

- [**CONSENSUS.md**](05-raft/CONSENSUS.md) — agreement and impossibility: the consensus
  specification, FLP and its two circumventions, timing models and failure detectors
  (P, ◇P, Ω; CHT), quorum arithmetic (majority vs. `3f+1`), protocol families
  (Paxos/Raft/PBFT/HotStuff), consensus vs. atomic commitment, CAP.
- [**CONSISTENCY_AND_CONCURRENCY.md**](CONSISTENCY_AND_CONCURRENCY.md) — consistency
  conditions and concurrency control: histories and linearizability (Herlihy–Wing),
  sequential consistency and composability, transactions and ACID formally,
  schedules/serializability (conflict vs. view), strict serializability, 2PL and its variants,
  MVCC/OCC, replication vs. partitioning.

## Syllabus context and planned extensions

The current eight modules cover the *agreement-centric* core of a distributed-systems course.
Benchmarked against a full curriculum — e.g., Cambridge's *Concurrent and Distributed Systems*
(whose distributed half runs: models and faults → time → logical time and broadcast →
replication → consensus → 2PC and consistency → CRDTs/Spanner case studies) and CCGR's own
span — the natural extensions, in rough priority order:

| Candidate module | Content | Sources |
|---|---|---|
| **Logical time & broadcast** | happens-before, Lamport and vector clocks; FIFO / causal / total-order broadcast; the equivalence of total-order broadcast and consensus | Lamport 1978; CCGR Ch. 3, §6.1 |
| **Concurrency in the small** | a dedicated shared-memory module: semaphores, condition variables/monitors, producer–consumer, reader–writer, deadlock (detection, avoidance, ordering), priority inversion; atomics and a lock-free structure in Rust | Herlihy–Shavit; course notes |
| **Concurrency control, hands-on** | implement 2PL with deadlock handling, OCC, and MVCC/snapshot isolation over the store; exhibit the isolation anomalies (lost update, write skew) as tests | Bernstein et al. 1987; Cahill et al. 2008 |
| **Eventual consistency & CRDTs** | convergent replicated data types (counters, sets, registers); anti-entropy and gossip; the AP side of CAP | Shapiro et al. 2011; DeCandia et al. 2007 |
| **Physical time** | clock drift and synchronization, NTP; bounded-uncertainty clocks and commit-wait (TrueTime); Spanner as a capstone case study | Corbett et al. 2012 |
| **Membership & failure detection at scale** | gossip protocols, SWIM, φ-accrual detectors | Das et al. 2002 |
| **Global snapshots** | consistent cuts; Chandy–Lamport | Chandy & Lamport 1985 |

## Using this material

Each module directory contains: `README.md` (the module notes: objectives, system model,
formal specifications, algorithm and correctness argument, implementation correspondence,
limitations, exercises, references), `src/` (the Rust implementation), and `demos/`
(scripted failure experiments that reproduce the notes' claims against real processes over
TCP). Modules are self-standing but reference one another; the intended order is numerical.
Exercises range from proofs and counterexample constructions to implementation extensions and
are suitable as homework.

Build any module with `cargo build` in its directory; the demos require only Python 3 and the
compiled binary.

## Author

**Luca Zanolini**
[Website](https://lucazanolini.com) · [GitHub](https://github.com/luca-zanolini) ·
[LinkedIn](https://www.linkedin.com/in/luca-zanolini) · [X](https://x.com/luca_zanolini)
