//! Development engine: serves least-cost queries to the map in a plain browser, where the
//! desktop app's IPC is absent. Not part of the app, which has no server (Section 22.1).
//!
//! `cargo run --release -p ceteris-geo --example engine`, then `npm run dev` in `app/`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir};
use ceteris_geo::paths::PathCache;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pack = Pack::load(&default_dir(), &TransportParams::default())?;
    let n = pack.provinces.len();
    let mut cache = PathCache::default();
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    eprintln!("engine: {n} provinces, listening on 127.0.0.1:7878");
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        let source = line
            .split_whitespace()
            .nth(1)
            .and_then(|path| path.strip_prefix("/engine/cost/"))
            .and_then(|i| i.parse::<u32>().ok());
        match source.filter(|&s| (s as usize) < n) {
            Some(s) => {
                let body = cache.tree(&pack.graph, s).province_bytes(n);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )?;
                stream.write_all(&body)?;
            }
            None => write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )?,
        }
    }
    Ok(())
}
