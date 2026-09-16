use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

type Store = Arc<Mutex<HashMap<String, (u64, i64)>>>;

struct Tx {
    reads: HashMap<String, u64>,  // key -> version I saw   (my photographs)
    writes: HashMap<String, i64>, // key -> value I intend  (my private drafts)
}

fn read(tx: &mut Tx, store: &Store, key: &str) -> i64 {
    if let Some(value) = tx.writes.get(key) {
        return *value;
    }
    let (version, value) = {
        let store_guard = store.lock().unwrap();
        store_guard.get(key).copied().unwrap_or((0, 0))
    };
    tx.reads.insert(key.to_string(), version);
    value
}

fn write(tx: &mut Tx, key: &str, value: i64) {
    tx.writes.insert(key.to_string(), value);
}

fn commit(tx: Tx, store: &Store) -> bool {
    let mut store_guard = store.lock().unwrap();
    for (key, &version) in &tx.reads {
        if let Some(&(current_version, _)) = store_guard.get(key) {
            if current_version != version {
                return false;
            }
        }
    }
    for (key, &value) in &tx.writes {
        let entry = store_guard.entry(key.to_string()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 = value;
    }
    true
}

fn transfer(store: &Store, amount: i64) {
    loop {
        let mut tx = Tx {
            reads: HashMap::new(),
            writes: HashMap::new(),
        };
        let x = read(&mut tx, store, "x");
        let y = read(&mut tx, store, "y");
        write(&mut tx, "x", x - amount);
        write(&mut tx, "y", y + amount);
        if commit(tx, store) {
            println!(
                "transfer: committed ({} -> x, {} -> y)",
                x - amount,
                y + amount
            );
            return;
        }
        println!("transfer: CONFLICT — retrying");
    }
}

fn audit(store: &Store) {
    loop {
        let mut tx = Tx {
            reads: HashMap::new(),
            writes: HashMap::new(),
        };
        let x = read(&mut tx, store, "x");
        thread::sleep(Duration::from_millis(5)); // the straddle
        let y = read(&mut tx, store, "y");
        if commit(tx, store) {
            println!("audit: TOTAL {}   (x={x}, y={y})", x + y);
            return;
        }
        println!("audit: view TORE (saw x={x}, y={y} = {}) — retrying", x + y);
    }
}

fn main() {
    let store: Store = Arc::new(Mutex::new(HashMap::new()));
    store.lock().unwrap().insert("x".into(), (0, 50)); // (version 0, balance 50)
    store.lock().unwrap().insert("y".into(), (0, 50)); // (version 0, balance 50)

    let store_clone2 = Arc::clone(&store);
    let handle2 = thread::spawn(move || {
        audit(&store_clone2);
    });

    let store_clone = Arc::clone(&store);
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(2));
        transfer(&store_clone, 30);
    });

    handle.join().unwrap();
    handle2.join().unwrap();
}
