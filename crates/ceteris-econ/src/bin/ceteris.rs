//! `ceteris export|run`: Arrow export of a P2 world and of a run from its data state.
//!
//! ```text
//! ceteris export --fixture --out DIR
//! ceteris run --fixture --ticks N [--frictionless] --out DIR
//! ```

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use ceteris_econ::export::{write_inputs, write_run};
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_trade::{Kernel, TradeParams};

const USAGE: &str = "usage:
  ceteris export --fixture --out DIR
  ceteris run --fixture --ticks N [--frictionless] --out DIR";

/// Where the inputs of a world come from.
enum Source {
    Fixture,
}

impl Source {
    fn load(&self) -> EconInputs {
        match self {
            Source::Fixture => small_world(),
        }
    }
}

struct Args {
    source: Source,
    out: PathBuf,
    ticks: Option<u32>,
    frictionless: bool,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = args;
    let (mut source, mut out, mut ticks, mut frictionless) = (None, None, None, false);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fixture" => source = Some(Source::Fixture),
            "--pack" => return Err("--pack needs the pack loader, which is not built yet".into()),
            "--frictionless" => frictionless = true,
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            "--ticks" => {
                let n = args.next().ok_or("--ticks needs a count")?;
                ticks = Some(n.parse().map_err(|_| format!("bad tick count '{n}'"))?);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(Args {
        source: source.ok_or("choose an input with --fixture")?,
        out: out.ok_or("--out is required")?,
        ticks,
        frictionless,
    })
}

fn export(args: &Args) -> Result<(), String> {
    if args.ticks.is_some() || args.frictionless {
        return Err("--ticks and --frictionless belong to run".into());
    }
    let inputs = args.source.load();
    let (params, trade) = (EconParams::default(), TradeParams::default());
    let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
    write_inputs(&args.out, &inputs, &kernel, &params, &trade).map_err(|e| e.to_string())
}

fn run(args: &Args) -> Result<(), String> {
    let ticks = args.ticks.ok_or("--ticks is required")?;
    let params = if args.frictionless {
        EconParams::frictionless()
    } else {
        EconParams::default()
    };
    let mut sim = EconWorld::sim(Arc::new(args.source.load()), params, TradeParams::default());
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
