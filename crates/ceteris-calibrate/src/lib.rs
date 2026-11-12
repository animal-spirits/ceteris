#![allow(clippy::disallowed_methods)] // offline owner-side fitting: its output is shipped as pack data, never recomputed by a run
//! Steady-state calibration of the 2000 data state (ADR 0089): [`steady`] solves it from a
//! `ceteris export` read by the reference model's reader, [`write`] writes the tables that
//! `EconInputs::from_pack` loads; [`benchmark`] runs a calibrated pack under [`shock`]s and
//! measures welfare (ADRs 0200, 0207).

pub mod air_share;
pub mod balance;
pub mod benchmark;
pub mod block;
pub mod border;
pub mod costs;
pub mod dta;
pub mod elasticity;
pub mod gravity;
pub mod json;
pub mod pyfmt;
pub mod rho;
pub mod rho_panel;
pub mod shock;
pub mod stability;
pub mod steady;
pub mod summary;
pub mod write;
pub mod yotov;

use std::path::Path;

use ceteris_econ::export::write_inputs;
use ceteris_econ::{EconInputs, EconParams};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_reference::io::Inputs;
use ceteris_trade::{Kernel, TradeParams};

use steady::{CalibrationError, Limits, Steady};

/// Exports the data state of the pack in `pack` into `out`, as `ceteris export --pack` does;
/// `storage` under the approved storage values (`TradeParams::with_storage`), which a
/// calibration of the export then carries (RFC 0286 screen).
pub fn export_pack(pack: &Path, out: &Path, storage: bool) -> Result<(), String> {
    let graph = Pack::load(pack, &TransportParams::default()).map_err(|e| e.to_string())?;
    let inputs = EconInputs::from_pack_data(&graph, pack).map_err(|e| e.to_string())?;
    let params = EconParams::default();
    let trade = if storage {
        TradeParams::default().with_storage()
    } else {
        TradeParams::default()
    };
    let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
    write_inputs(out, &inputs, &kernel, &params, &trade).map_err(|e| e.to_string())
}

/// Calibrates `inp` at `u_star` and writes the tables into `out`, keyed by `pack_province`;
/// nothing is written when the calibration fails.
pub fn run(
    inp: &Inputs,
    u_star: &[f64],
    pack_province: &[usize],
    out: &Path,
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<Steady, CalibrationError> {
    let s = steady::calibrate_with(inp, u_star, limits, progress)?;
    write::write(out, inp, &s, pack_province)?;
    Ok(s)
}
