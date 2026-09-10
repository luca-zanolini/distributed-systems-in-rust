# Module 10 — Byzantine Reliable Broadcast

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011), Chapter 3. Prerequisites:
[Module 02](../02-networked-kv-store/) (processes and links),
[Module 04](../04-replicated-kv-store/) (quorums); [Module 07](../07-raft/) provides the
contrast with the crash model. The crash-model broadcast hierarchy summarized in §2 is built in
full in [Module 06](../06-logical-time-broadcast/) *(planned)*; this module's treatment is
self-contained. Theory companion: [CONSENSUS.md](../07-raft/CONSENSUS.md).*

**Abstract.** This module introduces the **broadcast** abstraction — one process disseminating a
message to a group so that the group can agree on what was delivered — and follows it into the
**fail-arbitrary (Byzantine)** model, where a faulty process may deviate from its protocol in
any way, including **equivocation**: sending conflicting messages to different peers. The
centerpiece is **Bracha's double-echo broadcast** (Bracha 1987; CCGR Algorithm 3.18), which
realizes **Byzantine reliable broadcast**: all correct processes agree on what — if anything — a
designated sender broadcast, *even when the sender itself is the faulty process*. The
construction requires `N > 3f`, uses only authenticated point-to-point links (no digital
signatures), and buys agreement through **quorum amplification**: an *echo* round for
consistency, a *ready* round for totality. The module also presents the crash-model broadcast
ladder (best-effort, regular reliable, uniform reliable) both as the specification context for
the Byzantine variants and as the foil that explains their design: the crash-model technique of
*relaying content* is worthless against a liar, and replacing relayed content with *counted
witnesses* is precisely Bracha's move. Equivocation and supermajority quorums appear here;
leader replacement and view changes — the hard machinery of Byzantine *consensus* — are deferred
to Module 11, of which this broadcast is the structural nucleus.

---

## Learning objectives

After completing this module, the reader should be able to:

1. define the broadcast abstraction and explain why group communication raises an agreement
   problem that point-to-point communication does not;
2. state the crash-model broadcast hierarchy — **best-effort** (BEB1–BEB3), **regular reliable**
   (RB1–RB4), **uniform reliable** (URB4) — with the algorithmic idea implementing each, and
   place the ordered variants (FIFO, causal, total-order) above it;
3. define the fail-arbitrary model and **equivocation**; explain why faultiness is a worst-case
   label rather than an observable, why `≤ f` presupposes unforgeable identities, and why the
   Byzantine model admits no *uniform* variants and no algorithm-independent failure detectors;
4. state the specifications of **Byzantine consistent broadcast** (BCB1–BCB4) and **Byzantine
   reliable broadcast** (adding **BRB5 totality**); explain why crash-model *agreement* is a
   liveness property while Byzantine *consistency* is a safety property, and why consistency and
   totality together recover agreement;
5. explain why crash-model relaying fails under Byzantine faults and how witness counting
   replaces it; derive `N > 3f` and the Byzantine quorum `> (N+f)/2`;
6. describe the double-echo algorithm (SEND → ECHO → READY) with the role of each threshold —
   echo quorum, ready amplification, delivery — and prove consistency (quorum intersection) and
   totality (amplification);
7. compare the three CCGR algorithms — Authenticated Echo (3.16), Signed Echo (3.17),
   Authenticated Double-Echo (3.18) — by property achieved, communication cost, and
   cryptographic assumption;
8. explain what authenticated links do and do not guarantee about the sender field of a message,
   and why the protocol needs no digital signatures.

---

## 1. Motivation

Point-to-point communication (Module 02) serves two parties; most distributed coordination —
replication, membership, ledgers — requires a process to send a message *to a group*. The naive
transposition of point-to-point reliability ("no message lost or duplicated") is deceptively
inadequate for groups, because a **sender can fail midway**: under crash faults it may reach
some recipients and not others; under arbitrary faults it may *deliberately* tell different
recipients different things. Either way, group members end up with inconsistent views — and
repairing that is an agreement problem, graded by how much failure it withstands. Broadcast
abstractions package these grades behind clean interfaces (CCGR Ch. 3): a `Broadcast(m)` request
and a `Deliver(p, m)` indication, with properties that strengthen from *best-effort* through
*reliable* to *totally ordered* — and, in the fail-arbitrary model, with a new property,
**consistency**, that has no crash-model counterpart because it addresses a failure mode
(lying) that crash-stop processes cannot exhibit.

This module climbs to the top of the *unordered* part of that ladder in the hardest fault
model: **Byzantine reliable broadcast**, the fail-arbitrary equivalent of reliable broadcast,
restricted to a single message from a designated sender. It is a foundational primitive: the
atomic unit of Bracha's own asynchronous Byzantine agreement, of HoneyBadgerBFT, and of the
DAG-based protocols (DAG-Rider, Narwhal/Bullshark) underlying several 2020s blockchains; and its
echo/ready structure is exactly the prepare/commit skeleton of PBFT (Module 11).

## 2. The broadcast ladder in the crash model

The crash-model hierarchy is both the specification context for §4 and the design foil for §5.
It is presented here in compressed form and implemented in full in Module 06; each rung is a
module box in CCGR's style (events: request ⟨ Broadcast | m ⟩, indication ⟨ Deliver | p, m ⟩).

### 2.1 Best-effort broadcast (CCGR Module 3.1)

- **BEB1 (Validity).** If a correct process broadcasts `m`, every correct process eventually
  delivers `m`.
- **BEB2 (No duplication).** No message is delivered more than once.
- **BEB3 (No creation).** If a process delivers `m` with sender `s`, then `s` broadcast `m`.

*Algorithm* ("Basic Broadcast", Alg. 3.1): send `m` to every process over perfect links; deliver
on receipt. One communication step, `O(N)` messages. The reliability burden rests entirely on
the sender: **all guarantees are conditional on the sender staying correct**. If the sender
crashes mid-loop, some correct processes deliver and others never do — BEB permits this. (This
module's naive M1 implementation *is* Basic Broadcast; §7 shows what its conditional guarantee
becomes when the sender lies rather than crashes.)

### 2.2 Regular reliable broadcast (CCGR Module 3.2)

Adds the guarantee best-effort lacks — agreement among the correct, *even if the sender crashes
mid-broadcast*:

- **RB1 (Validity).** If a correct process `p` broadcasts `m`, then `p` eventually delivers `m`.
- **RB2/RB3.** No duplication / no creation, as before.
- **RB4 (Agreement).** If a message `m` is delivered by some correct process, then `m` is
  eventually delivered by every correct process.

*Algorithms.* "Lazy Reliable Broadcast" (Alg. 3.2, fail-stop): use best-effort broadcast, and
when the perfect failure detector reports the sender crashed, the receivers *relay* its
messages. "Eager Reliable Broadcast" (Alg. 3.3, fail-silent): drop the failure detector and
relay unconditionally — **every process re-broadcasts every message upon first delivery**.
Eager relaying needs no timing assumptions and tolerates any number of crashes, at `O(N²)`
messages.

A point worth savoring, because the Byzantine world will invert it: **RB4 agreement is a
*liveness* property** (CCGR §3.3.1). An execution can always be extended to satisfy it —
deliver to the laggards later — so nothing "bad" ever becomes irrevocable. Contrast §4's
Byzantine *consistency*, which is safety: two conflicting deliveries are a violation no future
can repair.

### 2.3 Uniform reliable broadcast (CCGR Module 3.3)

Regular reliable broadcast constrains only *correct* processes: a process may deliver `m` and
then crash, with no other process ever delivering it. If deliveries have external effects
(a printed receipt, a dispensed banknote), that is unacceptable. **Uniformity** strengthens
agreement to cover the faulty:

- **URB4 (Uniform agreement).** If `m` is delivered by *any* process — correct or faulty —
  then every correct process eventually delivers `m`.

*Algorithm* ("All-Ack", Alg. 3.4): deliver only after *all correct* processes are known to have
seen (relayed) the message; the fail-silent variant ("Majority-Ack", Alg. 3.5) delivers after a
**majority** has seen it and consequently requires `N > 2f`. Uniformity is where quorums first
enter broadcast — a preview of §5's arithmetic.

### 2.4 Above the ladder: ordering, and the Byzantine analogue

Orthogonal to reliability, broadcast can constrain delivery *order*: **FIFO** (per-sender
order), **causal** (happens-before order), and **total-order** broadcast (a single global
order — equivalent to consensus, CCGR Ch. 6). These are Module 06's subject. The Byzantine
rung of the reliability ladder is this module's subject, with one structural change (CCGR
§3.10.1): the primitives below are **single-instance** — one broadcast of one message by an
a-priori-known sender — because in the fail-arbitrary model a faulty sender is not even bound
to *tag* its messages honestly, so "agreeing on the set of messages from `s`" must be built
one message at a time. Multi-message *Byzantine broadcast channels* (§3.12) are layered on top
by numbering instances. Finally, the Byzantine model has **no uniform variants**: nothing can
be demanded of a faulty process's deliveries, since a Byzantine process may claim — or fake —
any delivery whatsoever.

## 3. The fail-arbitrary model

- **Fault classes nest.** Crash-stop is the mildest failure (halt, forever silent); omission
  and crash-recovery lie between; **arbitrary (Byzantine)** is the strongest: a faulty process
  may send any message, withhold any message, and coordinate with other faulty processes.
  Every guarantee proved under Byzantine faults therefore holds a fortiori under crashes — the
  demonstrations of §7 exploit this by using a *silent* node as the cheapest Byzantine node.
- **The classification is static and retrospective** (CCGR §3.10.1). A process that deviates at
  any point of an execution is *faulty* in that execution, from the start; the rest are
  *correct*. Specifications quantify over correct processes only.
- **Faultiness is a worst-case label, not an observable.** A Byzantine process that happens to
  follow its protocol produces executions indistinguishable from a correct one's. No property,
  proof, or receiver may depend on *knowing who is faulty*; `f` bounds how many identities
  *may* deviate, and the adversary chooses — per execution, possibly adaptively — whether and
  how they do. Safety must hold in *all* executions, including those where every faulty process
  behaves impeccably. (Indistinguishability arguments of exactly this shape prove the `3f+1`
  lower bound.)
- **Equivocation** is the characteristic Byzantine deviation for broadcast: sending `m` to some
  processes and `m′ ≠ m` to others. Cryptography does not remove it — a faulty sender will
  cheerfully **sign both messages** (CCGR §3.10.1). What defeats equivocation is
  *cross-checking among receivers*, which is the entire content of §5.
- **The `≤ f` bound presupposes unforgeable identity.** Byzantine tolerance counts *identities*
  that misbehave. If identities can be minted or spoofed, one faulty process wields unbounded
  identities and every quorum bound collapses — the **Sybil attack** (Douceur 2002). Identity
  must come from outside the protocol: MACs/PKI in permissioned systems, economic identity
  (stake, work) in open ones. §6 states what our implementation assumes here.
- **No failure detectors.** The crash-model escape of packaging timing into an oracle
  (Modules 04–05) does not extend: a Byzantine process can behave perfectly toward any
  detector and deviate elsewhere, and it is known that failure detectors for the
  fail-arbitrary model cannot be defined independently of the algorithm using them (CCGR §2.10,
  citing Doudou et al.). Byzantine protocols rely on quorums and — where needed —
  authentication instead.

## 4. Specifications

Both abstractions are single-instance, with a designated sender `s` known a priori to all.

**Byzantine consistent broadcast** (CCGR Module 3.11):

- **BCB1 (Validity).** If `s` is correct and broadcasts `m`, every correct process eventually
  delivers `m`.
- **BCB2 (No duplication).** Every correct process delivers at most one message.
- **BCB3 (Integrity).** If a correct process delivers `m` with sender `s`, and `s` is correct,
  then `s` broadcast `m`.
- **BCB4 (Consistency).** If a correct process delivers `m` and another correct process
  delivers `m′`, then `m = m′`.

Consistency is the new, distinctly Byzantine property — *"not even an issue for crash-stop
processes"* (CCGR §3.1.2) — and it is **safety**: a split, once it occurs, is beyond repair.
Note what BCB does *not* promise: with a faulty sender, some correct processes may deliver
while others deliver nothing, ever. Consistency without totality is agreement's safety half
only.

**Byzantine reliable broadcast** (CCGR Module 3.12) supplies the liveness half:

- **BRB1–BRB4.** As BCB1–BCB4.
- **BRB5 (Totality).** If some correct process delivers a message, every correct process
  eventually delivers a message.

**Consistency + totality = agreement.** All-deliveries-equal (BRB4) and all-or-none (BRB5)
together yield precisely RB4's agreement, transplanted to the fail-arbitrary model: if any
correct process delivers `m`, every correct process delivers `m`. The crash-model property that
a lying sender destroyed is thus reassembled from two halves — one safety, one liveness — each
enforced by its own round of the algorithm.

Both abstractions require **`N > 3f`** (shown impossible otherwise; CCGR §3.10.2): the bound
arises already for *broadcast*, before any consensus is attempted.

## 5. From relaying to witnessing — and the algorithm

### 5.1 Why the crash-model trick dies

Eager Reliable Broadcast (§2.2) achieves crash-model agreement by **relaying content**: deliver
on first receipt, re-broadcast what you delivered. The relay is trustworthy because a
crash-stop process never forwards anything but what it received. A Byzantine relayer, however,
forwards *fabrications*: "the sender said `m′`" is exactly as trustworthy as the relayer —
worthless. Under authenticated links (no signatures), **second-hand content cannot be
verified**, so the Byzantine protocol never relays it. Instead, each process announces —
first-hand, in its own name, one authenticated hop — *what it heard*, and receivers **count
witnesses per value**. Enough independent witnesses for the same value substitute for the
unavailable transferable proof. (With digital signatures, relayed content *can* carry its own
proof; that is Algorithm 3.17 and Module 11's world — see §5.4.)

### 5.2 The algorithm: authenticated double-echo (CCGR Algorithm 3.18; Bracha 1987)

Three message types, three thresholds. Every process counts, per value, the **distinct**
processes it has heard from — first message per process only; later or conflicting messages
from the same identity are ignored.

```
Phase          rule                                            effect
────────────────────────────────────────────────────────────────────────────
SEND    s → all:  SEND m                                       the sender's claim
ECHO    on first SEND from s:  → all:  ECHO m                  witness the claim
READY   when  #ECHO(m) > (N+f)/2   → all:  READY m             echo quorum reached
         or   #READY(m) > f        → all:  READY m             amplification
deliver when  #READY(m) > 2f                                   delivery
```

Each process sends at most one ECHO and at most one READY (flags `sentecho`, `sentready`), and
delivers at most once (`delivered`; BCB2). With `N = 4, f = 1`: echo quorum `> 2.5` → **3**;
amplify at `> 1` → **2**; deliver at `> 2` → **3**. In code the thresholds are integer-safe:
`2*count > N+f`, `count > f`, `count > 2*f`.

```
        SEND            ECHO                     READY
  s ───────►  each ─── all ───►  #ECHO(m) > (N+f)/2 ─── all ───►  #READY(m) > 2f  ⇒ deliver
                                 (or #READY(m) > f: adopt & relay READY — amplification)
```

The **echo round** makes the sender's claim *witnessed*: no value counts unless a Byzantine
quorum vouches for it — this is where equivocation dies. The **ready round** makes delivery
*contagious among the correct*: the amplification step lets processes that never assembled an
echo quorum adopt the quorum-certified value from `f+1` first-hand READY witnesses — this is
where totality comes from. Omitting the ready round yields exactly Byzantine *consistent*
broadcast (Algorithm 3.16, "Authenticated Echo Broadcast": deliver on the echo quorum), the
weaker primitive of §4.

### 5.3 Correctness

**Consistency (BRB4).** A correct process sends READY for `m` only on an **echo quorum**: a set
`E` with `|E| > (N+f)/2`. Two such sets `E, E′` satisfy
`|E ∩ E′| > (N+f)/2 + (N+f)/2 − N = f`, so their intersection contains a **correct** process —
which echoed *one* value. Hence all echo quorums certify the same value, all correct READYs
carry it, and two correct deliveries cannot differ. This is Module 04's quorum intersection
with a Byzantine twist: the overlap must exceed `f` so that at least one member is *honest* —
the reason the quorum is a supermajority (`> (N+f)/2`) rather than a majority, and the origin
of `N > 3f`. Sets with this pairwise-honest-intersection property are **Byzantine quorums**
(CCGR §2.7.3; Malkhi–Reiter 1998).

**Validity (BRB1).** A correct sender's `m` is echoed by all `≥ N − f` correct processes, and
`N − f > (N+f)/2` because `N > 3f`; every correct process reaches the echo quorum, sends READY,
and — the same arithmetic again, `N − f > 2f` — delivers.

**Totality (BRB5).** Let some correct process deliver `m`: it saw `> 2f` READYs, of which more
than `f` — i.e. at least `f+1` — came from correct processes. Those `f+1` correct READYs reach
*every* correct process, tripping the amplification threshold `> f` everywhere; so all
`≥ N − f` correct processes send READY for `m`, and `N − f > 2f` of them are enough for every
correct process to deliver. Amplification converts *one* correct delivery into *all* — and note
its threshold `f+1` is precisely calibrated: at most `f` READYs can be fabrications, so `f+1`
matching READYs guarantee at least one honest first-hand witness of the echo quorum.

**No duplication (BRB2), Integrity (BRB3)** follow from the `delivered` guard and from BCB4 +
the echo quorum containing a correct witness of the actual SEND, respectively.

**Why `N > 3f`, from first principles.** Availability: a process may wait for at most `N − f`
messages (`f` faulty processes may stay silent forever), so quorums cannot exceed `N − f`.
Honest intersection: two quorums of size `Q` overlap in `≥ 2Q − N` processes, of which up to
`f` may be faulty; an honest common witness needs `2Q − N > f`. With `Q = N − f`:
`N − 2f > f`, i.e. `N > 3f`. Both constraints bind simultaneously — the same derivation as for
Byzantine consensus ([CONSENSUS.md](../07-raft/CONSENSUS.md) §6), already unavoidable one level
below it.

### 5.4 The three algorithms compared

| | **Authenticated Echo** (Alg. 3.16) | **Signed Echo** (Alg. 3.17) | **Authenticated Double-Echo** (Alg. 3.18, Bracha) |
|---|---|---|---|
| implements | Byzantine *consistent* bc | Byzantine *consistent* bc | Byzantine *reliable* bc |
| rounds | SEND, ECHO (2 steps) | SEND, sign-ECHO to sender, FINAL (3 steps) | SEND, ECHO, READY (3 steps) |
| messages | `O(N²)` | **`O(N)`** | `O(N²)` |
| cryptography | authenticated links (MACs) | **digital signatures** + auth. links | authenticated links (MACs) |
| key idea | count first-hand witnesses | sender relays a **certificate** of signed echoes | witness counting + READY amplification |
| totality | no | no | **yes** |

The table is the module's crypto-economics lesson in miniature: signatures make evidence
*transferable* (one relay can carry a certificate — linear messages), while the signature-free
protocols pay quadratic communication to keep all evidence first-hand. PBFT and HotStuff
(Module 11) industrialize the certificate column; Bracha perfects the redundancy column.

## 6. System model of the implementation

- **Processes.** `N = 4` nodes, `f = 1`; the designated sender is fixed by the `--sender`
  argument given identically to all nodes (a-priori agreement on `s`).
- **Links.** TCP, with the **authenticated perfect links** abstraction (CCGR §2.4.6)
  *implemented*, not merely assumed: every message carries an ed25519 signature over a
  domain-separated statement of its content, verified against the claimed sender's known public
  key at a single gate before any protocol logic runs. A message that fails the gate is dropped
  and logged. Key distribution is a trusted setup (a `keygen` step writes `keys/<port>.sk/.pk`;
  all nodes know all public keys) — the explicit anti-Sybil assumption under which "at most `f`
  Byzantine" is meaningful. See §6.5 for why signatures here are strictly a *link-layer* choice.
- **Timing.** Asynchronous — no timeouts anywhere in the protocol. Byzantine *reliable
  broadcast* is solvable in the asynchronous model (no conflict with FLP: with a faulty sender
  the primitive may legitimately deliver nothing, so no decision is *forced*); Byzantine
  *consensus* is not — the precise sense in which broadcast is the easier problem.
- **Adversary.** Deviation is exercised at runtime from the sender's console (`bcast equiv a b`)
  rather than fixed at spawn time: being Byzantine is a *capability*, and whether it is
  exercised is the adversary's choice per execution (§3). A silent (killed) node doubles as the
  mildest Byzantine node.

## 6.5 The authentication layer: MACs vs signatures

The abstraction the proofs stand on — *if a correct process delivers a message attributed to a
correct sender, that sender sent it* — can be discharged by two different primitives, and the
choice is more instructive than it first appears.

**A MAC is spoken testimony; a signature is a notarized affidavit.** A MAC is computed with a
key the two endpoints *share*: the receiver is certain of the sender precisely because it knows
it did not forge the tag itself — which is also exactly why it can convince *nobody else*.
Knowledge authenticated by MACs is real but stuck inside its recipient (**non-transferable**).
A signature is verifiable by anyone holding the public key, no matter how many hands the
message passed through: knowledge made **portable**.

Bracha's algorithm needs only the testimony grade. Every message is **first-hand** — an ECHO
says "the sender showed *me* this value," a READY says "*I* saw a quorum" — and no process ever
forwards another's message. That is why the protocol is signature-free *in principle*, and why
its cost is `O(N²)`: **all-to-all gossip is the price of non-transferable knowledge**. When a
protocol does exploit transferability, its shape changes: in *Signed Echo Broadcast*
(CCGR Alg. 3.17) the sender collects `2f+1` signed echoes into a forwardable bundle — a
**certificate**, knowledge made portable — and the all-to-all round disappears (`O(N)`
messages, one certificate). Every certificate in a protocol marks a spot where transferability
is being spent; Module 11's view change is the load-bearing example, and HotStuff is the design
you get by spending it everywhere.

This implementation uses **ed25519 signatures but only as link authentication** — each
signature is checked by its direct recipient at the gate and never forwarded — for uniformity
with Module 11, where the same primitive *is* used transferably. Two consequences worth
noting:

1. **The structure of Bracha is untouched by the choice.** Thresholds, tallies, amplification —
   byte-for-byte identical with MACs, with signatures, or (as the code stood before this layer)
   with blind trust. Authentication is a layer *below* the protocol; the arms above the gate
   never changed. (One small code-shape difference had we used MACs: a MAC is per recipient-pair,
   so `seal` would move inside the per-peer send loop and compute `N` tags per broadcast; a
   signature is computed once and shipped to all.)
2. **Signatures stop impersonation, not equivocation.** The equivocation demo behaves
   *identically* under the authentication layer: the Byzantine sender owns its key and validly
   signs both `SEND:attack` and `SEND:retreat`. Defeating equivocation is the *protocol's* job
   (the echo quorum), not the crypto's. Nor does the layer change resilience: `N = 3f+1` stands
   regardless — signatures reduce the bound (to any `f`, Dolev–Strong) only under *full*
   synchrony, never in our asynchronous setting.

Mechanics, visible in the code: a canonical, **domain-separated statement** per message
(`SEND:{m}` / `ECHO:{m}` / `READY:{m}` — the type tag prevents an ECHO signature from being
replayed as a READY), a `seal` function signing on the way out, and a `verify_envelope` gate on
the way in that fails *soft* on every hostile shape — unknown sender, malformed hex, wrong
length, invalid signature — because on a network boundary, malformed is malicious and the
correct response is to drop, never to crash. The `demos/forged_ready.py` experiment replays the
pre-layer attack (three impersonated READYs = a fake `2f+1` quorum, enough to make any node
deliver an arbitrary value) and watches it die at the gate.

## 7. Development of the implementation

| # | Design | Deficiency exposed / property gained |
|---|---|---|
| M1 | naive: deliver on first SEND (= CCGR Alg. 3.1, Basic Broadcast) | with a *crashing* sender this loses only BEB's conditional validity; with an *equivocating* sender it *splits the cluster* (`attack` / `retreat` / `retreat`) — consistency violated |
| M2 | ECHO round; deliver on echo quorum `> (N+f)/2` (Alg. 3.16) | equivocation can no longer split; but with a faulty sender some correct processes may deliver while others deliver nothing — consistency without totality |
| M3 | READY round + amplification; deliver at `> 2f` readys (Alg. 3.18) | **totality**: any correct delivery becomes universal — full BRB |

The `demos/` scripts stage the contrast: `equivocation.py` reproduces the M1 split as a foil,
then shows the M3 protocol refusing to split (either all correct nodes deliver one value, or
none deliver — both outcomes satisfy consistency, and which occurs depends on where the liar's
own echo lands); `honest.py` exercises validity + totality; `fault_tolerance.py` delivers with
one node down (`N > 3f` at work).

## 8. Correspondence between theory and code

| Concept | Realization (`src/main.rs`) |
|---|---|
| designated sender `s`, known a priori | `--sender <addr>` on every node; SEND accepted only if `from == sender` |
| authenticated links (assumed) | trusted `from` field on each message (§6, §9) |
| per-identity, first-testimony-only counting | `echos`/`readys`: `HashMap<from, value>` + `entry().or_insert()` |
| echo quorum `> (N+f)/2` → READY | `2 * count > n + f` in the `Echo` arm |
| amplification `> f` → READY | `count > f` in the `Ready` arm |
| delivery `> 2f`, at most once | `count > 2 * f` guarded by `!delivered` (BCB2) |
| `al`-send to all *including self* | `broadcast` fans out to every peer **and** to `me` (loopback), so every state change flows through the listener arms uniformly |
| single ECHO / single READY per process | `sentecho` / `sentready` flags, set before the corresponding broadcast |
| equivocation (the attack) | `bcast equiv a b`: SEND `a` to one half of the peers, `b` to the other |

## 9. Limitations and outlook

- **Authenticated links are now enforced** (§6.5): the pre-layer attack — a keyless socket
  manufacturing a fake `2f+1` READY quorum and breaking integrity/consistency single-handedly,
  the Sybil collapse of §3 — is reproduced and defeated in `demos/forged_ready.py`. What
  remains assumed is the **trusted setup** (out-of-band public-key distribution): the layer
  authenticates known identities, it cannot conjure the identity space itself.
- **Signature-free by design — in the protocol, not the plumbing.** The signatures of §6.5 are
  link authentication only (a MAC would serve identically); Bracha still buys agreement from
  redundancy (`O(N²)`, all first-hand) rather than portable evidence. Transferable use of the
  same primitive — certificates — is deliberately left to Module 11's view change.
- **Single-shot instance.** One broadcast, one sender; a second `bcast` is correctly ignored by
  processes that already delivered (BCB2). Multi-message *Byzantine broadcast channels*
  (CCGR §3.12) tag instances with sequence numbers; a production system runs many instances
  concurrently, keyed by `(sender, seq)`.
- **Crash-stop nodes, volatile state.** Supporting crash-recovery would require persisting
  `sentecho`/`sentready`/the maps *before* emitting the corresponding messages (persist before
  you externalize, Modules 07–08) — a recovered amnesiac node would otherwise echo twice and
  become an equivocator against its own past.
- **No leader, no view change — by design.** The sender is an instance parameter, not a
  replaceable role; with a faulty sender, delivering nothing is a correct outcome, so nothing
  must be rescued. Module 11 adds the obligation to make *progress* despite a faulty leader —
  the obligation that summons the view change.
- **Bandwidth.** Every process retransmits the full message in every ECHO/READY; for large `m`,
  erasure-coded variants disseminate fragments plus a digest (Cachin–Tessaro's verifiable
  information dispersal; HoneyBadger's RBC) at a fraction of the bandwidth.

## 10. Exercises

1. **(The identity assumption is load-bearing.)** The protocol tolerates `f = 1` for `N = 4`.
   Show that with unauthenticated `from` fields (plain TCP, as in the lab), a *single*
   Byzantine process can violate consistency, and name the exact counting step it subverts.
2. **(Threshold necessity.)** Give an execution with `N = 4, f = 1` in which lowering the echo
   quorum from 3 to 2 lets an equivocating sender make two correct processes deliver different
   values.
3. **(Amplification is necessary for totality.)** Construct an execution in which, without the
   `#READY(m) > f` rule, one correct process delivers `m` and another never delivers anything —
   and check that amplification repairs exactly this run. (Hint: a faulty sender can SEND to
   just enough processes that a single correct process assembles an echo quorum.)
4. **(Consistent vs. reliable.)** Modify the implementation to deliver on the echo quorum
   (Algorithm 3.16). Exhibit an execution satisfying BCB1–BCB4 but violating BRB5, and state in
   one sentence what the ready round purchases.
5. **(Signed Echo Broadcast.)** Implement Algorithm 3.17: witnesses sign `⟨ECHO, m⟩` and return
   the signature to the sender, which relays a certificate of `> (N+f)/2` signatures. Compare
   message counts with double-echo, and identify precisely which property of signatures the
   certificate exploits.
6. **(Byzantine witness.)** Extend the adversary so a *non-sender* can equivocate in the ECHO
   round (`a` to some peers, `b` to others). Argue consistency still holds for `f = 1`, and
   determine the smallest `N` at which two colluding Byzantine witnesses can break Algorithm
   3.18's consistency.
7. **(Crash ladder, forward reference.)** Implement Eager Reliable Broadcast (Alg. 3.3) for
   crash faults over the same node skeleton, then run the equivocating sender against it and
   identify which of RB1–RB4 fails and at which relay step trust breaks down. (This exercise
   previews Module 06 and motivates §5.1.)

## Historical and practical notes

*(In the tradition of CCGR's chapter notes: context, provenance, and production history for the
ideas of this module and its neighbors.)*

- **Why "Byzantine."** Lamport recounts that the fault model was nearly named after a
  different nation: wanting generals of a nationality that would offend no reader, he first
  titled the paper *The Albanian Generals Problem* (Albania then being a closed society);
  Jack Goldberg pointed out that Albanians abroad might reasonably object, and the safely
  extinct Byzantines were chosen instead (Lamport, notes to *My Writings*). The `3f + 1`
  bound itself predates the story: it appears in Pease–Shostak–Lamport (JACM 1980), formulated
  during the design of the SIFT fault-tolerant aircraft-control computer; the 1982
  Byzantine-generals paper is the *retelling* that made it famous — an early lesson in the
  value of a good narrative for a technical result.

- **Byzantine quorums are younger than they look.** Quorums for consistency date to Thomas
  (1979) and Gifford (1979) — Module 04's weighted voting — but the *Byzantine* quorum system,
  with its honest-intersection requirement, was formalized only by Malkhi and Reiter (1998),
  a decade after Bracha's protocol had been using the `> (N+f)/2` sets implicitly.

- **Bracha's broadcast outlived the protocol it served.** In Bracha (1987) the double-echo
  broadcast is a *subroutine*: the paper's headline contribution was a randomized asynchronous
  Byzantine *agreement* protocol, and the broadcast primitive was scaffolding to constrain
  what liars could inject into the voting. The scaffolding proved more durable than the
  building: today the agreement protocol is rarely deployed, while "Bracha broadcast" is a
  standard component — HoneyBadgerBFT (Miller et al., CCS 2016) uses an erasure-coded
  descendant (in the lineage of Cachin–Tessaro's verifiable information dispersal) to cut
  bandwidth, and the DAG-based protocols of the 2020s (DAG-Rider, Narwhal/Bullshark) are
  essentially *graphs of reliably-broadcast blocks* — Bracha instances as the edges of a
  ledger.

- **PBFT's MAC gambit.** Castro & Liskov's headline engineering claim (OSDI 1999) was that
  Byzantine fault tolerance could be *practical*, and the key optimization was cryptographic:
  replace digital signatures with vectors of pairwise MACs in the common case — the paper
  reports MACs computable roughly three orders of magnitude faster than the signatures of the
  day. The subtlety: MACs are not *transferable* (a MAC convinces only its addressee), and the
  place where transferability is genuinely needed is the **view change**. The conference
  protocol still signed its view-change messages; eliminating even those (Castro's thesis and
  the journal version) yielded a MAC-only view change whose intricacy became legendary — the
  clearest illustration that *signatures don't change what is solvable, but they radically
  change what is simple*.

- **The view change is where BFT protocols go to die.** Zyzzyva (SOSP 2007), a celebrated
  speculative successor of PBFT, was shown a decade later to violate *safety* — the flaw
  hiding, precisely, in the interaction of speculation with its view change (Abraham, Gueta,
  Malkhi et al., *Revisiting Fast Practical Byzantine Fault Tolerance*, 2017). HotStuff's
  linear view change (PODC 2019) is best read as a direct response to this history: make the
  most dangerous component simple enough to get right. (Module 11 inherits this moral.)

- **The signature economics flipped.** The 2000s doctrine was "MACs where you can, signatures
  where you must." Two decades later, production BFT signs everything: BLS signatures
  (Boneh–Lynn–Shacham, ASIACRYPT 2001) made certificates *aggregatable* — thousands of votes
  compress into one verifiable object — which is how Ethereum's beacon chain digests
  attestations from hundreds of thousands of validators per epoch. Cheap transferable
  evidence turned the linear, certificate-passing protocol designs (Signed Echo, HotStuff)
  from a theoretical curiosity into the deployed default.

- **Where this module's assumption lives in practice.** Our "authenticated perfect links" are
  CCGR §2.4.6, implementable with MACs; the book's chapter notes observe that TLS or SSH
  tunnels provide the abstraction in practice — and that for authentication alone, *encryption
  is not needed and might be turned off for performance*. In blockchains the validator's
  signing key doubles as its identity. The lab simplification — trusting a self-declared
  sender field on localhost — is exactly the gap a MAC would close, and it is called out as an
  honest limit above.

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: the broadcast hierarchy (Modules
  3.1–3.3; Algorithms 3.1–3.5); Byzantine consistent and reliable broadcast (§3.10–3.11,
  Modules 3.11–3.12; Algorithms 3.16–3.18); Byzantine broadcast channels (§3.12); Byzantine
  quorums (§2.7.3); authenticated links (§2.4.6). ISBN 978-3-642-15259-7.

**Byzantine broadcast and agreement**
- G. Bracha, *Asynchronous Byzantine Agreement Protocols*, Information and Computation 75(2),
  1987. (The double-echo broadcast implemented here.)
- L. Lamport, R. Shostak, M. Pease, *The Byzantine Generals Problem*, ACM TOPLAS 4(3), 1982;
  M. Pease, R. Shostak, L. Lamport, *Reaching Agreement in the Presence of Faults*, JACM 27(2),
  1980. (`N > 3f`.)
- D. Malkhi, M. Reiter, *Byzantine Quorum Systems*, Distributed Computing 11(4), 1998.
- M. Castro, B. Liskov, *Practical Byzantine Fault Tolerance*, OSDI 1999.
- A. Miller, Y. Xia, K. Croman, E. Shi, D. Song, *The Honey Badger of BFT Protocols*, CCS 2016.
- C. Cachin, S. Tessaro, *Asynchronous Verifiable Information Dispersal*, SRDS 2005.
  (Erasure-coded reliable broadcast.)
- J. R. Douceur, *The Sybil Attack*, IPTPS 2002. (Why `≤ f` needs unforgeable identities.)

**Broadcast foundations (crash model; developed in Module 06)**
- V. Hadzilacos, S. Toueg, *A Modular Approach to Fault-Tolerant Broadcasts and Related
  Problems*, Cornell TR 94-1425, 1994. (The classical taxonomy of broadcast specifications.)

---

## Running the code

```bash
cargo build
cargo run -- keygen 6000 6001 6002 6003   # trusted setup: keys/<port>.sk/.pk (gitignored)
```

Start a 4-node cluster (each node lists the other three, and the same designated sender):
```bash
cargo run -- 6000 127.0.0.1:6001 127.0.0.1:6002 127.0.0.1:6003 --sender 127.0.0.1:6000
cargo run -- 6001 127.0.0.1:6000 127.0.0.1:6002 127.0.0.1:6003 --sender 127.0.0.1:6000
cargo run -- 6002 127.0.0.1:6000 127.0.0.1:6001 127.0.0.1:6003 --sender 127.0.0.1:6000
cargo run -- 6003 127.0.0.1:6000 127.0.0.1:6001 127.0.0.1:6002 --sender 127.0.0.1:6000
```
In the **sender's** terminal, `bcast hello` broadcasts honestly; `bcast equiv attack retreat`
equivocates (one half of the peers hears `attack`, the other `retreat`). Nodes log protocol
events to standard error. The `demos/` scripts reproduce the experiments of §7 (they drive the
sender's stdin through `subprocess`, which is more reliable than a shell pipeline).

---
*[Course home](../) · Previous: [Module 09 (planned)](../09-concurrency-control/) · Next:
[Module 11 — Byzantine Consensus (planned)](../11-byzantine-consensus/) · Theory map:
[CONSENSUS.md](../07-raft/CONSENSUS.md)*
