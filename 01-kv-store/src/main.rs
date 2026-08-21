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

    fn save(&self, path: &str) -> std::io::Result<()> {
        let mut data = String::new();
        for (key, value) in &self.map {
            data.push_str(&format!("{key}\t{value}\n"));
        }
        std::fs::write(path, data)?;
        Ok(())
    }

    fn load(&mut self, path: &str) -> std::io::Result<()> {
        if !std::path::Path::new(path).exists() {
            return Ok(()); 
        }
        let contents = std::fs::read_to_string(path)?;
        for line in contents.lines() {
            if let Some((key, value)) = line.split_once('\t') {
                self.map.insert(key.to_string(), value.to_string());
            }
        }
        Ok(())
    }
}

fn main() -> std::io::Result<()> {
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
            ["set", key, value] => store.set(key.to_string(), value.to_string()),
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
                store.save("store.db").expect("Failed to save store");
                break;
            }
            _ => println!("unknown command"),
        }
    }
    Ok(())
}
