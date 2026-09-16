# Module 09 — Concurrency Control

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 03](../03-shared-memory-concurrency/) (locks, RAII guards, deadlock),
[Module 08](../08-two-phase-commit/) (transactions, atomic commitment),
[CONSISTENCY_AND_CONCURRENCY.md](../CONSISTENCY_AND_CONCURRENCY.md) §§4–7 (serializability).
**Status: complete.***

**Abstract.** A transaction is a multi-step program that must appear to execute alone. This
module builds, over a single in-memory key-value store, the three classical mechanisms that
create that appearance — **strict two-phase locking**, **optimistic concurrency control**
(Kung–Robinson), and **multiversion concurrency control / snapshot isolation** — and, before
any of them, the *anomalies* they exist to kill, each one exhibited as a small program whose
output shows a bank losing money, certifying a state that never existed, freezing solid, or
violating an invariant while every participant follows the rules. The module ends at the
modern frontier: **write skew**, the anomaly that survives snapshot isolation and separates
it from serializability, with serializable snapshot isolation (SSI) in outline. Where
Module 03 asked *"how do threads share memory safely?"* and Module 08 asked *"how do
machines commit together?"*, this module asks the question in between: *"how do concurrent
multi-step programs interleave without lying?"*

## Learning objectives

After this module you can:

- name and *reproduce* the classical isolation anomalies — lost update, read skew, dirty
  read, write skew — and say precisely which interleaving causes each;
- implement strict 2PL with a lock table, explain growing/shrinking phases, the
  plain/strict/rigorous hierarchy, and why RAII guards *are* the shrinking phase;
- explain the two-tier architecture of every real engine: **latches** (physical, protect
  the data structure for microseconds) vs **locks** (logical, protect transaction semantics
  for the transaction's lifetime);
- implement OCC's read/validate/write lifecycle and argue why read-only transactions must
  validate too;
- implement MVCC with version histories and snapshot reads, and prove that a snapshot
  reader cannot observe a torn view;
- exhibit write skew, explain why first-committer-wins does not catch it, and sketch the
  SSI repair;
- choose an engine for a workload by its *currency*: 2PL pays in waiting, OCC in wasted
  work, MVCC in storage.

## 1. System model: transactions without a database

One process; each **transaction is a thread** running a short straight-line program of
reads and writes against a shared store, with a deliberate `sleep` standing in for real
work (network calls, computation) between steps. The store is a
`HashMap<String, i64>` — accounts and balances — behind a `Mutex`.

That store mutex plays a precise and *deliberately insufficient* role: it is a **latch**,
not a lock. It protects the HashMap's physical integrity for the microseconds of a single
`get` or `insert` — nothing more. Transaction-level correctness (a deposit's
read-compute-write appearing atomic) is exactly what a latch cannot provide, and the gap
between the two tiers is where every anomaly in §2 lives. Real engines draw the same line:
latches guard pages and nodes; locks guard rows and predicates for a transaction's
lifetime (the classic split, in Gray & Reuter's terminology).

The correctness standard throughout is **serializability**: the concurrent execution must
be equivalent to *some* serial order of the transactions (see the course consistency
notes for conflict-serializability and its relation to linearizability). The anomalies of
§2 are precisely the observable ways an execution can fail this standard.

Honest scope: one machine, no durability (Module 08 owns logging and recovery), no aborts
initiated by the transactions themselves, and integer balances. The mechanisms, not the
plumbing, are the subject.

## 2. The anomalies (M1): the crime lives between operations

The defining feature of both exhibits: **every individual store operation is perfectly
locked.** No data race, no torn integer, nothing `unsafe` — Rust's fearless concurrency is
fully in force, and the bank still loses money. The anomaly is never *in* an operation; it
is *between* them.

**Lost update** (`lost_update.rs`). Two deposits (+10, +20) on x=100. Each reads the
balance (locked, flawless), computes, sleeps 5 ms, and writes back (locked, flawless).
Both read 100; the second write blindly overwrites the first:

```
read 100, wrote 110
read 100, wrote 120
final balance: 120        ← 30 deposited, 20 recorded; the survivor varies by run
```

**Read skew** (`read_skew.rs`). A transfer moves 30 from x to y (debit, 10 ms gap,
credit); an auditor reads x before the debit and y after the credit... or in our timing,
x *after* the debit and y *before* the credit:

```
audit: saw x = 20
audit: saw y = 50   →  TOTAL 70
final balance: x = 20, y = 80
```

The auditor certifies TOTAL 70 for a bank whose durable total was 100 at every instant.
It observed a *state that never existed* — half of another transaction.

The classical taxonomy (Berenson et al. 1995, repairing the ANSI SQL isolation levels)
names more: **dirty read** (reading a value an uncommitted transaction may yet revoke),
**non-repeatable read** (read skew confined to one item read twice), **phantoms** (the
footprint problem for keys that don't exist yet — honest limitation 5, exercise 8), and **write skew**
(§6, the finale). Each isolation level is defined by which of these it excludes;
serializability excludes them all.

## 3. Strict two-phase locking (M2): prevention

**The rule.** Every transaction acquires a lock on every item it touches before touching
it (**growing phase**), and releases nothing until it releases everything (**shrinking
phase**). The 2PL theorem (Eswaran–Gray–Lorie–Traiger 1976): if every transaction is
two-phase, every execution is conflict-serializable — the serial order is the order of
lock-points.

The hierarchy, by *when* locks are released:

| variant | releases | guarantees |
|---|---|---|
| plain 2PL | any time after the last acquire | serializable, but exposes uncommitted data |
| **strict 2PL** | write locks at commit/abort | + no dirty reads, cascadeless, recoverable |
| rigorous 2PL | all locks at commit/abort | + commit order = serial order |

**In Rust, strict 2PL is one idiom: the RAII guard.** The lock table is
`Arc<HashMap<String, RwLock<()>>>` — one logical lock per key, *separate from* the store
latch (the two-tier split of §1 made visible as two different maps). A transaction's first
lines acquire guards; the closing brace releases them all at once:

```rust
let _tx_from = locks.get(from).unwrap().write().unwrap();   // growing
let _tx_to   = locks.get(to).unwrap().write().unwrap();     // growing
/* ... read, sleep, write — the whole transaction body ... */
}   // ← the shrinking phase is the brace: everything at once, at the end
```

Holding *all* guards — read guards included — for the transaction's scope is precisely
**rigorous** 2PL, the top row of the table, which implies strict; Rust makes the
sloppy variants (release early, forget to release) harder to write than the correct one.
Shared/exclusive mode is `RwLock`'s read/write split: the auditor takes `.read()` on both
keys (auditors don't exclude auditors), transfers take `.write()`.

**The footprint law.** Lock every item you touch — and the *invariant you must preserve
dictates the footprint*. The transfer maintains x+y = const, an invariant spanning two
keys, so it locks two keys before touching either. An n-item invariant needs an n-item
footprint. (Write skew, §6, is exactly what happens to an engine that forgets this law's
read-side half.)

Both anomalies die on screen: the racing deposits serialize (`read 110` or `read 120`,
depending on which deposit wins the race — either way the second
deposit *sees* the first; final 130, every run), and the auditor is **delayed, not
deceived** (TOTAL 100; the log shows it waiting out the transfer).

**The price** (`deadlock_2pl.rs`): two opposite transfers, x→y and y→x, each grabs its
first key and sleeps 5 ms before grabbing its second. AB/BA — the program freezes
*before its first print*, silently, forever: Module 03's dining philosophers with account
names for forks, all four Coffman conditions present. The cure is also Module 03's:
**sorted acquisition** (`deadlock_2pl_fixed.rs` orders the two keys before locking —
a monotone ladder cannot cycle). Real engines can't statically order every footprint, so
they run **detection** instead: a waits-for graph, cycle check, and a victim transaction
killed and retried — which is why every serious system requires transactions to be
**restartable** (side-effect-free until commit). Prevention needs foresight; detection
needs undo.

## 4. Optimistic concurrency control (M3): detection

Kung & Robinson (1981) inverted the bet: conflicts are rare, so stop paying for locks —
run free, *check at the end*, redo on collision.

A transaction becomes a private notebook:

```rust
struct Tx {
    reads:  HashMap<String, u64>,  // key -> version I saw   (photographs)
    writes: HashMap<String, i64>,  // key -> value I intend  (drafts)
}
```

The store's values carry **version numbers** — `(u64, i64)`, bumped on every commit (the
third career of Module 04's write timestamps). The lifecycle:

1. **Read phase.** `read` checks my own drafts first (read-your-own-writes), else reads
   the store and *photographs the version number*. `write` only writes the draft. No
   locks are held across the transaction's gap — the sleep sits in the open, fearless.
2. **Validation.** `commit(tx, ...)` takes the store latch **once** and asks: for every
   photograph, does the store's current version still match? One mismatch → return
   `false`, the whole attempt is garbage.
3. **Write phase.** Under the *same* guard — validation and installation must be one
   atomic instant, or a rival could slip between them — install every draft, bumping
   versions.

Note the signature: `fn commit(tx: Tx, ...) -> bool` takes the transaction **by value**.
Commit *consumes* the notebook; the type system enforces "a transaction ends exactly
once" — no double-commit, no post-commit writes, at compile time.

The racing deposits now produce OCC's signature sound:

```
committed: 100 -> 110
CONFLICT on x — retrying
committed: 110 -> 130
```

Same final 130 as 2PL — but the loser *redid work* instead of waiting. And the auditor
exhibit (`read_skew_occ.rs`) shows the subtlest clause: **read-only transactions must
validate too.** The auditor's first attempt reads x and y five milliseconds apart,
straddling a transfer — its view tears (sees 130). Validation catches the stale
photograph and the tear becomes *unreportable*:

```
audit: view TORE (saw x=50, y=80 = 130) — retrying
audit: TOTAL 100   (x=20, y=80)
```

An honest hole, kept as a teaching artifact: validation's `if let` silently passes a
photographed key that has *vanished* from the store. Harmless here (nothing deletes), but
a real engine needs deletion to install a versioned **tombstone**, not remove the entry
(exercise 4 — and Module 04 met the same resurrection bug in replication).

## 5. MVCC and snapshot isolation (M4): sidestepping

2PL's auditor waited; OCC's auditor retried. MVCC's auditor does neither, ever — because
the store **never discards old versions**:

```rust
struct Db {
    clock: u64,                              // ticks once per commit
    data: HashMap<String, Vec<(u64, i64)>>,  // per key: full version history
}
```

A transaction is born with a **snapshot timestamp** — the clock at `begin` — and one rule
governs every read: `read_at` returns the newest version whose commit timestamp is **≤ my
snapshot** (implemented as a reverse walk down the sorted history). The transaction
experiences the entire store frozen at one instant. Two reads ten milliseconds apart,
with a transfer committing in between, *cannot* disagree about the instant they describe:

```
committed: x -> 100, y -> 130      ← the transfer commits INSIDE the auditor's read gap
audit: x=130 y=100 TOTAL 230 (first try, no retry)   ← …and the auditor reads y as of its snapshot anyway
```

(The demo asserts this interleaving — the `committed` line must precede the `audit`
line — so the exhibit provably exercises the snapshot, not just a quiet store.)

Contrast the engines on the same torn-timing: OCC reads *the present, twice* — and the
present moved. MVCC reads *a fixed past* — and the past never changes. That is the whole
difference, and it is bought entirely by the version list: "the bank at time 5" remains
an answerable question after time 6 overwrites nothing.

Consequently the read-only transaction's lifecycle collapses: **no commit call exists**.
It begins, reads, and lets its `Tx` drop — there is nothing to validate, so there is
nothing that can fail. (Scope the "never" honestly: it holds here because versions are
never garbage-collected. A real engine that evicts old versions can still fail a long
reader — Oracle's "snapshot too old" — and under SSI even read-only transactions can be
aborted unless declared deferrable.) This is why every serious analytical read in
Postgres or Oracle runs against a snapshot.

Writers still race, and MVCC polices them the OCC way — **first-committer-wins**: at
commit, for each key in my *write set*, if the history contains any version newer than my
snapshot, someone beat me to it — CONFLICT, retry. (Scene 1 of `mvcc.rs`: racing deposits,
one retry, final 130.) The whole engine is one sentence: **OCC for writes, time travel
for reads.** The costs are storage (histories grow without garbage collection —
exercise 5; Postgres calls the collector VACUUM) and the timestamp machinery itself.

## 6. Write skew: the crack in the armor

First-committer-wins validates *only the write set* — that narrowing is exactly what
freed the readers. Here is what it costs (`write_skew.rs`).

**The bank's rule:** x + y ≥ 0 — one account may overdraw if the other covers it. Every
withdrawal therefore checks the *combined* balance before proceeding. Both accounts hold
100. Two withdrawals of 150 run concurrently — one against x, one against y. Each begins,
reads **both** accounts on its snapshot (total 200 — plenty), and writes **only its own**:

```
withdraw 150 from y: committed (I checked: total was 200, rule holds)
withdraw 150 from x: committed (I checked: total was 200, rule holds)
final: x=-50 y=-50 total=-100
INVARIANT BROKEN: x + y < 0 — both txs committed, each certain the rule held.
```

No CONFLICT was printed, because none exists to print: the write sets are **disjoint** —
{x} and {y} — so each transaction's first-committer-wins check inspects a key the other
never wrote. Both validations pass honestly. Every transaction told the truth about its
snapshot; the *composition* lied. This is **write skew**: the constraint spans keys that
each transaction *reads but does not write*, and a write-set-only check is structurally
blind to it. (It is also precisely a violation of §3's footprint law, read-side half.)

Score the engines on this one anomaly and the module's arc inverts:

- **Strict 2PL prevents it.** The footprint law makes each withdrawal lock *both* keys
  (shared on the one it reads, exclusive on the one it writes); the S/X conflict
  serializes them, and the second withdrawal sees total 50 and refuses.
- **OCC detects it.** OCC validates the *read set* — T2 photographed x, T1's commit
  bumped x's version, T2's validation fails. The "primitive" engine of §4 beats the
  sophisticated one here, exactly *because* its validation is broader.
- **Snapshot isolation admits it.** Both commit; the invariant dies. SI is therefore
  strictly weaker than serializability — and this anomaly is the separator. Berenson et
  al. (1995) introduced it to show SI fits nowhere in the ANSI ladder; Oracle shipped SI
  under the name SERIALIZABLE for years.

The whole divide fits in the two questions the engines ask at commit:

> **OCC asks: "did anything I *read* change?"** — T2 read x, so its stale photograph of x
> is caught.
> **MVCC asks: "did anything I *wrote* change?"** — T2 wrote only y, so x is never
> inspected.

MVCC's narrower question is not a bug but the module's central trade made visible: a
snapshot makes every read untouchable (whatever others append, `read_at(my snapshot)`
answers identically forever), so reads *need* no validation — which is exactly what frees
read-only transactions from ever failing. But "my reads are a consistent picture of my
snapshot's instant" is not "my reads are still true *now*, at commit" — and a constraint
over keys you read but did not write lives precisely in that gap.

**The repair, in outline — serializable snapshot isolation** (Cahill–Röhm–Fekete 2008).
Track, cheaply and pessimistically, the **rw-antidependencies** between concurrent
transactions (T1 → T2 when T1 *read* an item of which a concurrent T2 wrote a newer
version — T1 saw the old one); a *dangerous structure* — two consecutive
rw-antidependency edges — is the necessary skeleton of every SI anomaly, and aborting one
transaction in it restores serializability without locks. PostgreSQL's SERIALIZABLE level
has been exactly this since 9.1 (Ports & Grittner 2012). The manual, folkloric fix is
**materializing the conflict**: force the write sets to overlap (both withdrawals also
update a common row; `SELECT ... FOR UPDATE`) so first-committer-wins can see the race —
exercise 6.

## 7. Three postures, three currencies

| | lost update | read skew | write skew | deadlock | read-only can fail? | pays in |
|---|---|---|---|---|---|---|
| naive (latch only) | ✗ admits | ✗ admits | ✗ admits | — | — | correctness |
| **strict 2PL** | ✓ prevents | ✓ prevents | ✓ prevents* | ✗ its disease | no — but it *waits* | **waiting** |
| **OCC** | ✓ detects | ✓ detects | ✓ detects | none | **yes** — retries | **wasted work** |
| **MVCC / SI** | ✓ detects (FCW) | ✓ impossible | ✗ **admits** | none | **no — never**† | **storage** |

\* given the full footprint (§3); forget the read-side locks and 2PL degrades to SI's blindness.
† in this engine (unbounded histories, no SSI); real systems can evict old versions
("snapshot too old") or abort read-only transactions under SSI.

One sentence each: **2PL prevents** — lock everything you touch, conflicting transactions
wait, nothing bad ever happens. **OCC detects** — run free, check at commit, redo on
collision. **MVCC sidesteps** — keep every version, read a frozen instant, collide only
on writes. Pessimist, optimist, time-traveler — and the engineering choice is a workload
question: high contention favors locking (retry storms waste more than queues), low
contention favors optimism, read-heavy analytics demand snapshots. Production engines
mix all three tiers: PostgreSQL is MVCC with SSI on top (when SERIALIZABLE is
requested; the default level is snapshot-style) and ordinary latches below;
InnoDB pairs MVCC reads with 2PL-style row and next-key locks.

## Theory ↔ code

| concept | where |
|---|---|
| latch vs lock (two-tier) | store `Mutex` vs `locks: HashMap<String, RwLock<()>>` |
| anomaly exhibits | `lost_update.rs`, `read_skew.rs` |
| strict 2PL, S/X modes | `*_2pl.rs` — guards to the brace; `.read()` vs `.write()` |
| growing/shrinking phases | acquire lines / the closing brace |
| AB/BA deadlock + ordered cure | `deadlock_2pl.rs` / `deadlock_2pl_fixed.rs` |
| OCC read/validate/write | `occ.rs` — `Tx` notebook, `commit` consumes self |
| read-only must validate | `read_skew_occ.rs` — "view TORE" |
| version history + snapshot read | `mvcc.rs` — `Db::clock`, `read_at` reverse walk |
| first-committer-wins | `mvcc.rs::commit` — write-set-only version check |
| read-only can't fail | `mvcc.rs::audit` — no commit call exists |
| write skew | `write_skew.rs` — disjoint writes, dead invariant |

## Honest limitations

1. **The store latch is global.** One `Mutex` serializes all physical access; real
   engines use fine-grained latching (per page/partition) so the tiers scale
   independently. Here the latch is held for microseconds and the point is the *logical*
   tier, but the simplification is real.
2. **No undo.** The 2PL exhibits write in place and never abort; a real strict-2PL engine
   pairs locks with an undo log (Module 08 built the redo side). OCC and MVCC dodge this
   by construction — drafts and versions are never visible early.
3. **OCC's vanished-key hole** (§4): validation passes photographed keys that have been
   deleted. Fine here (no deletes); a real engine versions its tombstones.
4. **No garbage collection of versions.** MVCC histories grow forever; real MVCC ties
   collection to the oldest live snapshot (VACUUM, purge). Exercise 5.
5. **No phantoms.** The footprint law as stated covers keys that *exist*; a transaction
   whose predicate is "all keys with prefix p" can be invalidated by an *insert* it never
   read. Predicate/next-key locking is the classical answer (Eswaran et al. 1976 saw this
   in the same paper that gave us 2PL). Exercise 8.
6. **Deadlock handled by static ordering only** — no waits-for detection, no victim
   selection. Exercise 3.
7. **Single machine.** These locks span threads, not machines. Module 08's `prepared`
   state *is* strict 2PL stretched across a network — and distributed MVCC needs a
   distributed clock, which is Spanner's TrueTime story (planned case-study notes).
8. **The exhibits are schedule-rigged, not schedule-proof.** The races are made
   near-deterministic by generous sleeps, not synchronized by barriers; on a heavily
   loaded machine an exhibit can miss its interleaving (the demo scripts then fail
   honestly rather than lie). Module 03's closing lesson applies verbatim: a demo can
   show one schedule, never quantify over all of them — that is the formal-methods
   bridge's job.

## Exercises

1. **Dirty read.** Add an abort path to the naive deposit (write, sleep, then undo);
   show a concurrent reader acting on the undone value. Then argue from the guard scopes
   why strict 2PL makes the exhibit unwritable.
2. **The upgrade deadlock.** Two auditors-turned-writers each take `.read()` on x, then
   both request `.write()` on x. Deadlock — with identical acquisition orders! Explain
   why sorted keys don't save you, and why real lock managers offer atomic upgrade (or
   `SELECT ... FOR UPDATE` acquires X eagerly).
3. **Detection.** Build a waits-for graph for the lock table (who holds, who wants);
   detect the AB/BA cycle at runtime and kill a victim. What must be true of the victim's
   transaction for "kill and retry" to be sound?
4. **Tombstones for OCC.** Add `delete` to the OCC engine such that validation catches
   "the key I read no longer exists." (Deletion bumps a version rather than removing it.)
5. **VACUUM.** Add version garbage collection to MVCC: a version is dead when a newer
   version's timestamp is ≤ the oldest live snapshot. Track live snapshots; prove your
   collector never frees a reachable version.
6. **Materialize the conflict.** Fix `write_skew.rs` *without* changing the engine: make
   both withdrawals write a common key. Show first-committer-wins now serializes them,
   and measure what it costs the non-conflicting case.
7. **SSI-lite.** Extend the MVCC `Tx` with a read set (keys only); at commit, abort any
   transaction whose read set intersects a concurrent committer's write set. Show it
   kills write skew — then explain why it aborts more than Cahill's dangerous-structure
   test (hint: run the scene-2 auditor under it).
8. **Phantom.** Add `sum_prefix("acc:")` used by an auditor while another transaction
   *inserts* `acc:new`. Exhibit the phantom under all three engines; sketch the predicate
   lock that would catch it.
9. **The crossover.** Benchmark 2PL vs OCC while sweeping contention: T threads hammering
   1 key vs T keys. Find the workload where the retry storm loses to the queue, and plot
   wait time vs wasted work (Module 03 benchmarking discipline: warm, repeated, baselined).
10. **Toward the formal bridge.** Model first-committer-wins in a specification language
    (Quint/TLA+): states = version histories + live snapshots, invariant =
    serializability of committed history. The model checker should hand you write skew
    as a counterexample — the exhibit of §6, found by search instead of by timing.

## Running everything

```sh
cargo build

# M1 — the anomalies
cargo run --bin lost_update
cargo run --bin read_skew

# M2 — strict 2PL: cures and price
cargo run --bin lost_update_2pl
cargo run --bin read_skew_2pl
cargo run --bin deadlock_2pl        # freezes forever, by design — Ctrl-C
cargo run --bin deadlock_2pl_fixed

# M3 — OCC
cargo run --bin occ
cargo run --bin read_skew_occ

# M4 — MVCC and the finale
cargo run --bin mvcc
cargo run --bin write_skew

# scripted, with verdicts (module 03 house style)
python3 demos/anomalies.py
python3 demos/strict_2pl.py         # includes the deadlock-freeze-by-timeout exhibit
python3 demos/occ.py
python3 demos/mvcc.py
```

## References

- P. A. Bernstein, V. Hadzilacos, N. Goodman, *Concurrency Control and Recovery in
  Database Systems*, Addison-Wesley, 1987. The classical text; 2PL theory,
  serializability. Freely available from the authors.
- K. P. Eswaran, J. N. Gray, R. A. Lorie, I. L. Traiger, "The Notions of Consistency and
  Predicate Locks in a Database System," *CACM* 19(11), 1976. The 2PL theorem — and,
  in the same paper, predicate locks and the phantom problem.
- H. T. Kung, J. T. Robinson, "On Optimistic Methods for Concurrency Control," *ACM
  TODS* 6(2), 1981. OCC: read/validation/write phases.
- H. Berenson, P. Bernstein, J. Gray, J. Melton, E. O'Neil, P. O'Neil, "A Critique of
  ANSI SQL Isolation Levels," *SIGMOD* 1995. The anomaly taxonomy; snapshot isolation
  named and separated from serializability by write skew.
- M. J. Cahill, U. Röhm, A. D. Fekete, "Serializable Isolation for Snapshot Databases,"
  *SIGMOD* 2008. SSI: dangerous structures of rw-antidependencies.
- D. R. K. Ports, K. Grittner, "Serializable Snapshot Isolation in PostgreSQL," *VLDB*
  2012. SSI in production.
- D. P. Reed, *Naming and Synchronization in a Decentralized Computer System*, MIT PhD
  thesis, 1978. Multiversion origins.
- C. H. Papadimitriou, "The Serializability of Concurrent Database Updates," *JACM*
  26(4), 1979. Serializability theory; testing conflict-serializability.
- J. Gray, A. Reuter, *Transaction Processing: Concepts and Techniques*, Morgan
  Kaufmann, 1993. The engineer's encyclopedia; isolation degrees.

---
*[Course home](../) · Previous: [Module 08](../08-two-phase-commit/) · Next:
[Module 10](../10-byzantine-broadcast/)*
