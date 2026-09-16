use crdts::GCounter;

fn main() {
    let mut a = GCounter::new(3);
    let mut b = GCounter::new(3);
    let mut c = GCounter::new(3);

    for _ in 0..3 {
        a.increment(0);
    }
    for _ in 0..5 {
        b.increment(1);
    }
    for _ in 0..2 {
        c.increment(2);
    }

    println!(
        "During partition: a={}, b={}, c={} (true global count: {})",
        a.value(),
        b.value(),
        c.value(),
        a.value() + b.value() + c.value()
    );

    b.merge(&a);
    println!("b after hearing a:       {:?} = {}", b.slots, b.value());
    b.merge(&a); // the SAME gossip again — M1's poison, now a no-op
    println!(
        "b after hearing a AGAIN: {:?} = {} (duplicate harmless)",
        b.slots,
        b.value()
    );

    c.merge(&b);
    a.merge(&c);
    b.merge(&a); // closing round so everyone catches up
    c.merge(&a);

    println!("final a: {:?} = {}", a.slots, a.value());
    println!("final b: {:?} = {}", b.slots, b.value());
    println!("final c: {:?} = {}", c.slots, c.value());
}
