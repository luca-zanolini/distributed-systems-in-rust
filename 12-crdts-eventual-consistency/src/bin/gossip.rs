// M4 — the gossip epidemic: 5 replicas, a partition, and provable convergence.
//
// The claim on trial: replicas that only ever exchange state PAIRWISE, on an
// arbitrary schedule, with no coordinator and no ordering guarantees, all end
// up identical once the same news has reached everyone. Convergence is not
// negotiated — it is computed, replica-locally, by the join.
//
// Determinism note (house rule): no randomness, no timing. The gossip schedule
// is a fixed rotation — in round r, replica i pulls from neighbor (i + r) % N.
// An epidemic does not need luck, only enough rounds for news to percolate.

use crdts::GCounter;

const N: usize = 5;

// The partition: {0, 1} on one side, {2, 3, 4} on the other.
fn same_side(i: usize, j: usize) -> bool {
    (i < 2) == (j < 2)
}

fn all_equal(replicas: &[GCounter]) -> bool {
    replicas.windows(2).all(|w| w[0].slots == w[1].slots)
}

fn print_all(replicas: &[GCounter], label: &str) {
    for (i, r) in replicas.iter().enumerate() {
        println!("{label} replica {i}: {:?} = {}", r.slots, r.value());
    }
}

// One gossip round. `partitioned` decides whether cross-split pairs may talk.
// NOTE: merges within a round are sequential and in-place, so replica i+1 may pull a
// state ALREADY enriched this round (a chain like 4→0→1→2→3 can converge in one
// "round"). Real gossip rounds overlap in exactly this asynchronous way; a synchronous
// model would snapshot all states at round start and converge more slowly.
fn round(replicas: &mut Vec<GCounter>, r: usize, partitioned: bool) {
    for i in 0..N {
        let j = (i + r) % N;
        if i == j || (partitioned && !same_side(i, j)) {
            continue; // the partition drops this exchange on the floor
        }
        // The clone IS the network message: gossip ships a copy of the
        // neighbor's state, and the borrow checker makes our simulation
        // admit it (replicas[i].merge(&replicas[j]) would double-borrow).
        let msg = replicas[j].clone();
        replicas[i].merge(&msg);
    }
}

fn main() {
    let mut replicas: Vec<GCounter> = (0..N).map(|_| GCounter::new(N)).collect();

    // Local work, no communication: replica i counts i+1 events.
    // True global total: 1+2+3+4+5 = 15.
    for (i, r) in replicas.iter_mut().enumerate() {
        for _ in 0..=i {
            r.increment(i);
        }
    }
    println!("--- after local counting (no gossip yet; true total 15) ---");
    print_all(&replicas, "  ");

    // Phase 1: gossip UNDER the partition. News spreads within each island only.
    for r in 1..=3 {
        round(&mut replicas, r, true);
    }
    println!("--- after 3 partitioned rounds ({{0,1}} | {{2,3,4}}) ---");
    print_all(&replicas, "  ");
    println!("  news crossed only inside each island — no replica has heard anything from the other side");

    // Phase 2: the partition heals. Same rotation, all pairs allowed.
    let mut healed_rounds = 0;
    for r in 4.. {
        round(&mut replicas, r, false);
        healed_rounds += 1;
        if all_equal(&replicas) {
            break;
        }
    }
    println!("--- after {healed_rounds} healed rounds ---");
    print_all(&replicas, "  ");
    println!(
        "VERDICT: all {N} replicas identical = {} — epidemic convergence, zero coordination",
        replicas[0].value()
    );
}
