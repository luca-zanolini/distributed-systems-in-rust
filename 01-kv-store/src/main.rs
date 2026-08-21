use std::collections::HashMap;
use std::io;

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

fn main() {
    let mut store = Store::new();
    loop {
        let mut line = String::new();
        let bytes = io::stdin().read_line(&mut line).unwrap();
        if bytes == 0 {
            break;   // EOF: Ctrl+D, or end of piped input
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["set", key, value] => store.set(key.to_string(), value.to_string()),
            ["get", key] => match store.get(key) {
                Some(value) => println!("{}", value),
                None => println!("Key not found"),
            },
            ["remove", key] => match store.remove(key) {
                Some(value) => println!("Removed: {}", value),
                None => println!("Key not found"),
            },
            [] => {},
            ["exit"] => break,
            _ => println!("unknown command"),
        }
    }
}
