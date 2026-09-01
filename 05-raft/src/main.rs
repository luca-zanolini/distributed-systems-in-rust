use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn port_of(addr: &str) -> u16 {
    addr.rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or(u16::MAX)
}

// RequestVote RPC: ask a peer for its vote. Some((their_term, granted)) or None if unreachable.
fn request_vote(peer: &str, term: u64, me: &str) -> Option<(u64, bool)> {
    let mut conn = TcpStream::connect(peer).ok()?;
    conn.write_all(format!("requestvote {term} {me}\n").as_bytes()).ok()?;
    let mut reply = String::new();
    BufReader::new(&conn).read_line(&mut reply).ok()?;
    match reply.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["vote", t, granted] => Some((t.parse().ok()?, *granted == "yes")),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role { Follower, Candidate, Leader }

struct State {
    term: u64,
    role: Role,
    voted_for: Option<String>,
    last_heard: Instant,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).cloned().unwrap_or_else(|| "6000".to_string());
    let peers: Vec<String> = args.iter().skip(2).cloned().collect();
    let me = format!("127.0.0.1:{port}");
    let total = peers.len() + 1;
    let majority = total / 2 + 1;
    let election_timeout = Duration::from_millis(1500 + (port_of(&me) as u64 % 7) * 300);
    println!("node {me} — peers {peers:?} — timeout {election_timeout:?}, majority {majority}");

    let state = Arc::new(Mutex::new(State {
        term: 0, role: Role::Follower, voted_for: None, last_heard: Instant::now(),
    }));

    // ---- election + heartbeat thread ----
    {
        let me = me.clone();
        let peers = peers.clone();
        let state = Arc::clone(&state);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(200));

            // A leader just sends heartbeats (which reset followers' election timers).
            let (role, term) = { let s = state.lock().unwrap(); (s.role, s.term) };
            if role == Role::Leader {
                for peer in &peers {
                    if let Ok(mut c) = TcpStream::connect(peer) {
                        let _ = writeln!(c, "heartbeat {term} {me}");
                    }
                }
                continue;
            }

            // Follower/candidate: has the leader gone silent past our timeout?
            let timed_out = { state.lock().unwrap().last_heard.elapsed() > election_timeout };
            if !timed_out { continue; }

            // Become a candidate for a NEW term (short lock), snapshot the term.
            let term = {
                let mut s = state.lock().unwrap();
                s.term += 1;
                s.role = Role::Candidate;
                s.voted_for = Some(me.clone());
                s.last_heard = Instant::now();
                println!("term {}: {me} → CANDIDATE (requesting votes)", s.term);
                s.term
            };
            
            let total = peers.len() + 1; // including self
            let votes: usize = {
                let mut count = 1; // self-vote
                for peer in &peers {
                    if let Some((_, granted)) = request_vote(peer, term, &me) {
                        if granted {
                            count += 1;
                        }
                    }
                }
                count
            };  

            // Won? Re-lock and confirm we're STILL a candidate in the SAME term.
            let mut s = state.lock().unwrap();
            if s.role == Role::Candidate && s.term == term && votes >= majority {
                s.role = Role::Leader;
                println!("term {term}: {me} → LEADER ({votes}/{total} votes)");
            }
        });
    }

    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let mut writer = match stream.try_clone() { Ok(w) => w, Err(_) => continue };
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() { continue; }

        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["requestvote", term, candidate] => {
                let cand_term: u64 = term.parse().unwrap_or(0);
                let candidate = candidate.to_string();
                let (my_term, granted) = {
                    let mut s = state.lock().unwrap();
                    if cand_term > s.term {
                        s.term = cand_term;
                        s.role = Role::Follower;
                        s.voted_for = None;
                    }
                    let granted = cand_term == s.term
                        && (s.voted_for.is_none() || s.voted_for.as_deref() == Some(candidate.as_str()));
                    if granted {
                        s.voted_for = Some(candidate.clone());
                        s.last_heard = Instant::now();
                    }
                    (s.term, granted)   
                };
                let _ = writeln!(writer, "vote {my_term} {}", if granted { "yes" } else { "no" });
            }
            ["heartbeat", term, _leader] => {
                let hb_term: u64 = term.parse().unwrap_or(0);
                let mut s = state.lock().unwrap();
                if hb_term >= s.term {
                    if hb_term > s.term { s.voted_for = None; }
                    s.term = hb_term;
                    s.role = Role::Follower;
                    s.last_heard = Instant::now(); // a live leader → reset our election clock
                }
            }
            _ => {}
        }
    }
}
