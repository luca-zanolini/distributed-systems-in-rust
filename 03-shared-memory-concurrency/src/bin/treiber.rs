use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;

struct Node {
    value: u64,
    next: *mut Node,
}

pub struct TreiberStack {
    top: AtomicPtr<Node>,
}

impl TreiberStack {
    pub fn new() -> Self {
        TreiberStack {
            top: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn push(&self, v: u64) {
        let node = Box::into_raw(Box::new(Node {
            value: v,
            next: ptr::null_mut(),
        }));

        loop {
            let seen = self.top.load(Ordering::Acquire);
            // SAFETY: `node` came from Box::into_raw above — valid, aligned, and
            // exclusively ours until the CAS below publishes it.
            unsafe {
                (*node).next = seen;
            }
            match self
                .top
                .compare_exchange(seen, node, Ordering::Release, Ordering::Acquire)
            {
                Ok(_) => return, 
                Err(_) => continue, 
            }
        }
    }

    pub fn pop(&self) -> Option<u64> {
        loop {
            let seen = self.top.load(Ordering::Acquire);
            if seen.is_null() {
                return None; 
            }

            // SAFETY: `seen` was a non-null top, and nodes are never freed in this
            // design — the pointee is still alive even if a rival popped it after
            // our load (the load-bearing leak is what makes this dereference sound).
            let next = unsafe { (*seen).next };

            match self
                .top
                .compare_exchange(seen, next, Ordering::Release, Ordering::Acquire)
            {
                Ok(_) => {
                    // SAFETY: the won CAS unlinked `seen`, so no new thread can reach
                    // it; the never-freed guarantee keeps the allocation alive for us.
                    let value = unsafe { (*seen).value };

                    return Some(value);
                }
                Err(_) => continue,
            }
        }
    }
}

fn main() {
    const THREADS: u64 = 4;
    const PER_THREAD: u64 = 100_000;

    let stack = Arc::new(TreiberStack::new());

    let mut handles = Vec::new();
    for t in 0..THREADS {
        let stack = Arc::clone(&stack);
        handles.push(thread::spawn(move || {
            for i in 0..PER_THREAD {
                stack.push(t * 1_000_000 + i);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let mut per_thread = [0u64; THREADS as usize];
    let mut total = 0u64;
    while let Some(v) = stack.pop() {
        per_thread[(v / 1_000_000) as usize] += 1;
        total += 1;
    }

    for (t, n) in per_thread.iter().enumerate() {
        println!("producer {t}: {n} of {PER_THREAD} survived");
    }
    println!("total: {total} of {}", THREADS * PER_THREAD);
    if total == THREADS * PER_THREAD && per_thread.iter().all(|&n| n == PER_THREAD) {
        println!("CONSERVATION EXACT — no push lost, no pop duplicated");
    } else {
        println!("CONSERVATION VIOLATED");
    }
}
