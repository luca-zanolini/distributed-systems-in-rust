//! 01 — Key-value store.
//!
//! In the language of the reference text (Cachin, Guerraoui & Rodrigues, *Introduction to
//! Reliable and Secure Distributed Programming*, 2nd ed., 2011 — "CCGR"): a KV store is a set of
//! read/write **registers**, one per key (CCGR Ch. 4). This is the single-process, failure-free
//! case, so the register is trivially atomic. `save`/`load` below is **stable storage**
//! (CCGR §2.2.4) — what a process uses to survive a crash and recover its state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;

#[derive(Serialize, Deserialize)]
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

    fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    fn load(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !std::path::Path::new(path).exists() {
            return Ok(());
        }
        let contents = std::fs::read_to_string(path)?;
        *self = serde_json::from_str(&contents)?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::new();
    store.load("store.db")?;
    loop {
        let mut line = String::new();
        let bytes = io::stdin().read_line(&mut line).unwrap();
        if bytes == 0 {
            break; // EOF: Ctrl+D, or end of piped input
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["set", key, rest @ ..] => {
                let value = rest.join(" ");
                store.set(key.to_string(), value);
            }
            ["get", key] => match store.get(key) {
                Some(value) => println!("{}", value),
                None => println!("Key not found"),
            },
            ["remove", key] => match store.remove(key) {
                Some(value) => println!("Removed: {}", value),
                None => println!("Key not found"),
            },
            [] => {}
            ["exit"] => {
                store.save("store.db")?;
                break;
            }
            _ => println!("unknown command"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut store = Store::new();
        store.set("k".to_string(), "v".to_string());
        assert_eq!(store.get("k"), Some(&"v".to_string()));
    }

    #[test]
    fn missing_key() {
        let store = Store::new();
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn remove_key() {
        let mut store = Store::new();
        store.set("k".to_string(), "v".to_string());
        assert_eq!(store.remove("k"), Some("v".to_string()));
        assert_eq!(store.get("k"), None);
    }
}
