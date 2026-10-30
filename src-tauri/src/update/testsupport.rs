//! A tiny HTTP/1.1 server on 127.0.0.1 for the updater tests: fixed bodies, redirects, slow and stalled sends.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub enum Reply {
    Body(Vec<u8>),
    Redirect(String),
    /// Sends `body` in `chunk`-sized pieces, pausing `every` between them: slow, but never idle for long.
    Trickle { body: Vec<u8>, chunk: usize, every: Duration },
    /// Sends the first `head` bytes, then goes silent for `hold` before closing.
    Stall { body: Vec<u8>, head: usize, hold: Duration },
}

/// Starts a server on an ephemeral port and returns its origin (`http://127.0.0.1:port`).
/// `routes` is built from that origin, so bodies can name their own URLs.
pub fn serve(routes: impl FnOnce(&str) -> Vec<(String, Reply)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback is available");
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let routes = Arc::new(routes(&origin));
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let routes = Arc::clone(&routes);
            thread::spawn(move || handle(stream, &routes));
        }
    });
    origin
}

fn handle(mut stream: TcpStream, routes: &[(String, Reply)]) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
            break;
        }
    }
    let _ = match routes.iter().find(|(p, _)| *p == path) {
        Some((_, Reply::Body(body))) => send_ok(&mut stream, body.len(), |s| s.write_all(body)),
        Some((_, Reply::Redirect(to))) => stream.write_all(
            format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
        ),
        Some((_, Reply::Trickle { body, chunk, every })) => send_ok(&mut stream, body.len(), |s| {
            body.chunks(*chunk).try_for_each(|part| {
                thread::sleep(*every);
                s.write_all(part)
            })
        }),
        Some((_, Reply::Stall { body, head, hold })) => send_ok(&mut stream, body.len(), |s| {
            s.write_all(&body[..(*head).min(body.len())])?;
            thread::sleep(*hold);
            Ok(())
        }),
        None => stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
    };
}

/// Status line and headers for a `length`-byte 200 response, then whatever `body` writes.
fn send_ok(
    stream: &mut TcpStream,
    length: usize,
    body: impl FnOnce(&mut TcpStream) -> std::io::Result<()>,
) -> std::io::Result<()> {
    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n").as_bytes())?;
    body(stream)
}
