//! Treiber stack (1986) — STEP B: lock-free.
//!
//! Same chain as step A (see git history for the single-threaded version);
//! two changes only:
//!   1. `top` is now an AtomicPtr — an atomic cell whose word is an address.
//!   2. Each pointer swing became a compare-and-swap RETRY LOOP:
//!      photograph top -> prepare the move -> CAS(photo -> replacement)
//!      -> Ok: done | Err: the world moved, re-photograph, try again.
//! Nobody ever waits. A thread frozen mid-operation holds NOTHING; every
//! other thread's CAS proceeds. That is lock-freedom: progress cannot be
//! taken hostage by any single thread's death or preemption
//! (Treiber : Mutex :: quorum consensus : 2PC).

use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;

struct Node {
    value: u64,
    /// Address of the node below me — no paperwork. null = bottom.
    next: *mut Node,
}

pub struct TreiberStack {
    /// The chain's only entrance, now atomically swingable.
    top: AtomicPtr<Node>,
}

impl TreiberStack {
    pub fn new() -> Self {
        TreiberStack {
            top: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Note `&self`, not `&mut self`: atomics mutate through shared
    /// references (interior mutability, like Mutex) — this is what lets the
    /// stress test share the stack via a plain Arc with no lock anywhere.
    pub fn push(&self, v: u64) {
        // Born owned, orphaned immediately (into_raw). `next` gets its real
        // aim inside the loop — it must be re-aimed on every retry anyway.
        let node = Box::into_raw(Box::new(Node {
            value: v,
            next: ptr::null_mut(),
        }));

        loop {
            // Photograph the entrance.
            let seen = self.top.load(Ordering::Acquire);

            // Aim my node's next at the photograph. Fence needed (raw deref),
            // but the vouch is easy: until my CAS succeeds, `node` is PRIVATE
            // — no other thread has ever seen this address. Exclusive by
            // construction; no rival can touch it.
            unsafe {
                (*node).next = seen;
            }

            // "If top still equals my photograph, make it my node." Release
            // on success PUBLISHES the node's fields (value, next) to whoever
            // Acquire-loads top next — the flag-publishes-data pattern.
            match self
                .top
                .compare_exchange(seen, node, Ordering::Release, Ordering::Acquire)
            {
                Ok(_) => return, // my swing landed; the node is now public
                Err(_) => continue, // rival moved top between my load and my
                                  // CAS: stale photo, retry. Cost of losing:
                                  // one re-aim. Nobody waited.
            }
        }
    }

    pub fn pop(&self) -> Option<u64> {
        loop {
            let seen = self.top.load(Ordering::Acquire);
            if seen.is_null() {
                return None; // empty at the moment of the photograph
            }

            // THE FENCE, concurrent edition. We dereference `seen`, an address
            // a rival may have popped a microsecond ago. What vouches for us:
            //   fact (b) — nodes are NEVER freed (the load-bearing leak), so
            //   any address that was ever in the structure points at valid
            //   memory forever. Reading a stale node is harmlessly reading
            //   history; the CAS below detects staleness and retries.
            //   (Fact (a) — "nothing changed since I looked" — died with
            //   step A. The CAS is its replacement: trust, but verify.)
            let next = unsafe { (*seen).next };

            match self
                .top
                .compare_exchange(seen, next, Ordering::Release, Ordering::Acquire)
            {
                Ok(_) => {
                    // The CAS proves `seen` was still top when we unlinked
                    // it, so this pop uniquely claimed the node: any rival
                    // aiming at the same photograph lost its CAS and retried.
                    let value = unsafe { (*seen).value };
                    // `seen` is now LEAKED: unlinked, ownerless, never freed.
                    // Freeing it here = use-after-free for any rival still
                    // holding this address from ITS photograph. Deferred
                    // reclamation (crossbeam epochs, hazard pointers) is the
                    // grown-up fix; the leak is the honest student one.
                    //
                    // ABA footnote: CAS asks "is top the same ADDRESS?", not
                    // "is the world unchanged?". If addresses were recycled,
                    // "same address" could mean "different node" and a CAS
                    // could wrongly succeed. Our immunity is DERIVED from
                    // fact (b): never freed => allocator never reuses these
                    // addresses => same address really is the same node.
                    // Real reclamation must solve ABA too.
                    return Some(value);
                }
                Err(_) => continue, // stale photo; re-photograph, retry
            }
        }
    }
}

fn main() {
    const THREADS: u64 = 4;
    const PER_THREAD: u64 = 100_000;

    let stack = Arc::new(TreiberStack::new());

    // Phase 1: four rival producers, no lock in sight.
    let mut handles = Vec::new();
    for t in 0..THREADS {
        let stack = Arc::clone(&stack);
        handles.push(thread::spawn(move || {
            for i in 0..PER_THREAD {
                // Tag values so producers stay distinguishable: t*1M + i.
                stack.push(t * 1_000_000 + i);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    // Phase 2: drain single-threaded and audit conservation.
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
