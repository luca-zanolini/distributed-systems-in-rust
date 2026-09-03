# Module 03 — Replication and Quorums: The (1, N) Regular Register

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011). Prerequisites:
[Module 01](../01-kv-store/), [Module 02](../02-networked-kv-store/).*

**Abstract.** This module replicates the store of Module 02 across several nodes so that data
survives crashes. It develops, through a sequence of increasingly strong designs, the **(1, N)
regular register** implemented by **majority voting** (CCGR §4.2.3): writes are timestamped and
propagated to a majority **quorum**, reads consult a quorum and return the value with the highest
timestamp, and correctness rests on the fact that any two majorities intersect. Along the way the
module demonstrates — experimentally, not only in statement — the tension between consistency and
availability formalized by the **CAP theorem**, and the **crash-recovery** model with state
transfer. The register this module completes is the strongest object obtainable *without*
agreement; the questions it provably leaves open (who is the writer? who succeeds a failed
writer? what order do concurrent writes take?) motivate Modules 04 and 05.

---

## Learning objectives

After completing this module, the reader should be able to:

1. define replication and articulate its two goals (fault tolerance, availability) and its
   central cost (the consistency of copies);
2. specify the (1, N) regular register and implement it by majority voting;
3. state and prove the **quorum intersection lemma** and derive the general condition
   `R + W > N`;
4. explain the CAP trade-off as it manifests concretely in synchronous replication to an
   unreachable replica;
5. describe crash-recovery with state transfer, and its limitations relative to stable storage;
6. state precisely why a regular register is weaker than an atomic one, and what the
   read-impose (write-back) step adds;
7. enumerate the questions replication cannot answer without agreement.

---

## 1. Motivation

A single node is a single point of failure: if it crashes, the data and the service are lost.
**Replication** maintains copies of the state on several nodes so that the data survives the loss
of a subset of them. It is the foundation of every fault-tolerant storage system — and the moment
there is more than one copy, four questions arise, each a chapter of this course:

- **Consistency.** When do the copies agree, and what may a read return when they do not?
- **Availability under failure.** If some copies are unreachable, are writes still accepted?
- **Recovery.** A crashed replica restarts without its volatile state; how does it rejoin?
- **Ordering.** If operations can originate at several places, who determines their order?

Production systems occupy well-known points in this space: primary/backup replication
(PostgreSQL, MySQL, Redis), quorum replication with `R + W > N` (Dynamo, Cassandra, Riak),
consensus-backed replication (etcd, ZooKeeper, Spanner, CockroachDB), and log-shipping recovery
(Kafka, Postgres WAL shipping). This module builds the quorum-replicated point.

## 2. System model

- **Processes.** `N` server processes (nodes) `Π = {p₁, …, p_N}`, pairwise connected; clients
  are external processes as in Module 02. One distinguished node (the *primary*) performs all
  writes — the "(1, N)" in the register's name: one writer, `N` readers.
- **Links.** Perfect point-to-point links (Module 02; provided by TCP).
- **Failures.** Crash-stop for the running cluster; at most `f` nodes may crash, with
  `N ≥ 2f + 1` (a majority of nodes remains correct). Milestone M5 additionally admits
  **crash-recovery** (CCGR §2.2.4): a node may crash, lose its volatile state (*amnesia*), and
  rejoin via state transfer.
- **Timing.** Asynchronous. No timing assumption is needed: the register's safety and liveness
  hold in a fully asynchronous system, which is precisely why registers are obtainable "below"
  the consensus boundary drawn by FLP (see [CONSENSUS.md](../05-raft/CONSENSUS.md)).

## 3. The abstraction: a (1, N) regular register

**Specification (regular register; CCGR §4.1.2, adapted).** One designated process may invoke
`write(v)`; any process may invoke `read()`. Assuming the writer invokes operations
sequentially:

- **RR1 (Termination — liveness).** Every operation invoked by a correct process eventually
  completes.
- **RR2 (Validity — safety).** A read returns the value of the last completed write, or of a
  write concurrent with the read. In particular, a read *not* concurrent with any write returns
  the last value written.

Regularity forbids reading stale values but permits an anomaly under read–write concurrency:
two sequential reads overlapping one write may return *new-then-old* values. The **atomic**
(linearizable) register forbids this as well; §6 states what the upgrade requires. The formal
hierarchy — safe, regular, atomic — is developed in
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) §3.

## 4. Algorithm: majority voting

Every stored value carries a **timestamp** assigned by the single writer (a plain integer:
one writer means no ties, which is exactly what "(1, N)" buys; multiple writers would require
timestamp pairs or vector clocks).

- **Write(k, v).** The primary increments the timestamp for `k`, applies `(ts, v)` locally,
  sends `repl ts k v` to all replicas, and completes when a **write quorum** `W = ⌊N/2⌋ + 1`
  of nodes (counting itself) has acknowledged.
- **Read(k).** The reading coordinator queries all nodes for their `(ts, value)` pair for `k`,
  waits for a **read quorum** `R = ⌊N/2⌋ + 1` of replies (counting its own copy), and returns
  the value with the **highest timestamp**.

**Lemma (quorum intersection).** Any two subsets `Q₁, Q₂ ⊆ Π` with `|Q₁|, |Q₂| ≥ ⌊N/2⌋ + 1`
satisfy `Q₁ ∩ Q₂ ≠ ∅`.
*Proof.* `|Q₁ ∩ Q₂| = |Q₁| + |Q₂| − |Q₁ ∪ Q₂| ≥ (⌊N/2⌋+1) + (⌊N/2⌋+1) − N ≥ 1`. ∎

**Correctness sketch (RR2).** Let `write(v)` with timestamp `t` be the last completed write
before a read begins. Its write quorum `Q_w` holds `(t, v)`. The read's quorum `Q_r` intersects
`Q_w`, so the read receives at least one reply with timestamp ≥ `t`; timestamps grow only through
the single writer, so any strictly larger timestamp belongs to a concurrent write. Taking the
maximum-timestamp reply therefore returns the last completed or a concurrent write — RR2.
Termination (RR1) holds because at most `f ≤ ⌊N/2⌋` nodes crash, so a quorum of correct nodes
always exists and eventually replies. ∎

Majority voting is the symmetric point (`R = W = ⌊N/2⌋+1`) of the general condition
**`R + W > N`**, whose two extremes are instructive: *read-one/write-all* (`R = 1, W = N`;
CCGR §4.2.2) has the cheapest reads and completely fragile writes; majority voting tolerates a
crashed minority on both sides. Dynamo-style systems expose `R` and `W` as per-operation knobs
on this same line.

## 5. Development of the implementation

The implementation was built in six milestones, each exposing the deficiency the next one
repairs. The sequence itself is the pedagogy: it retraces the design space of replication.

| # | Design | Deficiency exposed |
|---|---|---|
| M1 | asynchronous primary→backup forwarding | primary acknowledges before the backup confirms; copies can silently diverge |
| M2 | synchronous replication (primary waits for the backup's ack) | if the backup is down, *every* write fails: consistency purchased with availability — the CAP trade-off, observed directly |
| M3 | fan-out to `N` backups, wait for **all** | one crashed backup blocks all writes: more replicas *reduced* availability |
| M4 | **quorum writes** (majority of acks) | a stale minority always exists; a write that fails its quorum is still applied locally (no rollback — a foreshadowing of atomic commit, Module 06) |
| M5 | crash-recovery by **state transfer** (`dump` snapshot on restart) | reads still consult one node, so a recovering or bypassed node can serve stale data |
| M6 | **quorum reads** with timestamps (the complete register) | the remaining questions — writer failover, concurrent writers, all-or-nothing writes — are not answerable by quorums alone |

**Experiments.** The module's demonstrations make three of these observations concrete:
(i) *CAP (M2):* kill the backup; a synchronous write fails — the system is consistent but
unavailable, where the M1 design would have acknowledged and diverged. (ii) *Quorum tolerance
(M4):* of three nodes, kill one — writes succeed on 2/3; kill two — writes are refused
(`ERR no quorum`). (iii) *Read quorums mask staleness (M6):* restart a node empty; a read served
*by that node* still returns the latest value, because the node's own stale copy is outvoted by
the quorum's timestamps.

**A remark on the role of quorum reads.** With a single *fixed* primary — the configuration this
module actually runs — the primary applies each write before replicating it and is therefore
never stale; a read served by the primary alone would already be correct, and the read quorum
adds nothing. Quorum reads become load-bearing exactly when the *reading coordinator can be
stale*: after a failover (a replica that missed the last write takes over as primary), or when
any node may coordinate reads (the configuration of experiment (iii)). This is why the module
builds the general read path even though its default deployment does not strictly require it —
and why Module 04 (failover) is what gives it force.

## 6. From regular to atomic

Majority voting as implemented yields a **regular** register. The **atomic** (linearizable)
register additionally requires that once some read returns a value, no later read returns an
older one — ruling out the new-then-old anomaly among reads concurrent with a write. The standard
repair is **read-impose** ("write-back", CCGR §4.3): before returning, a reader writes the
winning `(ts, v)` back to a quorum, ensuring every subsequent quorum intersects a set that has
seen it. This implementation deliberately omits the write-back; Exercise 3 adds it. The general
result that single-register reads and writes — but *not* arbitrary read-modify-write objects —
are implementable wait-free in asynchronous crash-prone systems is due to Attiya, Bar-Noy and
Dolev (the **ABD** algorithm), which majority voting closely follows.

## 7. Correspondence between theory and code

| Concept | Realization |
|---|---|
| versioned register per key | `Store::map: HashMap<String, (u64, String)>` — `(timestamp, value)` |
| single writer, monotone timestamps | primary assigns `next_ts` (stored ts + 1) under one lock |
| write quorum | `set`: apply locally, forward `repl ts k v`, await majority of acks |
| read quorum | `get`: poll `readts k` from a majority, return the max-timestamp value |
| crash-recovery state transfer | `--catch-up <addr>`: pull a `dump` snapshot, apply, then serve |
| CP behavior under partition | refuse writes/reads without a quorum (`ERR no quorum`) |

Wire protocol (newline-framed): `set k v` / `get k` / `remove k` (client);
`repl ts k v` (primary → replica; terminal — a replica never re-forwards);
`readts k` → `ts value` or `none` (read quorum); `dump` → the store as `repl` lines
(state transfer, reusing the replication apply path).

## 8. Limitations and outlook

- **No failover.** The primary is fixed; its crash halts writes permanently. Electing a
  replacement requires the cluster to *agree* on one — leader election. *(→ Module 04.)*
- **Single writer.** Multiple writers need timestamp pairs (rank, counter) or vector clocks, and
  a rule for concurrent-write resolution. *(→ CCGR §4.4; Dynamo's sibling design.)*
- **Deletion is not versioned.** `remove` drops the key outright, so a stale replica can
  *resurrect* it through a later read quorum; correct deletion writes a **tombstone** `(ts, ⊥)`.
  *(→ Exercise 2.)*
- **Snapshot catch-up races with writes.** A write arriving during `dump` transfer can be missed;
  log-based catch-up (snapshot + log suffix) closes the race. *(→ Module 05's log.)*
- **No stable storage.** A node acknowledges writes held only in memory; a crash after ack loses
  them. Durable acknowledgment requires logging to stable storage first. *(→ CCGR §4.5;
  Modules 05–06.)*
- **Regular, not atomic.** No read-impose. *(→ Exercise 3.)*
- **No agreement.** Ordering concurrent operations, all-or-nothing multi-node writes, and
  writer succession all require consensus or atomic commit. *(→ Modules 04–06.)*

## 9. Exercises

1. **(Quorum arithmetic.)** Generalize the lemma of §4: for read and write quorums of sizes
   `R` and `W`, prove that `R + W > N` is necessary and sufficient for every read quorum to
   intersect every write quorum. For `N = 5`, list all `(R, W)` on the boundary and discuss the
   operational meaning of the extremes.
2. **(Tombstones.)** Implement versioned deletion: `remove` writes `(ts, ⊥)` through the normal
   write path, reads treat ⊥ as absence, and a compaction pass eventually discards old
   tombstones. What goes wrong if a tombstone is compacted away while some replica still holds
   the older live value?
3. **(Atomic register.)** Add read-impose: a read writes its winning `(ts, v)` to a write quorum
   before returning. Construct an execution of the *current* code exhibiting the new-then-old
   anomaly (two sequential reads during one write), and argue that read-impose eliminates it.
4. **(CAP, precisely.)** In the M2 configuration (one backup, synchronous), state exactly which
   property from the specification of §3 is sacrificed when the backup is unreachable, and which
   would be sacrificed by the M1 design instead. Relate both to Gilbert & Lynch's formalization.
5. **(Amnesia.)** Devise an execution in which a node acknowledges a write, crashes, recovers
   via `--catch-up` from a peer that was *not* in that write's quorum, and the write is
   subsequently lost by a full-cluster read — despite quorums being used throughout. Which
   assumption of §2 does this violate, and what repairs it?

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: registers and their hierarchy (Ch. 4);
  majority voting (§4.2.3); read-one/write-all (§4.2.2); read-impose (§4.3); quorums (§2.7.3);
  crash-recovery (§2.2.4). ISBN 978-3-642-15259-7.

**Quorum replication**
- H. Attiya, A. Bar-Noy, D. Dolev, *Sharing Memory Robustly in Message-Passing Systems*,
  JACM 42(1), 1995. (The ABD emulation of atomic registers over message passing.)
- D. K. Gifford, *Weighted Voting for Replicated Data*, SOSP 1979. (The origin of quorum
  intersection for replication.)
- G. DeCandia et al., *Dynamo: Amazon's Highly Available Key-value Store*, SOSP 2007.

**Consistency vs. availability**
- S. Gilbert, N. Lynch, *Brewer's Conjecture and the Feasibility of Consistent, Available,
  Partition-Tolerant Web Services*, ACM SIGACT News 33(2), 2002.

**Toward agreement**
- M. Fischer, N. Lynch, M. Paterson, *Impossibility of Distributed Consensus with One Faulty
  Process*, JACM 32(2), 1985.
- F. Schneider, *Implementing Fault-Tolerant Services Using the State Machine Approach*,
  ACM Computing Surveys 22(4), 1990.
- D. Ongaro, J. Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*,
  USENIX ATC 2014.

---

## Running the code

```bash
cargo build && cargo test
```

Start a three-node cluster (each node lists the other two as peers):
```bash
cargo run -- 4000 127.0.0.1:4001 127.0.0.1:4002
cargo run -- 4001 127.0.0.1:4000 127.0.0.1:4002
cargo run -- 4002 127.0.0.1:4000 127.0.0.1:4001
```
Interact via the bundled client (`cargo run --bin client`); restart a crashed node with
`--catch-up 127.0.0.1:4000` so it performs state transfer before serving. The `demos/` scripts
drive the three experiments of §5 against real sockets.

---
*[Course home](../) · Previous: [Module 02](../02-networked-kv-store/) · Next:
[Module 04 — Failure Detection and Leader Election](../04-leader-election/)*
