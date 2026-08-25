# 02 — Networked Key-Value Store

`01`'s key-value store, now reachable over the network: a **TCP server** that owns the
store, and a **client** that talks to it over a socket. The data lives in one place, and
many clients read and write it **concurrently**. This is where the store stops being a
local program and becomes a (single-node) network service.

---

## Theory — networking, client/server, and concurrency

### 1. Why put the store on the network

A local store (project `01`) is usable by exactly one process on one machine. The moment
you want **several clients**, **remote access**, or a **single source of truth** shared
across programs, the data must live behind a **network service**: one process owns the
state, and others reach it by exchanging messages over a socket.

That one change introduces the whole subject of networked systems: a **protocol** (how
messages are framed and interpreted), **request/response** round-trips, the reality that
the **network can delay, drop, or reorder** and a peer can vanish, and — as soon as more
than one client is served at once — **concurrency over shared mutable state**.

### 2. Where this shows up in practice, and why it matters

Almost every backend service is a networked request/response server in front of a shared store:

- **Databases & caches** speak a wire protocol over TCP — Redis (RESP), PostgreSQL, MySQL,
  etcd, MongoDB. A client library opens a socket and exchanges framed messages, exactly like
  ours, just richer.
- **RPC frameworks** (gRPC, Thrift, Cap'n Proto) are this pattern generalized: a typed
  request goes out, a typed response comes back.
- **The concurrency model is a server's core engineering decision:** thread-per-connection
  (what we built), thread pools, or event-driven/async (`epoll`/`io_uring`, Tokio). It
  determines how many clients you can serve at once — the classic **C10K** question.

Understand "a concurrent server in front of a shared store, speaking a framed protocol" and
you understand the shape of most backend infrastructure.

### 3. What it precisely is

- **Client/server:** one process (the **server**) owns the resource and **listens**; others
  (**clients**) **connect** and drive it. The roles are asymmetric.
- **Request/response (RPC-shaped):** the client sends a command; the server executes it and
  returns a reply. Our `set k v` → `OK` is a hand-rolled remote procedure call.
- **Framing:** TCP is a **byte stream**, not a message stream — it does not mark where one
  message ends and the next begins. So the *protocol* must. We use **newline framing** (`\n`
  ends each command and each reply).
- **Concurrency + shared state:** to serve many clients at once you run their handlers
  concurrently — but they all touch the *same* store, so you need **synchronization** to
  avoid data races. We use a **thread per connection** with the store behind an
  **`Arc<Mutex<Store>>`** (shared ownership + mutual exclusion).
- **Fault isolation:** one client's failure must not take down the others. A per-connection
  error is contained and logged, never propagated to the whole process.

### 4. Teaching notes

- **A byte stream is not messages.** The most common beginner networking bug is assuming one
  `read` = one message. It isn't. Framing is *your* job; here, newlines.
- **Where you lock decides your concurrency.** A `Mutex` held for one *operation* lets
  clients run in parallel; the *same* `Mutex` held for a whole *connection* silently
  serializes them. Same primitive, opposite behavior. (We hit exactly this bug.)
- **Fearless concurrency.** Rust's `Send`/`Sync` marker traits make the compiler *reject* a
  program that shares state unsafely — you are forced into `Arc`/`Mutex`/channels, so data
  races are caught at compile time, not in production.
- **A server must never die from one client.** Fault isolation is what separates a *service*
  from a *script*.
- **The network is not reliable.** EOF, resets, broken pipes, and delays are *normal*, not
  exceptional — designing for them is the job.

### 5. How this project reflects the theory (and where it stops)

| Theory | In this code |
|---|---|
| client/server | `src/main.rs` (server, listens) + `src/bin/client.rs` (client, connects) |
| request/response (RPC) | one command line in → one response line out |
| framing | `\n`-terminated commands and replies; read via `BufReader::lines()` |
| concurrency | `thread::spawn` per connection |
| shared mutable state | the store behind `Arc<Mutex<Store>>`, locked **per operation** |
| fault isolation | `handle_client` returns `Result`; `main` logs the error and keeps serving |

**Honest limits — i.e. the syllabus beyond this project:**

- **Single node.** One server process on one machine. No replication or failover — if it
  dies, the service is gone. *(→ replication, consensus.)*
- **In-memory, not durable.** The store resets on restart (we dropped `01`'s persistence to
  focus on networking). *(→ reuse `serde` save/load, or a write-ahead log.)*
- **One global lock.** Every access serializes on a single `Mutex` — simple and correct, but
  a bottleneck at scale. *(→ sharded/per-key locking, lock-free structures.)*
- **Thread per connection.** Fine for hundreds of clients; it falls over at tens of thousands
  (each thread has real memory and scheduling cost). *(→ async/`Tokio`; the C10K problem.)*
- **A trivial text protocol** with no versioning, auth, or backpressure. *(→ real wire
  protocols, TLS, framing libraries.)*

Each limit is a signpost to a later project.

---

## Run

Two binaries (the server is the default; the client is the extra):
```bash
cargo run                 # start the server (listens on 127.0.0.1:4000)
cargo run --bin client    # in another terminal: the interactive client
cargo test                # unit tests for the Store
```
In the client, type `set name Luca`, `get name`, `remove name`; Ctrl+D (or Ctrl+C) to quit.

## Design

- **`Store`** — the same `HashMap<String, String>` API as `01` (`set`/`get`/`remove`).
- **Server (`main.rs`)** — binds a `TcpListener`, and for each accepted connection
  `thread::spawn`s a handler. The store is shared as `Arc<Mutex<Store>>`; each handler locks
  it *per operation*. Per-connection errors are caught in `main` and logged (fault isolation).
- **`handle_client`** — reads newline-framed commands off the socket (`BufReader::lines()`),
  dispatches with a slice-pattern `match`, and writes a reply. `try_clone` provides a separate
  write handle; `write_all` sends the response.
- **Client (`bin/client.rs`)** — `TcpStream::connect`, then a loop: read a line from stdin,
  send it (re-adding the `\n`), read one reply, print it.

## What I learned

*Rust:* `std::net` (`TcpListener`/`TcpStream`), the `Read`/`Write` traits and `BufReader`,
`try_clone`, multi-binary crates (`src/bin/`, `default-run`), and **fearless concurrency** —
threads, `Arc`, `Mutex`, `.lock()`, and the `Send`/`Sync` traits that enforce it.
*Distributed systems:* client/server and request/response, **message framing** over a byte
stream, **fault isolation** (a service surviving a broken connection), and **shared mutable
state** under concurrency — including that *where* you lock decides whether clients run in
parallel.

---

## References

**Client/server & RPC**
- Andrew Birrell & Bruce Nelson, *Implementing Remote Procedure Calls*, ACM TOCS, 1984. The
  paper that established RPC; our `command → reply` is a hand-rolled version.

**Networking foundations**
- Vinton Cerf & Robert Kahn, *A Protocol for Packet Network Intercommunication*, IEEE
  Transactions on Communications, 1974. The origin of TCP/IP.
- *RFC 793 — Transmission Control Protocol*, 1981. TCP itself: the reliable, ordered byte
  stream this server rides on.
- Jerome Saltzer, David Reed & David Clark, *End-to-End Arguments in System Design*, ACM
  TOCS, 1984. Where reliability and functionality belong in a networked system — a
  foundational design principle.

**Concurrency & scale**
- Dan Kegel, *The C10K Problem*, 1999. Why thread-per-connection eventually breaks, and how
  event-driven / async servers handle tens of thousands of connections — the road past this
  project's model.
- L. Peter Deutsch, *The Fallacies of Distributed Computing* (~1994). "The network is
  reliable / latency is zero / bandwidth is infinite / …" — the assumptions this project
  learns not to make.

**Where this is heading**
- Giuseppe DeCandia et al., *Dynamo: Amazon's Highly Available Key-value Store*, SOSP 2007. A
  networked KV store made *distributed* — replication, partitioning, and the consistency
  tradeoffs `02` does not yet face.

---
Part of [distributed-systems-in-rust](../).
