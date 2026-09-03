# Module 03 — Shared-Memory Concurrency *(planned)*

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisite:
[Module 02](../02-networked-kv-store/). **Status: planned — not yet built.***

**Planned scope.** The concurrency-in-the-small module: correctness and coordination among
threads of a single process, beyond the mutex introduced in Module 02.

Planned content:

- **Synchronization primitives.** Semaphores; condition variables and monitors; the
  producer–consumer and reader–writer problems; reader–writer locks (`RwLock`).
- **Deadlock.** Necessary conditions; resource-allocation graphs; prevention by lock ordering;
  detection and recovery; livelock and priority inversion.
- **Atomics and lock-free programming.** Rust's `std::sync::atomic`, memory orderings, and one
  lock-free data structure (e.g., a Treiber stack), with a discussion of the ABA problem.
- **Rust's concurrency model.** `Send`/`Sync` as compile-time race freedom; what the type
  system does and does not rule out (data races vs. deadlocks and logical races).

Deliverables, when built: a set of small programs (one per primitive/hazard) with failure
demonstrations, lecture notes in this README, and exercises. Theory companion:
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) §§1–3.

**Principal sources.** M. Herlihy & N. Shavit, *The Art of Multiprocessor Programming*, 2nd
ed., 2020; E. W. Dijkstra, *Cooperating Sequential Processes*, 1965; C. A. R. Hoare,
*Monitors: An Operating System Structuring Concept*, CACM 1974; L. Lamport, *A New Solution of
Dijkstra's Concurrent Programming Problem* (bakery algorithm), CACM 1974.

---
*[Course home](../) · Previous: [Module 02](../02-networked-kv-store/) · Next:
[Module 04](../04-replicated-kv-store/)*
