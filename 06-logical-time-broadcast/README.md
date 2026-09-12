# Module 06 — Logical Time and Broadcast

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011), Chapter 3. Prerequisites:
[Module 02](../02-networked-kv-store/) (processes and links). Related:
[Module 10](../10-byzantine-broadcast/) §2 surveys the full broadcast ladder; this module
implements its crash-model core. **Status: Part I (reliable broadcast, logical clocks, causal
broadcast) complete; Part II (total-order broadcast over Module 07's consensus) planned.***

**Abstract.** Processes in a distributed system have no shared clock, and their messages arrive
in uncontrolled orders; yet applications need to reason about which events *could have
influenced* which. This module builds the classical answer in three steps. First, **reliable
broadcast** (CCGR Alg. 3.3, eager relaying) makes dissemination survive a crashing sender — and
an experiment with an out-of-model equivocating sender shows precisely where the crash-model
specification stops making promises. Second, **Lamport clocks** (Lamport 1977/78) timestamp
events consistently with the **happens-before** relation, at the price of *compressing away
concurrency*. Third, **vector clocks** (Fidge; Mattern) *characterize* happens-before exactly —
incomparable stamps certify concurrency — and power **causal-order broadcast** (CCGR Alg. 3.15,
"Waiting Causal Broadcast"): every message carries its author's causal history, and every
receiver holds it back until its own history has caught up. The demonstrations make each
property physical: a killed sender whose message survives, a message visibly *waiting* for its
cause, and two concurrent messages that different nodes deliver in different orders — the last
being exactly what causal order does *not* forbid, and the doorway to total-order broadcast,
which is equivalent to consensus.

---

## Learning objectives

After completing this module, the reader should be able to:

1. define the **happens-before** relation and concurrency, and explain why physical time is
   unavailable as an ordering principle in an asynchronous system;
2. specify regular **reliable broadcast** (RB1–RB4) and implement it by eager relaying; explain
   why crash-model *agreement* is a liveness property, and what an equivocating sender reveals
   about the limits of the crash-model specification;
3. define **Lamport clocks**, state their guarantee (`e → e′ ⟹ LC(e) < LC(e′)`), explain why
   the converse fails, and derive a total order from them by tie-breaking;
4. define **vector clocks**, state and prove the **characterization theorem**
   (`e → e′ ⟺ VC(e) < VC(e′)`) via the history-inventory lemma, and use incomparability as a
   *proof* of concurrency;
5. distinguish the two vector-clock bookkeeping conventions (event clock vs. prerequisite
   stamp) and explain why a stamping rule and a delivery test must form a matched pair;
6. specify **causal-order broadcast** (CRB) and implement the waiting algorithm: stamp with the
   causal past, park messages whose past is missing, drain on every delivery;
7. state precisely what causal order does **not** provide — one agreed order for concurrent
   messages — and why that step (total-order broadcast) is equivalent to consensus.

---

## 1. Motivation: ordering without clocks

In the asynchronous model there are no synchronized clocks and no delay bounds, so "what
happened first" cannot be answered with wall time — and even with perfect clocks, *earlier* is
not the relation applications need. What matters is **influence**: could event `e` have
affected event `e′`? A reply is *after* the message it answers in every sense that matters,
regardless of what any clock says.

Lamport's insight (1978) was to define order *from communication itself*. The
**happens-before** relation `→` is the smallest transitive relation such that:

1. if `e` precedes `e′` at the same process, then `e → e′`;
2. if `e` is the sending of a message and `e′` its receipt, then `e → e′`.

Events unrelated by `→` are **concurrent** (`e ∥ e′`): no causal path connects them, neither
could have influenced the other. `→` is a *partial* order — and that partiality is not a defect
but a fact about distributed executions that any honest timestamping scheme must represent.

This module asks three questions in sequence: how do we *disseminate* an event to everyone
despite crashes (§3)? how do we *timestamp* events respecting `→` (§4–§5)? and how do we
*deliver* messages in an order respecting `→` (§6)?

## 2. System model

- **Processes.** `N = 4` nodes, identified by address, with a globally agreed ordering
  (addresses sorted; `rank(p)` = index — every node computes the same ranks).
- **Failures.** Crash-stop. The eager-relay layer tolerates any number of crashes; the
  demonstrations kill at most one node.
- **Links.** Perfect point-to-point links (TCP), as in Module 02.
- **Timing.** Asynchronous — nothing in this module uses a timeout. (The `--slow-from` flag
  *injects* delay to make asynchrony's consequences visible; it assumes nothing.)

## 3. Reliable broadcast by eager relaying

**Specification (regular reliable broadcast; CCGR Module 3.2).** Events
⟨ Broadcast | m ⟩ and ⟨ Deliver | p, m ⟩, for any number of senders:

- **RB1 (Validity).** If a correct process `p` broadcasts `m`, then `p` eventually delivers `m`.
- **RB2 (No duplication).** No message is delivered more than once.
- **RB3 (No creation).** If a process delivers `m` with sender `s`, then `s` broadcast `m`.
- **RB4 (Agreement).** If a message `m` is delivered by some correct process, then `m` is
  eventually delivered by every correct process.

**Algorithm (Eager Reliable Broadcast; CCGR Alg. 3.3).** On first delivery of a message:
deliver it, then **re-broadcast it**. No failure detector, no timing assumption; if the origin
crashed mid-fan-out, whoever received the message relays it to everyone. Dedup by
`(origin, seq)` — the sender's per-origin sequence number, which already rides in the causal
stamp (keying on the *payload* would silently drop a repeated payload while the sender's
counter advanced, permanently wedging that origin's causal stream; a defect found by
adversarial review). Cost: `O(N²)` messages per broadcast. `demos/crash_relay.py` kills the
origin ~50 ms after it broadcasts; every survivor delivers — RB4 observed. (Honesty note: on
loopback the origin's own fan-out completes in ~1 ms, so this run does not *isolate* the
relay mechanism — the survivors hold direct copies; isolating the relay would require
suppressing part of the origin's fan-out.)

Note that **RB4 is a liveness property** (CCGR §3.3.1): it can always be satisfied by
extensions of a finite execution. Nothing about the *content* of deliveries can go irreparably
wrong in the crash model — a point the next paragraph sharpens.

**The model-gap experiment.** What happens when a *correct crash-model algorithm* meets an
adversary its model excludes? We ran an **equivocating sender** (one origin, value `a` to half
the cluster, `b` to the other half) against eager RB, milestone M1 of this module
(reproducible at commit `863b7da`, where the sender had an `equiv` command):

```
node 6001: delivered a, then b        node 6002: delivered b, then a   (all nodes: BOTH values)
```

Check the specification against this run: RB2 ✓ (each message once), RB3 ✓ (the sender did
send both), RB4 ✓ (the relays faithfully spread *both* values to everyone). **Every property
holds — and the outcome is nonsense anyway:** one "broadcast" produced two deliveries from one
sender. The crash-model specification cannot even express the violation, because its
properties quantify per-message and silently assume senders tag messages honestly (one
broadcast event ↔ one message). This is CCGR §3.10.1's exact argument for why the
fail-arbitrary model *reformulates* broadcast — per-instance, with a consistency property —
rather than reusing the crash-model specification: **a proof is only as good as its model.**
(The Byzantine reformulation and its algorithms are [Module 10](../10-byzantine-broadcast/).)

## 4. Lamport clocks

**Definition.** Each process keeps an integer `lc`, and: (1) increments it on each local event
(in this module: each broadcast); (2) attaches it to every message; (3) on receipt sets
`lc := max(lc, received) + 1`.

**Theorem (clock condition).** `e → e′ ⟹ LC(e) < LC(e′)`. *Proof:* both generating rules only
ever move a clock strictly upward along the two edges that generate `→`, and transitivity
follows from transitivity of `<`. ∎

**What the converse would say — and why it fails.** From `LC(e) < LC(e′)` one may *not*
conclude `e → e′`: integers are totally ordered, so the stamps always exhibit *some* order,
including between events that were concurrent. The Lamport clock **compresses the partial
order into a total one, and concurrency is what the compression discards.** Our M2 run
(commit `e480823`) showed both failure directions at once: two concurrent broadcasts received
*equal* stamps (`lc = 1` for both `ping` and `pong`), and nodes delivered them in different
orders while the stamps stood silent.

**What Lamport clocks are for.** Whenever *any* causality-consistent total order suffices:
sort by `(lc, process-id)` and every node computes the **same** total order of all events —
Lamport's own construction, used in his distributed mutual-exclusion algorithm, in
last-writer-wins registers, and (conceptually) wherever a system needs a global sequence
without a global sequencer. The order is *fair fiction*: it never contradicts causality, but
between concurrent events it is arbitrary — and no one can tell which parts are real.

## 5. Vector clocks

**Definition.** Each process keeps `V: Vec<u64>` with one entry per process. Interpretation and
maintenance are best stated as an invariant rather than a rule list:

> **Lemma (history inventory).** For any event `e′` and process `p`: `VC(e′)[p]` equals the
> number of `p`'s events in the causal past of `e′` (inclusive of `e′` itself).
>
> *Proof sketch.* Values of `p`'s counter reach other processes **only inside messages** (the
> receiver's max-merge), so a value `k` at `e′` traces back, hop by causal hop, to `p`'s `k`-th
> event — no causal path, no propagation (upper bound). Max-merges never lose values along an
> existing path (lower bound). ∎

**Theorem (characterization).** `e → e′ ⟺ VC(e) < VC(e′)` (componentwise `≤`, somewhere
strict). The forward direction is monotonicity along causal paths, as for Lamport clocks. The
converse — the direction Lamport clocks lack — follows from the lemma: if `e` is `p`'s `k`-th
event and `VC(e′)[p] ≥ k`, then `p`'s `k`-th event, i.e. `e`, is in `e′`'s causal past. In
practice one checks a **single component**: for distinct events `e ≠ e′`,
`e → e′  ⟺  VC(e)[origin(e)] ≤ VC(e′)[origin(e)]`.

**Concurrency becomes provable.** If `VC(e)` and `VC(e′)` are *incomparable* — each has some
strictly larger entry — then by the theorem neither `e → e′` nor `e′ → e`: the events are
**certified concurrent**. A zero in `e′`'s entry for `p` is positive evidence that *no* message
chain from `p` had reached `e′`. The codomain finally has the same shape as the thing measured:
vectors under componentwise `≤` form a partial order, so "no order" is an expressible answer.

**The price, and its optimality.** `O(n)` integers per message, and the membership must be
known to size the vector. This is not an implementation artifact: Charron-Bost (1991) showed
that characterizing causality in a system of `n` processes requires vector timestamps of
dimension `n`. An inventory of `n` independent histories needs `n` counters; systems that
cannot afford it (huge or dynamic `n`) retreat to Lamport scalars, per-key version vectors, or
dotted variants — giving up either detection or granularity.

### 5.1 Lamport vs. vector clocks

| | **Lamport clock** | **Vector clock** |
|---|---|---|
| guarantee | `e → e′ ⟹ LC(e) < LC(e′)` (one-directional) | `e → e′ ⟺ VC(e) < VC(e′)` (characterization) |
| concurrency | invisible (stamps always ordered) | **provable** (incomparable stamps) |
| size per message | `O(1)` | `O(n)`; needs known membership (optimal — Charron-Bost) |
| yields | a causality-consistent **total** order (with tie-break) | the causal **partial** order itself |
| use when | any consistent order will do: log ordering, LWW timestamps, mutual-exclusion queues, sequencing | the *absence* of order is information: causal delivery (this module), conflict detection (Dynamo's concurrent siblings), consistent snapshots, debugging races |

Rule of thumb: **to *order* events, Lamport suffices; to *know when there is no order*, pay
for vectors.** Causality can be *respected* in `O(1)` but *detected* only in `O(n)`.

### 5.2 Two bookkeeping conventions (a classic stumbling block)

The literature uses two equivalent-but-offset conventions, and mixing them produces either
premature deliveries or eternal waiting:

- **Event clock (Fidge–Mattern; §5's theorems).** Increment your own entry *first*, then
  stamp: a stamp **includes its own event** ("I am `p`'s 2nd event" reads `VC[p] = 2`). Pairs
  with the ISIS-style delivery test: deliver `m` from `j` when `ts(m)[j] = V[j] + 1` and
  `ts(m)[k] ≤ V[k]` otherwise.
- **Prerequisite stamp (CCGR Alg. 3.15; our code).** Stamp *before* incrementing:
  `W[self] = lsn` counts the sender's **earlier** messages, so the stamp **excludes its own
  event** — it is a *precondition list*, "deliver me only after your history covers this."
  Pairs with the uniform test `W ≤ V`.

Same mathematics, shifted by one in the origin's slot. The inviolable rule: **the stamping
rule and the delivery test must be a matched pair.** (Stamp after incrementing but test
`W ≤ V`, and every message lists *itself* as a prerequisite and waits forever.)

## 6. Causal-order broadcast

**Specification (causal-order reliable broadcast; CCGR Module 3.9).** RB1–RB4, plus:

- **CRB5 (Causal delivery).** If `broadcast(m₁) → broadcast(m₂)`, then no process
  delivers `m₂` unless it has already delivered `m₁`. (CCGR states this for *all* processes,
  not just correct ones — a per-process safety property — and the implementation satisfies
  that stronger form.)

**The idea, in one story.** I want to broadcast a message, and I attach my **history**: my
vector `V` — how many messages I have delivered from each process — with my own slot set to
how many messages *I* have broadcast before this one. You receive it, and your goal is to
deliver it — but *safely*: everything I had seen when I wrote it might have influenced it. So
you ask one question, slot by slot: **"have I already delivered everything the author had?"**
(`W ≤ your V`).

- **Yes** → nothing that influenced my message is missing at your end; deliver it, record it
  (`V[me] += 1`) — and since your history just grew, re-check your waiting room: my message
  may be exactly what someone else's message was waiting for.
- **No** — say `W[q] = 3` but your `V[q] = 2` → there is a message from `q`, possibly the very
  question mine answers, that hasn't reached you yet. Don't drop mine, don't deliver it —
  **park it**. When `q`'s message arrives and is delivered, the re-check releases mine, in the
  right order.

Reliability and order are separate layers with a clean division of labor: **the relay layer
guarantees my message reaches you; the stamp guarantees you don't hand it over too early.**

**Algorithm (Waiting Causal Broadcast; CCGR Alg. 3.15).** State: `V` (delivered-per-process),
`lsn` (own broadcasts), `pending` (parked `(origin, W, m)` triples).

```
broadcast(m):   W := V;  W[self] := lsn;  lsn += 1;  rb-broadcast (self, W, m)
on rb-deliver (origin, W, m):
    add (origin, W, m) to pending
    while ∃ (o', W', m') ∈ pending with W' ≤ V:      // componentwise
        remove it;  V[rank(o')] += 1;  crb-deliver (o', m')
```

*Correctness sketch.* CRB5: if `broadcast(m₁) → broadcast(m₂)`, then by the history-inventory
lemma `m₂`'s stamp dominates the entry counting `m₁` (its author had delivered `m₁`, or `m₁`
is its own earlier message, counted by the `lsn` slot); a process passes the `W ≤ V` test for
`m₂` only after its `V` covers that entry — i.e., only after delivering `m₁`. Liveness (no
eternal parking): the RB layer eventually delivers every message everywhere, so the causal
past named by any stamp eventually arrives and the drain loop's re-scan releases the chain;
the `while` re-scan is what lets one delivery unlock others. RB1–RB4 are inherited from the
relay layer underneath. ∎

**What causal order does *not* give.** Concurrent broadcasts carry incomparable stamps, and
CRB says nothing about their relative order: different nodes may deliver them differently
(`demos/concurrent_no_total_order.py` shows one node delivering `pong, ping` and the rest
`ping, pong`, all ending at the same `V`). Forcing **one agreed order** on causally unrelated
messages is **total-order broadcast** — equivalent to consensus (CCGR Ch. 6), which is why it
cannot live in this timeout-free module: it is Part II, to be built over
[Module 07](../07-raft/)'s Raft.

## 7. Development of the implementation

| # | Design | Property gained / lesson exposed |
|---|---|---|
| M1 | eager relay, dedup by `(origin, m)` | RB4 despite a crashing sender; the equivocator experiment: every RB property holds and the outcome is still nonsense — the spec's model boundary, found experimentally |
| M2 | Lamport clock (`lc`, max+1 on receipt), stamps in the log | concurrent broadcasts got *equal* stamps and divergent delivery orders: consistency with `→`, but no detection of `∥` |
| M3 | vector clocks + waiting causal delivery (`pending`, `W ≤ V`, drain) | causal order enforced: the *answer* visibly waits for its *question*; concurrent messages remain unordered — correctly |

(M1 and M2 live at commits `863b7da` and `e480823`; HEAD is M3. The `equiv` command exists
in the M1 and M2 revisions and was removed at M3.)

## 8. Correspondence between theory and code

| Concept | Realization (`src/main.rs`) |
|---|---|
| agreed process ranks | membership sorted identically everywhere; `rank_of(addr, all)` |
| eager RB (Alg. 3.3) | dedup via `delivered.insert((origin, m))`, then unconditional re-broadcast |
| prerequisite stamp | `w = v.clone(); w[my_rank] = lsn; lsn += 1` — stamp *before* count |
| deliverability `W ≤ V` | `w.iter().zip(v.iter()).all(\|(a, b)\| a <= b)` |
| waiting room | `pending: Vec<(String, Vec<u64>, String)>`; "Holding" log line when parked |
| drain (one delivery unlocks others) | `while let Some(i) = pending.iter().position(...)` → `remove(i)`, `v[rank] += 1`, re-scan |
| slow link (demo knob) | `--slow-from <addr> <ms>`: sleep before locking, for messages *originating* at `addr` |
| concurrent handling | listener is thread-per-connection (`Arc<Mutex<State>>` clones per thread) so a delayed message doesn't serialize the rest |

Wire format: `DATA <origin> <v0,v1,...> <m>` (vector comma-joined; parsed with
`split(',') … collect::<Option<Vec<u64>>>()` — all-or-nothing parsing).

## 9. Limitations and outlook

- **Total-order broadcast is Part II.** Concurrent messages are delivered in per-node orders;
  one agreed sequence requires consensus. Planned: a TOB layer over Module 07's Raft, closing
  the equivalence circle (TOB ⟺ consensus).
- **`pending` and `delivered` grow without bound.** Production systems garbage-collect
  stability information (messages known delivered everywhere); CCGR's Alg. 3.14 sketches the
  mechanism (acknowledgements + a perfect failure detector).
- **Static membership.** The vector's dimension and the ranks are fixed at launch; dynamic
  membership requires reconfiguration or dotted/version-vector variants.
- **Crash-stop, volatile state.** A recovering node would return with `V = 0` and re-deliver
  history (or block on it); crash-recovery needs logged variants (CCGR §3.6–3.7).
- **No-waiting alternative.** CCGR Alg. 3.13 achieves causal delivery with *no* parking by
  piggybacking the entire causal past on every message — constant latency, unbounded messages;
  the waiting algorithm inverts the trade. (Exercise 3.)
- **The M1 equivocator is out of model here** — tolerated analysis-wise by none of this
  module's algorithms; that is Module 10's problem, solved there with quorums.

## 10. Exercises

1. **(Lamport total order.)** Extend M2's logic: deliver messages in `(lc, sender)` order using
   a hold-back queue. What additional assumption about *message stability* do you need before a
   message can be safely delivered, and why does this reintroduce waiting?
2. **(The matched-pair rule.)** Take the M3 code and change the stamp to *post*-increment
   (`lsn += 1; w[my_rank] = lsn;`) without changing the delivery test. Predict precisely what
   happens, run it, and explain in one sentence. Then write the ISIS-style delivery test that
   *matches* the post-increment stamp.
3. **(No-waiting causal broadcast.)** Implement CCGR Alg. 3.13: piggyback the list of
   causally preceding messages instead of a vector; deliver the past before the message, no
   parking. Measure message sizes in a 100-broadcast run and compare with M3.
4. **(Consistent snapshots — Chandy–Lamport.)** Using the happens-before machinery: implement
   marker-based snapshots over the M3 cluster (a coordinator broadcasts a marker; each node
   records its state on first marker receipt and the messages arriving on each channel until
   the marker returns). Argue the recorded global state is a *consistent cut*: no received
   message unsent.
5. **(Concurrency detection as a service.)** Add a `compare <i> <j>` command that, given two
   logged stamps, prints `→`, `←`, or `∥`. Verify against the `concurrent_no_total_order` demo
   that `ping` and `pong` are certified `∥`.
6. **(Proof.)** Write out the history-inventory lemma's induction in full (base: initial
   vectors; steps: local event, send, receive/max-merge), and derive both directions of the
   characterization theorem from it.

## Historical and practical notes

- **The most influential paper in distributed computing.** *Time, Clocks, and the Ordering of
  Events* (CACM 1978) is routinely cited as the field's founding paper — CCGR's own chapter
  notes call causality/logical time "probably the most influential work in the area." Lamport
  has remarked that the paper's deep content — that distributed systems are about causality,
  and state-machine replication falls out of total order — was widely absorbed via its *clock
  algorithm*, which he considered the lesser contribution.
- **Vector clocks were discovered at least twice, a decade later.** Fidge (1988) and Mattern
  (1989) independently formalized the vector construction; the underlying idea of version
  vectors for detecting conflicting file replicas is older still (Parker et al., *Detection of
  Mutual Inconsistency in Distributed Systems*, IEEE TSE 1983 — the LOCUS system). The gap
  between 1978 and 1988 is itself instructive: it took ten years to notice that the *codomain*
  of the clock should be a partial order.
- **The `n` is essential.** Charron-Bost (IPL 1991) proved vector dimension `n` necessary to
  characterize causality among `n` processes — the `O(n)` header is a lower bound, not an
  engineering failure. Practical systems economize by scoping: Dynamo keeps a version vector
  *per key* (over the replicas of that key, not the whole cluster), and "dotted version
  vectors" (Preguiça et al. 2010) refine per-key precision under many clients.
- **Causal broadcast shipped — and started a famous fight.** Birman's ISIS toolkit (the
  CBCAST/ABCAST primitives; Birman & Joseph, SOSP 1987) built group communication on causal
  and total order and was used, among others, in the New York Stock Exchange's systems. It
  also provoked one of systems research's classic disputes: Cheriton & Skeen's *Understanding
  the Limitations of Causally and Totally Ordered Communication* (SOSP 1993) argued the
  primitives were the wrong abstraction layer — prompting Birman's published rebuttal. The
  argument (end-to-end semantics vs. communication-layer guarantees) is the group-communication
  instance of the end-to-end argument from Module 02, and thirty years later both camps can
  claim vindication: causal order thrives *inside* systems (replicated data stores, CRDT
  transports) rather than as a general application API.
- **Where each clock lives in production.** Lamport-style totally ordered scalars: LWW
  timestamps (often wall-clock in practice, as in Cassandra's cell timestamps — no max+1
  receive rule, so physical LWW rather than a true Lamport clock), Raft/Paxos terms and
  ballots (a term *is* a Lamport clock over leadership events — Module 07). Vector-style:
  Dynamo/Riak sibling detection, OT/CRDT collaboration backends, and the causal-consistency
  metadata of
  systems like COPS and MongoDB's causal sessions (implemented, notably, with *hybrid logical
  clocks* — Kulkarni et al. 2014 — which bound a Lamport clock to physical time to get both
  monotonicity and meaningful wall-clock readings).

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: reliable broadcast (Module 3.2,
  Algs. 3.2–3.3); FIFO and causal broadcast (§3.9, Module 3.9; no-waiting Alg. 3.13,
  garbage-collected Alg. 3.14, waiting/vector Alg. 3.15); the fail-arbitrary boundary
  (§3.10.1). ISBN 978-3-642-15259-7.

**Logical time**
- L. Lamport, *Time, Clocks, and the Ordering of Events in a Distributed System*, CACM 21(7),
  1978.
- C. J. Fidge, *Timestamps in Message-Passing Systems That Preserve the Partial Ordering*,
  Proc. 11th Australian Computer Science Conference, 1988.
- F. Mattern, *Virtual Time and Global States of Distributed Systems*, Proc. Workshop on
  Parallel and Distributed Algorithms, 1989.
- B. Charron-Bost, *Concerning the Size of Logical Clocks in Distributed Systems*, Information
  Processing Letters 39(1), 1991.
- D. S. Parker et al., *Detection of Mutual Inconsistency in Distributed Systems*, IEEE
  Transactions on Software Engineering SE-9(3), 1983. (Version vectors.)
- S. Kulkarni, M. Demirbas, D. Madappa, B. Avva, M. Leone, *Logical Physical Clocks*
  (Hybrid Logical Clocks), OPODIS 2014. (The longer title *…and Consistent Snapshots in
  Globally Distributed Databases* is the UB tech-report version, CSE 2014-04.)

**Broadcast and group communication**
- V. Hadzilacos, S. Toueg, *A Modular Approach to Fault-Tolerant Broadcasts and Related
  Problems*, Cornell TR 94-1425, 1994.
- K. Birman, T. Joseph, *Exploiting Virtual Synchrony in Distributed Systems*, SOSP 1987.
  (ISIS; CBCAST/ABCAST.)
- D. Cheriton, D. Skeen, *Understanding the Limitations of Causally and Totally Ordered
  Communication*, SOSP 1993.
- K. M. Chandy, L. Lamport, *Distributed Snapshots: Determining Global States of Distributed
  Systems*, ACM TOCS 3(1), 1985. (Exercise 4.)

---

## Running the code

```bash
cargo build
```

Start a 4-node cluster (any node may broadcast from its stdin):
```bash
cargo run -- 6000 127.0.0.1:6001 127.0.0.1:6002 127.0.0.1:6003
cargo run -- 6001 127.0.0.1:6000 127.0.0.1:6002 127.0.0.1:6003
cargo run -- 6002 127.0.0.1:6000 127.0.0.1:6001 127.0.0.1:6003
cargo run -- 6003 127.0.0.1:6000 127.0.0.1:6001 127.0.0.1:6002 --slow-from 127.0.0.1:6000 800
```
Type `bcast <m>` in any terminal. The optional `--slow-from <addr> <ms>` delays a node's
processing of messages originating at `addr` — the knob that makes causal waiting observable.
The `demos/` scripts reproduce §7's experiments (`crash_relay.py`, `causal_order.py`,
`concurrent_no_total_order.py`).

---
*[Course home](../) · Previous: [Module 05](../05-leader-election/) · Next:
[Module 07 — Consensus: Raft](../07-raft/) · The Byzantine rung of the broadcast ladder:
[Module 10](../10-byzantine-broadcast/)*
