use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn transfer(store: &Arc<Mutex<HashMap<String, i64>>>, from: &str, to: &str, amount: i64) {
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

fn audit(store: &Arc<Mutex<HashMap<String, i64>>>) {
    let x = { store.lock().unwrap().get("x").copied().unwrap_or(0) };
    println!("audit: saw x = {x}");
    thread::sleep(Duration::from_millis(3)); // straddle the transferor's gap
    let y = { store.lock().unwrap().get("y").copied().unwrap_or(0) };
    println!("audit: saw y = {y}   →  TOTAL {}", x + y);
}

fn main() {
    let store = Arc::new(Mutex::new(HashMap::new()));
    let store1 = Arc::clone(&store);
    let store2 = Arc::clone(&store);
    store.lock().unwrap().insert("x".into(), 50);
    store.lock().unwrap().insert("y".into(), 50);
    let t1 = thread::spawn(move || {
        transfer(&store1, "x", "y", 30);
    });

    let t2 = thread::spawn(move || {
        thread::sleep(Duration::from_millis(3));
        audit(&store2);
    });

    t1.join().unwrap();
    t2.join().unwrap();

    let final_balance_x = store.lock().unwrap().get("x").copied().unwrap_or(0);
    let final_balance_y = store.lock().unwrap().get("y").copied().unwrap_or(0);
    println!("final balance: x = {final_balance_x}, y = {final_balance_y}");
}
