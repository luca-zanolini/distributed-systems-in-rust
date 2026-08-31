use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::time::Instant;

// the numeric port of an address like "127.0.0.1:5001" → 5001 (used to ORDER nodes)
fn port_of(addr: &str) -> u16 {
    addr.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(u16::MAX)
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).cloned().unwrap_or_else(|| "5000".to_string());
    let peers: Vec<String> = args.iter().skip(2).cloned().collect();
    let me = format!("127.0.0.1:{port}"); // this node's id = its address (matches peer addrs)
    println!("node {me} — peers {peers:?}");

    // shared: when did we last hear from each peer?
    let last_heard: Arc<Mutex<HashMap<String, (Instant, String)>>> =
        Arc::new(Mutex::new(HashMap::new()));
    // heartbeat + monitor thread
    {
        let me = me.clone();
        let peers = peers.clone();
        let last_heard = Arc::clone(&last_heard);
        thread::spawn(move || {
            let timeout = Duration::from_secs(3);
            let mut suspected: HashSet<String> = HashSet::new();
            let mut current_status: Option<String> = None;
            let total = peers.len() + 1;
            let majority = total / 2 + 1;

            loop {
                if let Ok(map) = last_heard.lock() {
                    for addr in &peers {
                        let down = match map.get(addr) {
                            Some((t, _)) => t.elapsed() > timeout,
                            None => false,
                        };
                        if down && !suspected.contains(addr) {
                            println!("SUSPECT {addr}");
                            suspected.insert(addr.clone());
                        } else if !down && suspected.contains(addr) {
                            println!("{addr} ALIVE again");
                            suspected.remove(addr);
                        }
                    }
                }

                let mut candidates: Vec<String> = vec![me.clone()];
                for p in &peers {
                    if !suspected.contains(p) {
                        candidates.push(p.clone());
                    }
                }
                let choice = candidates.into_iter().min_by_key(|a| port_of(a)).unwrap();

                for addr in &peers {
                    if let Ok(mut s) = TcpStream::connect(addr) {
                        let _ = writeln!(s, "ping {me} {choice}");
                    }
                }

                let votes: usize = {
                    let mut count = if choice == me { 1 } else { 0 };
                    if let Ok(map) = last_heard.lock() {
                        for addr in &peers {
                            if !suspected.contains(addr) {
                                if let Some((_, vote)) = map.get(addr) {
                                    if vote == &me {
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                    count
                };

                let status = if votes >= majority {
                    format!("I AM LEADER ({votes}/{total} votes)")
                } else if choice == me {
                    format!("candidate, NO majority ({votes}/{total} votes) — standing down")
                } else {
                    format!("voting for {choice}")
                };
                if Some(&status) != current_status.as_ref() {
                    println!("{status}");
                    current_status = Some(status);
                }

                thread::sleep(Duration::from_secs(1));
            }
        });
    }

    // listener
    {
        let last_heard = Arc::clone(&last_heard);
        let listener = TcpListener::bind(format!("127.0.0.1:{port}"))?;
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    if let Some(rest) = line.strip_prefix("ping ") {
                        let mut parts = rest.split_whitespace();
                        if let (Some(sender), Some(vote)) = (parts.next(), parts.next()) {
                            if let Ok(mut map) = last_heard.lock() {
                                map.insert(sender.to_string(), (Instant::now(), vote.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
