//! `ceteris export|run|pack`: Arrow export of a P2 world and of a run from its data state, and
//! pack tools.
//!
//! ```text
//! ceteris pack compare A B [--iterative] [--table NAME]
//! ceteris pack check-sources --profile research|release
//! ceteris export --fixture|--pack DIR --out DIR
//! ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--from-equilibrium DIR]
//!     [--data-state] [--perturb SEED] [--history [--history-every N] [--history-format arrow|csv]] --out DIR
//! ```
//!
//! `--pack` loads the 2000 world of a built pack (`EconInputs::from_pack`) instead of the
//! generated fixture: `run` starts from its calibrated steady state when the pack holds the
//! calibration tables (ADR 0089), `export` writes its uncalibrated data state, which the
//! calibration starts from. `--from-equilibrium` starts the run from a GE reference solution
//! (`python -m ceteris.reference solve --out DIR`) instead of the data state. `--data-state`
//! starts a calibrated pack from the data state of its calibrated inputs (prices 1, base
//! wages and employment, one month of base use in stock, nothing in flight) instead of the
//! steady state. `--perturb SEED` moves the start by `EconWorld::perturb` (10%, ADR 0089) and
//! prints the factors' range. `--history` also writes the per-tick series of
//! `ceteris_econ::history`, every N ticks (default 1), as Arrow with zstd compression or, with
//! `--history-format csv`, as CSV.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

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
use ceteris_pack::sources::{self, Profile};
use ceteris_trade::{Kernel, TradeParams};

const USAGE: &str = "usage:
  ceteris pack compare A B [--iterative] [--table NAME]
  ceteris pack check-sources [--profile research|release]
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

/// `pack compare A B [--iterative] [--table NAME]`: exit 1 when the directories differ.
fn pack(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    match args.next().as_deref() {
        Some("compare") => {}
        Some("check-sources") => return check_sources(args),
        _ => return Err("expected a pack command, compare or check-sources".into()),
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

fn main() -> ExitCode {
    let mut argv = std::env::args().skip(1);
    let command = argv.next().unwrap_or_default();
    if command == "pack" {
        return pack(argv).unwrap_or_else(|e| {
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
        _ => Err("expected a command, export, run or pack".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ceteris: {e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
