use std::sync::Arc;
use std::thread;
use std::time::Duration;

use shared_memory_concurrency::Semaphore;

fn main() {
    let sem = Arc::new(Semaphore::new(3));
    let mut handles = Vec::new();
    for i in 0..8 {
        let sem = Arc::clone(&sem);
        handles.push(thread::spawn(move || {
            sem.acquire();
            println!("worker {i} ENTER");
            thread::sleep(Duration::from_millis(200));
            println!("worker {i} EXIT");
            sem.release();
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}
