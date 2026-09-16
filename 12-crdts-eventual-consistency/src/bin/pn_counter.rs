use crdts::GCounter;

struct PNCounter {
    pos: GCounter,
    neg: GCounter,
}
impl PNCounter {
    fn new(num_replicas: usize) -> Self {
        Self {
            pos: GCounter::new(num_replicas),
            neg: GCounter::new(num_replicas),
        }
    }

    fn increment(&mut self, replica_id: usize) {
        self.pos.increment(replica_id);
    }

    fn decrement(&mut self, replica_id: usize) {
        self.neg.increment(replica_id);
    }

    fn value(&self) -> i64 {
        // Sound while each pile stays below i64::MAX (`as` would wrap, not saturate) —
        // comfortably true for counters fed by human-scale events.
        self.pos.value() as i64 - self.neg.value() as i64
    }

    fn merge(&mut self, other: &Self) {
        self.pos.merge(&other.pos);
        self.neg.merge(&other.neg);
    }
}

fn main() {
    let mut a = PNCounter::new(2);
    let mut b = PNCounter::new(2);

    // partition: a counts 3 likes and one retraction; b counts 2 likes
    for _ in 0..3 {
        a.increment(0);
    }
    a.decrement(0); // never subtracts — records the retraction in the negative pile
    for _ in 0..2 {
        b.increment(1);
    }
    println!(
        "During partition: a={}, b={} (true global value: {})",
        a.value(),
        b.value(),
        a.value() + b.value()
    );

    b.merge(&a);
    println!("b after hearing a:       P={:?} N={:?} = {}", b.pos.slots, b.neg.slots, b.value());
    b.merge(&a); // duplicate delivery — must change nothing
    println!("b after hearing a AGAIN: P={:?} N={:?} = {} (duplicate harmless)", b.pos.slots, b.neg.slots, b.value());
    a.merge(&b);

    println!("final a: P={:?} N={:?} = {}", a.pos.slots, a.neg.slots, a.value());
    println!("final b: P={:?} N={:?} = {}", b.pos.slots, b.neg.slots, b.value());
}