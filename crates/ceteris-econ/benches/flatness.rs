//! The flatness benchmark (ADR 0234, task G1): the median tick time of the last five years of a
//! run against the first five, row counts per state table per decade, and peak resident
//! memory. Release build, default scenario, the calibrated pack (`CETERIS_PACK`, default
//! `data/packs/2000`) or the fixture. `FLATNESS_YEARS` sets the length (default 100; CI sets
//! 10 and reads the row counts only). Writes `tests/perf/flatness.md`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use ceteris_core::flatness::{self, Decade};
use ceteris_core::sim::SNAPSHOT_EVERY;
use ceteris_core::timing;
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;

const TICKS_PER_YEAR: usize = 12;

fn inputs() -> (EconInputs, &'static str) {
    let dir = std::env::var_os("CETERIS_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"),
        PathBuf::from,
    );
    if EconInputs::has_calibration(&dir) {
        let pack = Pack::load(&dir, &TransportParams::default()).expect("pack loads");
        return (
            EconInputs::from_pack(&pack, &dir).expect("pack inputs load"),
            "pack",
        );
    }
    eprintln!(
        "notice: no calibrated pack at {}; running the fixture",
        dir.display()
    );
    (small_world(), "fixture")
}

fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// Process CPU time (user plus system) in ms, from `ps`; the bench forbids the unsafe a
/// `getrusage` call needs.
fn cpu_ms() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "time=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps runs");
    let t = String::from_utf8_lossy(&out.stdout);
    t.trim().split(':').fold(0.0, |acc, part| {
        acc * 60.0 + part.parse::<f64>().unwrap_or(0.0)
    }) * 1e3
}

/// The one-minute load average.
fn load1() -> f64 {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .expect("sysctl runs");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(f64::NAN)
}

fn tables(sim: &ceteris_core::sim::Sim<EconWorld>) -> Vec<(String, usize)> {
    let w = sim.world();
    vec![
        ("ledger_accounts".into(), w.ledger.n_accounts()),
        ("snapshots".into(), sim.snapshot_hashes().len()),
    ]
}

fn main() {
    let years: usize = std::env::var("FLATNESS_YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let (inputs, source) = inputs();
    let trade = inputs.trade_params();
    // FLATNESS_SPANS=<path>: per-year wall time per span name, as CSV (year,name,ms)
    let spans_out = std::env::var_os("FLATNESS_SPANS").map(PathBuf::from);
    if spans_out.is_some() {
        timing::enable();
    }
    let mut span_ms = std::collections::BTreeMap::<(usize, &'static str), f64>::new();
    let mut sim = EconWorld::sim(Arc::new(inputs), EconParams::default(), trade);
    let mut secs = Vec::with_capacity(years * TICKS_PER_YEAR);
    let mut decades = vec![Decade {
        year: 0,
        rows: tables(&sim),
    }];
    let mut cpu0 = if spans_out.is_some() { cpu_ms() } else { 0.0 };
    for year in 1..=years {
        let wall0 = Instant::now();
        for _ in 0..TICKS_PER_YEAR {
            let t = Instant::now();
            sim.step().expect("invariants hold");
            secs.push(t.elapsed().as_secs_f64());
            for sp in timing::take() {
                *span_ms.entry((year, sp.name)).or_default() += (sp.end - sp.start) as f64 / 1e6;
            }
        }
        if spans_out.is_some() {
            let cpu1 = cpu_ms();
            span_ms.insert((year, "_wall"), wall0.elapsed().as_secs_f64() * 1e3);
            span_ms.insert((year, "_cpu"), cpu1 - cpu0);
            span_ms.insert((year, "_load"), load1());
            cpu0 = cpu1;
        }
        if year % 10 == 0 {
            decades.push(Decade {
                year: year as u32,
                rows: tables(&sim),
            });
        }
    }

    if let Some(path) = &spans_out {
        let mut csv = String::from("year,name,ms\n");
        for ((y, n), ms) in &span_ms {
            writeln!(csv, "{y},{n},{ms:.3}").unwrap();
        }
        std::fs::write(path, csv).expect("spans written");
    }

    let window = 5 * TICKS_PER_YEAR;
    let ratio = if years >= 10 {
        flatness::ratio(&secs, TICKS_PER_YEAR)
    } else {
        f64::NAN
    };
    // one snapshot per SNAPSHOT_EVERY ticks, kept until the history tiers cap them (0236)
    let snapshot_bound = years * TICKS_PER_YEAR / SNAPSHOT_EVERY as usize + 1;
    let failures = flatness::check(ratio, &decades, &[("snapshots", snapshot_bound)]);

    let mut out = String::new();
    writeln!(out, "# Flatness report ({source}, {years} years)\n").unwrap();
    if years >= 10 {
        let first = flatness::median(&secs[..window]);
        let last = flatness::median(&secs[secs.len() - window..]);
        writeln!(
            out,
            "Median tick years 1 to 5: {:.2} ms; last five years: {:.2} ms; ratio {ratio:.3}.\n",
            first * 1e3,
            last * 1e3
        )
        .unwrap();
    }
    match peak_rss_kib() {
        Some(k) => writeln!(out, "Peak resident memory: {:.1} MiB.\n", k as f64 / 1024.0).unwrap(),
        None => writeln!(
            out,
            "Peak resident memory: not available on this platform.\n"
        )
        .unwrap(),
    }
    writeln!(
        out,
        "| Year | {} |",
        decades[0]
            .rows
            .iter()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    )
    .unwrap();
    writeln!(out, "|---|{}", "---|".repeat(decades[0].rows.len())).unwrap();
    for d in &decades {
        writeln!(
            out,
            "| {} | {} |",
            d.year,
            d.rows
                .iter()
                .map(|r| r.1.to_string())
                .collect::<Vec<_>>()
                .join(" | ")
        )
        .unwrap();
    }
    writeln!(
        out,
        "\nFailures: {}",
        if failures.is_empty() {
            "none".into()
        } else {
            format!("{failures:?}")
        }
    )
    .unwrap();
    print!("{out}");
    let report = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/flatness.md");
    std::fs::create_dir_all(report.parent().unwrap()).unwrap();
    std::fs::write(report, &out).expect("report written");
    assert!(failures.is_empty(), "flatness gate failed");
}
