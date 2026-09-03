# Module 06 — Atomic Commitment: Two-Phase Commit and Its Blocking Behavior

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011). Prerequisites:
[Module 05](../05-raft/) (for the contrast with consensus). Theory companions:
[CONSENSUS.md](../05-raft/CONSENSUS.md),
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md).*

**Abstract.** This module implements **two-phase commit (2PC)**, the classical protocol for
**atomic commitment** of a transaction whose effects span several nodes, and demonstrates —
constructively, on a running system — its defining weakness: a coordinator crash between the
voting and decision phases leaves participants *in doubt*, holding locks, unable to terminate.
Where Module 05's consensus makes progress with any majority, atomic commitment requires
**unanimity** through a single coordinator, and this difference in decision rule produces an
inversion of fault behavior. The module introduces transactions and their properties (with the
formal treatment in the theory companion), specifies **non-blocking atomic commitment (NBAC)**
following CCGR §6.1, presents 2PC and its correctness for the properties it does satisfy,
exhibits the blocking execution, connects the participant's in-doubt state to **strict
two-phase locking**, and surveys the repairs (three-phase commit; **Paxos Commit**, atomic
commitment over consensus). The domain is a bank transfer across accounts held on different
nodes — a transaction over **partitioned** data, in contrast to the **replicated** data of
Modules 03 and 05.

---

## Learning objectives

After completing this module, the reader should be able to:

1. define a distributed transaction over partitioned data and state the ACID properties,
   distinguishing atomicity (all-or-nothing under failure) from isolation (correctness under
   concurrency);
2. specify NBAC and identify which property 2PC fails;
3. describe both phases of 2PC, including the participant's obligations on voting YES
   (durability of the vote; holding locks);
4. exhibit the blocking execution and explain *why* an in-doubt participant can neither decide
   unilaterally nor learn the outcome from its peers;
5. explain why persistence of the in-doubt state is required for safety yet makes blocking
   permanent rather than curing it;
6. contrast atomic commitment with consensus along both axes (decision function; fault
   tolerance) and state the failure-detector separation (P vs. ◇P/Ω);
7. relate the participant's `prepared` state to strict two-phase locking;
8. describe Paxos Commit and the layered architecture (2PC across shards, each shard a
   consensus group) used by systems such as Spanner.

---

## 1. The problem: transactions over partitioned data

### 1.1 Partitioning versus replication

The preceding modules kept **the same** data on every node (replication: fault tolerance
through redundancy). This module's nodes hold **different** data — disjoint **partitions**
(shards) of the whole: account *A* on one node, account *B* on another. The two regimes pose
different problems and admit different decision rules:

| | Replication (03, 05) | Partitioning + atomic commitment (06) |
|---|---|---|
| each node holds | a copy of the same data | a distinct shard |
| goal | survive node loss | all-or-nothing effects across shards |
| a node is | interchangeable | irreplaceable (its shard exists nowhere else) |
| decision rule | **majority** | **unanimity** |
| one node's crash | tolerated | forces abort — or, for the coordinator, blocks |

### 1.2 Transactions

A **transaction** is a finite sequence of operations on named data items, terminated by
*commit* or *abort*, that the system must make appear as an indivisible unit. The transfer
"move 30 from *A* to *B*" is the canonical example: a write on *A* (debit) *and* a write on *B*
(credit), which must take effect together or not at all — partial effect (a debit without the
credit) is precisely the anomaly to be excluded. The classical correctness contract is **ACID**
(Gray; Härder & Reuter):

- **Atomicity** — all of the transaction's effects are installed, or none (all-or-nothing under
  *failure*);
- **Consistency** — the transaction preserves the application's invariants (here: money is
  conserved; the deltas sum to zero);
- **Isolation** — concurrent transactions do not interfere; the standard formalization is
  **serializability** (equivalence of the concurrent execution to some serial one);
- **Durability** — once committed, effects survive crashes (stable storage).

Formal definitions — transactions, histories, serializability and its variants, and the
relations between these properties — are developed in
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md); this module needs
atomicity and durability centrally, and touches isolation through locking (§5).

When a transaction touches a single node, that node can decide commit/abort locally. When it
spans several — each of which may be *unable* to perform its part (an overdraft, a violated
constraint, a crash) — the nodes face an agreement problem: all must reach the *same*
commit/abort outcome, and commit must be possible only if *every* participant can do its part.
This is **atomic commitment**.

### 1.3 Specification: non-blocking atomic commitment

Following CCGR §6.1, each participant casts a vote in {YES, NO} and processes decide in
{COMMIT, ABORT}:

- **NBAC1 (Uniform agreement — safety).** No two processes decide differently (whether or not
  they subsequently crash).
- **NBAC2 (Integrity).** No process decides twice.
- **NBAC3 (Commit-validity).** COMMIT is decided only if *all* participants voted YES.
- **NBAC4 (Abort-validity).** ABORT is decided only if some participant voted NO or crashed.
- **NBAC5 (Termination — liveness).** Every correct process eventually decides.

2PC satisfies NBAC1–NBAC4. It fails **NBAC5** under coordinator failure — the subject of §4.

## 2. System model

- **Processes.** One **coordinator** and `n` **participants** `p₁, …, p_n`, each holding one
  account (balance ∈ ℤ, initially 100). The topology is a **star**: every protocol message is
  coordinator↔participant; participants never communicate with each other. (This is 2PC as
  classically defined, and the topology is load-bearing for the blocking result.)
- **Failures.** Crash-recovery for participants (stable storage; §3.3). The coordinator may
  crash and, in the blocking demonstration, never recover.
- **Links.** Perfect point-to-point links (TCP). An unreachable participant is
  indistinguishable from a crashed one; the coordinator treats a missed reply as a NO
  (conservative, per NBAC4).
- **Timing.** Asynchrony suffices for every safety property; the blocking behavior is a
  *liveness* failure and no timing assumption on the participants' side repairs it (§4.3).
- **Faults are non-malicious.** Processes follow the protocol or crash; Byzantine behavior
  (equivocation by the coordinator, false votes) is out of model until Modules 07–08.

## 3. The protocol

A transaction is submitted to the coordinator as per-participant **deltas** — the transfer
above is `⟨−30 to p₁, +30 to p₂⟩`; the deltas of a well-formed transfer sum to zero — under a
transaction identifier *txid*.

```
Phase 1 (voting)     coordinator → each pᵢ :  PREPARE txid δᵢ
                     each pᵢ → coordinator :  VOTE txid {YES | NO}

     decision rule:  COMMIT  iff  every participant replied VOTE YES
                     ABORT   otherwise (any NO, or any missing reply)

Phase 2 (decision)   coordinator → each pᵢ :  COMMIT txid | ABORT txid
                     each pᵢ → coordinator :  ACK txid
```

### 3.1 Phase 1 — voting

On `PREPARE txid δ`, a participant votes YES iff it *can and may* apply δ: the balance would
remain non-negative, **and** it holds no other in-doubt transaction. Before replying YES it
must:

1. **record the vote durably** — write `(txid, δ)` to stable storage and `fsync` it, *then*
   reply (persist-before-externalize, exactly as in Module 05 §4); and
2. **enter the in-doubt state** — set `prepared = (txid, δ)`, reserving the resources. From
   this point the participant has issued a *promise*: it guarantees it will be able to commit
   δ if told to. It must refuse conflicting work (here: any other PREPARE) until released.

A YES vote is thus binding and durable; a NO vote requires neither durability nor a lock, since
NO forces ABORT regardless of anything else (NBAC3).

### 3.2 Phase 2 — decision

The coordinator computes the outcome — the logical conjunction of the votes — and imposes it.
On `COMMIT txid`, a participant with `prepared = (txid, δ)` applies δ to its balance, clears
`prepared`, persists, and acknowledges. On `ABORT txid`, it discards the reservation (balance
untouched), clears `prepared`, persists, and acknowledges. In both cases the *release of the
in-doubt state* happens only here — the shrinking phase of the lock discipline (§5).

### 3.3 Correctness of the non-liveness properties

*NBAC1:* the only source of decisions is the single coordinator, which computes one outcome per
txid and sends the same verdict to all. *NBAC3:* COMMIT requires the full conjunction of YES
votes; a single NO — or an unreachable participant, whose vote cannot be confirmed — yields
ABORT. *NBAC4:* ABORT arises only from a NO or a missing (crashed/unreachable) participant.
*Durability of the outcome at a participant:* a participant that voted YES has its vote on
stable storage; if it crashes and recovers, it is *still in doubt* — it comes back holding
`(txid, δ)` and awaiting the verdict, so a crash cannot cause it to forget a promise the
coordinator may already have acted on (the double-vote / lost-commit anomaly is excluded, as
demonstrated in `demos/persistence.py`).

## 4. The blocking behavior

### 4.1 The execution

Let the coordinator crash *after* collecting a full set of YES votes and *before* delivering
any verdict. Every participant is in doubt, and:

- **it cannot decide unilaterally.** Deciding ABORT may contradict a COMMIT the coordinator
  already sent to some other participant before crashing (violating NBAC1); deciding COMMIT may
  likewise contradict an ABORT. Both outcomes are consistent with the participant's local
  state — this is precisely what "in doubt" means;
- **it cannot consult its peers** — the star topology provides no participant↔participant
  channel, and no participant saw any vote but its own;
- **it cannot escape by restarting** — by §3.1 the in-doubt state is durable, so recovery
  returns it to the same state. (Volatility would restore liveness at the price of safety:
  a recovered participant that forgot its YES could vote for a conflicting transaction.)

The participant therefore waits indefinitely, holding its reservation; every future transaction
that touches the reserved resources is refused. NBAC5 fails. The demonstration
`demos/blocking.py` stages exactly this execution (a coordinator that stops after Phase 1) and
then shows a subsequent, well-formed transaction being refused by every participant.

### 4.2 Blocking is not deadlock

The stranded participant is not in a waiting *cycle*: it awaits a single external event (the
verdict) that will never arrive. The distinction matters because the standard remedies differ —
deadlock is broken by victim selection or ordering; blocking here can only be resolved by
supplying the missing event from elsewhere, which is exactly what the repairs of §6 do.

### 4.3 The theoretical position

The blocking of 2PC is not an implementation defect but the shadow of a genuine impossibility
gap. In the failure-detector hierarchy, NBAC is *harder* than consensus: consensus is solvable
with the eventual leader detector Ω and a correct majority (Module 05), whereas NBAC in general
requires the **perfect** failure detector *P* — deciding COMMIT requires certainty that no
participant has crashed (NBAC4 ties the outcome to crashes), and certainty about crashes is
exactly what no eventually-accurate detector provides (CCGR Ch. 6 develops NBAC from consensus
plus a perfect failure detector). Two consequences follow: under partial synchrony one should
*expect* atomic commitment to inherit consensus's machinery rather than avoid it; and the
"unanimity vs. majority" contrast with Module 05 is a difference in *validity properties*, not
merely in engineering.

| | Consensus (05) | Atomic commitment (06) |
|---|---|---|
| decision function | any proposed value | conjunction of votes (COMMIT iff all YES) |
| decision quorum | majority | all participants, via one coordinator |
| detector needed | ◇P / Ω (with majority) | P (in general) |
| coordinator/leader crash | new leader elected; progress resumes | participants block in doubt |

## 5. The in-doubt state is a lock: strict two-phase locking

The participant's `prepared` field is not merely protocol bookkeeping; it is an **exclusive
lock** on the participant's resources, held according to **strict two-phase locking (2PL)**.

**Two-phase locking.** A transaction's lock acquisitions all precede its lock releases: a
*growing* phase (acquire only) followed by a *shrinking* phase (release only). Basic 2PL
guarantees conflict-serializability. **Strict 2PL** further holds all *exclusive* locks until
commit/abort, which additionally provides recoverability and precludes cascading aborts
(**rigorous 2PL** holds shared locks too). The naming collision is unfortunate and worth
flagging once: 2P*C*'s phases (vote, decide) and 2P*L*'s phases (grow, shrink) are unrelated.

The correspondence: voting YES *acquires* the lock (growing phase — and the participant's
refusal of further PREPAREs while in doubt is exactly conflict prevention); the verdict
*releases* it (shrinking phase, at commit/abort — strict 2PL precisely). This yields the
module's sharpest formulation of §4:

> **The blocking of 2PC is a strict-2PL lock whose shrinking phase never arrives.** Holding
> locks to the verdict is what makes the protocol's isolation and recoverability correct; the
> same discipline is what makes a lost verdict catastrophic.

The full treatment of 2PL and its variants, serializability, and the mechanism/guarantee
distinction is in [CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) §§4–6.

## 6. Repairs: toward non-blocking atomic commitment

- **Cooperative termination / three-phase commit** (Skeen 1981). Adding a *pre-commit* round
  and letting in-doubt participants poll one another allows termination when failures are
  crash-stop and the network is synchronous; under partitions 3PC can violate safety, and it is
  rarely deployed.
- **Paxos Commit** (Gray & Lamport 2006). The principled repair, given §4.3: make the *decision
  itself* fault-tolerant by running it through consensus. Each participant's vote is registered
  in a consensus instance (or the coordinator is a replicated state machine); no single crash
  can then withhold the verdict. 2PC is the degenerate case with one acceptor.
- **The layered architecture.** Production systems compose both regimes of §1.1: data is
  partitioned into shards, each shard is *replicated* as a consensus group (Module 05), and
  cross-shard transactions run *atomic commitment* (this module) across shard leaders, with the
  per-shard groups standing in for both durable participants and a durable coordinator. Google
  Spanner is the canonical example (2PC over Paxos groups); CockroachDB's parallel commit is an
  optimized variant over Raft ranges.

## 7. Correspondence between theory and code

| Concept | Realization |
|---|---|
| transaction over partitions | per-participant deltas: `transfer δ₁ δ₂ …`, `δᵢ → participantᵢ` |
| NBAC3 (commit-validity) | `all_yes`: COMMIT iff every reply equals `VOTE txid YES` |
| NBAC4 (abort-validity) | a NO vote *or* an unreachable participant (`send → None`) forces ABORT |
| in-doubt state / strict-2PL lock | `prepared: Option<(u64, i64)>`; set on YES, refused-while-held, cleared on verdict |
| durable vote (persist-before-externalize) | `persist()` fsyncs balance + `prepared` before replying; `load()` restores on restart |
| the blocking execution | the `transfer-crash` command: run Phase 1, then stop — no verdict is ever sent |

Implementation notes. The participant is deliberately **passive and sequential** — a single
accept loop, no shared-state concurrency (contrast Module 05's timer thread and
`Arc<Mutex<State>>`): it takes no step except in response to the coordinator. This passivity is
the architectural mirror of the blocking result — a process with no autonomous behavior has no
mechanism by which to rescue itself. The coordinator's `transfer-crash` is a test hook that
realizes the §4.1 crash point deterministically, in the tradition of fault injection.

## 8. Limitations and outlook

- **The coordinator is a deliberate single point of failure** — the object of study. The repair
  is Paxos Commit (§6). *(→ Exercise 5.)*
- **Transaction identifiers are not unique across coordinator restarts.** The txid counter is
  volatile; a restarted coordinator reuses identifiers, and a new transaction's ABORT could
  then wrongly release an unrelated in-doubt lock — a genuine safety defect, found during
  development. Persistent or consensus-allocated txids repair it. *(→ Exercise 3.)*
- **Whole-node locking.** One in-doubt transaction at a time per participant; real systems lock
  at item granularity, admitting concurrent disjoint transactions.
- **Sequential Phase 1** (an optimization, not a correctness issue); no presumed-abort /
  presumed-commit log optimizations; no coordinator-recovery protocol to re-adopt in-doubt
  participants after a restart.

## 9. Exercises

1. **(In-doubt reasoning.)** In the execution of §4.1, suppose an in-doubt participant
   unilaterally aborts after a timeout. Construct the completion of the execution that violates
   NBAC1. Then explain why a timeout on the *coordinator's* side (aborting when a vote is slow)
   is, by contrast, always safe.
2. **(Presumed abort.)** In industrial 2PC, a coordinator that finds no record of a txid answers
   ABORT ("presumed abort"), letting it forget aborted transactions. Specify precisely which log
   writes this removes, and re-verify NBAC1/NBAC4 under coordinator crash-recovery.
3. **(Identifier discipline.)** Demonstrate the txid-collision defect against the current code
   (two coordinator sessions), then repair it (persist the counter, or derive txids from a
   durable epoch) and re-run the demonstration.
4. **(Termination protocol.)** Add a participant↔participant channel and implement cooperative
   termination: an in-doubt participant polls its peers; if any has decided, it adopts that
   decision; if any has *not voted*, all may abort. Which executions of §4.1 does this rescue,
   and which (all participants in doubt) remain blocked?
5. **(Paxos Commit, on paper.)** Design — at the level of messages and state — atomic
   commitment for this module's transfer using Module 05's Raft as a service: where do votes
   live, who proposes the outcome, and why does a coordinator crash no longer block? Compare
   message counts with plain 2PC in the failure-free case.
6. **(2PL.)** Give a two-transaction, two-account schedule admitted if participants release
   their reservation immediately after voting (violating strict 2PL) that is not
   conflict-serializable, and verify the current implementation refuses it.

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: non-blocking atomic commitment
  (§6.1); consensus (Ch. 5); failure detectors P vs. ◇P (§2.6). ISBN 978-3-642-15259-7.

**Atomic commitment**
- J. Gray, *Notes on Data Base Operating Systems*, in *Operating Systems: An Advanced Course*,
  Springer LNCS 60, 1978. (2PC.)
- D. Skeen, *Nonblocking Commit Protocols*, SIGMOD 1981. (Blocking analysis; 3PC.)
- P. Bernstein, V. Hadzilacos, N. Goodman, *Concurrency Control and Recovery in Database
  Systems*, Addison-Wesley, 1987. (2PC, 2PL, recovery; freely available online.)
- J. Gray, L. Lamport, *Consensus on Transaction Commit*, ACM TODS 31(1), 2006. (Paxos Commit.)

**Transactions and isolation**
- T. Härder, A. Reuter, *Principles of Transaction-Oriented Database Recovery*, ACM Computing
  Surveys 15(4), 1983. (ACID.)
- J. Gray, A. Reuter, *Transaction Processing: Concepts and Techniques*, Morgan Kaufmann, 1993.
- C. H. Papadimitriou, *The Serializability of Concurrent Database Updates*, JACM 26(4), 1979.

**Systems**
- J. C. Corbett et al., *Spanner: Google's Globally-Distributed Database*, OSDI 2012.
  (2PC over Paxos groups.)

---

## Running the code

```bash
cargo build
```

Start two participants (accounts, initial balance 100):
```bash
cargo run -- participant 6000
cargo run -- participant 6001
```
Drive them with a coordinator (commands on stdin):
```bash
cargo run -- coordinator 127.0.0.1:6000 127.0.0.1:6001
transfer -30 30        # both can perform their part → COMMIT
transfer -150 150      # p₁ cannot (overdraft) → NO → ABORT; neither balance changes
transfer-crash -30 30  # coordinator stops after Phase 1 → participants blocked in doubt
```
Per-participant state persists in `2pc-<port>.state`. The `demos/` scripts (`happy.py`,
`abort.py`, `persistence.py`, `blocking.py`) reproduce §3–§4 end to end.

---
*[Course home](../) · Previous: [Module 05](../05-raft/) · Next: Module 07 — Byzantine Reliable
Broadcast (planned) · Theory maps: [CONSENSUS.md](../05-raft/CONSENSUS.md) ·
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md)*
