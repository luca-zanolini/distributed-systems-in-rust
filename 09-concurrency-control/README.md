# Module 09 — Concurrency Control *(planned)*

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 08](../08-two-phase-commit/),
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) §§4–7.
**Status: planned — not yet built.***

**Planned scope.** The isolation mechanisms of transactional systems, implemented and broken
over the course's key-value store: the module that turns the serializability theory of the
consistency notes into running code and failing tests.

Planned content:

- **Two-phase locking**, with shared/exclusive locks, lock upgrades, and deadlock handling
  (ordering and detection); strict vs. rigorous variants and their recovery guarantees.
- **Optimistic concurrency control** (validation at commit; Kung–Robinson).
- **Multiversion concurrency control / snapshot isolation**, and **write skew** — the anomaly
  that separates snapshot isolation from serializability; serializable snapshot isolation in
  outline.
- **Isolation anomalies as tests.** Lost update, dirty read, non-repeatable read, write skew:
  each exhibited as a failing test under a too-weak mechanism and a passing test under the
  right one.

Deliverables, when built: a transactional layer over the store with pluggable concurrency
control (2PL / OCC / MVCC), an anomaly test suite, lecture notes, exercises.

**Principal sources.** P. Bernstein, V. Hadzilacos, N. Goodman, *Concurrency Control and
Recovery in Database Systems*, 1987; H. T. Kung & J. T. Robinson, *On Optimistic Methods for
Concurrency Control*, ACM TODS 1981; H. Berenson et al., *A Critique of ANSI SQL Isolation
Levels*, SIGMOD 1995; M. Cahill, U. Röhm, A. Fekete, *Serializable Isolation for Snapshot
Databases*, SIGMOD 2008.

---
*[Course home](../) · Previous: [Module 08](../08-two-phase-commit/) · Next:
[Module 10](../10-byzantine-broadcast/)*
