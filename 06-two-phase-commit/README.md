# 06 — Two-Phase Commit

A working implementation of **Two-Phase Commit (2PC)** — the classic **atomic commit** protocol — and
a demonstration of its famous **blocking flaw**. Where `05` made a set of *replicas* agree on one log
(consensus), this project makes a set of *different* nodes — **partitions**, each holding its own data
— commit a single transaction **all-or-nothing**. A coordinator asks every participant "can you do
your part?"; only if **all** say YES does the transaction commit; a single NO (or crash) aborts
everyone. The domain is a **bank transfer**: move $30 from account A to account B, atomically, when A
and B live on different machines.

The point of building it, right after Raft, is to make one sentence concrete and unforgettable:

> **Consensus ≠ atomic commit.** Raft (majority) keeps making progress when a minority crashes. 2PC
> (unanimity, one coordinator) **grinds to a permanent halt** when the coordinator dies at the wrong
> moment. You can watch it wedge on your own terminal (`demos/blocking.py`).

> **Companion theory maps:** [`CONSENSUS.md`](../05-raft/CONSENSUS.md) (why 2PC ≠ consensus, in the
> impossibility landscape) and [`CONSISTENCY_AND_CONCURRENCY.md`](../CONSISTENCY_AND_CONCURRENCY.md)
> (linearizability vs serializability, 2PL, the mechanisms). This README is about *2PC, the protocol
> we built*.

---

## Theory — atomic commit

### 1. The problem: atomic commit ≠ replication

A transaction can span **multiple** data items on **multiple** machines: a transfer *debits* A **and**
*credits* B. These are **partitions** (shards) — different data, one copy each — not **replicas** (the
same data copied for fault tolerance, as in `03`/`05`). The requirement is **atomicity**: either
*both* the debit and the credit happen, or *neither* does. No node can see the others' data, so a
**coordinator** orchestrates a single all-or-nothing decision across them.

| | Replication (`03`, `05`) | Partitioning + atomic commit (`06`) |
|---|---|---|
| each node holds | the **same** data (a copy) | **different** data (a shard) |
| goal | fault tolerance (survive a node dying) | atomicity of a multi-shard operation |
| decision rule | **majority** (quorum) | **unanimity** (every participant) |
| one node dying | tolerated (others have the data) | **fatal to atomicity** (its shard is unique *and* its vote is required) |

### 2. Non-Blocking Atomic Commit — the specification (CCGR §6.1)

The abstraction 2PC *tries* to implement. Each participant proposes a **vote** (YES/commit or
NO/abort); all must **decide** one common outcome (COMMIT or ABORT):

- **Agreement** — no two participants decide differently.
- **Commit-Validity** — the decision is COMMIT **only if all participants voted YES**.
- **Abort-Validity** — the decision is ABORT **only if some participant voted NO or is faulty**.
- **Termination** — every correct participant **eventually decides**. *(The "non-blocking" clause.)*

2PC gets Agreement + both Validity properties right. **It fails the Termination clause** under
coordinator failure — which is the whole story of this project. So 2PC implements *atomic commit*, but
**not** *non-blocking* atomic commit.

### 3. The protocol — two phases

```
                COORDINATOR                         PARTICIPANTS (star topology; they never talk to each other)
  Phase 1   ── PREPARE <txid> <delta> ─────────►    each: can I apply my delta AND am I free?
  (voting)                                             YES → durably log "prepared", LOCK, become in-doubt
            ◄──────── VOTE <txid> YES|NO ─────         NO  → refuse
                                                    
            decide: COMMIT iff EVERY vote is YES
                    ABORT if any NO / any unreachable
  Phase 2   ── COMMIT | ABORT <txid> ────────────►    COMMIT → apply delta, release lock
  (decision)                                            ABORT  → discard reservation, release lock
            ◄──────────── ACK <txid> ──────────────
```

The key state is a participant's **in-doubt** window: from the moment it votes YES until it hears the
verdict, it has made a **binding promise** it cannot take back, and it **holds a lock** (refuses other
transactions). A YES is not an opinion — it is a guarantee the coordinator may already have acted on.

### 4. The blocking flaw (2PC's fatal weakness)

If the coordinator crashes **after** collecting votes but **before** broadcasting the verdict, every
participant that voted YES is stranded **in-doubt**:

- It **cannot decide alone.** COMMIT might contradict an ABORT the coordinator already sent someone;
  ABORT might contradict a COMMIT. Either guess can violate Agreement.
- It **cannot ask its peers** — the topology is a **star**; participants have no channel to each other
  and never saw the other votes (only the coordinator did).
- It **cannot escape by restarting** — the in-doubt lock is *durable* (§stable storage), so a reboot
  brings it back *still in-doubt*. Persistence buys **safety**, not **liveness**.

So the lock is held **forever**, and every future transaction touching that data is refused. The
cluster is wedged. See `demos/blocking.py`. This is exactly the **Termination** clause of NBAC (§2)
failing.

### 5. Why *consensus ≠ atomic commit* (and why NBAC is, in one sense, harder)

They look similar (both "agree on one value") but differ on two axes:

- **Decision function.** Consensus may decide *any proposed* value. Atomic commit's outcome is a
  *function of the votes* — COMMIT **iff all YES** (a logical AND). One NO forces ABORT.
- **Fault tolerance / quorum.** Consensus tolerates a minority of crashes (majority still decides —
  `05`). Atomic commit needs *every* participant's YES; one crash forces ABORT, and a *coordinator*
  crash **blocks**.

The sharp, formal way to say it (CCGR): **NBAC requires a *perfect* failure detector P**, whereas
consensus needs only an *eventually perfect* one (◇P / Ω). Why? Because Abort-Validity ties the
decision to whether a participant **crashed** — and to decide COMMIT you must be *sure* nobody has
crashed, which only P (never a false suspicion, never a missed crash) can tell you. Consensus can
tolerate the false suspicions of ◇P because it doesn't have to distinguish "slow" from "dead" to be
*safe*. In the failure-detector hierarchy, **NBAC sits *above* consensus.**

### 6. The fixes (out of scope, but where the road leads)

- **3PC (three-phase commit).** Adds a "pre-commit" phase so a stranded participant can run a
  *termination protocol* (ask peers) and decide. Non-blocking **under synchrony + no partitions** —
  but unsafe under network partitions, so rarely used in practice.
- **Paxos Commit (Gray & Lamport, 2006).** The real fix: replace the single fragile coordinator with
  a **consensus group**, and run the *commit decision itself* through consensus (`05`). Now no single
  failure strands anyone. This is literally **2PC layered on top of Raft/Paxos** — which is how
  Spanner and CockroachDB do cross-shard transactions: each shard is a Raft group (replication), and
  a multi-shard transaction runs 2PC over the shard-leaders (atomic commit).

---

## Deep dive — locking, 2PL, and what `prepared` really is

The participant's `prepared` state is not just a flag; it is a **lock**, and 2PC is doing **Two-Phase
Locking** across machines. This section connects the code to the transaction-isolation theory (fuller
treatment in [`CONSISTENCY_AND_CONCURRENCY.md §6`](../CONSISTENCY_AND_CONCURRENCY.md)).

### Growing and shrinking phases

**Two-Phase Locking (2PL)** — the discipline that guarantees **serializability** (specifically
*conflict*-serializability) — splits a transaction's lifetime by one rule:

- **Growing phase** — may **acquire** locks, may **not release** any.
- **Shrinking phase** — may **release** locks, may **not acquire** any new ones.

Once you release your first lock you can never grab another, so there is a single **peak** where the
transaction holds *all* its locks at once — that peak is its **serialization point**, which is what
makes the schedule equivalent to a serial order. *(Unlucky naming: 2P**L**'s two phases are unrelated
to 2P**C**'s prepare/commit phases.)*

### Plain vs Strict vs Rigorous 2PL

| Variant | Releases locks… | Prevents |
|---|---|---|
| **Plain 2PL** | as soon as done with an object (but no acquire after any release) | non-serializable schedules — but still allows **cascading aborts** / non-recoverable schedules (a peer can read an uncommitted write) |
| **Strict 2PL** | holds all **write (exclusive)** locks until **commit/abort**, then releases | cascading aborts; guarantees **recoverable, cascadeless** schedules |
| **Rigorous 2PL** | holds **all** locks (read *and* write) until commit/abort | same, and simplest to reason about — the common industrial choice |

### `prepared` **is** Strict 2PL across machines

Map the code onto the theory:

- **Acquire (growing)** — voting `VOTE YES` sets `prepared = Some((txid, delta))`: the participant
  **locks** its account. It refuses any other transaction while holding it (`prepared.is_none()` guard).
- **Hold** — it keeps the lock from the YES vote all the way through the transaction, releasing
  **nothing** early.
- **Release (shrinking, at the very end)** — `COMMIT`/`ABORT` clears `prepared`: locks released **at
  commit/abort time**, all at once.

That "hold the exclusive lock until the verdict" is exactly **Strict 2PL** — and it is *why* no reader
can observe a half-applied transfer (isolation), *and* why a dead coordinator (verdict never comes)
holds the lock **forever**. **The blocking flaw is a stuck 2PL lock:** the shrinking phase never
happens.

---

## How this project evolved — one problem at a time

| # | We built… | …which exposed |
|---|---|---|
| **M1** | **happy path** — coordinator drives PREPARE → collect votes → COMMIT; participants apply | a single-participant tx wouldn't need any of this — the hard part is *multiple* shards |
| **M1½** | **abort / atomicity** — one NO (insufficient funds) vetoes everyone; a YES-voter still doesn't apply | the YES vote is a *promise*, and a promise costs a **lock** held until the verdict |
| **M2** | **durability** — fsync `balance` + the in-doubt `prepared` **before every reply**; reload on restart | a durable lock is **safe** (never breaks a promise) but now **can't be forgotten to escape blocking** |
| **M3** | **the blocking flaw** — `transfer-crash` kills the coordinator after PREPARE | 2PC is **safe but not live**: the stranded lock is held forever — *consensus would have survived this* |

The arc: **commit** (M1) → **veto & lock** (M1½) → **make the lock durable** (M2) → **watch it block**
(M3).

---

## How the code reflects the theory — and where it stops

| Theory | In this code |
|---|---|
| atomic commit over partitions | coordinator sends a per-participant `delta`; `deltas[i]` → `participants[i]` |
| Commit-Validity (unanimity / AND) | `all_yes` — COMMIT only if **every** reply is exactly `VOTE <txid> YES` |
| Abort-Validity (a NO or a crash aborts) | a NO vote *or* an unreachable participant (`send` → `None`) ⇒ not-YES ⇒ ABORT |
| in-doubt / Strict 2PL lock | `prepared: Option<(u64, i64)>`; set on YES, held until verdict, cleared on COMMIT/ABORT |
| durability / stable storage (CCGR §2.2.4) | `persist()` fsyncs `balance` + `prepared` **before every reply**; `load()` reloads on startup |
| the blocking flaw | `transfer-crash` reaches "votes collected, no verdict" and stops → participants wedged |

**Honest limits — the syllabus beyond this project (each a signpost):**

- **The coordinator is a single point of failure (by design).** It is neither replicated nor
  persistent — that *is* the blocking flaw we set out to show. The fix is **Paxos Commit** (§6): run
  the decision through consensus (`05`). *(→ fault-tolerant coordinator.)*
- **No participant timeout / termination protocol.** A stranded participant waits **forever**; it
  never times out to ask peers or presume-abort. Real systems add timeouts + a cooperative
  termination protocol (and **presumed-abort/presumed-commit** log optimizations). *(→ 3PC,
  termination protocols.)*
- **Transaction ids aren't globally unique across coordinator restarts.** `txid` is an in-memory
  counter, so a *restarted* coordinator reuses ids — and a new tx's `ABORT` could then wrongly match
  (and release) an old in-doubt lock: a real safety hole. The fix is a persistent/coordinated txid
  (which Paxos Commit gets for free from the consensus layer). *(→ globally-unique txids.)*
- **Coarse, whole-node locking.** A participant holds *one* in-doubt tx at a time (one lock for the
  whole account), so concurrent transactions serialize hard. Real systems lock at row granularity.
  *(→ fine-grained locking.)*
- **Sequential Phase 1.** The coordinator sends PREPAREs one participant at a time; a real one fans
  them out concurrently. *(An optimization, not a correctness gap.)*
- **No recovering-coordinator re-attach.** A restarted coordinator has no "who is in-doubt?" query to
  reconnect to stranded participants and finish the job. *(→ coordinator recovery.)*

---

## Run

```bash
cargo build
```

Start a **2-node cluster** (two bank branches, each starting at $100):
```bash
cargo run -- participant 6000
cargo run -- participant 6001
```
Then drive them with a coordinator (reads commands from stdin):
```bash
cargo run -- coordinator 127.0.0.1:6000 127.0.0.1:6001
transfer -30 30        # move $30 from p0 to p1 → COMMIT (both can afford)
transfer -150 150      # p0 can't afford → NO → ABORT (nobody applies)
transfer-crash -30 30  # coordinator dies after PREPARE → participants wedged in-doubt
```
`transfer <d0> <d1> …` maps `deltas[i]` to `participants[i]`; they should sum to zero (money is
conserved). `transfer-crash` is a test hook that runs Phase 1 then stops — modelling a coordinator
crash at the worst moment.

**Wire protocol** (newline-framed):

| Message | Direction | Meaning |
|---|---|---|
| `PREPARE <txid> <delta>` → `VOTE <txid> YES\|NO` | coordinator → participant | Phase 1: can you apply your delta (and are you free)? |
| `COMMIT <txid>` / `ABORT <txid>` → `ACK <txid>` | coordinator → participant | Phase 2: the unanimous verdict |

**Demos** (`demos/`) drive a real cluster over TCP:

| Script | Shows |
|---|---|
| `happy.py` | all YES → **COMMIT**; both apply (100→70, 100→130) |
| `abort.py` | one NO → **ABORT**; a YES-voter locks but does **not** apply (atomicity) |
| `persistence.py` | committed state **and** the in-doubt lock survive a crash; a reboot doesn't unblock |
| `blocking.py` | coordinator dies after PREPARE → participants **wedged in-doubt forever** |

## Design & notable implementation details

- **A participant is passive and sequential** — no `Arc<Mutex>` (unlike `05`). It has no timer, no
  initiative; it only reacts to the coordinator, one message at a time, so a plain `for conn in
  listener.incoming()` loop over local `balance`/`prepared` variables is race-free. That passivity is
  itself the reason a stranded participant can't self-rescue.
- **The star topology is the whole story.** Every message comes from the coordinator; participants
  never message each other. Centralizing the decision keeps each participant trivial — and makes the
  coordinator's death unrecoverable.
- **Persist before you externalize.** `fsync` the promise *before* replying YES, the new balance
  *before* acking COMMIT — otherwise a crash could break a promise the coordinator already relied on.
- **`send` returning `None` = a NO.** An unreachable participant can't confirm YES, so treating a
  connection failure as not-a-yes is exactly Abort-Validity: uncertainty ⇒ abort.

## What I learned

*Rust:* a passive single-threaded TCP server (no shared-state locking needed — a nice contrast with
`05`); `Option<(u64, i64)>` as a durable lock; slice-pattern command parsing (`["PREPARE", txid,
delta]`, `deltas @ ..`); `str::parse` with `if let (Some(Ok(..)), ..)`; fsync via `File::create` +
`sync_all()`; and reading state back on startup.

*Distributed systems:* **atomic commit** vs **consensus** (unanimity/AND + perfect FD vs
majority + ◇P); the **NBAC** properties and which one 2PC breaks (**Termination**); the **in-doubt**
window as a **Strict 2PL** lock (acquire on YES, hold to the verdict, release at commit/abort); why
**durability strengthens blocking** rather than curing it; **partitioning vs replication**; and the
fix — **Paxos Commit** = atomic commit *over* consensus.

---

## References

**Course reference text**
- Christian Cachin, Rachid Guerraoui & Luís Rodrigues, *Introduction to Reliable and Secure
  Distributed Programming*, 2nd ed., Springer, 2011. For `06`: **Non-Blocking Atomic Commit** (§6.1),
  **consensus** (Ch. 5), **failure detectors** P vs ◇P (§2.6). ISBN 978-3-642-15259-7.

**Atomic commit**
- Jim Gray, *Notes on Data Base Operating Systems*, 1978. The original **two-phase commit** protocol.
- Philip Bernstein, Vassos Hadzilacos & Nathan Goodman, *Concurrency Control and Recovery in Database
  Systems*, Addison-Wesley, 1987. 2PC, 3PC, 2PL, recovery — the canonical text (freely online).
- Dale Skeen, *Nonblocking Commit Protocols*, ACM SIGMOD 1981. Why 2PC blocks; **three-phase commit**.
- Jim Gray & Leslie Lamport, *Consensus on Transaction Commit*, ACM TODS 31(1), 2006. **Paxos
  Commit** — the non-blocking fix that runs the decision through consensus.

**Isolation & locking (see also `CONSISTENCY_AND_CONCURRENCY.md`)**
- Jim Gray & Andreas Reuter, *Transaction Processing: Concepts and Techniques*, 1993. ACID, 2PL.
- C. H. Papadimitriou, *The Serializability of Concurrent Database Updates*, JACM 1979.

---
Part of [distributed-systems-in-rust](../).  ·  Theory maps: [CONSENSUS.md](../05-raft/CONSENSUS.md) · [CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md)
