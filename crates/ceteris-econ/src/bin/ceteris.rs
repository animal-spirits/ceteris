//! `ceteris export|run`: Arrow export of a P2 world and of a run from its data state.
//!
//! ```text
//! ceteris export --fixture|--pack DIR --out DIR
//! ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--from-equilibrium DIR]
//!     [--history [--history-every N]] --out DIR
//! ```
//!
//! `--pack` loads the 2000 world of a built pack (`EconInputs::from_pack`) instead of the
//! generated fixture: `run` starts from its calibrated steady state when the pack holds the
//! calibration tables (ADR 0089), `export` writes its uncalibrated data state, which the
//! calibration starts from. `--from-equilibrium` starts the run from a GE reference solution
//! (`python -m ceteris.reference solve --out DIR`) instead of the data state. `--history` also
//! writes the per-tick series of `ceteris_econ::history`, every N ticks (default 1).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::export::{write_inputs, write_run};
use ceteris_econ::fixture::small_world;
use ceteris_econ::history::run_recorded;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_trade::{Kernel, TradeParams};

const USAGE: &str = "usage:
  ceteris export --fixture|--pack DIR --out DIR
  ceteris run --fixture|--pack DIR --ticks N [--frictionless] [--from-equilibrium DIR]
    [--history [--history-every N]] --out DIR";

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
    history: bool,
    history_every: Option<u32>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = args;
    let (mut source, mut out, mut ticks, mut frictionless) = (None, None, None, false);
    let mut equilibrium = None;
    let (mut history, mut history_every) = (false, None);
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
            "--history" => history = true,
            "--history-every" => {
                let n = args.next().ok_or("--history-every needs a count")?;
                match n.parse::<u32>() {
                    Ok(v) if v > 0 => history_every = Some(v),
                    _ => return Err(format!("bad sampling interval '{n}'")),
                }
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
        history,
        history_every,
    })
}

fn export(args: &Args) -> Result<(), String> {
    if args.ticks.is_some()
        || args.frictionless
        || args.equilibrium.is_some()
        || args.history
        || args.history_every.is_some()
    {
        return Err(
            "--ticks, --frictionless, --from-equilibrium, --history and --history-every belong to run"
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
    let params = if args.frictionless {
        EconParams::frictionless()
    } else {
        EconParams::default()
    };
    let (inputs, trade) = (Arc::new(args.source.load(true)?), TradeParams::default());
    let world = match &args.equilibrium {
        Some(dir) => {
            let solution = Equilibrium::read(dir).map_err(|e| e.to_string())?;
            EconWorld::from_equilibrium(inputs, params, trade, &solution)
        }
        None => EconWorld::new(inputs, params, trade),
    };
    let mut sim = Sim::new(world, schedule(), SEED);
    if args.history {
        let history = run_recorded(&mut sim, ticks, args.history_every.unwrap_or(1))
            .map_err(|v| v.to_string())?;
        write_run(&args.out, sim.world()).map_err(|e| e.to_string())?;
        return history.write(&args.out).map_err(|e| e.to_string());
    }
    sim.run_until(ticks).map_err(|v| v.to_string())?;
    write_run(&args.out, sim.world()).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let mut argv = std::env::args().skip(1);
    let command = argv.next().unwrap_or_default();
    let result = match command.as_str() {
        "export" | "run" => parse(argv).and_then(|args| {
            if command == "export" {
                export(&args)
            } else {
                run(&args)
            }
        }),
        _ => Err("expected a command, export or run".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ceteris: {e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
