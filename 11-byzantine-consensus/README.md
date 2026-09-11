# Module 11 — Byzantine Consensus

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 07 (Raft)](../07-raft/), [Module 10 (Byzantine Reliable Broadcast)](../10-byzantine-broadcast/),
and the consensus-theory notes in [CONSENSUS.md](../07-raft/CONSENSUS.md).*

*Status: **complete** — normal case, view change, ed25519-signed certificates and
envelopes, NEW-VIEW re-verification, and stable storage; seven reproducible demos
including three attacks that bounce. The module was built by iterated attack: each
security layer exists because a demo first showed its absence being exploited.*

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
digital signatures. That diagnosis became the specification for the module's second
half: ed25519-signed prepares assembled into verifiable certificates; envelope
authentication on every message; a NEW-VIEW protocol in which the new leader *forwards*
its signed evidence and every replica re-derives the selection instead of trusting it —
CCGR's signed conditional collect, rediscovered from the attack side; and stable
storage under persist-before-externalize discipline, because in a signed protocol
amnesia is not vote-loss but *self-equivocation*. The finale puts a real decision at
stake and lets a Byzantine new leader lie about the evidence it itself forwards: every
node — the liar included — re-derives the truth and refuses, the escalating timer
rotates past the spent view, and the next honest leader is forced by quorum-intersected
certificates to propose the decided value. The lock-in theorem, executing against a
live adversary.

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
- state the rule deciding which signatures become certificates (knowledge crossing a
  boundary) and recognize NEW-VIEW as CCGR's signed conditional collect;
- explain why crash-recovery without stable storage turns a *signed* correct node into
  an equivocator, and what persist-before-externalize protects;
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
- **Channels:** TCP point-to-point, with **authenticated envelopes implemented**: every
  message carries an ed25519 signature over a domain-separated canonical statement of
  its content, verified against the claimed sender's known public key at a single gate
  before any protocol logic runs (see Module 10's §6.5 for the MAC-vs-signature
  discussion; here we additionally *exploit transferability* — §6.4).
- **Cryptography:** ed25519 signatures throughout, under a **trusted setup**: a `keygen`
  step writes per-node keypairs and all public keys are known to all — the explicit
  anti-Sybil assumption without which "at most `f` Byzantine" is meaningless.
- **Crash recovery:** durable facts (`view`, the PREPARE I signed, my certificate, my
  decision) are fsynced to `pbft-<port>.state` *before* the corresponding message
  leaves the machine (persist-before-externalize, Module 07's discipline) — see §6.6
  for why this matters *more* in a signed protocol.
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
  least one *correct* process (Byzantine quorum intersection, cf. Malkhi & Reiter 1998;
  already used
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
  `prepared : Option<Cert>` is the highest prepare certificate this node ever
  assembled. One complaint per target view, deduplicated and escalated by the
  monotone counter `highest_vc_sent`.
- **Complaining is abandonment (the cutoff).** A process that has complained past its
  current view **stops participating in it**: all three vote arms require
  `highest_vc_sent <= view`, and the counter is persisted before the complaint is
  sent. This is CCGR's `halt`-on-abort (Alg. 5.18) and PBFT's rule that a view-changing
  replica stops accepting messages in the views it left behind — and it is
  load-bearing for safety, not hygiene: without it, a process could truthfully claim
  "nothing prepared" in its VIEWCHANGE and *then* help decide a value in the old view,
  opening the next view unconstrained — agreement breakable by message scheduling
  alone, zero Byzantine nodes (§7.2).
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
the raw material of PBFT's new-view certificate and of CCGR's conditional
collect output (Algorithms 5.16–5.18: our `collected`/WRITE/ACCEPT are the book's read
phase, write phase, and accept — see §9) — and applies the selection rule:

> among the claims, re-propose the value of the **highest prepared view**;
> only if no claim is `Some` may the leader propose fresh.

The re-proposal is an ordinary PRE-PREPARE for the new view — replicas need no special
handling; the new view simply replays the standard protocol. `demos/view_change_unwedge.py`
shows the full arc: wedge → `ENTERED VIEW 1 (leader …, expected None)` → *(nothing
certified — the leader may propose fresh
proposal"* (correct: the wedge formed no certificate) → `propose c` → all four decide,
Byzantine ex-leader included. M2's permanent wedge becomes a five-second detour.

### 6.4 The read phase made verifiable: NEW-VIEW as conditional collect

The selection rule of §6.3 originally ran only in the leader's head — replicas accepted
any proposal from `leader_of(view)` unchecked, and §8 shows what that permits. The
repaired protocol makes the leader **show its work**:

- The **VIEWCHANGE tally keeps each sender's envelope signature**. This is the moment a
  link-authentication signature is promoted to **transferable evidence**: the same bytes
  that authenticated a first-hand message become an affidavit the instant they are
  forwarded (Module 10 §6.5's distinction, now load-bearing).
- On collecting > 2f VIEWCHANGEs for view `v`, the leader-to-be broadcasts
  **`NewView { view, vcs, proposal, sig }`**: the ≥ 2f+1 signed VIEWCHANGE records it
  entered on, plus the proposal those records force (`Some(value)`) or `None` if
  nothing is protected. The proposal rides *inside* the NewView — PBFT bundles its
  pre-prepares into NEW-VIEW for the same reason — eliminating the ordering race
  between "enter the view" and "receive the proposal".
- **Receiving a valid NewView is the only door into a view.** Each replica: verifies
  the NewView's own envelope (the gate); *rebuilds each forwarded record as the
  ViewChange it once was and re-verifies it through the same `verify_envelope` code
  path the original traveled* — shared verification paths, so live and forwarded
  checking cannot drift; requires ≥ 2f+1 valid, distinct records for this view;
  **re-runs the identical `select_value` function** over the verified evidence; and
  rejects the NewView outright if the leader's proposal deviates from its own derived
  conclusion. Only then does it enter — clearing view-scoped state and recording the
  derived constraint as `expected`, which the PrePrepare arm enforces for the rest of
  the view. (A precision worth stating: with the proposal embedded, the PrePrepare-arm
  `expected` guard is *provably redundant* — `expected.is_some() ⟹ preprepared` holds
  at all times, so every deviation is caught earlier, at the NewView homework check or
  at `!preprepared`. It is retained deliberately: it states the view-binding invariant
  directly rather than leaning on the incidental coupling the embedded design creates,
  it becomes the *sole* enforcement point in the non-embedded variant of Exercise 6,
  and its rejection log names the crime where `!preprepared`'s silence would not.)
- **The leader gets no shortcut.** Its own NewView loops back through its own listener
  and the same arm: it re-verifies its own evidence and checks its own homework. A
  *lying* leader therefore **rejects its own NewView** — the lie exists only in the
  outbound message, never in the honest arm — and never even enters the view it
  claimed to open (`demos/byzantine_new_leader.py` shows the liar logging the
  rejection of its own message).
- A view that produces no valid entry is **spent**: the complaint counter
  (`highest_vc_sent`, a monotone high-water mark) escalates the next complaint to
  `max(view, highest_vc_sent) + 1`, rotating *past* the failed leader instead of
  re-knocking on its door forever. With `n = 3f + 1`, any `f + 1` consecutive views
  contain a correct leader; after GST, one of them lands.

This is CCGR's **Signed Conditional Collect** (Module 5.14, Algorithm 5.16), arrived at
from the attack side: signed inputs = the VIEWCHANGEs; `COLLECTED` = the NewView; the
integrity property CC2 (a faulty leader cannot attribute inputs to correct processes
that never made them) = the per-record re-verification; and computing `binds` from the
collected vector at *every* process = `select_value` shared between leader and
replicas. When we chose PBFT's shape over the book's stack, we skipped conditional
collect as happy-path overhead; the view change is where it turns out to be
load-bearing, and every attack demo since was the protocol saying so.

### 6.5 Which signatures become certificates — and which don't

The module now contains two certificate species — the prepare certificate (`Cert`:
≥ 2f+1 signed PREPAREs, a quorum about a *value*) and the new-view certificate (the
`vcs` vector: ≥ 2f+1 signed VIEWCHANGEs, a quorum about *entering a view*) — while
COMMIT and PREPREPARE signatures are verified at the gate and then discarded. The rule
deciding who gets collected:

> **A signature gets collected into a certificate if and only if somebody else, in some
> other context, will later need the proof. Certificates exist exactly where knowledge
> must cross a boundary — into the next view, to another node, to an outside observer.
> Where knowledge is consumed on the spot, you count envelopes and throw them away.**

COMMIT quorums are consumed on the spot (each node assembles its own decision from
envelopes it received first-hand), so no commit certificate exists *here* — but in
multi-shot systems one does: a laggard catching up, or a light client, is precisely
"somebody else needing the proof", and Tendermint's block commit is exactly a collected
certificate of precommits (Exercise 9). A PREPREPARE carries no quorum at all, but its
signature is not wasted: two signed PREPREPAREs from one leader for one view with
different values are *transferable proof of equivocation* — slashing evidence, the
accountability dividend of signing everything.

### 6.6 Stable storage: in a signed protocol, amnesia is self-equivocation

Module 07 taught that a consensus node restarting blank can double-vote and lose
committed entries. Signatures raise the stakes in kind, not just degree: accept a
PRE-PREPARE for `a`, *sign* PREPARE `(v, a)`, crash, restart blank — and a Byzantine
leader gladly re-proposes `b`; you sign `(v, b)`. Two valid signatures from one
"correct" key on conflicting statements: **the restarted node has manufactured the raw
material for two conflicting certificates, and the `≤ f` assumption silently collapses
— crash-recovery without stable storage turns correct nodes Byzantine.** The counting
argument breaks too: a decision guarantees ≥ 2f+1 certificate holders, but one amnesiac
leaves 2f, whose intersection with a view-change quorum can be *exactly the liar* at
`f = 1`.

The persistent subset (fsynced to `pbft-<port>.state` *before* the corresponding
message is sent — persist-before-externalize; `sync_all()` is the line that makes it
survive power loss rather than merely process death):

| persisted | why |
|---|---|
| `view` | never rewind views — a rewound node re-accepts old-view pre-prepares |
| `my_prepare` — the `(view, value)` I *signed* | the anti-self-equivocation record: "I already spoke in this view" |
| `prepared` — my highest certificate | the lock-in witness my future VIEWCHANGEs must present |
| `decided` | integrity across restarts |
| `highest_vc_sent` | the abandon rule must survive a crash — a rebooted node must not resume voting in a view it complained past |

Deliberately *not* persisted: the tally maps (soft quorum-evidence, re-accumulable —
losing them costs at worst this view's progress) and the one-shot flags (re-sent
messages are deduplicated by `or_insert` at every receiver — idempotency absorbs
repetition; only *contradiction* is fatal, and `my_prepare` prevents exactly that).
`preprepared` is *derived* at load time from `my_prepare`-vs-`view` — persist facts,
derive judgments. One shape worth stating as an invariant: across a run, `prepared` is
a **view-monotone step function whose value coordinate is free until the first decision
anywhere and constant ever after** — that is the lock-in theorem restated as a
data-structure property (Exercise 10). `demos/crash_recovery.py` kills the entire
cluster after a decision and restarts it; all four nodes come back knowing.

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
`(v, m)` (or a later certificate; see below) in every subsequent VIEWCHANGE. The word
"subsequent" is doing real work, and it is the **abandon rule** (§6.2) that pays for
it: because a correct process stops voting in a view once it has complained past it,
its COMMIT causally *precedes* any VIEWCHANGE it later sends, so the later claim
necessarily reports the certificate. Without the cutoff this step is false — a
truthful `None` claim could be followed by a decision in the abandoned view (CCGR's
proof leans on exactly this via `halt`-on-abort, printed p. 257: "no correct process
has sent a WRITE message in any epoch between ts′ and ts*"). Any enter
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

## 8. The forgery arc: three attacks, three layers of defense

The module's security was built by iterated attack — each layer exists because a demo
first exploited its absence. The finished arc, before the story of how it was found:

| attacker | attack | dies at | demo |
|---|---|---|---|
| keyless outsider | forged COMMITs → fabricated decision in the *normal case* | the envelope gate | `forged_commits.py` |
| keyed **insider** (7002) | validly-signed VIEWCHANGE carrying a certificate with garbage inner signatures | `verify_cert` | `forged_certificate.py` |
| the **new leader itself** | validly-signed NewView with genuine evidence and a lying conclusion | every replica's re-run of `select_value` — including the liar's own | `byzantine_new_leader.py` |

Each layer catches exactly what it can see and nothing more: the gate answers *who
speaks*; certificate verification answers *whether the forwarded evidence checks out*;
re-derivation answers *whether the conclusion follows from the evidence*. All three are
necessary; the demos prove each is insufficient alone.

**How the first hole was found.** Everything in §7.2 carried a quiet hypothesis:
*"provided claims are truthful."* The original, unauthenticated implementation let a
raw Python
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
signers. This module implements exactly that: the certificate *is* its 2f+1
signatures, verified — never believed — by leader and replicas alike, and the same
injection now dies with one rejection line per lie. The residual attacker after
certificates is the one no signature can stop: a leader that forwards honest evidence
and lies about its *conclusion* — which is why the NewView's re-derivation layer
(§6.4) exists, and why `byzantine_new_leader.py` is the module's true final exam.

**Historical note: the MAC detour.** PBFT's celebrated throughput already rested on
MACs in **OSDI 1999**: the common case was authenticated with vectors of pairwise MACs,
while view-change and new-view messages still carried signatures. The **TOCS 2002**
version and Castro's thesis then eliminated those remaining signatures too — and
because MACs are precisely *non-transferable*, the view change had to be redesigned
around the hearsay problem, growing substantially more complex. The descendants show
both costs: Zyzzyva (SOSP 2007) carried a view-change safety bug found a decade later
(Abraham et al., 2017 — rooted in its *speculative fast path*'s interaction with the
view change, a complexity-of-the-rare-path lesson rather than a MAC artifact), and
Aardvark (NSDI 2009) showed even *clients* could exploit MAC authenticators to wedge
MAC-based protocols outright. Modern practice
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
| join on `> f` | NEWEPOCH amplification `> f` | f+1 view-changes → join (OSDI §4.5.2) |
| enter on `> 2f` | start epoch on `> 2f` | NEW-VIEW from `2f + 1` VCs |
| read-phase max-prepared-view selection (`select_value`) | `binds` / `quorumhighest` / `certifiedvalue` over collected `S` | new primary's pre-prepare selection from `V` |
| `NewView { vcs, proposal, sig }` | **Signed Conditional Collect** (Module 5.14, Alg. 5.16): `[COLLECTED, M, Σ]` | NEW-VIEW with signed VIEWCHANGEs + bundled pre-prepares |
| replicas re-verify records + re-run `select_value` | CC1/CC2 + every process computing `binds` from `M` | replicas validate NEW-VIEW before accepting |
| escalating complaints (`highest_vc_sent`) | timestamps increase past failed epochs | timers per view, exponential in practice |
| persist-before-externalize (`pbft-<port>.state`) | logged variants (§5.4 style) | replicas log state to stable storage |

**Deliberately omitted from PBFT** (each a signpost, none accidental): sequence numbers
and pipelining (we decide once; PBFT decides a log), checkpoints/garbage collection and
watermarks (meaningless without a log), the MAC-based fast path (§8's historical note),
and state-transfer for laggards.

## 10. Correspondence between theory and code

| mechanism | code |
|---|---|
| message alphabet + wire format | `enum Message` (5 variants), serde_json lines via `encode`/`decode` |
| canonical statements, domain-separated | `statement()` — one function for signer and verifier; `prepare_statement` shared with `verify_cert` (certificates outlive their messages) |
| envelope authentication | `seal` (sign-at-the-choke-point) → wire → the single `verify_envelope` gate before the `match` |
| trusted setup | `keygen` subcommand → `keys/<port>.sk/.pk`; `load_keys` at boot |
| canonical node order / `leader(v)` | `nodes.sort_by_key(port_of)`; `leader_of` |
| one PRE-PREPARE per view, leader-only, constrained | PrePrepare arm: leader + view + `!preprepared` + `expected.is_none_or(\|e\| *e == m)` |
| first-testimony-wins, view-pure tallies | Prepare/Commit arms record only current-view votes; `entry(from).or_insert`; `viewchanges` keeps each sender's highest-view claim |
| the abandon rule (cutoff) | all three vote arms require `highest_vc_sent <= view`; the counter is persisted |
| Byzantine quorum | `2 * count > n + f` in the Prepare and Commit arms |
| prepare certificate (real object) | `Cert { view, value, sigs }` assembled from the tally at quorum; checked by `verify_cert` (distinct signers, per-sig soft-fail, `2·valid > n+f`) |
| progress timer + escalation | timer thread: 500 ms poll, 4 s timeout; `target = view.max(highest_vc_sent) + 1`; clock reset on fire |
| complain / join | ViewChange arm: record (sig kept — forwardable), `> f` join with `nv > highest_vc_sent` dedup |
| leader's collect-and-forward | count branch (`> 2f`): harvest signed records → `select_value` → broadcast NewView; **no state mutation** |
| the only door into a view | NewView arm: guards → rebuild-and-`verify_envelope` each record → ≥ 2f+1 distinct → re-run `select_value` → proposal must match → enter + `expected` + embedded prepare |
| uniform self-entry (liar self-rejects) | `broadcast` self-send → the leader processes its own NewView through the same arm |
| persist-before-externalize | `persist()` (serde_json + `sync_all`) at every mutation site, including the complaint counter; `load()` + derived `preprepared` at boot |
| view-scoped reset vs survivors | tallies/flags/`expected` cleared on entry; `decided`, `prepared`, `my_prepare`, `highest_vc_sent` survive |
| chaos knobs | `--drop-commits` (skip incoming COMMITs), `--evil-leader` (override the NewView proposal); stdin: `bcast equiv a b`, `fakecert m` |

## 11. Limitations

The original list's first three items — unverifiable claims, self-declared identity,
the view-entry race — are **closed** (certificates §6.4–6.5, envelope authentication
§3, NewView-synchronized entry §6.4). The honest residue:

1. **Trusted setup.** `keygen` + out-of-band public keys is an *assumption*, stated,
   not solved: the layer authenticates known identities; it cannot conjure the
   identity space (Sybil). Real systems anchor it in a PKI, a genesis file, or stake.
2. **Fixed timeout.** 4 s forever; a network slower than that livelocks through views.
   PBFT doubles the timeout per view change (exponential backoff) so that after GST
   some view eventually outlives the network's true delay (Exercise 7).
3. **Single-shot.** One decision; no sequence numbers, checkpoints, log, or state
   transfer — see §9. Consequently no *commit certificate* exists either (§6.5,
   Exercise 9).
4. **Crash-atomicity of `persist`.** `File::create` truncates before writing; a crash
   in that window leaves an empty state file. Real systems write a temp file and
   atomically rename (Exercise 11). And there is no *proactive recovery* — PBFT's
   TOCS 2002 second half (periodic reboot + re-keying so undetected compromises age
   out) is out of scope.
5. **Canonical serialization by convention.** Signed statements embed `serde_json`
   output; deterministic for our fixed types, but "canonical bytes for signing" is a
   real engineering topic (and bug source) that production systems solve explicitly.
6. **Stringly-typed signatures.** `sig: String` filled by `seal` is a convention, not
   a type guarantee — nothing statically prevents encoding an unsealed message
   (Exercise 12 makes illegal states unrepresentable).
7. **Duplicate NewViews.** Between quorum and its own loopback entry, extra arriving
   VIEWCHANGEs can re-trigger the leader's broadcast; receivers deduplicate via the
   `view <= state.view` guard. Mostly harmless — but a re-triggered NewView can
   harvest a *different* record set and proposal, splitting entry state within one
   view and wedging it until the timer rotates. Liveness-only, timeout-absorbed.
8. **The Byzantine repertoire is scripted.** Equivocation, silence, forged
   certificates, a lying NewView — each a targeted experiment confirming a specific
   prediction, not an adversarial search over all behaviors; and the demos assert
   safety, not exhaustively verify it. Model checking is a later course phase.
9. **Unbounded view numbers, unbounded maps** — fine for a toy's lifetime.

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
9. **Commit certificates.** Apply §6.5's rule: add a `Cert`-like object of collected
   COMMIT signatures and a `prove-decision` stdin command that emits it; write the
   verifier. Who is "somebody else in another context" here, and what statement must
   the signatures cover for the proof to be replay-safe across views?
10. **The step function.** Prove: in every run where some correct process decides,
    the value coordinate of every correct process's `prepared` field is eventually
    constant. (This is the lock-in theorem as a data-structure invariant; §6.6.)
11. **Atomic persistence.** Close limitation 4: write to `pbft-<port>.state.tmp`,
    `sync_all`, then `rename`. Why is `rename` the operation that makes this atomic,
    and what does the recovery path do if it finds both files?
12. **Make illegal states unrepresentable.** Split `Message` into an unsigned type
    and a `Sealed` wrapper such that `encode` only accepts sealed messages and
    `seal` is the sole constructor. Which of this module's near-miss bugs does the
    type system now catch at compile time?

## Historical and practical notes

- **"Practical."** Before PBFT (OSDI 1999), Byzantine agreement was widely regarded as
  a theoretical curiosity — synchronous protocols with unrealistic assumptions or
  asynchronous ones with prohibitive costs. Castro & Liskov's title was a thesis
  statement: three message delays, MACs instead of signatures, and a replicated NFS
  within a few percent of the unreplicated one. The `3f + 1` bound traces to
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
  protocol whose single-slot core this module implements (the original is multi-shot,
  MAC-authenticated in the common case, signed at the view change).
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
cargo run -- keygen 7000 7001 7002 7003   # trusted setup: keys/<port>.sk/.pk (gitignored)

# four terminals (n = 4, f = 1; leader of view 0 is the lowest port):
cargo run -- 7000 127.0.0.1:7001 127.0.0.1:7002 127.0.0.1:7003
cargo run -- 7001 127.0.0.1:7000 127.0.0.1:7002 127.0.0.1:7003
cargo run -- 7002 127.0.0.1:7000 127.0.0.1:7001 127.0.0.1:7003
cargo run -- 7003 127.0.0.1:7000 127.0.0.1:7001 127.0.0.1:7002

# stdin, on the current view's leader:
propose lunch            # normal case: everyone decides
bcast equiv a b          # Byzantine attack: equivocate + withhold (view-0 leader only)
fakecert evil            # Byzantine insider: claim a forged certificate (any node)

# chaos flags (append to a node's argument list):
#   --drop-commits       ignore incoming COMMITs (prepared-but-undecided state)
#   --evil-leader        as new leader, propose 'evil' regardless of the evidence
```

Nodes persist durable state to `pbft-<port>.state` (gitignored) and recover on
restart. Scripted, reproducible experiments (each prints a verdict):

```bash
cd demos
python3 happy_path.py             # normal case: 4/4 decide, no view change
python3 equivocation_wedge.py     # safety without liveness: no split, no decision
python3 view_change_unwedge.py    # NewView entry + fresh proposal restore liveness
python3 crash_recovery.py         # whole cluster dies; disks remember (fsync!)
python3 forged_commits.py         # keyless outsider dies at the envelope gate
python3 forged_certificate.py     # keyed insider dies at verify_cert
python3 byzantine_new_leader.py   # lying leader deposed; decided value chaperoned
```

---
*[Course home](../) · Previous: [Module 10](../10-byzantine-broadcast/) · Next:
[Module 12 (planned)](../12-crdts-eventual-consistency/)*
