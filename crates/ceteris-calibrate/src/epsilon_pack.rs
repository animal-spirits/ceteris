//! The epsilon fit on a built pack (ADR 0272): the pack's export, the ICIO shares, the pack's
//! province-based country distances, the CEPII gravity of 2000 and BACI's 2000 flows go into
//! [`epsilon::fit_epsilon`].

use std::collections::BTreeMap;
use std::path::Path;

use ceteris_econ::EconInputs;
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_pack::read::natural_earth::{ADMIN0, read_codes};
use ceteris_pack::read::tariffs::{GRAVITY, read_distances};
use ceteris_reference::io::{self, Inputs};

use crate::border;
use crate::epsilon::{EpsilonFit, Gravity, Targets, fit_epsilon};
use crate::gravity::{N_TRADED, baci_flows};
use crate::steady::Limits;
use crate::write::Table;

/// What the fit ran on besides the fit itself.
pub struct PackFit {
    pub inputs: Inputs,
    pub fit: EpsilonFit,
    /// The model's country-pair distance, natural log, row-major.
    pub ln_d: Vec<f64>,
    pub groups: Vec<Vec<usize>>,
}

/// Fits epsilon on the pack at `pack`, `export` the pack's export ([`crate::export_pack`]),
/// reading sources from the fetch `cache`.
pub fn fit_pack(
    pack: &Path,
    export: &Path,
    cache: &Path,
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<PackFit, String> {
    let inp = io::load(export)?;

    let graph = Pack::load(pack, &TransportParams::default()).map_err(|e| e.to_string())?;
    let econ = EconInputs::from_pack_data(&graph, pack).map_err(|e| e.to_string())?;
    let n = inp.n_countries();
    if econ.n_countries() != n {
        return Err(format!(
            "{} countries in the export and {} in the pack",
            n,
            econ.n_countries()
        ));
    }
    let ln_d: Vec<f64> = econ.country_distw(&graph).iter().map(|d| d.ln()).collect();

    let n_traded = inp.traded.iter().filter(|&&t| t).count();
    if n_traded != N_TRADED {
        return Err(format!("{n_traded} traded goods, the fit needs {N_TRADED}"));
    }
    let theta_class = inp.theta_class[..n_traded].to_vec();
    let groups = border::friction_groups(&theta_class);
    let (nu_target, icio) = border::target_shares(pack, &groups)?;
    progress(&format!(
        "border friction: ICIO cross-border share by group {groups:?}: {nu_target:?}"
    ));

    let codes = Table::open(&pack.join("countries.arrow"))?.strings("code")?;
    let (iso, _) = read_codes(&cache.join(ADMIN0))?;
    let flows = baci_flows(cache, &codes, &iso)?;
    let weight: Vec<f64> = (0..n * n)
        .map(|k| flows[k * N_TRADED..(k + 1) * N_TRADED].iter().sum())
        .collect();

    // CEPII lists countries by ISO 3166 code; a pack country without one has no data pair.
    let mut of_iso: BTreeMap<String, usize> = BTreeMap::new();
    for (c, code) in codes.iter().enumerate() {
        of_iso
            .entry(iso.get(code).cloned().unwrap_or_else(|| code.clone()))
            .or_insert(c);
    }
    let (mut distw, mut contig) = (vec![f64::NAN; n * n], vec![f64::NAN; n * n]);
    for r in read_distances(&cache.join(GRAVITY), 2000)? {
        if let (Some(&o), Some(&d)) = (of_iso.get(&r.iso3_o), of_iso.get(&r.iso3_d)) {
            distw[o * n + d] = r.distw;
            contig[o * n + d] = r.contig;
        }
    }
    let gravity = Gravity::new(n, &distw, &contig);

    let targets = Targets {
        nu_target: &nu_target,
        icio: &icio,
        theta_class: &theta_class,
        ln_d: &ln_d,
        weight: &weight,
        gravity: &gravity,
        data_flows: &flows,
    };
    let fit = fit_epsilon(&inp, &targets, limits, progress).map_err(|e| e.to_string())?;
    Ok(PackFit {
        inputs: inp,
        fit,
        ln_d,
        groups,
    })
}
