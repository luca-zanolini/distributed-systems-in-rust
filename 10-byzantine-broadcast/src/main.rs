use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
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
    Send { from: String, m: String, sig: String },
    Echo { from: String, m: String, sig: String },
    Ready { from: String, m: String, sig: String },
}

/// keygen: a fresh ed25519 keypair per port under keys/ (hex-encoded).
/// The out-of-band distribution of public keys is the trusted-setup assumption:
/// the ≤ f bound is only meaningful over an unforgeable identity space.
fn keygen(ports: &[String]) {
    std::fs::create_dir_all("keys").expect("create keys/");
    for p in ports {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).expect("OS randomness");
        let sk = SigningKey::from_bytes(&seed);
        std::fs::write(format!("keys/{p}.sk"), hex::encode(sk.to_bytes())).unwrap();
        std::fs::write(
            format!("keys/{p}.pk"),
            hex::encode(sk.verifying_key().to_bytes()),
        )
        .unwrap();
        eprintln!("wrote keys/{p}.sk and keys/{p}.pk");
    }
}

fn load_keys(port: &str, nodes: &[String]) -> (SigningKey, HashMap<String, VerifyingKey>) {
    let read = |path: String| -> [u8; 32] {
        let hex_str = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{path} missing — run: cargo run -- keygen <ports...>"));
        hex::decode(hex_str.trim())
            .expect("valid hex")
            .try_into()
            .expect("32 bytes")
    };
    let sk = SigningKey::from_bytes(&read(format!("keys/{port}.sk")));
    let mut pks = HashMap::new();
    for addr in nodes {
        let p = port_of(addr);
        let pk = VerifyingKey::from_bytes(&read(format!("keys/{p}.pk"))).expect("valid pubkey");
        pks.insert(addr.clone(), pk);
    }
    (sk, pks)
}

fn prepare_statement(msg: &Message) -> String {
    match msg {
        Message::Send { m, .. } => format!("SEND:{}", m),
        Message::Echo { m, .. } => format!("ECHO:{}", m),
        Message::Ready { m, .. } => format!("READY:{}", m),
    }
}

/// The send-side of the authentication layer: stamp the message with our
/// signature over its canonical statement, then encode for the wire.
/// Wire format: TYPE from sighex m
fn seal(msg: &Message, sk: &SigningKey) -> String {
    let statement = prepare_statement(msg);
    let sig = hex::encode(sk.sign(statement.as_bytes()).to_bytes());
    match msg {
        Message::Send { from, m, .. } => format!("SEND {from} {sig} {m}\n"),
        Message::Echo { from, m, .. } => format!("ECHO {from} {sig} {m}\n"),
        Message::Ready { from, m, .. } => format!("READY {from} {sig} {m}\n"),
    }
}

fn decode(s: &str) -> Option<Message> {
    let mut parts = s.trim().splitn(4, ' ');
    let kind = parts.next()?;
    let from = parts.next()?.to_string();
    let sig = parts.next()?.to_string();
    let m = parts.next()?.to_string();
    match kind {
        "SEND" => Some(Message::Send { from, m, sig }),
        "ECHO" => Some(Message::Echo { from, m, sig }),
        "READY" => Some(Message::Ready { from, m, sig }),
        _ => None,
    }
}


fn verify_envelope(msg: &Message, pks: &HashMap<String, VerifyingKey>) -> bool {
    let from = match msg {
        Message::Send { from, .. } => from,
        Message::Echo { from, .. } => from,
        Message::Ready { from, .. } => from,
    };
    let pk = match pks.get(from) {
        Some(pk) => pk,
        None => return false,
    };
    let statement = prepare_statement(msg);
    let sig = match msg {
        Message::Send { sig, .. } => sig,
        Message::Echo { sig, .. } => sig,
        Message::Ready { sig, .. } => sig,
    };
    let sig_bytes = match hex::decode(sig) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let sig = match Signature::try_from(sig_bytes.as_slice()) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    pk.verify(statement.as_bytes(), &sig).is_ok()
}


struct State {
    sentecho: bool,
    sentready: bool,
    delivered: bool,
    echos: HashMap<String, String>,
    readys: HashMap<String, String>,
}

fn broadcast(peers: &[String], me: &str, message: &Message, sk: &SigningKey) {
    let line = seal(message, sk);
    for peer in peers.iter().cloned().chain(std::iter::once(me.to_string())) {
        let line = line.clone();
        std::thread::spawn(move || {
            if let Ok(mut stream) = TcpStream::connect(&peer) {
                let _ = stream.write_all(line.as_bytes());
            }
        });
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.get(1).map(String::as_str) == Some("keygen") {
        keygen(&args[2..]);
        return;
    }

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

    let nodes: Vec<String> = std::iter::once(me.clone())
        .chain(peers.iter().cloned())
        .collect();
    let (sk, pks) = load_keys(&port, &nodes);

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
            let sk = sk.clone();
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
                                    sig: String::new(),
                                },
                                &sk,
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
                                    sig: String::new(),
                                },
                                &sk,
                            );
                            broadcast(
                                &peers[half..],
                                &me,
                                &Message::Send {
                                    from: me.clone(),
                                    m: n.to_string(),
                                    sig: String::new(),
                                },
                                &sk,
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
            if !verify_envelope(&msg, &pks) {
                eprintln!("DROPPED unauthenticated message claiming from {}", match &msg {
                    Message::Send { from, .. } => from,
                    Message::Echo { from, .. } => from,
                    Message::Ready { from, .. } => from,
                });
                continue;
            }
            match msg {
                Message::Send { from, m, .. } => {
                    let mut state = state.lock().unwrap();
                    if sender.as_deref() == Some(from.as_str()) && !state.sentecho {
                        state.sentecho = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Echo {
                                from: me.clone(),
                                m: m.clone(),
                                sig: String::new(),
                            },
                            &sk,
                        );
                    }
                }
                Message::Echo { from, m, .. } => {
                    let mut state = state.lock().unwrap();
                    state.echos.entry(from).or_insert(m.clone());
                    let count = state.echos.values().filter(|v| **v == m).count();
                    if 2 * count > n + f && !state.sentready {
                        state.sentready = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Ready {
                                from: me.clone(),
                                m: m.clone(),
                                sig: String::new(),
                            },
                            &sk,
                        );
                    }
                }
                Message::Ready { from, m, .. } => {
                    let mut state = state.lock().unwrap();
                    state.readys.entry(from).or_insert(m.clone());
                    let count = state.readys.values().filter(|v| **v == m).count();
                    if count > f && !state.sentready {
                        state.sentready = true;
                        broadcast(
                            &peers,
                            &me,
                            &Message::Ready {
                                from: me.clone(),
                                m: m.clone(),
                                sig: String::new(),
                            },
                            &sk,
                        );
                    }
                    if count > 2 * f && !state.delivered {
                        state.delivered = true;
                        eprintln!("Delivered message: {}", m);
                    }
                }
            }
        } else {
            eprintln!("Received unknown message: {}", line.trim());
        }
    }
}
