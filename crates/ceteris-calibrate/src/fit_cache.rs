//! A disk cache of epsilon-fit trials. A trial is a deterministic function of the inputs, the
//! targets, the limits and epsilon, so a stored trial equals a recomputed one bit for bit: f64
//! values are stored as hex bit patterns. A run that dies or repeats (a resumed rented machine, a
//! report-only rerun) replays its cached trials and computes only the missing ones.
//!
//! The key covers the data, not the code: the caller keeps one cache directory per code revision
//! (`scripts/cloud/fit.sh` names it by the git tree hash).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::border::Fit;
use crate::epsilon::{Targets, Trial};
use crate::steady::Limits;
use ceteris_reference::io::Inputs;

/// Hashes what `Debug` writes without building the string.
struct HashWriter(Sha256);

impl std::fmt::Write for HashWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.update(s.as_bytes());
        Ok(())
    }
}

fn bits(hasher: &mut Sha256, values: &[f64]) {
    hasher.update((values.len() as u64).to_le_bytes());
    for v in values {
        hasher.update(v.to_bits().to_le_bytes());
    }
}

pub struct TrialCache {
    dir: PathBuf,
    key: String,
}

impl TrialCache {
    /// The cache in `dir` for these inputs, targets and limits; `dir` is created on the first store.
    pub fn new(dir: &Path, inp: &Inputs, t: &Targets, limits: Limits) -> Self {
        let mut w = HashWriter(Sha256::new());
        write!(w, "{inp:?}{limits:?}{:?}{:?}", t.icio, t.theta_class).expect("hashing cannot fail");
        let mut h = w.0;
        for v in [t.nu_target, t.ln_d, t.weight, t.data_flows] {
            bits(&mut h, v);
        }
        let key = h
            .finalize()
            .iter()
            .take(16)
            .fold(String::new(), |mut s, b| {
                let _ = write!(s, "{b:02x}");
                s
            });
        Self {
            dir: dir.to_owned(),
            key,
        }
    }

    fn file(&self, epsilon: f64) -> PathBuf {
        self.dir
            .join(format!("{}-{:016x}.trial", self.key, epsilon.to_bits()))
    }

    /// `Some(None)` is a cached trial where no nu reaches the targets.
    pub fn load(&self, epsilon: f64) -> Option<Option<Trial>> {
        let text = std::fs::read_to_string(self.file(epsilon)).ok()?;
        decode(&text, epsilon)
    }

    pub fn store(&self, epsilon: f64, trial: &Option<Trial>) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let file = self.file(epsilon);
        let part = file.with_extension("part");
        std::fs::write(&part, encode(trial)).map_err(|e| format!("{}: {e}", part.display()))?;
        std::fs::rename(&part, &file).map_err(|e| format!("{}: {e}", file.display()))
    }
}

fn floats(values: &[f64]) -> String {
    values
        .iter()
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn ints<'a>(values: impl IntoIterator<Item = &'a usize>) -> String {
    values
        .into_iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn encode(trial: &Option<Trial>) -> String {
    let Some(t) = trial else {
        return "none\n".into();
    };
    let f = &t.fit;
    let groups = f.groups.iter().map(ints).collect::<Vec<_>>().join(",");
    let clamped: Vec<usize> = f.clamped.iter().map(|&c| usize::from(c)).collect();
    format!(
        "trial\ngroups {groups}\nnu {}\ntarget {}\nshare {}\nevaluations {}\nclamped {}\n\
         ln_d0 {}\nfloored {}\npooled {}\nclasses {}\n",
        floats(&f.nu),
        floats(&f.target),
        floats(&f.share),
        f.evaluations,
        ints(&clamped),
        floats(&t.ln_d0),
        ints(&t.floored),
        floats(&[t.pooled]),
        floats(&t.classes),
    )
}

fn decode(text: &str, epsilon: f64) -> Option<Option<Trial>> {
    let mut lines = text.lines();
    match lines.next()? {
        "none" => return Some(None),
        "trial" => {}
        _ => return None,
    }
    let mut field =
        |name: &str| -> Option<&str> { lines.next()?.strip_prefix(name).map(str::trim) };
    let parse_f = |s: &str| -> Option<Vec<f64>> {
        s.split_whitespace()
            .map(|h| u64::from_str_radix(h, 16).ok().map(f64::from_bits))
            .collect()
    };
    let parse_u =
        |s: &str| -> Option<Vec<usize>> { s.split_whitespace().map(|v| v.parse().ok()).collect() };
    let groups = field("groups")?
        .split(',')
        .map(parse_u)
        .collect::<Option<Vec<_>>>()?;
    let nu = parse_f(field("nu")?)?;
    let target = parse_f(field("target")?)?;
    let share = parse_f(field("share")?)?;
    let evaluations = field("evaluations")?.parse().ok()?;
    let clamped = parse_u(field("clamped")?)?
        .into_iter()
        .map(|c| c != 0)
        .collect();
    let ln_d0 = parse_f(field("ln_d0")?)?;
    let floored = parse_u(field("floored")?)?;
    let pooled = *parse_f(field("pooled")?)?.first()?;
    let classes = parse_f(field("classes")?)?;
    Some(Some(Trial {
        epsilon,
        fit: Fit {
            groups,
            nu,
            target,
            share,
            evaluations,
            clamped,
        },
        ln_d0,
        floored,
        pooled,
        classes,
    }))
}
