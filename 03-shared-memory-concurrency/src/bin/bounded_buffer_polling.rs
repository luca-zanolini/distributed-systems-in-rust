use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct BoundedBuffer {
    queue: Mutex<VecDeque<u64>>,
    capacity: usize,
}

impl BoundedBuffer {
    fn new(capacity: usize) -> Self {
        BoundedBuffer {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }
    fn put(&self, v: u64) {
        loop {
            {
                let mut queue = self.queue.lock().unwrap();
                if queue.len() < self.capacity {
                    queue.push_back(v);
                    return;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn get(&self) -> u64 {
        loop {
            let mut queue = self.queue.lock().unwrap();
            if let Some(v) = queue.pop_front() {
                return v;
            }
            drop(queue);
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn main() {
    let buffer = Arc::new(BoundedBuffer::new(10));

    let producer = {
        let buffer = Arc::clone(&buffer);
        thread::spawn(move || {
            for i in 0..50 {
                buffer.put(i);
                println!("Produced {}", i);
            }
        })
    };

    let consumer = {
        let buffer = Arc::clone(&buffer);
        thread::spawn(move || {
            for _ in 0..50 {
                let v = buffer.get();
                println!("Consumed {}", v);
            }
        })
    };

    producer.join().unwrap();
    consumer.join().unwrap();
}
