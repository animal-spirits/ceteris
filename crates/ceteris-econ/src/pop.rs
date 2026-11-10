//! Cohort cells, ageing, Brass mortality and tempo fertility (P4 Branch A, `specs/pop/SPEC.md`). Counts are exact integers;
//! every flow is a pair of transfers, so population changes only by births, deaths and moves.

/// Five-year age bands 0-4 .. 95-99 plus 100 and over.
pub const N_AGE: usize = 22;
pub const N_SEX: usize = 2;
/// None or in school, below secondary, secondary, tertiary.
pub const N_EDU: usize = 4;
/// Cells per province.
pub const CELLS: usize = N_AGE * N_SEX * N_EDU;
/// Years per band: the share of a band that ages out each year is `1 / BAND_YEARS`.
const BAND_YEARS: u64 = 5;
/// Ticks (months) per ageing step.
pub const AGEING_PERIOD: u32 = 12;

/// Population by (province, age band, sex, education), province-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cohorts {
    n: Vec<u64>,
    /// Fifths of a person per cell not yet moved out of its band (0..5); carried so repeated
    /// floors do not bias ageing downward.
    residue: Vec<u8>,
    scratch: Vec<u64>,
}

#[inline]
pub fn cell(a: usize, s: usize, e: usize) -> usize {
    (a * N_SEX + s) * N_EDU + e
}

impl Cohorts {
    pub fn new(n_provinces: usize) -> Self {
        Self {
            n: vec![0; n_provinces * CELLS],
            residue: vec![0; n_provinces * CELLS],
            scratch: vec![0; CELLS],
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.n.len() / CELLS
    }

    pub fn get(&self, p: usize, a: usize, s: usize, e: usize) -> u64 {
        self.n[p * CELLS + cell(a, s, e)]
    }

    pub fn set(&mut self, p: usize, a: usize, s: usize, e: usize, v: u64) {
        self.n[p * CELLS + cell(a, s, e)] = v;
    }

    pub fn counts(&self) -> &[u64] {
        &self.n
    }

    pub fn total(&self) -> u64 {
        self.n.iter().sum()
    }

    /// Whether `tick` (the cohort tick count, never a date: ADR 0241) is an ageing tick.
    pub fn is_ageing_tick(tick: u32) -> bool {
        tick.is_multiple_of(AGEING_PERIOD)
    }

    /// Moves persons to the next band once: out of band `a < N_AGE - 1` goes
    /// `(N + residue) / 5`, the remainder carried in the cell's residue; the last band is
    /// absorbing. Outflows are taken from the pre-step counts, so nobody moves twice.
    pub fn age(&mut self) {
        for (n, res) in self
            .n
            .as_chunks_mut::<CELLS>()
            .0
            .iter_mut()
            .zip(self.residue.as_chunks_mut::<CELLS>().0.iter_mut())
        {
            let out = &mut self.scratch;
            for a in 0..N_AGE - 1 {
                for k in 0..N_SEX * N_EDU {
                    let c = a * N_SEX * N_EDU + k;
                    let total = n[c] + u64::from(res[c]);
                    out[c] = total / BAND_YEARS;
                    res[c] = (total % BAND_YEARS) as u8;
                }
            }
            for k in 0..N_SEX * N_EDU {
                out[(N_AGE - 1) * N_SEX * N_EDU + k] = 0;
            }
            for c in 0..CELLS {
                n[c] -= out[c];
                if c >= N_SEX * N_EDU {
                    n[c] += out[c - N_SEX * N_EDU];
                }
            }
        }
    }

    /// Ageing on the monthly clock: acts only on ageing ticks.
    pub fn age_at(&mut self, tick: u32) {
        if Self::is_ageing_tick(tick) {
            self.age();
        }
    }
}

/// Largest-remainder (Hamilton) rounding of `exact` to integers summing to `round(sum)`; ties
/// go to the lowest index, no cell exceeds its cap. Order-independent of threads: sequential.
fn hamilton<const K: usize>(exact: &[f64; K], cap: &[u64; K]) -> [u64; K] {
    let mut out = [0u64; K];
    let mut floors = 0;
    let mut sum = 0.0;
    for k in 0..K {
        out[k] = (exact[k] as u64).min(cap[k]);
        floors += out[k];
        sum += exact[k];
    }
    let mut left = (libm::round(sum) as u64).saturating_sub(floors);
    let mut order: [usize; K] = core::array::from_fn(|k| k);
    let frac = |k: usize| exact[k] - libm::floor(exact[k]);
    order.sort_by(|&i, &j| frac(j).total_cmp(&frac(i)).then(i.cmp(&j)));
    for &k in &order {
        if left == 0 {
            break;
        }
        if out[k] < cap[k] {
            out[k] += 1;
            left -= 1;
        }
    }
    out
}

/// Brass relational life table: `logit(1 - l_x) = alpha_e + beta * logit(1 - l_x^std)`
/// (`specs/pop/SPEC.md`, Mortality). `std_l[s][a]` is survivorship to the start of band `a`
/// (`std_l[s][0] = 1`; the last entry closes the open band and must be positive).
#[derive(Clone, Debug, PartialEq)]
pub struct Brass {
    pub std_l: [[f64; N_AGE + 1]; N_SEX],
    /// Level by education.
    pub alpha: [f64; N_EDU],
    pub beta: f64,
}

impl Brass {
    fn transformed(&self, s: usize, e: usize, a: usize) -> f64 {
        let l = self.std_l[s][a];
        if a == 0 {
            return 1.0;
        }
        let y = self.alpha[e] + self.beta * libm::log((1.0 - l) / l);
        1.0 / (1.0 + libm::exp(y))
    }

    /// Probability of dying in one month for a person in band `a`, from band survival
    /// `l_{a+1} / l_a` spread over 60 months.
    pub fn monthly_q(&self, a: usize, s: usize, e: usize) -> f64 {
        let ratio = self.transformed(s, e, a + 1) / self.transformed(s, e, a);
        1.0 - libm::pow(ratio.clamp(0.0, 1.0), 1.0 / 60.0)
    }

    fn table(&self) -> [f64; CELLS] {
        let mut q = [0.0; CELLS];
        for a in 0..N_AGE {
            for s in 0..N_SEX {
                for e in 0..N_EDU {
                    q[cell(a, s, e)] = self.monthly_q(a, s, e);
                }
            }
        }
        q
    }
}

/// First and one-past-last band of childbearing: 15-19 .. 45-49.
const FERT_BANDS: core::ops::Range<usize> = 3..10;
/// Standard deviation of age at childbearing, years (spread held fixed; only the mean shifts).
const TEMPO_SD: f64 = 5.5;
/// Male births per 100 female births (data constant).
const SEX_RATIO: [f64; N_SEX] = [105.0, 100.0];
/// Sub-points per band when integrating the age-at-birth density.
const SUBPOINTS: usize = 5;

/// Fertility by quantum and tempo (ADR 0032, Bongaarts and Feeney 1998): per (province,
/// education) a completed-fertility quantum `tfr`, a mean age at childbearing `mu` and its
/// drift `mu_rate` (years per year). Period fertility is `tfr * (1 - mu_rate)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Fertility {
    pub tfr: Vec<[f64; N_EDU]>,
    pub mu: Vec<[f64; N_EDU]>,
    pub mu_rate: Vec<[f64; N_EDU]>,
}

impl Fertility {
    pub fn uniform(n_provinces: usize, tfr: f64, mu: f64) -> Self {
        Self {
            tfr: vec![[tfr; N_EDU]; n_provinces],
            mu: vec![[mu; N_EDU]; n_provinces],
            mu_rate: vec![[0.0; N_EDU]; n_provinces],
        }
    }

    /// Share of the age-at-birth distribution in each childbearing band: a gamma density on
    /// age 15 with mean `mu` and fixed spread, truncated to 15-49 and renormalised.
    pub fn band_weights(mu: f64) -> [f64; FERT_BANDS.end - FERT_BANDS.start] {
        let m = (mu - 15.0).max(1.0);
        let shape = m * m / (TEMPO_SD * TEMPO_SD);
        let scale = TEMPO_SD * TEMPO_SD / m;
        let mut w = [0.0; FERT_BANDS.end - FERT_BANDS.start];
        for (i, a) in FERT_BANDS.enumerate() {
            for j in 0..SUBPOINTS {
                let y = (5 * a) as f64 + (j as f64 + 0.5) * 5.0 / SUBPOINTS as f64 - 15.0;
                w[i] += libm::exp((shape - 1.0) * libm::log(y) - y / scale);
            }
        }
        let total: f64 = w.iter().sum();
        w.map(|x| x / total)
    }

    /// Annual births per woman in each childbearing band.
    pub fn asfr(tfr: f64, mu: f64, mu_rate: f64) -> [f64; FERT_BANDS.end - FERT_BANDS.start] {
        let scale = tfr * (1.0 - mu_rate).max(0.0) / 5.0;
        Self::band_weights(mu).map(|x| x * scale)
    }
}

/// Persons born and dead in a step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flows {
    pub births: u64,
    pub deaths: u64,
}

impl Cohorts {
    /// One month of mortality: deaths per (province, band) apportioned over sex and education.
    pub fn die(&mut self, brass: &Brass) -> u64 {
        let q = brass.table();
        let mut deaths = 0;
        for n in self.n.as_chunks_mut::<CELLS>().0.iter_mut() {
            for a in 0..N_AGE {
                let base = a * N_SEX * N_EDU;
                let exact: [f64; N_SEX * N_EDU] =
                    core::array::from_fn(|k| n[base + k] as f64 * q[base + k]);
                let cap: [u64; N_SEX * N_EDU] = core::array::from_fn(|k| n[base + k]);
                for (k, d) in hamilton(&exact, &cap).into_iter().enumerate() {
                    n[base + k] -= d;
                    deaths += d;
                }
            }
        }
        deaths
    }

    /// One month of births: per province, mothers by education and band give the exact
    /// expectation, rounded once and split by sex; newborns enter band 0, education 0.
    pub fn bear(&mut self, fert: &Fertility) -> u64 {
        let mut births = 0;
        for (p, n) in self.n.as_chunks_mut::<CELLS>().0.iter_mut().enumerate() {
            let mut exact = 0.0;
            for e in 0..N_EDU {
                let rates = Fertility::asfr(fert.tfr[p][e], fert.mu[p][e], fert.mu_rate[p][e]);
                for (i, a) in FERT_BANDS.enumerate() {
                    exact += rates[i] / 12.0 * n[cell(a, 1, e)] as f64;
                }
            }
            let total = libm::round(exact) as u64;
            let ratio: f64 = SEX_RATIO.iter().sum();
            let split = hamilton(
                &SEX_RATIO.map(|r| total as f64 * r / ratio),
                &[u64::MAX; N_SEX],
            );
            for (s, b) in split.into_iter().enumerate() {
                n[cell(0, s, 0)] += b;
            }
            births += total;
        }
        births
    }
}

/// Demography on the monthly clock: age (yearly), mortality, fertility, in that order
/// (systems 220 to 222). A `None` mechanism is off and leaves counts as ageing alone does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Demography {
    pub mortality: Option<Brass>,
    pub fertility: Option<Fertility>,
}

impl Demography {
    pub fn step(&self, cohorts: &mut Cohorts, tick: u32) -> Flows {
        cohorts.age_at(tick);
        let deaths = self.mortality.as_ref().map_or(0, |b| cohorts.die(b));
        let births = self.fertility.as_ref().map_or(0, |f| cohorts.bear(f));
        Flows { births, deaths }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn filled(seed: u64, provinces: usize) -> Cohorts {
        let mut c = Cohorts::new(provinces);
        let mut x = seed | 1;
        for v in &mut c.n {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *v = x % 5000;
        }
        c
    }

    #[test]
    fn population_is_conserved_by_cohort_cell() {
        // Ageing is a pure transfer: per province, sex and education the total over bands is
        // unchanged, and no cell goes negative (u64 would panic in debug).
        let mut c = filled(7, 3);
        let before = c.clone();
        for t in 0..120 {
            c.age_at(t);
        }
        assert_eq!(c.total(), before.total());
        for p in 0..3 {
            for s in 0..N_SEX {
                for e in 0..N_EDU {
                    let sum = |c: &Cohorts| (0..N_AGE).map(|a| c.get(p, a, s, e)).sum::<u64>();
                    assert_eq!(sum(&c), sum(&before));
                }
            }
        }
    }

    #[test]
    fn ageing_shifts_bands_once_per_year() {
        let mut c = Cohorts::new(1);
        for a in 0..N_AGE {
            c.set(0, a, 0, 2, 1000);
        }
        let start = c.clone();
        for t in 1..12 {
            c.age_at(t);
        }
        assert_eq!(c, start, "no step off the yearly tick");
        c.age_at(12);
        // One fifth leaves each band exactly once: no one crosses two bands.
        assert_eq!(c.get(0, 0, 0, 2), 800);
        for a in 1..N_AGE - 1 {
            assert_eq!(c.get(0, a, 0, 2), 1000, "band {a}: out 200, in 200");
        }
        assert_eq!(c.get(0, N_AGE - 1, 0, 2), 1200);
    }

    #[test]
    fn residue_removes_the_floor_bias() {
        // 3 persons in band 0: floor(3/5) = 0 forever without the carry; with it, 3 per 5 years.
        let mut c = Cohorts::new(1);
        c.set(0, 0, 0, 0, 3);
        for _ in 0..5 {
            c.age();
        }
        assert_eq!(c.get(0, 1, 0, 0), 3 - c.get(0, 0, 0, 0) - c.get(0, 2, 0, 0));
        assert!(c.get(0, 1, 0, 0) + c.get(0, 2, 0, 0) >= 2);
    }

    proptest! {
        #[test]
        fn ageing_conserves_and_is_deterministic(seed in any::<u64>(), years in 1u32..40) {
            let mut a = filled(seed, 2);
            let mut b = a.clone();
            let total = a.total();
            for t in 0..years * 12 {
                a.age_at(t);
                b.age_at(t);
            }
            prop_assert_eq!(a.total(), total);
            prop_assert_eq!(a, b);
        }
    }

    /// Gompertz-like survivorship, enough to exercise the transform.
    fn std_l() -> [[f64; N_AGE + 1]; N_SEX] {
        let mut l = [[1.0; N_AGE + 1]; N_SEX];
        for (s, row) in l.iter_mut().enumerate() {
            for (a, v) in row.iter_mut().enumerate() {
                let x = 5.0 * a as f64;
                *v = libm::exp(
                    -0.0002 * (1.0 + 0.1 * s as f64) * (libm::exp(0.085 * x) - 1.0) / 0.085 * 1.0,
                )
                .max(1e-6);
            }
        }
        l
    }

    fn brass(alpha: f64) -> Brass {
        Brass {
            std_l: std_l(),
            alpha: [alpha; N_EDU],
            beta: 1.0,
        }
    }

    #[test]
    fn brass_identity_reproduces_the_standard() {
        let b = brass(0.0);
        for a in 0..N_AGE {
            let want = 1.0 - libm::pow(b.std_l[0][a + 1] / b.std_l[0][a], 1.0 / 60.0);
            assert!((b.monthly_q(a, 0, 0) - want).abs() < 1e-12, "band {a}");
        }
        assert!(brass(0.5).monthly_q(10, 0, 0) > b.monthly_q(10, 0, 0));
        assert!(brass(-0.5).monthly_q(10, 0, 0) < b.monthly_q(10, 0, 0));
    }

    #[test]
    fn hamilton_preserves_the_rounded_total() {
        let r = hamilton(&[1.5, 2.25, 0.25, 1.0], &[9; 4]);
        assert_eq!(r.iter().sum::<u64>(), 5);
        assert_eq!(r, [2, 2, 0, 1]);
        assert_eq!(
            hamilton(&[0.5, 0.5], &[9; 2]),
            [1, 0],
            "ties to the lowest index"
        );
        assert_eq!(
            hamilton(&[0.9, 0.9], &[0, 9]),
            [0, 1],
            "capped at the cell count"
        );
    }

    #[test]
    fn mortality_off_equals_status_quo() {
        let mut off = filled(11, 3);
        let mut ageing_only = off.clone();
        let d = Demography::default();
        for t in 0..120 {
            assert_eq!(d.step(&mut off, t), Flows::default());
            ageing_only.age_at(t);
        }
        assert_eq!(off, ageing_only);
    }

    #[test]
    fn deaths_and_births_balance_the_population_change() {
        let mut c = filled(5, 4);
        let mut fert = Fertility::uniform(4, 2.1, 28.0);
        fert.mu_rate[1] = [0.05; N_EDU];
        let d = Demography {
            mortality: Some(brass(0.0)),
            fertility: Some(fert),
        };
        let mut pop = c.total();
        for t in 0..240 {
            let f = d.step(&mut c, t);
            pop = pop + f.births - f.deaths;
            assert_eq!(c.total(), pop);
        }
        assert!(c.total() != filled(5, 4).total());
    }

    #[test]
    fn postponement_lowers_period_tfr_and_not_cohort_fertility() {
        // Bongaarts and Feeney 1998: mean age rising r years per year lowers period TFR by
        // 1 - r while the cohort's completed fertility keeps the quantum.
        let (tfr, mu0, r) = (2.0, 26.0, 0.1);
        let period = |t: f64| Fertility::asfr(tfr, mu0 + r * t, r).iter().sum::<f64>() * 5.0;
        let cohort = |born: f64| {
            (0..35)
                .map(|y| {
                    let t = born + 15.0 + f64::from(y);
                    let band = (y / 5) as usize;
                    Fertility::asfr(tfr, mu0 + r * t, r)[band]
                })
                .sum::<f64>()
        };
        let flat = Fertility::asfr(tfr, mu0, 0.0).iter().sum::<f64>() * 5.0;
        assert!((flat - tfr).abs() < 1e-12);
        assert!((period(30.0) - tfr * (1.0 - r)).abs() < 1e-12);
        let cf = cohort(0.0);
        assert!(
            (cf - tfr).abs() / tfr < 0.06,
            "cohort {cf} vs quantum {tfr}"
        );
        assert!(cf > period(30.0) + 0.1);
    }

    proptest! {
        #[test]
        fn demography_balances_and_is_deterministic(seed in any::<u64>(), years in 1u32..20) {
            let mut a = filled(seed, 2);
            let mut b = a.clone();
            let d = Demography {
                mortality: Some(brass(0.2)),
                fertility: Some(Fertility::uniform(2, 3.0, 29.0)),
            };
            let mut pop = a.total();
            for t in 0..years * 12 {
                let f = d.step(&mut a, t);
                d.step(&mut b, t);
                pop = pop + f.births - f.deaths;
            }
            prop_assert_eq!(a.total(), pop);
            prop_assert_eq!(a, b);
        }
    }
}
