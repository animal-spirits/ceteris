//! P04d from 2000 air value shares (ADR 0277; geo SPEC, Air rate from mode shares): the air part
//! of the trade kernel's gateway-pair sum, fitted to the data's share by a Bernoulli
//! quasi-likelihood for fractional data (Papke and Wooldridge 1996).

use std::collections::BTreeMap;
use std::path::Path;

use ceteris_pack::econ::goods::hs_goods;
use ceteris_pack::read::baci::read_hs_cpa_version;
use ceteris_pack::read::natural_earth::{ADMIN0, Dbf, read_codes};
use ceteris_reference::io::pairwise_sum;
use rayon::prelude::*;

use crate::costs::{AIR, Graph, N_TRADED, Quantities, Transport, edge_quantities, tau};
use crate::dta::read_dta;
use crate::gravity::{TIME_COST, pack_index};
use crate::write::Table;

/// One gateway pair of a cell: `tau(r) = beta + slope * r`, with `slope = w_g f_road q[air]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Crossing {
    pub a: usize,
    pub b: usize,
    pub beta: f64,
    pub slope: f64,
}

impl Crossing {
    fn air(&self) -> bool {
        self.slope > 0.0
    }
}

/// One (importer, origin, good) cell: origin access `v` and destination access `u` per gateway,
/// the gateway pairs, the good's theta and the data's air share of value.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub cluster: usize,
    pub share: f64,
    pub weight: f64,
    pub theta: f64,
    pub v: Vec<f64>,
    pub u: Vec<f64>,
    pub crossings: Vec<Crossing>,
}

impl Cell {
    /// The model's air share and its derivative in r.
    pub fn model_share(&self, r: f64) -> (f64, f64) {
        let (mut n, mut d, mut dn, mut dd) = (0.0, 0.0, 0.0, 0.0);
        for x in &self.crossings {
            let tau = x.beta + x.slope * r;
            let vku = self.v[x.a] * tau.powf(-self.theta) * self.u[x.b];
            let dvku = -self.theta * vku * x.slope / tau;
            d += vku;
            dd += dvku;
            if x.air() {
                n += vku;
                dn += dvku;
            }
        }
        let s = n / d;
        (s, (dn - s * dd) / d)
    }

    /// False when the share is 0 or 1 at every r: no air crossing, or no other.
    fn informative(&self) -> bool {
        let reached = |air: bool| {
            self.crossings
                .iter()
                .any(|x| x.air() == air && self.v[x.a] * self.u[x.b] > 0.0)
        };
        reached(true) && reached(false)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AirFit {
    /// P04d / P04a.
    pub ratio: f64,
    /// Clustered by (importer, origin).
    pub se: f64,
    pub cells: usize,
    pub dropped: usize,
    pub iterations: usize,
    pub quasi_loglik: f64,
}

pub const AIR_TOL: f64 = 1e-12;
pub const AIR_MAX_ITER: usize = 100;

fn quasi_loglik(cells: &[&Cell], r: f64) -> f64 {
    cells
        .iter()
        .map(|c| {
            let s = c.model_share(r).0;
            c.weight * (c.share * s.ln() + (1.0 - c.share) * (1.0 - s).ln())
        })
        .sum()
}

/// Fisher scoring in ln r from `r0`, halving a step that lowers the quasi-likelihood. Fails
/// when r does not exceed 1 (DESIGN 5.3, road < air): the value is reported, not clamped.
pub fn fit(cells: &[Cell], r0: f64) -> Result<AirFit, String> {
    let used: Vec<&Cell> = cells.iter().filter(|c| c.informative()).collect();
    if used.is_empty() {
        return Err("air share: no cell has both an air and another crossing".into());
    }
    let mut x = r0.ln();
    let mut ll = quasi_loglik(&used, x.exp());
    let mut iterations = 0;
    for iteration in 1..=AIR_MAX_ITER {
        let r = x.exp();
        let (mut score, mut info) = (0.0, 0.0);
        for c in &used {
            let (s, ds) = c.model_share(r);
            let dsx = r * ds;
            let var = s * (1.0 - s);
            score += c.weight * (c.share - s) / var * dsx;
            info += c.weight * dsx * dsx / var;
        }
        let mut step = score / info;
        let mut next = quasi_loglik(&used, (x + step).exp());
        let mut halvings = 0;
        while (next.is_nan() || next < ll) && halvings < 60 {
            step /= 2.0;
            next = quasi_loglik(&used, (x + step).exp());
            halvings += 1;
        }
        x += step;
        ll = next;
        if step.abs() <= AIR_TOL {
            iterations = iteration;
            break;
        }
    }
    if iterations == 0 {
        return Err(format!(
            "air share did not converge in {AIR_MAX_ITER} steps: r {}",
            x.exp()
        ));
    }
    let r = x.exp();
    let mut info = 0.0;
    let mut by_cluster: std::collections::BTreeMap<usize, f64> = Default::default();
    for c in &used {
        let (s, ds) = c.model_share(r);
        let dsx = r * ds;
        let var = s * (1.0 - s);
        info += c.weight * dsx * dsx / var;
        *by_cluster.entry(c.cluster).or_default() += c.weight * (c.share - s) / var * dsx;
    }
    let meat: f64 = by_cluster.values().map(|g| g * g).sum();
    let fit = AirFit {
        ratio: r,
        se: r * meat.sqrt() / info,
        cells: used.len(),
        dropped: cells.len() - used.len(),
        iterations,
        quasi_loglik: ll,
    };
    if fit.ratio <= 1.0 {
        return Err(format!(
            "P04d: air / road {} does not exceed 1 (road < air)",
            fit.ratio
        ));
    }
    Ok(fit)
}

// ---------------------------------------------------------------- data: US imports 2000

/// The fetched files (ADR 0277, Build), under `data/raw/air-shares-2000`.
pub const US_IMPORTS: &str = "imp_detl_2000_12n.dta";
pub const CENSUS_COUNTRIES: &str = "country.txt";
const HS_KEY: &str = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx";
/// The BTiGE key's version for HS 1996, the US 2000 nomenclature.
const HS_1996: u8 = 1;

/// Air and total import value of one (importer, origin, traded good) cell, USD.
#[derive(Clone, Debug, PartialEq)]
pub struct Obs {
    pub importer: usize,
    pub origin: usize,
    pub good: usize,
    pub air: f64,
    pub total: f64,
}

/// What the US reader kept and left out, USD of general imports.
#[derive(Clone, Debug, PartialEq)]
pub struct UsNotes {
    pub total: f64,
    pub air: f64,
    pub unmatched_country: f64,
    pub unmatched_codes: Vec<String>,
    pub unmatched_hs: f64,
    pub local_goods: f64,
}

/// The pack's country codes, in pack order.
pub fn pack_countries(pack: &Path) -> Result<Vec<String>, String> {
    Table::open(&pack.join("countries.arrow"))?.strings("code")
}

/// Census Schedule C: country code to ISO alpha-2, from `Code | Name | ISO Code` rows.
fn census_iso2(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('|').map(str::trim).collect();
            (f.len() == 3 && f[0].len() == 4 && f[0].bytes().all(|c| c.is_ascii_digit()))
                .then(|| (f[0].to_owned(), f[2].to_owned()))
        })
        .filter(|(_, iso)| iso.len() == 2)
        .collect()
}

/// US general imports of 2000 by origin and traded good, summed over districts: air is
/// `air_val_yr`, the total `gen_val_yr`, land and vessel the rest. HS 1996 lines go to goods by
/// the BTiGE key; origins to pack countries by ISO code.
pub fn us_imports(
    data: &Path,
    cache: &Path,
    codes: &[String],
) -> Result<(Vec<Obs>, UsNotes), String> {
    let path = data.join(US_IMPORTS);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let d = read_dta(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let numbers = |n: &str| {
        d.get(n)
            .and_then(|c| c.numbers())
            .ok_or_else(|| format!("{US_IMPORTS}: no numeric {n}"))
    };
    let (hs10, total, air) = (
        numbers("commodity")?,
        numbers("gen_val_yr")?,
        numbers("air_val_yr")?,
    );
    let cty = d
        .get("cty_code")
        .and_then(|c| c.strings())
        .ok_or_else(|| format!("{US_IMPORTS}: no text cty_code"))?;

    let countries_path = data.join(CENSUS_COUNTRIES);
    let text = std::fs::read_to_string(&countries_path)
        .map_err(|e| format!("{}: {e}", countries_path.display()))?;
    let iso2 = census_iso2(&text);
    let admin0 = cache.join(ADMIN0);
    let iso3_of_iso2: BTreeMap<String, String> = Dbf::from_zip(&admin0)?
        .select(["ISO_A2_EH", "ISO_A3_EH"])?
        .into_iter()
        .filter_map(|[a2, a3]| {
            let (a2, a3) = (a2?, a3?);
            (a2 != "-99" && a3 != "-99").then(|| (a2.to_owned(), a3.to_owned()))
        })
        .collect();
    let (iso, _) = read_codes(&admin0)?;
    let index = pack_index(codes, &iso);
    let us = *index.get("USA").ok_or("no USA in the pack")?;
    let origin_of =
        |c: &str| -> Option<usize> { index.get(iso3_of_iso2.get(iso2.get(c)?)?).copied() };

    let key = read_hs_cpa_version(&cache.join(HS_KEY), HS_1996)?;
    let good_of: BTreeMap<i64, usize> = hs_goods(&key)
        .into_iter()
        .filter(|(k, _)| !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit()))
        .map(|(k, g)| (k.parse().expect("digits"), g))
        .collect();

    let mut cells: BTreeMap<(usize, usize), (f64, f64)> = BTreeMap::new();
    let mut notes = UsNotes {
        total: 0.0,
        air: 0.0,
        unmatched_country: 0.0,
        unmatched_codes: Vec::new(),
        unmatched_hs: 0.0,
        local_goods: 0.0,
    };
    for i in 0..hs10.len() {
        let (t, a) = (total[i], air[i]);
        if t.is_nan() || t <= 0.0 {
            continue;
        }
        let a = if a.is_nan() { 0.0 } else { a };
        notes.total += t;
        notes.air += a;
        let Some(origin) = origin_of(&cty[i]).filter(|&o| o != us) else {
            notes.unmatched_country += t;
            if !notes.unmatched_codes.contains(&cty[i]) {
                notes.unmatched_codes.push(cty[i].clone());
            }
            continue;
        };
        let Some(&good) = good_of.get(&((hs10[i] / 1e4).floor() as i64)) else {
            notes.unmatched_hs += t;
            continue;
        };
        if good >= N_TRADED {
            notes.local_goods += t;
            continue;
        }
        let c = cells.entry((origin, good)).or_default();
        c.0 += a;
        c.1 += t;
    }
    notes.unmatched_codes.sort();
    let obs = cells
        .into_iter()
        .map(|((origin, good), (air, total))| Obs {
            importer: us,
            origin,
            good,
            air,
            total,
        })
        .collect();
    Ok((obs, notes))
}

// ---------------------------------------------------------------- the pack's gateway pairs

/// Least-cost legs of the pack graph at one [`Transport`]: each province's access quantities
/// to its country's gateways, and the crossing quantities between all gateways.
struct Legs {
    gateways_of: Vec<Vec<usize>>,
    provinces_of: Vec<Vec<usize>>,
    access: Vec<Vec<Option<Quantities>>>,
    crossing: Vec<Vec<Option<Quantities>>>,
    weight_out: Vec<[f64; N_TRADED]>,
    weight_in: Vec<f64>,
}

fn legs(pack: &Path, p: &Transport) -> Result<Legs, String> {
    let read = |name: &str| Table::open(&pack.join(name));
    let (nodes, edges, gw, prov) = (
        read("nodes.arrow")?,
        read("edges.arrow")?,
        read("gateways.arrow")?,
        read("econ_provinces.arrow")?,
    );
    let countries = read("countries.arrow")?.strings("code")?;
    let index: BTreeMap<&str, usize> = countries
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();
    let country: Vec<usize> = read("provinces.arrow")?
        .strings("country")?
        .iter()
        .map(|c| {
            index
                .get(c.as_str())
                .copied()
                .ok_or_else(|| format!("province country {c} is not a pack country"))
        })
        .collect::<Result<_, _>>()?;
    let (np, nc) = (country.len(), countries.len());
    let va_cols = (0..10)
        .map(|g| prov.f64(&format!("va_{g}")))
        .collect::<Result<Vec<_>, _>>()?;
    let mode: Vec<u8> = edges.usize("mode")?.iter().map(|&m| m as u8).collect();
    let q_edge = edge_quantities(&mode, &edges.f64("length_km")?, &edges.f64("tri_m")?, p);
    let graph = Graph::new(
        nodes.rows(),
        &edges.usize("a")?,
        &edges.usize("b")?,
        &q_edge,
        p,
    );
    let gnode = gw.usize("node")?;
    let gcountry = gw.usize("country")?;
    let provinces_of: Vec<Vec<usize>> = (0..nc)
        .map(|c| (0..np).filter(|&s| country[s] == c).collect())
        .collect();
    let gateways_of: Vec<Vec<usize>> = (0..nc)
        .map(|c| (0..gnode.len()).filter(|&i| gcountry[i] == c).collect())
        .collect();
    let reached = |dist: &[f64], q: &[Quantities], n: usize| dist[n].is_finite().then_some(q[n]);
    let access = (0..np)
        .into_par_iter()
        .map(|s| {
            let (dist, q) = graph.tree(s);
            gateways_of[country[s]]
                .iter()
                .map(|&g| reached(&dist, &q, gnode[g]))
                .collect()
        })
        .collect();
    let crossing = gnode
        .par_iter()
        .map(|&s| {
            let (dist, q) = graph.tree(s);
            gnode.iter().map(|&n| reached(&dist, &q, n)).collect()
        })
        .collect();
    let weight_out = (0..np)
        .map(|s| std::array::from_fn(|g| va_cols[g][s]))
        .collect();
    let weight_in = (0..np)
        .map(|s| pairwise_sum(&(0..10).map(|g| va_cols[g][s]).collect::<Vec<_>>()))
        .collect();
    Ok(Legs {
        gateways_of,
        provinces_of,
        access,
        crossing,
        weight_out,
        weight_in,
    })
}

/// The cells of `obs` on the legs at `p`: access sums by gateway, as the kernel's
/// `v'_a` and `u'_b` without the origin price weights, with exporter weights the good's VA (all
/// VA where the country makes none of it) and importer weights all VA, as in
/// [`crate::costs::country_costs`]. Cells whose countries have no gateway are left out.
fn cells(
    l: &Legs,
    p: &Transport,
    theta: &[f64; N_TRADED],
    weight: &[f64],
    obs: &[Obs],
) -> Vec<Cell> {
    let road = p.per_tkm[0];
    let r = p.per_tkm[AIR as usize] / road;
    obs.iter()
        .filter_map(|o| {
            let g = o.good;
            let (w, lam, th) = (weight[g], TIME_COST[g], theta[g]);
            let k = |q: &Option<Quantities>| q.map_or(0.0, |q| tau(&q, w, lam, p).powf(-th));
            let (po, pd) = (&l.provinces_of[o.origin], &l.provinces_of[o.importer]);
            let (go, gd) = (&l.gateways_of[o.origin], &l.gateways_of[o.importer]);
            if go.is_empty() || gd.is_empty() {
                return None;
            }
            let makes = po.iter().any(|&s| l.weight_out[s][g] > 0.0);
            let v = (0..go.len())
                .map(|a| {
                    po.iter()
                        .map(|&s| {
                            let wo = if makes {
                                l.weight_out[s][g]
                            } else {
                                l.weight_in[s]
                            };
                            wo * k(&l.access[s][a])
                        })
                        .sum()
                })
                .collect();
            let u = (0..gd.len())
                .map(|b| {
                    pd.iter()
                        .map(|&s| l.weight_in[s] * k(&l.access[s][b]))
                        .sum()
                })
                .collect();
            let mut crossings = Vec::new();
            for (a, &ga) in go.iter().enumerate() {
                for (b, &gb) in gd.iter().enumerate() {
                    if let Some(q) = l.crossing[ga][gb] {
                        let slope = w * road * q[AIR as usize];
                        crossings.push(Crossing {
                            a,
                            b,
                            beta: tau(&q, w, lam, p) - slope * r,
                            slope,
                        });
                    }
                }
            }
            Some(Cell {
                cluster: o.importer * l.provinces_of.len() + o.origin,
                share: o.air / o.total,
                weight: o.total,
                theta: th,
                v,
                u,
                crossings,
            })
        })
        .collect()
}

/// One outer step of [`estimate`]: the ratio the legs were solved at and the fit on them.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub at: f64,
    pub fit: AirFit,
    /// Import-value weighted air share, model at the fitted ratio and data, over fitted cells.
    pub model_share: f64,
    pub data_share: f64,
}

pub const OUTER_TOL: f64 = 1e-9;
pub const OUTER_MAX: usize = 20;

/// Fits r on legs solved at r, then solves the legs again at the fitted r, until r moves by
/// less than [`OUTER_TOL`] in logs (the paths repeat). Air km on access legs are held at their
/// path's value within a step.
pub fn estimate(
    pack: &Path,
    obs: &[Obs],
    theta: &[f64; N_TRADED],
    base: &Transport,
) -> Result<Vec<Step>, String> {
    let weight = Table::open(&pack.join("econ_goods.arrow"))?.f64("weight")?;
    let mut r = base.per_tkm[AIR as usize] / base.per_tkm[0];
    let mut steps = Vec::new();
    for _ in 0..OUTER_MAX {
        let mut p = base.clone();
        p.per_tkm[AIR as usize] = r * p.per_tkm[0];
        let cs = cells(&legs(pack, &p)?, &p, theta, &weight, obs);
        let fit = fit(&cs, r)?;
        let used: Vec<&Cell> = cs.iter().filter(|c| c.informative()).collect();
        let total: f64 = used.iter().map(|c| c.weight).sum();
        let model_share = used
            .iter()
            .map(|c| c.weight * c.model_share(fit.ratio).0)
            .sum::<f64>()
            / total;
        let data_share = used.iter().map(|c| c.weight * c.share).sum::<f64>() / total;
        let done = (fit.ratio.ln() - r.ln()).abs() < OUTER_TOL;
        r = fit.ratio;
        steps.push(Step {
            at: p.per_tkm[AIR as usize] / p.per_tkm[0],
            fit,
            model_share,
            data_share,
        });
        if done {
            return Ok(steps);
        }
    }
    Err(format!(
        "air share: legs did not repeat in {OUTER_MAX} steps, last r {r}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_schedule_c_rows() {
        let text = "Produced: 31JAN14\nCode | Name | ISO Code\n5700 | China | CN\n1220 | Canada | CA\n0001 | Bad |\n";
        let m = census_iso2(text);
        assert_eq!(m.get("5700").map(String::as_str), Some("CN"));
        assert_eq!(m.len(), 2);
    }

    /// Two origin and two destination gateways; crossings 0-0 and 1-1 by sea, 0-1 by air.
    fn cell(cluster: usize, km: f64, w: f64, theta: f64) -> Cell {
        let sea = 1.0 + w * (16.0 + 0.0121 * km) + 0.002 * km / 25.0 / 24.0;
        let air_beta = 1.0 + w * 120.0 + 0.002 * (km / 500.0 + 24.0) / 24.0;
        Cell {
            cluster,
            share: 0.0,
            weight: 1.0 + km / 1e4,
            theta,
            v: vec![0.7, 0.3],
            u: vec![0.4, 0.6],
            crossings: vec![
                Crossing {
                    a: 0,
                    b: 0,
                    beta: sea,
                    slope: 0.0,
                },
                Crossing {
                    a: 1,
                    b: 1,
                    beta: sea * 1.1,
                    slope: 0.0,
                },
                Crossing {
                    a: 0,
                    b: 1,
                    beta: air_beta,
                    slope: w * 0.10 * 1.05 * km,
                },
            ],
        }
    }

    fn sample(r: f64) -> Vec<Cell> {
        let mut out = Vec::new();
        for (i, km) in [800.0, 2500.0, 6000.0, 9000.0, 14000.0]
            .into_iter()
            .enumerate()
        {
            for (j, (w, theta)) in [(2e-4, 4.0), (5e-5, 8.0), (1e-3, 2.9)]
                .into_iter()
                .enumerate()
            {
                let mut c = cell(i, km, w, theta);
                c.share = c.model_share(r).0;
                c.weight *= 1.0 + j as f64;
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn recovers_a_known_ratio() {
        let fit = fit(&sample(2.4), 5.37).unwrap();
        assert!((fit.ratio - 2.4).abs() <= 1e-9 * 2.4, "{fit:?}");
        assert!(fit.se < 1e-6, "exact shares leave no residual: {fit:?}");
    }

    #[test]
    fn share_is_the_gateway_pair_sum() {
        // Province level: K_oa and K_bd for 3 origins and 2 destinations, then the full sum.
        let k_oa = [[0.5, 0.1], [0.2, 0.3], [0.05, 0.4]];
        let k_bd = [[0.6, 0.2], [0.1, 0.7]];
        let (wo, wd) = ([0.5, 0.3, 0.2], [0.4, 0.6]);
        let mut c = cell(0, 6000.0, 2e-4, 4.0);
        c.v = (0..2)
            .map(|a| (0..3).map(|o| wo[o] * k_oa[o][a]).sum())
            .collect();
        c.u = (0..2)
            .map(|b| (0..2).map(|d| wd[d] * k_bd[b][d]).sum())
            .collect();
        let r = 3.0;
        let (mut air, mut all) = (0.0, 0.0);
        for o in 0..3 {
            for d in 0..2 {
                for x in &c.crossings {
                    let k = wo[o]
                        * wd[d]
                        * k_oa[o][x.a]
                        * (x.beta + x.slope * r).powf(-4.0)
                        * k_bd[x.b][d];
                    all += k;
                    if x.slope > 0.0 {
                        air += k;
                    }
                }
            }
        }
        assert!((c.model_share(r).0 - air / all).abs() <= 1e-12);
    }

    #[test]
    fn derivative_matches_a_central_difference() {
        let c = cell(0, 9000.0, 2e-4, 4.0);
        let (r, h) = (3.0, 1e-5);
        let fd = (c.model_share(r + h).0 - c.model_share(r - h).0) / (2.0 * h);
        let d = c.model_share(r).1;
        assert!((fd - d).abs() <= 1e-6 * d.abs(), "{fd} {d}");
    }

    #[test]
    fn share_falls_as_air_gets_dearer() {
        let c = cell(0, 6000.0, 2e-4, 4.0);
        let s: Vec<f64> = [1.0, 2.0, 4.0, 8.0]
            .iter()
            .map(|&r| c.model_share(r).0)
            .collect();
        assert!(s.windows(2).all(|p| p[1] < p[0]), "{s:?}");
    }

    #[test]
    fn a_common_border_factor_cancels() {
        let c = cell(0, 6000.0, 2e-4, 4.0);
        let mut scaled = c.clone();
        // (1 + nu)(1 + t) on every crossing multiplies each K_ab by the same factor.
        let f = 1.3_f64;
        for x in &mut scaled.crossings {
            x.beta *= f;
            x.slope *= f;
        }
        assert!((c.model_share(2.0).0 - scaled.model_share(2.0).0).abs() <= 1e-15);
    }

    #[test]
    fn cells_without_both_modes_are_dropped() {
        let mut cells = sample(2.4);
        let mut no_air = cell(9, 6000.0, 2e-4, 4.0);
        no_air.crossings.pop();
        no_air.share = 0.3;
        cells.push(no_air);
        let fit = fit(&cells, 5.0).unwrap();
        assert_eq!((fit.cells, fit.dropped), (15, 1));
    }

    #[test]
    fn a_ratio_below_road_stops() {
        assert!(
            fit(&sample(0.6), 2.0)
                .unwrap_err()
                .contains("does not exceed 1")
        );
    }
}
