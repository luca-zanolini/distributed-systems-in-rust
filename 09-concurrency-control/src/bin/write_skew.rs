use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct Db {
    clock: u64,                             // tick one per successful commit
    data: HashMap<String, Vec<(u64, i64)>>, // per key: (commit_ts, value), newest last
}

struct Tx {
    snapshot: u64,
    writes: HashMap<String, i64>,
}

type Store = Arc<Mutex<Db>>;

fn begin(store: &Store) -> Tx {
    let db = store.lock().unwrap();
    Tx {
        snapshot: db.clock,
        writes: HashMap::new(),
    }
}

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

// Withdraw `amount` from `key`, but only if the COMBINED balance covers it.
// Reads both accounts; writes only its own. This is the write-skew shape:
// the constraint spans keys the transaction reads but does not write.
fn withdraw_covered(store: &Store, key: &str, amount: i64) {
    loop {
        let mut tx = begin(store);
        let x = read(&tx, store, "x");
        let y = read(&tx, store, "y");
        if x + y - amount < 0 {
            println!("withdraw {amount} from {key}: REFUSED (total {} would not cover it)", x + y);
            return;
        }
        thread::sleep(Duration::from_millis(5)); 
        let balance = read(&tx, store, key);
        write(&mut tx, key, balance - amount);
        if commit(tx, store) {
            println!(
                "withdraw {amount} from {key}: committed (I checked: total was {}, rule holds)",
                x + y
            );
            return;
        }
        println!("CONFLICT on {key} — retrying");
    }
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
    println!("bank rule: x + y >= 0. start: x=100 y=100");

    let s1 = Arc::clone(&store);
    let h1 = thread::spawn(move || withdraw_covered(&s1, "x", 150));
    let s2 = Arc::clone(&store);
    let h2 = thread::spawn(move || withdraw_covered(&s2, "y", 150));
    h1.join().unwrap();
    h2.join().unwrap();

    let db = store.lock().unwrap();
    let x = read_at(&db, "x", db.clock);
    let y = read_at(&db, "y", db.clock);
    println!("final: x={x} y={y} total={}", x + y);
    if x + y < 0 {
        println!("INVARIANT BROKEN: x + y < 0 — both txs committed, each certain the rule held.");
    } else {
        println!("invariant held.");
    }
}
