use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

enum Message {
    Send { from: String, m: String },
    Echo { from: String, m: String },
    Ready { from: String, m: String },
}

fn encode(msg: &Message) -> String {
    match msg {
        Message::Send { from, m } => format!("SEND {from} {m}\n"),
        Message::Echo { from, m } => format!("ECHO {from} {m}\n"),
        Message::Ready { from, m } => format!("READY {from} {m}\n"),
    }
}

struct State {
    sentecho: bool,
    sentready: bool,
    delivered: bool,
    echos: HashMap<String, String>,
    readys: HashMap<String, String>,
}

fn decode(s: &str) -> Option<Message> {
    let (kind, value) = s.trim().split_once(' ')?;

    match kind {
        "SEND" => {
            let (from, m) = value.split_once(' ')?;
            Some(Message::Send {
                from: from.to_string(),
                m: m.to_string(),
            })
        }
        "ECHO" => {
            let (from, m) = value.split_once(' ')?;
            Some(Message::Echo {
                from: from.to_string(),
                m: m.to_string(),
            })
        }
        "READY" => {
            let (from, m) = value.split_once(' ')?;
            Some(Message::Ready {
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
    let sender: Option<String> = args
        .iter()
        .position(|arg| arg == "--sender")
        .and_then(|i| args.get(i + 1).cloned());

    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State {
        sentecho: false,
        sentready: false,
        delivered: false,
        echos: HashMap::new(),
        readys: HashMap::new(),
    }));

    let n = peers.len() + 1;
    let f = (n - 1) / 3;
    eprintln!("n = {n}, f = {f}");

    {
        let me = me.clone();
        if sender.as_deref() == Some(me.as_str()) {
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
                                &Message::Send {
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
                                &Message::Send {
                                    from: me.clone(),
                                    m: m.to_string(),
                                },
                            );
                            broadcast(
                                &peers[half..],
                                &me,
                                &Message::Send {
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
                Message::Send { from, m } => {
                    let mut state = state.lock().unwrap();
                    if sender.as_deref() == Some(from.as_str()) && !state.sentecho {
                        state.sentecho = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Echo {
                                from: me.clone(),
                                m: m.clone(),
                            },
                        );
                    }
                }
                Message::Echo { from, m } => {
                    let mut state = state.lock().unwrap();
                    state.echos.entry(from).or_insert(m.clone());
                    let count = state.echos.values().filter(|v| **v == m).count();
                    if count > (n + f) / 2 && !state.sentready {
                        state.sentready = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Ready {
                                from: me.clone(),
                                m: m.clone(),
                            },
                        );
                    }
                }
                Message::Ready { from, m } => {
                    let mut state = state.lock().unwrap();
                    state.readys.entry(from).or_insert(m.clone());
                    let count = state.readys.values().filter(|v| **v == m).count();
                    if count >= f + 1 && !state.sentready {
                        state.sentready = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Ready {
                                from: me.clone(),
                                m: m.clone(),
                            },
                        );
                    }
                    if count > (n + f) / 2 && !state.delivered {
                        state.delivered = true;
                        eprintln!("Delivered message: {}", m);
                    }
                }
            }
        } else {
            eprintln!("Received unknown message: {}", line);
        }
    }
}
