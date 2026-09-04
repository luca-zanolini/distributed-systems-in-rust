# Module 10 — Byzantine Reliable Broadcast

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Reference text:
**CCGR** (Cachin, Guerraoui & Rodrigues, 2nd ed., 2011), Chapter 3. Prerequisites:
[Module 04](../04-replicated-kv-store/) (quorums), [Module 07](../07-raft/) (for contrast with
the crash model). Theory companion: [CONSENSUS.md](../07-raft/CONSENSUS.md).*

**Abstract.** This module enters the **fail-arbitrary (Byzantine)** model, in which a faulty
process may deviate from its protocol in any way — including **equivocation**, sending
conflicting messages to different peers. It implements **Bracha's double-echo broadcast**
(Bracha 1987; CCGR Algorithm 3.18), which realizes **Byzantine reliable broadcast**: a single
designated sender disseminates one message such that all correct processes agree on what — if
anything — was delivered, *even when the sender itself is the faulty process*. The construction
requires `N > 3f`, uses only authenticated point-to-point links (no digital signatures), and
buys agreement through **quorum amplification** (an *echo* round for consistency, a *ready* round
for totality). The module is deliberately the first Byzantine primitive of the course:
equivocation and supermajority quorums appear here, but leader replacement and view changes —
the hard machinery of Byzantine *consensus* — are deferred to Module 11, of which this broadcast
is the structural nucleus.

---

## Learning objectives

After completing this module, the reader should be able to:

1. define the fail-arbitrary model and **equivocation**, and explain why a faulty sender makes
   reliable broadcast non-trivial in a way it is not under crash faults;
2. state the specifications of **Byzantine consistent broadcast** (BCB1–BCB4) and **Byzantine
   reliable broadcast** (adding **BRB5 totality**), and explain why *consistency* and *totality*
   together recover the *agreement* of the crash-model primitive;
3. explain why `N > 3f` is required, and derive the delivery quorum `> (N+f)/2`;
4. describe the double-echo algorithm (SEND → ECHO → READY) and state the role of each of its
   three thresholds — echo-quorum, ready-amplification, delivery;
5. prove consistency from Byzantine-quorum intersection, and explain how the amplification step
   secures totality;
6. explain why the protocol needs only authenticated links (not signatures), and what that
   assumption does and does not guarantee about the sender field of a message.

---

## 1. Motivation

Under crash faults (Modules 02–08), a process is trusted while it is alive: the difficulty is
only that it may *stop*. In the **fail-arbitrary** (Byzantine) model a faulty process may do
anything at all — send wrong values, send *different* values to different peers
(**equivocate**), stay selectively silent, or collude with other faulty processes. The
classification is static (CCGR §3.10.1): a process that ever deviates in an execution is
**faulty** for that entire execution; the others are **correct**, and specifications constrain
only the correct processes.

Equivocation is what breaks the crash-model broadcast. There, "reliable broadcast" guarantees
*agreement*: if one correct process delivers `m`, all do. But that guarantee tacitly assumes the
sender either sends `m` to everyone or crashes. A Byzantine sender can instead hand `m` to half
the processes and `m′` to the other half — and, crucially, **digital signatures do not help**:
a faulty sender will simply sign both `m` and `m′` (CCGR §3.10.1). Agreement must therefore be
reconstructed by making the *receivers* cross-check each other. That reconstruction is this
module.

Byzantine reliable broadcast is a foundational primitive. It is the atomic unit of Bracha's own
asynchronous Byzantine agreement, of HoneyBadgerBFT, and of the DAG-based protocols
(Narwhal/Bullshark) that underlie several 2020s blockchains; and, as §11 of the course notes,
its echo/ready structure is exactly the prepare/commit skeleton of PBFT.

## 2. System model

- **Processes.** A static set `Π` of `N` processes, of which at most `f` are Byzantine, with
  **`N > 3f`** (here `N = 4`, `f = 1`). One process `s ∈ Π` is the designated **sender**, known
  a priori to all (an instance parameter, not learned from messages).
- **Faults: arbitrary (Byzantine).** Faulty processes may deviate in any way. There are no
  "uniform" variants in this model: nothing can be required of a faulty process's own
  deliveries (CCGR §3.10.1).
- **Links: authenticated perfect links** (CCGR §2.4.6): a message delivered as being from `p`
  was indeed sent by `p`. Implementable with MACs; no digital signatures are used by this
  algorithm. (See §8 for how the implementation approximates this and where it falls short.)
- **Timing: asynchronous.** The algorithm assumes no bounds on message delay; it is safe and —
  because the broadcast concerns a single message and never has to *replace* the sender — also
  live in the asynchronous model (contrast Byzantine *consensus*, which cannot be, by FLP).

## 3. Specifications

**Byzantine consistent broadcast** (CCGR Module 3.11), for a designated sender `s`:

- **BCB1 Validity.** If `s` is correct and broadcasts `m`, every correct process eventually
  delivers `m`.
- **BCB2 No duplication.** Every correct process delivers at most one message.
- **BCB3 Integrity.** If a correct process delivers `m` with sender `s`, and `s` is correct,
  then `s` broadcast `m`.
- **BCB4 Consistency.** If two correct processes deliver `m` and `m′`, then `m = m′`.

Consistency is a *safety* property peculiar to the Byzantine model: with a faulty sender, some
correct processes may deliver a message and others may deliver nothing, but two that *do*
deliver never disagree. Consistency alone does **not** imply agreement.

**Byzantine reliable broadcast** (CCGR Module 3.12) adds the missing liveness half:

- **BRB1–BRB4.** As BCB1–BCB4.
- **BRB5 Totality.** If some correct process delivers a message, then every correct process
  eventually delivers a message.

**Consistency + totality = agreement.** BRB4 (all deliveries equal) and BRB5 (all-or-none
deliver) together yield exactly the agreement property of crash-model reliable broadcast: if any
correct process delivers `m`, every correct process delivers `m`. This is the guarantee the
faulty sender destroyed, rebuilt from receiver-side quorums.

## 4. The algorithm: authenticated double-echo (CCGR Algorithm 3.18)

Three message types and three thresholds. Every process counts, **per value**, the distinct
senders it has heard from (first message per sender only; a Byzantine peer may send several).

```
Phase          rule                                            effect
────────────────────────────────────────────────────────────────────────────
SEND    s → all:  SEND m                                       (the sender's claim)
ECHO    on first SEND from s:  → all:  ECHO m                  (witness the claim)
READY   when  #ECHO(m) > (N+f)/2   → all:  READY m             (echo quorum)
         or   #READY(m) > f        → all:  READY m             (amplification)
deliver when  #READY(m) > 2f                                   (delivery)
```

With `N = 4, f = 1`: echo quorum `> 2.5` → **3 echoes**; amplify at `> 1` → **2 readys**;
deliver at `> 2` → **3 readys**. (In code the thresholds are written to avoid integer-division
error: `2*count > N+f`, `count > f`, `count > 2*f`.)

```
        SEND            ECHO                 READY
  s ───────►  each ─── all ───►   #ECHO(m) > (N+f)/2  ─── all ───►  #READY(m) > 2f  ⇒ deliver
                                  (or #READY(m) > f, amplify)
```

The **echo** round makes the sender's claim witnessed: a value counts only if a Byzantine quorum
vouches for it. The **ready** round, with its amplification, propagates the decision to processes
that did not themselves reach an echo quorum — the mechanism behind totality.

## 5. Correctness

**Consistency (BCB4).** A correct process delivers `m` only after seeing a READY quorum, which
rests on some correct process having seen an **echo quorum** of `> (N+f)/2` echoes for `m`. A set
of `> (N+f)/2` processes is a **Byzantine quorum**; any two Byzantine quorums intersect in more
than `(N+f)/2 + (N+f)/2 − N = f` processes, hence in at least one **correct** process. A correct
process echoes only one value. So two correct processes that deliver `m` and `m′` rest on echo
quorums intersecting in a correct process that echoed both — forcing `m = m′`. (This is
quorum intersection from Module 04, now demanding an *honest* node in the overlap, which is why
the quorum is a supermajority rather than a simple majority.)

**Validity (BCB1).** If `s` is correct and broadcasts `m`, every correct process (≥ `N − f` of
them) echoes `m`; since `N > 3f`, `N − f > (N+f)/2`, so every correct process gathers an echo
quorum and delivers.

**Totality (BRB5).** Suppose some correct process delivers — it saw `> 2f` readys for `m`, so
more than `f` of them, i.e. at least `f + 1`, came from **correct** processes. Those `f + 1`
correct readys reach every correct process, each of which then crosses the **amplification**
threshold `#READY(m) > f` and itself sends READY. Now all `≥ N − f > 2f` correct processes send
READY for `m`, so every correct process eventually sees `> 2f` readys and delivers. The
amplification step is precisely what turns "one correct process delivered" into "all do."

**Why `N > 3f`.** Availability requires waiting for at most `N − f` messages (the `f` faulty may
be silent); consistency requires two quorums to intersect in a correct process. Both hold
simultaneously only when `N > 3f` — the same bound as Byzantine consensus, here shown to be
already necessary for *broadcast*.

## 6. Development of the implementation

| # | Design | Deficiency exposed / property gained |
|---|---|---|
| M1 | naive: deliver on first SEND | an equivocating sender **splits** the cluster (`attack` / `retreat` / `retreat`) — consistency violated |
| M2 | ECHO round; deliver on echo quorum `> (N+f)/2` (Alg. 3.16, *consistent* broadcast) | equivocation can no longer split; but some correct processes may deliver while others deliver nothing — no totality |
| M3 | READY round + amplification; deliver on `> 2f` readys (Alg. 3.18, *reliable* broadcast) | **totality**: if any correct process delivers, all do |

The `demos/` scripts stage exactly this contrast: `equivocation.py` reproduces the M1 split as a
foil, then shows the M3 protocol refusing to split; `honest.py` and `fault_tolerance.py` exercise
validity/totality and the `N > 3f` fault bound.

## 7. Correspondence between theory and code

| Concept | Realization (`src/main.rs`) |
|---|---|
| designated sender `s`, known a priori | `--sender <addr>`; SEND accepted only if `from == sender` |
| authenticated links (assumed) | the `from` field of each message is trusted (see §8) |
| per-sender, first-message-only counting | `echos`/`readys`: `HashMap<from, value>` with `entry().or_insert()` |
| echo quorum `> (N+f)/2` → READY | `2 * count > n + f` in the `Echo` arm |
| amplification `> f` → READY | `count > f` in the `Ready` arm |
| delivery `> 2f` | `count > 2 * f`, guarded by `!delivered` (BCB2) |
| `al`-send to all *including self* | `broadcast` sends to every peer **and** to `me` (loopback), so all protocol state is updated through the listener, uniformly |
| equivocation (the attack) | `bcast equiv a b` sends SEND `a` to one half of the peers, `b` to the other |

## 8. Limitations and outlook

- **Authenticated links are assumed, not enforced.** The implementation trusts the self-declared
  `from` field over plain TCP. This is the authenticated-perfect-links abstraction (CCGR §2.4.6)
  taken on faith; a **MAC** per channel (or TLS) would enforce it. Without enforcement a
  Byzantine *non-sender* could spoof identities and manufacture a fake quorum — a Sybil attack —
  which is why the `≤ f` fault bound is only meaningful over an unforgeable identity space. This
  is the single trust assumption the whole implementation rests on.
- **Signature-free — and that is the point.** Bracha's protocol buys transferable agreement from
  *redundancy* (all-to-all echoes, `O(N²)` messages) rather than from portable evidence. CCGR's
  **Signed Echo Broadcast** (Algorithm 3.17) achieves Byzantine *consistent* broadcast with
  digital signatures in `O(N)` messages and one extra step; the certificate-passing designs of
  Module 11 (PBFT, HotStuff) generalize that trade. This module deliberately stays in the
  signature-free world to isolate the quorum mechanics.
- **Single-shot, single sender.** One instance broadcasts one message with a fixed sender; a
  second `bcast` into a live cluster is (correctly) ignored by processes that already delivered
  (BCB2). CCGR's *Byzantine broadcast channels* (§3.12) layer many instances, tagged by a
  sequence number, into a multi-message primitive.
- **Crash-stop nodes (no recovery).** State is in memory. Supporting crash-recovery would require
  persisting `sentecho`/`echos`/… before emitting the corresponding messages (*persist before
  you externalize*, as in Modules 07–08), lest a recovered node equivocate against its own past.
- **No leader, no view change — by design.** The sender is a fixed instance parameter, not a
  replaceable role, and with a faulty sender delivering *nothing* is a correct outcome. Module 11
  adds the obligation to make progress despite a faulty *leader*, which is what summons the view
  change.

## 9. Exercises

1. **(One faulty node suffices without authentication.)** The protocol tolerates `f = 1`
   Byzantine process for `N = 4`. Show that if the `from` field is unauthenticated (as in the lab
   over plain TCP), a *single* Byzantine process can violate consistency, and identify precisely
   which counting step it subverts.
2. **(Threshold necessity.)** Give an execution with `N = 4, f = 1` in which lowering the echo
   quorum from `> (N+f)/2 = 3` to `2` lets an equivocating sender split two correct processes.
3. **(Amplification is necessary for totality.)** Construct an execution in which, *without* the
   `#READY(m) > f` amplification step, one correct process delivers `m` while another never does
   — and verify that amplification repairs it. (Hint: let the sender cause exactly enough echoes
   for a single correct process to send READY.)
4. **(Signed Echo Broadcast.)** Implement CCGR Algorithm 3.17: witnesses *sign* their echoes and
   return them to the sender, who assembles and relays a certificate. Compare message complexity
   with the double-echo version and identify the property that signatures make transferable.
5. **(Consistent vs. reliable.)** Modify the implementation to deliver on the echo quorum alone
   (Algorithm 3.16, Byzantine *consistent* broadcast). Exhibit an execution satisfying
   consistency but violating totality, and explain what the ready round adds.
6. **(Byzantine witness, not just a Byzantine sender.)** Extend the code so a *non-sender* can be
   Byzantine (e.g. echo `a` to some peers, `b` to others). Argue that consistency still holds for
   `f = 1`, and find the smallest `N` for which two Byzantine witnesses break it.

## Historical and practical notes

*(In the tradition of CCGR's chapter notes: context, provenance, and production history for the
ideas of this module and its neighbors.)*

- **Why "Byzantine."** Lamport recounts that the fault model was nearly named after a
  different nation: wanting generals of a nationality that would offend no reader, he first
  titled the paper *The Albanian Generals Problem* (Albania then being a closed society);
  Jack Goldberg pointed out that Albanians abroad might reasonably object, and the safely
  extinct Byzantines were chosen instead (Lamport, notes to *My Writings*). The `3f + 1`
  bound itself predates the story: it appears in Pease–Shostak–Lamport (JACM 1980); the 1982
  Byzantine-generals paper is the *retelling* that made it famous — an early lesson in the
  value of a good narrative for a technical result.

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
  CCGR §2.4.6, implementable with MACs; in production the same role is played by TLS on every
  connection (or, in blockchains, by the validator's signing key doubling as its identity).
  The lab simplification — trusting a self-declared sender field on localhost — is exactly
  the gap a MAC would close, and it is called out as an honest limit above.

## References

**Reference text**
- C. Cachin, R. Guerraoui, L. Rodrigues, *Introduction to Reliable and Secure Distributed
  Programming*, 2nd ed., Springer, 2011. For this module: Byzantine consistent and reliable
  broadcast (§3.10–3.11, Modules 3.11–3.12; Algorithms 3.16–3.18); Byzantine quorums (§2.7.3);
  authenticated links (§2.4.6). ISBN 978-3-642-15259-7.

**Byzantine broadcast and agreement**
- G. Bracha, *Asynchronous Byzantine Agreement Protocols*, Information and Computation 75(2),
  1987. (The double-echo broadcast implemented here.)
- L. Lamport, R. Shostak, M. Pease, *The Byzantine Generals Problem*, ACM TOPLAS 4(3), 1982;
  M. Pease, R. Shostak, L. Lamport, *Reaching Agreement in the Presence of Faults*, JACM 27(2),
  1980. (`N > 3f`.)
- M. Castro, B. Liskov, *Practical Byzantine Fault Tolerance*, OSDI 1999.
- A. Miller, Y. Xia, K. Croman, E. Shi, D. Song, *The Honey Badger of BFT Protocols*, CCS 2016.
- J. R. Douceur, *The Sybil Attack*, IPTPS 2002. (Why the `≤ f` bound needs unforgeable
  identities.)

---

## Running the code

```bash
cargo build
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
events to standard error. The `demos/` scripts reproduce the experiments of §6 (they drive the
sender's stdin through `subprocess`, which is more reliable than a shell pipeline).

---
*[Course home](../) · Previous: [Module 09 (planned)](../09-concurrency-control/) · Next:
[Module 11 — Byzantine Consensus (planned)](../11-byzantine-consensus/) · Theory map:
[CONSENSUS.md](../07-raft/CONSENSUS.md)*
