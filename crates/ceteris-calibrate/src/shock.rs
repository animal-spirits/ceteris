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
//! dropped, since route taus and bulk legs are exclusive
//! ([`EconInputs::fold_bulk_into_route_taus`]). Such a world runs with the bulk freight
//! rate off ([`trade_params`]); its kernel is the default one but for the shocked crossings. An
//! empty shock list changes nothing.

use ceteris_econ::EconInputs;
use ceteris_trade::TradeParams;

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

/// Parses a shock file; `codes` are the countries' ISO3 codes in row order, `n_goods` the
/// goods count.
pub fn parse(text: &str, codes: &[String], n_goods: usize) -> Result<Vec<Shock>, String> {
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
            Ok(g) if g < n_goods => Ok(g),
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
        if inputs.network.route_taus.is_none() {
            inputs.fold_bulk_into_route_taus(trade);
        }
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
