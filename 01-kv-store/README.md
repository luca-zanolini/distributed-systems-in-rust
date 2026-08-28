# 01 — Key-Value Store

A persistent key-value store with an interactive command-line REPL. This is the first
project in the repo and the foundation the later ones build on: nearly every distributed
store (Redis, etcd, DynamoDB, Cassandra, TiKV) is, at its core, a key-value store with
distribution and fault-tolerance layered on top.

---

## Theory — the key-value store in distributed systems

### 1. Why we start distributed storage with the key-value store

A key-value (KV) store is the simplest non-trivial storage abstraction: a map from
**keys** to **values**, with three operations — `get`, `put`, `delete`. That simplicity is
exactly *why* it is the right starting point for a distributed-systems course.

The data model is trivial — it is a dictionary. So when you make it *distributed*, **all of
the remaining difficulty is distribution itself**: durability, failure, partitioning,
replication, consistency, ordering, consensus. Nothing is hidden inside a complicated data
model (as it would be with, say, SQL and query planning). The KV store therefore isolates
the distributed-systems problems and lets you study them one at a time.

> One-sentence framing for a class: **"A distributed key-value store is a distributed
> `HashMap` — and that single word *distributed* is the entire course."**

It is also the *minimal building block* of larger systems: relational databases, file
systems, message queues, and coordination services can all be built on top of a KV store.

### 2. Where KV stores are used in practice, and why they matter

KV stores are the **substrate of modern cloud infrastructure**:

| System | Role | Consistency stance |
|---|---|---|
| **Redis**, **Memcached** | in-memory cache, sessions, rate-limiters | (usually) single-node / weak |
| **etcd**, **ZooKeeper**, **Consul** | cluster config, service discovery, locks, leader election | **strong** (consensus-backed) |
| **DynamoDB**, **Cassandra**, **Riak** | always-writable data at massive scale | **eventual** (highly available) |
| **RocksDB**, **LevelDB**, **Bitcask** | embedded storage *engine* inside bigger databases | single-node engine |

Why it matters: this abstraction runs the backbone of the internet. **Kubernetes keeps its
entire cluster state in etcd.** Amazon's shopping cart was the motivating use case for
**Dynamo.** **TiKV** and **CockroachDB** are distributed KV stores that expose SQL on top.
Understand the KV store's tradeoffs and you understand the core of cloud infrastructure.

### 3. What a key-value store actually is

**Data model:** an associative array / dictionary — `key → value`, where both are typically
opaque byte strings.

**Core operations:** `get(k) → value?`, `put(k, v)`, `delete(k)`; ordered stores add `scan`
(range queries).

The moment you go beyond a single in-memory map, two hard axes appear:

- **(a) Storage engine — single-node durability & performance.** How do bytes reach disk and
  get indexed? Common designs, from simplest to most sophisticated:
  - *in-memory hash map* — fastest, but volatile (lost on crash);
  - *append-only log + in-memory index* (**Bitcask**) — fast writes, bounded lookups;
  - *log-structured merge-tree* (**LSM-tree**: LevelDB/RocksDB/Cassandra) — write-optimized;
  - *B-tree* — read-optimized, the classic database index.
- **(b) Distribution — when one machine is not enough, or can fail:**
  - *partitioning / sharding* — split keys across nodes (**consistent hashing**; Dynamo, Chord);
  - *replication* — keep copies so data survives node loss;
  - *consistency* — when replicas disagree, what does a read return? **strong/linearizable**
    vs **eventual**. The **CAP theorem** says that under a network partition you must choose
    **C**onsistency *or* **A**vailability — you cannot have both;
  - *ordering & consensus* — to keep replicas in agreement you need agreement on the *order*
    of operations, i.e. **consensus** (Paxos, Raft). etcd is literally "a KV store on Raft."

So "KV store" spans a spectrum from a single in-memory `HashMap` to a globally-replicated,
consensus-backed, partitioned system — **the same tiny API, wildly different engineering.**

### 4. Teaching notes — the KV store as a lens on the whole course

- **The staircase.** A KV store is "just a `HashMap`" until you add **durability**, then
  **crash-safety**, then **networking**, then **replication**, then **consistency**, then
  **consensus** — and *each step is a chapter of distributed systems.* This repo climbs that
  staircase one project at a time.
- **The log is fundamental.** Append-only logs underlie Bitcask, LSM-trees, replication, and
  consensus. "The log" is arguably *the* central abstraction of distributed systems.
- **Order is not free.** A `HashMap` iterates in *random* order (see below) — a first hint
  that, once distributed, *ordering* is something you must actively impose (logical clocks,
  consensus), never assume.
- **CAP is a design fork, not a bug.** Every real system picks a side: Dynamo/Cassandra
  choose availability (AP); etcd/Spanner choose consistency (CP).
- **Formats evolve.** Changing how you serialize data breaks old files — real systems
  version their on-disk formats and ship migrations.

### 5. How *this* project reflects the theory (and where it deliberately stops)

| Theory | In this code |
|---|---|
| KV **data model** | `Store` = a `HashMap<String, String>` with `set` / `get` / `remove` |
| simplest **storage engine** | the in-memory `HashMap` — fastest and most volatile (the "before durability" baseline) |
| **durability** | `save`/`load` to `store.db`: rewrite the whole file on exit, reload on startup |
| **serialization** + **format migration** | `serde` + JSON; switching from the old tab format broke old files — the migration lesson, felt firsthand |
| a **client interface** | the REPL — a stand-in for the network clients that arrive in `02` |

**Honest limits (these are the syllabus for later, not accidents):**

- **Not crash-safe** — we save only on `exit`, so a crash before then loses everything. Real
  engines append to a write-ahead log and `fsync`. *(→ crash recovery, the log.)*
- **O(n) per save** — we rewrite the entire file every time. *(→ append-only logs, LSM-trees.)*
- **Single process** — no network, no clients over a socket. *(→ project `02`, TCP.)*
- **No replication, partitioning, consistency model, or consensus.** *(→ projects `03+`.)*

In short: this project is the **"K" and the "V."** The **"distributed"** is the rest of the
course, and every limit above is a signpost to the next project.

### 6. In the CCGR framework (the book's language)

From here on this repo also speaks the vocabulary of Cachin, Guerraoui & Rodrigues,
*Introduction to Reliable and Secure Distributed Programming* (2nd ed., 2011) — **"CCGR"**, the
reference text for the theory. Where `01` sits in its framework:

- **The object is a *register*.** A KV store is a set of **read/write registers** — one per key —
  the shared-storage abstraction CCGR develops in **Chapter 4**. A register supports two
  operations, **read** (our `get`) and **write** (our `set`; `remove` writes a distinguished
  "empty" value). This project implements the **single-process, failure-free case**: with no
  concurrency and no faults, every read returns the last value written, so the register is
  trivially **atomic / linearizable** (CCGR §4.1.3, *completeness and precedence*). The hard part
  of Chapter 4 — keeping a register atomic once it is **replicated** across processes that can
  fail — is exactly what project `03` begins, via **quorums**.
- **Persistence is *stable storage*.** Our `save`/`load` to `store.db` is CCGR's **stable
  storage** (§2.2.4): the `store`/`retrieve` operations a process uses to survive a
  **crash-recovery** fault and defeat **amnesia** — the loss of volatile state on restart. The
  book uses exactly this to lift crash-stop algorithms into the **fail-recovery** model (logged
  links §2.4.5, logged registers §4.5). So `01`'s durability is an early, informal meeting with a
  concept the later projects make precise.
- **Failure model: none yet.** One process, no faults — below even the **crash-stop** model
  (§2.2.2) the networked projects enter. In CCGR's classes of algorithms (§1.5), the distributed
  story starts at `02`.

---

## Run

```bash
cargo run     # start the REPL
cargo test    # run the unit tests
```

Example session:
```
set name Luca
get name              -> Luca
set country South Korea
get country           -> South Korea
remove name           -> Removed: Luca
get name              -> Key not found
exit
```
State is written to `store.db` (JSON) on exit and reloaded on startup, so data survives a restart.

## Design

- **`Store`** — a thin wrapper over `HashMap<String, String>` with `set` / `get` / `remove`.
- **REPL** — reads a line from stdin, splits it into words, and dispatches with a `match` on a
  slice pattern (`["set", key, rest @ ..]`, `["get", key]`, …). Handles end-of-input and unknown commands.
- **Persistence** — `serde` + `serde_json` serialize the store to JSON in `store.db`; `load`
  reads it back on startup, treating a missing file on the first run as an empty store.

## Notable implementation details

- **Ownership by intent:** `get` borrows and lends back (`Option<&String>`); `remove` returns
  an *owned* `String`, because the map gives that value up.
- **Multi-word values:** a slice-rest pattern (`rest @ ..`) plus `join(" ")` lets values contain spaces.
- **Errors:** file and JSON operations return `Result`; `?` propagates them to `main`, whose
  `Box<dyn Error>` return type can hold either an I/O or a JSON error.

## What I learned

*Rust:* structs and methods, ownership and borrowing (owned vs. borrowed parameters and
returns), `Option` / `Result` / `match`, the `?` operator, slice patterns, file I/O, `serde`
derive macros, `Box<dyn Error>`, and unit tests.
*Distributed systems:* **durability** and its weakest form; why order and crash-safety are not
free; and that changing a serialization format is a real migration problem.

---

## References

The most important papers behind this topic. This project is a single-node toy; these are the
systems and results it grows toward.

**Course reference text (the theory spine for the whole repo)**
- Christian Cachin, Rachid Guerraoui & Luís Rodrigues, *Introduction to Reliable and Secure
  Distributed Programming*, 2nd ed., Springer, 2011. The text this repo's theory is aligned to.
  For `01`: **stable storage** and the crash-recovery model (§2.2.4), and **registers** as the
  shared-storage abstraction (Ch. 4). ISBN 978-3-642-15259-7.

**Single-node storage engines**
- Justin Sheehy & David Smith, *Bitcask: A Log-Structured Hash Table for Fast Key/Value Data*,
  Basho Technologies, 2010. The append-log + in-memory-index design our persistence naturally
  evolves toward; the storage model behind Riak (and PingCAP's TP201).
  <https://riak.com/assets/bitcask-intro.pdf>
- Patrick O'Neil, Edward Cheng, Dieter Gawlick, Elizabeth O'Neil, *The Log-Structured
  Merge-Tree (LSM-Tree)*, Acta Informatica, 1996. The write-optimized engine behind LevelDB,
  RocksDB, Cassandra, and TiKV.

**Distributed key-value stores**
- Giuseppe DeCandia et al., *Dynamo: Amazon's Highly Available Key-value Store*, SOSP 2007.
  The archetypal highly-available (AP) KV store: consistent hashing, quorums, vector clocks,
  hinted handoff. Blueprint for Cassandra, Riak, and DynamoDB.
  <https://www.allthingsdistributed.com/files/amazon-dynamo-sosp2007.pdf>
- Fay Chang et al., *Bigtable: A Distributed Storage System for Structured Data*, OSDI 2006.
  A sorted, wide-column store; SSTables and tablets.
- James C. Corbett et al., *Spanner: Google's Globally-Distributed Database*, OSDI 2012.
  Externally-consistent, globally-distributed; the TrueTime clock.

**Foundations (the tradeoffs any distributed KV store must confront)**
- Seth Gilbert & Nancy Lynch, *Brewer's Conjecture and the Feasibility of Consistent,
  Available, Partition-Tolerant Web Services*, ACM SIGACT News, 2002. The formal **CAP theorem**.
- David Karger et al., *Consistent Hashing and Random Trees*, STOC 1997. Partitioning keys
  across nodes with minimal reshuffling when membership changes.
- Ion Stoica et al., *Chord: A Scalable Peer-to-peer Lookup Service for Internet Applications*,
  SIGCOMM 2001. A key-value store realized as a distributed hash table (DHT).

**Where this repo is heading (consistency via consensus)**
- Diego Ongaro & John Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*,
  USENIX ATC 2014. The consensus algorithm behind etcd, a strongly-consistent KV store.
- Leslie Lamport, *Paxos Made Simple*, 2001. The original consensus algorithm.
- Michael Fischer, Nancy Lynch, Michael Paterson, *Impossibility of Distributed Consensus with
  One Faulty Process (FLP)*, JACM 1985. Why consensus is fundamentally hard: no deterministic
  protocol can guarantee agreement in an asynchronous system with even one crash.

---
Part of [distributed-systems-in-rust](../).
