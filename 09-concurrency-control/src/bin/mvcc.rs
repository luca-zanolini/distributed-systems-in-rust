use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct Db {
    clock: u64,                             // tick one per successful commit
    data: HashMap<String, Vec<(u64, i64)>>, // a list of (commit_ts, value) pairs, newest pushed at the end.
}

struct Tx {
    snapshot: u64,
    writes: HashMap<String, i64>,
}

fn begin(store: &Store) -> Tx {
    let snapshot;
    let db = store.lock().unwrap();
    snapshot = db.clock;

    Tx {
        snapshot,
        writes: HashMap::new(),
    }
}

fn read(tx: &Tx, store: &Store, key: &str) -> i64 {
    if let Some(value) = tx.writes.get(key) {
        return *value;
    }
    let db = store.lock().unwrap();
    read_at(&db, key, tx.snapshot)
}

fn write(tx: &mut Tx, key: &str, value: i64) {
    tx.writes.insert(key.to_string(), value);
}

type Store = Arc<Mutex<Db>>;

fn read_at(db: &Db, key: &str, snapshot: u64) -> i64 {
    if let Some(versions) = db.data.get(key) {
        for &(ts, value) in versions.iter().rev() {
            if ts <= snapshot {
                return value;
            }
        }
    }
    0
}

fn commit(tx: Tx, store: &Store) -> bool {
    let mut db = store.lock().unwrap();
    for key in tx.writes.keys() {
        if let Some(versions) = db.data.get(key) {
            if versions.iter().any(|&(ts, _)| ts > tx.snapshot) {
                return false;
            }
        }
    }
    db.clock += 1;
    let now = db.clock;
    for (key, value) in tx.writes {
        db.data.entry(key).or_default().push((now, value));
    }

    true
}

fn deposit(store: &Store, key: &str, amount: i64) {
    loop {
        let mut tx = begin(store);
        let balance = read(&tx, store, key);
        thread::sleep(Duration::from_millis(5)); // the gap — fearless, lock-free
        write(&mut tx, key, balance + amount);
        if commit(tx, store) {
            println!("committed: {} -> {}", balance, balance + amount);
            return;
        }
        println!("CONFLICT on {key} — retrying"); // the engine's signature sound
    }
}

fn transfer(store: &Store, from: &str, to: &str, amount: i64) {
    loop {
        let mut tx = begin(store);
        let from_balance = read(&tx, store, from);
        if from_balance < amount {
            println!("Insufficient funds in {from}");
            return;
        }
        let to_balance = read(&tx, store, to);
        thread::sleep(Duration::from_millis(5)); // the gap — fearless, lock-free
        write(&mut tx, from, from_balance - amount);
        write(&mut tx, to, to_balance + amount);
        if commit(tx, store) {
            println!(
                "committed: {from} -> {}, {to} -> {}",
                from_balance - amount,
                to_balance + amount
            );
            return;
        }
        println!("CONFLICT on {from} or {to} — retrying"); // the engine's signature sound
    }
}

fn audit(store: &Store) {
    let tx = begin(store);
    let x = read(&tx, store, "x");
    thread::sleep(Duration::from_millis(5)); // the straddle
    let y = read(&tx, store, "y");
    println!("audit: x={x} y={y} TOTAL {} (first try, no retry)", x + y);
}

fn main() {
    let store: Store = Arc::new(Mutex::new(Db {
        clock: 0,
        data: HashMap::new(),
    }));

    {
        let mut db = store.lock().unwrap();
        db.data.insert("x".into(), vec![(0, 100)]);
        db.data.insert("y".into(), vec![(0, 100)]);
    }
    println!("--- scene 1: racing deposits (first-committer-wins) ---");
    let store_dep1 = Arc::clone(&store);
    let h_dep1 = thread::spawn(move || {
        deposit(&store_dep1, "x", 10);
    });
    let store_dep2 = Arc::clone(&store);
    let h_dep2 = thread::spawn(move || {
        deposit(&store_dep2, "x", 20);
    });
    h_dep1.join().unwrap();
    h_dep2.join().unwrap();

    println!("--- scene 2: auditor straddles a transfer ---");
    let store_audit = Arc::clone(&store);
    let h_audit = thread::spawn(move || {
        audit(&store_audit);
    });
    thread::sleep(Duration::from_millis(1)); // let the auditor read x first
    let store_transfer = Arc::clone(&store);
    let h_transfer = thread::spawn(move || {
        transfer(&store_transfer, "x", "y", 30);
    });
    h_audit.join().unwrap();
    h_transfer.join().unwrap();
}
