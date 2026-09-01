use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn port_of(addr: &str) -> u16 {
    addr.rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or(u16::MAX)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role { Follower, Candidate, Leader }

struct State {
    term: u64,
    role: Role,
    voted_for: Option<String>,   // who I voted for in `term`
    last_heard: Instant,          // last contact from a leader — the election-timeout clock
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).cloned().unwrap_or_else(|| "6000".to_string());
    let peers: Vec<String> = args.iter().skip(2).cloned().collect();
    let me = format!("127.0.0.1:{port}");
    let total = peers.len() + 1;
    let majority = total / 2 + 1;
    // per-node jitter breaks split votes (real Raft re-randomizes every election)
    let election_timeout = Duration::from_millis(1500 + (port_of(&me) as u64 % 7) * 300);
    println!("node {me} — peers {peers:?} — timeout {election_timeout:?}, majority {majority}");

    let state = Arc::new(Mutex::new(State {
        term: 0,
        role: Role::Follower,
        voted_for: None,
        last_heard: Instant::now(),
    }));

    // election-timer thread
    let election = {
        let me = me.clone();
        let state = Arc::clone(&state);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(200)); // tick
            let mut s = state.lock().unwrap();
            if s.role != Role::Leader && s.last_heard.elapsed() > election_timeout {
                s.term += 1;
                s.role = Role::Candidate;
                s.voted_for = Some(me.clone());
                s.last_heard = Instant::now();
                println!("term {}: {me} → CANDIDATE (voted self)", s.term);
                // self-vote counts as 1; if that alone is already `majority`
                // (a single-node cluster), win now:
                if 1 >= majority {
                    s.role = Role::Leader;
                    println!("term {}: {me} → LEADER", s.term);
                }
            }
        })
    };

    election.join().unwrap(); 
}
