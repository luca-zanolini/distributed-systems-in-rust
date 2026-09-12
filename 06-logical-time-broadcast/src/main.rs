use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

enum Message {
    Data {
        from: String,
        w: Vec<u64>,
        m: String,
    },
}

fn rank_of(addr: &str, all: &[String]) -> usize {
    all.iter().position(|a| a == addr).unwrap()
}


fn encode(msg: &Message) -> String {
    match msg {
        Message::Data { from, w, m } => {
            let w_str = w
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(",");
            format!("DATA {from} {w_str} {m}\n")
        }
    }
}

struct State {
    v: Vec<u64>,                              // v[rank(q)] = # delivered from q
    lsn: u64,                                 // # of my own broadcasts
    pending: Vec<(String, Vec<u64>, String)>, // (origin, W, m) awaiting causal past
    delivered: HashSet<(String, u64)>,        // RB dedup: (origin, per-origin seq)
}

fn decode(s: &str) -> Option<Message> {
    let (kind, rest) = s.trim().split_once(' ')?; // "DATA" | "from w_str m"
    if kind != "DATA" {
        return None;
    }
    let (from, rest) = rest.split_once(' ')?; // from | "w_str m"
    let (w_str, m) = rest.split_once(' ')?; // w_str | m

    let w = w_str
        .split(',') // "2,0,1" → "2", "0", "1"
        .map(|x| x.parse::<u64>().ok()) // each → Some(2) / None if garbage
        .collect::<Option<Vec<u64>>>()?;

    Some(Message::Data {
        from: from.to_string(),
        w,
        m: m.to_string(),
    })
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

    // membership, sorted identically everywhere → consistent ranks
    let mut all = peers.clone();
    all.push(me.clone());
    all.sort();
    let n = all.len();
    let my_rank = rank_of(&me, &all);

    // --slow-from <addr> <ms> : delay processing of messages ORIGINATING at addr (demo knob)
    let slow_from: Option<(String, u64)> = args
        .iter()
        .position(|a| a == "--slow-from")
        .and_then(|i| Some((args.get(i + 1)?.clone(), args.get(i + 2)?.parse().ok()?)));

    let state = Arc::new(Mutex::new(State {
        v: vec![0; n], // n zeros: delivered nothing from anyone
        lsn: 0,
        pending: Vec::new(),
        delivered: HashSet::new(),
    }));

    // delivered: HashSet<(String, u64)>,      // (origin, seq) pairs already delivered

    {
        let me = me.clone();
        let peers = peers.clone();
        let state = state.clone();
        thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                let parts: Vec<&str> = line.split_whitespace().collect();

                match parts.as_slice() {
                    ["bcast", m] => {
                        let mut state = state.lock().unwrap();
                        let mut w = state.v.clone(); // my causal past: what I've delivered from everyone
                        w[my_rank] = state.lsn; // ...plus which of MY messages precede this one
                        state.lsn += 1; // (stamp FIRST, then count)
                        eprintln!("Broadcasting '{m}' with W={w:?}");
                        broadcast(
                            &peers,
                            &me,
                            &Message::Data {
                                from: me.clone(),
                                w,
                                m: m.to_string(),
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

        // one clone of each shared thing PER connection-thread
        let state = state.clone();
        let peers = peers.clone();
        let me = me.clone();
        let all = all.clone();
        let slow_from = slow_from.clone();

        thread::spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                return;
            }
            let Some(Message::Data { from, w, m }) = decode(&line) else {
                eprintln!("Received unknown message: {}", line.trim());
                return;
            };

            // demo knob: simulate a slow link from one origin — sleep BEFORE taking the lock
            if let Some((slow_addr, ms)) = &slow_from {
                if from == *slow_addr {
                    thread::sleep(std::time::Duration::from_millis(*ms));
                }
            }

            let mut state = state.lock().unwrap();

            // ── RB layer: dedup, relay ──
            // Dedup key = (origin, per-origin sequence number). The seq already
            // rides in the stamp: W[rank(origin)] is the sender's lsn for THIS
            // message. Keying on (origin, payload) instead would silently drop a
            // repeated payload while the sender's lsn advanced — permanently
            // wedging that origin's entire causal stream at every node.
            let Some(origin_rank) = all.iter().position(|a| a == &from) else {
                eprintln!("dropping message from unknown origin {from}");
                return;
            };
            if w.len() != all.len() {
                eprintln!("dropping malformed stamp from {from} (len {})", w.len());
                return;
            }
            if !state.delivered.insert((from.clone(), w[origin_rank])) {
                return; // seen it: drop
            }
            broadcast(
                &peers,
                &me,
                &Message::Data {
                    from: from.clone(),
                    w: w.clone(),
                    m: m.clone(),
                },
            );

            let ready = w.iter().zip(state.v.iter()).all(|(a, b)| a <= b);
            if !ready {
                eprintln!(
                    "Holding '{m}' from {from}: causal past not yet delivered (W={w:?}, V={:?})",
                    state.v
                );
            }
            state.pending.push((from, w, m));

            while let Some(i) = state
                .pending
                .iter()
                .position(|(_, w, _)| w.iter().zip(state.v.iter()).all(|(a, b)| a <= b))
            {
                let (origin, _, m) = state.pending.remove(i);
                let r = rank_of(&origin, &all);
                state.v[r] += 1;
                eprintln!("Delivered from {origin}: {m}  [V={:?}]", state.v);
            }
        });
    }
}
