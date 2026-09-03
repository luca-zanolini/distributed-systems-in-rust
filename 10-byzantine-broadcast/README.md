# Module 10 — Byzantine Reliable Broadcast *(next to be built)*

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 04](../04-replicated-kv-store/) (quorums), [Module 07](../07-raft/) (for contrast with
the crash model). **Status: next in the build order.***

**Planned scope.** The first Byzantine primitive: reliable broadcast when the *sender itself*
may be faulty — the problem of a sender that **equivocates**, telling different processes
different things. Bracha's protocol solves it with `N = 3f + 1` processes and two rounds of
quorum amplification (*echo*, then *ready*), introducing the supermajority-quorum reasoning of
the Byzantine world without the complexity of leader replacement.

Planned content:

- **The Byzantine fault model**: arbitrary deviation, equivocation, collusion; what
  authentication does and does not change.
- **Specification** (Byzantine reliable broadcast, CCGR Ch. 3): validity, no duplication,
  integrity, **consistency**, and **totality**.
- **Bracha's algorithm**: SEND → ECHO (witness a single value: `2f+1` echoes) → READY
  (amplification: `f+1` readies suffice to join, `2f+1` to deliver), and why each threshold is
  what it is.
- **The quorum arithmetic** `N > 3f` derived from availability + honest intersection
  (see [CONSENSUS.md](../07-raft/CONSENSUS.md) §6), exercised concretely.
- **The bridge to Module 11**: PBFT's prepare/commit certificates as the echo/ready pattern in
  service of agreement.

Deliverables, when built: a Bracha implementation over TCP with a scriptable *equivocating
sender* and demonstrations that `3f` honest processes cannot be split; module notes; exercises.

**Principal sources.** G. Bracha, *Asynchronous Byzantine Agreement Protocols*, Information
and Computation 75(2), 1987; CCGR Ch. 3 (Byzantine broadcast variants); L. Lamport,
R. Shostak, M. Pease, *The Byzantine Generals Problem*, ACM TOPLAS 4(3), 1982.

---
*[Course home](../) · Previous: [Module 09 (planned)](../09-concurrency-control/) · Next:
[Module 11 (planned)](../11-byzantine-consensus/)*
