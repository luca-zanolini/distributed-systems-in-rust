# Module 02 — The Networked Store: Processes, Links, and Local Concurrency

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011). Prerequisite:
[Module 01](../01-kv-store/).*

**Abstract.** This module places Module 01's store behind a TCP server and adds a client, turning
a local program into a single-node network service accessed concurrently by many clients. It
introduces the two most basic abstractions of the course's system model — **processes** and
**point-to-point links** (CCGR §2.1, §2.4) — together with the **crash-stop** failure model, the
distinction between **safety and liveness** properties, message **framing** over a byte stream,
and mutual exclusion over shared state *within* one process. A recurring theme begins here: the
guarantees a layer provides (TCP as a perfect link) and the obligations it leaves to the layer
above (framing, and later end-to-end retries).

---

## Learning objectives

After completing this module, the reader should be able to:

1. state the fair-loss → stubborn → perfect link hierarchy and the three properties of perfect
   point-to-point links, and explain in what sense TCP implements them;
2. define the crash-stop failure model and classify given service guarantees as safety or
   liveness properties;
3. explain why a byte stream does not delimit messages, and implement a framing discipline;
4. explain how lock granularity determines whether concurrent clients are actually served in
   parallel, and why mutual exclusion inside one process is unrelated to distributed shared
   memory;
5. explain fault isolation: why a per-connection failure must not terminate the service.

---

## 1. Motivation

A local store is usable by exactly one process on one machine. Serving several clients, remote
access, or a single source of truth shared across programs requires a **network service**: one
process owns the state; others reach it by exchanging messages. This single change introduces the
subject matter of networked systems — a **protocol** (how messages are framed and interpreted),
**request/response** interaction, a network that can delay, drop, or reorder, peers that can
vanish, and, as soon as more than one client is served concurrently, **synchronization over
shared mutable state**.

The pattern built here — a concurrent server in front of a shared store, speaking a framed
protocol — is the shape of most backend infrastructure: database wire protocols (Redis's RESP,
PostgreSQL, etcd), RPC frameworks (gRPC, Thrift), and caches. The server's concurrency model
(thread-per-connection, thread pools, or event-driven I/O) is one of its central engineering
decisions; this module uses thread-per-connection and discusses its limits.

## 2. System model

- **Processes (CCGR §2.1.1).** The server and each client are processes. Processes share no
  memory; they interact only by exchanging messages over links.
- **Links.** Communication uses **perfect point-to-point links** (CCGR §2.4.4), specified below.
  TCP, together with the operating system's transport implementation, provides this abstraction,
  which is why the module does not re-implement retransmission or deduplication (cf. CCGR §2.4.7).
- **Failures: crash-stop (CCGR §2.2.2).** A process may fail by halting and thereafter takes no
  further steps. A process that never fails in an execution is **correct** in that execution. A
  client that disconnects abruptly (EOF, connection reset, broken pipe) is modeled as a crashed
  process; the server must remain correct.
- **Timing.** No timing assumptions are needed in this module: the server is purely reactive.

**Specification (perfect point-to-point links, CCGR §2.4.4).** For processes `p`, `q`:

- **PL1 (Reliable delivery).** If a correct process `p` sends message `m` to a correct process
  `q`, then `q` eventually delivers `m`.
- **PL2 (No duplication).** No message is delivered by a process more than once.
- **PL3 (No creation).** If some process `q` delivers a message `m` with sender `p`, then `m`
  was previously sent to `q` by `p`.

CCGR constructs this abstraction as a tower — fair-loss links (messages may be lost but a message
sent infinitely often is delivered infinitely often), strengthened to stubborn links
(retransmission), strengthened to perfect links (deduplication) — and notes that transport
protocols such as TCP already provide the result. This module adopts that stance; the tower is
re-derived at the application layer in later modules, when a single transport connection no
longer spans the whole system end-to-end.

**Safety and liveness (CCGR §2.1.3).** Every service guarantee decomposes into properties of two
kinds: a **safety** property states that nothing bad ever happens (its violation occurs at a
finite point in an execution and is irremediable); a **liveness** property states that something
good eventually happens. For this module's service: *"every reply corresponds to a command that
was actually sent, and reflects a real operation on the store"* is safety (compare PL3);
*"every command from a correct, connected client eventually receives a reply"* is liveness.

## 3. Design

### 3.1 Protocol

TCP delivers an ordered **byte stream**, not a sequence of messages: it does not mark where one
message ends and the next begins. Delimiting messages — **framing** — is therefore the
application's responsibility. This module uses newline framing: each command and each reply is
one line terminated by `\n`. (Length-prefixed framing is the common binary alternative;
Exercise 2.)

The protocol is request/response: `set k v` → `OK`, `get k` → the value, `remove k` → a
confirmation. This is a remote procedure call in miniature: a typed request crosses the network,
a typed response returns (Birrell & Nelson 1984).

### 3.2 Concurrency within the server

The server spawns one thread per accepted connection. All handler threads access the same store,
which is shared as `Arc<Mutex<Store>>`:

- `Arc` — atomically reference-counted shared ownership: all threads co-own the store, which
  lives as long as any of them;
- `Mutex` — mutual exclusion: `lock()` grants exclusive access; the guard releases the lock when
  it goes out of scope.

**Lock granularity determines concurrency.** A mutex held for the duration of a single
*operation* lets clients interleave; the same mutex held for the duration of a *connection*
serializes all clients silently — the service remains correct but is no longer concurrent. This
implementation locks per operation. (The distinction was discovered here as a live bug, and
recurs in Module 05 as the rule *never hold a lock across network I/O*.)

**A scope note.** The `Mutex` here is *local* mutual exclusion among threads of one process. It
is unrelated to CCGR's Chapter 4 "shared memory," which is a *distributed* register emulated over
message passing among processes that share nothing. CCGR assumes each process handles its events
mutually exclusively (§1.4.1); the mutex is how a multi-threaded process discharges that
assumption. Distributed shared memory begins in Module 03. The formal treatment of local
concurrency — data races, linearizability of a single object, and why a mutex provides it — is
in [CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md).

### 3.3 Fault isolation

A service must tolerate the crash of its clients. The per-connection handler returns
`Result`; the accept loop logs a handler's error and continues serving. A client's abrupt
disconnection — a crash in the model of §2 — is thereby contained: the failure of one connection
never terminates the process. This is the module's first exercise in *fault tolerance*, in its
simplest form.

## 4. Correspondence between theory and code

| Concept | Realization |
|---|---|
| processes exchanging messages | server (`src/main.rs`) and client (`src/bin/client.rs`) over TCP |
| perfect point-to-point links (PL1–PL3) | assumed from TCP; not re-implemented |
| framing over a byte stream | `\n`-terminated lines; `BufReader::lines()` |
| request/response (RPC) | one command line in, one response line out |
| crash-stop client failure, fault isolation | `handle_client → Result`; accept loop logs and continues |
| local mutual exclusion | `Arc<Mutex<Store>>`, locked per operation |

## 5. Limitations and outlook

- **Single node.** No replication; the server is a single point of failure. *(→ Module 03.)*
- **No durability.** The store is in-memory only (Module 01's persistence was set aside to focus
  on networking); a restart loses all state. *(→ write-ahead logging; stable storage.)*
- **One global lock.** Correct but serializing under contention; production systems shard locks
  or use lock-free structures.
- **Thread per connection.** Adequate for hundreds of clients; at tens of thousands, thread
  memory and scheduling cost dominate — the classic C10K argument for event-driven I/O
  (Kegel 1999).
- **Minimal protocol.** No versioning, authentication, encryption, or backpressure.

## 6. Exercises

1. **(Safety vs. liveness.)** Classify each of the following as safety, liveness, or neither,
   with justification: (a) "no reply is sent twice for one command"; (b) "if the client sends
   `get k` after a completed `set k v`, and no other write intervenes, the reply is `v`";
   (c) "the server eventually accepts every connection attempt"; (d) "the server never crashes."
2. **(Framing.)** Replace newline framing with length-prefixed framing (a 4-byte big-endian
   length followed by the payload). What new failure modes appear (consider a corrupted or
   adversarial length field), and how should the server bound them?
3. **(Lock granularity.)** Modify the server to hold the store's lock for an entire connection,
   and demonstrate experimentally — with two concurrent clients — that throughput degrades to
   that of a sequential server while correctness is preserved. Explain why no test that uses a
   single client can detect the change.
4. **(End-to-end argument.)** TCP provides PL1–PL3 per connection, yet a client that reconnects
   and retries a `set` after a timeout can still cause a duplicate write. Reconcile this with
   PL2, and propose a mechanism (request identifiers, idempotent operations) restoring
   exactly-once *effect*. (Saltzer, Reed & Clark 1984 is the relevant reading.)
5. **(Thread pool.)** Replace thread-per-connection with a fixed-size thread pool. What liveness
   property of §2 becomes conditional, and on what?

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: processes and messages (§2.1),
  safety/liveness (§2.1.3), crash-stop faults (§2.2.2), the link hierarchy and perfect
  point-to-point links (§2.4). ISBN 978-3-642-15259-7.

**Client/server and RPC**
- A. Birrell, B. Nelson, *Implementing Remote Procedure Calls*, ACM TOCS 2(1), 1984.

**Networking foundations**
- V. Cerf, R. Kahn, *A Protocol for Packet Network Intercommunication*, IEEE Trans.
  Communications 22(5), 1974.
- *RFC 793 — Transmission Control Protocol*, 1981.
- J. Saltzer, D. Reed, D. Clark, *End-to-End Arguments in System Design*, ACM TOCS 2(4), 1984.

**Concurrency and scale**
- D. Kegel, *The C10K Problem*, 1999.
- L. P. Deutsch, *The Fallacies of Distributed Computing*, c. 1994.

---

## Running the code

```bash
cargo run                 # server (listens on 127.0.0.1:4000)
cargo run --bin client    # interactive client, in another terminal
cargo test                # unit tests
```

In the client: `set name Luca`, `get name`, `remove name`; Ctrl+D to quit.

---
*[Course home](../) · Previous: [Module 01](../01-kv-store/) · Next:
[Module 03 — The Replicated Store](../03-replicated-kv-store/)*
