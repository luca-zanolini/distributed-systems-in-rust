use std::collections::HashMap; // the map that backs the store
use std::io::Write; // brings write_all() into scope
use std::io::{BufRead, BufReader}; // buffered reader + its .lines() method
use std::net::TcpListener; // the server socket (accepts connections)
use std::net::TcpStream;

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

fn handle_client(store: &mut Store, stream: TcpStream) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?; // 2nd handle to the SAME socket, for writing
    let reader = BufReader::new(stream); // wrap the socket to read it line by line
    for line in reader.lines() {
        // one iteration per COMMAND line from this client
        let line = line?; // the line (newline stripped), or an I/O error
        let parts: Vec<&str> = line.split_whitespace().collect(); // split into words
        let response = match parts.as_slice() {
            // match on the shape of the words
            ["set", key, rest @ ..] => {
                // "set" + key + one-or-more value words
                store.set(key.to_string(), rest.join(" ")); // store it (value may have spaces)
                "OK\n".to_string() // this arm's response value
            }
            ["get", key] => {
                // "get" + key
                match store.get(key) {
                    // look it up
                    Some(value) => format!("{}\n", value), // found -> send the value
                    None => "Key not found\n".to_string(), // absent -> not found
                }
            }
            ["remove", key] => {
                // "remove" + key
                match store.remove(key) {
                    // delete it; inspect what was there
                    Some(_) => "OK\n".to_string(), // existed -> OK
                    None => "Key not found\n".to_string(), // didn't exist -> not found
                }
            }
            _ => "unknown command\n".to_string(), // anything else
        };
        writer.write_all(response.as_bytes())?; // send the response back over the socket
    } // (inner loop ends when the client disconnects = EOF)
    Ok(())
}

fn main() -> std::io::Result<()> {
    // Result return so we can use ?
    let listener = TcpListener::bind("127.0.0.1:4000")?; // claim the port, start listening
    let mut store = Store::new(); // one store, shared across all connections
    for stream in listener.incoming() {
        // one iteration per incoming CONNECTION
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            } // bad accept -> skip it
        };
        if let Err(e) = handle_client(&mut store, stream) {
            eprintln!("client error: {e}"); // one client failed -> log it, keep serving
        }
    }
    Ok(()) // unreachable in practice: the accept loop runs forever
}
