use std::collections::HashMap; // the map that backs the store
use std::io::Write; // brings write_all() into scope
use std::io::{BufRead, BufReader}; // buffered reader + its .lines() method
use std::net::TcpListener; // the server socket (accepts connections)
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

struct Store {
    map: HashMap<String, (u64, String)>, // key -> (timestamp, value)
}

impl Store {
    fn new() -> Self {
        Store {
            map: HashMap::new(),
        }
    }

    // The timestamp to stamp the NEXT write to `key` with: current + 1, or 1 if new.
    fn next_ts(&self, key: &str) -> u64 {
        match self.map.get(key) {
            Some((ts, _)) => ts + 1,
            None => 1,
        }
    }

    // Store a versioned value. Timestamp chosen by the CALLER (the primary).
    fn write(&mut self, key: String, ts: u64, value: String) {
        self.map.insert(key, (ts, value));
    }

    // Read the versioned value, OWNED (cloned) so a read-quorum can freely collect
    // results from several sources without borrow-lifetime fights.
    fn read(&self, key: &str) -> Option<(u64, String)> {
        self.map.get(key).cloned()
    }

    fn remove(&mut self, key: &str) -> Option<(u64, String)> {
        self.map.remove(key)
    }
}

fn forward(addr: &str, line: &str) -> std::io::Result<String> {
    let mut b = TcpStream::connect(addr)?;
    b.write_all(line.as_bytes())?;
    b.write_all(b"\n")?;
    let mut ack = String::new();
    BufReader::new(&b).read_line(&mut ack)?; // ← wait for the backup to confirm
    Ok(ack)
}

// Ask a replica for its versioned value of `key`: Ok(Some((ts,value))) if it has it,
// Ok(None) if absent, Err if unreachable.
fn read_from(addr: &str, key: &str) -> std::io::Result<Option<(u64, String)>> {
    let mut conn = TcpStream::connect(addr)?;
    conn.write_all(format!("readts {key}\n").as_bytes())?;
    let mut reply = String::new();
    BufReader::new(&conn).read_line(&mut reply)?;
    let reply = reply.trim();
    if reply == "none" {
        return Ok(None);
    }
    let mut parts = reply.splitn(2, ' ');
    match (parts.next(), parts.next()) {
        (Some(ts), Some(value)) => Ok(Some((ts.parse().unwrap_or(0), value.to_string()))),
        _ => Ok(None),
    }
}

fn handle_client(
    store: Arc<Mutex<Store>>,
    stream: TcpStream,
    backups: Vec<String>,
) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?; // 2nd handle to the SAME socket, for writing
    let reader = BufReader::new(stream); // wrap the socket to read it line by line
    for line in reader.lines() {
        // one iteration per COMMAND line from this client
        let line = line?; // the line (newline stripped), or an I/O error
        let parts: Vec<&str> = line.split_whitespace().collect(); // split into words
        // What to forward to backups if this command is a write — carries the timestamp
        // so every replica stores the SAME (ts, value). None for reads.
        let mut to_replicate: Option<String> = None;
        let mut response = match parts.as_slice() {
            ["set", key, rest @ ..] => {
                let value = rest.join(" ");
                // Pick the timestamp and write UNDER ONE LOCK — read-then-write must be
                // atomic, or two concurrent sets could grab the same ts.
                let ts = {
                    let mut guard = store.lock().unwrap();
                    let ts = guard.next_ts(key);
                    guard.write(key.to_string(), ts, value.clone());
                    ts
                };
                to_replicate = Some(format!("repl {ts} {key} {value}")); // replicate WITH the ts
                "OK\n".to_string()
            }
            // INTERNAL verb: a versioned write forwarded by the primary (also the line
            // format `dump` emits). A replica applies it and does NOT re-replicate.
            ["repl", ts, key, rest @ ..] => {
                let ts: u64 = ts.parse().unwrap_or(0);
                store
                    .lock()
                    .unwrap()
                    .write(key.to_string(), ts, rest.join(" "));
                "OK\n".to_string()
            }
            ["get", key] => {
                let mut versions: Vec<(u64, String)> = Vec::new();
                let mut responses = 0;

                // 1) our own copy
                responses += 1;
                if let Some(v) = store.lock().unwrap().read(key) {
                    versions.push(v);
                }

                for addr in &backups {
                    match read_from(addr, key) {
                        Ok(reply) => {
                            responses += 1;
                            if let Some(v) = reply {
                                versions.push(v);
                            }
                        }
                        Err(e) => eprintln!("read from {addr} failed: {e}"),
                    }
                }
                // 3) quorum check + pick the freshest
                let total = backups.len() + 1;
                let quorum = total / 2 + 1;
                if responses < quorum {
                    "ERR no read quorum\n".to_string()
                } else {
                    if let Some((_ts, value)) = versions.into_iter().max_by_key(|(ts, _)| *ts) {
                        format!("{value}\n")
                    } else {
                        "Key not found\n".to_string()
                    }
                }
            }

            ["dump"] => {
                let guard = store.lock().unwrap();
                let mut out = String::new();
                for (key, (ts, value)) in &guard.map {
                    out.push_str(&format!("repl {ts} {key} {value}\n"));
                }
                out.push_str("END\n");
                out // tail expression → this arm's value
            }
            ["remove", key] => {
                // Best-effort: remove is NOT yet versioned (tombstones deferred), so a
                // stale replica could resurrect the key under a quorum read. A follow-up.
                to_replicate = Some(format!("remove {key}"));
                match store.lock().unwrap().remove(key) {
                    Some(_) => "OK\n".to_string(),
                    None => "Key not found\n".to_string(),
                }
            }
            ["readts", key] => {
                let guard = store.lock().unwrap();
                match guard.read(key) {
                    Some((ts, value)) => format!("{ts} {value}\n"),
                    None => "none\n".to_string(),
                }
            }
            _ => "unknown command\n".to_string(),
        };

        // A write (set/remove) set `to_replicate`; forward it and require a quorum of acks.
        if let Some(repl_line) = &to_replicate {
            let mut acks = 1; // the primary already has it → counts as 1
            for addr in &backups {
                match forward(addr, repl_line) {
                    Ok(_) => acks += 1,
                    Err(e) => eprintln!("replication to {addr} failed: {e}"),
                }
            }
            let total = backups.len() + 1; // primary + all backups
            let quorum = total / 2 + 1; // a majority
            if acks < quorum {
                response = format!("ERR no quorum ({acks}/{total}, need {quorum})\n");
            }
        }
        writer.write_all(response.as_bytes())?; // send the response back over the socket
    } // (inner loop ends when the client disconnects = EOF)
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).map(String::as_str).unwrap_or("4000");
    let mut backups: Vec<String> = Vec::new();
    let mut catch_up: Option<String> = None;
    let mut rest = args.iter().skip(2);
    while let Some(arg) = rest.next() {
        if arg.as_str() == "--catch-up" {
            catch_up = rest.next().cloned(); // consume the NEXT arg as the primary's address
        } else {
            backups.push(arg.clone());
        }
    }

    if backups.is_empty() {
        println!("node on :{port} — standalone / backup");
    } else {
        println!("PRIMARY on :{port} — forwarding writes to {backups:?}");
    }

    // Result return so we can use ?
    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))?; // claim the port, start listening

    let store = Arc::new(Mutex::new(Store::new())); // shared, lockable store
    // Catch-up (anti-entropy): a recovering node pulls a full snapshot before it serves.
    if let Some(primary) = &catch_up {
        println!("catching up from {primary} ...");
        let mut conn = TcpStream::connect(primary)?; // open a connection (just like forward())
        conn.write_all(b"dump\n")?; // ask for the snapshot
        let reader = BufReader::new(&conn); // read the reply line by line
        for line in reader.lines() {
            let line = line?;
            if line == "END" {
                break;
            } // sentinel → snapshot complete
            // Each snapshot line is "repl <ts> <key> <value>" — apply it as a versioned write.
            if let Some(stripped) = line.strip_prefix("repl ") {
                let mut parts = stripped.splitn(3, ' ');
                if let (Some(ts), Some(key), Some(value)) =
                    (parts.next(), parts.next(), parts.next())
                {
                    let ts: u64 = ts.parse().unwrap_or(0);
                    store
                        .lock()
                        .unwrap()
                        .write(key.to_string(), ts, value.to_string());
                }
            }
        }
        println!("caught up.");
    }
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };
        let store = Arc::clone(&store); // a handle for THIS client's thread
        let backups = backups.clone();
        thread::spawn(move || {
            // serve it concurrently
            if let Err(e) = handle_client(store, stream, backups) {
                eprintln!("client error: {e}");
            }
        });
    }
    Ok(()) // unreachable in practice: the accept loop runs forever
}

#[test]
fn next_ts_starts_at_one_then_increments() {
    let mut store = Store::new();
    assert_eq!(store.next_ts("k"), 1); // absent → 1
    store.write("k".to_string(), 1, "v".to_string());
    assert_eq!(store.next_ts("k"), 2); // present at ts 1 → 2
}
#[test]
fn write_and_read() {
    let mut store = Store::new();
    store.write("k".to_string(), 1, "v".to_string());
    assert_eq!(store.read("k"), Some((1, "v".to_string())));
}
#[test]
fn missing_key_is_none() {
    let store = Store::new();
    assert_eq!(store.read("missing"), None);
}
