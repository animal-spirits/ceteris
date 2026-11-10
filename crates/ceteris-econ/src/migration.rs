//! Internal migration logit (P4 Branch D1, ADR 0109, `specs/pop/SPEC.md`, Migration). A share
//! `mu_move` of each origin cell reconsiders; destinations are drawn by
//! `mu_od = exp((beta V_d - m_od)/nu) / sum_k exp((beta V_k - m_ok)/nu)`, with values `V` from one
//! Caliendo-Dvorkin-Parro Bellman update per tick. Flows are integers by Hamilton apportionment.
//! [`Migration`] is the state and tick step of system 227 (`EconWorld::with_migration`; off by
//! default). Shortcuts, tracked in #210: dense province-by-province costs instead of the sparse
//! choice set, no pack costs (the caller supplies them), flow utility `ln wage`, no settlement
//! classes (D3).

use crate::admission::{Account, hamilton, ration};
use crate::pop::{CELLS, Cohorts};
use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

/// Log-sum-exp of `x`, stable against large magnitudes.
fn logsumexp(x: &[f64]) -> f64 {
    let max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    max + libm::log(x.iter().map(|&v| libm::exp(v - max)).sum::<f64>())
}

/// Scaled utilities `(beta V_d - m_od)/nu` over destinations for origin `o`; `cost` is row-major
/// with `cost[o * n + d]`, zero on the diagonal.
fn scaled(v: &[f64], cost: &[f64], o: usize, beta: f64, nu: f64) -> Vec<f64> {
    let n = v.len();
    (0..n)
        .map(|d| (beta * v[d] - cost[o * n + d]) / nu)
        .collect()
}

/// Destination shares `mu_od` for origin `o`, summing to one.
pub fn shares(v: &[f64], cost: &[f64], o: usize, beta: f64, nu: f64) -> Vec<f64> {
    let s = scaled(v, cost, o, beta, nu);
    let lse = logsumexp(&s);
    s.iter().map(|&x| libm::exp(x - lse)).collect()
}

/// One value update: `V_o = u_o + nu ln sum_d exp((beta V_d - m_od)/nu)`.
pub fn update_values(
    v: &[f64],
    flow_utility: &[f64],
    cost: &[f64],
    beta: f64,
    nu: f64,
) -> Vec<f64> {
    (0..v.len())
        .map(|o| flow_utility[o] + nu * logsumexp(&scaled(v, cost, o, beta, nu)))
        .collect()
}

/// Integer flows out of origin `o` (including the diagonal, who stay) for `stock` workers of
/// which a share `mu_move` reconsider. The movers never exceed `stock` and sum exactly.
pub fn flows(
    stock: u64,
    mu_move: f64,
    v: &[f64],
    cost: &[f64],
    o: usize,
    beta: f64,
    nu: f64,
) -> Vec<u64> {
    let movers = libm::round(mu_move * stock as f64) as u64;
    let exact: Vec<f64> = shares(v, cost, o, beta, nu)
        .iter()
        .map(|&s| s * movers as f64)
        .collect();
    hamilton(&exact, &vec![movers; v.len()])
}

/// Migration state and parameters: costs, values and the per-country admission accounts.
#[derive(Clone, Debug, PartialEq)]
pub struct Migration {
    /// Moving costs `m_od`, row-major over provinces, zero on the diagonal.
    pub cost: Vec<f64>,
    pub beta: f64,
    pub nu: f64,
    /// Monthly share of each cell that reconsiders.
    pub mu_move: f64,
    /// Diaspora preference in admissions.
    pub chi: f64,
    /// Share of rejected applicants who migrate irregularly.
    pub z: f64,
    /// Values `V` per province, carried between ticks.
    pub v: Vec<f64>,
    /// Admission cap and admissions this year, per destination country.
    pub accounts: Vec<Account>,
    /// Persons of origin country `i` settled in country `j` by migration, `i * n_countries + j`.
    pub diaspora: Vec<u64>,
}

impl Migration {
    pub fn new(cost: Vec<f64>, n_countries: usize, beta: f64, nu: f64, mu_move: f64) -> Self {
        let n = (cost.len() as f64).sqrt() as usize;
        assert_eq!(n * n, cost.len(), "square cost matrix");
        Self {
            cost,
            beta,
            nu,
            mu_move,
            chi: 0.0,
            z: 0.0,
            v: vec![0.0; n],
            accounts: vec![Account::uncapped(); n_countries],
            diaspora: vec![0; n_countries * n_countries],
        }
    }

    /// One monthly step: updates `V` from `wage`, draws every origin cell's flows from the
    /// pre-tick counts, rations cross-border applications per destination country, and moves
    /// the admitted and irregular. Persons only change province: the total is conserved.
    pub fn step(&mut self, cohorts: &mut Cohorts, wage: &[f64], country: &[usize], tick: u32) {
        let n = self.v.len();
        let nc = self.accounts.len();
        let month = u64::from(tick % 12);
        if month == 0 {
            self.accounts.iter_mut().for_each(Account::new_year);
        }
        let utility: Vec<f64> = wage.iter().map(|&w| libm::log(w.max(1e-12))).collect();
        self.v = update_values(&self.v, &utility, &self.cost, self.beta, self.nu);

        // (origin province, cell, destination province, persons), in fixed iteration order
        let mut domestic: Vec<(usize, usize, usize, u64)> = Vec::new();
        let mut foreign: Vec<Vec<(usize, usize, usize, u64)>> = vec![Vec::new(); nc];
        for o in 0..n {
            for c in 0..CELLS {
                let stock = cohorts.counts()[o * CELLS + c];
                if stock == 0 {
                    continue;
                }
                let f = flows(
                    stock,
                    self.mu_move,
                    &self.v,
                    &self.cost,
                    o,
                    self.beta,
                    self.nu,
                );
                for (d, &k) in f.iter().enumerate().filter(|&(d, &k)| d != o && k > 0) {
                    if country[d] == country[o] {
                        domestic.push((o, c, d, k));
                    } else {
                        foreign[country[d]].push((o, c, d, k));
                    }
                }
            }
        }
        let mut moves = domestic;
        for (dc, apps) in foreign.iter().enumerate() {
            if apps.is_empty() {
                continue;
            }
            let a: Vec<u64> = apps.iter().map(|m| m.3).collect();
            let share: Vec<f64> = apps
                .iter()
                .map(|m| {
                    let stock = self.diaspora[country[m.0] * nc + dc] as f64;
                    stock / (stock + 1.0)
                })
                .collect();
            let room = self.accounts[dc].room(month);
            let out = ration(&a, &share, room, self.chi, self.z);
            self.accounts[dc].record(out.admitted.iter().sum());
            for (i, &(o, c, d, _)) in apps.iter().enumerate() {
                let k = out.admitted[i] + out.irregular[i];
                if k > 0 {
                    self.diaspora[country[o] * nc + dc] += k;
                    moves.push((o, c, d, k));
                }
            }
        }
        let n_cells = cohorts.counts().len();
        let mut next = cohorts.counts().to_vec();
        for (o, c, d, k) in moves {
            next[o * CELLS + c] -= k;
            next[d * CELLS + c] += k;
        }
        debug_assert_eq!(next.len(), n_cells);
        cohorts.replace_counts(next);
    }

    pub fn encode(&self, e: &mut Encoder) {
        e.f64s(&self.cost);
        e.f64s(&[self.beta, self.nu, self.mu_move, self.chi, self.z]);
        e.f64s(&self.v);
        let split = |x: u64| [x as u32, (x >> 32) as u32];
        e.u32s(
            &self
                .accounts
                .iter()
                .flat_map(|a| split(a.cap))
                .collect::<Vec<_>>(),
        );
        e.u32s(
            &self
                .accounts
                .iter()
                .flat_map(|a| split(a.used))
                .collect::<Vec<_>>(),
        );
        e.u32s(
            &self
                .diaspora
                .iter()
                .flat_map(|&x| split(x))
                .collect::<Vec<_>>(),
        );
    }

    pub fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let cost = d.f64s()?;
        let k = d.f64s()?;
        let v = d.f64s()?;
        let join = |w: &[u32]| -> Vec<u64> {
            w.chunks(2)
                .map(|p| u64::from(p[0]) | u64::from(p[1]) << 32)
                .collect()
        };
        let (cap, used) = (join(&d.u32s()?), join(&d.u32s()?));
        let diaspora = join(&d.u32s()?);
        if k.len() != 5 || cost.len() != v.len() * v.len() || cap.len() != used.len() {
            return Err(DecodeError("migration section does not fit".into()));
        }
        Ok(Self {
            cost,
            beta: k[0],
            nu: k[1],
            mu_move: k[2],
            chi: k[3],
            z: k[4],
            v,
            accounts: cap
                .iter()
                .zip(&used)
                .map(|(&cap, &used)| Account { cap, used })
                .collect(),
            diaspora,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn shares_sum_to_one_and_favour_high_value() {
        let v = [1.0, 2.0, 3.0];
        let cost = [0.0; 9];
        let s = shares(&v, &cost, 0, 1.0, 1.0);
        assert!((s.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(s[2] > s[1] && s[1] > s[0]);
    }

    #[test]
    fn infinite_costs_off_the_diagonal_keep_everyone_home() {
        let v = [1.0, 5.0];
        let cost = [0.0, 1e9, 1e9, 0.0];
        assert_eq!(flows(1000, 0.5, &v, &cost, 0, 1.0, 1.0), vec![500, 0]);
    }

    #[test]
    fn zero_mobility_moves_nobody() {
        let v = [1.0, 5.0];
        let cost = [0.0; 4];
        assert_eq!(flows(1000, 0.0, &v, &cost, 0, 1.0, 1.0), vec![0, 0]);
    }

    #[test]
    fn value_update_is_a_contraction_for_beta_below_one() {
        let cost = [0.0, 0.5, 0.5, 0.0];
        let u = [1.0, 2.0];
        let mut v = vec![0.0, 0.0];
        for _ in 0..400 {
            v = update_values(&v, &u, &cost, 0.9, 1.0);
        }
        let next = update_values(&v, &u, &cost, 0.9, 1.0);
        assert!(v.iter().zip(&next).all(|(a, b)| (a - b).abs() < 1e-9));
    }

    proptest! {
        #[test]
        fn flows_sum_to_movers(
            stock in 0u64..1_000_000,
            mu in 0.0f64..1.0,
            v in proptest::collection::vec(-5.0f64..5.0, 4),
            c in proptest::collection::vec(0.0f64..5.0, 16),
        ) {
            let f = flows(stock, mu, &v, &c, 1, 0.95, 0.7);
            prop_assert_eq!(f.iter().sum::<u64>(), libm::round(mu * stock as f64) as u64);
        }
    }
}
