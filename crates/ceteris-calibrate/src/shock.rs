//! Comparative-statics shocks to a world's inputs (ADR 0200, ADR 0207), for the benchmark
//! harness ([`crate::benchmark`]).
//!
//! A shock file has one shock per line, countries named by ISO3 code (rows of
//! `econ_countries.arrow`), goods by index; `#` starts a comment:
//!
//! ```text
//! tariff IMPORTER EXPORTER GROUP ADD   ad valorem points added; GROUP 0, 1 or -1 for both
//! ntb EXPORTER IMPORTER FACTOR          crossing tau times FACTOR (iceberg, no revenue)
//! border FACTOR [GOOD]                  every crossing's tau times FACTOR, for GOOD or all goods
//! prod COUNTRY GOOD FACTOR              A of the country's enterprises of GOOD times FACTOR
//! ```
//!
//! Iceberg shocks go through `Network::route_taus`, filled with the kernel's least-cost factors
//! (S05) of the leg each good uses: the bulk legs for the bulk classes (ADR 0247), which are then
//! dropped, since route taus and bulk legs are exclusive. Such a world runs with the bulk freight
//! rate off ([`trade_params`]); its kernel is the default one but for the shocked crossings. An
//! empty shock list changes nothing.

use ceteris_econ::EconInputs;
use ceteris_trade::TradeParams;
use ceteris_trade::goods::N_GOODS;
use ceteris_trade::network::{Leg, LegTaus};

#[derive(Clone, Debug, PartialEq)]
pub enum Shock {
    /// `add` to the ad valorem tariff of `importer` on `exporter`, for one tariff group or both.
    Tariff {
        importer: usize,
        exporter: usize,
        group: Option<usize>,
        add: f32,
    },
    /// Every crossing from `exporter` to `importer`: tau times `factor`.
    Ntb {
        exporter: usize,
        importer: usize,
        factor: f64,
    },
    /// Every crossing: tau times `factor`, for `good` or every good.
    Border { factor: f64, good: Option<usize> },
    /// A of the enterprises of `good` in `country` times `factor`.
    Prod {
        country: usize,
        good: usize,
        factor: f64,
    },
}

/// Parses a shock file; `codes` are the countries' ISO3 codes in row order.
pub fn parse(text: &str, codes: &[String]) -> Result<Vec<Shock>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let at = |e: String| format!("line {}: {e}", n + 1);
        let f: Vec<&str> = line.split_whitespace().collect();
        let arity = |want: &[usize]| {
            if want.contains(&f.len()) {
                Ok(())
            } else {
                Err(at(format!("'{}' takes {:?} fields", f[0], want)))
            }
        };
        let country = |i: usize| {
            codes
                .iter()
                .position(|c| c == f[i])
                .ok_or_else(|| at(format!("unknown country '{}'", f[i])))
        };
        let good = |i: usize| match f[i].parse::<usize>() {
            Ok(g) if g < N_GOODS => Ok(g),
            _ => Err(at(format!("'{}' is not a good", f[i]))),
        };
        let factor = |i: usize| match f[i].parse::<f64>() {
            Ok(x) if x.is_finite() && x > 0.0 => Ok(x),
            _ => Err(at(format!("'{}' is not a positive factor", f[i]))),
        };
        out.push(match f[0] {
            "tariff" => {
                arity(&[5])?;
                let group = match f[3] {
                    "-1" => None,
                    "0" => Some(0),
                    "1" => Some(1),
                    g => return Err(at(format!("'{g}' is not a tariff group"))),
                };
                let add = f[4]
                    .parse::<f32>()
                    .ok()
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| at(format!("'{}' is not a tariff", f[4])))?;
                Shock::Tariff {
                    importer: country(1)?,
                    exporter: country(2)?,
                    group,
                    add,
                }
            }
            "ntb" => {
                arity(&[4])?;
                Shock::Ntb {
                    exporter: country(1)?,
                    importer: country(2)?,
                    factor: factor(3)?,
                }
            }
            "border" => {
                arity(&[2, 3])?;
                Shock::Border {
                    factor: factor(1)?,
                    good: if f.len() > 2 { Some(good(2)?) } else { None },
                }
            }
            "prod" => {
                arity(&[4])?;
                Shock::Prod {
                    country: country(1)?,
                    good: good(2)?,
                    factor: factor(3)?,
                }
            }
            other => return Err(at(format!("unknown shock '{other}'"))),
        });
    }
    Ok(out)
}

/// Applies `shocks` to `inputs`, whose network was built for `trade` (ADR 0247: its bulk legs
/// match `trade`'s bulk freight rate).
pub fn apply(inputs: &mut EconInputs, shocks: &[Shock], trade: &TradeParams) {
    let nc = inputs.n_countries();
    if shocks
        .iter()
        .any(|s| matches!(s, Shock::Ntb { .. } | Shock::Border { .. }))
    {
        fold_route_taus(inputs, trade);
        let net = &mut inputs.network;
        let taus = net.route_taus.as_mut().expect("route taus just set");
        for (i, l) in net.crossings.iter().enumerate() {
            let ex = usize::from(net.gateway_country[l.from as usize]);
            let im = usize::from(net.gateway_country[l.to as usize]);
            for s in shocks {
                let (factor, only) = match *s {
                    Shock::Border { factor, good } => (factor, good),
                    Shock::Ntb {
                        exporter,
                        importer,
                        factor,
                    } if exporter == ex && importer == im => (factor, None),
                    _ => continue,
                };
                for (g, t) in taus.crossings[i].iter_mut().enumerate() {
                    if only.is_none_or(|x| x == g) {
                        *t *= factor;
                    }
                }
            }
        }
    }
    for s in shocks {
        match *s {
            Shock::Tariff {
                importer,
                exporter,
                group,
                add,
            } => {
                for gr in 0..2 {
                    if group.is_none_or(|x| x == gr) {
                        inputs.wedges.tariff[(importer * nc + exporter) * 2 + gr] += add;
                    }
                }
            }
            Shock::Prod {
                country,
                good,
                factor,
            } => {
                for e in 0..inputs.n_enterprises() {
                    let i = inputs.enterprises.province[e] as usize;
                    if inputs.country_of(i) == country
                        && usize::from(inputs.enterprises.good[e]) == good
                    {
                        inputs.enterprises.a[e] *= factor;
                    }
                }
            }
            Shock::Ntb { .. } | Shock::Border { .. } => {}
        }
    }
}

/// The trade parameters to run `inputs` with after [`apply`]: `trade` with the bulk freight
/// rate off where the bulk legs were folded into route taus.
pub fn trade_params(inputs: &EconInputs, trade: TradeParams) -> TradeParams {
    if inputs.network.bulk.is_none() && inputs.network.route_taus.is_some() {
        trade.bulk_off()
    } else {
        trade
    }
}

/// Sets the network's route taus to the kernel's least-cost factors under `trade`, from the
/// bulk legs for the bulk classes' goods, and drops the bulk legs.
fn fold_route_taus(inputs: &mut EconInputs, trade: &TradeParams) {
    if inputs.network.route_taus.is_some() {
        return;
    }
    let bulk = inputs.network.bulk.take();
    let weight = inputs.weight;
    let on_bulk = |g: usize| {
        bulk.as_ref()
            .is_some_and(|b| b.classes[usize::from(trade.theta_class[g])])
    };
    let factors = |base: &[Leg], bulk: Option<&Vec<Leg>>| {
        let leg = |i: usize, g: usize| bulk.filter(|_| on_bulk(g)).map_or(&base[i], |b| &b[i]);
        let taus: Vec<[f64; N_GOODS]> = (0..base.len())
            .map(|i| {
                std::array::from_fn(|g| {
                    let l = leg(i, g);
                    // the kernel's least-cost factor, bit for bit
                    1.0 + l.cost_per_t * weight[g] + trade.time_cost[g] * l.hours / 24.0
                })
            })
            .collect();
        let hours = (0..base.len())
            .map(|i| std::array::from_fn(|g| leg(i, g).hours))
            .collect();
        (taus, hours)
    };
    let net = &inputs.network;
    let (domestic, domestic_hours) = factors(&net.domestic, bulk.as_ref().map(|b| &b.domestic));
    let (access, access_hours) = factors(&net.access, bulk.as_ref().map(|b| &b.access));
    let (crossings, crossing_hours) = factors(&net.crossings, bulk.as_ref().map(|b| &b.crossings));
    inputs.network.route_taus = Some(LegTaus {
        domestic,
        access,
        crossings,
        domestic_hours,
        access_hours,
        crossing_hours,
    });
}
