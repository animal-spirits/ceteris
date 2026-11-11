//! Endemic disease state (P4 Branch B2, ADR 0167, `specs/pop/SPEC.md`): prevalence by
//! (province, age band, disease) with treatment and prevention coverage levers. Prevalence feeds an
//! additive monthly death probability and a labour-loss factor; the kernel is not yet in the tick.

use crate::pop::{Cohorts, N_AGE, N_EDU, N_SEX};

/// HIV, malaria, TB.
pub const N_DISEASE: usize = 3;
pub const HIV: usize = 0;
pub const MALARIA: usize = 1;
pub const TB: usize = 2;
/// Working-age bands 15-19 .. 60-64.
const WORKING: core::ops::Range<usize> = 3..13;

/// Per-disease coefficients, monthly, fitted offline per region (ADR 0167).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiseaseParams {
    /// New cases per susceptible.
    pub incidence: f64,
    /// Share of incidence removed at full prevention coverage (bednets).
    pub prevention: f64,
    /// Monthly exit from prevalence (recovery, or death counted in `excess_q`) untreated.
    pub exit: f64,
    /// Extra monthly exit at full treatment coverage (TB cure).
    pub treat_exit: f64,
    /// Monthly death probability per prevalent untreated person, over the cause-group path.
    pub excess_q: f64,
    /// Share of excess mortality and labour loss removed at full treatment coverage (ART).
    pub treat_eff: f64,
    /// Effective-labour loss per prevalent untreated working-age person.
    pub labour_loss: f64,
}

/// Coverage in [0, 1] per (province, disease), each lever a prevention or treatment share.
pub type Coverage = Vec<[f64; N_DISEASE]>;

/// Coverage a health budget buys: `budget / (unit_cost * persons)`, capped at 1.
pub fn coverage_from_budget(budget: f64, unit_cost: f64, persons: f64) -> f64 {
    if persons <= 0.0 || unit_cost <= 0.0 {
        return 1.0;
    }
    (budget / (unit_cost * persons)).clamp(0.0, 1.0)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Disease {
    pub params: [DiseaseParams; N_DISEASE],
    /// `x[(p * N_AGE + a) * N_DISEASE + d]`, shares in [0, 1].
    pub prevalence: Vec<f64>,
}

impl Disease {
    pub fn zero(n_provinces: usize, params: [DiseaseParams; N_DISEASE]) -> Self {
        Self {
            params,
            prevalence: vec![0.0; n_provinces * N_AGE * N_DISEASE],
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.prevalence.len() / (N_AGE * N_DISEASE)
    }

    /// One month of incidence and exit under coverage `cov`.
    pub fn step(&mut self, cov: &[[f64; N_DISEASE]]) {
        for (i, x) in self.prevalence.iter_mut().enumerate() {
            let (p, d) = (i / (N_AGE * N_DISEASE), i % N_DISEASE);
            let (k, c) = (&self.params[d], cov[p][d]);
            let inflow = k.incidence * (1.0 - k.prevention * c) * (1.0 - *x);
            let outflow = (k.exit + k.treat_exit * c) * *x;
            *x = (*x + inflow - outflow).clamp(0.0, 1.0);
        }
    }

    /// Additive monthly death probability per (province, band), `[p * N_AGE + a]`.
    pub fn excess_q(&self, cov: &[[f64; N_DISEASE]]) -> Vec<f64> {
        let mut out = vec![0.0; self.prevalence.len() / N_DISEASE];
        for (pa, q) in out.iter_mut().enumerate() {
            let p = pa / N_AGE;
            for (d, k) in self.params.iter().enumerate() {
                *q += self.prevalence[pa * N_DISEASE + d]
                    * k.excess_q
                    * (1.0 - k.treat_eff * cov[p][d]);
            }
        }
        out
    }

    /// Effective labour per working-age person in province `p`, population-weighted over
    /// working bands, in (0, 1].
    pub fn labour_factor(&self, cohorts: &Cohorts, p: usize, cov: &[[f64; N_DISEASE]]) -> f64 {
        let (mut lost, mut total) = (0.0, 0.0);
        for a in WORKING {
            let n: u64 = (0..N_SEX)
                .flat_map(|s| (0..N_EDU).map(move |e| (s, e)))
                .map(|(s, e)| cohorts.get(p, a, s, e))
                .sum();
            let loss: f64 = (0..N_DISEASE)
                .map(|d| {
                    let k = &self.params[d];
                    self.prevalence[(p * N_AGE + a) * N_DISEASE + d]
                        * k.labour_loss
                        * (1.0 - k.treat_eff * cov[p][d])
                })
                .sum();
            lost += n as f64 * loss.min(1.0);
            total += n as f64;
        }
        if total == 0.0 {
            1.0
        } else {
            1.0 - lost / total
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pop::{Brass, toy};

    fn params() -> [DiseaseParams; N_DISEASE] {
        [
            DiseaseParams {
                incidence: 0.002,
                prevention: 0.0,
                exit: 0.0,
                treat_exit: 0.0,
                excess_q: 0.01,
                treat_eff: 0.9,
                labour_loss: 0.4,
            },
            DiseaseParams {
                incidence: 0.01,
                prevention: 0.5,
                exit: 0.05,
                treat_exit: 0.0,
                excess_q: 0.0005,
                treat_eff: 0.0,
                labour_loss: 0.1,
            },
            DiseaseParams {
                incidence: 0.001,
                prevention: 0.0,
                exit: 0.01,
                treat_exit: 0.1,
                excess_q: 0.003,
                treat_eff: 0.8,
                labour_loss: 0.2,
            },
        ]
    }

    #[test]
    fn zero_prevalence_equals_the_cause_group_path() {
        let (d, cohorts) = toy(2);
        let brass: Brass = d.mortality.unwrap();
        let mut plain = cohorts.clone();
        let mut with = cohorts;
        let dis = Disease::zero(2, params());
        let cov = vec![[0.0; N_DISEASE]; 2];
        let extra = dis.excess_q(&cov);
        for _ in 0..24 {
            let a = plain.die(&brass);
            let b = with.die_with(&brass, Some(&extra));
            assert_eq!(a, b);
        }
        assert_eq!(plain, with);
    }

    #[test]
    fn prevalence_adds_deaths_and_treatment_removes_them() {
        let (d, cohorts) = toy(1);
        let brass = d.mortality.unwrap();
        let mut dis = Disease::zero(1, params());
        dis.prevalence.iter_mut().for_each(|x| *x = 0.1);
        let deaths = |cov: f64| {
            let c = vec![[cov; N_DISEASE]];
            cohorts.clone().die_with(&brass, Some(&dis.excess_q(&c)))
        };
        let base = cohorts.clone().die(&brass);
        assert!(deaths(0.0) > base);
        assert!(deaths(1.0) < deaths(0.0));
    }

    #[test]
    fn art_scale_up_raises_labour_supply_in_a_toy_province() {
        let (_, cohorts) = toy(1);
        let mut dis = Disease::zero(1, params());
        for a in 0..N_AGE {
            dis.prevalence[a * N_DISEASE + HIV] = 0.2;
        }
        let at = |c: f64| dis.labour_factor(&cohorts, 0, &[[c, 0.0, 0.0]]);
        assert!(at(0.0) < 1.0);
        assert!(at(0.5) > at(0.0));
        assert!(at(1.0) > at(0.5));
        assert_eq!(
            Disease::zero(1, params()).labour_factor(&cohorts, 0, &[[0.0; 3]]),
            1.0
        );
    }

    #[test]
    fn prevalence_stays_in_unit_interval_and_bednets_lower_the_malaria_level() {
        let run = |c: f64| {
            let mut dis = Disease::zero(1, params());
            let cov = vec![[0.0, c, 0.0]];
            for _ in 0..600 {
                dis.step(&cov);
            }
            dis.prevalence[MALARIA]
        };
        assert!((0.0..=1.0).contains(&run(0.0)));
        assert!(run(1.0) < run(0.0));
    }

    #[test]
    fn budget_buys_coverage_up_to_full() {
        assert_eq!(coverage_from_budget(50.0, 1.0, 100.0), 0.5);
        assert_eq!(coverage_from_budget(500.0, 1.0, 100.0), 1.0);
        assert_eq!(coverage_from_budget(-1.0, 1.0, 100.0), 0.0);
    }
}
