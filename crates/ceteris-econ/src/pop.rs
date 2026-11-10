//! Cohort cells and ageing (P4 Branch A1, `specs/pop/SPEC.md`). Counts are exact integers;
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
}
