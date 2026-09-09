use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

fn port_of(addr: &str) -> u16 {
    addr.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(u16::MAX)
}

enum Message {
    PrePrepare { from: String, view: u64, m: String },
    Prepare { from: String, view: u64, m: String },
    Commit { from: String, view: u64, m: String },
}

fn encode(msg: &Message) -> String {
    match msg {
        Message::PrePrepare { from, view, m } => format!("PREPREPARE {from} {view} {m}\n"),
        Message::Prepare { from, view, m } => format!("PREPARE {from} {view} {m}\n"),
        Message::Commit { from, view, m } => format!("COMMIT {from} {view} {m}\n"),
    }
}

struct State {
    view: u64,
    sentcommit: bool,
    decided: bool,
    preprepared: bool,
    prepares: HashMap<String, String>,
    commits: HashMap<String, String>,
}

fn decode(s: &str) -> Option<Message> {
    let (kind, value) = s.trim().split_once(' ')?;

    match kind {
        "PREPREPARE" => {
            let (from, rest) = value.split_once(' ')?;
            let (view, m) = rest.split_once(' ')?;
            Some(Message::PrePrepare {
                from: from.to_string(),
                view: view.parse().ok()?,
                m: m.to_string(),
            })
        }
        "PREPARE" => {
            let (from, rest) = value.split_once(' ')?;
            let (view, m) = rest.split_once(' ')?;
            Some(Message::Prepare {
                from: from.to_string(),
                view: view.parse().ok()?,
                m: m.to_string(),
            })
        }
        "COMMIT" => {
            let (from, rest) = value.split_once(' ')?;
            let (view, m) = rest.split_once(' ')?;
            Some(Message::Commit {
                from: from.to_string(),
                view: view.parse().ok()?,
                m: m.to_string(),
            })
        }
        _ => None,
    }
}

fn broadcast(peers: &[String], me: &str, message: &Message) {
    let msg = encode(message);
    for peer in peers.iter().cloned().chain(std::iter::once(me.to_string())) {
        let msg = msg.clone();
        std::thread::spawn(move || {
            if let Ok(mut stream) = TcpStream::connect(&peer) {
                let _ = stream.write_all(msg.as_bytes());
            }
        });
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let port = args.get(1).cloned().unwrap_or_else(|| "6000".to_string());
    let me = format!("127.0.0.1:{port}");
    let peers: Vec<String> = args
        .iter()
        .skip(2)
        .take_while(|arg| !arg.starts_with("--"))
        .cloned()
        .collect();

    let candidates = std::iter::once(me.clone()).chain(peers.iter().cloned());

    let leader = candidates.min_by_key(|a| port_of(a)).unwrap();

    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State {
        view: 0,
        sentcommit: false,
        decided: false,
        preprepared: false,
        prepares: HashMap::new(),
        commits: HashMap::new(),
    }));

    let n = peers.len() + 1;
    let f = (n - 1) / 3;
    eprintln!("n = {n}, f = {f}");

    {
        let me = me.clone();
        if leader == me {
            let peers = peers.clone();
            thread::spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    let parts: Vec<&str> = line.split_whitespace().collect();

                    match parts.as_slice() {
                        ["bcast", m] => {
                            eprintln!("Propose: {m}");
                            broadcast(
                                &peers,
                                &me,
                                &Message::PrePrepare {
                                    from: me.clone(),
                                    view: 0,
                                    m: m.to_string(),
                                },
                            );
                        }

                        ["bcast", "equiv", m, n] => {
                            eprintln!("Equivocating: {m} / {n}");
                            let half = peers.len() / 2;
                            broadcast(
                                &peers[..half],
                                &me,
                                &Message::PrePrepare {
                                    from: me.clone(),
                                    view: 0,
                                    m: m.to_string(),
                                },
                            );
                            broadcast(
                                &peers[half..],
                                &me,
                                &Message::PrePrepare {
                                    from: me.clone(),
                                    view: 0,
                                    m: n.to_string(),
                                },
                            );
                        }
                        _ => {
                            eprintln!("Unknown command");
                        }
                    }
                }
            });
        }
    }

    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            continue;
        }
        if let Some(msg) = decode(&line) {
            match msg {
                Message::PrePrepare { from, view: v, m } => {
                    let mut state = state.lock().unwrap();
                    if from == leader && v == state.view && !state.preprepared {
                        state.preprepared = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Prepare {
                                from: me.clone(),
                                view: v,
                                m: m.clone(),
                            },
                        );
                    }
                }
                Message::Prepare { from, view: v, m } => {
                    let mut state = state.lock().unwrap();
                    state.prepares.entry(from).or_insert(m.clone());
                    let count = state.prepares.values().filter(|v| **v == m).count();
                    if 2 * count > n + f && !state.sentcommit {
                        state.sentcommit = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Commit {
                                from: me.clone(),
                                view: v,
                                m: m.clone(),
                            },
                        );
                    }
                }
                Message::Commit { from, view: v, m } => {
                    let mut state = state.lock().unwrap();
                    state.commits.entry(from).or_insert(m.clone());
                    let count = state.commits.values().filter(|v| **v == m).count();
                    if count > 2 * f && !state.decided {
                        state.decided = true;
                        eprintln!("DECIDED on message: {}", m);
                    }
                }
            }
        } else {
            eprintln!("Received unknown message: {}", line);
        }
    }
}
