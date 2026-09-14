use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

const THREADS: usize = 4;
const ITERS: usize = 1_000_000;

fn mutex_counter() -> (u64, std::time::Duration) {
    let counter = Arc::new(Mutex::new(0u64));
    let t0 = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..THREADS {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..ITERS {
                *counter.lock().unwrap() += 1;
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let n = *counter.lock().unwrap();
    (n, t0.elapsed())
}

fn broken_counter() -> (u64, std::time::Duration) {
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..THREADS {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..ITERS {
                counter.store(counter.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let n = counter.load(Ordering::Relaxed);
    (n, t0.elapsed())
}

fn fetch_add_counter() -> (u64, std::time::Duration) {
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..THREADS {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..ITERS {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let n = counter.load(Ordering::Relaxed);
    (n, t0.elapsed())
}

fn main() {
    let (n, d) = broken_counter();
    println!("broken:    count = {n}  ({d:?})");
    let (n, d) = fetch_add_counter();
    println!("fetch_add: count = {n}  ({d:?})");
    let (n, d) = mutex_counter();
    println!("mutex:     count = {n}  ({d:?})");
}
