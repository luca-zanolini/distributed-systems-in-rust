use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

struct Semaphore {
    permits: Mutex<usize>,
    available: Condvar,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Semaphore {
            permits: Mutex::new(permits),
            available: Condvar::new(),
        }
    }

    fn acquire(&self) {
        let mut count = self.permits.lock().unwrap();
        while *count == 0 {
            count = self.available.wait(count).unwrap();
        }
        *count -= 1;
    }

    fn release(&self) {
        let mut count = self.permits.lock().unwrap();
        *count += 1;
        self.available.notify_one();
    }
}

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
