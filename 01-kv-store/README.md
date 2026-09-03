# Module 01 — The Key-Value Store: State, Durability, and the Register

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
Cachin, Guerraoui & Rodrigues, "Introduction to Reliable and Secure Distributed Programming",
2nd ed., Springer 2011 — cited throughout as **CCGR**. Prerequisites: none.*

**Abstract.** This module builds a persistent key-value store with an interactive command-line
interface, and uses it to introduce three ideas the entire course rests on: the **register** as
the elementary shared-storage abstraction (CCGR Ch. 4), **stable storage** and the distinction
between volatile and durable state (CCGR §2.2.4), and the observation that nearly every
distributed storage system — Redis, etcd, DynamoDB, Cassandra, TiKV — is a key-value store with
distribution and fault tolerance layered on top. The implementation is deliberately single-process
and failure-free; every property that holds trivially here becomes a theorem to be earned in the
later modules.

---

## Learning objectives

After completing this module, the reader should be able to:

1. define the key-value data model and explain why its simplicity makes it the canonical object
   of study for distributed storage;
2. define a **read/write register** and state why, in a single-process failure-free execution,
   the implementation trivially satisfies its sequential specification;
3. distinguish **volatile** from **stable** storage, define the **crash-recovery** failure model
   informally, and explain the role of stable storage in it;
4. identify the durability, crash-safety, and distribution properties this implementation does
   *not* provide, and name the technique that provides each.

---

## 1. Motivation: why a course on distributed systems begins with a dictionary

A key-value (KV) store is the simplest non-trivial storage abstraction: a map from **keys** to
**values** supporting `get`, `put`, and `delete`. That simplicity is methodological, not
incidental. Because the data model is trivial, making the store *distributed* isolates precisely
the difficulties that are inherent to distribution itself — durability, failure, replication,
consistency, ordering, agreement — with nothing hidden inside a complex data model or query
language. The KV store therefore serves as the course's *model organism*: each module adds one
distribution concern to the same object.

The abstraction also matters in practice. KV stores are the substrate of modern infrastructure:

| System | Role | Consistency stance |
|---|---|---|
| Redis, Memcached | in-memory cache, sessions, rate limiting | single-node / weak |
| etcd, ZooKeeper, Consul | cluster configuration, service discovery, locks, leader election | strong (consensus-backed) |
| DynamoDB, Cassandra, Riak | highly available storage at scale | eventual |
| RocksDB, LevelDB, Bitcask | embedded storage engines inside larger databases | single-node engine |

Kubernetes stores its entire cluster state in etcd; Amazon's Dynamo was designed around the
shopping-cart workload; TiKV and CockroachDB expose SQL over a distributed KV core. The tradeoffs
of this one abstraction span the design space of cloud storage.

## 2. System model

This module uses the degenerate case of the course's system model, stated here so that later
modules can strengthen it incrementally.

- **Processes.** A single process executes a sequence of steps; there is no concurrency and no
  message passing.
- **Failures.** None are tolerated. (The persistence mechanism gestures at the **crash-recovery**
  model — a process may stop and later restart, losing volatile state — but this module does not
  yet handle a crash at an arbitrary point; see §6.)
- **Timing.** Irrelevant with a single process; timing models become meaningful in Module 02.

From Module 02 onward the model becomes: a static set of `N` processes
`Π = {p₁, …, p_N}` communicating by message passing over a network, with an explicit failure
model (crash-stop, crash-recovery, or Byzantine) and an explicit timing model (synchronous,
partially synchronous, or asynchronous).

## 3. The abstraction: a register

The elementary object of shared storage is the **read/write register** (CCGR Ch. 4). A register
stores a value and offers two operations:

- `read() → v` — returns the current value;
- `write(v)` — replaces the value.

Its **sequential specification** is: *a read returns the value written by the most recent
preceding write* (or an initial value ⊥ if none exists). A key-value store is a collection of
registers, one per key; `get(k)` is a read of register `k`, `set(k, v)` a write, and
`remove(k)` a write of a distinguished empty value.

In this module there is one process, no concurrency, and no failure, so every execution is
sequential and the implementation satisfies the sequential specification by construction. The
substance of register theory — and of Modules 04 onward — is preserving this specification when
the register is **replicated** across processes that fail and messages that are delayed:
the *regular* and *atomic* (linearizable) register conditions of CCGR §4.1 are exactly graded
weakenings and restorations of the sequential specification under concurrency. Module 04
constructs a fault-tolerant register; the theory companion
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) develops the consistency
conditions formally.

## 4. Durability and stable storage

A process's memory is **volatile**: its contents do not survive a crash. **Stable storage**
(CCGR §2.2.4) is an abstraction offering `store` and `retrieve` operations whose effects persist
across crashes; it is what allows an algorithm designed for crash-stop failures to be lifted into
the **crash-recovery** model, where a process may crash, lose its volatile state (*amnesia*), and
rejoin. In practice stable storage means a file system with explicit flushing (`fsync`), and the
discipline that matters is *when* state reaches disk relative to the messages a process sends —
a point this course returns to repeatedly (Modules 07 and 08 both depend on it, under the slogan
*persist before you externalize*).

This module implements the weakest useful form of durability: the store is serialized to disk
(`store.db`, JSON via `serde`) when the user exits, and reloaded on startup. Data therefore
survives an orderly restart but not a crash — a distinction made precise in §6.

Two further observations that recur throughout the course:

- **The log is fundamental.** Append-only logs underlie single-node engines (Bitcask, LSM-trees),
  replication streams, write-ahead logging, and the replicated log at the heart of consensus
  (Module 07). The whole-file rewrite used here is the baseline against which the log is the
  improvement.
- **Serialization formats are contracts.** Changing the on-disk format breaks previously written
  files; production systems version their formats and provide migrations. (This project switched
  formats once during development and paid exactly this cost.)

## 5. Implementation

### 5.1 Structure

- **`Store`** — a wrapper over `HashMap<String, String>` exposing `set` / `get` / `remove`.
- **REPL** — reads a line from stdin, tokenizes it, and dispatches via a `match` on a slice
  pattern (`["set", key, rest @ ..]`, `["get", key]`, …).
- **Persistence** — `serde` + `serde_json` serialize the map to `store.db`; `load` treats a
  missing file as an empty store (first run).

### 5.2 Correspondence between theory and code

| Concept | Realization |
|---|---|
| register per key (sequential specification) | `Store` over `HashMap<String, String>`; single-threaded access |
| volatile state | the in-memory map |
| stable storage (weak form) | `save`/`load` of `store.db` on exit/startup |
| serialization and format evolution | `serde` JSON; a format change during development required a migration |
| client interface | the REPL — a stand-in for the network clients of Module 02 |

### 5.3 Notes on the Rust implementation

- **Ownership expresses intent:** `get` borrows (`Option<&String>`); `remove` returns an owned
  `String`, because the map relinquishes the value.
- **Multi-word values:** the slice-rest pattern (`rest @ ..`) with `join(" ")` admits values
  containing spaces.
- **Error handling:** file and JSON operations return `Result`; `?` propagates to `main`, whose
  `Box<dyn Error>` return type unifies I/O and serialization errors.

## 6. Limitations and outlook

Each limitation below is deliberate and names the module or technique that addresses it.

- **Not crash-safe.** State is saved only on exit; a crash beforehand loses all writes since
  startup. Crash safety requires logging each update to stable storage *before* acknowledging it
  (write-ahead logging + `fsync`). *(→ stable storage discipline, Modules 07–08.)*
- **O(n) persistence.** The entire store is rewritten on save. Append-only logs and LSM-trees
  make the cost of persistence proportional to the update, at the price of compaction machinery.
- **Single process.** No network interface. *(→ Module 02.)*
- **No replication, no consistency model, no agreement.** One copy of the data; the questions of
  consistency between copies and agreement on update order do not yet arise. *(→ Modules 04–07.)*

## 7. Exercises

1. **(Crash safety.)** Modify the implementation to append each successful `set`/`remove` to a
   log file and `fsync` it before printing the confirmation, replaying the log on startup.
   Measure the throughput cost relative to the current design. What is the crash-safety guarantee
   now, stated precisely?
2. **(Compaction.)** The log of Exercise 1 grows without bound. Implement periodic compaction
   (write a snapshot, truncate the log) and state the invariant that must hold between snapshot
   and log for recovery to be correct.
3. **(Specification.)** Give a precise argument that every execution of this module's store
   satisfies the register's sequential specification. Which of the assumptions (single process,
   no crash) does your argument use, and where?
4. **(Format migration.)** Design a versioned on-disk format for the store, and a migration path
   that loads version *n* files into a version *n+1* store. What should the implementation do
   when it encounters a file from version *n+2*?

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: stable storage and the crash-recovery
  model (§2.2.4); registers (Ch. 4). ISBN 978-3-642-15259-7.

**Single-node storage engines**
- J. Sheehy, D. Smith, *Bitcask: A Log-Structured Hash Table for Fast Key/Value Data*, Basho
  Technologies, 2010. <https://riak.com/assets/bitcask-intro.pdf>
- P. O'Neil, E. Cheng, D. Gawlick, E. O'Neil, *The Log-Structured Merge-Tree (LSM-Tree)*,
  Acta Informatica 33(4), 1996.

**Distributed key-value stores**
- G. DeCandia et al., *Dynamo: Amazon's Highly Available Key-value Store*, SOSP 2007.
  <https://www.allthingsdistributed.com/files/amazon-dynamo-sosp2007.pdf>
- F. Chang et al., *Bigtable: A Distributed Storage System for Structured Data*, OSDI 2006.
- J. C. Corbett et al., *Spanner: Google's Globally-Distributed Database*, OSDI 2012.

**Foundations**
- S. Gilbert, N. Lynch, *Brewer's Conjecture and the Feasibility of Consistent, Available,
  Partition-Tolerant Web Services*, ACM SIGACT News 33(2), 2002.
- D. Karger et al., *Consistent Hashing and Random Trees*, STOC 1997.
- I. Stoica et al., *Chord: A Scalable Peer-to-peer Lookup Service for Internet Applications*,
  SIGCOMM 2001.
- M. Fischer, N. Lynch, M. Paterson, *Impossibility of Distributed Consensus with One Faulty
  Process*, JACM 32(2), 1985.

---

## Running the code

```bash
cargo run     # start the REPL
cargo test    # unit tests
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
State is written to `store.db` (JSON) on exit and reloaded on startup.

---
*[Course home](../) · Next: [Module 02 — The Networked Store](../02-networked-kv-store/)*
