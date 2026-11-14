//! `ceteris export|run|pack`: Arrow export of a P2 world and of a run from its data state, and
//! pack tools.
//!
//! ```text
//! ceteris pack compare A B [--iterative] [--table NAME]
//! ceteris pack check-sources --profile research|release
//! ceteris pack fetch [--record]
//! ceteris pack record-counties
//! ceteris pack build [--out DIR]
//! ceteris pack subset --pack DIR [--config FILE] (--out DIR | --schema)
//! ceteris pack build --econ-only [--pack DIR] [--out DIR]
//! ceteris reference solve --inputs DIR --out DIR
//! ceteris calibrate --pack DIR [--no-border-fit] [--storage] [--fit-cache DIR] | --inputs DIR [--out DIR]
//! ceteris calibrate stability [--storage] [FIXTURE_DIR]
//! ceteris calibrate rho (--panel FILE | --baci ZIP --hs-key XLSX --tariffs CSV|--gtd-war CSV [--lines ISO3] [--write-panel FILE])
//!     [--base Y] [--shock Y]
//! ceteris gravity [--pack DIR] [--write] [--out FILE]
//! ceteris air-share [--pack DIR] [--data DIR] [--set pooled|us|eu] [--profile]
//! ceteris export --fixture|--pack DIR [--storage] --out DIR
//! ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--no-drift] [--from-equilibrium DIR]
//!     [--data-state] [--perturb SEED] [--history [--history-every N] [--history-format arrow|csv]]
//!     [--timing] [--inject-halt TICK] [--price-anchor off|e1|e3] --out DIR
//! ceteris replay BUNDLE [--out DIR]
//! ```
//!
//! A halt (an invariant violation, or `--inject-halt TICK`, which halts a healthy run at that
//! tick) writes a repro bundle, `OUT/bundle/bundle.txt` (ADR 0214): the engine version, the
//! arguments of the run and the halt record. `replay` runs it again and exits 0 when the same
//! halt recurs.
//!
//! `--pack` loads the 2000 world of a built pack (`EconInputs::from_pack`) instead of the
//! generated fixture: `run` starts from its calibrated steady state when the pack holds the
//! calibration tables (ADR 0089), `export` writes its uncalibrated data state, which the
//! calibration starts from. `--from-equilibrium` starts the run from a GE reference solution
//! (`ceteris reference solve --out DIR`) instead of the data state. `--data-state`
//! starts a calibrated pack from the data state of its calibrated inputs (prices 1, base
//! wages and employment, one month of base use in stock, nothing in flight) instead of the
//! steady state. `--perturb SEED` moves the start by `EconWorld::perturb` (10%, ADR 0089) and
//! prints the factors' range. `--no-drift` turns the initial drift off (S23, ADR 0103), as the
//! P2 gate runs. `--history` also writes the per-tick series of
//! `ceteris_econ::history`, every N ticks (default 1), as Arrow with zstd compression or, with
//! `--history-format csv`, as CSV. `--timing` records wall-time spans (`ceteris_core::timing`:
//! the tick, each system, the T2b phases and each good's T2b task) into `timing.csv` and prints
//! each span's median per tick; the run's state is the same as without it (ADR 0294).
//! `--price-anchor` picks the price rule (`ceteris_calibrate::benchmark::price_anchor`): `off`
//! (the default), `e1` the anchored rule (ADRs 0338, 0340) or `e3` with ADR 0343's package.
//!
//! `reference solve` solves the GE reference of a `ceteris export` directory and writes it with
//! the column layout of the run export (`prices`, `wages`, `enterprises`, `flows`, `home`) plus
//! `solution.json`, printing the solve's summary.
//!
//! `calibrate --pack DIR` exports the pack's data state to a temporary directory, calibrates it
//! as the steady state (ADR 0089) at the pack's u* and writes the six calibration tables into
//! `--out` (default `target/calibration/<pack name>`, never the pack), printing one line per
//! pass and the summary. Copying the tables into the pack installs them: `EconInputs::from_pack`
//! loads them at their `calibration_format` and the static digest of the export they were
//! solved from. Before the steady state it fits the border friction per friction group to ICIO's
//! cross-border shares (ADRs 0092, 0276) and reports the shares at the steady state;
//! `--no-border-fit` keeps the default instead, for comparisons, and reads no ICIO shares.
//! `--storage` exports and calibrates under the approved storage values
//! (`TradeParams::with_storage`): the tables' static digest records it, and a run of a pack
//! holding them has storage on (RFC 0286 screen). `export --pack DIR --storage` writes the
//! export such a calibration is solved from, the V1 reference's inputs.
//! `--inputs DIR` calibrates an export directory at its own u* and border friction, its
//! provinces serving as their own pack provinces, and needs `--out` (not `--no-border-fit`).
//!
//! `calibrate stability` prints the stability table of the stock-flow price rule for the
//! fixture in FIXTURE_DIR (default `tests/golden/p2-fixture`); `--storage` prints the storage
//! loop's table instead: per storable good the kappa1 limit of the storage loop and the psi
//! limit, at the fixture's storage values or, where storage is off, the approved ones (ADR 0091).
//!
//! `gravity` estimates theta by class, the border friction and the mode cost ratios by PPML
//! structural gravity on 2000 BACI and the pack's least-cost costs (`--pack`, default
//! `data/packs/2000`; the sources from the fetch cache), printing the table of estimates, the
//! sample counts and any stop rule that fires; it exits 1 when one fires. `--write` writes the
//! estimates as TOML into `--out` (default `crates/ceteris-pack/config/gravity_2000.toml`, beside
//! the pack configuration) when no stop rule fires.
//!
//! `air-share` fits P04d to the air share of 2000 imports by importer, origin and traded good
//! (ADR 0277) through the kernel's gateway-pair sum on the pack (`--pack`): US 2000, EU15 2002
//! hindcast to 2000, or both pooled (`--set`, default pooled), from the files in `--data`
//! (default `data/raw/air-shares-2000`), printing the readers' notes and each outer step. It
//! exits 1 when the fit stops.

/// mimalloc as global allocator (ADR 0297): allocation never touches results.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use ceteris_calibrate::steady::Limits;
use ceteris_calibrate::summary;
use ceteris_calibrate::write as calibration;
use ceteris_calibrate::{benchmark, border, epsilon, gravity};
use ceteris_core::invariant::Violation;
use ceteris_core::rng::Seed;
use ceteris_core::sim::Sim;
use ceteris_core::timing;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::export::{write_inputs, write_run};
use ceteris_econ::fixture::small_world;
use ceteris_econ::history::{Format, run_recorded};
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_pack::compare::{self, Tolerance};
use ceteris_pack::econ;
use ceteris_pack::fetch;
use ceteris_pack::json::Json;
use ceteris_pack::sources::{self, Profile};
use ceteris_reference::model::Model;
use ceteris_reference::{io as reference_io, output, solve};
use ceteris_trade::{Kernel, PriceAnchor, TradeParams};

const USAGE: &str = "usage:
  ceteris pack compare A B [--iterative] [--table NAME]
  ceteris pack check-sources [--profile research|release]
  ceteris setup [--from DIR]
  ceteris pack fetch [--record]
  ceteris pack record-counties
  ceteris pack build [--out DIR]
  ceteris pack subset --pack DIR [--config FILE] (--out DIR | --schema)
  ceteris pack build --econ-only [--pack DIR] [--out DIR]
  ceteris reference solve --inputs DIR --out DIR
  ceteris calibrate --pack DIR [--no-border-fit] [--storage] [--fit-cache DIR] | --inputs DIR [--out DIR]
  ceteris calibrate stability [--storage] [FIXTURE_DIR]
  ceteris calibrate rho (--panel FILE | --baci ZIP --hs-key XLSX --tariffs CSV|--gtd-war CSV [--lines ISO3] [--write-panel FILE])
    [--base YEAR] [--shock YEAR]
  ceteris gravity [--pack DIR] [--write] [--out FILE]
  ceteris air-share [--pack DIR] [--data DIR] [--set pooled|us|eu] [--profile]
  ceteris export --fixture|--pack DIR [--storage] --out DIR
  ceteris ensemble FILE --branch B [--baseline A] --ticks N --fixture|--pack DIR --out DIR
  ceteris effects FILE --branch B [--baseline A] --ticks N --fixture|--pack DIR --out DIR
  ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--no-drift] [--from-equilibrium DIR]
    [--data-state] [--perturb SEED] [--history [--history-every N] [--history-format arrow|csv]]
    [--timing] [--inject-halt TICK] [--price-anchor off|e1|e3] --out DIR
  ceteris replay BUNDLE [--out DIR]";

/// Size of the `--perturb` draw: the 10% of ADR 0089's V1 test.
const PERTURBATION: f64 = 0.1;

/// Where the inputs of a world come from.
enum Source {
    Fixture,
    Pack(PathBuf),
}

impl Source {
    /// The world's inputs: calibrated when the pack holds calibration tables, or the data
    /// state (`calibrated` false, as `export` writes it for the calibration).
    fn load(&self, calibrated: bool) -> Result<EconInputs, String> {
        match self {
            Source::Fixture => Ok(small_world()),
            Source::Pack(dir) => {
                let pack =
                    Pack::load(dir, &TransportParams::default()).map_err(|e| e.to_string())?;
                let inputs = if calibrated {
                    EconInputs::from_pack(&pack, dir)
                } else {
                    EconInputs::from_pack_data(&pack, dir)
                };
                inputs.map_err(|e| e.to_string())
            }
        }
    }
}

struct Args {
    source: Source,
    out: PathBuf,
    ticks: Option<u32>,
    frictionless: bool,
    no_drift: bool,
    equilibrium: Option<PathBuf>,
    data_state: bool,
    perturb: Option<u64>,
    history: bool,
    history_every: Option<u32>,
    history_format: Option<Format>,
    timing: bool,
    inject_halt: Option<u32>,
    anchor: Option<PriceAnchor>,
    storage: bool,
    /// The arguments without `--out`, as a bundle records them.
    argv: Vec<String>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let all: Vec<String> = args.collect();
    let mut argv = Vec::new();
    let mut it = all.iter();
    while let Some(a) = it.next() {
        if a == "--out" {
            it.next();
        } else {
            argv.push(a.clone());
        }
    }
    let mut args = all.into_iter();
    let mut inject_halt = None;
    let (mut source, mut out, mut ticks, mut frictionless) = (None, None, None, false);
    let (mut equilibrium, mut no_drift) = (None, false);
    let (mut data_state, mut perturb) = (false, None);
    let (mut history, mut history_every, mut history_format) = (false, None, None);
    let mut timing = false;
    let mut anchor = None;
    let mut storage = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fixture" => source = Some(Source::Fixture),
            "--pack" => {
                let dir = args.next().ok_or("--pack needs a directory")?;
                source = Some(Source::Pack(PathBuf::from(dir)));
            }
            "--frictionless" => frictionless = true,
            "--no-drift" => no_drift = true,
            "--from-equilibrium" => {
                let dir = args.next().ok_or("--from-equilibrium needs a directory")?;
                equilibrium = Some(PathBuf::from(dir));
            }
            "--data-state" => data_state = true,
            "--perturb" => {
                let n = args.next().ok_or("--perturb needs a seed")?;
                perturb = Some(n.parse().map_err(|_| format!("bad seed '{n}'"))?);
            }
            "--price-anchor" => {
                let name = args.next().ok_or("--price-anchor needs off, e1 or e3")?;
                anchor = benchmark::price_anchor(&name)?;
            }
            "--storage" => storage = true,
            "--history" => history = true,
            "--timing" => timing = true,
            "--inject-halt" => {
                let n = args.next().ok_or("--inject-halt needs a tick")?;
                inject_halt = Some(n.parse().map_err(|_| format!("bad tick '{n}'"))?);
            }
            "--history-every" => {
                let n = args.next().ok_or("--history-every needs a count")?;
                match n.parse::<u32>() {
                    Ok(v) if v > 0 => history_every = Some(v),
                    _ => return Err(format!("bad sampling interval '{n}'")),
                }
            }
            "--history-format" => {
                history_format = Some(match args.next().as_deref() {
                    Some("arrow") => Format::Arrow,
                    Some("csv") => Format::Csv,
                    _ => return Err("--history-format needs arrow or csv".into()),
                });
            }
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            "--ticks" => {
                let n = args.next().ok_or("--ticks needs a count")?;
                ticks = Some(n.parse().map_err(|_| format!("bad tick count '{n}'"))?);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(Args {
        source: source.ok_or("choose an input with --fixture or --pack")?,
        out: out.ok_or("--out is required")?,
        ticks,
        frictionless,
        no_drift,
        equilibrium,
        data_state,
        perturb,
        history,
        history_every,
        history_format,
        timing,
        inject_halt,
        anchor,
        storage,
        argv,
    })
}

fn export(args: &Args) -> Result<(), String> {
    if args.ticks.is_some()
        || args.frictionless
        || args.no_drift
        || args.equilibrium.is_some()
        || args.data_state
        || args.perturb.is_some()
        || args.history
        || args.history_every.is_some()
        || args.history_format.is_some()
        || args.timing
        || args.inject_halt.is_some()
        || args.anchor.is_some()
    {
        return Err(
            "--ticks, --frictionless, --no-drift, --from-equilibrium, --data-state, --perturb, --history, \
             --history-every, --history-format, --timing, --inject-halt and --price-anchor belong to run"
                .into(),
        );
    }
    if let Source::Pack(dir) = &args.source {
        return ceteris_calibrate::export_pack(dir, &args.out, args.storage);
    }
    if args.storage {
        return Err("--storage needs --pack".into());
    }
    let inputs = args.source.load(false)?;
    let (params, trade) = (EconParams::default(), TradeParams::default());
    let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
    write_inputs(&args.out, &inputs, &kernel, &params, &trade).map_err(|e| e.to_string())
}

enum RunFailure {
    Halt(Violation),
    Other(String),
}

impl From<String> for RunFailure {
    fn from(e: String) -> Self {
        Self::Other(e)
    }
}

impl From<&str> for RunFailure {
    fn from(e: &str) -> Self {
        Self::Other(e.into())
    }
}

/// Runs, and on a halt writes the repro bundle beside the run's output.
fn run(args: &Args) -> Result<(), String> {
    match run_core(args) {
        Ok(()) => Ok(()),
        Err(RunFailure::Other(e)) => Err(e),
        Err(RunFailure::Halt(v)) => {
            let dir = args.out.join("bundle");
            write_bundle(&dir, &args.argv, &v)?;
            Err(format!("halted, bundle in {}: {v}", dir.display()))
        }
    }
}

const BUNDLE_FILE: &str = "bundle.txt";

fn write_bundle(dir: &Path, argv: &[String], v: &Violation) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut text = format!("ceteris-bundle 1\nengine={}\n", env!("CARGO_PKG_VERSION"));
    for a in argv {
        text.push_str(&format!("arg={}\n", a.replace('\n', " ")));
    }
    text.push_str(&format!(
        "halt-tick={}\nhalt-invariant={}\nhalt-detail={}\n",
        v.tick,
        v.invariant,
        v.detail.replace('\n', " ")
    ));
    let file = dir.join(BUNDLE_FILE);
    std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))
}

/// `replay BUNDLE [--out DIR]`: exit 0 when the bundle's halt recurs, 1 otherwise.
fn replay(args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut args = args;
    let (mut bundle, mut out) = (None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            _ if bundle.is_none() => bundle = Some(PathBuf::from(a)),
            _ => return Err(format!("unknown argument '{a}'")),
        }
    }
    let bundle = bundle.ok_or("replay needs a bundle directory")?;
    let file = if bundle.is_dir() {
        bundle.join(BUNDLE_FILE)
    } else {
        bundle
    };
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut lines = text.lines();
    if lines.next() != Some("ceteris-bundle 1") {
        return Err(format!("{}: not a version 1 bundle", file.display()));
    }
    let (mut argv, mut halt) = (Vec::new(), [None, None, None]);
    for line in lines {
        match line.split_once('=') {
            Some(("arg", v)) => argv.push(v.to_string()),
            Some(("halt-tick", v)) => halt[0] = Some(v.to_string()),
            Some(("halt-invariant", v)) => halt[1] = Some(v.to_string()),
            Some(("halt-detail", v)) => halt[2] = Some(v.to_string()),
            _ => {}
        }
    }
    let [Some(tick), Some(invariant), Some(detail)] = halt else {
        return Err(format!("{}: no halt record", file.display()));
    };
    let scratch;
    let out = match out {
        Some(o) => o,
        None => {
            scratch = tempfile::tempdir().map_err(|e| format!("temporary directory: {e}"))?;
            scratch.path().to_owned()
        }
    };
    argv.extend(["--out".into(), out.display().to_string()]);
    match run_core(&parse(argv.into_iter())?) {
        Err(RunFailure::Halt(v))
            if v.tick.to_string() == tick
                && v.invariant == invariant
                && v.detail.replace('\n', " ") == detail =>
        {
            println!("reproduced: {v}");
            Ok(ExitCode::SUCCESS)
        }
        Err(RunFailure::Halt(v)) => {
            eprintln!("different halt: {v}");
            Ok(ExitCode::FAILURE)
        }
        Err(RunFailure::Other(e)) => Err(e),
        Ok(()) => {
            eprintln!("no halt: the run completed");
            Ok(ExitCode::FAILURE)
        }
    }
}

fn run_core(args: &Args) -> Result<(), RunFailure> {
    let ticks = args.ticks.ok_or("--ticks is required")?;
    if args.storage {
        return Err(
            "--storage belongs to export: a run takes storage from the pack's calibration".into(),
        );
    }
    if args.history_every.is_some() && !args.history {
        return Err("--history-every needs --history".into());
    }
    if args.history_format.is_some() && !args.history {
        return Err("--history-format needs --history".into());
    }
    let params = EconParams {
        drift: !args.no_drift,
        ..if args.frictionless {
            EconParams::frictionless()
        } else {
            EconParams::default()
        }
    };
    if args.data_state && args.equilibrium.is_some() {
        return Err("--data-state and --from-equilibrium choose different starts".into());
    }
    if args.data_state && matches!(args.source, Source::Fixture) {
        return Err("--data-state needs --pack: the fixture has no calibrated steady state".into());
    }
    let mut inputs = args.source.load(true)?;
    if args.data_state && inputs.steady.take().is_none() {
        return Err(
            "--data-state needs a calibrated pack: this one has no calibration tables and \
             starts from its data state already"
                .into(),
        );
    }
    let trade = TradeParams {
        anchor: args.anchor,
        ..inputs.trade_params()
    };
    let inputs = Arc::new(inputs);
    let mut world = match &args.equilibrium {
        Some(dir) => {
            let solution = Equilibrium::read(dir).map_err(|e| e.to_string())?;
            EconWorld::from_equilibrium(inputs, params, trade, &solution)
        }
        None => EconWorld::new(inputs, params, trade),
    };
    if let Some(seed) = args.perturb {
        let f = world.perturb(seed, PERTURBATION);
        let (lo, hi) = f.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), &x| {
            (lo.min(x), hi.max(x))
        });
        let mean = f.iter().sum::<f64>() / f.len() as f64;
        println!(
            "perturbed {} values (seed {seed}, size {PERTURBATION}): factors {lo} to {hi}, mean {mean}",
            f.len()
        );
    }
    let mut sim = Sim::new(world, schedule(), SEED);
    if args.timing {
        timing::enable();
    }
    // An injected halt stops the healthy run at its tick; later ticks are never run.
    let stop = args.inject_halt.map_or(ticks, |h| h.min(ticks));
    if args.history {
        let history = run_recorded(&mut sim, stop, args.history_every.unwrap_or(1))
            .map_err(RunFailure::Halt)?;
        inject(args, stop)?;
        write_run(&args.out, sim.world()).map_err(|e| e.to_string())?;
        history
            .write(&args.out, args.history_format.unwrap_or(Format::Arrow))
            .map_err(|e| e.to_string())?;
    } else {
        sim.run_until(stop).map_err(RunFailure::Halt)?;
        inject(args, stop)?;
        write_run(&args.out, sim.world()).map_err(|e| e.to_string())?;
    }
    if args.timing {
        write_timing(&args.out, &timing::take()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn inject(args: &Args, tick: u32) -> Result<(), RunFailure> {
    match args.inject_halt {
        Some(h) if h == tick => Err(RunFailure::Halt(Violation::new(
            tick,
            "injected",
            "--inject-halt".into(),
        ))),
        _ => Ok(()),
    }
}

/// `timing.csv`, one row per span, and each span's median per tick in ms on stdout: spans of
/// one name in one tick add up, except `t2b.good`, which is the tick's slowest good.
fn write_timing(out: &Path, spans: &[timing::Span]) -> std::io::Result<()> {
    let mut csv = String::from("tick,name,good,thread,start_ns,end_ns\n");
    let mut per_tick: BTreeMap<(&str, u32), f64> = BTreeMap::new();
    for s in spans {
        let field = |v: u32| {
            if v == timing::NONE {
                String::new()
            } else {
                v.to_string()
            }
        };
        csv += &format!(
            "{},{},{},{},{},{}\n",
            s.tick,
            s.name,
            field(s.good),
            field(s.thread),
            s.start,
            s.end
        );
        let ms = (s.end - s.start) as f64 / 1e6;
        let slot = per_tick.entry((s.name, s.tick)).or_default();
        *slot = if s.name == "t2b.good" {
            slot.max(ms)
        } else {
            *slot + ms
        };
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("timing.csv"), csv)?;
    let mut by_name: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for ((name, _), ms) in per_tick {
        by_name.entry(name).or_default().push(ms);
    }
    for (name, mut ms) in by_name {
        ms.sort_by(f64::total_cmp);
        println!(
            "{name:18} median {:8.3} ms over {} ticks",
            ms[ms.len() / 2],
            ms.len()
        );
    }
    Ok(())
}

/// `pack check-sources [--profile research|release]`: exit 1 when the manifest has problems.
fn check_sources(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut profile = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--profile" => {
                let name = args.next().ok_or("--profile needs research or release")?;
                profile =
                    Some(Profile::parse(&name).ok_or("--profile must be research or release")?);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let report = sources::check(&sources::workspace_root(), profile)?;
    for problem in &report.problems {
        eprintln!("{problem}");
    }
    println!("{}", report.summary());
    Ok(if report.problems.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// `pack fetch [--record]`: exit 1 when a file is unpinned or does not match its pin.
fn fetch_sources(args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut record = false;
    for a in args {
        match a.as_str() {
            "--record" => record = true,
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    // Failures carry the message Python printed; no usage text follows them.
    match fetch::run(&sources::workspace_root(), record) {
        Ok(true) => Ok(ExitCode::SUCCESS),
        Ok(false) => Ok(ExitCode::FAILURE),
        Err(message) => {
            eprintln!("{message}");
            Ok(ExitCode::FAILURE)
        }
    }
}

/// `setup [--from DIR]`: the first-run path (ADR 0215): lists the sources, checks the disk,
/// downloads what is missing and takes manual files from DIR. Exit 1 until every file verifies.
fn setup_verb(args: Vec<String>) -> Result<ExitCode, String> {
    let from = match args.as_slice() {
        [] => None,
        [flag, dir] if flag == "--from" => Some(PathBuf::from(dir)),
        _ => return Err("usage: ceteris setup [--from DIR]".into()),
    };
    let done = ceteris_pack::setup::run(
        &sources::workspace_root(),
        from.as_deref(),
        ceteris_pack::setup::BUILD_BYTES,
        &mut std::io::stdout(),
    )?;
    Ok(if done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// `pack record-counties`: rewrites `counties.lock.toml` from the geoBoundaries API, downloading
/// the chosen files into the cache.
fn record_counties(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    if let Some(a) = args.next() {
        return Err(format!("unknown argument '{a}'"));
    }
    ceteris_pack::geo::counties::record(&sources::workspace_root())?;
    Ok(ExitCode::SUCCESS)
}

/// `pack subset --pack DIR [--config FILE] --out DIR`: the small CI pack of RFC 0353 from an
/// uncalibrated pack. `--schema` instead prints every table and column of DIR and exits 1 when
/// the config has no aggregation rule for one.
fn subset(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let (mut pack, mut config, mut out, mut schema) = (None, None, None, false);
    while let Some(a) = args.next() {
        let mut dir = |what: &str| {
            args.next()
                .map(PathBuf::from)
                .ok_or(format!("{what} needs a value"))
        };
        match a.as_str() {
            "--pack" => pack = Some(dir("--pack")?),
            "--config" => config = Some(dir("--config")?),
            "--out" => out = Some(dir("--out")?),
            "--schema" => schema = true,
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let pack = pack.ok_or("pack subset needs --pack")?;
    let config = config.unwrap_or_else(|| {
        sources::workspace_root().join("crates/ceteris-pack/config/subset_ci.toml")
    });
    let text =
        std::fs::read_to_string(&config).map_err(|e| format!("{}: {e}", config.display()))?;
    let config = ceteris_pack::subset::Config::parse(&text)?;
    if schema {
        let (lines, missing) = ceteris_pack::subset::schema(&pack, &config)?;
        lines.iter().for_each(|l| println!("{l}"));
        missing.iter().for_each(|m| eprintln!("no rule: {m}"));
        return Ok(if missing.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }
    let out = out.ok_or("pack subset needs --out or --schema")?;
    let report = ceteris_pack::subset::subset(&pack, &config, &out)?;
    println!(
        "{} provinces, {} countries, {} tables, {} bytes",
        report.provinces,
        report.countries,
        report.tables.len(),
        report.bytes
    );
    Ok(ExitCode::SUCCESS)
}

/// `pack build [--out DIR]`: the whole pack from the fetch cache into DIR (default
/// `data/packs/<year>`), on the owner-side `geos` feature; prints the build stats.
///
/// `pack build --econ-only [--pack DIR] [--out DIR]`: the econ tables of a pack's P1 tables, from
/// the fetch cache; prints the summaries to stderr, then each table's SHA-256 and the build
/// stats as JSON.
fn build(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let (mut econ_only, mut pack, mut out) = (false, None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--econ-only" => econ_only = true,
            "--pack" => {
                pack = Some(PathBuf::from(
                    args.next().ok_or("--pack needs a directory")?,
                ))
            }
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    if !econ_only {
        if pack.is_some() {
            return Err("--pack needs --econ-only".into());
        }
        return build_pack(out);
    }
    let pack = match pack {
        Some(pack) => pack,
        None => ceteris_pack::build::default_out(&sources::workspace_root())?,
    };
    let out = out.unwrap_or_else(|| pack.clone());
    let cache = sources::workspace_root().join(fetch::CACHE_DIR);
    let report = econ::build::build(&pack, &out, &cache)?;
    let tables = Json::object(
        report
            .tables
            .iter()
            .map(|(name, d)| (name, Json::Str(ceteris_pack::table::hex(d)))),
    );
    for line in &report.summary {
        eprintln!("{line}");
    }
    println!(
        "{}",
        Json::object([("tables", tables), ("stats", report.stats.json())]).text()
    );
    Ok(ExitCode::SUCCESS)
}

#[cfg(feature = "geos")]
fn build_pack(out: Option<PathBuf>) -> Result<ExitCode, String> {
    let out = match out {
        Some(out) => out,
        None => ceteris_pack::build::default_out(&sources::workspace_root())?,
    };
    let built = ceteris_pack::build::build(&out)?;
    println!("{}", built.stats.pretty());
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(feature = "geos"))]
fn build_pack(_: Option<PathBuf>) -> Result<ExitCode, String> {
    Err("pack build needs the owner-side `geos` feature, or --econ-only".into())
}

/// `pack compare A B [--iterative] [--table NAME]`: exit 1 when the directories differ.
fn pack(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    match args.next().as_deref() {
        Some("compare") => {}
        Some("check-sources") => return check_sources(args),
        Some("fetch") => return fetch_sources(args),
        Some("record-counties") => return record_counties(args),
        Some("build") => return build(args),
        Some("subset") => return subset(args),
        _ => {
            return Err(
                "expected a pack command, compare, check-sources, fetch, record-counties, build or subset"
                    .into(),
            );
        }
    }
    let (mut dirs, mut tolerance, mut only) = (Vec::new(), Tolerance::DEFAULT, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--iterative" => tolerance = Tolerance::ITERATIVE,
            "--table" => only = Some(args.next().ok_or("--table needs a name")?),
            other if other.starts_with("--") => return Err(format!("unknown argument '{other}'")),
            dir => dirs.push(PathBuf::from(dir)),
        }
    }
    let [a, b] = dirs.as_slice() else {
        return Err("pack compare needs two directories".into());
    };
    let report = compare::tables_named(a, b, tolerance, only.as_deref())?;
    for line in report.lines() {
        println!("{line}");
    }
    Ok(if report.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// `reference solve --inputs DIR --out DIR`: exit 1 without an equilibrium.
fn reference(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    if args.next().as_deref() != Some("solve") {
        return Err("expected a reference command, solve".into());
    }
    let (mut inputs, mut out) = (None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--inputs" => {
                inputs = Some(PathBuf::from(
                    args.next().ok_or("--inputs needs a directory")?,
                ))
            }
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let inputs = inputs.ok_or("--inputs is required")?;
    let out = out.ok_or("--out is required")?;
    let inp = reference_io::load(&inputs)?;
    let model = Model::new(&inp)?;
    let eq = match solve::equilibrium(&model, None, solve::TOL, None) {
        Ok(eq) => eq,
        Err(e) => {
            eprintln!("no equilibrium: {e}");
            return Ok(ExitCode::FAILURE);
        }
    };
    output::write(&out, &inp, &eq)?;
    println!(
        "{}",
        output::json_object(&output::info_json(&eq.info, true))
    );
    Ok(ExitCode::SUCCESS)
}

/// `calibrate stability [--storage] [FIXTURE_DIR]`.
fn stability(args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut args = args.peekable();
    let storage = args.next_if(|a| a == "--storage").is_some();
    let fixture = args.next().map_or_else(
        || sources::workspace_root().join("tests/golden/p2-fixture"),
        PathBuf::from,
    );
    if let Some(a) = args.next() {
        return Err(format!("unknown argument '{a}'"));
    }
    let rows = if storage {
        ceteris_calibrate::stability::storage_table(&fixture)?
    } else {
        ceteris_calibrate::stability::table(&fixture)?
    };
    for row in rows {
        println!("{row}");
    }
    Ok(ExitCode::SUCCESS)
}

/// `calibrate rho (--panel FILE | --baci ZIP --hs-key XLSX --tariffs CSV|--gtd-war CSV [--lines ISO3] [--write-panel FILE])
/// [--base YEAR] [--shock YEAR]`: P53 from local projections of a class, pair, year, flow, tariff
/// panel (ADR 0027), read from a file or extracted from BACI and an exporter, importer, year,
/// class, tariff file.
fn rho(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    use ceteris_calibrate::rho::{Design, estimate, parse_panel, report};
    use ceteris_calibrate::rho_panel::{Tariffs, extract, hs_classes, years};
    let (mut panel, mut baci, mut hs_key, mut tariffs, mut write_panel) =
        (None, None, None, None, None);
    let (mut gtd_war, mut lines) = (None, None::<String>);
    let mut design = Design {
        base: 2017,
        shock: 2019,
    };
    while let Some(a) = args.next() {
        let mut year = |name: &str| {
            args.next()
                .and_then(|v| v.parse::<i32>().ok())
                .ok_or(format!("{name} needs a year"))
        };
        match a.as_str() {
            "--panel" => panel = args.next().map(PathBuf::from),
            "--baci" => baci = args.next().map(PathBuf::from),
            "--hs-key" => hs_key = args.next().map(PathBuf::from),
            "--tariffs" => tariffs = args.next().map(PathBuf::from),
            "--gtd-war" => gtd_war = args.next().map(PathBuf::from),
            "--lines" => lines = args.next(),
            "--write-panel" => write_panel = args.next().map(PathBuf::from),
            "--base" => design.base = year("--base")?,
            "--shock" => design.shock = year("--shock")?,
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let read =
        |p: &PathBuf| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
    let rows = match (panel, baci, hs_key, tariffs.is_some() != gtd_war.is_some()) {
        (Some(p), None, None, false) => parse_panel(&read(&p)?)?,
        (None, Some(b), Some(k), true) => {
            let (classes, years) = (hs_classes(&b, &k)?, years(design, 2024));
            let tariffs = match (&tariffs, &gtd_war) {
                (Some(t), _) => Tariffs::parse(&read(t)?)?,
                (_, Some(w)) => Tariffs::from_gtd_war(w, &classes, &years, lines.is_some())?,
                _ => unreachable!(),
            };
            let rows = extract(&b, &classes, &tariffs, &years, lines.as_deref())?;
            if let Some(out) = write_panel {
                let mut text = String::from("class,pair,year,flow,tariff\n");
                for r in &rows {
                    text += &format!(
                        "{},{},{},{},{}\n",
                        r.class, r.pair, r.year, r.flow, r.tariff
                    );
                }
                std::fs::write(&out, text).map_err(|e| format!("{}: {e}", out.display()))?;
            }
            rows
        }
        _ => {
            return Err(
                "give --panel, or --baci, --hs-key and one of --tariffs or --gtd-war together"
                    .into(),
            );
        }
    };
    let (fits, skipped) = estimate(&rows, design);
    print!("{}", report(&fits, &skipped));
    Ok(if fits.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// `gravity [--pack DIR] [--write] [--out FILE]`: exit 1 when a stop rule fires.
fn gravity(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let root = sources::workspace_root();
    let (mut pack, mut write, mut out) = (root.join("data/packs/2000"), false, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--pack" => pack = PathBuf::from(args.next().ok_or("--pack needs a directory")?),
            "--write" => write = true,
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a file")?)),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let out = out.unwrap_or_else(|| root.join(sources::CONFIG_DIR).join("gravity_2000.toml"));
    let est = gravity::estimate(&pack, &root.join(fetch::CACHE_DIR))?;
    println!("{}", gravity::report(&est));
    println!("{}", est.notes.json().pretty());
    let fired = gravity::stop_rules(&est);
    for f in &fired {
        println!("stop rule: {f}");
    }
    if write && gravity::write_toml(&est, &out)? {
        println!("wrote {}", out.display());
    }
    Ok(if fired.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// `air-share [--pack DIR] [--data DIR] [--set pooled|us|eu] [--profile]`: the path-consistent
/// estimate of P04d / P04a (ADR 0277), or with `--profile` the grid it starts from.
fn air_share(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    use ceteris_calibrate::air_share::{
        HS_1996, HS_2002, US_2000, US_2002, estimate, eu_hindcast, pack_countries, profile, trend,
        us_imports,
    };
    use ceteris_calibrate::costs::Transport;
    use ceteris_calibrate::gravity::N_TRADED;
    let root = sources::workspace_root();
    let (mut pack, mut data) = (
        root.join("data/packs/2000"),
        root.join("data/raw/air-shares-2000"),
    );
    let mut set = "pooled".to_owned();
    let mut want_profile = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--profile" => want_profile = true,
            "--pack" => pack = PathBuf::from(args.next().ok_or("--pack needs a directory")?),
            "--data" => data = PathBuf::from(args.next().ok_or("--data needs a directory")?),
            "--set" => set = args.next().ok_or("--set needs pooled, us or eu")?,
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let cache = root.join(fetch::CACHE_DIR);
    let codes = pack_countries(&pack)?;
    let (us, us_notes) = us_imports(&data, US_2000, HS_1996, &cache, &codes)?;
    println!("us 2000 {us_notes:?}");
    let mut obs = Vec::new();
    if set != "eu" {
        obs.extend(us.iter().cloned());
    }
    if set != "us" {
        let (us02, notes02) = us_imports(&data, US_2002, HS_2002, &cache, &codes)?;
        println!("us 2002 {notes02:?}");
        let shift = trend(&us, &us02);
        println!("trend 2002 to 2000, log-odds by good {shift:?}");
        let (eu, eu_notes) = eu_hindcast(&data, &cache, &codes, &shift)?;
        println!("eu 2000 hindcast {eu_notes:?}");
        obs.extend(eu);
    }
    if !["pooled", "us", "eu"].contains(&set.as_str()) {
        return Err(format!("--set {set}: expected pooled, us or eu"));
    }
    println!("set {set}, cells {}", obs.len());
    let all = TradeParams::default().theta;
    let theta: [f64; N_TRADED] = std::array::from_fn(|g| all[g]);
    let base = Transport::default();
    if want_profile {
        for point in profile(&pack, &obs, &theta, &base)? {
            println!("{point:?}");
        }
        return Ok(ExitCode::SUCCESS);
    }
    let est = estimate(&pack, &obs, &theta, &base)?;
    println!("{est:?}");
    println!(
        "P04d / P04a {:.4} (P04d {:.4} USD per tonne-km), se of ln r {:.4} (cluster bootstrap, \
         {} of {} replications at a bracket edge) and {:.4} (curvature), 90% interval {:.4} to {:.4}",
        est.ratio,
        est.ratio * base.per_tkm[0],
        est.se_ln_bootstrap,
        est.at_edge,
        ceteris_calibrate::air_share::BOOT_REPLICATIONS,
        est.se_ln_curvature,
        est.interval.0,
        est.interval.1
    );
    Ok(ExitCode::SUCCESS)
}

/// `calibrate --pack DIR [--no-border-fit] | --inputs DIR [--out DIR]`: exit 1 without a
/// calibration.
fn calibrate(args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut args = args.peekable();
    if args.peek().map(String::as_str) == Some("stability") {
        args.next();
        return stability(args);
    }
    if args.peek().map(String::as_str) == Some("rho") {
        args.next();
        return rho(args);
    }
    let (mut pack, mut inputs, mut out, mut fit_border) = (None, None, None, true);
    let (mut storage, mut fit_epsilon, mut epsilon_on) = (false, false, true);
    let mut fit_cache = None;
    let mut epsilon_from: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fit-epsilon" => fit_epsilon = true,
            "--no-epsilon-fit" => epsilon_on = false,
            "--fit-cache" => {
                fit_cache = Some(PathBuf::from(
                    args.next().ok_or("--fit-cache needs a directory")?,
                ))
            }
            "--epsilon-from" => {
                epsilon_from = Some(PathBuf::from(
                    args.next().ok_or("--epsilon-from needs an epsilon.json")?,
                ))
            }
            "--no-border-fit" => fit_border = false,
            "--storage" => storage = true,
            "--pack" => {
                pack = Some(PathBuf::from(
                    args.next().ok_or("--pack needs a directory")?,
                ))
            }
            "--inputs" => {
                inputs = Some(PathBuf::from(
                    args.next().ok_or("--inputs needs a directory")?,
                ))
            }
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    // The export of a pack lives until the reference has read it.
    let mut export = None;
    let (dir, u_star, pack_province, out, pack) = match (pack, inputs) {
        (Some(pack), None) => {
            let canonical =
                std::fs::canonicalize(&pack).map_err(|e| format!("{}: {e}", pack.display()))?;
            let name = canonical
                .file_name()
                .map(|n| n.to_owned())
                .ok_or("--pack names no directory")?;
            let out = out.unwrap_or_else(|| {
                sources::workspace_root()
                    .join("target/calibration")
                    .join(name)
            });
            if std::fs::canonicalize(&out).is_ok_and(|o| o == canonical) {
                return Err(
                    "--out is the pack: write the tables elsewhere and copy them in".into(),
                );
            }
            let tmp = tempfile::tempdir().map_err(|e| format!("temporary directory: {e}"))?;
            ceteris_calibrate::export_pack(&pack, tmp.path(), storage)?;
            let pack_province = calibration::read_pack_provinces(tmp.path())?;
            let u_star = calibration::read_u_star(&pack)?;
            let dir = export.insert(tmp).path().to_owned();
            (dir, Some(u_star), Some(pack_province), out, Some(pack))
        }
        (None, Some(_)) if !fit_border => return Err("--no-border-fit needs --pack".into()),
        (None, Some(_)) if storage => return Err("--storage needs --pack".into()),
        (None, Some(inputs)) => (inputs, None, None, out.ok_or("--inputs needs --out")?, None),
        _ => return Err("choose one input with --pack or --inputs".into()),
    };
    let mut inp = reference_io::load(&dir)?;
    drop(export);
    if fit_epsilon {
        let pack = pack.as_ref().filter(|_| fit_border);
        let pack = pack.ok_or("--fit-epsilon needs --pack and the border fit")?;
        return fit_epsilon_driver(&inp, pack, &out, fit_cache.as_deref());
    }
    let u_star = u_star.unwrap_or_else(|| inp.u_star.clone());
    let pack_province = pack_province.unwrap_or_else(|| (0..inp.n_provinces()).collect());
    let mut progress = |line: &str| println!("{line}");
    // A pack's border friction is fitted to ICIO's cross-border shares (ADR 0092) before the
    // steady state; the fixture keeps its own.
    let mut distance = None;
    let icio = match &pack {
        Some(_) if !fit_border => {
            println!(
                "border friction: kept at {:?} (--no-border-fit)",
                inp.border_nu
            );
            None
        }
        Some(pack) if epsilon_on => {
            // ADR 0272: the dark distance cost and the border friction are fitted together
            let n_traded = inp.traded.iter().filter(|&&t| t).count();
            let groups = border::friction_groups(&inp.theta_class[..n_traded]);
            let (_, icio) = border::target_shares(pack, &groups)?;
            // epsilon, ln d_0 and nu of a fit written earlier by --fit-epsilon, or a fit now
            let (epsilon, ln_d0, nu, ln_d) = match &epsilon_from {
                Some(file) => {
                    let (epsilon, ln_d0, nu) = epsilon::read_fit(file)?;
                    let data = ceteris_calibrate::epsilon::pack_data(
                        pack,
                        &sources::workspace_root().join(fetch::CACHE_DIR),
                    )?;
                    println!(
                        "distance cost: epsilon {epsilon} read from {}",
                        file.display()
                    );
                    (epsilon, ln_d0, nu, data.ln_d)
                }
                None => {
                    let Some((fit, ln_d)) =
                        run_epsilon_fit(&inp, pack, fit_cache.as_deref(), &mut progress)?
                    else {
                        return Ok(ExitCode::FAILURE);
                    };
                    let t = &fit.trial;
                    println!(
                        "distance cost: epsilon {} ({} trials), pooled coefficient {} against the data's {}, \
                         floored classes {:?}",
                        t.epsilon, fit.evaluations, t.pooled, fit.data_pooled, t.floored
                    );
                    (t.epsilon, t.ln_d0.clone(), t.fit.nu.clone(), ln_d)
                }
            };
            println!("border friction: nu {nu:?}");
            let d = calibration::Distance {
                epsilon,
                ln_d0,
                ln_d,
            };
            inp = epsilon::at_distance(&inp, &d)?;
            inp = border::at_border_nu(&inp, &border::nu_of_goods(&groups, &nu, &inp.border_nu))?;
            distance = (epsilon > 0.0).then_some(d);
            Some((icio, groups))
        }
        Some(pack) => {
            let n_traded = inp.traded.iter().filter(|&&t| t).count();
            let groups = border::friction_groups(&inp.theta_class[..n_traded]);
            let (target, icio) = border::target_shares(pack, &groups)?;
            println!("border friction: ICIO cross-border share by group {groups:?}: {target:?}");
            let fit = match border::fit_border_nu(
                &inp,
                &groups,
                &target,
                &icio,
                Limits::default(),
                &mut progress,
            ) {
                Ok(fit) => fit,
                Err(e) => {
                    eprintln!("no calibration: {e}");
                    return Ok(ExitCode::FAILURE);
                }
            };
            println!(
                "border friction: nu {:?}, share {:?}, clamped at nu 0 {:?}, {} evaluations",
                fit.nu, fit.share, fit.clamped, fit.evaluations
            );
            inp = border::at_border_nu(&inp, &fit.nu_of_goods(&inp.border_nu))?;
            Some((icio, groups))
        }
        None => None,
    };
    let s = match ceteris_calibrate::run(
        &inp,
        &u_star,
        &pack_province,
        &out,
        Limits::default(),
        &mut progress,
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("no calibration: {e}");
            return Ok(ExitCode::FAILURE);
        }
    };
    match &distance {
        Some(d) => calibration::write_distance(&out, d)?,
        None => calibration::clear_distance(&out)?,
    }
    if let Some((icio, groups)) = icio {
        let mut spend = s.spend_hh.clone();
        for (a, b) in spend.data.iter_mut().zip(&s.spend_int.data) {
            *a += b;
        }
        let share = border::cross_border_shares(&inp, &s.price, &spend, &icio, &groups)?;
        println!("border friction at the steady state: cross-border share {share:?}");
    }
    println!("{}", summary::summary(&inp, &s).pretty());
    println!("wrote {}", out.display());
    Ok(ExitCode::SUCCESS)
}

/// The dark distance cost of ADR 0272 fitted to the data's pooled distance coefficient, the
/// border friction refitted at each trial: the fit and the country-pair `ln d` it used.
fn run_epsilon_fit(
    inp: &reference_io::Inputs,
    pack: &Path,
    cache: Option<&Path>,
    progress: &mut dyn FnMut(&str),
) -> Result<Option<(ceteris_calibrate::epsilon::EpsilonFit, Vec<f64>)>, String> {
    use ceteris_calibrate::epsilon::{Targets, fit_epsilon_cached, pack_data};
    let n_traded = inp.traded.iter().filter(|&&t| t).count();
    let theta_class = &inp.theta_class[..n_traded];
    let groups = border::friction_groups(theta_class);
    let (nu_target, icio) = border::target_shares(pack, &groups)?;
    let data = pack_data(pack, &sources::workspace_root().join(fetch::CACHE_DIR))?;
    let t = Targets {
        nu_target: &nu_target,
        icio: &icio,
        theta_class,
        ln_d: &data.ln_d,
        weight: &data.weight,
        gravity: &data.gravity,
        data_flows: &data.data_flows,
    };
    match fit_epsilon_cached(inp, &t, Limits::default(), cache, progress) {
        Ok(fit) => Ok(Some((fit, data.ln_d))),
        Err(e) => {
            eprintln!("no epsilon fit: {e}");
            Ok(None)
        }
    }
}

/// `calibrate --pack DIR --fit-epsilon`: only the epsilon fit, written as `epsilon.json` into the
/// output directory, no calibration tables.
fn fit_epsilon_driver(
    inp: &reference_io::Inputs,
    pack: &Path,
    out: &Path,
    cache: Option<&Path>,
) -> Result<ExitCode, String> {
    let mut progress = |line: &str| println!("{line}");
    let Some((fit, _)) = run_epsilon_fit(inp, pack, cache, &mut progress)? else {
        return Ok(ExitCode::FAILURE);
    };
    let json = fit.json().pretty();
    println!("{json}");
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let file = out.join("epsilon.json");
    std::fs::write(&file, json).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("wrote {}", file.display());
    Ok(ExitCode::SUCCESS)
}

/// The scenario-file verbs of the workbench registry (`ceteris_run::commands::REGISTRY`).
fn scenario_verb(verb: &str, args: Vec<String>) -> Result<ExitCode, String> {
    use ceteris_run::commands as c;
    let file = args.first().ok_or("expected a scenario FILE")?;
    let flag = |name: &str| -> Option<&str> {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).map(String::as_str)
    };
    let need = |name: &str| flag(name).ok_or(format!("missing {name}"));
    let num = |name: &str| -> Result<u32, String> {
        need(name)?
            .parse()
            .map_err(|_| format!("{name} wants a tick"))
    };
    let read = || -> Result<ceteris_run::Scenario, String> {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        ceteris_run::Scenario::parse(&text).map_err(|e| e.to_string())
    };
    let write = |s: &ceteris_run::Scenario| {
        std::fs::write(file, s.to_toml()).map_err(|e| format!("{file}: {e}"))
    };
    match verb {
        "new" => {
            let seeds = flag("--seeds").unwrap_or("1");
            let seeds = seeds
                .split(',')
                .map(|s| s.parse().map_err(|_| "--seeds wants numbers".to_string()))
                .collect::<Result<Vec<u64>, _>>()?;
            write(&c::new_scenario(
                need("--pack-hash")?,
                need("--engine")?,
                seeds,
            ))?;
        }
        "fork" => {
            let s = c::fork(&read()?, need("--from")?, num("--at")?, need("--id")?)
                .map_err(|e| e.to_string())?;
            write(&s)?;
        }
        "edit" => {
            let kind = need("--kind")?;
            let kind = ceteris_run::EditKind::from_name(kind)
                .ok_or(format!("unknown edit kind `{kind}`"))?;
            let value = need("--value")?
                .parse()
                .map_err(|_| "--value wants a number".to_string())?;
            let edit = c::user_edit(
                num("--at")?,
                kind,
                need("--target")?,
                value,
                flag("--query").map(String::from),
            );
            write(&c::add_edit(&read()?, need("--branch")?, edit).map_err(|e| e.to_string())?)?;
        }
        "rebase" => {
            let (s, skipped) =
                c::rebase(&read()?, need("--branch")?, need("--onto")?, num("--at")?)
                    .map_err(|e| e.to_string())?;
            write(&s)?;
            skipped.iter().for_each(|l| println!("{l}"));
        }
        "cherry-pick" => {
            let picks = need("--edits")?
                .split(',')
                .map(|s| s.parse().map_err(|_| "--edits wants indices".to_string()))
                .collect::<Result<Vec<usize>, _>>()?;
            let (s, skipped) = c::cherry_pick(&read()?, need("--from")?, need("--onto")?, &picks)
                .map_err(|e| e.to_string())?;
            write(&s)?;
            skipped.iter().for_each(|l| println!("{l}"));
        }
        "share" => {
            let seed = need("--seed")?
                .parse()
                .map_err(|_| "--seed wants a number".to_string())?;
            println!(
                "{}",
                c::share(&read()?, need("--branch")?, seed).map_err(|e| e.to_string())?
            );
        }
        _ => {
            let (a, b) = (
                args.get(1).ok_or("expected two branches")?,
                args.get(2).ok_or("expected two branches")?,
            );
            for line in c::diff(&read()?, a, b).map_err(|e| e.to_string())? {
                println!("{line}");
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `serve --scenario FILE --tables DIR --token-file F [--port P]`: the read-only API on
/// 127.0.0.1 (default port 7878). A missing token file is created with a random token (mode
/// 0600). Runs until killed. `api schema` prints the tables.
fn serve_verb(command: &str, args: Vec<String>) -> Result<ExitCode, String> {
    use std::io::Read;

    if command == "api" {
        if args.first().map(String::as_str) != Some("schema") {
            return Err("expected api schema".into());
        }
        for (table, cols) in ceteris_run::outputs::TABLES {
            let cols: Vec<String> = cols.iter().map(|(c, k)| format!("{c}:{k:?}")).collect();
            println!("{table}({})", cols.join(", "));
        }
        return Ok(ExitCode::SUCCESS);
    }
    let flag = |name: &str| -> Option<&str> {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).map(String::as_str)
    };
    let need = |name: &str| flag(name).ok_or(format!("missing {name}"));
    let file = need("--scenario")?;
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let scenario = ceteris_run::Scenario::parse(&text).map_err(|e| e.to_string())?;
    let port: u16 = flag("--port")
        .unwrap_or("7878")
        .parse()
        .map_err(|_| "--port wants a number".to_string())?;
    let token_path = PathBuf::from(need("--token-file")?);
    let token = match std::fs::read_to_string(&token_path) {
        Ok(t) => t.trim().to_string(),
        Err(_) => {
            let mut raw = [0u8; 16];
            std::fs::File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut raw))
                .map_err(|e| format!("cannot draw a token: {e}"))?;
            let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
            std::fs::write(&token_path, format!("{token}\n"))
                .map_err(|e| format!("{}: {e}", token_path.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| e.to_string())?;
            }
            token
        }
    };
    let server = Arc::new(ceteris_run::serve::Server::new(
        token,
        scenario,
        PathBuf::from(need("--tables")?),
    ));
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = ceteris_run::serve::bind(addr).map_err(|e| e.to_string())?;
    println!(
        "listening on {}",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    for stream in listener.incoming().flatten() {
        let server = Arc::clone(&server);
        std::thread::spawn(move || server.serve(stream));
    }
    Ok(ExitCode::SUCCESS)
}

/// `fetch HASH --scenario FILE --ticks N --fixture|--pack DIR`: finds the (branch, seed) of the
/// scenario file whose run hash is HASH, recomputes that member to `ticks` and prints its state
/// hash. Pulling chunks from a mirror is not built.
fn fetch_verb(args: Vec<String>) -> Result<ExitCode, String> {
    use ceteris_econ::ensemble::run_member;
    use ceteris_run::ensemble::register_priors;

    let hash = args.first().ok_or("expected a run HASH")?;
    let flag = |name: &str| -> Option<&str> {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).map(String::as_str)
    };
    let need = |name: &str| flag(name).ok_or(format!("missing {name}"));
    let file = need("--scenario")?;
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let scenario = ceteris_run::Scenario::parse(&text).map_err(|e| e.to_string())?;
    let ticks: u32 = need("--ticks")?
        .parse()
        .map_err(|_| "--ticks wants a count".to_string())?;
    let source = match (flag("--pack"), args.iter().any(|a| a == "--fixture")) {
        (Some(dir), false) => Source::Pack(PathBuf::from(dir)),
        (None, true) => Source::Fixture,
        _ => return Err("choose an input with --fixture or --pack DIR".into()),
    };
    let mut found = None;
    for b in &scenario.branches {
        for &seed in &scenario.seeds {
            if ceteris_run::commands::share(&scenario, &b.id, seed).map_err(|e| e.to_string())?
                == *hash
            {
                found = Some((b.id.clone(), seed));
            }
        }
    }
    let (branch, seed) = found.ok_or("no branch and seed of the scenario has that run hash")?;
    let inputs = source.load(true)?;
    let base = (EconParams::default(), inputs.trade_params());
    let inputs = Arc::new(inputs);
    let log = scenario.edits_of(&branch).map_err(|e| e.to_string())?;
    let m = run_member(
        &inputs,
        &base,
        &register_priors(),
        &log,
        seed,
        ticks,
        |_| (),
    )
    .map_err(|e| format!("{e:?}"))?;
    let state: String = m.state_hash.iter().map(|b| format!("{b:02x}")).collect();
    println!("{branch} seed {seed} tick {ticks} state {state}");
    Ok(ExitCode::SUCCESS)
}

/// `bisect FILE --good A --bad B --ticks N --fixture|--pack DIR [--seed S] [--tol T]`: finds the
/// first of B's own edits after which a declared outcome of member S (the scenario's first seed
/// by default) differs from A's by more than the relative tolerance T (default 1e-9). A must be
/// B's parent: the branch with none of its own edits is then A itself.
fn bisect_verb(args: Vec<String>) -> Result<ExitCode, String> {
    use ceteris_econ::ensemble::{outcomes, run_member};
    use ceteris_run::ensemble::register_priors;

    let file = args.first().ok_or("expected a scenario FILE")?;
    let flag = |name: &str| -> Option<&str> {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).map(String::as_str)
    };
    let need = |name: &str| flag(name).ok_or(format!("missing {name}"));
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let scenario = ceteris_run::Scenario::parse(&text).map_err(|e| e.to_string())?;
    let (good, bad) = (need("--good")?, need("--bad")?);
    if scenario
        .branch(bad)
        .map_err(|e| e.to_string())?
        .parent
        .as_deref()
        != Some(good)
    {
        return Err(format!("--good must be the parent of {bad}"));
    }
    let ticks: u32 = need("--ticks")?
        .parse()
        .map_err(|_| "--ticks wants a count".to_string())?;
    let seed: u64 = match flag("--seed") {
        Some(s) => s.parse().map_err(|_| "--seed wants a number".to_string())?,
        None => *scenario.seeds.first().ok_or("the scenario has no seeds")?,
    };
    let tol: f64 = match flag("--tol") {
        Some(t) => t.parse().map_err(|_| "--tol wants a number".to_string())?,
        None => 1e-9,
    };
    let source = match (flag("--pack"), args.iter().any(|a| a == "--fixture")) {
        (Some(dir), false) => Source::Pack(PathBuf::from(dir)),
        (None, true) => Source::Fixture,
        _ => return Err("choose an input with --fixture or --pack DIR".into()),
    };
    let inputs = source.load(true)?;
    let base = (EconParams::default(), inputs.trade_params());
    let inputs = Arc::new(inputs);
    let priors = register_priors();
    let run = |s: &ceteris_run::Scenario, branch: &str| -> Result<Vec<f64>, String> {
        let log = s.edits_of(branch).map_err(|e| e.to_string())?;
        let m = run_member(&inputs, &base, &priors, &log, seed, ticks, |w| {
            outcomes(w)
                .into_iter()
                .map(|(_, v)| v)
                .collect::<Vec<f64>>()
        })
        .map_err(|e| format!("{e:?}"))?;
        Ok(m.outcome)
    };
    let reference = run(&scenario, good)?;
    let mut failure = None;
    let found = ceteris_run::vcs::bisect(&scenario, bad, seed, |cut, id| match run(cut, id) {
        Ok(v) => v
            .iter()
            .zip(&reference)
            .any(|(a, b)| (a - b).abs() > tol * b.abs().max(f64::MIN_POSITIVE)),
        Err(e) => {
            failure = Some(e);
            false
        }
    })
    .map_err(|e| e.to_string())?;
    if let Some(e) = failure {
        return Err(e);
    }
    match found.culprit {
        Some(i) => {
            let e = &scenario.branch(bad).map_err(|e| e.to_string())?.edits[i];
            println!(
                "edit {i}: {} {:?} {} = {} ({} probes)",
                e.tick, e.kind, e.target, e.value, found.probes
            );
        }
        None => println!(
            "no edit of {bad} moves an outcome beyond {tol:e} ({} probes)",
            found.probes
        ),
    }
    Ok(ExitCode::SUCCESS)
}

/// `ensemble` and `effects`: run a branch (and its baseline) over the scenario's seeds, with
/// the register priors drawn per member, and write the `outcomes` and `effects` tables.
fn ensemble_verb(verb: &str, args: Vec<String>) -> Result<ExitCode, String> {
    use ceteris_econ::ensemble::{outcomes, run_pair};
    use ceteris_run::effects::effect;
    use ceteris_run::ensemble::register_priors;
    use ceteris_run::outputs::{Column, batch, ipc};

    let file = args.first().ok_or("expected a scenario FILE")?;
    let flag = |name: &str| -> Option<&str> {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).map(String::as_str)
    };
    let need = |name: &str| flag(name).ok_or(format!("missing {name}"));
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let scenario = ceteris_run::Scenario::parse(&text).map_err(|e| e.to_string())?;
    let branch = need("--branch")?;
    let baseline = match flag("--baseline") {
        Some(b) => Some(b.to_string()),
        None if verb != "ensemble" => scenario
            .branch(branch)
            .map_err(|e| e.to_string())?
            .parent
            .clone(),
        None => None,
    };
    let baseline = match (verb, baseline) {
        ("effects" | "explain", None) => {
            return Err(format!(
                "{verb} needs --baseline (the branch has no parent)"
            ));
        }
        (_, b) => b,
    };
    let ticks: u32 = need("--ticks")?
        .parse()
        .map_err(|_| "--ticks wants a count".to_string())?;
    let source = match (flag("--pack"), args.iter().any(|a| a == "--fixture")) {
        (Some(dir), false) => Source::Pack(PathBuf::from(dir)),
        (None, true) => Source::Fixture,
        _ => return Err("choose an input with --fixture or --pack DIR".into()),
    };
    let out = if verb == "blame" {
        PathBuf::new()
    } else {
        PathBuf::from(need("--out")?)
    };

    let inputs = source.load(true)?;
    let base = (EconParams::default(), inputs.trade_params());
    let inputs = Arc::new(inputs);
    let branch_log = scenario.edits_of(branch).map_err(|e| e.to_string())?;
    let baseline_log = match &baseline {
        Some(b) => scenario.edits_of(b).map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    let priors = register_priors();
    if verb == "blame" {
        let own: Vec<_> = branch_log
            .iter()
            .filter(|e| !baseline_log.contains(e))
            .cloned()
            .collect();
        return blame(&inputs, &base, &priors, &own, &scenario.seeds, ticks);
    }
    if verb == "explain" {
        return explain(
            &inputs,
            &base,
            &priors,
            (&baseline_log, &branch_log),
            &scenario.seeds,
            ticks,
            branch,
            &out,
            args.iter().any(|a| a == "--reference"),
        );
    }
    let pair = run_pair(
        &inputs,
        &base,
        &priors,
        (&baseline_log, &branch_log),
        &scenario.seeds,
        ticks,
        outcomes,
    )
    .map_err(|e| format!("{e:?}"))?;

    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let (mut who, mut member, mut name, mut value) = (vec![], vec![], vec![], vec![]);
    let mut members = vec![(branch.to_string(), &pair.branch)];
    if let Some(b) = &baseline {
        members.push((b.clone(), &pair.baseline));
    }
    for (id, ms) in &members {
        for (k, m) in ms.iter().enumerate() {
            for (o, v) in &m.outcome {
                who.push(id.clone());
                member.push(k as u32);
                name.push((*o).to_string());
                value.push(*v);
            }
        }
    }
    let rows = who.len();
    let table = batch(
        "outcomes",
        vec![
            Column::Text(who),
            Column::Tick(member),
            Column::Tick(vec![ticks; rows]),
            Column::Text(name),
            Column::Real(value),
        ],
    )
    .map_err(|e| e.to_string())?;
    let write = |name: &str, bytes: Vec<u8>| {
        let path = out.join(name);
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    };
    write("outcomes.arrow", ipc(&table).map_err(|e| e.to_string())?)?;

    let Some(baseline) = baseline else {
        return Ok(ExitCode::SUCCESS);
    };
    let n_out = pair.branch[0].outcome.len();
    let cols: [Vec<f64>; 7] = Default::default();
    let [
        mut mean,
        mut lo,
        mut hi,
        mut p05,
        mut p50,
        mut p95,
        mut sign,
    ] = cols;
    let mut names = vec![];
    for i in 0..n_out {
        let pick = |ms: &[ceteris_econ::ensemble::Member<Vec<(&'static str, f64)>>]| {
            ms.iter().map(|m| m.outcome[i].1).collect::<Vec<f64>>()
        };
        let e = effect(&pick(&pair.branch), &pick(&pair.baseline), Seed(1));
        let label = pair.branch[0].outcome[i].0;
        println!(
            "{label}: {:+.6e} [{:+.6e}, {:+.6e}] sign share {:.2}",
            e.interval.mean, e.interval.lo, e.interval.hi, e.sign_share
        );
        names.push(label.to_string());
        mean.push(e.interval.mean);
        lo.push(e.interval.lo);
        hi.push(e.interval.hi);
        p05.push(e.p05);
        p50.push(e.p50);
        p95.push(e.p95);
        sign.push(e.sign_share);
    }
    let table = batch(
        "effects",
        vec![
            Column::Text(vec![branch.to_string(); n_out]),
            Column::Text(vec![baseline; n_out]),
            Column::Text(names),
            Column::Tick(vec![ticks; n_out]),
            Column::Real(mean),
            Column::Real(lo),
            Column::Real(hi),
            Column::Real(p05),
            Column::Real(p50),
            Column::Real(p95),
            Column::Real(sign),
        ],
    )
    .map_err(|e| e.to_string())?;
    write("effects.arrow", ipc(&table).map_err(|e| e.to_string())?)?;
    Ok(ExitCode::SUCCESS)
}

/// `blame`: the edits of a branch beyond its baseline ranked by their leave-one-out effect,
/// averaged over the seeds, per outcome.
fn blame(
    inputs: &Arc<ceteris_econ::inputs::EconInputs>,
    base: &(EconParams, ceteris_trade::TradeParams),
    priors: &[ceteris_run::ensemble::ParamPrior],
    edits: &[ceteris_run::Edit],
    seeds: &[u64],
    ticks: u32,
) -> Result<ExitCode, String> {
    use ceteris_econ::ensemble::{OUTCOME_NAMES, edit_effects, outcomes};
    use ceteris_run::attribution::blame;

    let all = |w: &ceteris_econ::world::EconWorld| -> Vec<f64> {
        outcomes(w).into_iter().map(|(_, v)| v).collect()
    };
    let per_seed = seeds
        .iter()
        .map(|&k| edit_effects(inputs, base, priors, edits, k, ticks, all))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e:?}"))?;
    for (o, name) in OUTCOME_NAMES.iter().enumerate() {
        let mean: Vec<f64> = (0..edits.len())
            .map(|i| per_seed.iter().map(|m| m[i][o]).sum::<f64>() / per_seed.len() as f64)
            .collect();
        for i in blame(&mean) {
            let e = &edits[i];
            println!(
                "{name} {:+.6e} edit {i}: {} {:?} {} = {}",
                mean[i], e.tick, e.kind, e.target, e.value
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Solves the GE reference with the baseline's and the branch's tariff edits in force from the
/// start (14.7) and prints the long-run effect on each declared outcome beside the simulated
/// one: the world price index and the mean wage over provinces.
fn reference_gap(
    inputs: &Arc<ceteris_econ::inputs::EconInputs>,
    base: &(EconParams, ceteris_trade::TradeParams),
    (baseline_log, branch_log): (&[ceteris_run::Edit], &[ceteris_run::Edit]),
    simulated: &[f64],
    out: &std::path::Path,
) -> Result<(), String> {
    use ceteris_econ::edit::tariff_inputs;
    use ceteris_econ::export::write_inputs;

    let solve_log = |log: &[ceteris_run::Edit], name: &str| -> Result<[f64; 2], String> {
        let inp = tariff_inputs(inputs, log).map_err(|e| format!("{e:?}"))?;
        let kernel = Kernel::build(&inp.network, &inp.weight, &inp.wedges, &base.1);
        let dir = out.join("reference").join(name);
        write_inputs(&dir, &inp, &kernel, &base.0, &base.1).map_err(|e| e.to_string())?;
        let ref_inputs = reference_io::load(&dir)?;
        let model = Model::new(&ref_inputs)?;
        let eq = solve::equilibrium(&model, None, solve::TOL, None)
            .map_err(|e| format!("no equilibrium for {name}: {e}"))?;
        let n = eq.wage.len() as f64;
        Ok([eq.price_index, eq.wage.iter().sum::<f64>() / n])
    };
    let (before, after) = (
        solve_log(baseline_log, "baseline")?,
        solve_log(branch_log, "branch")?,
    );
    for (o, name) in ceteris_econ::ensemble::OUTCOME_NAMES.iter().enumerate() {
        let long_run = after[o] - before[o];
        println!(
            "reference {name}: {long_run:+.6e} simulated {:+.6e} gap {:+.6e}",
            simulated[o],
            simulated[o] - long_run
        );
    }
    Ok(())
}

/// `explain`: the Shapley split of the paired effect over the shut-able channels (prices,
/// wages), per member, with bootstrap intervals over members, written as `channel_shares`.
#[allow(clippy::too_many_arguments)]
fn explain(
    inputs: &Arc<ceteris_econ::inputs::EconInputs>,
    base: &(EconParams, ceteris_trade::TradeParams),
    priors: &[ceteris_run::ensemble::ParamPrior],
    logs: (&[ceteris_run::Edit], &[ceteris_run::Edit]),
    seeds: &[u64],
    ticks: u32,
    branch: &str,
    out: &std::path::Path,
    reference: bool,
) -> Result<ExitCode, String> {
    use ceteris_econ::ensemble::{channel_shares, outcomes};
    use ceteris_run::effects::bootstrap_mean;
    use ceteris_run::outputs::{Column, batch, ipc};

    let all = |w: &ceteris_econ::world::EconWorld| -> Vec<f64> {
        outcomes(w).into_iter().map(|(_, v)| v).collect()
    };
    let per_member = seeds
        .iter()
        .map(|&k| channel_shares(inputs, base, priors, logs, k, ticks, all))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e:?}"))?;
    if reference {
        let simulated: Vec<f64> = (0..ceteris_econ::ensemble::OUTCOME_NAMES.len())
            .map(|o| per_member.iter().map(|m| m[o].total).sum::<f64>() / per_member.len() as f64)
            .collect();
        reference_gap(inputs, base, logs, &simulated, out)?;
    }
    let (mut outcome, mut channel) = (vec![], vec![]);
    let (mut share, mut lo, mut hi) = (vec![], vec![], vec![]);
    for (o, name) in ceteris_econ::ensemble::OUTCOME_NAMES.iter().enumerate() {
        let mut rows: Vec<(String, Vec<f64>)> = vec![("direct".into(), vec![])];
        rows.extend(
            ceteris_econ::ensemble::CHANNELS
                .iter()
                .map(|c| (format!("{c:?}").to_lowercase(), vec![])),
        );
        for m in &per_member {
            rows[0].1.push(m[o].direct);
            for (i, (_, v)) in m[o].shares.iter().enumerate() {
                rows[i + 1].1.push(*v);
            }
        }
        for (c, xs) in rows {
            let i = bootstrap_mean(&xs, Seed(1), 1000, 0.9);
            println!("{name} {c}: {:+.6e} [{:+.6e}, {:+.6e}]", i.mean, i.lo, i.hi);
            outcome.push((*name).to_string());
            channel.push(c);
            share.push(i.mean);
            lo.push(i.lo);
            hi.push(i.hi);
        }
    }
    let rows = outcome.len();
    let table = batch(
        "channel_shares",
        vec![
            Column::Text(vec![branch.to_string(); rows]),
            Column::Text(outcome),
            Column::Text(channel),
            Column::Real(share),
            Column::Real(lo),
            Column::Real(hi),
        ],
    )
    .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let path = out.join("channel_shares.arrow");
    std::fs::write(&path, ipc(&table).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let mut argv = std::env::args().skip(1);
    let command = argv.next().unwrap_or_default();
    if [
        "new",
        "fork",
        "edit",
        "diff",
        "share",
        "rebase",
        "cherry-pick",
    ]
    .contains(&command.as_str())
    {
        return scenario_verb(&command, argv.collect()).unwrap_or_else(|e| {
            eprintln!("ceteris: {e}");
            ExitCode::FAILURE
        });
    }
    if command == "serve" || command == "api" {
        return serve_verb(&command, argv.collect()).unwrap_or_else(|e| {
            eprintln!("ceteris: {e}");
            ExitCode::FAILURE
        });
    }
    if command == "replay" {
        return replay(argv).unwrap_or_else(|e| {
            eprintln!("ceteris: {e}");
            ExitCode::FAILURE
        });
    }
    if command == "setup" {
        return setup_verb(argv.collect()).unwrap_or_else(|e| {
            eprintln!("ceteris: {e}");
            ExitCode::FAILURE
        });
    }
    if command == "bisect" {
        return bisect_verb(argv.collect()).unwrap_or_else(|e| {
            eprintln!("ceteris: {e}");
            ExitCode::FAILURE
        });
    }
    if command == "fetch" {
        return fetch_verb(argv.collect()).unwrap_or_else(|e| {
            eprintln!("ceteris: {e}");
            ExitCode::FAILURE
        });
    }
    if ["ensemble", "effects", "explain", "blame"].contains(&command.as_str()) {
        return ensemble_verb(&command, argv.collect()).unwrap_or_else(|e| {
            eprintln!("ceteris: {e}");
            ExitCode::FAILURE
        });
    }
    if ["pack", "reference", "calibrate", "gravity", "air-share"].contains(&command.as_str()) {
        let result = match command.as_str() {
            "pack" => pack(argv),
            "reference" => reference(argv),
            "gravity" => gravity(argv),
            "air-share" => air_share(argv),
            _ => calibrate(argv),
        };
        return result.unwrap_or_else(|e| {
            eprintln!("ceteris: {e}\n{USAGE}");
            ExitCode::FAILURE
        });
    }
    let result = match command.as_str() {
        "export" | "run" => parse(argv).and_then(|args| {
            if command == "export" {
                export(&args)
            } else {
                run(&args)
            }
        }),
        _ => Err(
            "expected a command, export, run, pack, reference, calibrate, gravity or air-share"
                .into(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ceteris: {e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
