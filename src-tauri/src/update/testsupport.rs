//! A tiny HTTP/1.1 server on 127.0.0.1 for the updater tests: fixed bodies, one request per connection.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

#[derive(Clone)]
pub enum Reply {
    Body(Vec<u8>),
    Redirect(String),
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
        Some((_, Reply::Body(body))) => {
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(body))
        }
        Some((_, Reply::Redirect(to))) => stream.write_all(
            format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
        ),
        None => stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
    };
}
