//! Structured output of a long calibration run: one JSON checkpoint per stage or trial and a
//! final `result.json`, written also when the run fails, so no fitted value is lost with the
//! process. A write that fails is reported on stderr and never stops the run.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::border;
use crate::epsilon::Trial;
use crate::json::Json;

pub struct RunLog {
    dir: PathBuf,
    start: Instant,
    seq: Cell<usize>,
}

impl RunLog {
    pub fn new(dir: &Path) -> Result<Self, String> {
        let dir = dir.to_path_buf();
        std::fs::create_dir_all(dir.join("checkpoints"))
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self {
            dir,
            start: Instant::now(),
            seq: Cell::new(0),
        })
    }

    /// `checkpoints/NNN-<stage>.json`, numbered in call order.
    pub fn checkpoint(&self, stage: &str, data: Json) {
        let n = self.seq.get();
        self.seq.set(n + 1);
        let doc = Json::object([
            ("stage", Json::Str(stage.into())),
            ("seq", Json::Int(n as i64)),
            ("seconds", Json::Float(self.start.elapsed().as_secs_f64())),
            ("data", data),
        ]);
        self.write(&format!("checkpoints/{n:03}-{stage}.json"), &doc);
    }

    /// `result.json`: the final status, its data (`Json::Null` on failure) and the error lines.
    pub fn finish(&self, ok: bool, data: Json, errors: &[String]) {
        let doc = Json::object([
            ("status", Json::Str(if ok { "ok" } else { "failed" }.into())),
            ("seconds", Json::Float(self.start.elapsed().as_secs_f64())),
            ("checkpoints", Json::Int(self.seq.get() as i64)),
            (
                "errors",
                Json::List(errors.iter().map(|e| Json::Str(e.clone())).collect()),
            ),
            ("data", data),
        ]);
        self.write("result.json", &doc);
    }

    // rename, so a kill mid-write leaves the previous file, not a torn one
    fn write(&self, name: &str, doc: &Json) {
        let file = self.dir.join(name);
        let tmp = file.with_extension("tmp");
        let done = std::fs::write(&tmp, doc.pretty()).and_then(|()| std::fs::rename(&tmp, &file));
        if let Err(e) = done {
            eprintln!("run log: {}: {e}", file.display());
        }
    }
}

/// One epsilon trial: epsilon, nu, `d_0`, s_c, the pooled coefficient and each class's held-out
/// coefficient.
pub fn trial_json(t: &Trial, data_pooled: f64) -> Json {
    let floats = |v: &[f64]| Json::List(v.iter().map(|&x| Json::Float(x)).collect());
    Json::object([
        ("epsilon", Json::Float(t.epsilon)),
        ("model_pooled", Json::Float(t.pooled)),
        ("data_pooled", Json::Float(data_pooled)),
        ("classes", floats(&t.classes)),
        ("nu", floats(&t.fit.nu)),
        ("nu_evaluations", Json::Int(t.fit.evaluations as i64)),
        ("ln_d0", floats(&t.ln_d0)),
        ("scale", floats(&t.scale)),
        (
            "floored",
            Json::List(t.floored.iter().map(|&c| Json::Int(c as i64)).collect()),
        ),
        ("n_classes", Json::Int(border::N_CLASSES as i64)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(dir: &Path, name: &str) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
    }

    #[test]
    fn checkpoints_and_result_parse() {
        let dir = tempfile::tempdir().unwrap();
        let log = RunLog::new(dir.path()).unwrap();
        log.checkpoint("trial", Json::object([("epsilon", Json::Float(0.37))]));
        log.checkpoint("trial", Json::Null);
        log.finish(true, Json::object([("epsilon", Json::Float(0.37))]), &[]);
        let c = read(dir.path(), "checkpoints/001-trial.json");
        assert_eq!(c["seq"], 1);
        let r = read(dir.path(), "result.json");
        assert_eq!(r["status"], "ok");
        assert_eq!(r["checkpoints"], 2);
        assert_eq!(r["data"]["epsilon"], 0.37);
    }

    #[test]
    fn a_failed_run_writes_its_error_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log = RunLog::new(dir.path()).unwrap();
        log.checkpoint("trial", Json::Float(f64::NAN));
        log.finish(false, Json::Null, &["no epsilon fit: stuck".into()]);
        let r = read(dir.path(), "result.json");
        assert_eq!(r["status"], "failed");
        assert_eq!(r["errors"][0], "no epsilon fit: stuck");
        assert!(r["data"].is_null());
        assert!(dir.path().join("checkpoints/000-trial.json").exists());
    }
}
