use std::collections::{HashMap, HashSet};

// (replica_id, seq) — globally unique WITHOUT coordination, but only under two
// preconditions this module assumes throughout: replica ids are distinct (assigned
// at birth, never reused) and each replica mints seqs single-threadedly.
type Tag = (usize, u64);

struct ORSet {
    id: usize,
    seq: u64,                              // my personal tag mint
    adds: HashMap<String, HashSet<Tag>>,   // element -> live-or-dead tags ever added
    tombs: HashSet<Tag>,                   // tags I have removed (observed & killed)
}
impl ORSet {
    fn new(id: usize) -> Self {
        Self {
            id,
            seq: 0,
            adds: HashMap::new(),
            tombs: HashSet::new(),
        }
    }

    fn add(&mut self, element: String) {
        let tag = (self.id, self.seq);
        self.seq += 1;
        self.adds.entry(element).or_default().insert(tag);
    }

    fn remove(&mut self, element: &String) {
        if let Some(tags) = self.adds.get(element) {
            for tag in tags {
                self.tombs.insert(*tag);
            }
        }
    }

    fn contains(&self, element: &String) -> bool {
        if let Some(tags) = self.adds.get(element) {
            for tag in tags {
                if !self.tombs.contains(tag) {
                    return true;
                }
            }
        }
        false
    }

    fn merge(&mut self, other: &Self) {
        for (element, tags) in &other.adds {
            self.adds.entry(element.clone()).or_default().extend(tags);
        }
        self.tombs.extend(&other.tombs);
    }
}

fn main() {
    let mut a = ORSet::new(0);
    let mut b = ORSet::new(1);

    a.add("apple".to_string());
    a.add("banana".to_string());
    b.add("banana".to_string());
    b.add("cherry".to_string());

    println!("During partition: a contains apple? {}", a.contains(&"apple".to_string()));
    println!("During partition: b contains cherry? {}", b.contains(&"cherry".to_string()));

    a.merge(&b);
    b.merge(&a);

    println!("After merge: a contains banana? {}", a.contains(&"banana".to_string()));
    println!("After merge: b contains banana? {}", b.contains(&"banana".to_string()));

    // --- scene 1: remove, then RE-ADD — the move a 2P-Set cannot make ---
    a.remove(&"banana".to_string());
    println!("a removes banana:   a contains banana? {}", a.contains(&"banana".to_string()));
    a.add("banana".to_string()); // fresh tag — no tombstone has ever seen it
    println!(
        "a RE-ADDS banana:   a contains banana? {}  (a 2P-Set would be stuck at false forever)",
        a.contains(&"banana".to_string())
    );

    // --- scene 2: stale gossip cannot resurrect ---
    a.remove(&"banana".to_string()); // kills every tag a can see, re-add included
    a.merge(&b); // b's STALE copy (still has the old live tags) gossips back in
    println!(
        "stale b gossips in: a contains banana? {}  (old tags hit a's tombstones — no resurrection)",
        a.contains(&"banana".to_string())
    );

    // --- scene 3: concurrent add/remove — add wins ---
    b.add("eggs".to_string());
    a.merge(&b);
    b.merge(&a); // both agree: eggs present
    a.remove(&"eggs".to_string()); // partition: a kills the tags it has observed...
    b.add("eggs".to_string()); //            ...while b concurrently re-adds (fresh tag)
    a.merge(&b);
    b.merge(&a); // heal
    println!(
        "concurrent remove(a) vs add(b), healed: a: {}, b: {}  — ADD WINS (the fresh tag outlives every tombstone)",
        a.contains(&"eggs".to_string()),
        b.contains(&"eggs".to_string())
    );
}