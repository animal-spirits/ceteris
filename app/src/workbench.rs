//! W1 host: the branch session on its own thread, so a run-to over a real world never blocks
//! the frame loop. Registry lines go in; a snapshot of the tree, the edit log and the state
//! hash at the cursor comes out after each one.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::branches::Row;
use crate::commands::BranchCommand;
use crate::session::Session;

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub branch: String,
    pub cursor: u32,
    pub rows: Vec<Row>,
    pub log: Vec<String>,
    /// Lower-case hex of the state hash at the cursor; empty if the view did not build.
    pub hash: String,
    /// Why the last command failed, or the session could not start.
    pub error: Option<String>,
}

pub struct Host {
    lines: Sender<String>,
    snapshots: Receiver<Snapshot>,
}

/// Where the chunk store of branch runs lives: the platform's cache folder, one per engine
/// build, because a store refuses to open under another engine.
pub fn cache_root(engine: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let xdg = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from);
    let base = match (cfg!(target_os = "macos"), home, xdg) {
        (true, Some(h), _) => h.join("Library/Caches/Ceteris"),
        (_, _, Some(x)) => x.join("ceteris"),
        (_, Some(h), _) => h.join(".cache/ceteris"),
        _ => std::env::temp_dir().join("ceteris-cache"),
    };
    base.join("branches").join(engine)
}

fn hex(hash: [u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn snapshot(session: &mut Session, error: Option<String>) -> Snapshot {
    let (hash, view_error) = match session.view() {
        Ok(sim) => (hex(sim.state_hash()), None),
        Err(e) => (String::new(), Some(e.to_string())),
    };
    Snapshot {
        branch: session.branch().to_owned(),
        cursor: session.cursor(),
        rows: session.rows(),
        log: session.log(),
        hash,
        error: error.or(view_error),
    }
}

impl Host {
    /// Builds the session on the new thread and publishes its first snapshot; `wake` runs after
    /// each publish. The thread ends when the host is dropped.
    pub fn spawn(
        build: impl FnOnce() -> Result<Session, String> + Send + 'static,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (lines, input) = channel::<String>();
        let (output, snapshots) = channel();
        std::thread::spawn(move || {
            let mut session = match build() {
                Ok(s) => s,
                Err(e) => {
                    let _ = output.send(Snapshot {
                        branch: String::new(),
                        cursor: 0,
                        rows: vec![],
                        log: vec![],
                        hash: String::new(),
                        error: Some(e),
                    });
                    wake();
                    return;
                }
            };
            if output.send(snapshot(&mut session, None)).is_err() {
                return;
            }
            wake();
            while let Ok(line) = input.recv() {
                let error = match BranchCommand::parse(&line) {
                    Some(cmd) => session.execute(cmd).err(),
                    None => Some(format!("not a branch command: {line}")),
                };
                if output.send(snapshot(&mut session, error)).is_err() {
                    return;
                }
                wake();
            }
        });
        Self { lines, snapshots }
    }

    /// Sends one registry line, such as `branch.fork tariff` or `time.run-to 24`.
    pub fn send(&self, line: &str) {
        let _ = self.lines.send(line.to_owned());
    }

    /// The newest snapshot published since the last call. Never blocks.
    pub fn latest(&self) -> Option<Snapshot> {
        self.snapshots.try_iter().last()
    }

    /// Blocks for the next snapshot; for tests.
    pub fn wait(&self) -> Option<Snapshot> {
        self.snapshots.recv().ok()
    }
}
