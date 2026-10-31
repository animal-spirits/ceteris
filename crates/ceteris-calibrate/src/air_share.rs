//! P04d from 2000 air value shares (ADR 0277; geo SPEC, Air rate from mode shares): the air part
//! of the trade kernel's gateway-pair sum, fitted to the data's share by a Bernoulli
//! quasi-likelihood for fractional data (Papke and Wooldridge 1996).

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

#[cfg(test)]
mod tests {
    use super::*;

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
