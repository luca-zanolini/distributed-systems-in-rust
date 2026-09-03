# Module 06 — Logical Time and Broadcast *(planned)*

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 04](../04-replicated-kv-store/), [Module 05](../05-leader-election/).
**Status: planned — not yet built.***

**Planned scope.** Ordering events without synchronized clocks, and the broadcast hierarchy —
the classical bridge between replication and consensus.

Planned content:

- **The happens-before relation** (Lamport 1978); causality vs. real time; **Lamport clocks**
  (consistent with happens-before) and **vector clocks** (characterizing it exactly).
- **The broadcast hierarchy** (CCGR Ch. 3): best-effort, (uniform) reliable, FIFO, causal, and
  **total-order broadcast**, each specified by its properties and implemented over the
  perfect-links layer of Module 02.
- **Total-order broadcast ⟺ consensus** (CCGR Ch. 6): the reduction in both directions — the
  theoretical reason Module 07's replicated log is a consensus problem.
- **Consistent global snapshots** as an application of causality: Chandy–Lamport marker
  snapshots over the running cluster (planned as an exercise).

Deliverables, when built: a causal-broadcast implementation with vector clocks; a total-order
broadcast layered over [Module 07](../07-raft/)'s Raft; demonstrations of FIFO/causal/total
anomalies; exercises including Chandy–Lamport.

**Principal sources.** L. Lamport, *Time, Clocks, and the Ordering of Events in a Distributed
System*, CACM 21(7), 1978; C. Fidge / F. Mattern (vector clocks), 1988; K. M. Chandy &
L. Lamport, *Distributed Snapshots*, ACM TOCS 3(1), 1985; CCGR Chs. 3, 6.

---
*[Course home](../) · Previous: [Module 05](../05-leader-election/) · Next:
[Module 07](../07-raft/)*
