use std::io::{BufRead, BufReader, Write};   // read_line/.lines() (BufRead) + write_all (Write)
use std::net::TcpStream;                    // the client socket

fn main() -> std::io::Result<()> {          // Result return so we can use ?
    let mut stream = TcpStream::connect("127.0.0.1:4000")?; // open a connection to the server
    let mut server = BufReader::new(stream.try_clone()?);   // 2nd handle, to read the server's replies

    for line in std::io::stdin().lock().lines() { // one iteration per line YOU type (until EOF)
        let line = line?;                   // the typed command (newline stripped), or an error
        stream.write_all(line.as_bytes())?; // send the command bytes to the server
        stream.write_all(b"\n")?;           // re-add the newline (the server frames on '\n')
        let mut response = String::new();   // buffer for the server's reply
        server.read_line(&mut response)?;   // read exactly one reply line
        print!("{response}");               // print it (it already ends in '\n')
    }

    Ok(())                                  // reached when stdin hits EOF (Ctrl+D)
}
