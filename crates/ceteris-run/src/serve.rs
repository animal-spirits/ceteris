//! The read-only API of a workbench (`ceteris serve`, specs/p3/PLAN.md F4): newline-delimited
//! JSON-RPC over a loopback TCP socket, a token file with a read scope, and tables as Arrow IPC
//! streams. A request is one line `{"id":1,"token":"...","method":"...","params":{...}}`; the
//! reply is one line `{"id":1,"result":...}` or `{"id":1,"error":{"code":C,"message":M}}`. A
//! `table.read` result carries `bytes`, and that many raw bytes of the IPC stream follow the
//! reply line. The server never touches a world, so reads cannot change a state hash.
//! `tick.subscribe` turns a connection into a stream of `{"event":"tick","tick":N}` lines, one per
//! tick the owner of the server announces with `Server::publish`; the connection takes no
//! further requests. The CLI's `serve` runs no world, so it announces nothing yet (tracked in
//! #201).

use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};

use serde_json::{Value, json};

use crate::commands::REGISTRY;
use crate::outputs::TABLES;
use crate::scenario::Scenario;

const NOT_FOUND: i64 = -32601;
const BAD_PARAMS: i64 = -32602;
const DENIED: i64 = -32001;
const READ_SCOPE: i64 = -32002;
const NO_TABLE: i64 = -32003;

/// Binds `addr`, which must be a loopback address.
pub fn bind(addr: SocketAddr) -> io::Result<TcpListener> {
    if !addr.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{addr} is not a loopback address"),
        ));
    }
    TcpListener::bind(addr)
}

pub struct Server {
    pub token: String,
    pub scenario: Scenario,
    /// Where the `.arrow` tables of the verbs' `--out` live.
    pub tables: PathBuf,
    ticks: (Mutex<Option<u64>>, Condvar),
}

enum Reply {
    Result(Value),
    Table(Vec<u8>),
    Error(i64, String),
}

impl Server {
    pub fn new(token: String, scenario: Scenario, tables: PathBuf) -> Self {
        Server {
            token,
            scenario,
            tables,
            ticks: (Mutex::new(None), Condvar::new()),
        }
    }

    /// Announces that `tick` is complete to every subscriber.
    pub fn publish(&self, tick: u64) {
        *self.ticks.0.lock().unwrap() = Some(tick);
        self.ticks.1.notify_all();
    }

    /// Writes each announced tick above `seen` to `out` until the write fails.
    fn stream_ticks(&self, out: &mut impl Write, mut seen: Option<u64>) -> io::Result<()> {
        let mut now = self.ticks.0.lock().unwrap();
        loop {
            while *now == seen {
                now = self.ticks.1.wait(now).unwrap();
            }
            seen = *now;
            writeln!(out, "{}", json!({"event": "tick", "tick": seen}))?;
            out.flush()?;
        }
    }

    fn call(&self, method: &str, params: &Value) -> Reply {
        match method {
            "api.schema" => Reply::Result(json!(
                TABLES
                    .iter()
                    .map(|(t, cols)| json!({
                        "table": t,
                        "columns": cols.iter().map(|(c, k)| json!([c, format!("{k:?}")])).collect::<Vec<_>>(),
                    }))
                    .collect::<Vec<_>>()
            )),
            "command.list" => Reply::Result(json!(
                REGISTRY
                    .iter()
                    .map(|c| json!({"name": c.name, "usage": c.usage, "summary": c.summary, "built": c.built}))
                    .collect::<Vec<_>>()
            )),
            "scenario.branches" => Reply::Result(json!(
                self.scenario.branches.iter().map(|b| &b.id).collect::<Vec<_>>()
            )),
            "table.read" => {
                let Some(name) = params["name"].as_str() else {
                    return Reply::Error(BAD_PARAMS, "table.read wants params.name".into());
                };
                if !TABLES.iter().any(|(t, _)| *t == name) {
                    return Reply::Error(BAD_PARAMS, format!("unknown table '{name}'"));
                }
                match std::fs::read(self.tables.join(format!("{name}.arrow"))) {
                    Ok(bytes) => Reply::Table(bytes),
                    Err(_) => Reply::Error(NO_TABLE, format!("table '{name}' has not been written")),
                }
            }
            m if REGISTRY.iter().any(|c| c.name == m) => {
                Reply::Error(READ_SCOPE, format!("'{m}' needs a write scope"))
            }
            m => Reply::Error(NOT_FOUND, format!("no method '{m}'")),
        }
    }

    fn line(&self, line: &str) -> (Value, Option<Vec<u8>>) {
        let Ok(req) = serde_json::from_str::<Value>(line) else {
            return (error(Value::Null, BAD_PARAMS, "not JSON"), None);
        };
        let id = req["id"].clone();
        if req["token"].as_str() != Some(self.token.as_str()) {
            return (error(id, DENIED, "bad token"), None);
        }
        let Some(method) = req["method"].as_str() else {
            return (error(id, BAD_PARAMS, "missing method"), None);
        };
        match self.call(method, &req["params"]) {
            Reply::Result(v) => (json!({"id": id, "result": v}), None),
            Reply::Table(b) => (json!({"id": id, "result": {"bytes": b.len()}}), Some(b)),
            Reply::Error(c, m) => (error(id, c, &m), None),
        }
    }

    /// Answers the requests of one connection until it closes.
    pub fn serve(&self, stream: TcpStream) -> io::Result<()> {
        let mut out = stream.try_clone()?;
        for line in BufReader::new(stream).lines() {
            let line = line?;
            if serde_json::from_str::<Value>(&line).is_ok_and(|r| {
                r["token"].as_str() == Some(self.token.as_str())
                    && r["method"].as_str() == Some("tick.subscribe")
            }) {
                let seen = *self.ticks.0.lock().unwrap();
                writeln!(
                    out,
                    "{}",
                    json!({"id": serde_json::from_str::<Value>(&line)?["id"], "result": "subscribed"})
                )?;
                return self.stream_ticks(&mut out, seen);
            }
            let (reply, bytes) = self.line(&line);
            writeln!(out, "{reply}")?;
            if let Some(b) = bytes {
                out.write_all(&b)?;
            }
            out.flush()?;
        }
        Ok(())
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::sync::Arc;
    use std::thread;

    use super::*;

    fn scenario() -> Scenario {
        Scenario::parse(
            "pack_hash = \"abc\"\nengine = \"test+1\"\nseeds = [1]\n\n[[branch]]\nid = \"main\"\n",
        )
        .unwrap()
    }

    fn start(dir: &std::path::Path) -> (SocketAddr, Arc<Server>) {
        let listener = bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = listener.local_addr().unwrap();
        let server = Arc::new(Server::new("secret".into(), scenario(), dir.to_path_buf()));
        let s = Arc::clone(&server);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let s = Arc::clone(&s);
                thread::spawn(move || s.serve(stream));
            }
        });
        (addr, server)
    }

    fn ask(addr: SocketAddr, request: &str) -> (Value, Vec<u8>) {
        let mut stream = TcpStream::connect(addr).unwrap();
        writeln!(stream, "{request}").unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        let n = reply["result"]["bytes"].as_u64().unwrap_or(0) as usize;
        let mut raw = vec![0; n];
        reader.read_exact(&mut raw).unwrap();
        (reply, raw)
    }

    #[test]
    fn subscribers_get_each_published_tick() {
        let dir = tempfile::tempdir().unwrap();
        let (addr, server) = start(dir.path());
        let mut stream = TcpStream::connect(addr).unwrap();
        writeln!(
            stream,
            r#"{{"id":1,"token":"secret","method":"tick.subscribe"}}"#
        )
        .unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.contains("subscribed"));
        for t in [3u64, 4] {
            server.publish(t);
            line.clear();
            reader.read_line(&mut line).unwrap();
            assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["tick"], t);
        }
    }

    #[test]
    fn non_loopback_bind_is_refused() {
        let e = bind("0.0.0.0:0".parse().unwrap()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn token_and_scope_are_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let (addr, _) = start(dir.path());
        let (r, _) = ask(addr, r#"{"id":1,"token":"nope","method":"api.schema"}"#);
        assert_eq!(r["error"]["code"], DENIED);
        let (r, _) = ask(addr, r#"{"id":2,"token":"secret","method":"edit"}"#);
        assert_eq!(r["error"]["code"], READ_SCOPE);
        let (r, _) = ask(addr, r#"{"id":3,"token":"secret","method":"nothing"}"#);
        assert_eq!(r["error"]["code"], NOT_FOUND);
        let (r, _) = ask(
            addr,
            r#"{"id":4,"token":"secret","method":"scenario.branches"}"#,
        );
        assert_eq!(r["result"], json!(["main"]));
        let (r, _) = ask(addr, r#"{"id":5,"token":"secret","method":"api.schema"}"#);
        assert_eq!(r["result"].as_array().unwrap().len(), TABLES.len());
    }

    #[test]
    fn serve_reads_leave_tables_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("effects.arrow");
        std::fs::write(&file, b"arrow-bytes").unwrap();
        let (addr, _) = start(dir.path());
        let req = r#"{"id":1,"token":"secret","method":"table.read","params":{"name":"effects"}}"#;
        let (r, raw) = ask(addr, req);
        assert_eq!(r["result"]["bytes"], 11);
        assert_eq!(raw, b"arrow-bytes");
        assert_eq!(std::fs::read(&file).unwrap(), b"arrow-bytes");
        let (r, _) = ask(
            addr,
            r#"{"id":2,"token":"secret","method":"table.read","params":{"name":"outcomes"}}"#,
        );
        assert_eq!(r["error"]["code"], NO_TABLE);
        let (r, _) = ask(
            addr,
            r#"{"id":3,"token":"secret","method":"table.read","params":{"name":"../x"}}"#,
        );
        assert_eq!(r["error"]["code"], BAD_PARAMS);
    }
}
