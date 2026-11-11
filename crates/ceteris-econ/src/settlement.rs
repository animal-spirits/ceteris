//! Settlement classes inside a province (P4 Branch D3, ADR 0074, `specs/pop/SPEC.md`): persons
//! split across rural, town and city; monthly class logit with agglomeration net of congestion
//! (P167); slum share of the city rising where housing lags (P168). The tick system is 229
//! (`EconWorld::with_settlement`, off by default); the initial class split has no pack source
//! (GHSL) yet, so callers supply it.

use crate::admission::hamilton;
use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

pub const N_CLASS: usize = 3;
pub const RURAL: usize = 0;
pub const TOWN: usize = 1;
pub const CITY: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct Settlement {
    /// Off: `step` leaves everything as it is.
    pub enabled: bool,
    /// Logit dispersion.
    pub nu: f64,
    /// Monthly share of each class that reconsiders.
    pub mu_move: f64,
    /// Agglomeration elasticity of class utility in class size.
    pub agglomeration: [f64; N_CLASS],
    /// Congestion cost per unit of occupancy `N_c / H_c` (P167).
    pub congestion: f64,
    /// Slum share of the city when housing is nil (P168).
    pub slum_max: f64,
    /// Monthly adjustment of the slum share towards its target.
    pub slum_speed: f64,
    /// Moving cost `m[from * N_CLASS + to]`, zero on the diagonal.
    pub cost: [f64; N_CLASS * N_CLASS],
    /// Persons `[p * N_CLASS + c]`.
    pub persons: Vec<u64>,
    /// Housing capacity in persons `[p * N_CLASS + c]`.
    pub housing: Vec<f64>,
    /// Slum share of each province's city, in [0, 1].
    pub slum: Vec<f64>,
}

impl Settlement {
    pub fn new(persons: Vec<u64>, housing: Vec<f64>) -> Self {
        let n = persons.len() / N_CLASS;
        Self {
            enabled: false,
            nu: 1.0,
            mu_move: 0.0,
            agglomeration: [0.0; N_CLASS],
            congestion: 0.0,
            slum_max: 0.0,
            slum_speed: 0.0,
            cost: [0.0; N_CLASS * N_CLASS],
            persons,
            housing,
            slum: vec![0.0; n],
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.persons.len() / N_CLASS
    }

    /// Class utility in province `p` at the current sizes, given class real wages.
    fn utility(&self, p: usize, wage: &[f64]) -> [f64; N_CLASS] {
        let mut u = [0.0; N_CLASS];
        for (c, uc) in u.iter_mut().enumerate() {
            let i = p * N_CLASS + c;
            let n = self.persons[i] as f64;
            *uc = libm::log(wage[i].max(1e-12)) + self.agglomeration[c] * libm::log(n + 1.0)
                - self.congestion * n / self.housing[i].max(1.0);
        }
        u
    }

    /// One monthly step from the pre-tick counts. Persons only change class: totals per
    /// province are conserved.
    pub fn step(&mut self, wage: &[f64]) {
        if !self.enabled {
            return;
        }
        for p in 0..self.n_provinces() {
            let u = self.utility(p, wage);
            let mut next = [0i64; N_CLASS];
            for o in 0..N_CLASS {
                let stock = self.persons[p * N_CLASS + o];
                let movers = libm::round(self.mu_move * stock as f64) as u64;
                let s: Vec<f64> = (0..N_CLASS)
                    .map(|d| (u[d] - self.cost[o * N_CLASS + d]) / self.nu)
                    .collect();
                let max = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let z: f64 = s.iter().map(|&x| libm::exp(x - max)).sum();
                let exact: Vec<f64> = s
                    .iter()
                    .map(|&x| libm::exp(x - max) / z * movers as f64)
                    .collect();
                let flow = hamilton(&exact, &[movers; N_CLASS]);
                for (d, &k) in flow.iter().enumerate() {
                    if d != o {
                        next[o] -= k as i64;
                        next[d] += k as i64;
                    }
                }
            }
            for (c, &delta) in next.iter().enumerate() {
                let i = p * N_CLASS + c;
                self.persons[i] = (self.persons[i] as i64 + delta) as u64;
            }
            let i = p * N_CLASS + CITY;
            let pressure =
                (1.0 - self.housing[i] / (self.persons[i] as f64).max(1.0)).clamp(0.0, 1.0);
            let target = self.slum_max * pressure;
            self.slum[p] += self.slum_speed * (target - self.slum[p]);
        }
    }

    /// Rescales each province's classes to `totals[p]` persons, in proportion to the current
    /// split; migration changes province totals under the class split.
    pub fn sync_totals(&mut self, totals: &[u64]) {
        for (p, &total) in totals.iter().enumerate() {
            let cell = p * N_CLASS..(p + 1) * N_CLASS;
            let sum: u64 = self.persons[cell.clone()].iter().sum();
            if sum == total {
                continue;
            }
            let w: Vec<f64> = if sum == 0 {
                vec![1.0; N_CLASS]
            } else {
                self.persons[cell.clone()]
                    .iter()
                    .map(|&n| n as f64)
                    .collect()
            };
            let scale = total as f64 / w.iter().sum::<f64>();
            let exact: Vec<f64> = w.iter().map(|x| x * scale).collect();
            let out = hamilton(&exact, &[total; N_CLASS]);
            self.persons[cell].copy_from_slice(&out);
        }
    }

    pub fn encode(&self, e: &mut Encoder) {
        let split = |x: u64| [x as u32, (x >> 32) as u32];
        e.bools(&[self.enabled]);
        let mut par = vec![
            self.nu,
            self.mu_move,
            self.congestion,
            self.slum_max,
            self.slum_speed,
        ];
        par.extend(self.agglomeration);
        par.extend(self.cost);
        e.f64s(&par);
        e.u32s(
            &self
                .persons
                .iter()
                .flat_map(|&x| split(x))
                .collect::<Vec<_>>(),
        );
        e.f64s(&self.housing);
        e.f64s(&self.slum);
    }

    pub fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        let enabled = d.bools()?;
        let par = d.f64s()?;
        let words = d.u32s()?;
        let housing = d.f64s()?;
        let slum = d.f64s()?;
        let n = slum.len();
        if enabled.len() != 1
            || par.len() != 5 + N_CLASS + N_CLASS * N_CLASS
            || words.len() != 2 * N_CLASS * n
            || housing.len() != N_CLASS * n
        {
            return Err(DecodeError("settlement section does not fit".into()));
        }
        let persons = words
            .chunks(2)
            .map(|p| u64::from(p[0]) | u64::from(p[1]) << 32)
            .collect();
        let mut s = Settlement::new(persons, housing);
        s.enabled = enabled[0];
        [s.nu, s.mu_move, s.congestion, s.slum_max, s.slum_speed] =
            [par[0], par[1], par[2], par[3], par[4]];
        s.agglomeration.copy_from_slice(&par[5..5 + N_CLASS]);
        s.cost.copy_from_slice(&par[5 + N_CLASS..]);
        s.slum = slum;
        Ok(s)
    }

    /// Persons living in slums in province `p`.
    pub fn slum_persons(&self, p: usize) -> f64 {
        self.slum[p] * self.persons[p * N_CLASS + CITY] as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toy() -> Settlement {
        let mut s = Settlement::new(vec![8000, 1500, 500], vec![9000.0, 3000.0, 1000.0]);
        s.nu = 0.5;
        s.mu_move = 0.02;
        s.agglomeration = [0.0, 0.05, 0.1];
        s.congestion = 0.5;
        s.slum_max = 0.6;
        s.slum_speed = 0.1;
        s.cost = [0.0, 0.3, 0.6, 0.3, 0.0, 0.3, 0.6, 0.3, 0.0];
        s
    }

    const WAGE: [f64; 3] = [1.0, 1.3, 1.8];

    #[test]
    fn settlements_off_equals_status_quo() {
        let mut s = toy();
        let before = s.clone();
        for _ in 0..24 {
            s.step(&WAGE);
        }
        assert_eq!(s, before);
    }

    #[test]
    fn classes_conserve_province_totals_and_shift_towards_cities() {
        let mut s = toy();
        s.enabled = true;
        for _ in 0..120 {
            s.step(&WAGE);
        }
        assert_eq!(s.persons.iter().sum::<u64>(), 10_000);
        assert!(s.persons[CITY] > 500);
        assert!(s.persons[RURAL] < 8000);
    }

    #[test]
    fn congestion_stops_city_growth_at_finite_size() {
        let mut s = toy();
        s.enabled = true;
        s.congestion = 4.0;
        for _ in 0..600 {
            s.step(&WAGE);
        }
        let city = s.persons[CITY];
        for _ in 0..120 {
            s.step(&WAGE);
        }
        assert!(city < 6000);
        assert!(s.persons[CITY].abs_diff(city) < 60);
    }

    #[test]
    fn slums_rise_where_housing_lags_and_stay_nil_where_it_keeps_up() {
        let mut lag = toy();
        lag.enabled = true;
        lag.congestion = 0.0;
        let mut ample = lag.clone();
        ample.housing[CITY] = 1e9;
        for _ in 0..120 {
            lag.step(&WAGE);
            ample.step(&WAGE);
        }
        assert!(lag.slum[0] > 0.1);
        assert_eq!(ample.slum[0], 0.0);
        assert!(lag.slum_persons(0) <= lag.persons[CITY] as f64);
    }
}
