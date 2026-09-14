use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;

struct BoundedBuffer {
    queue: Mutex<VecDeque<u64>>,
    capacity: usize,
    not_full: std::sync::Condvar,
    not_empty: std::sync::Condvar,
}

impl BoundedBuffer {
    fn new(capacity: usize) -> Self {
        BoundedBuffer {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            not_full: std::sync::Condvar::new(),
            not_empty: std::sync::Condvar::new(),
        }
    }
    fn put(&self, v: u64) {
        let mut queue = self.queue.lock().unwrap();
        while queue.len() == self.capacity {
            queue = self.not_full.wait(queue).unwrap();
        }
        queue.push_back(v);
        self.not_empty.notify_one();
    }

    fn get(&self) -> u64 {
        let mut queue = self.queue.lock().unwrap();
        while queue.is_empty() {
            queue = self.not_empty.wait(queue).unwrap();
        }
        let v = queue.pop_front().unwrap();
        self.not_full.notify_one();
        v
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
