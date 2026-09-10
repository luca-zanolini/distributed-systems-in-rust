# Module 11 — Byzantine Consensus

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 07 (Raft)](../07-raft/), [Module 10 (Byzantine Reliable Broadcast)](../10-byzantine-broadcast/),
and the consensus-theory notes in [CONSENSUS.md](../07-raft/CONSENSUS.md).*

*Status: Milestones 1–4 complete — the **unauthenticated** protocol, including a live
demonstration of exactly why it must not be trusted. Milestone 5 (signed certificates,
"real PBFT") is planned and motivated by this module's closing demo.*

**Abstract.** We implement single-shot Byzantine consensus in the style of PBFT
(Castro & Liskov, 1999) for `n = 3f + 1` processes: a three-phase normal case
(pre-prepare / prepare / commit) under a leader, and a view change that rotates away
from a faulty one. We develop the protocol by attacking it: a correct leader decides in
one round trip (M1); an equivocating leader cannot split the decision but wedges the
cluster forever (M2); the view change restores liveness (M3). We prove the two safety
arguments — intra-view agreement by quorum intersection, and cross-view *lock-in* by the
commit phase — and then break the implementation on purpose: because our view-change
messages carry bare, unsigned *claims* of prepared values, a single forged message makes
all four nodes unanimously decide a value that no process ever proposed. The failure is
precise and instructive: first-hand votes need only authenticated channels, but the
view change forwards *hearsay*, and hearsay requires transferable authentication —
digital signatures. That diagnosis is the specification for Milestone 5.

## Learning objectives

After this module you should be able to:

- state weak and strong Byzantine consensus (CCGR Modules 5.10 / 5.11) and explain why
  the two validity notions are incomparable;
- explain what consensus adds over Byzantine reliable broadcast: an unconditional
  termination obligation, and therefore leader replacement;
- derive the `2·count > n + f` quorum threshold and prove intra-view agreement;
- explain the two distinct jobs of the prepare and commit phases, and prove the
  **lock-in** property: a decided value survives every view change;
- design a view change: progress timers, complain / join / enter thresholds, view-scoped
  state, and the read phase's *highest-prepared-view* selection rule;
- explain why the voting phases need no signatures but the view change does
  (first-hand testimony vs forwarded evidence), and reproduce the forgery attack;
- place PBFT in its lineage: CCGR's Byzantine epoch-change / epoch consensus, the MAC
  optimization and its costs, and the modern signed-vote practice of Tendermint,
  HotStuff, and Ethereum's finality gadget.

## 1. Motivation: from broadcast to agreement

Module 10 ended with Bracha's double-echo broadcast: a designated sender's value is
delivered consistently even if the sender equivocates. But broadcast makes a promise
only *about the sender's value* — if the sender is faulty, "nobody delivers anything"
is a perfectly correct outcome. There is no view change in Bracha's protocol because
there is nothing to rotate: the sender is not an office, it is a parameter of the
abstraction.

Consensus changes one word and thereby the whole architecture: **Termination** —
*every correct process eventually decides* — is unconditional. If the process currently
coordinating proposals is faulty, the protocol must *replace it and keep going*. That
single obligation forces into existence everything in this module that Module 10 did
not have: views (numbered leadership terms), progress timers, the view-change protocol,
and the machinery for a new leader to safely take over mid-flight.

The structural continuity is just as instructive as the difference. Our three phases
map onto Bracha's:

| this module | Module 10 (Bracha) | job |
|---|---|---|
| PRE-PREPARE | SEND | a distinguished process disseminates a value |
| PREPARE (all-to-all) | ECHO | witnesses testify to what they were told |
| COMMIT (all-to-all) | READY | "I know that a quorum knows" |

What is genuinely new is *when the phases' guarantees must survive*: Bracha's run one
instance and stop; ours must hand their evidence across a change of leadership.

## 2. Specification

We target **weak Byzantine consensus** (CCGR Module 5.10), for `N > 3f`:

- **WBC1 (Termination):** every correct process eventually decides some value.
- **WBC2 (Weak validity):** if all processes are correct and propose the same value
  `v`, no correct process decides a value other than `v`; if all processes are correct
  and some process decides `v`, then `v` was proposed by some process.
- **WBC3 (Integrity):** no correct process decides twice.
- **WBC4 (Agreement):** no two correct processes decide differently.

The **strong** variant (Module 5.11) replaces WBC2 by **BC2 (Strong validity)**: if all
correct processes propose the same `v`, no correct process decides other than `v`;
otherwise a correct process may only decide a value proposed by some *correct* process
or the special symbol `□`. Note the two notions are **incomparable**: strong validity
permits deciding `□` where weak validity (in the all-correct case) does not, and weak
validity says nothing at all once a single process is faulty (CCGR, pp. 245–246). Our
forgery demo (§8) makes the cluster decide a value proposed by *no* process — violating
strong validity on screen, and, in runs with a prior decision, agreement itself.

## 3. System model

- **Processes:** `n = 4`, tolerating `f = 1` fail-arbitrary (Byzantine) process —
  the optimal `N > 3f` resilience for this model (CCGR §5.6.1).
- **Timing:** safety must hold under full asynchrony; liveness assumes partial
  synchrony (timeouts eventually suffice — the progress timer is our GST proxy).
- **Channels:** TCP point-to-point. **Honesty note:** the `from` field of every message
  is a self-declared string, so our "authenticated links" are a convention that any raw
  socket can violate — the forgery demo does exactly this. Real deployments close this
  with per-link authentication (TLS/mTLS, MACs); we accept the gap and document it,
  because the deeper gap (§8) exists *even with* perfectly authenticated links.
- **Cryptography:** none, deliberately, until Milestone 5.
- **Single shot:** the cluster decides one value, once. Sequence numbers, checkpoints,
  and log truncation — PBFT's machinery for deciding a *stream* of values — are out of
  scope (§9).

## 4. The normal case (M1)

The leader of view `v` is `leader(v) = nodes[v mod n]` over the canonically sorted node
list — a function every node evaluates identically, which is what makes "the leader of
view v" an objective fact rather than a node-relative opinion.

```text
client        leader(0)         replica         replica         replica
  |               |                |               |               |
  |-- propose --> |                |               |               |
  |               |--- PRE-PREPARE(v=0, m) ------> (all)           |
  |               |                |               |               |
  |               <=========== PREPARE(v=0, m)  all-to-all ==========>
  |               |    each node, on 2·count > n+f distinct PREPAREs
  |               |    for (v, m): prepared := (v, m); send COMMIT
  |               <=========== COMMIT(v=0, m)   all-to-all ==========>
  |               |    each node, on 2·count > n+f: DECIDE m
```

Mechanics carried over from Module 10, now load-bearing for consensus:

- **First-testimony-wins tallies.** Each node records at most one PREPARE (and one
  COMMIT) per sender — `HashMap<from, value>` with `entry(from).or_insert(m)` — so a
  Byzantine node cannot vote twice, and cannot *revise* its vote with a second message.
- **Uniform self-send.** A node's own votes loop back through TCP and are counted by
  the same listener arms as everyone else's: all state transitions live in one place.
- **The threshold.** `2·count > n + f` (with `n = 3f + 1`: at least `2f + 1`) is the
  Byzantine quorum: two such sets intersect in more than `f` processes — hence in at
  least one *correct* process (Malkhi & Reiter's masking-quorum argument, already used
  for Bracha's echo quorum in Module 10).
- **Acceptance guard.** A node accepts one PRE-PREPARE per view, only from `leader(v)`,
  only for its current view.

**The two phases have different jobs.** A **prepare certificate** — a quorum of
PREPAREs for `(v, m)` — already gives agreement *within* view `v` (§7.1); if views never
changed, deciding at prepare-quorum would be safe and COMMIT would be redundant. The
commit phase exists for one reason: a decision must survive *future* view changes. A
COMMIT says "I hold a prepare certificate for `(v, m)`"; deciding only on a *quorum* of
COMMITs guarantees that certificate-holders are numerous enough that every future
view-change quorum must hear from one (§7.2). This is Raft's commit rule (Figure 8, cf.
[Module 07](../07-raft/)) in Byzantine clothing — and the third instance in this course
of the pattern *"the second phase is insurance against a change of coordinator."*

## 5. The Byzantine leader (M2)

A Byzantine leader has exactly two levers, and our scripted attacker
(`bcast equiv a b`) pulls both:

- **Equivocation:** send `a` to some replicas, `b` to others.
- **Vote-withholding:** contribute nothing to either tally, freezing the split. (Our
  first implementation accidentally fed the attacker's own loopback vote back into the
  race, and the attack's outcome depended on which PRE-PREPARE won a thread race inside
  the attacker — one run in six *decided*. A real adversary withholds; the demo now
  wedges deterministically. Even attackers need code review.)

Outcome, verified by `demos/equivocation_wedge.py`: prepare tallies stick at
`a: 1, b: 2` against a quorum of 3. **No certificate forms, nobody decides, forever.**
Contrast both halves with Module 10:

- The *same attack* that split the naive broadcast (attack/retreat/retreat) cannot
  split the decision here: two certificates for different values would need a common
  correct testifier, and first-testimony-wins forbids it. **Agreement is unconditional.**
- But Bracha could shrug at a faulty sender; consensus cannot. **Termination is taken
  hostage** — and a silent leader (no code needed: just never propose) takes it hostage
  just as effectively. Both leader pathologies present identically to the cure: no
  progress.

## 6. The view change (M3)

### 6.1 Trigger: a progress detector, not a failure detector

No node asks "is the leader alive?" — Module 05's per-node failure detection is the
wrong instrument, since an equivocating leader is very much alive. Each node runs a
**per-view progress timer**: *"have I decided yet?"* If a view outlives the timeout
(4 s here) without a decision, the node complains. This single predicate covers
equivocation, silence, and slowness without distinguishing them — it does not need to.

### 6.2 Complain, join, enter (CCGR Algorithm 5.15's shape)

- **Complain:** timer fires → broadcast `VIEWCHANGE(view+1, prepared)`, where
  `prepared : Option<(view, value)>` is the highest prepare certificate this node ever
  assembled (`-` on the wire for none). One complaint per view (`sent_viewchange`).
- **Join:** on receiving **> f** distinct VIEWCHANGEs for a view ahead of mine, send
  mine too, even if my own timer has not fired. More than `f` complainers must include
  a correct one, so the complaint is real; and f liars alone can never depose a working
  leader. This is Bracha's amplification pattern, third appearance in the course
  (ECHO-quorum → READY; > f READYs → READY; now > f VIEWCHANGEs → VIEWCHANGE).
- **Enter:** on **> 2f** distinct VIEWCHANGEs for view `v′ > view`: switch to `v′`,
  clear all *view-scoped* state (the prepare/commit tallies, the per-view one-shot
  flags), restart the timer. What survives: `decided`, and — crucially — `prepared`.

### 6.3 The read phase: what may the new leader propose?

On entering a view it leads, the new leader consults the collected VIEWCHANGE claims —
its unauthenticated miniature of PBFT's new-view certificate and of CCGR's conditional
collect output (Algorithms 5.16–5.18: our `collected`/WRITE/ACCEPT are the book's read
phase, write phase, and accept — see §9) — and applies the selection rule:

> among the claims, re-propose the value of the **highest prepared view**;
> only if no claim is `Some` may the leader propose fresh.

The re-proposal is an ordinary PRE-PREPARE for the new view — replicas need no special
handling; the new view simply replays the standard protocol. `demos/view_change_unwedge.py`
shows the full arc: wedge → `ENTERED VIEW 1` → *"no prepared value — awaiting fresh
proposal"* (correct: the wedge formed no certificate) → `propose c` → all four decide,
Byzantine ex-leader included. M2's permanent wedge becomes a five-second detour.

## 7. Correctness

### 7.1 Intra-view agreement

Two prepare certificates in the same view `v` for values `m ≠ m′` each contain more
than `(n+f)/2` distinct testimonies; the sets intersect in more than `f` processes, so
in at least one correct process — which testified for both `m` and `m′`. But a correct
process sends one PREPARE per view (one PRE-PREPARE accepted, and first-testimony-wins
discards revisions at every tally). Contradiction. The same argument covers commit
certificates. Hence within a view, all deciders decide the same value. ∎

### 7.2 Lock-in: decided values survive view changes

**Claim.** If some correct process decides `m` in view `v`, then every later view's
leader (correct or not — see §8 for what "not" costs) *selects* `m` under the read-phase
rule, provided claims are truthful.

**Proof sketch** (the counting is CCGR's, pp. 257–258). Deciding requires > 2f COMMITs;
each COMMIT sender holds a prepare certificate for `(v, m)`; so at least `2f + 1`
processes — among them at least `f + 1` correct — hold that certificate and will claim
`(v, m)` (or a later certificate; see below) in every subsequent VIEWCHANGE. Any enter
quorum has size ≥ `2f + 1`; two sets of ≥ `2f + 1` among `3f + 1` processes intersect in
≥ `f + 1`, hence in a correct process. So **every possible view-change quorum contains
at least one truthful claim of `(v, m)`** — the decided value is structurally impossible
to miss, and being of maximal prepared view among truthful claims (no correct process
prepares a different value in view `v` by §7.1, nor in later views by induction on this
very argument), the selection rule picks it. ∎

Two companion observations sharpen the theorem:

- **A lone certificate proves non-decision.** If only one process assembled a prepare
  certificate (the last PREPAREs reached it alone before the network failed), then only
  it ever sent COMMIT — one COMMIT against a quorum of `2f + 1` — so *nobody decided*,
  and a view-change quorum that misses this process abandons its value **safely**. The
  certificate was a candidacy, not an outcome. (Raft's twin: an entry on a minority log
  may be overwritten; a committed entry sits on a majority that every election quorum
  must intersect.)
- **The rule over-approximates, harmlessly.** If `2f + 1` COMMITs were *sent* but all
  delayed, nobody has decided — yet the claims still force the value forward. The
  leader cannot distinguish "decided somewhere" from "decidable in flight," and
  re-proposing an already-validly-proposed value costs nothing. Conservatism is free;
  only *missing* a decided value is fatal.

Note what the proofs use: counting, intersection, one-testimony-per-process. **No
timing.** Safety holds under arbitrary asynchrony; the timeout constant affects only
how long liveness takes, never whether agreement can break.

### 7.3 Liveness

After GST, timeouts stop firing spuriously; round-robin rotation reaches a correct
leader within at most `f + 1` view changes; the read phase gives that leader a
proposable value (inherited or fresh); the normal case then decides in three message
delays. Rotation-until-progress *is* the liveness engine — our demo's `ENTERED VIEW 1`,
`ENTERED VIEW 2` drumbeat while no proposal existed was not a bug but the protocol
patiently searching for a leader worth having.

## 8. The forgery: why unsigned view-changes are broken

Everything in §7.2 carried a quiet hypothesis: *"provided claims are truthful."*
`demos/forged_certificate.py` cashes in that hypothesis. During the wedge, a raw Python
socket — not a node at all — sends every node one message:

```text
VIEWCHANGE 127.0.0.1:7002 1 0 evil
```

an impersonation of replica 7002 claiming a prepare certificate for `(0, "evil")` that
never existed, for a value never proposed by anyone. When view 1 is entered, the new
leader's selection rule — operating *exactly as specified* — finds `(0, evil)` as the
maximal prepared claim, dutifully re-proposes it, and the quorum machinery — operating
*exactly as specified* — amplifies the lie into a unanimous decision. Even the
impersonated 7002 decides `evil`: its genuine VIEWCHANGE arrived second and lost
first-testimony-wins to the forgery. One 35-byte message; strong validity violated on
screen. In a run where view 0 had partially decided some value, the same injection with
a higher claimed view would override it — breaking **agreement**, the property the whole
edifice exists to protect. The attack does not evade the lock-in argument; it
*weaponizes* it.

The diagnosis is worth stating as a principle, because it explains the entire
cryptographic architecture of practical BFT:

> **First-hand testimony needs only authenticated channels. Forwarded evidence —
> hearsay — needs transferable authentication: signatures.**

The voting phases are first-hand: I count the PREPARE that *you* sent *me*, over a
channel that (in a real deployment) authenticates you. Bracha's entire protocol lives
in this regime, which is why it is signature-free at `O(n²)` messages. The view change
is different in kind: a VIEWCHANGE forwards to the new leader a summary of what *others*
told the sender. Channel authentication says who is speaking; it cannot make the
speaker's account of third parties checkable. PBFT therefore ships, inside each
view-change message, the certificate itself — the `2f + 1` *signed* PREPAREs — and the
new leader **verifies instead of believes**; its NEW-VIEW message then carries the
signed view-changes it selected from, so every replica re-verifies the selection too.
Forging a certificate means producing `2f + 1` valid signatures while controlling `f`
signers. That is Milestone 5, and this demo is its requirements document.

**Historical note: the MAC detour.** PBFT's celebrated throughput came from replacing
signatures with vectors of pairwise MACs (Castro & Liskov's TOCS 2002 version and
Castro's thesis) — but MACs are precisely *non-transferable*, so the view change had to
be redesigned around the hearsay problem, and grew substantially more complex. That
complexity proved expensive downstream: Zyzzyva (SOSP 2007) carried a view-change
safety bug found a decade later (Abraham et al., 2017), and Aardvark (NSDI 2009) showed
even *clients* could exploit MAC tricks to wedge MAC-based protocols. Modern practice
returned to signing every vote — Ed25519 and BLS made signatures cheap, and the
blockchain generation *needs* transferability anyway: a light client verifying a commit
certificate, or a slashing proof of equivocation (Ethereum, Tendermint), is exactly a
third party checking forwarded evidence. Tendermint's prevote/precommit, HotStuff's
quorum certificates, and Casper FFG's BLS-aggregated attestations are all
signed-vote-certificate machines. Twenty years of systems, one lesson: the cheap fast
path was paid for at the protocol's hardest corner, and everyone eventually moved the
cost back.

## 9. Correspondence: this module ↔ CCGR §5.6 ↔ PBFT

CCGR presents Byzantine consensus as a stack — Leader-Driven Consensus (Alg. 5.19) over
Byzantine Epoch-Change (5.15) over Byzantine Epoch Consensus (5.17–5.18) over Signed
Conditional Collect (5.16) — engineered so one correctness argument covers every epoch
uniformly, at the price of running the read phase (conditional collect) in *every*
epoch: five communication steps even under a stable correct leader. PBFT is what you
get by taking seriously the book's own concession (p. 260) that the first epoch's read
phase may be skipped: make the stable-leader path the cheap common case (three steps,
no read phase) and pay the heavy machinery only at the rare view change. Same protocol,
opposite amortization — and the asymmetry is *why* PBFT was "practical."

| this module | CCGR §5.6 | PBFT (OSDI '99) |
|---|---|---|
| view `v` | epoch timestamp `ets` | view `v` |
| `leader_of(v)`, round-robin | `leader(ets)` (§2.6.5) | primary `= v mod R` |
| PRE-PREPARE | leader's value after conditional collect | PRE-PREPARE |
| PREPARE, quorum `2·count > n+f` | WRITE, quorum `> (N+f)/2` | PREPARE, `2f` + self |
| `prepared: Option<(v, m)>` | `(valts, val)` + writeset | prepared certificate |
| COMMIT, quorum | ACCEPT, quorum | COMMIT, `2f + 1` |
| progress timer → VIEWCHANGE | complaint → NEWEPOCH | timer → VIEW-CHANGE |
| join on `> f` | NEWEPOCH amplification `> f` | — (implicit) |
| enter on `> 2f` | start epoch on `> 2f` | NEW-VIEW from `2f + 1` VCs |
| read-phase max-prepared-view selection | `binds` / `quorumhighest` / `certifiedvalue` over collected `S` | new primary's pre-prepare selection from `V` |
| *(absent — the forgery gap)* | signatures in conditional collect | signed VCs + NEW-VIEW re-verification |

**Deliberately omitted from PBFT** (each a signpost, none accidental): sequence numbers
and pipelining (we decide once; PBFT decides a log), checkpoints/garbage collection and
watermarks (meaningless without a log), the MAC-based fast path (§8's historical note),
and state-transfer for laggards.

## 10. Correspondence between theory and code

| mechanism | code |
|---|---|
| message alphabet + wire format | `enum Message`, `encode`/`decode` (`-` sentinel for `None`) |
| canonical node order / `leader(v)` | `nodes.sort_by_key(port_of)`; `leader_of` |
| one PRE-PREPARE per view, leader-only | PrePrepare arm: `from == leader_of(...) && v == state.view && !preprepared` |
| first-testimony-wins tallies | `prepares` / `commits` / `viewchanges` maps, `entry(from).or_insert` |
| Byzantine quorum | `2 * count > n + f` in the Prepare and Commit arms |
| prepare certificate | `state.prepared = Some((v, m))` at prepare-quorum |
| progress timer | timer thread: 500 ms poll, 4 s timeout, `!decided && !sent_viewchange` |
| complain / join / enter | ViewChange arm: broadcast on timer; `count > f` join; `count > 2f && nv > view` enter |
| view-scoped reset vs survivors | `prepares/commits/preprepared/sentcommit` cleared; `decided`, `prepared` kept |
| read phase / selection rule | enter branch: max-prepared-view scan → ordinary PRE-PREPARE |
| view-aware proposing | stdin thread: snapshot `(view, am-I-leader)` under a short lock, send after release |
| scripted Byzantine leader | `bcast equiv a b`: `send_to` halves, no self-send (vote-withholding) |

## 11. Limitations

Honest list; each is a signpost, and the first two are the point of the module:

1. **Claims, not certificates.** VIEWCHANGE carries an unverifiable `(view, value)`
   assertion; §8 demonstrates the consequence. Fix: Milestone 5 (signed prepares
   carried in the view change; NEW-VIEW re-verification).
2. **Self-declared identity.** Any socket can claim any `from`. Fix: authenticated
   channels (mTLS/MACs) — orthogonal to, and insufficient without, item 1.
3. **View-entry race.** Nodes enter a new view at slightly different moments; an eager
   new leader's PRE-PREPARE can reach a node still in the old view and be dropped
   (guard `v == state.view`). With loopback latencies this is invisible; under real
   asynchrony the protocol self-heals only by rotating again. Real PBFT synchronizes
   entry via the NEW-VIEW message.
4. **Fixed timeout.** 4 s forever; a network slower than that livelocks through views.
   PBFT doubles the timeout per view change (exponential backoff) so that after GST
   some view eventually outlives the network's actual delay.
5. **Single-shot.** One decision; no sequence numbers, checkpoints, or log — see §9.
6. **The Byzantine repertoire is scripted.** Our attacker equivocates in view 0 or
   stays silent; it does not lie in later views, vote maliciously, or collude. The
   demos are experiments confirming specific predictions, not an adversarial search.
7. **Unbounded view numbers, unbounded `viewchanges` map** — fine for a toy's lifetime.

## 12. Exercises

1. **Intra-view agreement.** Write out §7.1 as a full proof, including why
   `entry(from).or_insert` (rather than `insert`) is load-bearing: exhibit the
   vote-revision attack that plain `insert` would permit.
2. **The lock-in theorem.** Prove §7.2's claim by induction over views, and identify
   precisely where the hypothesis "claims are truthful" is used. Then show a run where
   `2f + 1` COMMITs are sent, none delivered, and the value is nonetheless preserved —
   why is this conservatism necessary for a *deterministic* rule?
3. **Thresholds.** Why must the join rule require `> f` rather than `≥ 1`? Exhibit the
   attack for `≥ 1`, and explain what breaks (safety? liveness? neither?) if the enter
   threshold is lowered to `> f`.
4. **Commit-phase deletion.** With a fixed, permanently correct leader and no view
   change, prove that deciding at prepare-quorum is safe. Then re-enable view changes
   and construct the execution in which prepare-quorum decision violates agreement.
   (This was the module's opening predict-question; make it rigorous.)
5. **Break agreement, not just validity.** The demo forgery violates strong validity
   in a run where nothing was decided. Construct the schedule in which the same
   injection breaks *agreement* — some correct process decides `m`, then others decide
   `evil`. Which additional timing control over message delivery do you need?
6. **The view-entry race.** Construct the interleaving of limitation 3 explicitly
   (who is in which view when the re-proposal arrives). Propose two fixes — buffering
   one future-view PRE-PREPARE vs a NEW-VIEW synchronization message — and compare
   what each assumes and costs.
7. **Adaptive timeouts.** Add exponential backoff (double the timeout each entered
   view; reset on decision). State the liveness property gained: after GST, why must
   some view eventually complete? Which of §7's proofs, if any, must change? (Answer:
   none — say why.)
8. **MAC-based view changes.** Read §4.5–4.6 of Castro's thesis (or TOCS 2002 §5).
   Explain in one page why replacing signatures with MACs forces the view-change
   protocol to change shape, and summarize the mechanism the thesis adopts instead.

## Historical and practical notes

- **"Practical."** Before PBFT (OSDI 1999), Byzantine agreement was widely regarded as
  a theoretical curiosity — synchronous protocols with unrealistic assumptions or
  asynchronous ones with prohibitive costs. Castro & Liskov's title was a thesis
  statement: three message delays, MACs instead of signatures, and a replicated NFS
  within a few percent of the unreplicated one. The `n = 3f + 1` bound it inhabits is
  Pease–Shostak–Lamport (1980); its partial-synchrony liveness stance is DLS (1988);
  FLP (1985) is why *some* extra assumption is non-negotiable.
- **The optimization that bit back.** The MAC fast path's complex view change became a
  recurring defect site across the descendants (Zyzzyva's bug, found ten years post
  facto; Aardvark's client-side attacks) — a standing lesson in where to spend one's
  complexity budget: the rare path that guards safety is the one that must stay simple.
- **The blockchain renaissance.** Interest in BFT consensus, dormant through the
  2000s outside a niche, returned at scale with proof-of-stake blockchains: Tendermint
  (2018) is a PBFT-shaped protocol with rotating proposers and signed votes;
  HotStuff (PODC 2019) linearizes PBFT's `O(n²)` view change to `O(n)` with threshold
  signatures and pipelining, and underlies Diem-lineage systems; Ethereum's Casper FFG
  finality gadget aggregates BLS-signed attestations — quorum certificates at the scale
  of hundreds of thousands of validators. The author of this course works on that
  protocol family; this module is its `n = 4` skeleton.
- **Accountability as a feature.** Signatures do more than block forgery: they make
  misbehavior *provable to third parties* (slashing evidence, light-client proofs) —
  transferable authentication turned from a defensive necessity into the economic
  mechanism securing open-membership systems.

## References

**Primary.**

- M. Castro, B. Liskov. *Practical Byzantine Fault Tolerance.* OSDI 1999. — The
  protocol this module implements, single-shot and unsigned.
- M. Castro, B. Liskov. *Practical Byzantine Fault Tolerance and Proactive Recovery.*
  ACM TOCS 20(4), 2002. — The full system: MAC optimization, view-change details,
  checkpoints, recovery.
- C. Cachin, R. Guerraoui, L. Rodrigues. *Introduction to Reliable and Secure
  Distributed Programming*, 2nd ed., Springer 2011 — §5.6 (Modules 5.10–5.14,
  Algorithms 5.15–5.19): the specification and the modular treatment; pp. 257–258 for
  the lock-in proof this module's §7.2 follows.

**Foundations.**

- M. Pease, R. Shostak, L. Lamport. *Reaching Agreement in the Presence of Faults.*
  JACM 27(2), 1980. — `n ≥ 3f + 1` is necessary.
- L. Lamport, R. Shostak, M. Pease. *The Byzantine Generals Problem.* ACM TOPLAS 4(3),
  1982. — The fault model's name and framing.
- M. Fischer, N. Lynch, M. Paterson. *Impossibility of Distributed Consensus with One
  Faulty Process.* JACM 32(2), 1985. — Why liveness needs partial synchrony.
- C. Dwork, N. Lynch, L. Stockmeyer. *Consensus in the Presence of Partial Synchrony.*
  JACM 35(2), 1988. — The timing model our progress timers inhabit; `3f + 1` for
  partial-synchrony BFT.
- G. Bracha. *Asynchronous Byzantine Agreement Protocols.* Information and
  Computation 75(2), 1987. — The signature-free, first-hand-testimony regime
  (Module 10) this module builds on.
- D. Malkhi, M. Reiter. *Byzantine Quorum Systems.* Distributed Computing 11(4),
  1998. — The quorum-intersection arguments in §7.

**Descendants and the signature question.**

- R. Kotla, L. Alvisi, M. Dahlin, A. Clement, E. Wong. *Zyzzyva: Speculative Byzantine
  Fault Tolerance.* SOSP 2007. — The speculative fast path; carried a view-change
  safety bug.
- I. Abraham, G. Gueta, D. Malkhi, L. Alvisi, R. Kotla, J.-P. Martin. *Revisiting Fast
  Practical Byzantine Fault Tolerance.* arXiv:1712.01367, 2017. — Documents the
  Zyzzyva bug, a decade later.
- A. Clement, E. Wong, L. Alvisi, M. Dahlin, M. Marchetti. *Making Byzantine Fault
  Tolerant Systems Tolerate Byzantine Faults.* NSDI 2009 (Aardvark). — Robustness
  against, among others, MAC-exploiting clients.
- E. Buchman, J. Kwon, Z. Milosevic. *The Latest Gossip on BFT Consensus.*
  arXiv:1807.04938, 2018. — Tendermint: PBFT-shaped, signed votes, rotating proposers.
- M. Yin, D. Malkhi, M. Reiter, G. Gueta, I. Abraham. *HotStuff: BFT Consensus with
  Linearity and Responsiveness.* PODC 2019. — Linear, pipelined re-engineering of the
  view change via threshold-signed quorum certificates.
- V. Buterin, V. Griffith. *Casper the Friendly Finality Gadget.* arXiv:1710.09437,
  2017. — Signed-attestation finality over a blockchain; this protocol family at
  production scale.

## Running the code

```bash
cargo build

# four terminals (n = 4, f = 1; leader of view 0 is the lowest port):
cargo run -- 7000 127.0.0.1:7001 127.0.0.1:7002 127.0.0.1:7003
cargo run -- 7001 127.0.0.1:7000 127.0.0.1:7002 127.0.0.1:7003
cargo run -- 7002 127.0.0.1:7000 127.0.0.1:7001 127.0.0.1:7003
cargo run -- 7003 127.0.0.1:7000 127.0.0.1:7001 127.0.0.1:7002

# on the current view's leader:
propose lunch            # normal case: everyone decides
bcast equiv a b          # Byzantine attack: equivocate + withhold (view-0 leader only)
```

Scripted, reproducible experiments (each prints a verdict):

```bash
cd demos
python3 happy_path.py             # M1: 4/4 decide, no view change
python3 equivocation_wedge.py     # M2: no split, no decision — safety without liveness
python3 view_change_unwedge.py    # M3: rotation + read phase restore liveness
python3 forged_certificate.py     # the reason Milestone 5 exists
```

---
*[Course home](../) · Previous: [Module 10](../10-byzantine-broadcast/) · Next:
[Module 12 (planned)](../12-crdts-eventual-consistency/)*
