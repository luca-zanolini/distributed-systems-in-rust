# Module 07 — Consensus: Raft and the Replicated State Machine

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011). Prerequisites:
[Module 04](../04-replicated-kv-store/), [Module 05](../05-leader-election/). Theory companion:
[CONSENSUS.md](CONSENSUS.md).*

**Abstract.** This module implements the **Raft** consensus algorithm (Ongaro & Ousterhout 2014)
for crash faults and uses it to realize **state-machine replication**: a cluster elects a leader
per **term**, replicates a log of client commands, marks an entry **committed** once a majority
stores it, and applies committed entries in order to a key-value state machine, so that the
service survives the failure of any minority — including the leader. The module unifies
Modules 04 and 05: the quorum-replicated data plane and the leader election become a
single algorithm in which the same majority-intersection argument guards both leadership and
commitment. The safety-critical content is concentrated in two rules — the **election
restriction** (leader completeness) and the **commit-term rule** (the "Figure 8" scenario) —
and in the **persistence discipline** for crash-recovery. Raft is the algorithm underlying etcd,
Consul, TiKV, and CockroachDB. The companion document [CONSENSUS.md](CONSENSUS.md) situates the
algorithm in the wider theory: FLP, timing models, failure detectors, quorums, and the Byzantine
generalization.

---

## Learning objectives

After completing this module, the reader should be able to:

1. state the (uniform) consensus specification and explain how a replicated log of consensus
   instances yields state-machine replication;
2. explain how Raft circumvents FLP: safety holds unconditionally, liveness under partial
   synchrony (randomized timeouts as an implementation of Ω);
3. define terms, the roles follower/candidate/leader, and the vote-once-per-term rule, and prove
   that at most one leader exists per term;
4. state the election restriction and prove leader completeness from quorum intersection;
5. reproduce the "Figure 8" counterexample and state the commit-term rule that excludes it;
6. specify which state must reach stable storage, and at which points, for the algorithm to be
   correct under crash-recovery (*persist before you externalize*);
7. distinguish committed from applied entries, and majority-commit from unanimity (Module 08).

---

## 1. Motivation

Modules 04 and 05 each stop one step short of fault-tolerant service. The replicated register
of Module 04 has no failover: its writer is fixed, and the writer's crash halts writes
permanently. The election of Module 05 produces a leader with no duties — and without epochs its
guarantees are only eventual. **Consensus** closes the gap: it lets a set of processes agree on
a single growing *sequence* of operations despite crashes, so that leadership can move without
losing, duplicating, or reordering anything already decided.

**Specification (consensus; CCGR Ch. 5).** Each process proposes a value; the abstraction
decides values such that:

- **C1 (Termination — liveness).** Every correct process eventually decides.
- **C2 (Validity).** A decided value was proposed by some process.
- **C3 (Integrity).** No process decides twice.
- **C4 (Agreement — safety).** No two *correct* processes decide differently.
  **Uniform agreement** strengthens C4 to all processes: no two processes decide differently,
  *even if one of them subsequently crashes*. Raft provides uniform agreement — an entry
  committed anywhere is never contradicted, even by a node that crashes immediately after
  applying it.

**State-machine replication** (Schneider 1990). Give every replica the same initial state, the
same totally ordered log of deterministic commands, and apply commands in log order: all
replicas compute identical states. The log is the object of agreement — in effect one consensus
instance per index — and the service state is a derived view, reconstructible by replay. Raft
decides the log directly rather than composing single-shot consensus instances, but the
specification it satisfies is the same, and total-order broadcast (deciding a sequence) is
equivalent to consensus (CCGR Ch. 6).

**Impossibility and its circumvention.** By FLP, no deterministic algorithm solves consensus in
a fully asynchronous system with even one crash. Raft assumes **partial synchrony**: its
randomized election timeouts are an implementation of the eventual leader detector Ω
(Module 05). The division of labor is strict — *safety never depends on timing*; timeouts affect
only liveness (elections may be delayed or repeated, commitments are never contradicted). See
[CONSENSUS.md](CONSENSUS.md) §§2–4.

## 2. System model

- **Processes.** `N` nodes, `N = 2f + 1`, tolerating `f` crash faults; every quorum below is a
  majority (`f + 1`).
- **Failures.** Crash-recovery: nodes may crash and rejoin, retaining only what they wrote to
  stable storage (§6).
- **Links.** Perfect point-to-point links (TCP), as in Modules 02 and 04.
- **Timing.** Partially synchronous: safety unconditional; liveness after stabilization.

## 3. The algorithm

### 3.1 Terms and leadership

Time is divided into **terms**, numbered monotonically. Each term has at most one leader; terms
act as a logical clock for leadership, and every message carries the sender's term. A node is
always in one of three roles:

- **Follower** — passive; responds to leaders and candidates; holds a randomized election
  timeout that resets on contact from the current leader.
- **Candidate** — on timeout, a follower increments its term, votes for itself, and solicits
  votes (`RequestVote`).
- **Leader** — a candidate that gathers votes from a majority; sends periodic `AppendEntries`
  (heartbeat and replication) to all others.

Two rules govern every role: a node grants at most **one vote per term** (persisted; §6), and a
node that sees a higher term than its own adopts it and steps down to follower.

**Proposition (at most one leader per term).** Each node votes at most once in term *t*;
becoming leader in *t* requires votes from a majority; two majorities intersect in a node that
voted only once. ∎

Randomized timeouts make split votes (two simultaneous candidates dividing the electorate)
transient rather than persistent — this is the liveness mechanism, and the point at which
partial synchrony enters.

### 3.2 Log replication and commitment

The leader appends each client command to its log as an **entry** `(term, command)` and
replicates it via `AppendEntries`. The leader marks index `i` **committed** when a majority of
nodes store the log up to `i` — in this implementation, computed as the majority-th largest of
the acknowledged log lengths (the median match index) — and applies committed entries, in
order, to the state machine. Followers learn the commit index from subsequent `AppendEntries`
and apply accordingly.

The distinction **committed vs. applied** is the distinction between *agreement* and
*execution*: an entry is committed when the cluster is bound to it (safety attaches here), and
applied when a given replica has executed it against its state machine (a lagging local view).

### 3.3 The two safety rules

Replication and majority commitment alone are not safe across leader changes. Two further rules
carry the safety argument.

**Election restriction (leader completeness).** A node grants its vote only to a candidate whose
log is *at least as up-to-date* as its own: the candidate's last entry has a higher term, or an
equal term and no shorter log.

*Proposition.* A leader elected in term *t* holds every entry committed in any term `< t`.
*Proof sketch.* A committed entry resides on a majority `Q_c`. The new leader's electing
majority `Q_v` intersects `Q_c`; the common voter holds the entry and, by the restriction, only
grants its vote to candidates whose log is at least as up-to-date — which, by induction over
elections, forces the winner's log to contain the entry. ∎

Leadership may therefore change, but never regresses the committed prefix — the same quorum
intersection as Module 04, now guarding *history* rather than a single value.

**Commit-term rule ("Figure 8").** A leader may count a majority toward commitment **only for
entries of its own term**; earlier-term entries become committed indirectly, when an own-term
entry above them commits. Without this rule there is a well-known counterexample (Ongaro &
Ousterhout, Fig. 8) in which an entry replicated on a majority — but from an older term — is
later overwritten by a leader that never saw it: majority replication alone is *not* commitment.
In the implementation, the guard is precisely
`agreed > commit_index ∧ log[agreed−1].term = current_term`.

### 3.4 Liveness

Under partial synchrony, eventually one candidate's timeout fires alone, it wins an undivided
election, and its heartbeats suppress further timeouts; every command then commits in one
majority round-trip. Before stabilization, elections may repeat — costing time, never safety.

## 4. Crash-recovery: the persistence discipline

A node that crashes and restarts with empty state is dangerous, not merely behind: it may vote
twice in a term (its earlier vote forgotten), or a majority restarting empty may silently drop a
committed entry. The rule is:

> **Persist before you externalize.** Any state whose loss would falsify a promise already sent
> to another process must be forced to stable storage (`fsync`) *before* the message is sent.

Concretely, `current_term`, `voted_for`, and the log are persisted before answering a
`RequestVote` or `AppendEntries` and before acknowledging a client write (this implementation
persists the commit index as well, as an optimization); the state machine (`kv`) and role are
volatile, reconstructed by replay on restart. Recovery composes two mechanisms: **reload and
replay** (stable storage restores identity — no double vote, no lost committed prefix) and
**catch-up** (the current leader's `AppendEntries` brings a lagging log forward — the general
mechanism that subsumes Module 04's snapshot transfer). This is CCGR's crash-recovery model
(§2.2.4) applied to consensus state.

## 5. Development of the implementation

| # | Design | Deficiency exposed |
|---|---|---|
| M1a | roles + terms + randomized timeout | a lone candidate cannot win: it holds only its own vote |
| M1b | `RequestVote`, one vote per term, majority; heartbeats | leadership solved; nothing is agreed upon yet |
| M2a | leader-side log + state machine (`set` → log → commit → apply) | the log exists only on the leader; its crash loses everything |
| M2b | `AppendEntries` replication + majority commit (median match index) | correct with a stable leader; adversarial leader changes can still lose committed data |
| M3 | election restriction + commit-term rule | safe across leader changes (crash faults) |
| M4 | persistence (`fsync` term/vote/log before replying) + reload/replay | correct under crash-recovery; a full-cluster restart preserves all data |

**Experiments** (`demos/`): `election.py` — exactly one leader per term; killing it yields a new
leader in a higher term. `replication_failover.py` — a value written to the leader survives the
leader's death and is served by its successor (it was committed on a majority).
`persistence.py` — the *entire cluster* is killed and restarted; data survives from stable
storage; a node killed while in the middle of the protocol recovers with its vote and log
intact.

## 6. In the CCGR framework

Raft instantiates CCGR's **fail-noisy leader-driven consensus** (Ch. 5, §5.3), providing
*uniform* consensus: an eventual leader detector Ω (Module 05; here implemented by randomized
timeouts) drives **epochs** — Raft's terms — and within an epoch values are imposed and locked
through **majority quorums** (§2.7.3). The correspondence: a term is an epoch; the election is
the epoch-change; `AppendEntries` is the epoch's propose/decide exchange; the election
restriction plays the role of the epoch-change's state handover (the new epoch must adopt the
locked value — in Raft, the committed prefix travels *in the winner's log* rather than being
collected from a quorum afterward).

## 7. Correspondence between theory and code

| Concept | Realization |
|---|---|
| terms (logical clock; ≤ 1 leader/term) | `State.term`; any higher term forces step-down |
| Ω via randomized timeouts | election timeout → candidate; `RequestVote`; majority of votes |
| replicated log | `Vec<Entry { term, cmd }>`; `AppendEntries` carries it |
| majority commitment | sort acknowledged log lengths; take the majority-th largest; commit-term guard |
| state machine | `apply()` replays committed entries into the KV map |
| leader completeness | up-to-date-log check in the vote handler |
| Figure-8 exclusion | `log[agreed−1].term == current_term` before advancing `commit_index` |
| crash-recovery | `persist()` (`fsync` term/vote/log/commit) before every externalization; `load()` + replay on start |

Implementation notes. Every node runs the identical two threads — an election/heartbeat timer
and a connection handler — sharing state via `Arc<Mutex<State>>`; the leader snapshots its log
under the lock, *releases it across network I/O*, and re-acquires it to commit (holding a lock
across a blocking round-trip is the canonical route to distributed deadlock). Wire protocol
(newline-framed): `requestvote term cand lastIdx lastTerm` → `vote term granted`;
`append term leader commit entries` → `appendack term logLen`; client commands `set`/`get`/
`remove` are accepted by the leader only.

## 8. Limitations and outlook

- **Wholesale log replication.** `AppendEntries` ships the entire log and followers adopt it;
  safe (the election restriction guarantees the leader's log is authoritative) but linear in
  history size. Production Raft replicates incrementally with per-follower `nextIndex` and the
  `prevLogIndex/prevLogTerm` consistency check. *(→ Exercise 4.)*
- **No log compaction.** The log grows without bound; recovery replays it in full. Production
  Raft snapshots the state machine, truncates the log, and adds an `InstallSnapshot` RPC for
  followers behind the truncation point.
- **Static membership.** Cluster reconfiguration requires joint consensus (or single-server
  changes) — a protocol of its own.
- **No client sessions.** A retried command may apply twice; exactly-once *effect* requires
  client identifiers and deduplication. Reads are served by the leader without a ReadIndex or
  lease, so a deposed leader could briefly serve stale reads.
- **Crash faults only.** Nodes may halt but never lie. Byzantine behavior invalidates the trust
  the protocol places in unauthenticated terms and acknowledgments; tolerating it requires
  `3f + 1` nodes, signed certificates, and a view-change — Modules 10–11.

## 9. Exercises

1. **(Uniform agreement.)** Exhibit an execution of a *hypothetical* Raft variant without the
   commit-term rule that violates uniform agreement (reconstruct Figure 8 concretely with five
   nodes: give the logs at each step). Verify that the implemented guard blocks the final,
   violating commitment.
2. **(Leader completeness.)** Write out the induction of §3.3 in full, making explicit where
   vote-once-per-term, quorum intersection, and the up-to-date comparison are each used — and
   construct a counterexample execution when the comparison uses log *length* alone (no terms).
3. **(Persistence points.)** For each of the persisted fields (`term`, `voted_for`, log), give a
   concrete violating execution if that field alone is lost on restart. Then argue why `kv` and
   `commit_index` need not be persisted for safety (this implementation persists the latter
   anyway — what does it save?).
4. **(Incremental replication.)** Implement `nextIndex`/`prevLogTerm` reconciliation. Measure
   steady-state message size against the wholesale design as the log grows.
5. **(Linearizable reads.)** The current leader serves reads from its local state machine.
   Construct a scenario (network partition, new leader elected elsewhere) where this returns a
   stale value, and implement or sketch ReadIndex: the leader confirms its leadership with a
   majority round before serving the read.
6. **(Randomization vs. ranks.)** Module 05 elected the *smallest id*; Raft elects *whoever
   times out first*. Discuss the liveness failure mode of each rule under (a) a crashed lowest
   node, (b) synchronized timeouts, and why Raft randomizes rather than ranks.

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: consensus (Ch. 5), leader-driven
  consensus (§5.3), epochs, quorums (§2.7.3), Ω (§2.6.5), crash-recovery (§2.2.4).
  ISBN 978-3-642-15259-7.

**Raft and its lineage**
- D. Ongaro, J. Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*,
  USENIX ATC 2014. (Figures 2 and 8 are the module's §3.)
- L. Lamport, *The Part-Time Parliament*, ACM TOCS 16(2), 1998; *Paxos Made Simple*, ACM SIGACT
  News 32(4), 2001.
- R. van Renesse, D. Altinbuken, *Paxos Made Moderately Complex*, ACM Computing Surveys 47(3),
  2015.
- F. Schneider, *Implementing Fault-Tolerant Services Using the State Machine Approach*,
  ACM Computing Surveys 22(4), 1990.

**Foundations**
- M. Fischer, N. Lynch, M. Paterson, *Impossibility of Distributed Consensus with One Faulty
  Process*, JACM 32(2), 1985.
- C. Dwork, N. Lynch, L. Stockmeyer, *Consensus in the Presence of Partial Synchrony*,
  JACM 35(2), 1988.

---

## Running the code

```bash
cargo build
```

Start a three-node cluster:
```bash
cargo run -- 6000 127.0.0.1:6001 127.0.0.1:6002
cargo run -- 6001 127.0.0.1:6000 127.0.0.1:6002
cargo run -- 6002 127.0.0.1:6000 127.0.0.1:6001
```
One node reports leadership; issue `set x 1` / `get x` to it over any TCP client (followers
reply `NOT LEADER`). Killing the leader yields a successor, in a higher term, serving the same
data. Per-node state persists in `raft-<port>.state`. The `demos/` scripts reproduce the
experiments of §5.

---
*[Course home](../) · Previous: [Module 06 (planned)](../06-logical-time-broadcast/) · Next:
[Module 08 — Atomic Commitment](../08-two-phase-commit/) · Theory map:
[CONSENSUS.md](CONSENSUS.md)*
