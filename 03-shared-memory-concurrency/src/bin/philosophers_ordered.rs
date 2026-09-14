use std::cmp::{max, min};
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
        let a = i;
        let b = (i + 1) % 5;
        let (first_idx, second_idx) = (a.min(b), a.max(b));
        let first = Arc::clone(&forks[first_idx]);
        let second = Arc::clone(&forks[second_idx]);
        handles.push(thread::spawn(move || {
            println!("Philosopher {i} is thinking");
            let _g1 = first.lock().unwrap();
            println!("Philosopher {i} picked up fork {first_idx}");
            thread::sleep(Duration::from_millis(1));
            let _g2 = second.lock().unwrap();
            println!("Philosopher {i} is eating");
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    println!("All philosophers have finished eating.");
}
