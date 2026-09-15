use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

fn deposit(
    store: &Arc<Mutex<HashMap<String, i64>>>,
    locks: &Arc<HashMap<String, RwLock<()>>>,
    key: &str,
    amount: i64,
) {
    let _tx_guard = locks.get(key).unwrap().write().unwrap(); // first line: the X-lock
    let balance = { store.lock().unwrap().get(key).copied().unwrap_or(0) }; // lock–read–unlock
    thread::sleep(Duration::from_millis(5));
    let new_balance = balance + amount;
    store.lock().unwrap().insert(key.to_string(), new_balance); // blind write
    println!("read {balance}, wrote {new_balance}");
}

fn main() {
    let store = Arc::new(Mutex::new(HashMap::new()));
    let store1 = Arc::clone(&store);
    let store2 = Arc::clone(&store);

    let locks: Arc<HashMap<String, RwLock<()>>> = Arc::new(
        [
            ("x".to_string(), RwLock::new(())),
            ("y".to_string(), RwLock::new(())),
        ]
        .into(),
    );

    let locks1 = Arc::clone(&locks);
    let locks2 = Arc::clone(&locks);

    store.lock().unwrap().insert("x".into(), 100);

    let t1 = thread::spawn(move || {
        deposit(&store1, &locks1, "x", 10);
    });

    let t2 = thread::spawn(move || {
        deposit(&store2, &locks2, "x", 20);
    });

    t1.join().unwrap();
    t2.join().unwrap();

    let final_balance = store.lock().unwrap().get("x").copied().unwrap_or(0);
    println!("final balance: {final_balance}");
}
