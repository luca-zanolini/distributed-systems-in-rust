use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use shared_memory_concurrency::Semaphore;

fn main() {
    let mut forks = Vec::new();
    for _ in 0..5 {
        forks.push(Arc::new(Mutex::new(())));
    }

    let mut handles = Vec::new();
    // Seat count from argv (default 4). 4 = the cure; 5 = the control experiment
    // (bouncer admits everyone -> the deadlock returns).
    let seats: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let table = Arc::new(Semaphore::new(seats));
    for i in 0..5 {
        let left = Arc::clone(&forks[i]);
        let right = Arc::clone(&forks[(i + 1) % 5]);
        let table = Arc::clone(&table);
        handles.push(thread::spawn(move || {
            println!("Philosopher {i} is thinking");
            table.acquire();
            {
                let _l = left.lock().unwrap();
                println!("Philosopher {i} picked up the left fork");
                thread::sleep(Duration::from_millis(1));
                let _r = right.lock().unwrap();
                println!("Philosopher {i} is eating");
            }
            table.release();
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    println!("All philosophers have finished eating.");
}
