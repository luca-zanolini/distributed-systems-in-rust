# Module 11 — Byzantine Consensus *(planned)*

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 07](../07-raft/), [Module 10](../10-byzantine-broadcast/).
**Status: planned — to be built after Module 10.***

**Planned scope.** Consensus when processes may lie: a PBFT-style protocol under partial
synchrony with `N = 3f + 1`, completing the course's agreement arc. Where Module 07 trusted
every message (a term number *is* a credential in the crash model), this module trusts only
**quorum certificates**.

Planned content:

- **From crash to Byzantine**: what breaks in Raft under equivocation; why `2f+1`-of-`3f+1`
  quorums replace majorities (honest intersection); the role of signatures.
- **The two-phase voting pattern** (pre-prepare / prepare / commit): prepare certificates
  ("a quorum accepted this ordering in this view") and commit certificates ("a quorum knows
  that a quorum knows"), and why the second phase is exactly what makes decisions survive a
  view-change.
- **The view-change**: rotating away from a faulty leader while provably preserving every
  possibly-committed value — the hardest and most defect-prone component of BFT systems,
  treated as a first-class topic.
- **Perspective**: PBFT's `O(n²)` communication and HotStuff's linear, pipelined re-engineering;
  the connection to proof-of-stake finality (Tendermint, Casper FFG).

Deliverables, when built: a single-shot (or small multi-shot) BFT consensus implementation with
scriptable Byzantine behaviors (equivocation, silence, invalid certificates), demonstrations of
safety under `f` Byzantine nodes and of the view-change, module notes, exercises.

**Principal sources.** M. Castro & B. Liskov, *Practical Byzantine Fault Tolerance*, OSDI
1999; M. Pease, R. Shostak, L. Lamport, JACM 27(2), 1980; M. Yin et al., *HotStuff*, PODC
2019; E. Buchman, J. Kwon, Z. Milosevic, *The Latest Gossip on BFT Consensus*, 2018; CCGR
Ch. 5 (Byzantine consensus).

---
*[Course home](../) · Previous: [Module 10](../10-byzantine-broadcast/) · Next:
[Module 12 (planned)](../12-crdts-eventual-consistency/)*
