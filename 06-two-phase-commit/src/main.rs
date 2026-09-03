use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

fn send(addr: &str, msg: &str) -> Option<String> {
    let mut stream = TcpStream::connect(addr).ok()?;
    writeln!(stream, "{msg}").ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    Some(line.trim().to_string())
}

// Write the durable state (committed balance + any in-doubt reservation) to disk and fsync it,
// so it survives a crash. Called BEFORE replying to anything that changes durable state.
fn persist(balance: i64, prepared: &Option<(u64, i64)>, path: &str) {
    let p = match prepared {
        Some((txid, delta)) => format!("{txid} {delta}"),
        None => "-".to_string(),
    };
    let data = format!("balance {balance}\nprepared {p}\n");
    if let Ok(mut f) = std::fs::File::create(path) {
        let _ = f.write_all(data.as_bytes());
        let _ = f.sync_all(); // fsync: force to the platter before we return
    }
}

// Reload durable state on startup. None => no file yet (fresh node).
fn load(path: &str) -> Option<(i64, Option<(u64, i64)>)> {
    let data = std::fs::read_to_string(path).ok()?;
    let (mut balance, mut prepared) = (100i64, None);
    for line in data.lines() {
        if let Some(v) = line.strip_prefix("balance ") {
            balance = v.parse().unwrap_or(100);
        } else if let Some(v) = line.strip_prefix("prepared ") {
            if v != "-" {
                let mut it = v.split_whitespace();
                if let (Some(Ok(t)), Some(Ok(d))) =
                    (it.next().map(str::parse), it.next().map(str::parse))
                {
                    prepared = Some((t, d));
                }
            }
        }
    }
    Some((balance, prepared))
}

fn run_participant(port: String) {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
    let state_path = format!("2pc-{port}.state");
    let (mut balance, mut prepared) = load(&state_path).unwrap_or((100, None));
    match &prepared {
        Some((txid, delta)) => println!(
            "participant on {port}, balance {balance} — RECOVERED IN-DOUBT on tx {txid} (delta {delta}), awaiting verdict"
        ),
        None => println!("participant on {port}, balance {balance}"),
    }

    for conn in listener.incoming() {
        let mut stream = match conn {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            continue;
        }

        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        let reply = match parts.as_slice() {
            ["PREPARE", txid, delta] => {
                let txid: u64 = txid.parse().unwrap_or(0);
                let delta: i64 = delta.parse().unwrap_or(0);
                let vote = if balance + delta >= 0 && prepared.is_none() {
                    prepared = Some((txid, delta));
                    persist(balance, &prepared, &state_path); // fsync the promise BEFORE we reply YES
                    "YES"
                } else {
                    "NO"
                };
                format!("VOTE {txid} {vote}")
            }
            ["COMMIT", txid] => {
                let txid: u64 = txid.parse().unwrap_or(0);
                if let Some((ptxid, delta)) = prepared {
                    if ptxid == txid {
                        balance += delta;
                        prepared = None;
                        persist(balance, &prepared, &state_path); // ← add this
                    }
                }
                format!("ACK {txid}")
            }
            ["ABORT", txid] => {
                let txid: u64 = txid.parse().unwrap_or(0);
                if let Some((ptxid, _)) = prepared {
                    if ptxid == txid {
                        prepared = None;
                        persist(balance, &prepared, &state_path); // ← add this
                    }
                }
                format!("ACK {txid}")
            }
            _ => format!("ERR unknown {}", line.trim()),
        };
        let _ = writeln!(stream, "{reply}");
        println!(
            "  [{port}] {} -> {reply}  (balance {balance}, prepared {prepared:?})",
            line.trim()
        );
    }
}

fn run_coordinator(participants: Vec<String>) {
    let stdin = std::io::stdin();
    let mut txid: u64 = 0;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        match parts.as_slice() {
            ["transfer", deltas @ ..] => {
                txid += 1;
                let deltas: Vec<i64> = deltas.iter().map(|d| d.parse().unwrap_or(0)).collect();
                let mut all_yes = true;
                for (i, participant) in participants.iter().enumerate() {
                    let delta = deltas.get(i).cloned().unwrap_or(0);
                    let reply = send(participant, &format!("PREPARE {txid} {delta}"));
                    if reply.as_deref() != Some(&format!("VOTE {txid} YES")) {
                        all_yes = false;
                    }
                }
                let decision = if all_yes { "COMMIT" } else { "ABORT" };
                for participant in &participants {
                    let _ = send(participant, &format!("{decision} {txid}"));
                }

                println!("tx {txid}: {decision}");
            }
            ["transfer-crash", deltas @ ..] => {
                txid += 1;
                let deltas: Vec<i64> = deltas.iter().map(|d| d.parse().unwrap_or(0)).collect();
                // PHASE 1 only: send PREPARE to everyone, collect votes (same as transfer)...
                for (i, participant) in participants.iter().enumerate() {
                    let delta = deltas.get(i).cloned().unwrap_or(0);
                    let _ = send(participant, &format!("PREPARE {txid} {delta}"));
                }
                // ...then CRASH — never send COMMIT/ABORT. Participants are now stranded in-doubt.
                println!("tx {txid}: coordinator CRASHED after PREPARE — no verdict sent!");
            }

            _ => eprintln!("usage: transfer <delta> <delta> ..."),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("participant") => {
            let port = args.get(2).cloned().unwrap_or_else(|| "6000".to_string());
            run_participant(port);
        }
        Some("coordinator") => {
            let participants: Vec<String> = args.iter().skip(2).cloned().collect();
            println!(
                "coordinator driving {} participants: {:?}",
                participants.len(),
                participants
            );
            run_coordinator(participants);
        }
        _ => {
            eprintln!("usage: two-phase-commit participant <port>");
            eprintln!("two-phase-commit coordinator <addr> <addr> ...");
        }
    }
}
