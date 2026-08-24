use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

fn main() -> std::io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:4000")?; // connect
    let mut server = BufReader::new(stream.try_clone()?); // read handle for responses

    for line in std::io::stdin().lock().lines() {
        let line = line?;
        stream.write_all(line.as_bytes())?; // send the command
        stream.write_all(b"\n")?; // + the newline frame
        let mut response = String::new();
        server.read_line(&mut response)?; // read one response line
        print!("{response}"); // (already ends in \n)
    }

    Ok(())
}
