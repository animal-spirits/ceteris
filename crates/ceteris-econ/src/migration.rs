//! Internal migration logit (P4 Branch D1, ADR 0109, `specs/pop/SPEC.md`, Migration). A share
//! `mu_move` of each origin cell reconsiders; destinations are drawn by
//! `mu_od = exp((beta V_d - m_od)/nu) / sum_k exp((beta V_k - m_ok)/nu)`, with values `V` from one
//! Caliendo-Dvorkin-Parro Bellman update per tick. Flows are integers by Hamilton apportionment.
//! [`Migration`] is the state and tick step of system 227 (`EconWorld::with_migration`; off by
//! default). Choices are a sparse [`ChoiceSet`] per origin (itself, anchors, cheapest others up
//! to `K_d`). Shortcuts, tracked in #210: no rule for the pooled country nests, flow utility `ln wage`, no settlement classes (D3).

use crate::admission::{Account, hamilton, ration};
use crate::pop::{CELLS, Cohorts};
use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

/// Log-sum-exp of `x`, stable against large magnitudes.
fn logsumexp(x: &[f64]) -> f64 {
    let max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    max + libm::log(x.iter().map(|&v| libm::exp(v - max)).sum::<f64>())
}

/// Moving-cost form `m_od = per_log_km ln(1 + d_od / scale_km) + border [country_o != country_d]`,
/// zero on the diagonal (`specs/pop/SPEC.md`, Migration). The coefficients are calibrated (D1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovingCost {
    pub per_log_km: f64,
    pub scale_km: f64,
    pub border: f64,
}

const EARTH_RADIUS_KM: f64 = 6371.0088;

fn great_circle_km(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let s = libm::sin((p2 - p1) / 2.0);
    let t = libm::sin((lon2 - lon1).to_radians() / 2.0);
    let a = s * s + libm::cos(p1) * libm::cos(p2) * (t * t);
    2.0 * EARTH_RADIUS_KM * libm::asin(a.min(1.0).sqrt())
}

impl MovingCost {
    /// Row-major `n x n` cost matrix for provinces at `(lon, lat)` degrees (pack centroids).
    pub fn matrix(&self, lon: &[f64], lat: &[f64], country: &[u16]) -> Vec<f64> {
        let n = lon.len();
        assert!(
            lat.len() == n && country.len() == n,
            "one point per province"
        );
        let mut cost = vec![0.0; n * n];
        for o in 0..n {
            for d in 0..n {
                if o != d {
                    let km = great_circle_km(lon[o], lat[o], lon[d], lat[d]);
                    let border = if country[o] == country[d] {
                        0.0
                    } else {
                        self.border
                    };
                    cost[o * n + d] =
                        self.per_log_km * libm::log(1.0 + km / self.scale_km) + border;
                }
            }
        }
        cost
    }

    /// The matrix for the economy provinces of a pack-loaded world (`ProvinceData::pack`).
    pub fn from_pack(
        &self,
        pack: &ceteris_geo::pack::Provinces,
        provinces: &crate::inputs::ProvinceData,
    ) -> Vec<f64> {
        let at = |v: &[f64]| {
            provinces
                .pack
                .iter()
                .map(|&i| v[i as usize])
                .collect::<Vec<_>>()
        };
        self.matrix(&at(&pack.lon), &at(&pack.lat), &provinces.country)
    }
}

/// Sparse choice set (`specs/pop/SPEC.md`, Bounds): per origin province the destinations it
/// considers and the moving costs to them, in CSR layout with ascending destination index.
/// The origin itself is always a member (staying).
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceSet {
    offsets: Vec<u32>,
    dest: Vec<u32>,
    cost: Vec<f64>,
}

impl ChoiceSet {
    /// Per origin: itself, every `anchor` (largest cities, foreign gateways) and the cheapest
    /// remaining destinations (ties to the lowest index) up to `k` members, or all anchors if
    /// they already exceed `k`. `cost` is row-major `n x n`. `k >= n` keeps every destination.
    pub fn build(cost: &[f64], n: usize, k: usize, anchors: &[usize]) -> Self {
        assert_eq!(n * n, cost.len(), "square cost matrix");
        let mut offsets = vec![0u32];
        let (mut dest, mut kept) = (Vec::new(), Vec::new());
        for o in 0..n {
            let row = &cost[o * n..(o + 1) * n];
            let mut member = vec![false; n];
            member[o] = true;
            anchors.iter().for_each(|&a| member[a] = true);
            let mut rest: Vec<usize> = (0..n).filter(|&d| !member[d]).collect();
            rest.sort_by(|&i, &j| row[i].total_cmp(&row[j]).then(i.cmp(&j)));
            let have = member.iter().filter(|&&m| m).count();
            rest.iter()
                .take(k.saturating_sub(have))
                .for_each(|&d| member[d] = true);
            for d in (0..n).filter(|&d| member[d]) {
                dest.push(d as u32);
                kept.push(row[d]);
            }
            offsets.push(dest.len() as u32);
        }
        Self {
            offsets,
            dest,
            cost: kept,
        }
    }

    /// Every destination for every origin.
    pub fn dense(cost: &[f64], n: usize) -> Self {
        Self::build(cost, n, n, &[])
    }

    pub fn n(&self) -> usize {
        self.offsets.len() - 1
    }

    fn row(&self, o: usize) -> std::ops::Range<usize> {
        self.offsets[o] as usize..self.offsets[o + 1] as usize
    }

    /// Destination provinces considered from `o`, ascending.
    pub fn destinations(&self, o: usize) -> &[u32] {
        &self.dest[self.row(o)]
    }
}

/// Scaled utilities `(beta V_d - m_od)/nu` over the choice set of origin `o`.
fn scaled(v: &[f64], set: &ChoiceSet, o: usize, beta: f64, nu: f64) -> Vec<f64> {
    let r = set.row(o);
    set.dest[r.clone()]
        .iter()
        .zip(&set.cost[r])
        .map(|(&d, &m)| (beta * v[d as usize] - m) / nu)
        .collect()
}

/// Destination shares `mu_od` over the choice set of `o` (aligned with
/// [`ChoiceSet::destinations`]), summing to one.
pub fn shares(v: &[f64], set: &ChoiceSet, o: usize, beta: f64, nu: f64) -> Vec<f64> {
    let s = scaled(v, set, o, beta, nu);
    let lse = logsumexp(&s);
    s.iter().map(|&x| libm::exp(x - lse)).collect()
}

/// One value update: `V_o = u_o + nu ln sum_d exp((beta V_d - m_od)/nu)` over the choice set.
pub fn update_values(
    v: &[f64],
    flow_utility: &[f64],
    set: &ChoiceSet,
    beta: f64,
    nu: f64,
) -> Vec<f64> {
    (0..v.len())
        .map(|o| flow_utility[o] + nu * logsumexp(&scaled(v, set, o, beta, nu)))
        .collect()
}

/// Integer flows out of an origin (including the diagonal, who stay) for `stock` workers of
/// which a share `mu_move` reconsider, given the origin's destination `shares`. The movers
/// never exceed `stock` and sum exactly.
pub fn flows(stock: u64, mu_move: f64, shares: &[f64]) -> Vec<u64> {
    let movers = libm::round(mu_move * stock as f64) as u64;
    let exact: Vec<f64> = shares.iter().map(|&s| s * movers as f64).collect();
    hamilton(&exact, &vec![movers; shares.len()])
}

/// Migration state and parameters: costs, values and the per-country admission accounts.
#[derive(Clone, Debug, PartialEq)]
pub struct Migration {
    /// Destinations considered by each origin and the moving costs to them.
    pub choice: ChoiceSet,
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
    pub fn new(choice: ChoiceSet, n_countries: usize, beta: f64, nu: f64, mu_move: f64) -> Self {
        let n = choice.n();
        Self {
            choice,
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
        self.v = update_values(&self.v, &utility, &self.choice, self.beta, self.nu);

        // (origin province, cell, destination province, persons), in fixed iteration order
        let mut domestic: Vec<(usize, usize, usize, u64)> = Vec::new();
        let mut foreign: Vec<Vec<(usize, usize, usize, u64)>> = vec![Vec::new(); nc];
        for o in 0..n {
            let dest = self.choice.destinations(o);
            let share = shares(&self.v, &self.choice, o, self.beta, self.nu);
            for c in 0..CELLS {
                let stock = cohorts.counts()[o * CELLS + c];
                if stock == 0 {
                    continue;
                }
                let f = flows(stock, self.mu_move, &share);
                for (&d, &k) in dest.iter().zip(&f) {
                    let d = d as usize;
                    if d == o || k == 0 {
                        continue;
                    }
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
        e.u32s(&self.choice.offsets);
        e.u32s(&self.choice.dest);
        e.f64s(&self.choice.cost);
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
        let (offsets, dest, cost) = (d.u32s()?, d.u32s()?, d.f64s()?);
        let k = d.f64s()?;
        let v = d.f64s()?;
        let join = |w: &[u32]| -> Vec<u64> {
            w.chunks(2)
                .map(|p| u64::from(p[0]) | u64::from(p[1]) << 32)
                .collect()
        };
        let (cap, used) = (join(&d.u32s()?), join(&d.u32s()?));
        let diaspora = join(&d.u32s()?);
        let n = v.len();
        let well_formed = offsets.len() == n + 1
            && offsets.windows(2).all(|w| w[0] < w[1])
            && offsets[n] as usize == dest.len()
            && dest.len() == cost.len()
            && dest.iter().all(|&x| (x as usize) < n);
        if k.len() != 5 || !well_formed || cap.len() != used.len() {
            return Err(DecodeError("migration section does not fit".into()));
        }
        Ok(Self {
            choice: ChoiceSet {
                offsets,
                dest,
                cost,
            },
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
    fn moving_costs_grow_with_distance_and_borders() {
        let m = MovingCost {
            per_log_km: 0.1,
            scale_km: 100.0,
            border: 0.5,
        };
        // equator points 0, 1 and 10 degrees east; the last is in another country
        let c = m.matrix(&[0.0, 1.0, 10.0], &[0.0; 3], &[0, 0, 1]);
        assert_eq!((c[0], c[4], c[8]), (0.0, 0.0, 0.0));
        assert_eq!(c[1], c[3]);
        assert!(c[1] > 0.0 && c[1] < c[2]);
        let km = 10.0 * 2.0 * EARTH_RADIUS_KM * std::f64::consts::PI / 360.0;
        assert!((c[2] - (0.1 * libm::log(1.0 + km / 100.0) + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn shares_sum_to_one_and_favour_high_value() {
        let v = [1.0, 2.0, 3.0];
        let set = ChoiceSet::dense(&[0.0; 9], 3);
        let s = shares(&v, &set, 0, 1.0, 1.0);
        assert!((s.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(s[2] > s[1] && s[1] > s[0]);
    }

    #[test]
    fn infinite_costs_off_the_diagonal_keep_everyone_home() {
        let v = [1.0, 5.0];
        let set = ChoiceSet::dense(&[0.0, 1e9, 1e9, 0.0], 2);
        assert_eq!(
            flows(1000, 0.5, &shares(&v, &set, 0, 1.0, 1.0)),
            vec![500, 0]
        );
    }

    #[test]
    fn zero_mobility_moves_nobody() {
        let v = [1.0, 5.0];
        let set = ChoiceSet::dense(&[0.0; 4], 2);
        assert_eq!(flows(1000, 0.0, &shares(&v, &set, 0, 1.0, 1.0)), vec![0, 0]);
    }

    #[test]
    fn value_update_is_a_contraction_for_beta_below_one() {
        let cost = ChoiceSet::dense(&[0.0, 0.5, 0.5, 0.0], 2);
        let u = [1.0, 2.0];
        let mut v = vec![0.0, 0.0];
        for _ in 0..400 {
            v = update_values(&v, &u, &cost, 0.9, 1.0);
        }
        let next = update_values(&v, &u, &cost, 0.9, 1.0);
        assert!(v.iter().zip(&next).all(|(a, b)| (a - b).abs() < 1e-9));
    }

    #[test]
    fn sparse_set_keeps_self_anchors_and_the_cheapest() {
        let cost = [
            0.0, 3.0, 1.0, 2.0, //
            3.0, 0.0, 1.0, 2.0, //
            1.0, 1.0, 0.0, 5.0, //
            2.0, 2.0, 5.0, 0.0,
        ];
        let set = ChoiceSet::build(&cost, 4, 3, &[3]);
        assert_eq!(set.destinations(0), &[0, 2, 3]);
        assert_eq!(set.destinations(2), &[0, 2, 3]);
        assert_eq!(set.destinations(3), &[0, 1, 3]);
    }

    #[test]
    fn full_size_sparse_set_is_the_dense_logit() {
        let cost = [0.0, 0.4, 0.9, 0.4, 0.0, 0.2, 0.9, 0.2, 0.0];
        let v = [0.3, 1.1, -0.2];
        let dense = ChoiceSet::dense(&cost, 3);
        assert_eq!(ChoiceSet::build(&cost, 3, 99, &[]), dense);
        let s = shares(&v, &dense, 1, 0.9, 0.5);
        assert!((s.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn excluded_destinations_receive_nothing() {
        let cost = [0.0, 9.0, 1.0, 9.0, 0.0, 9.0, 1.0, 9.0, 0.0];
        let set = ChoiceSet::build(&cost, 3, 2, &[]);
        assert_eq!(set.destinations(0), &[0, 2]);
        let f = flows(900, 1.0, &shares(&[0.0, 50.0, 0.0], &set, 0, 1.0, 1.0));
        assert_eq!(f.len(), 2);
        assert_eq!(f.iter().sum::<u64>(), 900);
    }

    proptest! {
        #[test]
        fn flows_sum_to_movers(
            stock in 0u64..1_000_000,
            mu in 0.0f64..1.0,
            v in proptest::collection::vec(-5.0f64..5.0, 4),
            c in proptest::collection::vec(0.0f64..5.0, 16),
        ) {
            let f = flows(stock, mu, &shares(&v, &ChoiceSet::dense(&c, 4), 1, 0.95, 0.7));
            prop_assert_eq!(f.iter().sum::<u64>(), libm::round(mu * stock as f64) as u64);
        }
    }
}
