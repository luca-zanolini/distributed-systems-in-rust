# Consensus — Lecture Notes

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Companion:
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md). The implementation this
document accompanies is [Module 07 (Raft)](README.md); it is also referenced from Modules 04,
04, and 06.*

**Abstract.** These notes map the theory of distributed agreement: the consensus specification;
the FLP impossibility and the two principled ways around it; timing models and their
failure-detector counterparts; how the fault model sets quorum sizes (majority for crash
faults, supermajority for Byzantine); the structure shared by the major protocol families
(Paxos, Raft, PBFT, HotStuff); the relationship — and the sharp differences — between consensus
and atomic commitment; and the CAP trade-off. Each concept is annotated with the module of this
course in which it is built. Sources: CCGR (Cachin–Guerraoui–Rodrigues, 2nd ed., 2011) and the
primary literature cited in §11; the Decentralized Thoughts "Consensus Cheat Sheet" is a useful
informal companion.

---

## 1. The problem

**Specification (consensus; CCGR Ch. 5).** Each process proposes a value; processes decide
values subject to:

- **C1 (Termination — liveness).** Every correct process eventually decides some value.
- **C2 (Validity — safety).** A decided value was proposed by some process.
- **C3 (Integrity — safety).** No process decides more than once.
- **C4 (Agreement — safety).** No two correct processes decide differently.

**Uniform consensus** strengthens C4 to **all** processes: no two processes decide
differently, even if one subsequently crashes. The distinction is not pedantic: an algorithm
may let a process decide and crash, its decision contradicted afterwards — permitted by C4,
forbidden by uniform agreement. Raft provides the uniform property (a committed entry binds
every future of the system).

The safety/liveness split organizes everything that follows: the impossibility results and
every "eventually" concern **liveness**; correctly designed protocols never trade **safety**
for them.

## 2. Impossibility: FLP

**Theorem (Fischer–Lynch–Paterson 1985).** In a fully asynchronous system, no deterministic
algorithm solves consensus if even one process may crash.

The mechanism of the proof matters more than its statement: with no bounds on message delay or
relative speed, a crashed process cannot be distinguished from a slow one, and an adversarial
scheduler can forever maintain the ambiguity, postponing any decision indefinitely. FLP is a
*liveness* impossibility — the adversary prevents termination, never forces a wrong decision.

Circumventing FLP means weakening the model, and there are two principled routes:

**Route 1 — strengthen the timing assumptions (partial synchrony).** Assume delay bounds that
hold *eventually* (§3). This power can be consumed in two equivalent forms:
- *directly*, as timeouts written into the protocol (Paxos, Raft, PBFT); or
- *modularly*, through the **failure-detector** interface (§4): prove the algorithm against an
  oracle's axioms, and implement the oracle from timeouts separately.

A failure detector is not a way of defeating asynchrony — the useful detectors are
unimplementable in a fully asynchronous system, by FLP itself. It is an *interface* to the
timing escape. Partial synchrony is **sufficient** to implement the eventual leader detector Ω;
the converse fails — Ω is implementable under strictly weaker assumptions (e.g., a single
eventually timely source; Aguilera et al.), so the relation is one-directional. The payoff of
the interface is modularity (CCGR §2.6.1): a clock-free safety proof with all timing
quarantined inside the detector's implementation.

**Route 2 — randomization.** Randomized algorithms (Ben-Or 1983; modern asynchronous BFT such
as HoneyBadgerBFT) terminate with probability 1 while keeping the fully asynchronous model.
This is the only route that retains full asynchrony; FLP forbids only *deterministic*
solutions.

**Fault-model note.** Both routes exist for crash *and* Byzantine faults (DLS treats Byzantine
partial synchrony; Ben-Or has Byzantine variants). The failure-detector *packaging*, however,
is crash-only: a Byzantine process is operational and deviating, not silent, so it evades any
crash detector — it can behave impeccably toward the detector while equivocating elsewhere
(CCGR §2.6 accordingly does not offer Byzantine failure detectors). In the Byzantine world the
role of detection is replaced by larger quorums with honest intersection (§6) plus
authentication, and leader replacement survives only as the protocol-specific **view-change**
(§7).

## 3. Timing models

The principal axis of the theory: what may be assumed about message delay and process speed?

| Model | Assumption | Deterministic consensus |
|---|---|---|
| **Synchronous** | known upper bounds on delay and step time | solvable; crashes are detectable by timeout |
| **Partially synchronous** | bounds exist but are unknown, and/or hold only after an unknown **GST** (global stabilization time) | solvable with a correct majority (Paxos, Raft) |
| **Asynchronous** | no bounds | impossible (FLP) |

Partial synchrony (Dwork–Lynch–Stockmeyer 1988) is the model practical systems inhabit: the
network is usually timely and occasionally arbitrary. Well-designed protocols are **safe
unconditionally** and **live after GST** — Raft's randomized election timeout is the canonical
example: before stabilization, elections may split and repeat (a liveness cost only); after it,
a single leader emerges and commits proceed. Modules 05 and 07 implement exactly this regime.

## 4. The failure-detector lens

A **failure detector** packages timing assumptions as a per-process oracle (CCGR §2.6);
Module 05 states the specifications formally. The correspondence:

| Timing model | Implementable detector |
|---|---|
| synchronous | **P** (perfect: strong completeness + strong accuracy) |
| partially synchronous | **◇P** (eventually perfect), **Ω** (eventual leader) |
| asynchronous | none of the above |

**Theorem (Chandra–Hadzilacos–Toueg 1996).** Ω is the *weakest* failure detector that solves
consensus, given a majority of correct processes (`f < n/2`). Without the majority assumption,
the weakest is the pair (Σ, Ω), where Σ — the quorum detector — supplies the intersecting-set
structure that a majority otherwise provides.

Consequently, with a correct majority: consensus is solvable iff Ω is implementable; and since
partial synchrony suffices for Ω (but is not necessary — §2), the timing lens and the detector
lens draw essentially the same boundary, with Ω marking it slightly more finely.

Summary: *synchrony yields P; partial synchrony yields ◇P and Ω; asynchrony yields nothing
strong enough (FLP).* Module 05 constructs ◇P and Ω from heartbeats; Module 07 consumes Ω.

## 5. Fault models

Timing determines *solvability*; the fault model determines *cost* — chiefly, quorum size
(§6). CCGR §2.2:

- **Crash-stop.** A faulty process halts and takes no further steps.
- **Crash-recovery.** A faulty process may halt and later rejoin, having lost volatile state;
  algorithms compensate with stable storage (Modules 07–08: *persist before you externalize*).
- **Byzantine (arbitrary).** A faulty process may deviate arbitrarily: lie, equivocate, send
  conflicting messages to different peers, collude. Authentication (signatures) limits but does
  not eliminate the deviations.

The crash-to-Byzantine transition replaces *trusting silence* ("no message means slow or
dead") with *distrusting content* ("any message may be false") — and changes the arithmetic.

## 6. Quorums: the arithmetic of agreement

**Definition.** A **quorum system** over `N` processes is a collection of subsets (quorums)
such that any two quorums intersect. Protocol steps (votes, acknowledgments, promises) are
validated by quorums; intersection is what carries information from one protocol step — or one
leadership epoch — to the next: the common member "remembers." (CCGR §2.7.3; Module 04 proves
the intersection lemma.)

**Crash faults: majority quorums.** With `N = 2f + 1` tolerating `f` crashes, quorums of size
`f + 1 = ⌈(N+1)/2⌉` intersect in at least one process, and a quorum of correct processes always
exists (availability). This is every quorum in Modules 04, 05, and 07.

**Byzantine faults: supermajority quorums.** Intersection must now contain at least one
*honest* process — a Byzantine one in the overlap may tell each side a different story. Two
constraints: **availability** — a process can wait for at most `N − f` replies (`f` faulty
processes may stay silent), so quorums cannot exceed `N − f`; **honest intersection** — two
quorums of size `Q` share `≥ 2Q − N` members, of which at most `f` are faulty, so honest
intersection needs `2Q − N > f`. With `Q = N − f`: `N > 3f`. Hence the classical sizing
**`N = 3f + 1`, `Q = 2f + 1`** — quorums are strict two-thirds supermajorities. This is the
arithmetic underlying PBFT, HotStuff, Tendermint, and proof-of-stake finality gadgets
(e.g., Ethereum's Casper FFG).

**Timing × fault, in one table** (entries: minimum replication to tolerate `f`):

| | synchronous | partially synchronous |
|---|---|---|
| **crash** | `f + 1` (timeouts detect crashes) | `2f + 1` — majority (Paxos, Raft) |
| **Byzantine** | `f + 1` with signatures (Dolev–Strong); `3f + 1` without (Pease–Shostak–Lamport) | `3f + 1`, even with signatures (DLS lower bound) |

Two readings of the table: **majority is the price of not distinguishing dead from slow** (the
partially synchronous crash cell); **two-thirds is the price of not distinguishing honest from
lying** (the Byzantine cells under partial synchrony).

## 7. Protocol families and round structure

Leader-based consensus protocols share a two-part skeleton: (1) establish a leader for an
epoch; (2) have the leader drive values through quorums. The families differ in rounds and
quorum type:

| | (Multi-)Paxos | Raft | PBFT | HotStuff |
|---|---|---|---|---|
| fault model | crash | crash | Byzantine | Byzantine |
| leadership | ballot / Prepare | term / election | view / view-change | view per round |
| replication | Accept | AppendEntries | pre-prepare → prepare → commit | pipelined three-phase chain |
| steady-state cost per command | 1 round-trip | 1 round-trip | 2 all-to-all rounds | linear, pipelined |
| quorum | majority | majority | `2f+1` of `3f+1` | `2f+1` of `3f+1` |

- **Raft ≈ Multi-Paxos** with a strong leader: elect once per term; thereafter one majority
  round-trip per command (Module 07).
- **PBFT** (Castro–Liskov 1999) requires a second voting phase: after *prepare* establishes
  agreement on ordering within a view, *commit* ensures the decision survives a view-change —
  lifting "a quorum knows" to "a quorum knows that a quorum knows," which is what a new leader
  can verifiably reconstruct despite lying predecessors.
- **HotStuff** (Yin et al. 2019) linearizes communication (leader-mediated, threshold
  signatures) and pipelines the phases; it is the design basis of several production BFT
  systems (DiemBFT/Jolteon lineage; Tendermint is the earlier chained-BFT relative).

**Leader election provides liveness; quorums provide safety.** A leader oracle only points at a
process so the group can make progress; it may be wrong without endangering correctness. In
crash consensus the two concerns separate cleanly: a wrong Ω costs an extra election, while
majority quorums and the election restriction carry safety. In Byzantine protocols the
concerns meet in the **view-change**, which must simultaneously depose a suspected leader
*and* prove, from `2f+1` signed certificates, a starting state consistent with every possibly
committed value. The view-change is where the second voting phase pays off, where classic PBFT
incurred its `O(n³)` worst case, and where HotStuff's chief innovation lies (a linear,
uniform rule). It is, by common experience, the most defect-prone component of deployed BFT
systems — the practical reason Module 11 treats it as a first-class topic rather than a
footnote.

## 8. Consensus is not atomic commitment

**Atomic commitment** (Module 08; CCGR §6.1) resembles consensus — all processes must reach one
decision — but differs in both defining dimensions:

- **Decision function.** Consensus may decide any proposed value (C2). Atomic commitment's
  outcome is a *function of all votes*: COMMIT only if every participant voted YES; one NO (or
  one crash) forces ABORT. Unanimity, not choice.
- **Fault tolerance.** Consensus proceeds with any majority. Two-phase commit **blocks**: a
  coordinator crash between voting and decision strands participants in doubt, holding locks,
  unable to terminate (demonstrated end-to-end in Module 08).

The failure-detector hierarchy makes the difference precise: consensus requires Ω (with a
majority), while non-blocking atomic commitment in general requires the **perfect** detector P
— deciding COMMIT requires *certainty* that no participant has crashed, which no
eventually-accurate detector supplies. In this exact sense NBAC is the harder problem, and the
practical repair is to *reuse* consensus rather than avoid it: **Paxos Commit**
(Gray–Lamport 2006) runs the commit decision through a consensus instance, eliminating the
single point of blocking; 2PC is its one-acceptor degenerate case. Layered architectures
(Spanner, CockroachDB) compose the two: consensus inside each replicated shard, atomic
commitment across shards.

Terminological caution: 2P**C**'s two phases (vote, decide) are unrelated to 2P**L**'s two
phases (lock growth, lock shrinkage) — see
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) §6 — and neither
corresponds to the phases of Paxos.

## 9. CAP

**Theorem (Gilbert–Lynch 2002, formalizing Brewer).** A read/write register cannot
simultaneously guarantee consistency (linearizability), availability (every request to a
non-failed node receives a response), and tolerance of network partitions.

During a partition, a system chooses: refuse service on the minority side (consistency over
availability — the choice of every quorum protocol in this course: Module 04 returns
`ERR no quorum`; Raft simply has no leader on a minority partition) or serve stale data
(availability over consistency — the Dynamo family, with reconciliation machinery downstream).
CAP is thus not an exotic limit but the operational face of quorum intersection: the same
arithmetic that provides safety necessarily withholds service from minorities.

## 10. The course on this map

| Module | Position |
|---|---|
| [01](../01-kv-store/), [02](../02-networked-kv-store/) | pre-agreement: the register; processes, links, local concurrency |
| [04](../04-replicated-kv-store/) | the (1, N) majority-quorum register — what is achievable *without* consensus, and what is not |
| [05](../05-leader-election/) | ◇P and Ω from heartbeats — partial synchrony, packaged |
| [06](../06-logical-time-broadcast/) (planned) | logical time and the broadcast hierarchy; total-order broadcast ⟺ consensus |
| [07](README.md) | crash consensus (Raft): uniform agreement, majority quorums, crash-recovery |
| [08](../08-two-phase-commit/) | atomic commitment (2PC): unanimity, blocking, the P-vs-Ω separation |
| [10](../10-byzantine-broadcast/) (planned) | **Byzantine reliable broadcast** (Bracha): `3f+1`, echo/ready quorum amplification — the first Byzantine primitive |
| [11](../11-byzantine-consensus/) (planned) | **Byzantine consensus** (PBFT-style): two-phase voting, certificates, view-change |

(Modules [03](../03-shared-memory-concurrency/), [09](../09-concurrency-control/), and
[12](../12-crdts-eventual-consistency/) — shared-memory concurrency, concurrency control, and
eventual consistency/CRDTs — sit off this map's agreement axis; see the
[course home](../README.md) for the full sequence.)

## 11. References

**Impossibility and models**
- M. Fischer, N. Lynch, M. Paterson, *Impossibility of Distributed Consensus with One Faulty
  Process*, JACM 32(2), 1985.
- C. Dwork, N. Lynch, L. Stockmeyer, *Consensus in the Presence of Partial Synchrony*,
  JACM 35(2), 1988.

**Failure detectors**
- T. D. Chandra, S. Toueg, *Unreliable Failure Detectors for Reliable Distributed Systems*,
  JACM 43(2), 1996.
- T. D. Chandra, V. Hadzilacos, S. Toueg, *The Weakest Failure Detector for Solving
  Consensus*, JACM 43(4), 1996.
- C. Delporte-Gallet, H. Fauconnier, R. Guerraoui, *Tight Failure Detection Bounds on Atomic
  Object Implementations*, JACM 57(4), 2010. (Σ.)
- M. K. Aguilera, C. Delporte-Gallet, H. Fauconnier, S. Toueg, *On Implementing Omega with
  Weak Reliability and Synchrony Assumptions*, PODC 2003.

**Crash consensus**
- L. Lamport, *The Part-Time Parliament*, ACM TOCS 16(2), 1998; *Paxos Made Simple*, ACM
  SIGACT News 32(4), 2001.
- D. Ongaro, J. Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*,
  USENIX ATC 2014.
- M. Ben-Or, *Another Advantage of Free Choice: Completely Asynchronous Agreement Protocols*,
  PODC 1983.

**Byzantine agreement and broadcast**
- M. Pease, R. Shostak, L. Lamport, *Reaching Agreement in the Presence of Faults*, JACM
  27(2), 1980; L. Lamport, R. Shostak, M. Pease, *The Byzantine Generals Problem*, ACM TOPLAS
  4(3), 1982.
- D. Dolev, H. R. Strong, *Authenticated Algorithms for Byzantine Agreement*, SIAM J.
  Computing 12(4), 1983.
- G. Bracha, *Asynchronous Byzantine Agreement Protocols*, Information and Computation 75(2),
  1987. (Reliable broadcast — Module 07.)
- M. Castro, B. Liskov, *Practical Byzantine Fault Tolerance*, OSDI 1999.
- M. Yin, D. Malkhi, M. K. Reiter, G. Gueta, I. Abraham, *HotStuff: BFT Consensus with
  Linearity and Responsiveness*, PODC 2019.
- E. Buchman, J. Kwon, Z. Milosevic, *The Latest Gossip on BFT Consensus*, arXiv:1807.04938,
  2018. (Tendermint.)
- A. Miller, Y. Xia, K. Croman, E. Shi, D. Song, *The Honey Badger of BFT Protocols*, CCS
  2016.

**Atomic commitment and CAP**
- J. Gray, *Notes on Data Base Operating Systems*, Springer LNCS 60, 1978.
- D. Skeen, *Nonblocking Commit Protocols*, SIGMOD 1981.
- J. Gray, L. Lamport, *Consensus on Transaction Commit*, ACM TODS 31(1), 2006.
- S. Gilbert, N. Lynch, *Brewer's Conjecture and the Feasibility of Consistent, Available,
  Partition-Tolerant Web Services*, ACM SIGACT News 33(2), 2002.

**Text and surveys**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011.
- Decentralized Thoughts, *Consensus Cheat Sheet*, 2021.
  <https://decentralizedthoughts.github.io/2021-10-29-consensus-cheat-sheet/>

---
*[Course home](../) · Implementation: [Module 07 — Raft](README.md)*
