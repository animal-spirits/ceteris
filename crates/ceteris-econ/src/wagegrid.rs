//! Wage state per (province, sector, skill) (P4 Branches C1 and C2, ADR 0105, `specs/pop/SPEC.md`),
//! a side state beside `EconWorld::wage` (system 225, `EconWorld::with_wage_grid`, off by
//! default). The labour market (E1, E2) still clears one wage per province; each month the grid
//! splits that wage over sectors and skills by the CES marginal products of [`SkillCes`] (the
//! employment-weighted mean of a sector's two wages is the province wage) and the workers of each
//! skill re-choose sector on those wages ([`SectorSwitching`]). The grid does not feed back
//! into E1. Worker counts have no pack source yet, so callers supply them; `s`, `alpha`, `m` and
//! `omega_s` are fields until their register codes are assigned in a spec PR.

use crate::sectors::SectorSwitching;
use crate::skills::SkillCes;
use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

/// Leading word of the snapshot section; no length prefix of another section reaches it.
pub const SNAPSHOT_TAG: u32 = 0x3A6E_6C01;

pub const LOW: usize = 0;
pub const HIGH: usize = 1;
pub const N_SKILL: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct WageGrid {
    /// Off: `step` leaves everything as it is.
    pub enabled: bool,
    pub skills: SkillCes,
    /// Sector switching per skill group, indexed by [`LOW`], [`HIGH`].
    pub switching: [SectorSwitching; N_SKILL],
    /// Wages `[(p * n_sectors + s) * N_SKILL + k]`, as of the last step.
    pub wage: Vec<f64>,
}

impl WageGrid {
    /// Workers `[p * n_sectors + s]` per skill group, wages starting at 1.
    pub fn new(skills: SkillCes, n_sectors: usize, low: Vec<u64>, high: Vec<u64>) -> Self {
        assert_eq!(low.len(), high.len());
        let n = low.len();
        Self {
            enabled: false,
            skills,
            switching: [
                SectorSwitching::new(n_sectors, low),
                SectorSwitching::new(n_sectors, high),
            ],
            wage: vec![1.0; n * N_SKILL],
        }
    }

    pub fn n_sectors(&self) -> usize {
        self.switching[LOW].n_sectors
    }

    pub fn n_provinces(&self) -> usize {
        self.switching[LOW].n_provinces()
    }

    /// One monthly step given the province wages the labour market cleared.
    pub fn step(&mut self, province_wage: &[f64]) {
        if !self.enabled {
            return;
        }
        let ns = self.n_sectors();
        for (p, &wbar) in province_wage.iter().enumerate() {
            for s in 0..ns {
                let i = p * ns + s;
                let (l, h) = (
                    self.switching[LOW].workers[i],
                    self.switching[HIGH].workers[i],
                );
                let (wl, wh) = if l == 0 || h == 0 {
                    (wbar, wbar)
                } else {
                    let (l, h) = (l as f64, h as f64);
                    let (mh, ml) = self.skills.marginal(h, l);
                    let mean = (mh * h + ml * l) / (h + l);
                    (wbar * ml / mean, wbar * mh / mean)
                };
                self.wage[i * N_SKILL + LOW] = wl;
                self.wage[i * N_SKILL + HIGH] = wh;
            }
        }
        for k in [LOW, HIGH] {
            let wage: Vec<f64> = self.wage.chunks(N_SKILL).map(|c| c[k]).collect();
            self.switching[k].step(&wage);
        }
    }

    pub fn encode(&self, e: &mut Encoder) {
        let split = |x: u64| [x as u32, (x >> 32) as u32];
        e.u32(SNAPSHOT_TAG);
        e.bools(&[
            self.enabled,
            self.skills.enabled,
            self.switching[LOW].enabled,
            self.switching[HIGH].enabled,
        ]);
        let mut par = vec![self.skills.s, self.skills.alpha];
        for k in &self.switching {
            par.extend([k.omega, k.m, k.cost]);
        }
        e.f64s(&par);
        e.u32(self.n_sectors() as u32);
        for k in &self.switching {
            e.u32s(&k.workers.iter().flat_map(|&x| split(x)).collect::<Vec<_>>());
        }
        e.f64s(&self.wage);
    }

    pub fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        if d.u32()? != SNAPSHOT_TAG {
            return Err(DecodeError("wage grid section tag".into()));
        }
        let flags = d.bools()?;
        let par = d.f64s()?;
        let ns = d.u32()? as usize;
        let mut workers = Vec::new();
        for _ in 0..N_SKILL {
            let words = d.u32s()?;
            workers.push(
                words
                    .chunks(2)
                    .map(|p| u64::from(p[0]) | u64::from(p[1]) << 32)
                    .collect::<Vec<u64>>(),
            );
        }
        let wage = d.f64s()?;
        let high = workers.pop().unwrap_or_default();
        let low = workers.pop().unwrap_or_default();
        if flags.len() != 4
            || par.len() != 2 + 3 * N_SKILL
            || ns == 0
            || low.len() != high.len()
            || low.len() % ns != 0
            || wage.len() != low.len() * N_SKILL
        {
            return Err(DecodeError("wage grid section does not fit".into()));
        }
        let mut g = WageGrid::new(SkillCes::new(par[0], par[1]), ns, low, high);
        g.enabled = flags[0];
        g.skills.enabled = flags[1];
        for (k, sw) in g.switching.iter_mut().enumerate() {
            sw.enabled = flags[2 + k];
            [sw.omega, sw.m, sw.cost] = [par[2 + 3 * k], par[3 + 3 * k], par[4 + 3 * k]];
        }
        g.wage = wage;
        Ok(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toy() -> WageGrid {
        // one province, two sectors; sector 0 is skill-intensive
        let mut g = WageGrid::new(
            SkillCes::new(1.6, 0.5),
            2,
            vec![2000, 8000],
            vec![6000, 2000],
        );
        for k in &mut g.switching {
            k.omega = 0.5;
            k.m = 0.05;
        }
        g
    }

    #[test]
    fn off_leaves_the_grid_unchanged() {
        let mut g = toy();
        g.skills.enabled = true;
        g.switching.iter_mut().for_each(|k| k.enabled = true);
        let before = g.clone();
        g.step(&[1.0]);
        assert_eq!(g, before);
    }

    #[test]
    fn wages_differ_by_cell_and_average_to_the_province_wage() {
        let mut g = toy();
        g.enabled = true;
        g.skills.enabled = true;
        g.step(&[2.0]);
        for s in 0..2 {
            let i = s;
            let (l, h) = (
                g.switching[LOW].workers[i] as f64,
                g.switching[HIGH].workers[i] as f64,
            );
            let (wl, wh) = (g.wage[i * 2 + LOW], g.wage[i * 2 + HIGH]);
            assert_ne!(wh, wl);
            assert!(((wl * l + wh * h) / (l + h) - 2.0).abs() < 1e-12);
        }
        // sector 1 is where the high group is scarce, so it pays it more than sector 0 does
        assert!(g.wage[2 + HIGH] > g.wage[HIGH]);
    }

    #[test]
    fn skills_off_gives_one_wage_per_province() {
        let mut g = toy();
        g.enabled = true;
        g.step(&[1.5]);
        assert!(g.wage.iter().all(|&w| w == 1.5));
    }

    #[test]
    fn switching_conserves_workers_per_province_and_skill() {
        let mut g = toy();
        g.enabled = true;
        g.skills.enabled = true;
        g.switching.iter_mut().for_each(|k| k.enabled = true);
        let totals: Vec<u64> = g.switching.iter().map(|k| k.workers.iter().sum()).collect();
        for _ in 0..24 {
            g.step(&[1.0]);
        }
        for (k, t) in g.switching.iter().zip(totals) {
            assert_eq!(k.workers.iter().sum::<u64>(), t);
        }
        assert_ne!(g.switching[HIGH].workers, vec![6000, 2000]);
    }

    #[test]
    fn snapshot_round_trips() {
        let mut g = toy();
        g.enabled = true;
        g.skills.enabled = true;
        g.step(&[1.2]);
        let mut e = Encoder::default();
        g.encode(&mut e);
        let bytes = e.into_bytes();
        let back = WageGrid::decode(&mut Decoder::new(&bytes)).unwrap();
        assert_eq!(back, g);
    }
}
