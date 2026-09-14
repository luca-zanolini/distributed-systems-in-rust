use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let state = Arc::new(Mutex::new(0u64));
    let mut handles = Vec::with_capacity(5);
    let t0 = Instant::now();
    for id in 0..4 {
        let reader = thread::spawn({
            let state = Arc::clone(&state);
            move || {
                for n in 0..25 {
                    let _lock = state.lock().unwrap();
                    thread::sleep(Duration::from_millis(5));
                    println!("Reader {id} read #{n}");
                    drop(_lock);
                }
            }
        });
        handles.push(reader);
    }
    let writer = thread::spawn({
        let state = Arc::clone(&state);
        move || {
            for n in 0..5 {
                let _lock = state.lock().unwrap();
                thread::sleep(Duration::from_millis(5));
                println!("Writer wrote #{n}");
                drop(_lock);
            }
        }
    });
    handles.push(writer);

    for h in handles {
        h.join().unwrap();
    }
    let t1 = Instant::now();
    println!("Elapsed time: {:?}", t1 - t0);
}
