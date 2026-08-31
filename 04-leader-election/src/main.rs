use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant; 


fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).cloned().unwrap_or_else(|| "5000".to_string());
    let peers: Vec<String> = args.iter().skip(2).cloned().collect();
    let me = format!("127.0.0.1:{port}");   // this node's id = its address (matches peer addrs)
    println!("node {me} — peers {peers:?}");

    // shared: when did we last hear from each peer?
    let last_heard: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

    // heartbeat + monitor thread
    {
        let me = me.clone();
        let peers = peers.clone();
        let last_heard = Arc::clone(&last_heard);
        thread::spawn(move || {
            let timeout = Duration::from_secs(3);
            let mut suspected: HashSet<String> = HashSet::new();
            loop {
                // send heartbeats
                for addr in &peers {
                    if let Ok(mut s) = TcpStream::connect(addr) { let _ = writeln!(s, "ping {me}"); }
                }
                // monitor heartbeats
                if let Ok(map) = last_heard.lock() {
                    for addr in &peers {
                        let down = match map.get(addr) {
                            Some(t) => t.elapsed() > timeout,
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
                    if let Some(sender) = line.strip_prefix("ping ") {
                        let sender = sender.trim().to_string();
                        if let Ok(mut map) = last_heard.lock() {
                            map.insert(sender, Instant::now());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

