//! Sector switching inside a province (P4 Branch C2, ADR 0105, `specs/pop/SPEC.md`): each month a
//! share `m` of the workers of every sector re-chooses by logit over the sectors of the province
//! on log wages net of a switching cost, with dispersion `omega` (not nu, the border friction).
//! The kernel is not yet in the tick: the wage state per (province, sector) is not widened and
//! `m`, `omega` have no register codes.

use crate::admission::hamilton;

#[derive(Clone, Debug, PartialEq)]
pub struct SectorSwitching {
    /// Off: `step` leaves everything as it is.
    pub enabled: bool,
    /// Logit dispersion.
    pub omega: f64,
    /// Monthly share of each sector that re-chooses.
    pub m: f64,
    /// Cost of leaving the own sector, in log wage; zero when staying.
    pub cost: f64,
    pub n_sectors: usize,
    /// Workers `[p * n_sectors + s]`.
    pub workers: Vec<u64>,
}

impl SectorSwitching {
    pub fn new(n_sectors: usize, workers: Vec<u64>) -> Self {
        Self {
            enabled: false,
            omega: 1.0,
            m: 0.0,
            cost: 0.0,
            n_sectors,
            workers,
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.workers.len() / self.n_sectors
    }

    /// One monthly step from the pre-tick counts, given expected wages `[p * n_sectors + s]`.
    /// Workers only change sector: totals per province are conserved.
    pub fn step(&mut self, wage: &[f64]) {
        if !self.enabled {
            return;
        }
        let ns = self.n_sectors;
        for p in 0..self.n_provinces() {
            let u: Vec<f64> = (0..ns)
                .map(|s| libm::log(wage[p * ns + s].max(1e-12)))
                .collect();
            let mut next = vec![0i64; ns];
            for o in 0..ns {
                let stock = self.workers[p * ns + o];
                let movers = libm::round(self.m * stock as f64) as u64;
                let v: Vec<f64> = (0..ns)
                    .map(|d| (u[d] - if d == o { 0.0 } else { self.cost }) / self.omega)
                    .collect();
                let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let z: f64 = v.iter().map(|&x| libm::exp(x - max)).sum();
                let exact: Vec<f64> = v
                    .iter()
                    .map(|&x| libm::exp(x - max) / z * movers as f64)
                    .collect();
                let flow = hamilton(&exact, &vec![movers; ns]);
                for (d, &k) in flow.iter().enumerate() {
                    if d != o {
                        next[o] -= k as i64;
                        next[d] += k as i64;
                    }
                }
            }
            for (s, &delta) in next.iter().enumerate() {
                let i = p * ns + s;
                self.workers[i] = (self.workers[i] as i64 + delta) as u64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toy() -> SectorSwitching {
        let mut s = SectorSwitching::new(2, vec![5000, 5000]);
        s.omega = 0.5;
        s.m = 0.05;
        s
    }

    #[test]
    fn switching_off_equals_status_quo() {
        let mut s = toy();
        let before = s.clone();
        for _ in 0..24 {
            s.step(&[1.0, 3.0]);
        }
        assert_eq!(s, before);
    }

    #[test]
    fn workers_are_conserved_per_province() {
        let mut s = SectorSwitching::new(3, vec![4000, 3000, 3000, 100, 50, 25]);
        s.enabled = true;
        s.omega = 0.4;
        s.m = 0.1;
        s.cost = 0.2;
        for _ in 0..60 {
            s.step(&[1.0, 1.5, 0.7, 2.0, 1.0, 1.0]);
        }
        assert_eq!(s.workers[..3].iter().sum::<u64>(), 10_000);
        assert_eq!(s.workers[3..].iter().sum::<u64>(), 175);
    }

    #[test]
    fn demand_shock_gap_closes_at_the_closed_form_logit_rate() {
        // Zero cost: every mover picks sector 1 with the same share q, so
        // n1' = (1 - m) n1 + m N q and n1 - N q decays by (1 - m) per month.
        let mut s = toy();
        s.enabled = true;
        let wage = [1.0, 1.5];
        let n = 10_000.0;
        let q = 1.0 / (1.0 + libm::exp((libm::log(1.0) - libm::log(1.5)) / s.omega));
        for t in 1..=40 {
            s.step(&wage);
            let want = n * q + (5000.0 - n * q) * libm::pow(1.0 - s.m, t as f64);
            assert!((s.workers[1] as f64 - want).abs() < 3.0, "t={t}");
        }
    }

    #[test]
    fn switching_cost_slows_exit_from_the_shocked_sector() {
        let mut free = toy();
        free.enabled = true;
        let mut costly = free.clone();
        costly.cost = 0.8;
        for _ in 0..12 {
            free.step(&[0.6, 1.0]);
            costly.step(&[0.6, 1.0]);
        }
        assert!(costly.workers[0] > free.workers[0]);
        assert!(free.workers[0] > 0);
    }
}
