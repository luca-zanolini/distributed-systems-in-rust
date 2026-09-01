# 04 — Leader Election

A cluster of nodes that **detects the loss of its leader and elects a new one by majority vote** —
with no central authority (the one you'd ask is the one that died). Nodes exchange **heartbeats**;
silence past a **timeout** marks a peer as suspected; each node **votes** for the lowest-id node it
still sees alive; and a node becomes leader only once a **majority** of votes back it.

This is the piece `03` was missing — **automatic failover** — and the last rung before **consensus**:
combine `04`'s election with `03`'s replicated log and you have **Raft**.

---

## Theory — failure detection & leader election

### 1. Why elect a leader

Many distributed protocols need **one** node in charge to make progress: a *primary* to order writes
(`03`), a sequencer, a lock holder, a coordinator. A single fixed leader is simple — but it can
**crash**, and then everything stalls. Fault tolerance therefore needs two things the group must do
**for itself**:

1. **Detect** that the leader is gone — even though you can't just "ask" it.
2. **Agree** on a replacement — without a central authority to appoint one.

That is leader election, and it is exactly what `03` lacked: its primary was fixed by hand, so a
primary crash was unrecoverable. `04` supplies the missing failover.

### 2. Where it shows up in practice

| Piece | Real systems |
|---|---|
| **Heartbeat failure detection** | Kubernetes node heartbeats, **Raft** election timeouts, gossip/**SWIM** (Consul, Serf), Cassandra's φ-accrual detector |
| **Leader election** | **Raft**/etcd leader, ZooKeeper (ZAB), Kubernetes leader-election leases, the Kafka controller |

Together they are the **control plane** underneath most distributed databases and orchestrators.

### 3. What it precisely is — two abstractions from CCGR Chapter 2 (§2.6)

- **Failure detector** — a module that reports which processes have crashed, *possibly inaccurately*.
  Heartbeats + a timeout give an **eventually perfect** detector, **◇P** (§2.6.4): it may briefly
  suspect a slow-but-alive node (a false positive), but **eventually** every crashed node is
  suspected (*strong completeness*) and it stops suspecting correct ones (*eventual strong
  accuracy*). Its stricter cousin **P** (§2.6.2) never makes mistakes — but only a **synchronous**
  system can implement it.
- **Leader election** — **◇Ω** (§2.6.5): eventually all correct nodes agree on a single correct
  leader. We build the **monarchical** rule ("the alive node with the smallest id leads"), then make
  it *safe* by requiring a **majority vote**.

> Why *eventually*, always? Because in an asynchronous / partially-synchronous network you can
> **never be certain** a node crashed — it might just be slow, or its message delayed (**FLP**,
> §2.5.1). A timeout is a *guess*. So detection and election are **eventual and self-correcting**,
> never instant or certain — the deep truth this whole project makes tangible.

### 4. How this project evolved — one problem at a time

Same as `03`: each milestone fixes the wound the last one exposed.

| # | We built… | …which exposed |
|---|---|---|
| **M1** | **heartbeats** — every node pings its peers each second | it's just a pulse; nobody yet *reacts* to silence |
| **M2** | **failure detection** — suspect a peer silent past a 3s timeout (retract on recovery) | now you can spot a crash, but no one is *in charge* — and a suspicion can be *wrong* (◇P, not P) |
| **M3** | **leader election** — the lowest-id alive node wins (monarchical ◇Ω) | each node decides **locally**, so a lone survivor crowns *itself*, and a partition could crown **two** leaders → **split-brain** |
| **M4** | **majority-vote election** — each node's vote rides its heartbeat; a leader needs a **quorum** of votes | split-brain gone (two majorities can't both form); but there are no **terms** yet → the door to **Raft** |

The arc: **detect** (M1–M2) → **elect** (M3) → **make election *safe*** with a quorum (M4).

> 🎓 **Three experiments that make it visceral.**
> 1. **Detection (M2):** kill a node → after ~3s the survivors print `SUSPECT`; restart it → `ALIVE again`. A detector that is *wrong-then-corrects* — exactly ◇P.
> 2. **Failover (M3/M4):** kill the leader → leadership moves to the next node, *by a fresh majority of votes* — no coordinator, no downtime beyond the timeout.
> 3. **Split-brain safety (M4):** kill 2 of 3 → the lone survivor tallies only its **own** vote (`1/3`) and **stands down**. M3 would have crowned it; real voting refuses.

### 5. The safety argument (quorum intersection — straight from `03`)

A leader needs a **majority of votes**, and each node casts exactly **one** vote (its current
`choice`, broadcast on its heartbeat). Because **any two majorities of N nodes share at least one
node**, and that shared node's single vote backs only one candidate, **two candidates can never both
reach a majority** → at most one leader. It's the *same* intersection theorem that made `03`'s
register correct, now applied to leadership — and it is the core idea of **Raft**.

### 6. In the CCGR framework

- **Failure detectors (§2.6):** ours is **◇P** — heartbeats + timeout, encoding a **partial-synchrony**
  assumption. Perfect **P** (§2.6.2) would need a synchronous system.
- **Leader election (§2.6.5):** the **monarchical eventual leader detector ◇Ω** (lowest id among the
  non-suspected), here gated by a majority quorum for safety.
- **Timing (§2.5):** the 3s timeout *is* the partial-synchrony knob — too short → false suspicions,
  too long → slow detection. **FLP** (§2.5.1) is why we can only ever be *eventual*.
- **Links:** heartbeats ride **best-effort** messages — a dropped ping needs no retransmission logic,
  the next second's ping covers it (closer to *fair-loss* links than the *perfect* links of `02`/`03`).
- **The ladder:** a failure detector is what circumvents FLP to make consensus solvable — indeed **Ω
  is the weakest failure detector for consensus** (Chandra–Hadzilacos–Toueg). `04` builds Ω; `05`
  (Raft) will use it.

### 7. Failure detectors vs. timing models — two lenses on the same thing

You can reason about what a distributed system can *solve* in **two equivalent ways**: directly via
the **timing model** (synchronous / partially synchronous / asynchronous), or via a **failure
detector** (P, ◇P, Ω). A failure detector is really just *synchrony packaged as an oracle* — it lets
you design an algorithm against **axiomatic properties** (no clocks, no timeouts in the proof) and
implement the detector *separately* from whatever timing the system actually has (CCGR §2.6.1).

| Timing model | Detector you can implement | Consensus? |
|---|---|---|
| **Synchronous** | **P** — *perfect*, never wrong (strong accuracy) | yes — even fail-stop, any `f < N` |
| **Partially synchronous** (GST) | **◇P / ◇Ω** — *eventually* accurate | yes — with a **majority**, `f < N/2` (Paxos, Raft) |
| **Asynchronous** | none strong enough | **no — FLP** |

- **Perfect FD ≈ synchrony.** With known delay bounds a timeout *never* misfires → strong accuracy.
  `P` is exactly the crash-detection power a synchronous system buys you.
- **◇Ω ≈ partial synchrony + GST.** This is DLS's *eventually-synchronous* model: before the
  (unknown) **Global Stabilization Time**, timeouts can be wrong — false suspicions, even two
  temporary leaders; *after* GST, delays are bounded, timeouts stop misfiring → **eventual accuracy**
  → a single stable leader eventually emerges and stays. ◇Ω *is* "eventually one correct leader."
- **Asynchrony.** You can't implement even ◇Ω deterministically (no eventual bound to lean on) — the
  failure-detector face of **FLP**.

The two lenses meet in one theorem: **Ω is the *weakest* failure detector that solves consensus**
(Chandra–Hadzilacos–Toueg, 1996). So *consensus is solvable ⟺ you can implement Ω ⟺ you have at
least partial synchrony.* This project lives in the **◇P / ◇Ω** regime; Raft (`05`) assumes the same.

> 🎓 Slogan: **synchrony → P; partial synchrony → ◇Ω; asynchrony → nothing (FLP).** Failure detectors
> are synchrony, packaged as an oracle.

### 8. How the code reflects the theory — and where it stops

| Theory | In this code |
|---|---|
| heartbeat | a 1s background thread pinging every peer: `ping <me> <vote>` |
| failure detector ◇P | `last_heard: HashMap<peer, (Instant, vote)>`; suspect if `elapsed > 3s` |
| monarchical leader ◇Ω | `choice = min_by_key(port_of)` over the non-suspected nodes |
| one vote per node | each node's `choice` **rides its heartbeat**; peers record it |
| majority election | tally votes cast *for me*; lead only if `votes ≥ ⌊N/2⌋+1` |

**Honest limits — the syllabus beyond this project (each a signpost):**

- **No terms.** A node re-votes every round; nothing stops it voting for different candidates over
  time, so under rapid churn there's a theoretical window. **Raft** adds a monotonic **term** +
  *vote-once-per-term* to close it. *(→ `05`, Raft.)*
- **The leader does nothing yet.** It is elected but idle — we don't route work to it. Wiring it into
  `03` (leader coordinates writes; on failover a new leader takes over) is precisely the combination
  that **becomes Raft.** *(→ `05`.)*
- **Fixed timeout (3s).** Real detectors **adapt** to network conditions (φ-accrual, Cassandra). A
  fixed timeout is a blunt trade of false-positives vs detection speed. *(→ adaptive FD.)*
- **Static membership.** Peers are hardcoded at launch — no join/leave/discovery. *(→ membership, gossip/SWIM.)*
- **Crashes, not partitions.** Killing a process is a *crash*; we can't easily simulate a true
  network *partition* (both sides up, unable to talk). The majority gate is what keeps a partitioned
  minority from leading, but full asymmetric-partition safety needs the *terms* above.
- **A fresh TCP connection per heartbeat** — simple but wasteful; real systems reuse connections or use UDP.

---

## Run

```bash
cargo build
cargo test        # unit tests for port_of (numeric node ordering)
```

Start a **3-node cluster** — every node lists the *other two* as peers:
```bash
cargo run -- 5000 127.0.0.1:5001 127.0.0.1:5002
cargo run -- 5001 127.0.0.1:5000 127.0.0.1:5002
cargo run -- 5002 127.0.0.1:5000 127.0.0.1:5001
```
Each node logs its own view: `SUSPECT …` / `… ALIVE again`, and its leadership status
`voting for …` / `I AM LEADER (v/n votes)` / `candidate, NO majority …`. Kill the lowest node
(`Ctrl+C`) and watch leadership fail over to the next.

**Wire protocol** (one line, newline-framed): `ping <sender-addr> <vote-addr>` — a heartbeat that
*also* carries the sender's current vote. That single message type is the whole protocol.

**Failure demos** (`demos/`) drive a real cluster over TCP:

| Script | Shows |
|---|---|
| `failure_detection.py` | kill a node → `SUSPECT`; restart it → `ALIVE again` (◇P: wrong-then-corrects) |
| `election.py` | failover by majority (`5000 → 5001`), then kill 2 of 3 → the survivor **stands down** (no quorum) |

## Design & notable implementation details

- **Every node runs the identical loop** (no coordinator): each second it (1) updates suspicions from
  `last_heard`, (2) recomputes its vote `choice`, (3) broadcasts `ping <me> <choice>`, (4) tallies the
  votes cast for it, (5) logs any status change. Convergence is emergent — it's a **self-stabilizing,
  steady-state** protocol, not a one-shot election.
- **Votes ride heartbeats.** Rather than a separate election RPC, each heartbeat carries the sender's
  current vote; every node keeps peers' latest votes in `last_heard: HashMap<peer, (Instant, vote)>`
  and counts locally. One message type does both failure detection *and* voting.
- **Two clocks:** a **1s** period (how often each node acts) and a **3s** timeout (silence → suspect).
  The timeout must exceed the heartbeat period, or a single late ping causes a false suspicion.
- **Node id = its address**, ordered by **port number** (`port_of` parses and compares numerically, so
  `10000 > 9000` — string order would get that backwards).
- **Concurrency:** the monitor thread and the listener share `last_heard` via `Arc<Mutex<…>>` —
  *local* shared memory between threads; across nodes there is only message passing.

## What I learned

*Rust:* background threads with **timers** (`Duration`, `thread::sleep`, `Instant::elapsed`),
sharing state across threads with `Arc<Mutex<HashMap>>` (writer thread + reader thread), tuple map
values `(Instant, String)`, iterator tools (`min_by_key`, `filter`, `split_whitespace`), `if let
Ok(..)`/`if let Some(..)` for best-effort I/O, and a small pure helper worth unit-testing (`port_of`).

*Distributed systems:* **failure detection** (heartbeats, timeouts, suspicion) and why it can only be
**eventually perfect (◇P)** under partial synchrony; **leader election (◇Ω)** and the monarchical
rule; **split-brain** and how a **majority vote** (quorum intersection, again) prevents it; the
difference between *seeing* a majority alive and *collecting* a majority of votes; and a concrete feel
for why this is the doorstep to **consensus / Raft**.

---

## References

**Course reference text (the theory spine for the whole repo)**
- Christian Cachin, Rachid Guerraoui & Luís Rodrigues, *Introduction to Reliable and Secure
  Distributed Programming*, 2nd ed., Springer, 2011. For `04`: **failure detectors** and **leader
  election** (Ch. 2, §2.6 — P, ◇P, ◇Ω), **timing assumptions** (§2.5). ISBN 978-3-642-15259-7.

**Failure detectors & the theory of election**
- Tushar Chandra & Sam Toueg, *Unreliable Failure Detectors for Reliable Distributed Systems*, JACM
  1996. The foundational theory of failure detectors (P, ◇P, ◇S) — exactly the abstraction M2 builds.
- Tushar Chandra, Vassos Hadzilacos & Sam Toueg, *The Weakest Failure Detector for Solving Consensus*,
  JACM 1996. **Ω** — a leader oracle — is the *weakest* detector that makes consensus solvable; `04`
  is building Ω.
- Michael Fischer, Nancy Lynch & Michael Paterson, *Impossibility of Distributed Consensus with One
  Faulty Process (FLP)*, JACM 1985. Why detection/election can only be *eventual*: no deterministic
  protocol decides in an asynchronous system with even one crash.

**Practice, and where `04` heads next**
- Abhinandan Das, Indranil Gupta & Ashish Motivala, *SWIM: Scalable Weakly-consistent Infection-style
  Process Group Membership Protocol*, DSN 2002. Gossip-based failure detection at scale (Consul, Serf).
- Diego Ongaro & John Ousterhout, *In Search of an Understandable Consensus Algorithm (Raft)*, USENIX
  ATC 2014. Leader election **with terms** + a replicated log — `04`'s election made complete. The
  natural next project.

---
Part of [distributed-systems-in-rust](../).
