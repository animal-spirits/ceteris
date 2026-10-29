//! `ceteris export|run|pack`: Arrow export of a P2 world and of a run from its data state, and
//! pack tools.
//!
//! ```text
//! ceteris pack compare A B [--iterative] [--table NAME]
//! ceteris pack check-sources --profile research|release
//! ceteris pack fetch [--record]
//! ceteris pack dump-reader NAME [ARGS]
//! ceteris pack record-counties
//! ceteris pack dump-geo DIR [--build [--centres FILE] [--transport]]
//! ceteris pack dump-web DIR
//! ceteris pack build [--out DIR] [--centres FILE] [--simple DIR]
//! ceteris pack build --econ-only [--pack DIR] [--out DIR]
//! ceteris reference solve --inputs DIR --out DIR
//! ceteris calibrate --pack DIR | --inputs DIR [--out DIR]
//! ceteris calibrate stability [FIXTURE_DIR]
//! ceteris gravity [--pack DIR] [--write] [--out FILE]
//! ceteris export --fixture|--pack DIR --out DIR
//! ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--from-equilibrium DIR]
//!     [--data-state] [--perturb SEED] [--history [--history-every N] [--history-format arrow|csv]] --out DIR
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
//! prints the factors' range. `--history` also writes the per-tick series of
//! `ceteris_econ::history`, every N ticks (default 1), as Arrow with zstd compression or, with
//! `--history-format csv`, as CSV.
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
//! solved from. `--inputs DIR` calibrates an export directory at its own u*, its provinces
//! serving as their own pack provinces, and needs `--out`.
//!
//! `calibrate stability` prints the stability table of the stock-flow price rule for the
//! fixture in FIXTURE_DIR (default `tests/golden/p2-fixture`).
//!
//! `gravity` estimates theta by class, the border friction and the mode cost ratios by PPML
//! structural gravity on 2000 BACI and the pack's least-cost costs (`--pack`, default
//! `data/packs/2000`; the sources from the fetch cache), printing the table of estimates, the
//! sample counts and any stop rule that fires; it exits 1 when one fires. `--write` writes the
//! estimates as TOML into `--out` (default `python/ceteris/packs/gravity_2000.toml`, beside the
//! pack configuration) when no stop rule fires.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use ceteris_calibrate::gravity;
use ceteris_calibrate::steady::Limits;
use ceteris_calibrate::summary;
use ceteris_calibrate::write as calibration;
use ceteris_core::sim::Sim;
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
use ceteris_pack::read::dump;
use ceteris_pack::sources::{self, Profile};
use ceteris_reference::model::Model;
use ceteris_reference::{io as reference_io, output, solve};
use ceteris_trade::{Kernel, TradeParams};

const USAGE: &str = "usage:
  ceteris pack compare A B [--iterative] [--table NAME]
  ceteris pack check-sources [--profile research|release]
  ceteris pack fetch [--record]
  ceteris pack dump-reader NAME [ARGS]
  ceteris pack record-counties
  ceteris pack dump-geo DIR [--build [--centres FILE] [--transport]]
  ceteris pack dump-web DIR
  ceteris pack build [--out DIR] [--centres FILE] [--simple DIR]
  ceteris pack build --econ-only [--pack DIR] [--out DIR]
  ceteris reference solve --inputs DIR --out DIR
  ceteris calibrate --pack DIR | --inputs DIR [--out DIR]
  ceteris calibrate stability [FIXTURE_DIR]
  ceteris gravity [--pack DIR] [--write] [--out FILE]
  ceteris export --fixture|--pack DIR --out DIR
  ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--from-equilibrium DIR]
    [--data-state] [--perturb SEED] [--history [--history-every N] [--history-format arrow|csv]] --out DIR";

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
    equilibrium: Option<PathBuf>,
    data_state: bool,
    perturb: Option<u64>,
    history: bool,
    history_every: Option<u32>,
    history_format: Option<Format>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = args;
    let (mut source, mut out, mut ticks, mut frictionless) = (None, None, None, false);
    let mut equilibrium = None;
    let (mut data_state, mut perturb) = (false, None);
    let (mut history, mut history_every, mut history_format) = (false, None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fixture" => source = Some(Source::Fixture),
            "--pack" => {
                let dir = args.next().ok_or("--pack needs a directory")?;
                source = Some(Source::Pack(PathBuf::from(dir)));
            }
            "--frictionless" => frictionless = true,
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
        equilibrium,
        data_state,
        perturb,
        history,
        history_every,
        history_format,
    })
}

fn export(args: &Args) -> Result<(), String> {
    if args.ticks.is_some()
        || args.frictionless
        || args.equilibrium.is_some()
        || args.data_state
        || args.perturb.is_some()
        || args.history
        || args.history_every.is_some()
        || args.history_format.is_some()
    {
        return Err(
            "--ticks, --frictionless, --from-equilibrium, --data-state, --perturb, --history, \
             --history-every and --history-format belong to run"
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
    let params = if args.frictionless {
        EconParams::frictionless()
    } else {
        EconParams::default()
    };
    if args.data_state && args.equilibrium.is_some() {
        return Err("--data-state and --from-equilibrium choose different starts".into());
    }
    if args.data_state && matches!(args.source, Source::Fixture) {
        return Err("--data-state needs --pack: the fixture has no calibrated steady state".into());
    }
    let mut inputs = args.source.load(true)?;
    if args.data_state {
        inputs.steady = None;
    }
    let (inputs, trade) = (Arc::new(inputs), TradeParams::default());
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
    if args.history {
        let history = run_recorded(&mut sim, ticks, args.history_every.unwrap_or(1))
            .map_err(|v| v.to_string())?;
        write_run(&args.out, sim.world()).map_err(|e| e.to_string())?;
        return history
            .write(&args.out, args.history_format.unwrap_or(Format::Arrow))
            .map_err(|e| e.to_string());
    }
    sim.run_until(ticks).map_err(|v| v.to_string())?;
    write_run(&args.out, sim.world()).map_err(|e| e.to_string())
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

/// `pack dump-reader NAME [ARGS]`: one econ source reader's output on the fetch cache, as
/// canonical JSON.
fn dump_reader(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let name = args
        .next()
        .ok_or_else(|| format!("dump-reader needs a reader: {}", dump::NAMES.join(", ")))?;
    let rest: Vec<String> = args.collect();
    println!("{}", dump::dump(&name, &rest)?.text());
    Ok(ExitCode::SUCCESS)
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

/// `pack dump-geo DIR [--build [--centres FILE] [--transport]]`: the geo modules' tables and
/// fills of the provinces and counties in DIR, or with `--build` of those built from the cache
/// (Voronoi splits seeded from, and the sea grid placed at, the cell centres of FILE, else
/// h3o's), written to DIR/rust; `--transport` adds the transport graph's tables.
fn dump_geo(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let dir = args.next().ok_or("dump-geo needs a directory")?;
    let (mut build, mut centres, mut transport) = (false, None, false);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--build" => build = true,
            "--centres" => centres = Some(args.next().ok_or("--centres needs a file")?),
            "--transport" => transport = true,
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    if build {
        build_geo(Path::new(&dir), centres.as_deref(), transport)?;
    } else if centres.is_some() {
        return Err("--centres needs --build".into());
    } else if transport {
        return Err("--transport needs --build".into());
    } else {
        ceteris_pack::geo::dump::dump(Path::new(&dir))?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(feature = "geos")]
fn build_geo(dir: &Path, centres: Option<&str>, transport: bool) -> Result<(), String> {
    use ceteris_pack::geo::provinces::Centres;
    let centres = match centres {
        Some(path) => Centres::read(Path::new(path))?,
        None => Centres::H3o,
    };
    ceteris_pack::geo::dump::build(dir, &centres, transport)
}

#[cfg(not(feature = "geos"))]
fn build_geo(_: &Path, _: Option<&str>, _: bool) -> Result<(), String> {
    Err("dump-geo --build needs the owner-side `geos` feature".into())
}

/// `pack dump-web DIR`: the map layers, tiles and fills of the provinces and counties in DIR,
/// written to DIR/rust (`ceteris_pack::web::dump`).
fn dump_web(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let dir = args.next().ok_or("dump-web needs a directory")?;
    if let Some(a) = args.next() {
        return Err(format!("unknown argument '{a}'"));
    }
    web_dump(Path::new(&dir))?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(feature = "geos")]
fn web_dump(dir: &Path) -> Result<(), String> {
    ceteris_pack::web::dump::dump(dir)
}

#[cfg(not(feature = "geos"))]
fn web_dump(_: &Path) -> Result<(), String> {
    Err("dump-web needs the owner-side `geos` feature".into())
}

/// `pack build [--out DIR] [--centres FILE] [--simple DIR]`: the whole pack from the fetch cache into DIR
/// (default `data/packs/<year>`), on the owner-side `geos` feature; prints the build stats.
/// `--centres` seeds the Voronoi splits from, and places the sea grid at, the cell centres of
/// FILE (`scripts/parity/h3_centres.py`) instead of h3o's. `--simple` also writes the simplified
/// provinces and counties the fills cover, as vertex tables, to DIR.
///
/// `pack build --econ-only [--pack DIR] [--out DIR]`: the econ tables of a pack's P1 tables, from
/// the fetch cache; prints the summaries to stderr, then each table's SHA-256 and the build
/// stats as JSON.
fn build(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let (mut econ_only, mut pack, mut out, mut centres, mut simple) =
        (false, None, None, None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--econ-only" => econ_only = true,
            "--pack" => {
                pack = Some(PathBuf::from(
                    args.next().ok_or("--pack needs a directory")?,
                ))
            }
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            "--centres" => centres = Some(args.next().ok_or("--centres needs a file")?),
            "--simple" => {
                simple = Some(PathBuf::from(
                    args.next().ok_or("--simple needs a directory")?,
                ))
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    if !econ_only {
        if pack.is_some() {
            return Err("--pack needs --econ-only".into());
        }
        return build_pack(out, centres.as_deref(), simple.as_deref());
    }
    if centres.is_some() || simple.is_some() {
        return Err("--centres and --simple build the whole pack, not --econ-only".into());
    }
    let pack = pack.unwrap_or_else(|| sources::workspace_root().join("data/packs/2000"));
    let out = out.unwrap_or_else(|| pack.clone());
    let cache = sources::workspace_root().join(fetch::CACHE_DIR);
    let report = econ::build::build(&pack, &out, &cache)?;
    let hex = |d: &[u8; 32]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let tables = Json::object(
        report
            .tables
            .iter()
            .map(|(name, d)| (name, Json::Str(hex(d)))),
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
fn build_pack(
    out: Option<PathBuf>,
    centres: Option<&str>,
    simple: Option<&Path>,
) -> Result<ExitCode, String> {
    use ceteris_pack::geo::provinces::Centres;
    let out = match out {
        Some(out) => out,
        None => ceteris_pack::build::default_out(&sources::workspace_root())?,
    };
    let centres = match centres {
        Some(path) => Centres::read(Path::new(path))?,
        None => Centres::H3o,
    };
    let built = ceteris_pack::build::build(&out, &centres, simple)?;
    println!("{}", built.stats.pretty());
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(feature = "geos"))]
fn build_pack(_: Option<PathBuf>, _: Option<&str>, _: Option<&Path>) -> Result<ExitCode, String> {
    Err("pack build needs the owner-side `geos` feature, or --econ-only".into())
}

/// `pack compare A B [--iterative] [--table NAME]`: exit 1 when the directories differ.
fn pack(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    match args.next().as_deref() {
        Some("compare") => {}
        Some("check-sources") => return check_sources(args),
        Some("fetch") => return fetch_sources(args),
        Some("dump-reader") => return dump_reader(args),
        Some("record-counties") => return record_counties(args),
        Some("dump-geo") => return dump_geo(args),
        Some("dump-web") => return dump_web(args),
        Some("build") => return build(args),
        _ => {
            return Err(
                "expected a pack command, compare, check-sources, fetch, record-counties, \
                 dump-reader, dump-geo, dump-web or build"
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

/// `calibrate stability [FIXTURE_DIR]`.
fn stability(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let fixture = args.next().map_or_else(
        || sources::workspace_root().join("tests/golden/p2-fixture"),
        PathBuf::from,
    );
    if let Some(a) = args.next() {
        return Err(format!("unknown argument '{a}'"));
    }
    for row in ceteris_calibrate::stability::table(&fixture)? {
        println!("{row}");
    }
    Ok(ExitCode::SUCCESS)
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
    let out = out.unwrap_or_else(|| root.join("python/ceteris/packs/gravity_2000.toml"));
    let est = gravity::estimate(&pack, &root.join(fetch::CACHE_DIR))?;
    println!("{}", gravity::report(&est));
    println!("{}", est.notes.json().pretty());
    let fired = gravity::stop_rules(&est);
    for f in &fired {
        println!("stop rule: {f}");
    }
    if write && fired.is_empty() {
        std::fs::write(&out, gravity::to_toml(&est))
            .map_err(|e| format!("{}: {e}", out.display()))?;
        println!("wrote {}", out.display());
    }
    Ok(if fired.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// `calibrate --pack DIR | --inputs DIR [--out DIR]`: exit 1 without a calibration.
fn calibrate(args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let mut args = args.peekable();
    if args.peek().map(String::as_str) == Some("stability") {
        args.next();
        return stability(args);
    }
    let (mut pack, mut inputs, mut out) = (None, None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
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
    let (dir, u_star, pack_province, out) = match (pack, inputs) {
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
            (dir, Some(u_star), Some(pack_province), out)
        }
        (None, Some(inputs)) => (inputs, None, None, out.ok_or("--inputs needs --out")?),
        _ => return Err("choose one input with --pack or --inputs".into()),
    };
    let inp = reference_io::load(&dir)?;
    drop(export);
    let u_star = u_star.unwrap_or_else(|| inp.u_star.clone());
    let pack_province = pack_province.unwrap_or_else(|| (0..inp.n_provinces()).collect());
    let mut progress = |line: &str| println!("{line}");
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
    println!("{}", summary::summary(&inp, &s).pretty());
    println!("wrote {}", out.display());
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let mut argv = std::env::args().skip(1);
    let command = argv.next().unwrap_or_default();
    if ["pack", "reference", "calibrate", "gravity"].contains(&command.as_str()) {
        let result = match command.as_str() {
            "pack" => pack(argv),
            "reference" => reference(argv),
            "gravity" => gravity(argv),
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
        _ => Err("expected a command, export, run, pack, reference, calibrate or gravity".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ceteris: {e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
