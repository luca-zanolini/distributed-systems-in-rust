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

fn deposit(store: &Store, key: &str, amount: i64) {
    loop {
        let mut tx = Tx {
            reads: HashMap::new(),
            writes: HashMap::new(),
        };
        let balance = read(&mut tx, store, key);
        thread::sleep(Duration::from_millis(5)); // the gap — fearless, lock-free
        write(&mut tx, key, balance + amount);
        if commit(tx, store) {
            println!("committed: {} -> {}", balance, balance + amount);
            return;
        }
        println!("CONFLICT on {key} — retrying"); // the engine's signature sound
    }
}

fn main() {
    let store: Store = Arc::new(Mutex::new(HashMap::new()));
    store.lock().unwrap().insert("x".into(), (0, 100)); // (version 0, balance 100)
    let store_clone = Arc::clone(&store);
    let handle = thread::spawn(move || {
        deposit(&store_clone, "x", 10);
    });
    let store_clone2 = Arc::clone(&store);
    let handle2 = thread::spawn(move || {
        deposit(&store_clone2, "x", 20);
    });
    handle.join().unwrap();
    handle2.join().unwrap();
}
