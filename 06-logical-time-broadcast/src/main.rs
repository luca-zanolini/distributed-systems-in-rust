use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

enum Message {
    Data { from: String, m: String },
}

fn encode(msg: &Message) -> String {
    match msg {
        Message::Data { from, m } => format!("DATA {from} {m}\n"),
    }
}

struct State {
    delivered: std::collections::HashSet<(String, String)>,
}

fn decode(s: &str) -> Option<Message> {
    let (kind, value) = s.trim().split_once(' ')?;

    match kind {
        "DATA" => {
            let (from, m) = value.split_once(' ')?;
            Some(Message::Data {
                from: from.to_string(),
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

    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State {
        delivered: std::collections::HashSet::new(),
    }));

    // delivered: HashSet<(String, String)>,   // (origin, m) pairs already delivered

    {
        let me = me.clone();
            let peers = peers.clone();
            thread::spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    let parts: Vec<&str> = line.split_whitespace().collect();

                    match parts.as_slice() {
                        ["bcast", m] => {
                            eprintln!("Broadcasting: {m}");
                            broadcast(
                                &peers,
                                &me,
                                &Message::Data {
                                    from: me.clone(),
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
                                &Message::Data {
                                    from: me.clone(),
                                    m: m.to_string(),
                                },
                            );
                            broadcast(
                                &peers[half..],
                                &me,
                                &Message::Data {
                                    from: me.clone(),
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
                Message::Data { from, m } => {
                    // on DATA { origin, m }:
                    let mut state = state.lock().unwrap();
                    if !state.delivered.contains(&(from.clone(), m.clone())) {
                        state.delivered.insert((from.clone(), m.clone()));
                        eprintln!("Delivered from {from}: {m}");
                        broadcast(
                            &peers,
                            &me,
                            &Message::Data {
                                from: from.clone(),
                                m: m.clone(),
                            },
                        );
                    }
                }
            }
        } else {
            eprintln!("Received unknown message: {}", line);
        }
    }
}
