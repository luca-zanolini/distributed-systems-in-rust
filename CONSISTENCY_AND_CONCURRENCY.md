# Consistency and Concurrency — Lecture Notes

*Part of **Concurrent and Distributed Systems in Rust** ([course home](README.md)). Companion
theory map: [CONSENSUS.md](07-raft/CONSENSUS.md) (agreement and impossibility). These notes are
referenced from Modules 02, 04, 07, and 08.*

**Abstract.** These notes organize a cluster of frequently conflated concepts along two
orthogonal axes: the **execution axis** — how work is scheduled (sequential, concurrent,
parallel) — and the **consistency axis** — how shared state is permitted to appear to its users
(linearizability and sequential consistency for single objects; serializability and strict
serializability for transactions). We give the standard formal definitions (histories in the
sense of Herlihy–Wing; schedules and serializability theory in the sense of
Bernstein–Hadzilacos–Goodman), the classical separating examples, the mechanisms that realize
each guarantee, and the relationships — including the frequently misstated ones — among the
ACID properties. Throughout, *mechanism* (mutexes, two-phase locking, multiversioning, quorums,
consensus) is kept distinct from *guarantee* (the consistency condition satisfied).

---

## Contents

0. Two orthogonal axes
1. The execution axis: sequential, concurrent, parallel
2. Shared state and mutual exclusion
3. Single-object consistency: histories, linearizability, sequential consistency
4. Transactions: ACID, schedules, serializability
5. Strict serializability
6. Mechanisms: how each guarantee is achieved
7. The ACID properties disentangled
8. Replication versus partitioning
9. Summary
10. References

---

## 0. Two orthogonal axes

Most confusion in this area stems from mixing two independent questions:

| Axis | Question | Terms | Property of |
|---|---|---|---|
| **Execution** | how is work scheduled onto processors and time? | sequential · concurrent · parallel | the program and hardware |
| **Consistency** | how may shared state appear to observers? | linearizability · sequential consistency · serializability · strict serializability | the correctness specification |

The axes are orthogonal: a concurrent (even parallel) execution can be linearizable — that is an
achievement of synchronization — and a sequential execution satisfies every consistency
condition trivially. The two meet at **shared mutable state**: the moment concurrent execution
touches common data, a consistency condition must be chosen and enforced by some coordination
mechanism. Absent sharing, the consistency axis is vacuous.

## 1. The execution axis: sequential, concurrent, parallel

- **Sequential** execution performs one task to completion before beginning the next.
- **Concurrent** execution *interleaves* tasks: several are in progress at once, though a single
  processor executes only one at any instant. Concurrency is a property of program
  *structure* — the decomposition into tasks whose steps may legally interleave.
- **Parallel** execution runs tasks *simultaneously* on multiple processors. Parallelism is a
  property of *execution*, and requires hardware.

Pike's formulation is standard: *concurrency is about dealing with many things at once;
parallelism is about doing many things at once.* Two consequences: concurrency does not require
parallelism (a single core interleaving threads is concurrent, not parallel), and parallelism
presupposes concurrency (only independently structured tasks can run simultaneously — the same
concurrent program becomes parallel when given more cores, without structural change).

| | in progress | at one instant | requires multiple cores |
|---|---|---|---|
| sequential | 1 | 1 | no |
| concurrent | many | 1 (interleaved) | no |
| parallel | many | many | yes |

Where the danger lies: a purely sequential program cannot race. Both concurrency (interleaving
at an inopportune point) and parallelism (true simultaneity) can corrupt shared state; hence
the consistency axis. In this course the contrast is embodied by Module 07's Raft node (timer
thread + connection handlers sharing state — concurrent, synchronized) versus Module 08's 2PC
participant (a single sequential loop — no synchronization required, by construction).

## 2. Shared state and mutual exclusion

A **mutex** enforces mutual exclusion: at most one thread holds it at a time; a thread
requesting a held mutex blocks until release. In Rust, `Mutex::lock` returns a guard whose
scope *is* the critical section — release is automatic at scope exit.

Two classical hazards. (i) *Long critical sections serialize the system* — whence the
engineering rule, recurring in Module 07, never to hold a lock across network I/O. (ii) *Cyclic
acquisition deadlocks*: thread A holds `L₁` and requests `L₂` while B holds `L₂` and requests
`L₁`. (A non-reentrant mutex yields the one-thread degenerate case: re-acquiring a held lock
self-deadlocks.)

The essential framing for everything below:

> **A mutex is a mechanism, not a consistency condition.** Serializing all accesses to one
> object through a lock is *one way* to make that object linearizable on one machine. It is not
> necessary (single-threaded event loops, lock-free algorithms, and multiversion schemes avoid
> it), and it is not by itself sufficient for transaction-level guarantees (§4: per-operation
> locking does not prevent the lost update).

## 3. Single-object consistency: histories, linearizability, sequential consistency

### 3.1 Histories

Following Herlihy and Wing (1990): operations on shared objects are modeled by two events, an
**invocation** and a matching **response**. A **history** `H` is a finite sequence of such
events from several processes on several objects. An operation is **complete** in `H` if its
response appears. `H` induces the **real-time partial order** `<_H` on complete operations:

> `op₁ <_H op₂`  iff  the response of `op₁` precedes the invocation of `op₂` in `H`.

Operations unrelated by `<_H` are **concurrent**. A history is **sequential** if each
invocation is immediately followed by its matching response (no overlap); a sequential history
is **legal** if each object's subsequence conforms to that object's sequential specification
(for a register: every read returns the most recently written value).

### 3.2 Linearizability

**Definition (Herlihy–Wing).** A history `H` is **linearizable** if, after completing or
discarding its pending operations, there exists a legal sequential history `S` over the same
operations such that `<_H ⊆ <_S`.

Equivalently: every operation appears to take effect atomically at a single **linearization
point** between its invocation and its response, and the resulting order extends real time. The
operational content: once an operation has completed, every subsequently issued operation
observes its effect — no observer sees the object "go back in time." An object specification
whose implementations must satisfy this is called **atomic** in CCGR's vocabulary.

**Locality (composability).** *Theorem (Herlihy–Wing).* `H` is linearizable iff `H|x` is
linearizable for every object `x`. Linearizability is thus a *local* property: independently
linearizable objects compose into a linearizable system. This is a principal reason it is the
default correctness condition for concurrent objects and replicated services.

### 3.3 Sequential consistency

**Definition (Lamport 1979).** A history is **sequentially consistent** if there exists a legal
sequential history over the same operations that preserves each *process's* program order
(but not necessarily `<_H` across processes).

Sequential consistency is a **global, multi-object** condition — a single total order over all
operations on all objects. It differs from linearizability in two ways, both essential:

1. **Real time is dropped.** A read may lawfully return a stale value even though a newer write
   completed earlier in real time, provided *some* legal total order explains all observations.
2. **It is not local.** A system of individually sequentially consistent objects need not be
   sequentially consistent as a whole; the composability theorem of §3.2 fails.

The slogan *linearizability = sequential consistency + real time* is therefore true as far as it
goes but incomplete: locality is the second, independent distinction.

### 3.4 The register hierarchy (CCGR)

For single registers, CCGR grades the guarantee under concurrency (safe and regular are defined
for a single writer; the multi-writer generalization is standard only for atomic):

- **Safe** — a read not concurrent with any write returns the last written value; a read
  concurrent with a write may return *any* value of the domain.
- **Regular** — a concurrent read returns either the last written or a concurrently written
  value (no fabricated values); sequential reads overlapping one write may still observe
  new-then-old. Module 04's majority-voting register is regular.
- **Atomic** — linearizable; the new-then-old inversion is excluded. The upgrade from regular
  is the read-impose (write-back) step (Module 04 §6; ABD).

### 3.5 Intuition

A useful mental model: a linearizable object behaves like **a single physical copy touched by
one operation at a time, honestly with respect to the wall clock**. If a whiteboard's "soup of
the day" is rewritten from *tomato* to *leek* and the write completes at 12:00:06, a reader
glancing at 12:00:07 must see *leek*; a reader glancing *during* the rewrite may see either, but
all observers' accounts must be reconcilable with a single instant at which the change took
effect. A replicated object that lets a completed write remain invisible to a later read — two
whiteboards, lazily synchronized — is precisely a non-linearizable one, and making replicated
objects linearizable is what quorums (Module 04) and consensus (Module 07) are *for*.

## 4. Transactions: ACID, schedules, serializability

### 4.1 Transactions and ACID

**Definition.** Fix a set of data items. A **transaction** `Tᵢ` is a finite sequence of
operations `rᵢ(x)` (read item `x`) and `wᵢ(x)` (write item `x`), terminated by exactly one of
`cᵢ` (commit) or `aᵢ` (abort).

The classical contract for transactional systems (Härder & Reuter 1983):

- **Atomicity.** For every transaction, either all of its writes are installed in the database
  state, or none are — in particular, an aborted or crashed transaction leaves no partial
  effects (undo/rollback under failure).
- **Consistency.** Each transaction, executed alone from a state satisfying the application's
  invariants, yields a state satisfying them. (This property constrains the *application's*
  transactions; the system supplies A, I, and D so that C is preserved under concurrency and
  failure.)
- **Isolation.** The concurrent execution of committed transactions is equivalent to some
  serial execution — formalized as serializability (§4.3).
- **Durability.** The effects of a committed transaction survive subsequent crashes (stable
  storage; write-ahead logging).

Note the different quantifiers: atomicity and durability are per-transaction properties about
*failure*; isolation is a property of the *set* of concurrently executing transactions about
*interference*. §7 returns to their relationships.

### 4.2 Schedules

**Definition.** A **history** (or **schedule**) `H` over transactions `T₁, …, Tₙ` is an
interleaving of all their operations that preserves the order of operations within each `Tᵢ`.
The **committed projection** `C(H)` is `H` restricted to the transactions that commit in `H`.
`H` is **serial** if the transactions do not interleave: all operations of one precede all
operations of the next, in some order.

**Definition (conflict).** Two operations **conflict** if they belong to different
transactions, access the same item, and at least one is a write (patterns `r/w`, `w/r`,
`w/w`).

### 4.3 Serializability

**Definition.** `H` is **serializable** if `C(H)` is equivalent to some serial history over the
committed transactions. The notion of *equivalence* selects the variant:

- **Conflict equivalence:** same operations, and every pair of conflicting operations ordered
  the same way. `H` is **conflict-serializable** iff its **serialization graph** `SG(H)` —
  nodes the committed transactions, an edge `Tᵢ → Tⱼ` whenever an operation of `Tᵢ` precedes
  and conflicts with one of `Tⱼ` — is **acyclic** (the serializability theorem). Testable in
  polynomial time; this is the notion practical systems enforce.
- **View equivalence:** same reads-from relation and same final writes. **View
  serializability** is strictly weaker (it admits schedules with blind writes that conflict
  serializability rejects) and deciding it is NP-complete (Papadimitriou 1979).

Thus conflict-serializable ⊊ view-serializable ⊊ final-state-serializable; "serializable" in
systems practice means *conflict*-serializable, and the acyclicity test characterizes exactly
that variant.

Three points deserve emphasis:

1. **Serializable ≠ serial.** Operations really do interleave; the requirement is only that the
   *outcome* equal that of some non-interleaved order. The property constrains the equivalence
   class of the execution, not the execution itself.
2. **"Some" order, not a specific one.** Any serial order over the committed transactions
   suffices — including one that disagrees with real time (§5 adds that constraint).
3. **It is a property of the whole set.** Individual transactions are not "serializable";
   schedules are.

### 4.4 The canonical anomaly: the lost update

Let item `x = 100`, and let `T₁` and `T₂` each read `x` and write back a decremented value
(`T₁` subtracts 10; `T₂` subtracts 20):

```
H:   r₁(x)=100   r₂(x)=100   w₁(x:=90)   w₂(x:=80)   c₁   c₂
```

Both serial orders yield `x = 70`; `H` yields `x = 80`, and `SG(H)` contains the cycle
`T₁ → T₂` (from `r₁` before `w₂`) and `T₂ → T₁` (from `r₂` before `w₁`). `H` is not
serializable: `T₂`'s update overwrites `T₁`'s without observing it — the **lost update**.

### 4.5 Linearizable yet not serializable

The same schedule, read at the granularity of *individual operations on the register `x`*, is
perfectly **linearizable**: the sequential history `r₁(x), r₂(x), w₁(x), w₂(x)` is legal for a
register (both reads correctly return 100, the last written value at their linearization
points) and extends real time. No single operation misbehaved.

The defect is at the *transaction* granularity: each read–write pair was intended as one
indivisible read-modify-write, and linearizability makes no promise about **groups** of
operations. This is the cleanest separation of the two conditions on the consistency axis:

> **Per-operation guarantees do not compose into per-transaction guarantees.** A system of
> linearizable objects can still exhibit non-serializable transactional behavior; conversely,
> serializability makes no real-time promise about individual operations. The two conditions
> answer different questions (one object/one operation vs. many objects/grouped operations).

## 5. Strict serializability

**Definition.** A schedule is **strictly serializable** if it is serializable via a serial
order that additionally respects the real-time partial order on transactions: if `Tᵢ`
committed before `Tⱼ` began, then `Tᵢ` precedes `Tⱼ` in the equivalent serial order.
(For overlapping transactions, either order remains admissible.)

Strict serializability is exactly *linearizability lifted from single operations to
transactions* — equivalently, serializability plus real time. The taxonomy:

| Condition | Unit | Scope | Real time | Local? |
|---|---|---|---|---|
| sequential consistency | operation | many objects (global order) | no | no |
| **linearizability** | operation | per object | yes | yes |
| **serializability** | transaction | many objects | no | — |
| **strict serializability** | transaction | many objects | yes | — |

Without the real-time constraint, plain serializability admits behavior in which a client
commits a transaction, then begins a second one that is ordered *before* the first — reading
state that "forgets" its own committed write. Strict serializability excludes such causal
reversals.

**Systems.** Google Spanner provides strict serializability ("external consistency") for
read–write transactions, using TrueTime bounded-uncertainty clocks and commit-wait;
FoundationDB provides it via a central sequencer's versionstamps. Precision matters in this
zoo: **CockroachDB** provides serializable isolation with single-key linearizability but, using
hybrid logical clocks with a bounded offset rather than TrueTime, does **not** guarantee strict
serializability in general (documented stale-read/causal-reverse anomalies within the clock
offset window). **Oracle**'s `SERIALIZABLE` isolation level is in fact snapshot isolation, which
is weaker than serializability (it admits write skew).

## 6. Mechanisms: how each guarantee is achieved

Consistency conditions are specifications; the following are implementation techniques, each
sufficient for its target guarantee and none uniquely necessary.

**Single-object linearizability.**
- *One machine:* a mutex serializing operations; a single-threaded event loop; lock-free
  algorithms based on atomic read-modify-write instructions (CAS).
- *Replicated:* there is no shared memory to lock — the guarantee must be engineered from
  quorum intersection (ABD-style majority reads/writes with write-back; Module 04) or from
  consensus establishing a total operation order (a replicated state machine; Module 07).

**Transaction serializability.**
- **Two-phase locking (2PL).** Each transaction acquires all its locks before releasing any
  (a growing phase, then a shrinking phase). *Theorem:* 2PL schedules are
  conflict-serializable — the peak at which a transaction holds all its locks supplies its
  position in a serial order. Basic 2PL already excludes the lost update (§4.4: each
  transaction would have to upgrade its read lock on `x` to a write lock while the other still
  holds its read lock — the attempt blocks, and if both attempt it, the resulting upgrade
  deadlock is broken by aborting one; in neither case does the non-serializable interleaving
  commit). **Strict 2PL** holds all *exclusive* locks to
  commit/abort, adding recoverability and precluding cascading aborts (§7); **rigorous 2PL**
  holds all locks to commit/abort. These stronger variants change the *recovery* properties,
  not serializability, which basic 2PL already provides. Costs: blocking, and deadlock
  (resolved by ordering, timeout, or victim abort). Module 08's `prepared` state is strict 2PL
  across machines.
- **Multiversion concurrency control (MVCC).** Maintain versions; give each reader a
  consistent snapshot. Snapshot isolation alone is *not* serializable (write skew);
  serializable snapshot isolation (SSI — Cahill et al.; PostgreSQL's `SERIALIZABLE`) adds
  conflict detection and aborts to restore it. (Spanner is not a pure example of this family:
  it snapshots read-only transactions but uses 2PL for read–write transactions.)
- **Optimistic concurrency control (OCC).** Execute without locks; validate the read/write
  sets at commit; abort and retry on conflict (Kung & Robinson 1981).
- **Timestamp ordering.** Assign start timestamps and enforce conflict order accordingly.

The engineering content of all four is the same trade: give up some concurrency (blocking, or
aborts and retries) to exclude exactly the non-serializable interleavings — pessimistically in
advance (locks), or optimistically after the fact (validation).

**Atomic commitment across nodes** — atomicity, a different letter of ACID — is Module 08's
2PC, with Paxos Commit (Gray & Lamport 2006) its non-blocking, consensus-based repair.

## 7. The ACID properties disentangled

A frequent conflation equates serializability with all-or-nothing behavior. The properties are
orthogonal, with precise points of contact:

| Property | Concern | Adversary | Mechanism (in this course) |
|---|---|---|---|
| atomicity | one transaction's effects | **failure** (crash, abort) | commit protocols (Module 08); undo logging |
| isolation / serializability | many transactions' interference | **concurrency** | 2PL / MVCC / OCC (§6) |
| durability | committed effects | crash | stable storage, WAL, fsync (Modules 01, 07, 08) |
| consistency (C) | application invariants | — | the application, given A/I/D |

Two independence tests. *Isolation is not about failure:* in a crash-free, abort-free
execution, the lost update of §4.4 still occurs — purely a concurrency phenomenon. *Atomicity
is not about concurrency:* a solitary transfer that debits `A`, then crashes before crediting
`B`, violates atomicity with no second transaction in sight.

Their point of contact is a modeling convention plus a family of *recovery* properties.
Serializability is defined over the **committed projection** `C(H)` (§4.2) — the theory
evaluates isolation as if each committed transaction were an indivisible unit, which is what
atomicity supplies. The properties actually governing the interaction of aborts with
concurrent readers are, in increasing strength: **recoverability** (no transaction commits
having read from an uncommitted one), **avoidance of cascading aborts** (read only committed
data), and **strictness** (no reading or overwriting of uncommitted data). Strict/rigorous 2PL
provide the stronger ones; this — not serializability — is what "holding locks to commit" buys
(§6).

## 8. Replication versus partitioning

Orthogonal to everything above is *where the data lives* — and the distinction explains why
Modules 04/07 and Module 08 have opposite fault profiles:

| | **Replication** (04, 07) | **Partitioning** (08) |
|---|---|---|
| each node holds | the same data (a copy) | different data (a shard) |
| purpose | fault tolerance, read scale | capacity; multi-item transactions across nodes |
| nodes are | interchangeable | individually irreplaceable |
| decision rule | majority quorum | unanimity (atomic commitment) |
| minority crash | tolerated | forces abort or blocks |

Production architectures compose the two: partition the data into shards; replicate each shard
as a consensus group (Raft/Paxos — Module 07); run atomic commitment (2PC — Module 08) across
shard leaders for multi-shard transactions, with the consensus layer making both participants
and coordinator durable. Spanner and CockroachDB instantiate this composition. In terms of the
taxonomy of §5: per-shard consensus supplies linearizable single-shard operations; cross-shard
2PC with 2PL supplies (strict) serializability for transactions, subject to the clock caveats
noted there.

## 9. Summary

- Two axes: **execution** (sequential / concurrent / parallel — structure vs. simultaneity)
  and **consistency** (what observers may see), meeting at shared mutable state.
- **Linearizability**: per-operation, per-object, real-time-respecting, *local/composable*.
  **Sequential consistency**: global order, program order only, not composable.
- **Serializability**: per-transaction, equivalence of `C(H)` to some serial order;
  conflict-serializability (acyclic `SG(H)`) is the enforceable variant; view-serializability
  is NP-complete. **Strict serializability** adds real time — linearizability at transaction
  granularity.
- A **linearizable object does not give serializable transactions** (the lost update runs on a
  perfectly linearizable register); per-operation and per-group guarantees are different
  contracts.
- **Mechanism ≠ guarantee**: mutexes and basic 2PL are sufficient, not necessary; strict 2PL
  buys recoverability, not serializability; going distributed replaces the mutex with quorums
  or consensus.
- **ACID**: atomicity (failure) and isolation (concurrency) are orthogonal; serializability is
  evaluated over the committed projection, with recoverability/strictness governing aborts.
- **Replication ≠ partitioning**: copies vs. shards; majority vs. unanimity — composed, they
  form the standard modern architecture (consensus within shards, atomic commitment across).

## 10. References

**Concurrency and parallelism**
- R. Pike, *Concurrency Is Not Parallelism*, talk, 2012. <https://go.dev/blog/waza-talk>
- M. Herlihy, N. Shavit, *The Art of Multiprocessor Programming*, 2nd ed., Morgan Kaufmann,
  2020.

**Single-object consistency**
- M. P. Herlihy, J. M. Wing, *Linearizability: A Correctness Condition for Concurrent
  Objects*, ACM TOPLAS 12(3), 1990. (Histories, linearizability, locality.)
- L. Lamport, *How to Make a Multiprocessor Computer That Correctly Executes Multiprocess
  Programs*, IEEE Trans. Computers C-28(9), 1979. (Sequential consistency.)
- H. Attiya, A. Bar-Noy, D. Dolev, *Sharing Memory Robustly in Message-Passing Systems*,
  JACM 42(1), 1995. (ABD.)
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. (Register hierarchy, Ch. 4.)

**Transactions and serializability**
- C. H. Papadimitriou, *The Serializability of Concurrent Database Updates*, JACM 26(4), 1979.
  (View vs. conflict serializability; NP-completeness.)
- P. Bernstein, V. Hadzilacos, N. Goodman, *Concurrency Control and Recovery in Database
  Systems*, Addison-Wesley, 1987. (Serializability theory, 2PL, recoverability hierarchy;
  freely available online.)
- T. Härder, A. Reuter, *Principles of Transaction-Oriented Database Recovery*, ACM Computing
  Surveys 15(4), 1983. (ACID.)
- J. Gray, A. Reuter, *Transaction Processing: Concepts and Techniques*, Morgan Kaufmann, 1993.
- H. T. Kung, J. T. Robinson, *On Optimistic Methods for Concurrency Control*, ACM TODS 6(2),
  1981.
- M. Cahill, U. Röhm, A. Fekete, *Serializable Isolation for Snapshot Databases*, SIGMOD 2008.
  (SSI.)
- A. Adya, *Weak Consistency: A Generalized Theory and Optimistic Implementations for
  Distributed Transactions*, PhD thesis, MIT, 1999. (Isolation levels, rigorously.)

**Distributed consistency: surveys and systems**
- P. Viotti, M. Vukolić, *Consistency in Non-Transactional Distributed Storage Systems*,
  ACM Computing Surveys 49(1), 2016.
- P. Bailis et al., *Highly Available Transactions: Virtues and Limitations*, VLDB 2014.
- K. Kingsbury, *Consistency Models*, Jepsen. <https://jepsen.io/consistency>
- M. Kleppmann, *Designing Data-Intensive Applications*, O'Reilly, 2017. (Chs. 7, 9.)
- J. C. Corbett et al., *Spanner: Google's Globally-Distributed Database*, OSDI 2012.
- J. Gray, L. Lamport, *Consensus on Transaction Commit*, ACM TODS 31(1), 2006.

---
*[Course home](README.md) · Companion: [CONSENSUS.md](07-raft/CONSENSUS.md)*
