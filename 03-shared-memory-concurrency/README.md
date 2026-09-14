# Module 03 — Shared-Memory Concurrency

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisite:
[Module 02](../02-networked-kv-store/). Theory companion:
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) §§1–3.*

**Abstract.** Every other module in this course coordinates *processes over a network*. This
one coordinates *threads inside one process*, where the "network" is shared memory: delivery
is instant and reliable, but interleaving is adversarial. We build one program per primitive
or hazard — condition variables, semaphores, deadlock and two cures, reader–writer locks,
atomics, and a lock-free stack — and for each we first construct the failure, measure it,
and then earn the repair. The module doubles as the course's execution-model primer (what a
thread, a stack, and the heap actually are) and as its honest map of what Rust's type system
does and does not guarantee about concurrent code.

## Learning objectives

After this module you can:

1. describe the execution model: processes, threads, stacks, the heap, and what it means
   for two threads to touch the same address;
2. use condition variables correctly (the `while` law, notify-the-counterpart) and explain
   why polling is a latency/CPU dilemma with no good setting;
3. build a counting semaphore from `Mutex` + `Condvar` and state how its *sticky* signal
   differs from a condvar's *ephemeral* one;
4. name the four Coffman conditions, produce a deadlock on demand, and break the cycle two
   different ways — by total ordering and by admission control — with a control experiment
   proving causation;
5. choose between `Mutex`, `RwLock`, and atomics, and predict when each wins — including
   the counter-intuitive case where the mutex ties the atomic;
6. explain compare-and-swap as "modify only if unchanged," write a CAS retry loop, and
   state precisely why a Treiber stack cannot lose elements;
7. articulate the boundary of "fearless concurrency": data races are unrepresentable in
   safe Rust; deadlocks, livelocks, and logical races are not prevented — and `unsafe`
   marks where the compiler's proof ends and yours begins.

## 1. System model: one process, many threads

This module runs in the purest *asynchronous* model of the course: no message loss, no
crashes of individual actors — only arbitrary interleaving decided by a scheduler we do not
control. Every pathology below is a scheduling pathology.

**The execution model, from the ground up.** A **process** is an address space plus
resources: one heap, one set of file descriptors, one program image — one kitchen. A
**thread** is an independent stream of instructions *inside* that kitchen: it has its own
program counter and its own **stack** (function frames, locals, guards — private by
convention, scoped by braces), but it shares *everything else*, above all the **heap** —
the open space where `Box`, `Arc`, and our stack nodes live. "Accessing memory" is a load
or store instruction against an address; when two threads load and store the *same*
address, the hardware's cache-coherence machinery decides what each one sees — and without
rules, the result is the data race, the torn read, the lost update.

Where today's actors live: the `Mutex` and the data it guards sit on the heap (reached via
`Arc` handles); each guard lives on one thread's stack and dies at a brace; a `move`
closure's captures are *moved from the spawning thread's world into the new thread's*;
Treiber nodes live on the heap owned, for most of their lives, by *no one* (§7).

**What Rust guarantees here — and what it does not.** `Send`/`Sync` make *data races
unrepresentable in safe code*: you cannot compile unsynchronized shared mutation (try to
share a bare `u64` mutably across threads — the compiler refuses; that is the whole
mechanism). But the type system is silent about *deadlock* (module exhibit:
`philosophers.rs` freezes forever, compiles warning-free), about *livelock*, and about
*logical* races (two atomics composed wrongly, §6). Fearless concurrency ends exactly
there, and `unsafe` (§7) is the door in the fence: not "this is broken" but "the proof
obligations are now the author's."

## 2. Waiting: the polling dilemma and the condition variable

**Program:** `bounded_buffer_polling.rs` (failure exhibit) vs `bounded_buffer.rs`.

The producer–consumer problem (Dijkstra 1965): a fixed-capacity queue, a producer that
must wait when it is full, a consumer that must wait when it is empty. *How do you wait
for a condition another thread will make true?*

**The naive answer is polling**: check, unlock, nap, retry. It works — and it is wrong
twice. Measured (warm, best-of-3, 50 items through a 10-slot buffer with 10 ms naps):
**~57 ms wall time with ~0 CPU** — the program's life is almost entirely sleep, because
every blocked side commits to a full blind nap even if the world changes a microsecond
later. Shrink the nap and the sleep converts to CPU burn; there is *no good value for the
dial*. Worse, polling performance depends on the accidental phase relationship of the two
sides' clocks (our 100 ms-nap variant was only 1.75× slower than the 10 ms one — batching
dynamics, not linearity): polling is not just slow, it is *hard to reason about*.

**The condition variable deletes the dial.** `Condvar::wait(guard)` atomically releases
the mutex and sleeps; `notify_one` wakes a waiter, which re-acquires the lock before
returning. Same program, no clock anywhere: **~5 ms** — bounded by the printing, not by
any timer. Three laws, each load-bearing:

- **Wait in a `while`, never an `if`.** A wakeup is evidence of *nothing* — not who woke
  you, not why, not that the condition holds (spurious wakeups exist; barging rivals may
  consume the state between notify and your re-acquisition). The only truth is the
  predicate, re-read under the lock.
- **Notify the counterpart, after the mutation.** A `put` makes "not empty" true, so it
  notifies `not_empty` — the room whose predicate it changed, not its own.
- **The guard-through-the-wait API is unforgeable.** `wait` *consumes* your guard (you
  provably hold the lock; you provably lose it while asleep) and returns a fresh one (you
  provably hold it again before touching the data). Three classical condvar bugs —
  wait-without-lock, sleep-with-lock, touch-after-wake-without-lock — are unrepresentable,
  by move semantics alone. A `(Mutex<State>, Condvar)` pair with its methods *is* a
  monitor (Hoare 1974), assembled by hand.

## 3. The semaphore: a sticky counter

**Program:** `semaphore.rs`, with the type in [`src/lib.rs`](src/lib.rs).

Dijkstra's counting semaphore, rebuilt from parts: `permits: Mutex<usize>` +
`available: Condvar`; `acquire` waits while zero then decrements; `release` increments and
notifies once. Demo: 3 permits, 8 workers of 200 ms each → measured 610 ms (predicted
600), peak concurrency exactly 3, admissions *pipelined* per-exit rather than in waves.

Two distinctions worth keeping:

- **Sticky vs ephemeral.** `release()` with nobody waiting *banks* a permit; a later
  `acquire` finds it. `notify_one()` with nobody waiting *evaporates*. A semaphore is a
  condvar plus memory — which is why the condvar alone must always be paired with a
  predicate.
- **No ownership.** Any thread may `release`, including one that never acquired — that is
  a feature (`Semaphore::new(0)` is a pure cross-thread event) and a footgun (nothing
  prevents double-release minting permits, and no RAII guards a permit: forgetting
  `release` leaks a seat forever). The mutex's guard is machine-checked custody; the
  semaphore's protocol is human-checked discipline.

Why `notify_one` suffices in `release`: one release creates exactly one admission's worth
of new truth. `notify_all` would be *merely wasteful* — a thundering herd of woken workers
re-checking and re-sleeping — but only because `acquire` waits in a `while`; with an `if`,
over-notification decrements a zero counter: panic in debug builds, a 2⁶⁴-permit garage in
release builds. The `while` demotes over-notification from catastrophe to waste.

## 4. Deadlock: the dining philosophers

**Programs:** `philosophers.rs` (failure exhibit), `philosophers_ordered.rs`,
`philosophers_waiter.rs`.

Five philosophers, five forks (`Mutex<()>` — the lock *is* the resource), each meal needs
both adjacent forks. The naive protocol — pick left, then right, with a deliberate 1 ms
sleep widening the window — freezes reliably: five "picked up the left fork" lines, zero
meals, a process alive at 0% CPU forever. Note first that *deadlock requires concurrency*:
the same protocol run sequentially (our first, accidental version) cannot deadlock — one
diner at a time never holds-and-waits against anyone.

**The Coffman conditions** (1971) — deadlock requires all four, so killing any one cures:

| # | Condition | Where it lives here |
|---|---|---|
| 1 | Mutual exclusion | a fork serves one holder (`Mutex`) |
| 2 | Hold and wait | holding left while requesting right |
| 3 | No preemption | only the holder's drop releases (RAII guarantees this!) |
| 4 | Circular wait | the `%5` ring: 0→1→2→3→4→0 |

**Cure 1 — total ordering (kills #4).** Acquire forks in ascending index (`min` first,
`max` second). Only the wrap-around philosopher changes behavior — and that one
symmetry-breaker suffices: any chain of waiters now climbs strictly upward and must
terminate at the top. In the transcript, the contrarian blocks *empty-handed*, the free
fork lands as someone's second, and the cascade unwinds — theory and log matching line for
line. (When no natural index exists, real systems order by address — `Arc::as_ptr` as
tiebreak.)

**Cure 2 — admission control (kills #4 by pigeonhole).** Keep the broken protocol; post
our own `Semaphore::new(4)` at the door. With at most 4 armed philosophers sharing 5
forks, someone's second fork is provably free; every waiter chain terminates at an eater.
Note what survives: hold-and-wait is intact — the cure removes the *ring*, not the
waiting. **Control experiment:** set the bouncer to 5 and the deadlock returns —

| seats | meals | outcome |
|---|---|---|
| 4 | 5 | clean exit |
| 5 | 0 | frozen forever |

One constant flips the outcome: the mechanism, not luck, is doing the work. 🎓 *Teaching
idea: run the control live — fix-on, fix-off — and let students name which Coffman
condition each cure attacks.*

## 5. Readers–writers: role-aware locking

**Programs:** `readers_writers_mutex.rs` (failure exhibit) vs `readers_writers.rs`.

Read-heavy shared state (100 reads of 5 ms, 5 writes of 5 ms, 4 reader threads + 1
writer). The mutex is *role-blind* — both roles call the same `lock()`; the roles exist
only in the print strings — so all 105 holds serialize: **644 ms** (≈105 × 6.1 ms; note
`sleep(5ms)` overshoots ~1 ms on macOS — measure, don't assume). `RwLock` distinguishes
`read()` (shared guard, many at once) from `write()` (exclusive): **217 ms, 3.0×** — the
log shows four simultaneous `read #0` lines, the queue collapsing on camera.

The anchor: **`RwLock` is the borrow checker enforced at runtime.** Read guard ↔ `&T`
(many shared, read-only — try `*g += 1` through one; the compiler refuses); write guard ↔
`&mut T` (exclusive, read-write); many-readers-XOR-one-writer *is* the aliasing law. The
same invariant is the databases' S/X lock (2PL, [Module 08](../08-two-phase-commit/), and
hands-on in Module 09), and the shared-memory shadow of Module 04's single-writer
register: one idea, four costumes.

Honesty note on fairness: `std::sync::RwLock` promises **no policy** — writer starvation
under a reader storm is platform-dependent (our macOS run showed writer-preference;
Linux may differ). Where fairness matters, use a lock that promises it. (Exercise 5
builds the probe.)

## 6. Atomics: agreement without locks

**Program:** `counters.rs`.

An atomic is a hardware-indivisible operation on one word — mutual exclusion provided by
the processor, per operation, no parking. Three implementations of the same workload
(4 threads × 1M increments), measured (release build, warm):

| version | loop body | final count | time |
|---|---|---|---|
| broken | `store(load()+1)` | **1,009,689 / 4M** | 1.2 ms |
| fetch_add | `fetch_add(1)` | 4,000,000 ✓ | 39 ms |
| mutex | `*lock() += 1` | 4,000,000 ✓ | 34 ms |

Three lessons, one per row:

1. **Atomics don't compose.** An atomic load and an atomic store with a gap between them
   is not an atomic increment: 75% of updates lost to the load-gap-store window — the
   same disease as check-then-act without a lock, at nanosecond scale. And the broken
   version is *30× fastest*: wrong answers, delivered efficiently. *Before admiring a
   fast number, check what it is a number of.*
2. **`fetch_add` fuses read-modify-write** into one indivisible step; conservation by
   construction.
3. **The mutex tied the atomic** — at *maximal* contention, parking losers acts as
   accidental backoff (fewer cores fight the cache line; the holder streaks), while
   `fetch_add` keeps every core bouncing the line every cycle. At low contention the
   atomic wins decisively. Contention level, not ideology, picks the winner.

**Memory orderings, the honest minimum.** `Relaxed` = this operation is atomic, nothing
promised about its ordering relative to *other* memory traffic — right for lone counters.
When one atomic *publishes* other memory (a flag guarding data, a stack's `top` guarding
node fields), the writer needs `Release` and the reader `Acquire` — see the Treiber CAS.
`SeqCst` buys a single global order at extra cost. The full model (C++'s, inherited) is
out of scope by design; this module uses exactly the three named here.

## 7. Lock-free: the Treiber stack

**Program:** `treiber.rs` (step A, single-threaded raw-pointer chain, is in git history;
the file is step B, lock-free).

**Why lock-free at all — not speed: *progress*.** A lock makes the structure's liveness
hostage to the current holder: preempt or kill the holder and everyone parks behind a door
nobody will open — the shared-memory 2PC coordinator, dead in its window (poisoning, like
"recovered in-doubt," detects but does not revive). **Lock-freedom** (Herlihy): freeze any
subset of threads anywhere, and some remaining thread still completes in finite steps. A
thread mid-`push` holds *nothing*; its death harms only itself. The analogy this course
earns: **Treiber : Mutex :: quorum consensus : 2PC** — a more intricate protocol buys
progress that no single participant's failure can hostage. The corollaries fall out: no
deadlock (nothing held ⇒ no hold-and-wait), no priority inversion, usable where blocking
is forbidden.

**Why a linked chain of nodes:** the hardware's atomic vocabulary is one word. CAS can
swing a single pointer; it cannot atomically update a `Vec`'s length and buffer. The
structure is re-architected until *every operation is one pointer swing* — push: swing
`top` to my node; pop: swing `top` past the departed. The nodes are the shape that fits
through the one-word door.

**The ownership arc of a node** (and the reason for raw pointers): born owned (a `Box` in
`push`) → *orphaned on purpose* (`Box::into_raw`: owner retired, no heir, no cleanup) →
named only by `*mut Node`, the contract-free address, because no contract would be true
(no owner's scope to back a borrow's lifetime; no exclusivity once rival threads hold the
same address mid-CAS) → **never freed**. Each `unsafe` block is where our proof replaces
the compiler's; the file's comments state the vouch at every fence.

**CAS: the third verb.** `store` = "become my value" (blind; the broken counter);
`fetch_add` = "change by this much" (fused); `compare_exchange` = **"become my value only
if you are still what I photographed — else refuse and tell me."** Both operations are the
same loop: photograph `top` → prepare → CAS → `Ok` done / `Err` re-photograph and retry.
Losers retry; nobody waits.

**Conservation is a theorem, not a test result.** For a push to be lost, two CASes would
have to succeed against the same photograph — impossible: the first success *changes*
`top`, staling the second. Stress run: 4 producers × 100k pushes, drained and audited —
**400,000 of 400,000, five runs of five** (`demos/treiber.py`). Contrast the broken
counter: same hardware, same `Ordering::Relaxed`, opposite guarantee — the difference is
entirely in the verb.

**The load-bearing leak, reclamation, and ABA.** Popped nodes are never freed. This is
not sloth; it is the module's last honest boundary. Freeing requires proving *no thread
still holds this address* — exactly the "nothing changed since I looked" class of claim
that dies in concurrent code. Our leak makes every address valid forever (dereferencing a
just-popped node reads harmless history), and *derives* ABA-immunity for free: CAS asks
"same address?", not "same world?" — dangerous when addresses recycle; ours never do.
Production answers — epoch-based reclamation (crossbeam), hazard pointers (Michael 2004)
— are machinery for proving a bounded version of that claim before daring to free.
Rust's ownership can even be *re-appointed* (`Box::from_raw`) when such a proof exists.

## 8. Benchmarking discipline (a meta-section this module earned)

Every number above obeys three rules learned the hard way: **warm** (macOS runs
Gatekeeper's assessment on a fresh binary's first exec — ~300–500 ms, once; all our
first measurements were polluted by it), **repeated** (best-of-N; one measurement is an
anecdote), **baselined** (time `true` before trusting `time`). Prefer the stopwatch
*inside* the laboratory: `Instant` (monotonic — durations) around the workload, never
wall-clock (`SystemTime` — timestamps) and never process-external timers for
sub-second effects. And the standing rule from the counters table: a speed number means
nothing until you check *what* it is a number of.

## Theory ↔ code

| Concept | Where |
|---|---|
| Monitor (Hoare 1974): mutex + condition + data | `bounded_buffer.rs` |
| Polling dilemma (latency ↔ CPU, no good dial) | `bounded_buffer_polling.rs` |
| Semaphore (Dijkstra 1965), sticky signal, no ownership | `src/lib.rs`, `semaphore.rs` |
| Coffman conditions; cure by ordering / by admission | `philosophers*.rs` |
| S/X locks = aliasing law at runtime; 2PL bridge | `readers_writers*.rs` |
| RMW atomicity; atoms don't compose; contention economics | `counters.rs` |
| CAS retry loop; lock-freedom; leak/reclamation/ABA | `treiber.rs` |
| Verdict-style failure reproduction, warmed + repeated | `demos/*.py` |

## Honest limitations

1. The Treiber stack **leaks by design**; long-running use is out of the question without
   real reclamation (crossbeam epochs) — deliberately out of scope.
2. Memory orderings are used correctly but taught minimally; no weak-memory litmus tests
   are run (Apple Silicon would show some; a dedicated exercise is future work).
3. Fairness is observed, not controlled: `std` locks promise none; we measured one
   platform's behavior once (§5).
4. All experiments are single-machine, single-socket, small thread counts; contention
   economics (§6) shift with topology.
5. The demos prove *presence* of the failure and *absence under test* of the fixed
   version's failure — schedule-dependent bugs need model checking for proof; that is
   the course's forthcoming Quint bridge, and this module is written to feed it.

## Exercises

1. **Two-semaphore buffer.** Re-derive M1 with `items = Semaphore(0)` and
   `spaces = Semaphore(cap)` and no condvar of your own. Which invariant does each
   counter carry? Where did the `while` loops go?
2. **Build your own `RwLock`** from `Mutex<(readers: u32, writer: bool)>` + two condvars.
   Decide and document your fairness policy; demonstrate writer starvation in the
   read-preferring variant.
3. **The `if`+`notify_all` disaster.** Change the semaphore's `while` to `if`, `release`
   to `notify_all`; predict debug and release behavior; verify (in debug!); explain via
   §3's underflow argument.
4. **Address-ordered philosophers.** Replace fork indices with `Arc::as_ptr` ordering.
   Prove the same ladder argument goes through with addresses as the total order.
5. **Starvation probe.** Under a continuous 4-reader storm, measure a writer's
   `write()`-acquisition latency distribution on your platform. Compare with a fair lock
   (e.g., `parking_lot` with fairness enabled).
6. **Single-threaded reclamation.** In step A of the stack (git history), free popped
   nodes with `Box::from_raw` and argue why the same line in step B would be
   use-after-free. State the exact claim reclamation must prove.
7. **ABA on paper.** With recycled addresses, construct the classic pop interleaving
   where CAS succeeds wrongly. Then show why our never-free policy makes the
   interleaving impossible.
8. **Spin vs park.** Give the counters shootout a fourth contender: a spin-lock
   (`AtomicBool` + `compare_exchange` + `hint::spin_loop`). Measure at 4 threads and at
   1; explain both results with §6's contention economics.
9. **Peak-concurrency audit.** Extend `demos/semaphore.py` to also assert *pipelined*
   admission (an ENTER within the log-window after each EXIT once the queue is warm).
10. **Toward Quint.** Write (in prose, as a spec) the state, actions, and invariant of
    the bounded buffer as a transition system: this is the model the course's
    formal-methods bridge will check mechanically.

## Running everything

```sh
cargo build                       # all binaries
cargo run --bin bounded_buffer    # etc.
python3 demos/bounded_buffer.py   # each demo prints a VERDICT line
python3 demos/philosophers.py
python3 demos/readers_writers.py
python3 demos/semaphore.py
python3 demos/treiber.py
```

`philosophers` (the naive binary) is *supposed* to hang — that is the exhibit; Ctrl-C it,
or let the demo script kill it on schedule.

## References

- E. W. Dijkstra, *Cooperating Sequential Processes* (EWD 123), 1965 — semaphores, the
  producer–consumer and dining-philosophers problems; this module's founding document.
- C. A. R. Hoare, *Monitors: An Operating System Structuring Concept*, CACM 17(10), 1974 —
  the mutex+condvar discipline of §2. (P. Brinch Hansen's *Operating System Principles*,
  1973, develops the same idea.)
- E. G. Coffman, M. J. Elphick, A. Shoshani, *System Deadlocks*, ACM Computing Surveys
  3(2), 1971 — the four conditions of §4.
- P. J. Courtois, F. Heymans, D. L. Parnas, *Concurrent Control with "Readers" and
  "Writers"*, CACM 14(10), 1971 — the readers–writers problem and its fairness variants.
- L. Lamport, *A New Solution of Dijkstra's Concurrent Programming Problem*, CACM 17(8),
  1974 — the bakery algorithm: mutual exclusion from reads and writes alone.
- R. K. Treiber, *Systems Programming: Coping with Parallelism*, IBM Research Report
  RJ 5118, 1986 — the lock-free stack of §7.
- M. Herlihy, *Wait-Free Synchronization*, ACM TOPLAS 13(1), 1991 — lock-/wait-freedom
  and the consensus hierarchy (registers 1, fetch-and-add 2, CAS ∞) behind §6–7.
- M. M. Michael, *Hazard Pointers: Safe Memory Reclamation for Lock-Free Objects*, IEEE
  TPDS 15(6), 2004 — the reclamation problem of §7, solved.
- M. Herlihy, N. Shavit, V. Luchangco, M. Spear, *The Art of Multiprocessor Programming*,
  2nd ed., Morgan Kaufmann, 2020 — the module's textbook-scale companion.
- The Rustonomicon (`doc.rust-lang.org/nomicon`) — Rust's own honest account of `unsafe`,
  `Send`/`Sync`, and atomics.

---
*[Course home](../) · Previous: [Module 02](../02-networked-kv-store/) · Next:
[Module 04](../04-replicated-kv-store/)*
