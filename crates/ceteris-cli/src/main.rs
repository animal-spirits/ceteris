//! `ceteris export|run|pack`: Arrow export of a P2 world and of a run from its data state, and
//! pack tools.
//!
//! ```text
//! ceteris pack compare A B [--iterative] [--table NAME]
//! ceteris pack check-sources --profile research|release
//! ceteris pack fetch [--record]
//! ceteris pack record-counties
//! ceteris pack build [--out DIR]
//! ceteris pack build --econ-only [--pack DIR] [--out DIR]
//! ceteris reference solve --inputs DIR --out DIR
//! ceteris calibrate --pack DIR [--no-border-fit] | --inputs DIR [--out DIR]
//! ceteris calibrate stability [--storage] [FIXTURE_DIR]
//! ceteris calibrate rho --panel FILE [--base Y] [--shock Y]
//! ceteris gravity [--pack DIR] [--write] [--out FILE]
//! ceteris air-share [--pack DIR] [--data DIR] [--set pooled|us|eu] [--profile]
//! ceteris export --fixture|--pack DIR --out DIR
//! ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--no-drift] [--from-equilibrium DIR]
//!     [--data-state] [--perturb SEED] [--history [--history-every N] [--history-format arrow|csv]]
//!     [--timing] --out DIR
//! ```
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
use ceteris_calibrate::{border, gravity};
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
use ceteris_trade::{Kernel, TradeParams};

const USAGE: &str = "usage:
  ceteris pack compare A B [--iterative] [--table NAME]
  ceteris pack check-sources [--profile research|release]
  ceteris pack fetch [--record]
  ceteris pack record-counties
  ceteris pack build [--out DIR]
  ceteris pack build --econ-only [--pack DIR] [--out DIR]
  ceteris reference solve --inputs DIR --out DIR
  ceteris calibrate --pack DIR [--no-border-fit] | --inputs DIR [--out DIR]
  ceteris calibrate stability [--storage] [FIXTURE_DIR]
  ceteris calibrate rho --panel FILE [--base YEAR] [--shock YEAR]
  ceteris gravity [--pack DIR] [--write] [--out FILE]
  ceteris air-share [--pack DIR] [--data DIR] [--set pooled|us|eu] [--profile]
  ceteris export --fixture|--pack DIR --out DIR
  ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--no-drift] [--from-equilibrium DIR]
    [--data-state] [--perturb SEED] [--history [--history-every N] [--history-format arrow|csv]]
    [--timing] --out DIR";

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
}

fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = args;
    let (mut source, mut out, mut ticks, mut frictionless) = (None, None, None, false);
    let (mut equilibrium, mut no_drift) = (None, false);
    let (mut data_state, mut perturb) = (false, None);
    let (mut history, mut history_every, mut history_format) = (false, None, None);
    let mut timing = false;
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
            "--history" => history = true,
            "--timing" => timing = true,
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
    {
        return Err(
            "--ticks, --frictionless, --no-drift, --from-equilibrium, --data-state, --perturb, --history, \
             --history-every, --history-format and --timing belong to run"
                .into(),
        );
    }
    if let Source::Pack(dir) = &args.source {
        return ceteris_calibrate::export_pack(dir, &args.out);
    }
    let inputs = args.source.load(false)?;
    let (params, trade) = (EconParams::default(), TradeParams::default());
    let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
    write_inputs(&args.out, &inputs, &kernel, &params, &trade).map_err(|e| e.to_string())
}

fn run(args: &Args) -> Result<(), String> {
    let ticks = args.ticks.ok_or("--ticks is required")?;
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
    let trade = inputs.trade_params();
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
    if args.history {
        let history = run_recorded(&mut sim, ticks, args.history_every.unwrap_or(1))
            .map_err(|v| v.to_string())?;
        write_run(&args.out, sim.world()).map_err(|e| e.to_string())?;
        history
            .write(&args.out, args.history_format.unwrap_or(Format::Arrow))
            .map_err(|e| e.to_string())?;
    } else {
        sim.run_until(ticks).map_err(|v| v.to_string())?;
        write_run(&args.out, sim.world()).map_err(|e| e.to_string())?;
    }
    if args.timing {
        write_timing(&args.out, &timing::take()).map_err(|e| e.to_string())?;
    }
    Ok(())
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

/// `pack record-counties`: rewrites `counties.lock.toml` from the geoBoundaries API, downloading
/// the chosen files into the cache.
fn record_counties(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    if let Some(a) = args.next() {
        return Err(format!("unknown argument '{a}'"));
    }
    ceteris_pack::geo::counties::record(&sources::workspace_root())?;
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
        _ => {
            return Err(
                "expected a pack command, compare, check-sources, fetch, record-counties or build"
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

/// `calibrate rho --panel FILE [--base YEAR] [--shock YEAR]`: P53 from local projections of a
/// class, pair, year, flow, tariff panel (ADR 0027).
fn rho(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    use ceteris_calibrate::rho::{Design, estimate, parse_panel, report};
    let (mut panel, mut design) = (
        None,
        Design {
            base: 2017,
            shock: 2019,
        },
    );
    while let Some(a) = args.next() {
        let mut year = |name: &str| {
            args.next()
                .and_then(|v| v.parse::<i32>().ok())
                .ok_or(format!("{name} needs a year"))
        };
        match a.as_str() {
            "--panel" => panel = args.next().map(PathBuf::from),
            "--base" => design.base = year("--base")?,
            "--shock" => design.shock = year("--shock")?,
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let panel = panel.ok_or("--panel needs a file")?;
    let text = std::fs::read_to_string(&panel).map_err(|e| format!("{}: {e}", panel.display()))?;
    let (fits, skipped) = estimate(&parse_panel(&text)?, design);
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
    while let Some(a) = args.next() {
        match a.as_str() {
            "--no-border-fit" => fit_border = false,
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
            ceteris_calibrate::export_pack(&pack, tmp.path())?;
            let pack_province = calibration::read_pack_provinces(tmp.path())?;
            let u_star = calibration::read_u_star(&pack)?;
            let dir = export.insert(tmp).path().to_owned();
            (dir, Some(u_star), Some(pack_province), out, Some(pack))
        }
        (None, Some(_)) if !fit_border => return Err("--no-border-fit needs --pack".into()),
        (None, Some(inputs)) => (inputs, None, None, out.ok_or("--inputs needs --out")?, None),
        _ => return Err("choose one input with --pack or --inputs".into()),
    };
    let mut inp = reference_io::load(&dir)?;
    drop(export);
    let u_star = u_star.unwrap_or_else(|| inp.u_star.clone());
    let pack_province = pack_province.unwrap_or_else(|| (0..inp.n_provinces()).collect());
    let mut progress = |line: &str| println!("{line}");
    // A pack's border friction is fitted to ICIO's cross-border shares (ADR 0092) before the
    // steady state; the fixture keeps its own.
    let icio = match &pack {
        Some(_) if !fit_border => {
            println!(
                "border friction: kept at {:?} (--no-border-fit)",
                inp.border_nu
            );
            None
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
