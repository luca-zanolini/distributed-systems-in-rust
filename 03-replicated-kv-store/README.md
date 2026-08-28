# 03 — Replicated Key-Value Store

`02`'s networked store, now **replicated across several nodes** so it survives crashes. The data
lives on multiple machines; a write is propagated to a **quorum** of them, and a read consults a
**quorum** and returns the freshest copy. A crashed node that comes back **catches up** from its
peers. Step by step this grows into a genuine distributed-systems object — the **(1, N)
Majority-Voting register** of Cachin, Guerraoui & Rodrigues (*Introduction to Reliable and Secure
Distributed Programming*, 2nd ed., 2011 — **"CCGR"**), §4.2.3.

This is the capstone of the "storage" arc of the repo: `01` was the **K** and **V**, `02` put it on
the **network**, and `03` makes it **fault-tolerant**. What it *cannot* yet do points straight at
the next subject: **consensus**.

---

## Theory — replication, quorums, and the register

### 1. Why replicate

A single node (project `02`) is a **single point of failure**: if it dies, the data and the service
die with it. **Replication** keeps copies of the state on several nodes so that (a) the data
survives the loss of some nodes, and (b) more nodes can serve the load. It is the foundation of
every fault-tolerant store.

But replication is not free — the moment there is more than one copy, a cascade of hard questions
opens up, and *each* is a distributed-systems topic:

- **Consistency** — when do the copies agree? What does a read return if they disagree?
- **Availability under failure** — if a copy is down or unreachable, do we still accept writes?
- **Recovery** — a crashed copy restarts having lost everything; how does it become current again?
- **Ordering / agreement** — if copies can accept operations independently, who decides the order?

`03` walks straight up this staircase, one milestone per step (see **§4, the evolution**).

### 2. Where this shows up in practice

| Style | Real systems |
|---|---|
| **Primary/backup** (leader replicates to followers) | PostgreSQL streaming replication, MySQL, Redis replication |
| **Quorum replication** (`R + W > N`) | **Dynamo**, Cassandra, Riak |
| **Consensus-backed** (strongly consistent) | **etcd**/ZooKeeper (Raft/ZAB), Spanner (Paxos), CockroachDB/TiKV |
| **Crash-recovery + catch-up** | Raft snapshot install, Kafka replica bootstrap, Postgres WAL shipping |

The **CAP theorem** is the fork every one of these must pick a side of; `03` (like etcd/Spanner)
chooses **C over A** — it refuses to serve without a quorum rather than risk a stale answer.

### 3. What it precisely is — the register

A replicated key-value store is, formally, a set of **read/write registers** — one per key — the
shared-memory abstraction CCGR develops in **Chapter 4**, emulated over message passing among
crash-prone nodes. A register has two operations: **read** (`get`) and **write** (`set`).

`03` implements the **(1, N) regular register** by the **"Majority Voting"** algorithm (CCGR §4.2.3):

- **(1, N)** — **one writer** (the primary), N readers. One writer is what lets us use a plain
  integer timestamp with no ties; multiple writers would need vector clocks (see limits).
- **Majority Voting** — every value carries a **timestamp**; a **write** goes to a majority, and a
  **read** collects a majority and returns the value with the **highest timestamp**.

The correctness rests on one fact you can prove in a sentence — **any two majorities of `N` nodes
share at least one node** (`⌊N/2⌋+1` + `⌊N/2⌋+1 > N`). So a read-quorum always intersects the
write-quorum of the last completed write, and that shared node carries the latest value; the
highest timestamp selects it. This is Dynamo's `R + W > N` in its symmetric, majority/majority form.

> 🎓 **Teaching idea.** The two classic register algorithms are just two points on the `R+W>N`
> line: **Read-One Write-All** (`W=N, R=1`, CCGR §4.2.2 — cheap reads, fragile writes) and
> **Majority Voting** (`W=R=⌊N/2⌋+1`, §4.2.3 — tolerant of a crashed minority on both sides). `03`
> starts life near the first and ends at the second (see §4).

### 4. How this project evolved — one problem at a time

The most useful way to read `03` is as a chain of *"we built X, then noticed problem Y, which
forced Z."* Every milestone fixes the wound the previous one exposed.

| # | We built… | …which exposed |
|---|---|---|
| **M1** | **one backup, async replication** — primary forwards each write to a backup | primary replies `OK` *before* the backup confirms → they can **silently diverge** |
| **M2** | **synchronous replication** — primary waits for the backup's ack, else `ERR` | now if the backup is **down, every write fails**: we traded availability for consistency (**CAP, felt firsthand**). Also: the value is applied locally *before* the ack → **no rollback** |
| **M3** | **fan-out to N backups** — forward to all, wait for all | waiting for **all** means **any one** dead backup blocks writes — more replicas made us *less* available (ironic!) |
| **M4** | **quorum writes** — succeed on a **majority** of acks | a write reaches only a majority → a **stale minority** always exists; a quorum-*failed* write is still applied locally (no rollback) |
| **M5** | **catch-up / anti-entropy** — a restarted node pulls a snapshot (`dump`) and converges | reads still come from **one** node, so a just-recovered or stale node can serve a **stale read**; the snapshot is point-in-time (a small race) |
| **M6** | **quorum reads** — version values, read a **majority**, take the highest timestamp | the register is complete — but **who is the primary?** what if it **crashes**? who orders **concurrent** writes? → **consensus** |

Read top to bottom, that is the entire logic of replication: *availability vs consistency*
(M1–M2), *fault tolerance via quorums* (M3–M4), *recovery* (M5), and *consistent reads* (M6). And
the last cell is the doorway out of this project.

> 🎓 **Teaching idea — three experiments that make it visceral.**
> 1. **CAP (M2):** kill the backup, issue a synchronous write → it *fails*. The system is
>    consistent but unavailable. Async (M1) would have said `OK` and diverged.
> 2. **Quorum tolerance (M4):** 3 nodes, kill one → writes still succeed (2/3); kill two → `ERR no
>    quorum`. The cluster shrugs off a *minority* but never a *majority*.
> 3. **Read quorum masks staleness (M6):** restart a node **empty**; a `get` on it still returns
>    the latest value (it polls a majority), while `readts` on it shows its own copy is empty.

### 5. The open questions `03` leaves — and why they need consensus

Even as a finished register, `03` cannot answer several questions, and **each is a consensus
problem**:

- **Who is the primary, and who takes over if it crashes?** Ours is fixed by hand. Automatic
  **failover** requires the nodes to *agree* on a leader → **leader election** (CCGR §2.6, eventual
  leader `Ω`; the heart of **Raft**).
- **How do we order concurrent writes from different clients/writers?** One writer sidesteps this;
  many writers need agreement on a **total order** of operations → **total-order broadcast /
  replicated state machine** (CCGR Ch. 6), which is *equivalent* to consensus.
- **How do we make a write all-or-nothing?** A quorum-failed write is applied-and-not-rolled-back.
  Making it atomic is **(non-blocking) atomic commit / 2PC** (CCGR Ch. 6).
- **Can we do all this when nodes may *lie*, not just crash?** That is **Byzantine** fault
  tolerance and the `>2/3` supermajority — a different quorum, a harder problem.
- **When is agreement even *possible*?** Under full asynchrony, **FLP** says deterministic
  consensus is impossible with even one crash; you need partial synchrony, randomization, or
  failure detectors.

These are the subject of the **future consensus project(s)** (`04+`), where a dedicated capstone
README will map the whole landscape (impossibility, timing models, majority vs `2/3`) — see the
repo's standing plan. `03` is exactly the rung of the ladder where you *feel* why consensus is
needed before you build it.

### 6. In the CCGR framework (the book's language)

- **The object:** the **(1, N) regular register**, implemented by **Majority Voting** (§4.2.3);
  the extreme it started from is **Read-One Write-All** (§4.2.2). Quorums are §2.7.3.
- **Failure models:** we assume **crash-stop** (§2.2.2) for the running cluster, and step into
  **crash-recovery** (§2.2.4) at M5 — a node crashes, loses volatile state (**amnesia**), and
  recovers by **state transfer** (our `dump`/catch-up) instead of stable storage.
- **Links:** node-to-node messaging rides **perfect point-to-point links** (Module 2.3), which TCP
  provides (as in `02`).
- **Safety vs liveness (§2.1.3):** "a read never returns a value older than the latest completed
  write" is safety; "a write to a reachable majority eventually completes" is liveness. Choosing to
  **refuse without a quorum** keeps safety at the cost of liveness under partition — the CAP choice.
- **Honest gap to *atomic*:** Majority-Voting gives a **regular** register. Full **atomic
  (linearizable)** reads need the reader to *write the winning value back* to a majority before
  returning (CCGR §4.3, "Read-Impose Write-Majority") — we do **not** do that write-back, so
  concurrent reads during an in-flight write could momentarily disagree.

### 7. How the code reflects the theory — and where it deliberately stops

| Theory | In this code |
|---|---|
| replicated **register**, versioned | `Store::map: HashMap<String, (u64, String)>` — `(timestamp, value)` per key |
| **write** to a quorum | `set` applies locally, forwards `repl <ts> k v` to peers, needs a majority of acks |
| **read** from a quorum | `get` polls `readts` from a majority, returns the max-`ts` value, else `ERR no read quorum` |
| single **(1, N)** writer, timestamps | `Store::next_ts` = stored ts + 1, assigned by the primary under one lock |
| **crash-recovery** state transfer | `--catch-up <primary>` pulls `dump` (a stream of `repl` lines) and applies it before serving |

**Honest limits — the syllabus beyond this project (each a signpost):**

- **No failover.** The primary is fixed; if it dies, no one is elected. *(→ leader election, Raft.)*
- **Single writer.** Keeps timestamps tie-free; multiple writers need vector clocks / an `(N,N)`
  register. *(→ CCGR §4.4, Dynamo's conflict resolution.)*
- **`remove` is not versioned → needs tombstones.** A delete just drops the key, so a stale replica
  can **resurrect** it under a quorum read. Correct deletion writes a *tombstone* `(ts, ⊥)`.
  *(→ Dynamo tombstones.)*
- **Catch-up is a point-in-time snapshot.** A write arriving mid-catch-up can be missed (small
  race). *(→ log-based catch-up: snapshot + log cutoff, à la Raft.)*
- **No stable storage.** A node acks a write it holds only in memory; a crash loses it (amnesia).
  Real durability **logs before acking**. *(→ CCGR §4.5 logged register; `01`'s persistence.)*
- **Regular, not atomic.** No read-write-back. *(→ CCGR §4.3.)*
- **No agreement on order / no consensus.** *(→ projects `04+`.)*

---

## Run

Build and test:
```bash
cargo build
cargo test        # unit tests for the versioned Store
```

Start a **3-node cluster** — every node lists the *other two* as peers (needed so any node can form
a read quorum):
```bash
cargo run -- 4000 127.0.0.1:4001 127.0.0.1:4002   # node A
cargo run -- 4001 127.0.0.1:4000 127.0.0.1:4002   # node B
cargo run -- 4002 127.0.0.1:4000 127.0.0.1:4001   # node C
```
Talk to node A with the bundled client (it connects to `127.0.0.1:4000`):
```bash
cargo run --bin client
> set name luca
OK
> get name
luca
```

**Wire protocol** (newline-framed, same channel for clients and replicas):

| Command | From | Meaning |
|---|---|---|
| `set <k> <v…>` | client | write; primary assigns a ts, replicates, needs a write quorum |
| `get <k>` | client | read; polls a majority, returns the highest-ts value |
| `remove <k>` | client | delete (best-effort, not versioned) |
| `repl <ts> <k> <v…>` | primary → replica | apply a versioned write (terminal — not re-forwarded) |
| `readts <k>` | reader → replica | reply `<ts> <value>` or `none` (for a read quorum) |
| `dump` | recovering node → peer | reply with the whole store as `repl …` lines + `END` |

**Failure demos** (Python scripts drive real sockets against the binary):

- Quorum writes — kill a backup, watch writes survive on a majority and fail without one.
- Catch-up — kill a node, write while it's down, restart with `--catch-up`, watch it converge.
- Read quorum — restart a node **empty**, then read *from it* and get the latest anyway.

Restart a crashed node so it catches up first:
```bash
cargo run -- 4002 127.0.0.1:4000 127.0.0.1:4001 --catch-up 127.0.0.1:4000
```

## Design & notable implementation details

- **Roles.** Any node can coordinate; by convention clients **write to one node** (the primary,
  keeping timestamps tie-free) but may **read from any** node. A node's positional args are its
  **peers**; `--catch-up <addr>` pulls a snapshot at startup.
- **Versioned store.** Values are `(u64, String)`. `next_ts` derives the next timestamp from the
  stored one — no separate counter, so it survives catch-up for free. Timestamp assignment
  (`next_ts` then `write`) is done **under a single lock** so two concurrent `set`s can't collide.
- **Replication carries the timestamp.** A backup must store the *same* `(ts, value)` the primary
  chose, so writes are forwarded as `repl <ts> k v` (not the raw `set` line). The `repl` verb is
  **terminal**: a replica applies it and never re-replicates. `dump` emits the store as `repl`
  lines, so catch-up reuses the exact same apply path.
- **Quorum arithmetic.** `total = peers + 1`, `quorum = ⌊total/2⌋ + 1`. Writes count the primary's
  own copy as one ack; reads count the coordinator's own copy as one response.
- **Concurrency.** Thread-per-connection, store behind `Arc<Mutex<…>>`, locked per operation
  (as in `02`).

## What I learned

*Rust:* tuples as map values and destructuring them (`(ts, value)`), `Option`/`match`/`if let`,
iterator adapters (`splitn`, `strip_prefix`, `max_by_key`), a small hand-rolled arg parser
(`while let` over an iterator), `String` building, and reusing `Arc`/`Mutex`/threads/`TcpStream`
from `02`. Compiler-error-driven refactoring (change the data model, let `rustc` list every call
site) and `cargo fmt`.

*Distributed systems:* **replication** and the **CAP** trade-off (experienced, not just stated);
**quorums** and the majority-intersection argument for **both** writes and reads; the **register**
abstraction and **`R + W > N`**; **versioning/timestamps** and why a single writer keeps them
simple; **crash-recovery**, **amnesia**, and **anti-entropy** (catch-up); **safety vs liveness**
and the **CP** choice; and a concrete feel for exactly which questions require **consensus**.

---

## References

**Course reference text (the theory spine for the whole repo)**
- Christian Cachin, Rachid Guerraoui & Luís Rodrigues, *Introduction to Reliable and Secure
  Distributed Programming*, 2nd ed., Springer, 2011. For `03`: **registers** and the **Majority
  Voting (1, N) regular register** (Ch. 4, §4.2.3), **quorums** (§2.7.3), **crash-recovery** and
  stable storage (§2.2.4), and the road ahead in **consensus** (Ch. 5–6). ISBN 978-3-642-15259-7.

**Quorum replication & consistency**
- Giuseppe DeCandia et al., *Dynamo: Amazon's Highly Available Key-value Store*, SOSP 2007. The
  archetype of quorum replication: `R + W > N`, versioning (vector clocks), hinted handoff, and
  Merkle-tree anti-entropy — the production versions of what `03` does by hand.
  <https://www.allthingsdistributed.com/files/amazon-dynamo-sosp2007.pdf>
- Seth Gilbert & Nancy Lynch, *Brewer's Conjecture and the Feasibility of Consistent, Available,
  Partition-Tolerant Web Services*, ACM SIGACT News, 2002. The formal **CAP theorem** — the choice
  `03` makes (C over A) when a quorum is unreachable.

**Where `03` points next (consensus)**
- Diego Ongaro & John Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*,
  USENIX ATC 2014. Leader election + a replicated log + snapshot install — the answers to the open
  questions in §5. etcd is "a KV store on Raft."
- Leslie Lamport, *The Part-Time Parliament* / *Paxos Made Simple*, 1998/2001. The original
  consensus algorithm.
- Michael Fischer, Nancy Lynch, Michael Paterson, *Impossibility of Distributed Consensus with One
  Faulty Process (FLP)*, JACM 1985. Why agreement is fundamentally hard: no deterministic protocol
  guarantees consensus in an asynchronous system with even one crash.
- Fred Schneider, *Implementing Fault-Tolerant Services Using the State Machine Approach*, ACM
  Computing Surveys, 1990. Replicated state machines and their equivalence to total-order broadcast.

---
Part of [distributed-systems-in-rust](../).
