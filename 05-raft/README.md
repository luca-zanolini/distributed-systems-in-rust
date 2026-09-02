# 05 — Raft

A working (crash-fault) implementation of the **Raft consensus algorithm** — the finale that
**fuses `03`'s replicated register with `04`'s leader election into one algorithm**. A cluster
elects a leader by term, replicates a **log** of client commands, commits an entry once a **majority**
holds it, and applies committed entries to a key-value **state machine** — so every node ends up in
the *identical* state, and the data **survives the leader's death**. This is the algorithm behind
**etcd, Consul, TiKV, and CockroachDB**.

> **The big-picture companion to this project is [`CONSENSUS.md`](CONSENSUS.md)** — a full map of
> consensus theory (FLP, timing models, crash vs. Byzantine, quorums, 2PC vs. consensus) that the
> whole repo has been building toward. This README is about *Raft, the algorithm we built*; that
> document is about *the landscape it sits in*.

---

## Theory — consensus and the replicated state machine

### 1. What problem consensus solves

Everything before this project could tolerate crashes but had a **hole**: `03`'s primary was fixed
by hand (no failover), and `04` elected a leader but that leader did nothing. **Consensus** closes
the hole — it lets a set of nodes **agree on a single, growing sequence of values** (here, client
commands) *despite* crashes, so that a leader can fail and the cluster keeps going with **no lost,
no reordered, no duplicated** decisions.

Consensus is the hardest problem in the crash-fault world: **FLP** proves it's *impossible* to solve
deterministically in a fully asynchronous network with even one crash (see `CONSENSUS.md`). Raft
sidesteps FLP the standard way — it assumes **partial synchrony** (randomized timeouts, which only
affect *liveness*, never *safety*).

### 2. State-machine replication (the model)

Raft implements **state-machine replication**: give every replica the same **initial state**, the
same **ordered log** of commands, and a **deterministic** apply function, and they all compute the
**identical** state. The log is the source of truth; each node's KV store is a *derived* view it
rebuilds by replaying committed entries. Agreeing on the *log order* is the consensus problem;
executing the commands is just deterministic replay.

### 3. Where it's used

| System | Uses |
|---|---|
| **etcd** (Kubernetes' brain) | Raft for all cluster state |
| **Consul**, **Nomad** | Raft |
| **TiKV / TiDB**, **CockroachDB** | Multi-Raft (one Raft group per shard) |
| **ZooKeeper** | ZAB (a Raft-like atomic broadcast) |
| **Spanner** | Multi-Paxos (Raft's older, subtler cousin) |

### 4. How this project evolved — one problem at a time

| # | We built… | …which exposed |
|---|---|---|
| **M1 · step 1** | **role/term state machine** — Follower/Candidate/Leader + a randomized election timeout that promotes a silent-leader's follower to Candidate | a lone candidate can't win a multi-node cluster — it only has its own vote |
| **M1 · step 2** | **RequestVote + heartbeats** — one vote per term, majority wins, leader heartbeats keep followers quiet | *who* leads is solved; *what* they agree on isn't |
| **M2 · step 1** | **log + state machine** (leader-side) — `set` → `log` → commit → `apply()` → KV | it only lives on the leader — a crash loses everything |
| **M2 · step 2** | **AppendEntries + majority commit** — replicate the log, commit the index a majority holds, followers apply | works with one stable leader, but adversarial timing can still lose committed data |
| **M3** | **safety** — the election restriction + the commit-term rule | the algorithm is now *provably* correct (for crash faults) |

The arc: **elect** (M1) → **replicate & commit** (M2) → **prove it safe** (M3).

### 5. The two safety rules (M3 — the subtle heart of Raft)

- **Election restriction → Leader Completeness.** A node grants its vote only if the candidate's log
  is **at least as up-to-date** as its own (*higher last-term wins; on a tie, longer log wins*). A
  committed entry lives on a majority; a candidate needs a majority of votes; the two majorities
  **overlap** in a node that holds the committed entry and will therefore *deny* a candidate missing
  it. So **the winner always has every committed entry** — no committed data is ever lost across a
  leader change.
- **Commit-term rule (Raft's Figure 8).** A leader may **directly commit only entries from its *own*
  term**; older entries commit *indirectly*, once a current-term entry above them commits. Without
  this, a majority-replicated entry from a previous term can still be overwritten — the subtlest bug
  in Raft.

> 🎓 Both are the same **quorum-intersection** theorem from `03`, now guarding *leadership* and
> *commitment*: any two majorities share a node, and that shared node's single vote / single log
> settles the outcome.

### 6. In the CCGR framework

Raft is a **fail-noisy uniform consensus** algorithm (CCGR **Chapter 5**): it tolerates crashes,
does *not* assume a perfect failure detector (only an eventual leader **Ω**, from partial synchrony),
and guarantees **uniform agreement** (even a node that crashes right after deciding agrees with the
others — our committed entries never diverge). Concretely it maps to CCGR's **"Leader-Driven
Consensus"** (§5.3): an eventual leader (`04`'s Ω, §2.6.5) drives an *epoch* (Raft's **term**), and
values are imposed and locked through **majority quorums** (§2.7.3). A Raft *term* is CCGR's *epoch*;
`AppendEntries` is the epoch's *propose/decide*; the election is the *epoch-change*.

### 7. How the code reflects the theory — and where it stops

| Theory | In this code |
|---|---|
| terms (logical clock, ≤1 leader/term) | `State.term`; higher term always wins (step down) |
| leader election (Ω) | randomized `election_timeout` → Candidate; `RequestVote`; majority |
| replicated log | `Vec<Entry>`; `AppendEntries` carries it |
| commit on a majority | leader takes the **majority-th largest** follower log length (the match-index median) |
| apply to a state machine | `apply()` replays committed entries into `kv` |
| Leader Completeness | the up-to-date-log vote restriction |
| Figure-8 safety | commit only when `log[agreed-1].term == currentTerm` |
| **crash-recovery persistence** | `persist()` fsyncs `term`/`votedFor`/`log`/`commit` **before every reply**; `load()` + replay on restart |

**Honest limits — the syllabus beyond this project (each a signpost):**

- **Brute-force log replication.** `AppendEntries` ships the *whole* log and followers *replace*
  theirs — safe here (the election restriction guarantees the leader has all committed entries), but
  wasteful. Real Raft uses **`prevLogIndex`/`prevLogTerm`** + per-follower **`nextIndex`** to
  replicate *incrementally* and reconcile a divergent follower by backing up. *(An optimization, not
  a safety gap.)*
- **Persistence: ✅ implemented.** `term`, `votedFor`, `log`, and `commit` are **fsync'd to
  `raft-<port>.state` before every reply**, and reloaded (with the log replayed into the KV) on
  restart — so a node survives a crash with **no double-vote and no lost committed entry**. This is
  CCGR's **crash-recovery** model backed by **stable storage** (§2.2.4), as in `01`/`03`. (See the
  `persistence.py` demo: kill the *whole* cluster, restart, data survives.)
- **No log compaction / snapshots.** The log grows forever. Real Raft **snapshots** the state machine
  and truncates. *(→ snapshotting.)*
- **Static membership.** No add/remove-node. Real Raft has a **joint-consensus** membership-change
  protocol. *(→ membership changes.)*
- **No client dedup / linearizable-read lease.** A retried client command could apply twice; reads
  are served by the leader but without a read-index/lease. *(→ client sessions, ReadIndex.)*
- **Crash faults only.** Nodes fail by stopping, never *lie*. Byzantine agreement (PBFT/HotStuff) is
  a different, harder problem — see [`CONSENSUS.md`](CONSENSUS.md).

---

## Run

```bash
cargo build
cargo test        # (unit tests to be added; the demos exercise the cluster end-to-end)
```

Start a **3-node cluster** — every node lists the others as peers:
```bash
cargo run -- 6000 127.0.0.1:6001 127.0.0.1:6002
cargo run -- 6001 127.0.0.1:6000 127.0.0.1:6002
cargo run -- 6002 127.0.0.1:6000 127.0.0.1:6001
```
Watch one node print `→ LEADER`. Then talk to the **leader** with any TCP client:
```
set x 1        → OK (log index 1)
get x          → 1
```
(A follower replies `NOT LEADER`.) Kill the leader and a new one is elected in a higher term,
serving the same data.

**Wire protocol** (newline-framed):

| Message | Direction | Meaning |
|---|---|---|
| `requestvote <term> <cand> <lastIdx> <lastTerm>` → `vote <term> <yes/no>` | candidate ↔ peer | election, with the up-to-date-log check |
| `append <term> <leader> <commit> <entries>` → `appendack <term> <logLen>` | leader ↔ follower | replication + heartbeat; reply reports log length |
| `set/remove/get …` | client → node | client commands (leader only) |

**Failure demos** (`demos/`) drive a real cluster over TCP:

| Script | Shows |
|---|---|
| `election.py` | a 3-node cluster elects one leader per term; kill it → a new leader in a higher term |
| `replication_failover.py` | write to the leader, **kill it**, and the data is served by the new leader (committed on a majority → survives) |
| `persistence.py` | write, **kill the *whole* cluster**, restart → the data survives (each node reloads `term`/`votedFor`/`log`/`commit` from disk) |

## Design & notable implementation details

- **Every node runs the identical loop** (no central coordinator): an election/heartbeat thread
  drives timers + AppendEntries, and a listener handles incoming RPCs + client commands. Shared
  `Arc<Mutex<State>>` between them.
- **Never hold the lock across network I/O.** The leader snapshots its log under a short lock,
  releases it, does the `AppendEntries` round, then re-locks to commit — otherwise two nodes could
  block on each other's locks.
- **Majority commit = the match-index median.** Sort every node's log length descending; the
  `majority`-th largest is the highest index a quorum holds. That single line is the whole
  commit rule.
- **Terms are the credential.** There's no identity/authentication check — a node trusts whoever
  carries a term ≥ its own (≤1 leader per term makes the term unambiguous). That trust is exactly
  what Byzantine faults break.

## What I learned

*Rust:* an `enum` role state machine; timers (`Instant`/`Duration`/`thread::sleep`); multi-thread
`Arc<Mutex<State>>` (many owners, one at a time, survives a thread's death); the discipline of
**never holding a lock across I/O**; and a real borrow-checker lesson — a `MutexGuard` `Deref`s the
*whole* struct, so `s.log.push(Entry { term: s.term, .. })` needs the term read out first.

*Distributed systems:* **consensus** and how it sidesteps **FLP** via partial synchrony; **terms**
as a logical clock for leadership; **one vote per term** + **majority** ⇒ ≤1 leader/term;
**committed vs. applied** (agree-on-order vs. execute); **state-machine replication**; the
**match-index median** commit rule; and the two safety rules (**Leader Completeness** and the
**Figure-8 commit-term rule**) that make Raft provably correct.

---

## References

**Course reference text**
- Christian Cachin, Rachid Guerraoui & Luís Rodrigues, *Introduction to Reliable and Secure
  Distributed Programming*, 2nd ed., Springer, 2011. For `05`: **consensus** (Ch. 5), esp.
  **fail-noisy / leader-driven consensus** (§5.3), **quorums** (§2.7.3), **eventual leader Ω**
  (§2.6.5). ISBN 978-3-642-15259-7.

**Raft and its lineage**
- Diego Ongaro & John Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*,
  USENIX ATC 2014. The algorithm we built (terms, election, log, safety — this is Figure 2 & 8).
- Leslie Lamport, *The Part-Time Parliament*, ACM TOCS 1998; *Paxos Made Simple*, 2001. Raft's older,
  more general cousin — **Raft ≈ understandable Multi-Paxos with a strong leader**.
- Robbert van Renesse & Deniz Altinbuken, *Paxos Made Moderately Complex*, ACM Computing Surveys,
  2015. The Multi-Paxos details Raft repackages.

**Why consensus is hard (see also `CONSENSUS.md`)**
- Michael Fischer, Nancy Lynch & Michael Paterson, *Impossibility of Distributed Consensus with One
  Faulty Process (FLP)*, JACM 1985. Why asynchronous deterministic consensus is impossible.
- Cynthia Dwork, Nancy Lynch & Larry Stockmeyer, *Consensus in the Presence of Partial Synchrony*,
  JACM 1988. The partial-synchrony model (GST) Raft's timeouts assume.

---
Part of [distributed-systems-in-rust](../).  ·  Theory map: [CONSENSUS.md](CONSENSUS.md)
