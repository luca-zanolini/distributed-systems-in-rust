use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    let mut forks = Vec::new();
    for _ in 0..5 {
        forks.push(Arc::new(Mutex::new(())));
    }

    let mut handles = Vec::new();
    for i in 0..5 {
        let left = Arc::clone(&forks[i]);
        let right = Arc::clone(&forks[(i + 1) % 5]);
        handles.push(thread::spawn(move || {
            println!("Philosopher {i} is thinking");
            let _l = left.lock().unwrap();
            println!("Philosopher {i} picked up the left fork");
            thread::sleep(Duration::from_millis(1));
            let _r = right.lock().unwrap();
            println!("Philosopher {i} is eating");
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    println!("All philosophers have finished eating.");
}
