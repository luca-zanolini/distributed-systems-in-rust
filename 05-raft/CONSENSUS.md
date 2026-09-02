# Consensus — a map

Everything in this repo has been climbing toward one problem: **getting a set of machines to agree**,
despite failures and no shared clock. This document is the map — *when agreement is possible, why,
and what it costs* — with pointers to where each idea is built in the projects. It is deliberately
theory-first; the *code* for consensus lives in [`05-raft`](README.md).

Primary inspiration and further reading: **Decentralized Thoughts — "Consensus Cheat Sheet"**
(<https://decentralizedthoughts.github.io/2021-10-29-consensus-cheat-sheet/>), and Cachin,
Guerraoui & Rodrigues, *Introduction to Reliable and Secure Distributed Programming*, 2nd ed. (2011)
— "CCGR".

---

## 1. The problem

**Consensus:** each process proposes a value; all must **decide** on one common value. The
specification (CCGR §5.1) is four properties:

- **Termination** *(liveness)* — every correct process eventually decides.
- **Validity** — a decided value was proposed by some process.
- **Integrity** — a process decides at most once.
- **Agreement** *(safety)* — no two correct processes decide differently.
  - **Uniform agreement** strengthens this: *no two processes decide differently, even one that later
    crashes.* Raft gives uniform agreement — a committed entry never diverges, even on a node that
    crashes right after committing.

Split it the CCGR way: **safety** (Agreement, Validity, Integrity — *nothing bad happens*) vs.
**liveness** (Termination — *something good eventually happens*). Almost every impossibility and
every "eventually" below is really a statement about **liveness**; safety is (almost) always
preserved.

---

## 2. Impossibility — FLP

> **FLP (Fischer–Lynch–Paterson, 1985):** in a fully **asynchronous** system, there is **no
> deterministic** algorithm that solves consensus if even **one** process may crash.

The intuition: with no timing bounds you can't distinguish a **crashed** process from a merely
**slow** one, so any protocol can be forced (by an adversarial scheduler) into an infinite run that
never decides. Crucially, FLP kills **liveness**, not safety — you can't be *stuck-free*, not *wrong*.

Three ways to circumvent it — you must weaken the async model somehow:

1. **Partial synchrony** (add *some* timing) — Paxos, Raft, PBFT.
2. **Randomization** (coin flips break the adversary's grip) — Ben-Or 1983; randomized BFT.
3. **Failure detectors** (an oracle about crashes) — Chandra–Toueg 1996.

**Fault-model caveat.** Partial synchrony and randomization are **fault-agnostic** — they weaken the
async adversary *itself*, so they work for **crash *and* Byzantine** (DLS covers Byzantine partial
synchrony; Ben-Or has a Byzantine variant). **Failure detectors, however, are a crash-fault tool
only:** a Byzantine process is *up and lying*, not stopped, so it **evades detection** — a malicious
node behaves correctly toward the detector and strikes elsewhere (CCGR §2.6.1 explicitly declines FDs
for Byzantine). The missing info for crash-FLP is *"is it alive?"* (a FD supplies it); for Byzantine
it's *"is it honest?"* — undetectable, so you don't detect it, you handle it **structurally**: bigger
quorums (`3f+1`, honest intersection) **+ cryptographic signatures**. The *leader-election* role does
survive into BFT, but only as a **specialized** Byzantine leader-detector / **view-change** (CCGR
§2.6.6) — driven by timeouts + algorithm-specific monitoring, **not** a generic crash detector.

These three are the same escape wearing different clothes (see §4).

---

## 3. Timing models

The single most important axis. What can you assume about message delay and relative process speed?

| Model | Assumption | Consensus? |
|---|---|---|
| **Synchronous** | known upper bounds on delay + step time | solvable, even simply |
| **Partially synchronous** | bounds exist but are **unknown**, or hold only **after an unknown GST** (Global Stabilization Time) | solvable with a majority (Paxos/Raft) |
| **Asynchronous** | no bounds at all | **impossible deterministically** (FLP) |

**Partial synchrony** (Dwork–Lynch–Stockmeyer, 1988) is the sweet spot real systems assume: the
network is *usually* timely, occasionally not. Protocols keep **safety always**, and regain
**liveness after GST** (once messages start arriving within the timeout). Raft's randomized election
timeout is exactly this: before GST, timeouts may misfire (split votes, extra elections — never a
lost commit); after GST, one stable leader emerges. *(Built in `04`'s failure detector; `05`'s
election.)*

---

## 4. The failure-detector lens (equivalent to timing)

You can package "how much synchrony you have" as a **failure detector** — an oracle that reports
suspected crashes, letting an algorithm avoid explicit clocks (CCGR §2.6). This is a *second lens on
the same resource*:

| Timing model | Detector you can build | Symbol |
|---|---|---|
| Synchronous | **perfect** — never wrong | **P** |
| Partially synchronous | **eventually perfect** / eventual leader | **◇P / Ω** |
| Asynchronous | none strong enough | — |

- **P** never suspects a correct process (strong accuracy) — only synchrony can guarantee that.
- **◇P / Ω** are *eventually* accurate: wrong for a while, correct after GST. **Ω** ("eventual
  leader") outputs one process that *eventually* all correct nodes agree is up.
- **The bridge theorem (Chandra–Hadzilacos–Toueg, 1996): Ω is the *weakest* failure detector that
  solves consensus.** So *consensus is solvable ⟺ you can implement Ω ⟺ you have (at least) partial
  synchrony.* The timing lens and the detector lens draw the **same** boundary.

> Slogan: **synchrony → P; partial synchrony → Ω; asynchrony → nothing (FLP).** Detectors are
> *synchrony, packaged as an oracle.* (Expanded in [`04`'s README](../04-leader-election/README.md).)

---

## 5. Fault models — this sets the *quorum size*

Timing decides *solvability*; the **fault model** decides *how many nodes you need* (CCGR §2.2):

- **Crash-stop** — a process halts and never returns. (Paxos, Raft.)
- **Crash-recovery** — halts and later recovers, losing volatile state (needs stable storage).
- **Byzantine / arbitrary** — a process may do *anything*: lie, equivocate, collude. (PBFT, HotStuff,
  blockchains.)

The jump from crash to Byzantine is the jump from **"trust the messages"** to **"verify everything"**
— and it changes the arithmetic.

---

## 6. Quorums — the arithmetic of agreement

A **quorum** is a set large enough that any two quorums **intersect**. That overlap is what carries a
decision forward: the shared node "remembers." (CCGR §2.7.3.)

- **Crash faults — majority.** With `N = 2f+1` nodes tolerating `f` crashes, a quorum is `f+1` (a
  **majority**). Any two majorities share **≥ 1** node, which remembers the last decision. *(This is
  every quorum in `03`, `04`, and `05`.)*
- **Byzantine faults — supermajority.** Now the shared node must be **honest** (a Byzantine one could
  lie to each side). Two constraints:
  - **availability:** you can only wait for `N − f` replies (`f` may be silent) → `Q ≤ N − f`;
  - **honest intersection:** two quorums share `≥ 2Q − N` nodes, and that must exceed `f` → `2Q − N > f`.
  - Together: `N > 3f`, so **`N = 3f+1`**, quorum **`Q = 2f+1`** (a **>⅔ supermajority**). This is
    where Ethereum's Casper FFG and every PoS BFT chain live.

**The full picture (timing × fault):**

| | **Synchronous** | **Partially synchronous** |
|---|---|---|
| **Crash** | `f+1` (timeouts detect crashes; majority not even needed for safety) | `2f+1` — **majority** (Paxos, Raft) |
| **Byzantine** | `f+1` *with signatures* (Dolev–Strong); `3f+1` without | `3f+1` — **>⅔**, even with signatures (DLS lower bound) |

Two reads of this table: **majority is the price of not being able to tell *dead* from *slow*** (the
partial-sync crash cell); **>⅔ is the price of not being able to tell *honest* from *lying*** (the
Byzantine cell).

---

## 7. Protocol families & round structure

All leader-based consensus shares a skeleton: **(1) establish leadership** for an epoch, **(2)
replicate/commit** a value. What differs is the number of rounds and the quorum.

| | **(Multi-)Paxos** | **Raft** | **PBFT** | **HotStuff** |
|---|---|---|---|---|
| Fault model | crash | crash | Byzantine | Byzantine |
| Leadership phase | Prepare (per ballot) | election (per term) | view / view-change | leader per view |
| Replication phase | Accept | AppendEntries | pre-prepare → prepare → commit | pipelined 3-chain |
| Round-trips **/ command** (steady state) | **1** | **1** | **2** all-to-all | **1** (pipelined), linear msgs |
| Quorum | majority | majority | `2f+1` of `3f+1` | `2f+1` of `3f+1` |

- **Raft ≈ Multi-Paxos** with a strong leader: elect once per term, then each command commits in **one
  majority round-trip** (`AppendEntries` → majority ack). *(That's exactly `05`.)*
- **PBFT** (Castro–Liskov, 1999) needs a **third phase**: after *prepare* (agree on ordering in this
  view), the *commit* phase makes the decision **survive a view-change** despite a lying leader —
  lifting "I know it's agreed" to "I know enough others know." Plus `>⅔` quorums and signatures.
- **HotStuff** (Yin et al., 2019) makes BFT **linear** (leader-to-all, threshold signatures) and
  **pipelined** — the basis of modern PoS chains (Tendermint, DiemBFT, Ethereum-adjacent designs).

> The extra Byzantine round and the bigger quorum are the *same* fact from two angles: you can't
> trust a single message, so you need one more round of cross-checking and one more slice of the
> cluster in every quorum.

---

## 8. Atomic commit is *not* consensus (2PC)

A frequent trap: **two-phase commit (2PC)** looks like consensus but solves a *different* problem —
**atomic commit** (a transaction is all-or-nothing across participants), and it does so **unsafely
under failure**:

- **2PC** (Gray, 1978): coordinator asks all participants to *prepare* (vote), then *commit* if all
  voted yes. **Blocking:** if the coordinator crashes at the wrong moment, participants are **stuck**
  holding locks — it does **not** tolerate coordinator failure. Atomic commit even requires *every*
  participant to agree (unanimity), unlike consensus's *majority*.
- **3PC** (Skeen, 1981): adds a phase to be non-blocking **under synchrony** — but breaks under
  network partitions.
- **Paxos Commit** (Gray & Lamport, 2006): run the commit *decision itself through consensus* → a
  **fault-tolerant** atomic commit. The clean fusion of the two ideas.

> **Consensus** (Paxos/Raft/PBFT) is **non-blocking and fault-tolerant**; **2PC** is **blocking and
> not**. Don't conflate 2PC's two phases with consensus's two phases — they solve different problems
> with opposite failure behavior. *(2PC is a Stage-4 project in the roadmap; BFT is Stage-5.)*

---

## 9. CAP — the design fork behind all of it

**CAP (Brewer 2000; Gilbert & Lynch 2002):** under a network **P**artition you must choose
**C**onsistency *or* **A**vailability. Consensus systems (etcd, Spanner, and our `03`/`05`) choose
**CP** — they **refuse** to make progress without a quorum rather than risk disagreement. `03` made
this choice visible (it returns `ERR no quorum`); Raft inherits it (no leader without a majority).

---

## 10. Where the repo sits on this map

| Project | On this map |
|---|---|
| `01`, `02` | pre-consensus: a register, then over the network |
| `03` | the **(1,N) majority register** — quorum reads/writes; **CP**; crash faults |
| `04` | **failure detection ◇P + eventual leader Ω** — partial synchrony, packaged |
| `05` | **crash consensus (Raft)** — Ω-driven, majority quorums, uniform agreement |
| **next** | **2PC / atomic commit** (Stage 4); **Byzantine consensus** (Stage 5) — the `3f+1`, `>⅔` world of your Ethereum work |

---

## References (verify every venue/year — it is 2026)

**The problem & impossibility**
- M. Fischer, N. Lynch, M. Paterson, *Impossibility of Distributed Consensus with One Faulty Process*,
  JACM 1985. (FLP.)
- C. Dwork, N. Lynch, L. Stockmeyer, *Consensus in the Presence of Partial Synchrony*, JACM 1988.
  (Partial synchrony / GST.)

**Failure detectors**
- T. Chandra, S. Toueg, *Unreliable Failure Detectors for Reliable Distributed Systems*, JACM 1996.
- T. Chandra, V. Hadzilacos, S. Toueg, *The Weakest Failure Detector for Solving Consensus*, JACM 1996.
  (Ω is weakest.)

**Crash consensus**
- L. Lamport, *The Part-Time Parliament*, ACM TOCS 1998; *Paxos Made Simple*, 2001.
- D. Ongaro, J. Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*, USENIX ATC 2014.
- M. Ben-Or, *Another Advantage of Free Choice*, PODC 1983. (Randomized consensus.)

**Byzantine consensus**
- L. Lamport, R. Shostak, M. Pease, *The Byzantine Generals Problem*, ACM TOPLAS 1982; and
  M. Pease, R. Shostak, L. Lamport, *Reaching Agreement in the Presence of Faults*, JACM 1980. (`3f+1`.)
- D. Dolev, H. Strong, *Authenticated Algorithms for Byzantine Agreement*, SIAM J. Computing 1983.
- M. Castro, B. Liskov, *Practical Byzantine Fault Tolerance*, OSDI 1999. (PBFT.)
- M. Yin, D. Malkhi, M. Reiter, G. Gueta, I. Abraham, *HotStuff: BFT Consensus with Linearity and
  Responsiveness*, PODC 2019.
- E. Buchman, J. Kwon, Z. Milosevic, *The Latest Gossip on BFT Consensus*, 2018. (Tendermint.)

**Atomic commit & CAP**
- J. Gray, *Notes on Data Base Operating Systems*, 1978. (2PC.)  ·  D. Skeen, *Nonblocking Commit
  Protocols*, SIGMOD 1981. (3PC.)
- J. Gray, L. Lamport, *Consensus on Transaction Commit*, ACM TODS 2006. (Paxos Commit.)
- S. Gilbert, N. Lynch, *Brewer's Conjecture and the Feasibility of Consistent, Available,
  Partition-Tolerant Web Services*, ACM SIGACT News 2002. (CAP.)

**Textbook & survey**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer 2011. (CCGR.)
- Decentralized Thoughts, *Consensus Cheat Sheet*, 2021.
  <https://decentralizedthoughts.github.io/2021-10-29-consensus-cheat-sheet/>

---
Part of [distributed-systems-in-rust](../).  ·  Implementation: [`05-raft`](README.md).
