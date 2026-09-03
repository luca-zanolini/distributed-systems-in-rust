# Module 05 — Failure Detection and Leader Election

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011). Prerequisites:
[Module 02](../02-networked-kv-store/), [Module 04](../04-replicated-kv-store/).*

**Abstract.** This module builds a cluster that detects the loss of its leader and elects a
replacement by majority vote, with no central authority. It introduces **failure detectors**
(CCGR §2.6) — in particular the eventually perfect detector **◇P**, implemented from heartbeats
and timeouts — and **leader election**, culminating in the eventual leader abstraction **Ω**.
Two theoretical points carry the module: first, that under partial synchrony failure detection
can only ever be *eventually* accurate, because a slow process is indistinguishable from a
crashed one; second, that a **majority-vote** gate — the same quorum-intersection argument as
Module 04 — is what prevents two leaders from coexisting (*split-brain*). The module supplies
exactly the failover capability whose absence limited Module 04, and it constructs the oracle
(Ω) under which Module 07's consensus algorithm is proved live.

---

## Learning objectives

After completing this module, the reader should be able to:

1. specify the failure detectors **P** and **◇P** by their completeness and accuracy properties,
   and explain which timing assumptions suffice to implement each;
2. specify the eventual leader detector **Ω** and implement a monarchical election rule on top of
   ◇P;
3. explain why any timeout-based detector must admit false suspicions, and relate this to the
   impossibility of distinguishing *slow* from *crashed* in an asynchronous system;
4. define split-brain and prove that requiring a majority of votes, one vote per process,
   excludes it;
5. place failure detectors and timing models side by side as two descriptions of the same
   assumption, and state the role of Ω in the solvability of consensus.

---

## 1. Motivation

Many distributed protocols require a single process to be *in charge* for progress: a primary
ordering writes (Module 04), a coordinator driving a commit (Module 08), a sequencer, a lock
service. A fixed leader is simple but mortal; when it crashes, the system must do two things
**by itself**:

1. **detect** that the leader is gone — although no process can observe another's crash
   directly; and
2. **agree** on a replacement — although the authority that would normally appoint one is
   precisely the process that failed.

These two capabilities — failure detection and leader election — are the control plane beneath
most replicated systems: Raft's election timeouts, ZooKeeper's leader, Kubernetes leases,
Kafka's controller, gossip-based membership (SWIM; Consul's Serf) are all instances.

## 2. System model

- **Processes.** `N` nodes `Π = {p₁, …, p_N}`, identified by their network address and totally
  ordered by an identifier (here: port number). At most `f` may fail, `N ≥ 2f + 1`.
- **Failures.** Crash-stop. (A crashed-and-restarted node in the demonstrations rejoins as a
  correct process; the election tolerates this.)
- **Links.** Heartbeats are sent best-effort over per-message TCP connections; a lost heartbeat
  is compensated by the next one. The abstraction actually relied on is closer to CCGR's
  **fair-loss links** than to the perfect links of Modules 02 and 04 — deliberately, since the
  protocol is periodic and self-correcting.
- **Timing: partial synchrony.** Message delays and process speeds are usually bounded, but the
  bounds are unknown and may hold only eventually (Dwork–Lynch–Stockmeyer). The heartbeat period
  (1 s) and suspicion timeout (3 s) encode this assumption operationally.

## 3. The abstractions

### 3.1 Failure detectors (CCGR §2.6)

A **failure detector** is a per-process oracle that outputs a set of *suspected* processes. It
abstracts timing: an algorithm is proved correct against the detector's axioms, and the detector
is implemented separately from whatever synchrony the network provides.

**Specification (perfect failure detector, P; CCGR §2.6.2).**
- **PFD1 (Strong completeness).** Eventually, every crashed process is permanently suspected by
  every correct process.
- **PFD2 (Strong accuracy).** No process is suspected before it crashes.

**Specification (eventually perfect failure detector, ◇P; CCGR §2.6.4).**
- **EPFD1 (Strong completeness).** As PFD1.
- **EPFD2 (Eventual strong accuracy).** Eventually, no correct process is suspected by any
  correct process.

P never errs, and is implementable only under synchrony (a timeout that provably never misfires
requires a known delay bound). ◇P may err — it can suspect a slow but correct process — but its
errors are temporary; heartbeats plus an (adaptable) timeout implement it under partial
synchrony. The gap between P and ◇P is the operational content of the slogan *a crashed process
is indistinguishable from a slow one*: without known bounds, any finite timeout can be outwaited
by an adversarial schedule.

### 3.2 Leader election: the eventual leader detector Ω (CCGR §2.6.5)

**Specification (Ω).** Each process outputs one process it currently *trusts* as leader.
- **ELD1 (Eventual accuracy).** There is a time after which every correct process trusts some
  *correct* process.
- **ELD2 (Eventual agreement).** There is a time after which no two correct processes trust
  different processes.

Ω does not promise a unique leader at every instant — during unstable periods two processes may
each consider themselves leader — only that the situation *stabilizes*. This weakness is
essential: Ω is implementable under partial synchrony, and it is precisely strong enough for
consensus (§6). The implementation is **monarchical**: each process trusts the smallest-id
process it does not currently suspect (via ◇P).

### 3.3 The majority gate

A locally computed choice is not yet safe: a process that suspects everyone else would crown
itself, and a partitioned minority could elect a second leader — **split-brain**. The module
therefore gates leadership on votes: each process broadcasts, with its heartbeat, the identity
of the node it currently supports; a node *acts* as leader only while a **majority** of
processes (counting itself) support it.

**Proposition (no split-brain).** If every process supports at most one candidate at a time and
acting as leader requires support from `⌊N/2⌋ + 1` processes, then at no time do two processes
both act as leader on the strength of the same round of support.
*Proof sketch.* Two majorities intersect (Module 04, quorum-intersection lemma); a process in
the intersection supports one candidate, not two. ∎

The qualification "same round" is doing real work: because processes re-vote continuously and
nothing binds a process to its past vote, there are transient schedules in which stale support
counts overlap. Closing this gap requires making support *epochal* — a monotone term with at most
one vote per process per term. That refinement is exactly Raft's, and it is the subject of
Module 07.

## 4. Development of the implementation

| # | Design | Deficiency exposed |
|---|---|---|
| M1 | heartbeats: every node pings all peers each second | a pulse with no consumer: nothing yet reacts to silence |
| M2 | failure detection: suspect a peer silent for > 3 s; retract on contact (◇P) | detection without governance — and suspicion can be wrong, by design |
| M3 | monarchical election: trust the smallest non-suspected id (Ω) | decisions are purely local: a lone survivor crowns itself; a partition can crown two |
| M4 | majority-vote gate: votes ride heartbeats; act as leader only with a quorum of support | split-brain excluded; remaining gap: votes are not epochal (no terms) — the door to Raft |

**Experiments** (`demos/`): (i) *Detection:* kill a node — after ~3 s survivors print `SUSPECT`;
restart it — `ALIVE again`: the detector errs and self-corrects, the observable content of ◇P.
(ii) *Failover:* kill the leader — support reassembles around the next id within a timeout.
(iii) *Standing down:* kill two of three — the survivor tallies only its own vote (1/3) and
refuses leadership; under the M3 design it would have crowned itself.

## 5. Two lenses on one assumption: detectors and timing models

The solvability of agreement can be stated either in terms of the **timing model** or in terms
of the strongest **failure detector** implementable in it:

| Timing model | Implementable detector | Consensus solvable? |
|---|---|---|
| Synchronous | **P** (never errs) | yes, for any `f < N` (crash) |
| Partially synchronous | **◇P**, **Ω** | yes, with `f < N/2` (Paxos, Raft) |
| Asynchronous | none of the above | not deterministically — FLP |

The two columns are linked by a fundamental result: **Ω is the weakest failure detector for
consensus** given a correct majority (Chandra–Hadzilacos–Toueg 1996; without the majority
assumption, the pair (Σ, Ω) is weakest, where Σ is the quorum detector). Partial synchrony is
*sufficient* to implement Ω — in fact strictly weaker timing assumptions suffice — and Ω, with a
majority, suffices for consensus. This module implements Ω; Module 07 consumes it. A fuller
treatment, including why the failure-detector interface does not extend to Byzantine faults, is
in [CONSENSUS.md](../07-raft/CONSENSUS.md) §§2–4.

## 6. Correspondence between theory and code

| Concept | Realization |
|---|---|
| heartbeat | a 1 s background thread sends `ping <sender> <vote>` to every peer |
| ◇P | `last_heard: HashMap<peer, (Instant, vote)>`; suspect when `elapsed > 3 s`; retract on contact |
| Ω (monarchical) | `choice = min_by_key(port)` over non-suspected nodes (self included) |
| one vote per process | the current `choice` travels on every heartbeat; peers record the latest |
| majority gate | count peers whose recorded vote names this node (+ own); lead iff `≥ ⌊N/2⌋+1` |

Design notes. Every node runs the identical loop — the protocol is symmetric, periodic, and
self-stabilizing rather than a one-shot election: each second a node updates suspicions,
recomputes its choice, broadcasts, tallies, and reports status changes. A single message type
(`ping`) carries both failure detection and voting. Two clocks govern the module — the 1 s
period and the 3 s timeout — and the timeout must comfortably exceed the period, or a single
delayed heartbeat produces a false suspicion. Node identifiers are compared numerically by port
(`port_of`), not lexicographically.

## 7. Limitations and outlook

- **No terms.** Votes are not epochal; a process may support different candidates over time
  within one unstable period, leaving transient windows under churn (§3.3). Raft's monotone
  *term* with one vote per term closes this. *(→ Module 07.)*
- **The leader has no duties.** It is elected and idle; wiring it to Module 04's replication —
  leader coordinates writes, failover installs a new coordinator — is precisely the combination
  that becomes Raft. *(→ Module 07.)*
- **Fixed timeout.** Production detectors adapt to measured network behavior (e.g. the
  φ-accrual detector); a fixed 3 s trades false suspicions against detection latency bluntly.
- **Static membership.** The peer set is fixed at launch; joining, leaving, and discovery are
  membership problems (gossip, SWIM).
- **Crashes are simulated by killing processes.** A true network partition (all processes up,
  communication severed) is not exercised; the majority gate is the mechanism that would contain
  it, with the caveat of §3.3.

## 8. Exercises

1. **(Detector classification.)** For each modification, state which of EPFD1/EPFD2 (or PFD2)
   the resulting detector satisfies, and under which timing assumption: (a) the timeout is
   halved to the heartbeat period; (b) the timeout doubles after every false suspicion;
   (c) suspicions are never retracted.
2. **(Accuracy/latency trade-off.)** Instrument the implementation to count false suspicions
   per hour while injecting artificial delay (e.g. `tc`-style delay or a sleep in the send
   path). Plot detection latency and false-suspicion rate as functions of the timeout, and
   identify the regime the 3 s default occupies.
3. **(Split-brain, precisely.)** Exhibit an execution of the M3 design (no majority gate) with
   `N = 4` in which two processes simultaneously act as leader. Then show where the same
   execution is blocked once the majority gate is added.
4. **(Transient double-support.)** Construct a schedule in which, without terms, stale votes
   allow a process to *briefly* tally a majority that includes support a peer has already moved
   elsewhere. Explain which property of Raft's terms (monotonicity, or vote-once-per-term)
   eliminates the schedule, and why the eventual guarantees of Ω are not violated by it.
5. **(Ω without ◇P.)** Ω is strictly weaker than ◇P. Sketch an implementation of Ω in a system
   where only *one* process's links are eventually timely (all others fully asynchronous), and
   argue that no implementation of ◇P exists there.

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: failure detectors and leader election
  (§2.6: P §2.6.2, ◇P §2.6.4, Ω §2.6.5); timing assumptions (§2.5). ISBN 978-3-642-15259-7.

**Theory**
- T. D. Chandra, S. Toueg, *Unreliable Failure Detectors for Reliable Distributed Systems*,
  JACM 43(2), 1996.
- T. D. Chandra, V. Hadzilacos, S. Toueg, *The Weakest Failure Detector for Solving Consensus*,
  JACM 43(4), 1996.
- C. Dwork, N. Lynch, L. Stockmeyer, *Consensus in the Presence of Partial Synchrony*,
  JACM 35(2), 1988.
- M. Fischer, N. Lynch, M. Paterson, *Impossibility of Distributed Consensus with One Faulty
  Process*, JACM 32(2), 1985.
- M. K. Aguilera, C. Delporte-Gallet, H. Fauconnier, S. Toueg, *On Implementing Omega with Weak
  Reliability and Synchrony Assumptions*, PODC 2003. (Ω from a single eventually-timely source.)

**Practice**
- A. Das, I. Gupta, A. Motivala, *SWIM: Scalable Weakly-consistent Infection-style Process Group
  Membership Protocol*, DSN 2002.
- N. Hayashibara, X. Défago, R. Yared, T. Katayama, *The φ Accrual Failure Detector*, SRDS 2004.
- D. Ongaro, J. Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*,
  USENIX ATC 2014.

---

## Running the code

```bash
cargo build && cargo test
```

Start a three-node cluster:
```bash
cargo run -- 5000 127.0.0.1:5001 127.0.0.1:5002
cargo run -- 5001 127.0.0.1:5000 127.0.0.1:5002
cargo run -- 5002 127.0.0.1:5000 127.0.0.1:5001
```
Each node logs `SUSPECT …` / `… ALIVE again` and its leadership status. Kill the lowest-id node
and observe failover; the `demos/` scripts (`failure_detection.py`, `election.py`) reproduce the
experiments of §4.

---
*[Course home](../) · Previous: [Module 04](../04-replicated-kv-store/) · Next:
[Module 06 — Logical Time and Broadcast (planned)](../06-logical-time-broadcast/) ·
[Module 07 — Consensus: Raft](../07-raft/)*
