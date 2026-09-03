# Module 12 — Eventual Consistency, CRDTs, and Gossip *(planned)*

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 04](../04-replicated-kv-store/), [Module 06](../06-logical-time-broadcast/).
**Status: planned — not yet built.***

**Planned scope.** The AP side of the CAP trade-off, developed constructively: replication that
never refuses writes, with convergence guaranteed by algebraic structure rather than by
coordination. The course's closing counterpoint to the strong-consistency arc (Modules 04–08).

Planned content:

- **Eventual and strong eventual consistency**, defined precisely; why "eventually consistent"
  without convergence guarantees is a weak contract, and what SEC adds.
- **Conflict-free replicated data types**: state-based (join-semilattice, merge = least upper
  bound) and operation-based (commutative effects) formulations and their equivalence;
  G-counter, PN-counter, G-set, OR-set, LWW-register — with the OR-set add/remove semantics as
  the worked example of design under concurrency.
- **Anti-entropy and gossip**: epidemic dissemination, digests, Merkle-tree synchronization;
  membership by gossip (SWIM) folded in here.
- **Perspective**: where CRDTs are the right tool (collaborative editing, offline-first,
  shopping carts) and where they are not (invariants requiring coordination — the link back to
  consensus).

Deliverables, when built: a CRDT library (counters, sets, LWW-register) replicated over a
gossip layer, with partition-and-converge demonstrations; module notes; exercises.

**Principal sources.** M. Shapiro, N. Preguiça, C. Baquero, M. Zawirski, *Conflict-free
Replicated Data Types*, SSS 2011 (and the accompanying comprehensive study, INRIA RR-7506);
G. DeCandia et al., *Dynamo*, SOSP 2007; A. Das, I. Gupta, A. Motivala, *SWIM*, DSN 2002;
S. Gilbert & N. Lynch, CAP, SIGACT News 2002.

---
*[Course home](../) · Previous: [Module 11 (planned)](../11-byzantine-consensus/) ·
End of the planned sequence — see the [course home](../) for case-study notes.*
