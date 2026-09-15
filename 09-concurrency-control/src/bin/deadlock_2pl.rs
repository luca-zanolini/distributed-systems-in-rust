use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

fn transfer(
    store: &Arc<Mutex<HashMap<String, i64>>>,
    locks: &Arc<HashMap<String, RwLock<()>>>,
    from: &str,
    to: &str,
    amount: i64,
) {
    let _tx_from = locks.get(from).unwrap().write().unwrap();
    thread::sleep(Duration::from_millis(5));
    let _tx_to = locks.get(to).unwrap().write().unwrap();
    {
        *store.lock().unwrap().get_mut(from).unwrap() -= amount;
    } // debit: lock–write–unlock
    println!("transfer: debited {from}");
    thread::sleep(Duration::from_millis(10)); // the gap — invariant privately false
    {
        *store.lock().unwrap().get_mut(to).unwrap() += amount;
    } // credit: lock–write–unlock
    println!("transfer: credited {to}");
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
    store.lock().unwrap().insert("x".into(), 50);
    store.lock().unwrap().insert("y".into(), 50);
    let t1 = thread::spawn(move || {
        transfer(&store1, &locks1, "x", "y", 30);
    });

    let t2 = thread::spawn(move || {
        transfer(&store2, &locks2, "y", "x", 30);
    });

    t1.join().unwrap();
    t2.join().unwrap();

    let final_balance_x = store.lock().unwrap().get("x").copied().unwrap_or(0);
    let final_balance_y = store.lock().unwrap().get("y").copied().unwrap_or(0);
    println!("final balance: x = {final_balance_x}, y = {final_balance_y}");
}
