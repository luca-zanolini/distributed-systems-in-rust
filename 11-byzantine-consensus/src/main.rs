use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::time::Instant;

fn port_of(addr: &str) -> u16 {
    addr.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(u16::MAX)
}

enum Message {
    PrePrepare {
        from: String,
        view: u64,
        m: String,
    },
    Prepare {
        from: String,
        view: u64,
        m: String,
    },
    Commit {
        from: String,
        view: u64,
        m: String,
    },
    ViewChange {
        from: String,
        newview: u64,
        prepared: Option<(u64, String)>,
    },
}

fn encode(msg: &Message) -> String {
    match msg {
        Message::PrePrepare { from, view, m } => format!("PREPREPARE {from} {view} {m}\n"),
        Message::Prepare { from, view, m } => format!("PREPARE {from} {view} {m}\n"),
        Message::Commit { from, view, m } => format!("COMMIT {from} {view} {m}\n"),
        Message::ViewChange {
            from,
            newview,
            prepared,
        } => {
            let prepared_str = if let Some((v, m)) = prepared {
                format!("{v} {m}")
            } else {
                "-".to_string()
            };
            format!("VIEWCHANGE {from} {newview} {prepared_str}\n")
        }
    }
}

struct State {
    view: u64,
    viewchanges: HashMap<String, (u64, Option<(u64, String)>)>, //(per sender: the view they want, the certificate they claim);
    sentcommit: bool,
    decided: bool,
    preprepared: bool,
    prepared: Option<(u64, String)>,
    prepares: HashMap<String, String>,
    commits: HashMap<String, String>,
    sent_viewchange: bool,
    view_entered: Instant,
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
        "VIEWCHANGE" => {
            let (from, rest) = value.split_once(' ')?;
            let (newview, prepared_str) = rest.split_once(' ')?;
            let prepared = if prepared_str == "-" {
                None
            } else {
                let (v, m) = prepared_str.split_once(' ')?;
                Some((v.parse().ok()?, m.to_string()))
            };
            Some(Message::ViewChange {
                from: from.to_string(),
                newview: newview.parse().ok()?,
                prepared,
            })
        }
        _ => None,
    }
}

fn send_to(targets: &[String], message: &Message) {
    let msg = encode(message);
    for target in targets.iter().cloned() {
        let msg = msg.clone();
        std::thread::spawn(move || {
            if let Ok(mut stream) = TcpStream::connect(&target) {
                let _ = stream.write_all(msg.as_bytes());
            }
        });
    }
}

fn leader_of(view: u64, nodes: &[String]) -> String {
    let index = (view as usize) % nodes.len();
    nodes[index].clone()
}

fn broadcast(peers: &[String], me: &str, message: &Message) {
    let mut targets: Vec<String> = peers.iter().cloned().collect();
    targets.push(me.to_string());
    send_to(&targets, message);
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

    let mut nodes: Vec<String> = std::iter::once(me.clone())
        .chain(peers.iter().cloned())
        .collect();
    nodes.sort_by_key(|a| port_of(a));

    let state: Arc<Mutex<State>> = Arc::new(Mutex::new(State {
        view: 0,
        viewchanges: HashMap::new(),
        sentcommit: false,
        decided: false,
        preprepared: false,
        prepared: None,
        prepares: HashMap::new(),
        commits: HashMap::new(),
        sent_viewchange: false,
        view_entered: Instant::now(),
    }));

    let n = peers.len() + 1;
    let f = (n - 1) / 3;
    eprintln!("n = {n}, f = {f}");

    {
        let me = me.clone();
        let peers = peers.clone();
        let state = Arc::clone(&state);
        let nodes = nodes.clone();
        thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                let parts: Vec<&str> = line.split_whitespace().collect();

                match parts.as_slice() {
                    ["propose", m] => {
                        let (view, i_lead) = {
                            let s = state.lock().unwrap();
                            (s.view, leader_of(s.view, &nodes) == me)
                        };
                        if i_lead {
                            eprintln!("Proposing: {m}");
                            broadcast(
                                &peers,
                                &me,
                                &Message::PrePrepare {
                                    from: me.clone(),
                                    view,
                                    m: m.to_string(),
                                },
                            );
                        } else {
                            eprintln!("Not the leader, cannot propose: {m}");
                        }
                    }

                    ["bcast", "equiv", m, n] => {
                        eprintln!("Equivocating: {m} / {n}");
                        let half = peers.len() / 2;
                        send_to(
                            &peers[..half],
                            &Message::PrePrepare {
                                from: me.clone(),
                                view: 0,
                                m: m.to_string(),
                            },
                        );
                        send_to(
                            &peers[half..],
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
    {
        let me = me.clone();
        let peers = peers.clone();
        let state = Arc::clone(&state);
        thread::spawn(move || {
            loop {
                let timeout = Duration::from_secs(4);
                thread::sleep(Duration::from_millis(500));
                let fire = {
                    let mut s = state.lock().unwrap();
                    if !s.decided && !s.sent_viewchange && s.view_entered.elapsed() > timeout {
                        s.sent_viewchange = true;
                        Some((s.view + 1, s.prepared.clone()))
                    } else {
                        None
                    }
                };
                if let Some((nv, prepared)) = fire {
                    broadcast(
                        &peers,
                        &me,
                        &Message::ViewChange {
                            from: me.clone(),
                            newview: nv,
                            prepared: prepared,
                        },
                    );
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
                Message::PrePrepare { from, view: v, m } => {
                    let mut state = state.lock().unwrap();
                    if from == leader_of(state.view, &nodes)
                        && v == state.view
                        && !state.preprepared
                    {
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
                    if 2 * count > n + f && !state.sentcommit && v == state.view {
                        state.sentcommit = true;
                        state.prepared = Some((v, m.clone()));
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
                    if count > 2 * f && !state.decided && v == state.view {
                        state.decided = true;
                        eprintln!("DECIDED on message: {}", m);
                    }
                }
                Message::ViewChange {
                    from,
                    newview: nv,
                    prepared,
                } => {
                    let mut state = state.lock().unwrap();
                    let entry = state
                        .viewchanges
                        .entry(from)
                        .or_insert((nv, prepared.clone()));
                    if nv > entry.0 {
                        *entry = (nv, prepared);
                    }

                    let count = state.viewchanges.values().filter(|(v, _)| *v == nv).count();
                    if count >= f + 1 && !state.sent_viewchange && nv > state.view {
                        state.sent_viewchange = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::ViewChange {
                                from: me.clone(),
                                newview: nv,
                                prepared: state.prepared.clone(),
                            },
                        );
                    }
                    if count > 2 * f && nv > state.view {
                        state.view = nv;
                        state.sent_viewchange = false;
                        state.prepares.clear();
                        state.commits.clear();
                        state.preprepared = false;
                        state.sentcommit = false;
                        state.view_entered = Instant::now();
                        eprintln!("ENTERED VIEW {}", nv);
                        if leader_of(nv, &nodes) == me {
                            let mut max_prepared: Option<(u64, String)> = None;
                            for (_from, (v, prepared)) in &state.viewchanges {
                                if *v == nv {
                                    if let Some((pv, val)) = prepared {
                                        if max_prepared.is_none()
                                            || pv > &max_prepared.as_ref().unwrap().0
                                        {
                                            max_prepared = Some((*pv, val.clone()));
                                        }
                                    }
                                }
                            }
                            if let Some((pv, val)) = max_prepared {
                                eprintln!(
                                    "Leader {} adopting prepared value from view {}: {}",
                                    me, pv, val
                                );
                                broadcast(
                                    &peers,
                                    &me,
                                    &Message::PrePrepare {
                                        from: me.clone(),
                                        view: nv,
                                        m: val,
                                    },
                                );
                            } else {
                                eprintln!("VIEW {nv}: no prepared value — awaiting fresh proposal");
                            }
                        }
                    }
                }
            }
        } else {
            eprintln!("Received unknown message: {}", line);
        }
    }
}
