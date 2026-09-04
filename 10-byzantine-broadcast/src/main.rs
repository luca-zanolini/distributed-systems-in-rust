use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

enum Message {
    Send(String),
    Echo(String),
    Ready(String),
}

fn encode(msg: &Message) -> String {
    match msg {
        Message::Send(m) => format!("SEND {m}\n"),
        Message::Echo(m) => format!("ECHO {m}\n"),
        Message::Ready(m) => format!("READY {m}\n"),
    }
}

fn decode(s: &str) -> Option<Message> {
    let (kind, value) = s.trim().split_once(' ')?;

    match kind {
        "SEND" => Some(Message::Send(value.to_string())),
        "ECHO" => Some(Message::Echo(value.to_string())),
        "READY" => Some(Message::Ready(value.to_string())),
        _ => None,
    }
}

fn broadcast(peers: &[String], message: &Message) {
    let msg = encode(message);
    for peer in peers.iter().cloned() {
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

    let n = peers.len() + 1; let f = (n - 1) / 3;
    println!("n = {n}, f = {f}");

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
                            println!("Broadcasting: {m}");
                            broadcast(&peers, &Message::Send(m.to_string()));
                        }

                        ["bcast", "equiv", m, n] => {
                            println!("Equivocating: {m} / {n}");
                            let half = peers.len() / 2;
                            broadcast(&peers[..half], &Message::Send(m.to_string()));
                            broadcast(&peers[half..], &Message::Send(n.to_string()));
                        }
                        _ => {
                            println!("Unknown command");
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
                Message::Send(m) => {
                    println!("Received SEND: {}", m);
                }
                Message::Echo(m) => {
                    println!("Received ECHO: {}", m);
                }
                Message::Ready(m) => {
                    println!("Received READY: {}", m);
                }
            }
        } else {
            println!("Received unknown message: {}", line);
        }
    }
}
