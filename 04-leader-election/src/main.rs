use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).cloned().unwrap_or_else(|| "5000".to_string());
    let peers: Vec<String> = args.iter().skip(2).cloned().collect();
    println!("node :{port} — peers {peers:?}");

    {
        let port = port.clone();
        let peers = peers.clone();
        thread::spawn(move || loop {
            for addr in &peers {
                if let Ok(mut stream) = TcpStream::connect(addr) {
                    let _ = writeln!(stream, "ping {}", port);
                }
            }
            thread::sleep(Duration::from_secs(1));
        });
    }

    {
        let port = port.clone();
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    if let Some(sender) = line.strip_prefix("ping ") {
                        println!("heartbeat from {}", sender.trim());
                    }
                }
            }
        }
    }       

    Ok(())
}
