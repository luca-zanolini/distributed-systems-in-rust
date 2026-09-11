use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

/// A prepare certificate: the evidence that a quorum prepared `value` in `view`.
/// `sigs` holds (signer address, hex signature) pairs — each signature is over the
/// canonical statement for (view, value). With `sigs` verified, this object is
/// transferable proof; with `sigs` empty or unchecked, it is hearsay.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Cert {
    view: u64,
    value: String,
    sigs: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum Message {
    PrePrepare {
        from: String,
        view: u64,
        m: String,
        sig: String,
    },
    Prepare {
        from: String,
        view: u64,
        m: String,
        sig: String,
    },
    Commit {
        from: String,
        view: u64,
        m: String,
        sig: String,
    },
    ViewChange {
        from: String,
        newview: u64,
        prepared: Option<Cert>,
        sig: String,
    },
}

/// The canonical, domain-separated statement an envelope signature covers.
/// For Prepare it is exactly the certificate statement: a Prepare's envelope
/// signature IS its certificate signature (self-contained, hence forwardable).
/// For ViewChange the statement covers the serialized certificate claim, so a
/// relayed/tampered claim breaks the envelope.
fn statement(msg: &Message) -> String {
    match msg {
        Message::PrePrepare { view, m, .. } => format!("PREPREPARE:{}:{}", view, m),
        Message::Prepare { view, m, .. } => prepare_statement(*view, m),
        Message::Commit { view, m, .. } => format!("COMMIT:{}:{}", view, m),
        Message::ViewChange {
            newview, prepared, ..
        } => format!(
            "VIEWCHANGE:{}:{}",
            newview,
            serde_json::to_string(prepared).expect("claim serializes")
        ),
    }
}

/// Send-side of the authentication layer: fill in our signature over the
/// canonical statement, then encode for the wire.
fn seal(msg: &Message, sk: &SigningKey) -> String {
    let signature = hex::encode(sk.sign(statement(msg).as_bytes()).to_bytes());
    let mut sealed = msg.clone();
    match &mut sealed {
        Message::PrePrepare { sig, .. }
        | Message::Prepare { sig, .. }
        | Message::Commit { sig, .. }
        | Message::ViewChange { sig, .. } => *sig = signature,
    }
    let mut s = serde_json::to_string(&sealed).expect("message serializes");
    s.push('\n');
    s
}

fn decode(s: &str) -> Option<Message> {
    serde_json::from_str(s.trim()).ok()
}

/// Receive-side of the authentication layer: true iff the envelope signature
/// verifies against the KNOWN key of the claimed sender over the canonical
/// statement. Soft-fail on every hostile shape — malformed is malicious.
fn verify_envelope(msg: &Message, pks: &HashMap<String, VerifyingKey>) -> bool {
    let (from, sig) = match msg {
        Message::PrePrepare { from, sig, .. }
        | Message::Prepare { from, sig, .. }
        | Message::Commit { from, sig, .. }
        | Message::ViewChange { from, sig, .. } => (from, sig),
    };
    let Some(pk) = pks.get(from) else {
        return false;
    };
    let Ok(bytes) = hex::decode(sig) else {
        return false;
    };
    let Ok(sig_obj) = Signature::try_from(bytes.as_slice()) else {
        return false;
    };
    pk.verify(statement(msg).as_bytes(), &sig_obj).is_ok()
}

struct State {
    view: u64,
    viewchanges: HashMap<String, (u64, Option<Cert>, String)>, // per sender: the view they want, the certificate they present, the signature
    sentcommit: bool,
    decided: Option<String>,
    preprepared: bool,
    my_prepare: Option<(u64, String)>, // the (view, value) I last signed a PREPARE for
    prepared: Option<Cert>,
    prepares: HashMap<String, (String, String)>, // per sender: (value, hex signature over the statement)
    commits: HashMap<String, String>,
    highest_vc_sent: u64, // the highest view I have complained toward
    view_entered: Instant,
}

/// keygen: write a fresh ed25519 keypair per port under keys/ (hex-encoded).
/// Distributing the public keys out of band is the trusted-setup assumption
/// PBFT makes too; keys/ is gitignored — secrets never enter Git.
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

/// Load this node's signing key and every node's verifying key from keys/.
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

fn verify_cert(cert: &Cert, pks: &HashMap<String, VerifyingKey>, n: usize, f: usize) -> bool {
    let mut seen = HashSet::new();
    let mut valid_sigs: usize = 0;
    for (from, sig) in &cert.sigs {
        if !seen.insert(from) {
            eprintln!("cert: duplicate signer {from} — not counted twice");
            continue;
        }
        let Some(pk) = pks.get(from) else {
            eprintln!("cert: unknown signer {from} — not counted");
            continue;
        };
        let Ok(bytes) = hex::decode(sig) else {
            eprintln!("cert: malformed signature from {from} — not counted");
            continue;
        };
        let Ok(sig_obj) = Signature::try_from(bytes.as_slice()) else {
            eprintln!("cert: malformed signature from {from} — not counted");
            continue;
        };
        if pk
            .verify(
                prepare_statement(cert.view, &cert.value).as_bytes(),
                &sig_obj,
            )
            .is_ok()
        {
            valid_sigs += 1;
        } else {
            eprintln!("cert: INVALID signature from {from} — not counted");
        }
    }
    2 * valid_sigs > n + f
}

fn prepare_statement(view: u64, m: &str) -> String {
    format!("PREPARE:{}:{}", view, m)
}

fn send_to(targets: &[String], message: &Message, sk: &SigningKey) {
    let line = seal(message, sk);
    for target in targets.iter().cloned() {
        let line = line.clone();
        std::thread::spawn(move || {
            if let Ok(mut stream) = TcpStream::connect(&target) {
                let _ = stream.write_all(line.as_bytes());
            }
        });
    }
}

fn leader_of(view: u64, nodes: &[String]) -> String {
    let index = (view as usize) % nodes.len();
    nodes[index].clone()
}

fn broadcast(peers: &[String], me: &str, message: &Message, sk: &SigningKey) {
    let mut targets: Vec<String> = peers.iter().cloned().collect();
    targets.push(me.to_string());
    send_to(&targets, message, sk);
}

#[derive(Serialize, Deserialize)]
struct Persistent {
    view: u64,
    my_prepare: Option<(u64, String)>,
    prepared: Option<Cert>,
    decided: Option<String>,
}

fn persist(s: &State, path: &str) {
    let p = Persistent {
        view: s.view,
        my_prepare: s.my_prepare.clone(),
        prepared: s.prepared.clone(),
        decided: s.decided.clone(),
    };
    let json = serde_json::to_string(&p).expect("state serializes");

    let mut file = std::fs::File::create(path).expect("create state file");
    file.write_all(json.as_bytes()).expect("write state");
    file.sync_all().expect("fsync state");
}

fn load(path: &str) -> Option<Persistent> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn select_value(
    viewchanges: &HashMap<String, (u64, Option<Cert>, String)>,
    nv: u64,
    pks: &HashMap<String, VerifyingKey>,
    n: usize,
    f: usize,
) -> Option<Cert> {
    let mut max_prepared: Option<Cert> = None;
    for (_from, (v, prepared, _)) in viewchanges {
        if *v == nv {
            if let Some(cert) = prepared {
                if !verify_cert(cert, &pks, n, f) {
                    eprintln!("Invalid certificate from view {}", cert.view);
                    continue;
                }
                if max_prepared.is_none() || cert.view > max_prepared.as_ref().unwrap().view {
                    max_prepared = Some(cert.clone());
                }
            }
        }
    }
    max_prepared
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

    let mut nodes: Vec<String> = std::iter::once(me.clone())
        .chain(peers.iter().cloned())
        .collect();
    nodes.sort_by_key(|a| port_of(a));

    let (sk, pks) = load_keys(&port, &nodes);

    let n = peers.len() + 1;
    let f = (n - 1) / 3;
    eprintln!("n = {n}, f = {f}");

    let state_path = format!("pbft-{port}.state");
    let disk = load(&state_path);
    if let Some(p) = &disk {
        eprintln!("RECOVERED: view={}, decided={:?}", p.view, p.decided);
    }

    let view = disk.as_ref().map_or(0, |p| p.view);
    let my_prepare = disk.as_ref().and_then(|p| p.my_prepare.clone());
    let preprepared = my_prepare.as_ref().is_some_and(|(v, _)| *v == view);

    let initial = State {
        view,
        viewchanges: HashMap::new(),
        sentcommit: false,
        decided: disk.as_ref().and_then(|p| p.decided.clone()),
        preprepared,
        my_prepare,
        prepared: disk.as_ref().and_then(|p| p.prepared.clone()),
        prepares: HashMap::new(),
        commits: HashMap::new(),
        highest_vc_sent: 0,
        view_entered: Instant::now(),
    };

    let state = Arc::new(Mutex::new(initial));

    {
        let me = me.clone();
        let peers = peers.clone();
        let state = Arc::clone(&state);
        let nodes = nodes.clone();
        let sk = sk.clone();
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
                                    sig: String::new(),
                                },
                                &sk,
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
                                sig: String::new(),
                            },
                            &sk,
                        );
                        send_to(
                            &peers[half..],
                            &Message::PrePrepare {
                                from: me.clone(),
                                view: 0,
                                m: n.to_string(),
                                sig: String::new(),
                            },
                            &sk,
                        );
                    }

                    // Byzantine INSIDER attack: broadcast a view-change claiming a
                    // prepare certificate that was never formed (garbage inner
                    // signatures) — but inside a perfectly valid, signed envelope.
                    // The envelope gate cannot catch this; only verify_cert can.
                    ["fakecert", m] => {
                        let nv = state.lock().unwrap().view + 1;
                        let fake = Cert {
                            view: nv - 1,
                            value: m.to_string(),
                            sigs: nodes.iter().map(|a| (a.clone(), "00".repeat(64))).collect(),
                        };
                        eprintln!(
                            "INSIDER: claiming forged certificate for '{m}' in my VIEWCHANGE"
                        );
                        broadcast(
                            &peers,
                            &me,
                            &Message::ViewChange {
                                from: me.clone(),
                                newview: nv,
                                prepared: Some(fake),
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
    {
        let me = me.clone();
        let peers = peers.clone();
        let state = Arc::clone(&state);
        let sk = sk.clone();
        thread::spawn(move || loop {
            let timeout = Duration::from_secs(4);
            thread::sleep(Duration::from_millis(500));
            let fire = {
                let mut s = state.lock().unwrap();
                if s.decided.is_none() && s.view_entered.elapsed() > timeout {
                    let target = s.view.max(s.highest_vc_sent) + 1;
                    s.highest_vc_sent = target;
                    s.view_entered = Instant::now();
                    Some((target, s.prepared.clone()))
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
                        prepared,
                        sig: String::new(),
                    },
                    &sk,
                );
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
            if !verify_envelope(&msg, &pks) {
                let from = match &msg {
                    Message::PrePrepare { from, .. }
                    | Message::Prepare { from, .. }
                    | Message::Commit { from, .. }
                    | Message::ViewChange { from, .. } => from,
                };
                eprintln!("DROPPED unauthenticated message claiming from {from}");
                continue;
            }
            match msg {
                Message::PrePrepare {
                    from, view: v, m, ..
                } => {
                    let mut state = state.lock().unwrap();
                    if from == leader_of(state.view, &nodes)
                        && v == state.view
                        && !state.preprepared
                    {
                        state.preprepared = true;
                        state.my_prepare = Some((v, m.clone()));
                        persist(&state, &state_path);
                        broadcast(
                            &peers,
                            &me,
                            &Message::Prepare {
                                from: me.clone(),
                                view: v,
                                m: m.clone(),
                                sig: String::new(),
                            },
                            &sk,
                        );
                    }
                }
                Message::Prepare {
                    from,
                    view: v,
                    m,
                    sig,
                } => {
                    // Envelope already verified at the gate — and for Prepare the
                    // envelope statement IS the certificate statement, so `sig` is
                    // certificate-grade evidence, safe to store in the tally.
                    let mut state = state.lock().unwrap();
                    state.prepares.entry(from).or_insert((m.clone(), sig));
                    let count = state.prepares.values().filter(|(val, _)| *val == m).count();
                    if 2 * count > n + f && !state.sentcommit && v == state.view {
                        state.sentcommit = true;
                        let cert_sigs: Vec<(String, String)> = state
                            .prepares
                            .iter()
                            .filter(|(_, (val, _))| *val == m)
                            .map(|(from, (_, sig))| (from.clone(), sig.clone()))
                            .collect();
                        state.prepared = Some(Cert {
                            view: v,
                            value: m.clone(),
                            sigs: cert_sigs,
                        });
                        persist(&state, &state_path);
                        broadcast(
                            &peers,
                            &me,
                            &Message::Commit {
                                from: me.clone(),
                                view: v,
                                m: m.clone(),
                                sig: String::new(),
                            },
                            &sk,
                        );
                    }
                }
                Message::Commit {
                    from, view: v, m, ..
                } => {
                    let mut state = state.lock().unwrap();
                    state.commits.entry(from).or_insert(m.clone());
                    let count = state.commits.values().filter(|v| **v == m).count();
                    if count > 2 * f && state.decided.is_none() && v == state.view {
                        state.decided = Some(m.clone());
                        persist(&state, &state_path);
                        eprintln!("DECIDED on message: {}", m);
                    }
                }
                Message::ViewChange {
                    from,
                    newview: nv,
                    prepared,
                    sig,
                } => {
                    let mut state = state.lock().unwrap();
                    let entry = state.viewchanges.entry(from).or_insert((
                        nv,
                        prepared.clone(),
                        sig.clone(),
                    ));
                    if nv > entry.0 {
                        *entry = (nv, prepared, sig.clone());
                    }

                    let count = state
                        .viewchanges
                        .values()
                        .filter(|(v, _, _)| *v == nv)
                        .count();
                    if count >= f + 1 && nv > state.highest_vc_sent && nv > state.view {
                        state.highest_vc_sent = nv;
                        broadcast(
                            &peers,
                            &me,
                            &Message::ViewChange {
                                from: me.clone(),
                                newview: nv,
                                prepared: state.prepared.clone(),
                                sig: String::new(),
                            },
                            &sk,
                        );
                    }
                    if count > 2 * f && nv > state.view {
                        state.view = nv;
                        state.prepares.clear();
                        state.commits.clear();
                        state.preprepared = false;
                        state.sentcommit = false;
                        state.view_entered = Instant::now();
                        persist(&state, &state_path);
                        eprintln!("ENTERED VIEW {}", nv);
                        if leader_of(nv, &nodes) == me {
                            let max_prepared = select_value(&state.viewchanges, nv, &pks, n,f);
                            if let Some(cert) = max_prepared {
                                eprintln!(
                                    "Leader {} adopting prepared value from view {}: {}",
                                    me, cert.view, cert.value
                                );
                                broadcast(
                                    &peers,
                                    &me,
                                    &Message::PrePrepare {
                                        from: me.clone(),
                                        view: nv,
                                        m: cert.value,
                                        sig: String::new(),
                                    },
                                    &sk,
                                );
                            } else {
                                eprintln!("VIEW {nv}: no prepared value — awaiting fresh proposal");
                            }
                        }
                    }
                }
            }
        } else {
            eprintln!("Received unknown message: {}", line.trim());
        }
    }
}
