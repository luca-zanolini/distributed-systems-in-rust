# Concurrency & Consistency — a theory map

> A cross-cutting reference for the distributed-systems track. It untangles a cluster of terms
> that are easy to conflate — **sequential, concurrent, parallel** (how work *runs*) versus
> **linearizability, serializability, strict serializability** (how shared state is *allowed to
> appear*) — with worked examples, the mechanisms that achieve each, and where they show up in the
> projects here (`03` quorum register, `05` Raft, `06` two-phase commit).
>
> Companion to [`05-raft/CONSENSUS.md`](05-raft/CONSENSUS.md) (the consensus/impossibility map).
> This one is about **concurrency control and consistency models**.

---

## 0. The one idea to hold onto: two orthogonal axes

Almost all the confusion here comes from mixing up two *independent* questions:

| Axis | Question | The terms | What it's a property of |
|---|---|---|---|
| **Execution** | *How* is work scheduled onto workers/time? | sequential · concurrent · parallel | the **program / hardware** |
| **Consistency** | *How* is shared state allowed to appear to observers? | linearizability · serializability · strict serializability · sequential consistency | the **shared object / correctness spec** |

They are orthogonal. You can run **concurrently** and still be **linearizable** (Raft does). You can run **sequentially** and trivially satisfy every consistency model (one worker, one op at a time). The execution axis is about *doing many things at once*; the consistency axis is about *keeping one shared story coherent while you do*. Keep them in separate mental columns.

The bridge between them is **shared mutable state**: the moment concurrent/parallel execution touches the same data, you need *coordination* (locks, quorums, consensus) to preserve a consistency model. No sharing → no consistency question at all.

---

## 1. Execution axis — sequential, concurrent, parallel

Running example: **cooks preparing dishes in a kitchen.**

- **Sequential** — one cook finishes dish 1 *completely*, then starts dish 2. One thing at a time, in order.
- **Concurrent** — one cook *interleaves*: chop for dish 1, stir dish 2, back to dish 1. Multiple dishes are *in progress*, but the single cook touches **only one at any instant**.
- **Parallel** — multiple cooks, each on a dish, hands moving **at the same physical instant**.

The distinction that trips people up (Rob Pike's phrasing):

> **Concurrency is about *dealing with* many things at once. Parallelism is about *doing* many things at once.**

- **Concurrency** is a property of **structure** — the program is decomposed into independent tasks that *can* be interleaved. A statement about the *logic*.
- **Parallelism** is a property of **execution** — tasks literally running simultaneously, which *requires* multiple execution units (cores / cooks).

Consequences:

- **Concurrency does not require parallelism.** One cook interleaving 5 dishes on one stove is concurrent with zero parallelism (a single-core OS time-slicing threads is exactly this).
- **Parallelism presupposes concurrency.** You can only run things in parallel if they were independent (concurrent) to begin with. Concurrency is the *design*; parallelism is one *runtime realization* of it — the same concurrent program becomes parallel simply by being given more cores.

| | # in progress | # at the exact same instant | needs multiple cores? |
|---|---|---|---|
| Sequential | 1 | 1 | no |
| Concurrent | many | 1 (interleaved) | no |
| Parallel | many | many | yes |

**Where the danger lives.** A purely *sequential* program can never race — there is only ever one point of control. Both *concurrency* (interleaving at an unlucky point) and *parallelism* (true simultaneity) can corrupt shared state. That is why the consistency axis exists, and why `06`'s 2PC **participant** (a single sequential accept-loop) needs no lock, while `05`'s Raft **node** (a timer thread + network handlers sharing `State`) needs `Arc<Mutex<…>>`.

---

## 2. The bridge: shared state and mutual exclusion

**Mutual exclusion (a `Mutex`)** is the most basic coordination primitive: one shared resource (the *knife*), and a worker must **acquire** it before use; if another holds it, this worker **blocks** until it's released. At most one worker holds it at a time.

In Rust, `.lock()` returns a `MutexGuard`; the lock is released automatically when the guard drops (goes out of scope). Two classic hazards the kitchen predicts exactly:

- **Holding too long starves everyone** → the rule *"never hold a lock across network I/O"*: lock, mutate, release, *then* do the slow thing.
- **Two knives grabbed in opposite orders → deadlock**: A holds knife-1 waiting for knife-2; B holds knife-2 waiting for knife-1; neither yields. (A std `Mutex` is non-reentrant, so even *one* worker locking the *same* mutex twice self-deadlocks.)

Crucial framing for what follows: **a mutex is a *mechanism*, not a consistency model.** It is *one way* to implement a linearizable object on a single machine. It is neither necessary (lock-free structures, single-threaded loops, MVCC all avoid it) nor, by itself, sufficient for the transaction-level guarantees (§4). Distinguishing *mechanism* from *guarantee* is the whole game.

---

## 3. Consistency axis I — single objects: linearizability

**Linearizability** (Herlihy & Wing, 1990) is a correctness condition for operations on a **single** shared object.

> An execution is **linearizable** if every operation appears to take effect **atomically at a single instant** (its *linearization point*) somewhere between its invocation and its response, and this instantaneous ordering is consistent with **real-time order**: if operation A returns before operation B is invoked, then A is ordered before B.

Equivalently — the **single-copy illusion**:

> The object behaves as if there were exactly **one copy**, in one place, touched by **one operation at a time**, honoring the wall clock. No stale reads; no going backwards in time.

### Worked example — a shared "soup of the day" whiteboard

```
Cook A:   [------ writes "leek" ------]        starts 12:00:03, done 12:00:06
Waiter 1:      [reads]                          reads 12:00:04  (during the write)
Waiter 2:                        [reads]        reads 12:00:07  (after the write completed)
```

- **Waiter 2 read *after* A finished → MUST see "leek".** A completed before Waiter 2 even looked; a finished write cannot be invisible to a later-starting read. Seeing "tomato" here is a **linearizability violation**.
- **Waiter 1 read *during* the write → may see either** "tomato" or "leek" — but whichever it sees pins the single instant of the snap, and everyone else's view must be consistent with that same snap.

### Weaker cousin: sequential consistency

**Sequential consistency** (Lamport, 1979) is a **global, multi-object** memory model: there is one total order over *all* operations on *all* objects that every process agrees on, and each process's own operations keep their **program order** — but it **drops the real-time requirement**. A read may return a stale value even if a newer write finished earlier in wall-clock time, as long as *some* legal global order explains all observations. Linearizability = sequential consistency **+ real-time**. (That real-time gap is exactly what distinguishes serializability from strict serializability at the transaction level — see §4/§5.) A second, subtler difference at PhD pitch: **linearizability is *local* / composable** — independently linearizable objects compose into a linearizable system — whereas **sequential consistency is *not* composable**: a system of individually sequentially-consistent objects need not be sequentially consistent as a whole. So the "+ real-time" gloss is necessary but not the *whole* story.

### The register hierarchy (CCGR vocabulary)

Cachin–Guerraoui–Rodrigues classify single-register guarantees by strength (weakest → strongest; note **safe** and **regular** are defined for a *single writer* — the multi-writer generalization is only meaningful for **atomic**):

- **Safe** — a read not concurrent with a write returns the last written value; a read *concurrent* with a write may return *any* value in the domain.
- **Regular** — like safe, but a concurrent read returns either the last value or the concurrently-written one (no garbage).
- **Atomic** — **= linearizable**: reads also respect real-time order among themselves (no "new then old" inversions across successive reads).

So CCGR's **atomic register** is precisely a linearizable single-value object. `03`'s ABD-style majority read/write with write-back builds exactly this.

---

## 4. Consistency axis II — transactions: serializability

**Serializability** (Papadimitriou, 1979; Bernstein–Hadzilacos–Goodman, 1987) is the correctness condition for **transactions** — groups of operations, potentially over **multiple** objects.

> A **schedule** (the actual interleaved sequence of all operations of all transactions) is **serializable** if it is *equivalent* to **some serial schedule** — one in which each transaction runs start-to-finish with no interleaving.

The **single-worker illusion**:

> However many workers actually interleaved their steps, the result must be identical to what **one worker** would have produced by running each transaction **completely, one after another**, in *some* order.

Three points people get wrong, in order of importance:

1. **Serializable ≠ serial.** Transactions *do* physically overlap; operations *do* interleave on the hardware. Only the **outcome** must match some non-interleaved order. It is "serial-*izable*" (reducible to serial), not "serial." The guarantee is on the *equivalence class* of the execution, not the execution.
2. ***Some* order, not a specific one.** The result need only match **at least one** serial order (A-then-B *or* B-then-A). The system picks whichever; real time does **not** constrain the choice (that's what strict serializability adds — §5).
3. **It's a property of the whole *set*.** Not "each transaction is ordered" but "the entire concurrent schedule collapses to one serial ordering of all of them." The general notion ("equivalent to *some* serial schedule") splits by what **equivalent** means: **view-serializability** (same reads-from relation + same final writes) is the broadest, but is **NP-complete** to check; **conflict-serializability** is the decidable subset that real systems actually enforce — its test is that the **conflict graph** (nodes = transactions, edge A→B when a conflicting op of A precedes one of B; two ops *conflict* if they touch the same object and ≥1 is a write) is **acyclic**. So conflict-serializable ⊊ view-serializable ⊊ (final-state) serializable; acyclicity characterizes the *conflict* variant, not serializability in general. (This subset/NP-completeness split is exactly Papadimitriou 1979.)

### The canonical anomaly — lost update

```
Pot has 100ml.
A: read 100 ─────────── write 90   (took 10)
B:     read 100 ── write 80        (took 20)
Result: pot says 80. But 30ml was taken → a serial run gives 100→90→70.
80 matches NEITHER "A then B" NOR "B then A" → NOT serializable.
```

### The distinction that clarifies everything: this pot is *linearizable*

Treat each **read** and each **write** as its own *single* operation on the pot (a register). A valid linearization exists: `readA→100, readB→100, write(90), write(80)`. Every single op took effect at one instant, no read was stale, real-time respected. **The register is linearizable — yet the lost update happened.**

Why? Because "read 100 *then* write 90" was meant to be **one indivisible unit** (a read-modify-write *transaction*), and linearizability makes no promise about *groups* of operations — only one at a time. Preventing this requires **serializability**, which governs the transaction as a unit.

> **This is the cleanest proof that linearizability and serializability are different guarantees:** a lost update on an object that is itself perfectly linearizable.

---

## 5. Strict serializability = the two combined

- **Serializable** — result matches *some* serial order (any).
- **Strict serializable** — result matches some serial order **that also respects real time**: if transaction A committed before B started, A must precede B in that order. (For transactions that *overlap*, either order is allowed — real time gives no constraint.)

So:

| Model | Unit | Multiple objects? | Respects real time? | = |
|---|---|---|---|---|
| Sequential consistency | single op | one *or many* (global) | **no** | total order + program order; **not composable** |
| **Linearizability** | single op | one object | **yes** | seq. consistency + real time |
| **Serializability** | transaction | many objects | **no** | some serial-equivalent order |
| **Strict serializability** | transaction | many objects | **yes** | serializability + linearizability |

> **Strict serializability = linearizability lifted from single operations to whole transactions.** The single-worker illusion *plus* the wall-clock honesty of the single-copy illusion. It is the gold standard: **Google Spanner** (via `TrueTime` + commit-wait) and **FoundationDB** (single-sequencer versioning) provide it. Plain serializability's freedom to reorder across real time causes surprising "the system went backwards in time" anomalies that strict serializability rules out; Spanner's `TrueTime` clocks exist precisely to pin down the global real-time order.
>
> *(Precision note: **CockroachDB** provides serializability and **single-key** linearizability but, using HLC clocks with a bounded max offset rather than TrueTime, does **not** guarantee strict serializability — stale-read / causal-reverse anomalies within the offset window are possible. **Oracle**'s `SERIALIZABLE` is actually snapshot isolation, not true serializability.)*

---

## 6. Mechanisms — how each guarantee is actually achieved

Consistency models are *specifications*; these are the *implementations*. Note the same guarantee can be reached many ways, and going distributed changes the toolbox.

### Single-object linearizability
- **Single machine:** a `Mutex` (serialize all ops), or a lock-free CAS structure, or a single-threaded event loop.
- **Replicated (no shared memory across machines):** **quorums** (ABD — `03`) or **consensus** (Raft/Paxos — `05`). *This* is the hard, distributed version: there is no network-spanning mutex, so you buy the single-copy illusion with majority intersection or a replicated log.

### Transaction serializability
- **Two-Phase Locking (2PL)** — *pessimistic*. **Basic 2PL**: all lock *acquires* precede all *releases* (a **growing** then a **shrinking** phase; no lock acquired after any is released). This alone guarantees **conflict-serializability** — and already prevents the lost update. **Strict 2PL** additionally holds all **exclusive (write)** locks until **commit/abort**; this does *not* add serializability (basic 2PL already has it) — it buys **recoverability** and **no cascading aborts**. **Rigorous 2PL** holds **all** locks (read *and* write) until commit/abort — simplest to reason about, the common industrial choice. All cost **blocking** and **deadlock** risk. *`06`'s `prepared` state is Strict/Rigorous 2PL:* an exclusive lock acquired at the YES vote and held until the COMMIT/ABORT verdict.
- **MVCC / snapshot-based** (Postgres, Oracle) — keep multiple versions; readers see a consistent snapshot and never block writers. *Serializable* Snapshot Isolation (SSI) adds conflict detection to promote snapshot isolation to serializability. *(Spanner is **not** a clean example here: it snapshots read-only transactions but uses **2PL** for read-write ones, so it isn't lock-free MVCC in the Postgres sense.)*
- **OCC (optimistic)** — run lock-free, then **validate** at commit; abort & retry on conflict.
- **Timestamp ordering** — assign each transaction a timestamp and enforce that order.

> **Mutex/2PL is *sufficient*, not *necessary*, for serializability.** And critically, locking is the **opposite of "smooth"** — it *trades throughput/parallelism for correctness*. Workers block; long-held locks (required by 2PL) increase waiting; lock-order cycles deadlock. The art of concurrency control is killing *only* the bad interleavings while giving up as little concurrency as possible — which is exactly why the lock-free schemes exist.

### Atomicity across machines (a *building block* of distributed serializability)
- **Two-Phase Commit (2PC)** — `06`. Gives **atomicity** (all-or-nothing) for a transaction spanning multiple *partitions*; the per-participant lock gives **isolation**. Together → serializability across machines. **Blocks** on coordinator failure (see `06/README` and `CONSENSUS.md §2PC≠consensus`).
- **Paxos Commit** (Gray & Lamport, 2006) — the non-blocking fix: run the commit *decision itself* through consensus (`05`), so no single coordinator failure can strand the participants.

---

## 7. Atomicity vs isolation vs consistency — don't conflate the ACID letters

A frequent conflation: *"serializability = all-or-nothing."* No — that's **atomicity**.

| ACID letter | Concern | Failure it prevents | Mechanism (in `06`) |
|---|---|---|---|
| **A**tomicity | one transaction vs **failure** | half-done transaction (crash/abort mid-way) | the **commit protocol** (2PC: all YES → COMMIT; any NO/crash → ABORT) |
| **I**solation | many transactions vs **each other** | anomalies from concurrent interleaving (lost update, etc.) | the **`prepared` lock** (2PL) |
| **C**onsistency | app-level invariants (e.g. money conserved) | broken invariants | app logic + A/I |
| **D**urability | committed state vs crash | losing a committed result | **fsync** / stable storage (`persist()` in `06`, `05`) |

The two independence tests:
- **Serializability is a problem even when nothing ever fails** — remove all crashes and the lost update *still* happens, purely from concurrency. That's isolation, not atomicity.
- **Atomicity is a problem even with one transaction and no concurrency** — a single transfer that debits A then crashes before crediting B has vanished money. That's atomicity, not isolation.

They *relate*, though as a **modeling convention**, not a theorem: serializability is evaluated over the **committed projection** of a schedule, so we assume each transaction is an all-or-nothing *unit* to be ordered. The property that actually governs what aborts may expose is **recoverability** (and its stronger cousins — avoiding cascading aborts, strictness), which is what Strict/Rigorous 2PL provides. So the isolation story *presupposes* atomicity/recoverability to even be stated — even though atomicity and isolation are, strictly, orthogonal ACID dimensions.

---

## 8. Replication vs partitioning — why `05` and `06` are different problems

One more orthogonality, because it explains the differing fault models:

| | **Replication** (`03`, `05`) | **Partitioning / sharding** (`06`) |
|---|---|---|
| what each node holds | the **same** data (a copy) | **different** data (a disjoint shard) |
| goal | **fault tolerance** — survive a node dying | **atomicity** of an op spanning several shards |
| nodes | interchangeable | each essential to its own slice |
| one node dying | **tolerated** (others have the data) | **fatal to atomicity** (its shard is unique *and* its vote is required) |
| decision rule | **majority** (quorum) | **unanimity** (every participant) |

This is why Raft tolerates a minority of crashes (survivors still hold replicas) while 2PC tolerates *no* participant loss (each holds an irreplaceable shard). Real systems layer them: **each shard is a Raft/Paxos group (replication), and cross-shard transactions run 2PC over the shard-leaders (atomic commit)** — which, with the coordinator itself replicated, is Paxos Commit. Spanner and CockroachDB are exactly this composition.

---

## 9. One-screen summary

- **Execution axis** (how work runs): *sequential* (one at a time) → *concurrent* (interleaved, one instant at a time) → *parallel* (truly simultaneous). Concurrency is structure; parallelism is execution; concurrency ⇏ parallelism but parallelism ⇒ concurrency.
- **Consistency axis** (how shared state appears):
  - **Linearizability** — one object, single ops, single-copy illusion, respects real time. *(mutex / quorums / consensus)*
  - **Serializability** — transactions over many objects, single-worker illusion, *some* serial-equivalent order, ignores real time. *(2PL / MVCC / OCC)*
  - **Strict serializability** — the two combined: transactions **and** real-time order. *(the gold standard)*
- **The pot proves they differ:** a linearizable register still suffers a lost update, because linearizability governs single operations, not transactions.
- **Mechanism ≠ guarantee:** a mutex/2PL is *one sufficient* way, trading concurrency for correctness; distributed settings replace the mutex with quorums/consensus.
- **Don't conflate ACID:** atomicity (vs failure) ≠ isolation/serializability (vs concurrency); atomicity is a prerequisite for the latter.
- **Replication ≠ partitioning:** same-data-many-copies (fault tolerance, majority) vs different-data-one-copy (atomic commit, unanimity). Real systems do both.

---

## 10. References

Verified, load-bearing sources for the above.

**Execution / concurrency vs parallelism**
- Rob Pike, *Concurrency Is Not Parallelism* (talk, 2012). The "dealing with vs doing" framing. <https://go.dev/blog/waza-talk>
- M. Herlihy & N. Shavit, *The Art of Multiprocessor Programming*, 2nd ed., 2020. Concurrency, linearizability, lock-free structures.

**Linearizability & sequential consistency**
- M. P. Herlihy & J. M. Wing, "Linearizability: A Correctness Condition for Concurrent Objects," *ACM TOPLAS* 12(3), 1990. The original definition.
- L. Lamport, "How to Make a Multiprocessor Computer That Correctly Executes Multiprocess Programs," *IEEE Trans. Computers* C-28(9), 1979. Sequential consistency.

**Serializability & transactions**
- C. H. Papadimitriou, "The Serializability of Concurrent Database Updates," *JACM* 26(4), 1979. Serializability & strictness; conflict-serializability theory.
- P. A. Bernstein, V. Hadzilacos, N. Goodman, *Concurrency Control and Recovery in Database Systems*, Addison-Wesley, 1987. The canonical text (freely available online). 2PL, serializability graphs.
- J. Gray & A. Reuter, *Transaction Processing: Concepts and Techniques*, 1993. ACID, locking, recovery.
- A. Adya, *Weak Consistency: A Generalized Theory and Optimistic Implementations for Distributed Transactions*, PhD thesis, MIT, 1999. Rigorous isolation-level definitions.

**Distributed consistency, surveys & maps**
- P. Viotti & M. Vukolić, "Consistency in Non-Transactional Distributed Storage Systems," *ACM Computing Surveys* 49(1), 2016. A unifying survey of ~50 models.
- P. Bailis et al., "Highly Available Transactions: Virtues and Limitations," *VLDB* 2014. How isolation/consistency interact with availability.
- Jepsen, *Consistency Models* (Kyle Kingsbury). Interactive lattice of models & their relationships. <https://jepsen.io/consistency>
- M. Kleppmann, *Designing Data-Intensive Applications*, O'Reilly, 2017. Ch. 7 (transactions), Ch. 9 (consistency & consensus) — the best modern synthesis.

**Atomic commit & the distributed tie-ins**
- J. Gray & L. Lamport, "Consensus on Transaction Commit," *ACM TODS* 31(1), 2006. Paxos Commit — the non-blocking fix to 2PC.
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed Programming*, 2nd ed., Springer, 2011. Register hierarchy (safe/regular/atomic), NBAC, consensus. **(CCGR — the reference textbook for this track.)**
- Companion map: [`05-raft/CONSENSUS.md`](05-raft/CONSENSUS.md) — FLP, timing models, quorums, crash vs Byzantine, and why 2PC ≠ consensus.
