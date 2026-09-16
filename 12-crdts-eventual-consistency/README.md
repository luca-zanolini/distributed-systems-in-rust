# Module 12 — Eventual Consistency, CRDTs, and Gossip

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 04](../04-replicated-kv-store/) (replication, the resurrection bug),
[Module 06](../06-logical-time-broadcast/) (vector clocks, causal broadcast),
[Module 09](../09-concurrency-control/) (lost update, last-writer-wins).
**Status: complete.***

**Abstract.** Every module of this course so far has bought agreement with coordination:
quorums that refuse minority writes, leaders that serialize, commit protocols that block,
Byzantine machinery that votes. This closing module takes the other branch of the CAP
trade-off and shows it is not a resignation but a theorem: replicas that **never refuse a
write and never wait for each other** — offline phones, partitioned datacenters — can
still be *guaranteed* to converge, provided the data structure itself is chosen so that
merging cannot lose, duplicate, or misorder information. The structures are **CRDTs**
(conflict-free replicated data types), the algebra is the **join-semilattice**, the
guarantee is **strong eventual consistency**, and the transport is **gossip**. We build
the classical menagerie — G-Counter, PN-Counter, OR-Set — each preceded by the failure
that motivates it, and end with a five-replica epidemic that heals a partition on screen.
Everything here is deterministic: a replica is a value, gossip is a function call, and
not one thread, socket, or sleep appears in the module.

## Learning objectives

After this module you can:

- state the two naive-merge failure modes (winner-picking loses concurrent work;
  accumulation double-counts duplicated deliveries) and exhibit both in ten lines;
- define a state-based CRDT formally — join-semilattice, inflationary updates,
  merge = least upper bound — and check a candidate design against the definition;
- state strong eventual consistency and explain why the ACI laws quotient out every
  delivery pathology (reordering, regrouping, duplication);
- design by the two recurring tricks: *record events, not effects* (PN) and *name
  occasions, not just values* (OR-Set tags);
- explain why removal is the hard operation of replicated data, and compare G-Set,
  2P-Set, and OR-Set semantics on re-add and on concurrent add/remove;
- run and reason about pull-based epidemic gossip, including why convergence needs no
  coordinator and how a partition merely delays the join.

## 1. System model: the AP regime

N replicas of one object. Each replica **accepts every local operation immediately** —
no locks, no quorums, no leader, no waiting. Pairs of replicas occasionally exchange
**state** over links that may delay, reorder, and duplicate messages arbitrarily (we do
not even assume FIFO). There are no Byzantine replicas: everyone follows the protocol
(see Honest limitations — this assumption is load-bearing).

In CAP terms this is the AP corner: available under partition, consistency deferred. The
promised consistency is **strong eventual consistency (SEC)** (Shapiro et al. 2011):

> **Eventual delivery** — an update applied at one correct replica eventually reaches
> all; **Convergence** — two replicas that have received the *same set* of updates are
> in the *same state* — regardless of delivery order, grouping, or multiplicity.

Note what the second clause quantifies over: the *set*, not the *sequence*. SEC is
strictly stronger than folklore "eventual consistency" ("stops changing eventually,
values agree eventually, promise"): converged state is a *function of the updates*, so
agreement needs no extra round of reconciliation — it is already computed.

The simulation style follows the model: a replica is a Rust value, an update is a method
call, gossip is `replica_i.merge(&msg)` where `msg` is a **clone** of the neighbor's
state — the clone *is* the network message, and the borrow checker forces the simulation
to admit it (`replicas[i].merge(&replicas[j])` double-borrows the `Vec`). Adversarial
delivery becomes adversarial *call schedules*: we merge twice (duplication), in strange
orders (reordering), and through third parties (relay), all deterministically.

## 2. The founding puzzle (M1): two bare integers cannot be merged

`naive_counter.rs`. Replicas a and b count during a partition: a records 3 increments,
b records 5; the true global count is 8. The partition heals and each side must merge
two `u64`s. Both obvious merges fail, each with a familiar disease:

- **`merge = max`** → both say 5. A's three increments *vanish* — max treats two
  disjoint concurrent contributions as comparable versions of one quantity and picks a
  winner. This is Module 09's **lost update** wearing replication clothes.
- **`merge = add`** → 8, correct — once. Gossip has no exactly-once: B's state arrives
  a second time and the counter says 13. Add is not **idempotent**; it cannot tell new
  news from old news re-delivered.

```
During partition: a=3, b=5 (true global count: 8)
After merge (max): merged_max=5          ← 3 increments LOST
After merge (add): merged_add=8          ← correct…
B's gossip arrives AGAIN: 13             ← …once. The same news counted twice.
```

The impossibility generalizes: max is duplicate-safe but lossy, add is lossless but
duplicate-poisoned, and **no function of two bare integers escapes**, because a single
integer carries no memory of *which increments it already contains*. The state is too
small. Every CRDT design begins with this move: **enlarge the state until merge has
enough structure to be safe.**

## 3. The G-Counter (M2): split the counter by author

`g_counter.rs`, engine in [`src/lib.rs`](src/lib.rs). One slot per replica:

```rust
pub struct GCounter {
    pub slots: Vec<u64>,   // index = replica id; I may only bump my own
}
```

The sacred rule: **replica i increments only `slots[i]`.** Value = sum of slots;
**merge = element-wise max**. My array is my *belief about everybody*: `slots[j]` is the
highest count I have heard replica j reach — my own slot is exact, others' may lag but
can never lie high, because slot j has a single writer and only grows. Under that
invariant, "bigger" really does mean "newer" *slot-wise*, so max loses nothing — the
exact property whose absence convicted max in M1, now true by construction. And max is
idempotent, so duplicates bounce.

The demo tortures delivery on purpose — a duplicate, a weird order, news of replica c
reaching a only *through* b (transitive relay, no special handling) — and ends:

```
final a: [3, 5, 2] = 10
final b: [3, 5, 2] = 10
final c: [3, 5, 2] = 10
```

Structurally this vector-of-monotone-counters merged by pointwise max is **Module 06's
vector clock**, promoted from causality metadata to the data itself.

### 3.1 The algebra: why this *had* to work

Check the merge's laws: **associative** (regrouping a gossip cascade is harmless),
**commutative** (who merges whom doesn't matter), **idempotent** (merge(x,x) = x). ACI —
one law per network pathology. A state set with an ACI merge is a **join-semilattice**:
states carry a partial order (for G-Counters, pointwise ≤ — "y knows everything x
knows"), and merge computes the **join ⊔**, the *least upper bound*: not a winner, not a
pile, but the smallest state containing both sides' knowledge.

**Definition (state-based CRDT).** A join-semilattice of states (S, ⊑, ⊔), a bottom
start state, **updates that are inflations** (s ⊑ update(s) — local operations only move
up), queries that read without writing, and **merge = ⊔** exactly.

**Theorem (SEC).** Under this contract, two replicas that have absorbed the same *set*
of updates — any order, any grouping, any multiplicity — are in the same state: the join
of those updates. *Proof sketch:* ACI makes the join of a finite set well-defined
independent of arrangement; inflationary updates and join-merges mean every replica's
state is exactly the join of the updates it has absorbed. The delivery schedule is
quotiented out by the algebra. ∎

Convergence, in other words, is not achieved by the network — it is *computed* by each
replica, locally, from whatever fragments arrive. The instructive edge case: `(u64, max)`
**is** a lawful CRDT — it converges! — to the *wrong answer* for a counter, because its
order treats concurrent contributions as comparable. CRDT-ness buys convergence;
**it does not buy meaning**. Correct design = a lattice (SEC) *plus* an encoding whose
converged state implements the promised abstraction. The G-Counter's vector is the
minimal state enlargement satisfying both.

## 4. The PN-Counter (M3a): record events, not effects

Users also retract. But a decrement `slots[me] -= 1` is a **non-inflationary update** —
it moves my state *down* the order, the SEC theorem's hypothesis dies, and concretely:
my old, higher value still circulates in others' arrays and in-flight messages, and the
next merge resurrects it — max cannot distinguish deliberate retreat from stale news.
This is Module 04's **deleted-key resurrection**, replayed in arithmetic.

The cure (`pn_counter.rs`): never subtract — **add to a different pile**. Two
G-Counters: P for increments, N for decrements; a decrement *increments* my slot in N;
visible value = P − N, computed at read time. Both piles grow monotonically, merge
slot-wise, and the demo shows a retraction surviving duplicated gossip:

```
final a: P=[3, 2] N=[1, 0] = 4
final b: P=[3, 2] N=[1, 0] = 4
```

The generalizing trick, used everywhere in CRDT design: **any non-monotone operation
becomes monotone by recording the *event* instead of applying the *effect*.** Events
only accumulate; the value is a query over the event record. (Also the module's first
**composition**: a product of CRDTs, merged component-wise, is a CRDT.)

## 5. Sets, and the hard problem of removal (M3b)

A set (`or_set.rs`) replicates *named membership* — a cart, a group, a to-do list — and
its ladder recapitulates the module:

1. **Adds only:** grow-only set (G-Set), merge = union. A perfect semilattice, trivially.
2. **Naive remove** (delete locally): non-monotone → the resurrection bug verbatim —
   every merge with a stale replica unions the element back in.
3. **The PN trick — Two-Phase Set:** second grow-only pile of **tombstones**; present =
   added ∧ ¬removed. Monotone, converges — and broken as a *set*: the tombstone kills
   the element's **name forever**. Monday's removal of "milk" out-vetoes Tuesday's
   honest re-add, eternally. The tension is precise: a remove must beat the *past* adds
   but lose to *future* ones — and the name "milk" is the same string all week, so no
   name-level tombstone can tell them apart.

**The OR-Set (observed-remove).** Make each add a distinct *event*: `add("milk")` mints
a globally unique **tag** — `(replica_id, local_seq)`, unique with no coordination, the
vector-clock trick's third career — and stores `("milk", tag)`. `remove("milk")`
tombstones **exactly the tags it currently observes** — you cannot kill what you have
not seen. Present = *some tag not tombstoned*.

```rust
adds:  HashMap<String, HashSet<Tag>>,  // element -> every add-event ever
tombs: HashSet<Tag>,                   // the observed-and-killed events
```

Merge = union on both piles (the join, coordinate-wise). The demo runs the three scenes
that justify the machinery:

```
a removes banana:   a contains banana? false
a RE-ADDS banana:   a contains banana? true   (a 2P-Set would be stuck at false forever)
stale b gossips in: a contains banana? false  (old tags hit a's tombstones — no resurrection)
concurrent remove(a) vs add(b), healed: a: true, b: true — ADD WINS
```

The last line deserves its name spelled out: when a remove and an add of the same
element race across a partition, *there is no correct answer* — the type's designer must
choose. OR-Set chooses **add-wins** (the remove tombstoned only tags it observed; the
concurrent add's fresh tag is untouched), which is what a shopping cart wants: when in
doubt, keep the customer's milk. A remove-wins variant is a legitimate alternative
design (exercise 6).

Two closing notes. *Sets are not carts:* membership is boolean; quantities need a
composed **map of PN-Counters** (per-item counts, merged per-key) — essentially how
Dynamo's cart should have worked; its actual anomalies (deleted items resurfacing) are
this module's exhibits in production. *A slogan for the two tricks:* counters needed
**who** in the state (slots); sets need **which occasion** (tags). A lattice must
remember exactly enough history for merge to never confuse distinct events.

## 6. Gossip (M4): convergence as an epidemic

`gossip.rs`. Five replicas, each counting locally (replica i counts i+1; true total 15).
A partition splits them {0,1} | {2,3,4}. Gossip is **pull-based and pairwise** on a fixed
deterministic rotation — in round r, replica i pulls a state-clone from neighbor
(i + r) mod 5; partitioned rounds drop cross-split pairs on the floor:

```
--- after 3 partitioned rounds ({0,1} | {2,3,4}) ---
  replica 0: [1, 2, 0, 0, 0] = 3     ← knows its island only
  replica 4: [0, 0, 3, 4, 5] = 12    ← a different truth, equally consistent
--- after 1 healed rounds ---
  all five: [1, 2, 3, 4, 5] = 15
VERDICT: all 5 replicas identical = 15 — epidemic convergence, zero coordination
```

Three observations the run makes concrete. **The partition is not an error state:** each
island serves reads and writes throughout, at full availability — the two truths are
both honest partial joins, and healing is just more merging, not a recovery protocol.
**News spreads transitively** — replica 0 learns of replica 4 through intermediaries
that happened to gossip in the right rotation; in randomized gossip this percolation
completes in O(log n) expected rounds (Demers et al. 1987, the classic epidemic
analysis). **Direction matters in the small:** our pull-rotation leaves replica 1
ignorant of replica 0 for all three partitioned rounds — deterministic schedules make
such asymmetries visible where randomness would blur them.

What real anti-entropy adds and we simulate away: exchanging **digests** first
(hashes/version vectors) so peers ship only what differs — Merkle trees in Dynamo and
Cassandra make "what differs" logarithmic; and **membership itself by gossip** (SWIM),
where who-is-alive is one more eventually-consistent, epidemic-maintained data set.

## 7. The other formulation, and the course's two arcs closing

**Op-based CRDTs.** Ship *operations* instead of states, applied at every replica;
correctness requires concurrent operations to **commute**, and the transport to deliver
each op exactly-once in **causal order**. That transport is not hypothetical here: it is
**Module 06's waiting causal broadcast**, verbatim. State- and op-based formulations are
equivalent in power (Shapiro et al. prove it by mutual emulation); state-based pays in
message size (whole states — delta-CRDTs repair this), op-based pays in transport
obligations. This module is state-based throughout because its transport assumptions
are almost nothing.

**LWW: the tempting fraud.** The "simplest" register CRDT — last-writer-wins by
timestamp — is a lattice (max on (ts, value) pairs) and converges beautifully. But you
spent Module 09 learning what last-writer-wins *means*: silent lost updates, now with
clock skew choosing the loser. LWW is a legitimate *choice* only when overwriting
concurrent work is acceptable semantics; it is the naive `max` of M1 with a wall-clock
alibi. (Exercise 1 builds it and exhibits the loss.)

And the closing contrast the whole course has been building to: **consensus makes
replicas agree on an *order of operations*; CRDTs make order irrelevant.** Modules 05–11
paid latency, availability, and machinery to serialize; Module 12 pays in *expressiveness*
— only lattice-shaped data, only designer-chosen conflict semantics, no invariants that
span concurrent updates (Module 09's write skew has no CRDT cure: "x + y ≥ 0" cannot be
maintained by parties that don't communicate — some problems *are* consensus problems).
Real systems mix regimes: CRDTs for carts, presence, counters, collaborative text;
consensus for balances, uniqueness, configuration. Knowing *which data belongs where*
is the actual engineering skill this module and Module 07 jointly teach.

## Theory ↔ code

| concept | where |
|---|---|
| the two naive-merge failures | `naive_counter.rs` — max forgets, add double-counts |
| join-semilattice, merge = ⊔ | `GCounter::merge` in `src/lib.rs` — pointwise max |
| updates are inflations | `GCounter::increment` — one slot up, none down |
| SEC through hostile delivery | `g_counter.rs` — duplicate + reorder + relay, `[3,5,2]` ×3 |
| record events, not effects | `pn_counter.rs` — decrement increments the N pile |
| composition (product CRDT) | `PNCounter { pos, neg }` — merge both, component-wise |
| unique tags without coordination | `or_set.rs` — `Tag = (replica_id, seq)` |
| observed-remove, add-wins | `or_set.rs` scenes 1–3 |
| gossip as clone + merge | `gossip.rs::round` — the clone is the network message |
| partition and heal | `gossip.rs` — same_side gate, then the join finishes the job |

## Honest limitations

1. **No Byzantine tolerance — and the assumption is load-bearing.** A lying replica can
   set `slots[me] = 10^18` or tombstone tags it never saw, and every honest replica will
   *converge to the lie* (merge trusts all input). CRDTs assume crash-prone but honest
   replicas; Byzantine-tolerant CRDTs are an active research area, and Modules 10–11
   show the price of dropping the assumption.
2. **Tombstones and tags grow forever.** Our OR-Set never garbage-collects `tombs`, and
   `adds` keeps dead tags; the optimized OR-Set (Bieniusa et al. 2012) and
   version-vector-based formulations bound this — at the price of the causal bookkeeping
   we simulated away. Same for G-Counter width: one slot per replica ever.
3. **Whole-state shipping.** Real state-based systems ship deltas (delta-CRDTs, Almeida
   et al. 2018) or digest-first diffs; our clones are the honest but naive baseline.
4. **Replica identity is static and dense** (ids 0..n at birth). Real systems face
   joining/leaving replicas, id reuse, and membership itself as a replicated dataset
   (SWIM).
5. **In-process simulation.** No serialization, no real links — deliberately: the module
   isolates the algebra. Modules 04/06 own the wire; composing the two is exercise 9.
6. **Convergence ≠ meaning** (§3.1): SEC is necessary, not sufficient — the abstraction
   each converged state implements is a separate design obligation, chosen per type
   (add-wins vs remove-wins has no "correct" answer).
7. **No cross-object invariants.** Anything spanning concurrent updates at different
   replicas (uniqueness, conservation, x+y ≥ 0) is out of scope *by nature* — that is
   the consensus boundary, not an implementation gap.

## Exercises

1. **LWW-Register.** Build it: `(timestamp, replica_id, value)`, merge = max by
   (ts, id). Exhibit a concurrent write silently lost, and explain why the tiebreaker
   id makes it a lattice but not a fair one.
2. **The cart.** Compose `HashMap<String, PNCounter>` (merge per-key) into a quantity
   cart. Two replicas each add one milk concurrently → 2. What does *removing more than
   you observed* do to quantities, and is that acceptable semantics?
3. **G-Set and 2P-Set.** Build both in twenty lines from the lib; reproduce the
   no-re-add flaw as a failing scene, mirroring `or_set.rs` scene 1.
4. **Tombstone GC.** Add a `stable(tag)` oracle to the OR-Set (a tag is stable when
   every replica has seen it — deliverable from version vectors) and purge stable
   tombstones. Argue why purging an *unstable* tombstone re-opens resurrection.
5. **Prove the OR-Set lattice.** Define the order ((A₁,T₁) ⊑ (A₂,T₂) iff A₁ ⊆ A₂ ∧
   T₁ ⊆ T₂), show merge is its join, adds and removes are inflations, and derive
   add-wins from "remove tombstones only observed tags."
6. **Remove-wins.** Redesign the set so concurrent remove beats add (hint: tombstone
   *elements* with a causal context, or tag removes and let adds carry the removes they
   have seen). What application wants this semantics?
7. **Byzantine G-Counter.** One replica inflates a foreign slot. Trace the poison
   through gossip; then bound the damage with signed per-slot updates (Module 10's
   machinery) and state what *cannot* be fixed that way (a liar inflating its *own* slot).
8. **Delta gossip.** Ship only slots that changed since the last exchange per neighbor.
   Measure message sizes on the M4 scenario; state the new correctness obligation
   (deltas must still be joins of updates, and re-delivery must stay harmless).
9. **Over the wire.** Port the G-Counter onto Module 04's TCP skeleton: `merge` verb
   carrying a serialized state, periodic anti-entropy thread, kill-and-heal demo. The
   algebra shouldn't notice the network.
10. **Toward the formal bridge.** Specify SEC for the G-Counter in Quint/TLA+: states,
    a nondeterministic gossip action, and the invariant that any two replicas with equal
    delivered-sets have equal state. Then break the spec (allow `slots[me] -= 1`) and
    let the checker find the resurrection counterexample that M3a narrates.

## Running everything

```sh
cargo build

cargo run --bin naive_counter   # M1: both bad merges
cargo run --bin g_counter       # M2: convergence through hostile delivery
cargo run --bin pn_counter      # M3a: decrement as a grow-only event
cargo run --bin or_set          # M3b: re-add, no-resurrection, add-wins
cargo run --bin gossip          # M4: partition + heal, epidemic convergence

# scripted, with verdicts
python3 demos/counters.py
python3 demos/or_set.py
python3 demos/gossip.py
```

## References

- M. Shapiro, N. Preguiça, C. Baquero, M. Zawirski, "Conflict-free Replicated Data
  Types," *SSS* 2011; and the companion INRIA report *A Comprehensive Study of
  Convergent and Commutative Replicated Data Types*, RR-7506, 2011. The definitions
  (CvRDT/CmRDT, SEC), the equivalence, and the original menagerie incl. OR-Set.
- A. Demers et al., "Epidemic Algorithms for Replicated Database Maintenance," *PODC*
  1987. Anti-entropy and rumor mongering; the O(log n) epidemic analysis.
- G. DeCandia et al., "Dynamo: Amazon's Highly Available Key-value Store," *SOSP* 2007.
  The AP design that mainstreamed the trade-off — and shipped the cart anomalies this
  module reproduces.
- A. Bieniusa et al., "An Optimized Conflict-free Replicated Set," INRIA RR-8083, 2012.
  OR-Set without unbounded tombstones.
- P. S. Almeida, A. Shoker, C. Baquero, "Delta State Replicated Data Types," *JPDC*
  2018. Shipping joins of small deltas instead of whole states.
- A. Das, I. Gupta, A. Motivala, "SWIM: Scalable Weakly-consistent Infection-style
  Process Group Membership Protocol," *DSN* 2002. Membership as an epidemic dataset.
- C. Baquero, N. Preguiça, "Why Logical Clocks Are Easy," *CACM* 59(4), 2016. The
  causality toolkit (vector clocks, version vectors) this module quietly reuses.
- M. Kleppmann, *Designing Data-Intensive Applications*, O'Reilly 2017, ch. 5. The
  practitioner's map of replication, conflicts, and convergence.

---
*[Course home](../) · Previous: [Module 11](../11-byzantine-consensus/) · This is the
final module of the built course.*
