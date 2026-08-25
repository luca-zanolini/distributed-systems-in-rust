use std::collections::HashMap; // the map that backs the store
use std::io::Write; // brings write_all() into scope
use std::io::{BufRead, BufReader}; // buffered reader + its .lines() method
use std::net::TcpListener; // the server socket (accepts connections)
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

struct Store {
    // our key-value store
    map: HashMap<String, String>, // key -> value, both owned Strings
}

impl Store {
    // methods on Store
    fn new() -> Self {
        // constructor (Self == Store)
        Store {
            map: HashMap::new(), // start empty
        }
    }
    fn set(&mut self, key: String, value: String) {
        // &mut: mutates; owns key+value (moved in)
        self.map.insert(key, value); // insert or overwrite
    }
    fn get(&self, key: &str) -> Option<&String> {
        // &self: read-only; borrows key, lends value
        self.map.get(key) // Some(&value) or None
    }
    fn remove(&mut self, key: &str) -> Option<String> {
        // returns the OWNED removed value
        self.map.remove(key) // Some(value) if it existed, else None
    }
}
fn handle_client(store: Arc<Mutex<Store>>, stream: TcpStream) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?; // 2nd handle to the SAME socket, for writing
    let reader = BufReader::new(stream); // wrap the socket to read it line by line
    for line in reader.lines() {
        // one iteration per COMMAND line from this client
        let line = line?; // the line (newline stripped), or an I/O error
        let parts: Vec<&str> = line.split_whitespace().collect(); // split into words
        let response = match parts.as_slice() {
            ["set", key, rest @ ..] => {
                store.lock().unwrap().set(key.to_string(), rest.join(" ")); // lock → mutate → auto-unlock
                "OK\n".to_string()
            }
            ["get", key] => {
                let guard = store.lock().unwrap(); // lock
                match guard.get(key) {
                    // read while holding it
                    Some(value) => format!("{value}\n"),
                    None => "Key not found\n".to_string(),
                } // guard drops here → unlock
            }
            ["remove", key] => {
                match store.lock().unwrap().remove(key) {
                    // lock → remove → auto-unlock
                    Some(_) => "OK\n".to_string(),
                    None => "Key not found\n".to_string(),
                }
            }
            _ => "unknown command\n".to_string(),
        };
        writer.write_all(response.as_bytes())?; // send the response back over the socket
    } // (inner loop ends when the client disconnects = EOF)
    Ok(())
}

fn main() -> std::io::Result<()> {
    // Result return so we can use ?
    let listener = TcpListener::bind("127.0.0.1:4000")?; // claim the port, start listening
    let store = Arc::new(Mutex::new(Store::new())); // shared, lockable store
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };
        let store = Arc::clone(&store); // a handle for THIS client's thread
        thread::spawn(move || {
            // serve it concurrently
            if let Err(e) = handle_client(store, stream) {
                eprintln!("client error: {e}");
            }
        });
    }
    Ok(()) // unreachable in practice: the accept loop runs forever
}
