# Module 10 — Byzantine Reliable Broadcast *(next to be built)*

*Part of **Concurrent and Distributed Systems in Rust** ([course home](../)). Prerequisites:
[Module 04](../04-replicated-kv-store/) (quorums), [Module 07](../07-raft/) (for contrast with
the crash model). **Status: next in the build order.***

**Planned scope.** The first Byzantine primitive: reliable broadcast when the *sender itself*
may be faulty — the problem of a sender that **equivocates**, telling different processes
different things. Bracha's protocol solves it with `N = 3f + 1` processes and two rounds of
quorum amplification (*echo*, then *ready*), introducing the supermajority-quorum reasoning of
the Byzantine world without the complexity of leader replacement.

Planned content:

- **The Byzantine fault model**: arbitrary deviation, equivocation, collusion; what
  authentication does and does not change.
- **Specification** (Byzantine reliable broadcast, CCGR Ch. 3): validity, no duplication,
  integrity, **consistency**, and **totality**.
- **Bracha's algorithm**: SEND → ECHO (witness a single value: `2f+1` echoes) → READY
  (amplification: `f+1` readies suffice to join, `2f+1` to deliver), and why each threshold is
  what it is.
- **The quorum arithmetic** `N > 3f` derived from availability + honest intersection
  (see [CONSENSUS.md](../07-raft/CONSENSUS.md) §6), exercised concretely.
- **The bridge to Module 11**: PBFT's prepare/commit certificates as the echo/ready pattern in
  service of agreement.

Deliverables, when built: a Bracha implementation over TCP with a scriptable *equivocating
sender* and demonstrations that `3f` honest processes cannot be split; module notes; exercises.

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
  the gap a MAC would close, and it is called out as an honest limit below.

**Principal sources.** G. Bracha, *Asynchronous Byzantine Agreement Protocols*, Information
and Computation 75(2), 1987; CCGR Ch. 3 (Byzantine broadcast variants); L. Lamport,
R. Shostak, M. Pease, *The Byzantine Generals Problem*, ACM TOPLAS 4(3), 1982.

---
*[Course home](../) · Previous: [Module 09 (planned)](../09-concurrency-control/) · Next:
[Module 11 (planned)](../11-byzantine-consensus/)*
