use std::collections::HashMap;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;

struct Store {
    map: HashMap<String, String>,
}

impl Store {
    fn new() -> Self {
        Store {
            map: HashMap::new(),
        }
    }
    fn set(&mut self, key: String, value: String) {
        self.map.insert(key, value);
    }
    fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }
    fn remove(&mut self, key: &str) -> Option<String> {
        self.map.remove(key)
    }
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4000")?;
    let mut store = Store::new();
    for stream in listener.incoming() {
        let stream = stream?; // a connected client
        let mut writer = stream.try_clone()?;
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            let response = match parts.as_slice() {
                ["set", key, rest @ ..] => {
                    store.set(key.to_string(), rest.join(" ")); 
                    "OK\n".to_string() 
                }
                ["get", key] => {
                    match store.get(key) {
                        Some(value) => format!("{}\n", value),
                        None => "Key not found\n".to_string(),
                    }
                }
                ["remove", key] => {
                    match store.remove(key) {
                        Some(_) => "OK\n".to_string(),
                        None => "Key not found\n".to_string(),
                    }
                }
                _ => "unknown command\n".to_string(),
            };
            writer.write_all(response.as_bytes())?;
        }
    }
    Ok(())
}
