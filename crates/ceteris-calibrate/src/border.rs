//! The border friction (P05; ADRs 0092, 0276; `specs/econ/SPEC.md`, Calibration): one nu per
//! friction group, a theta class's goods, except class [`PER_GOOD_CLASS`], each of whose goods
//! is a group of its own. nu solves, given theta, the cross-border share of the group's goods in
//! ICIO 2000.
//!
//! The moment of a group is the share of the ICIO economies' use of its goods that comes from
//! abroad, sum over those economies and goods of (1 - home share) absorption over sum of
//! absorption: from `econ_home_shares.arrow` for the target ([`target_shares`]), and from the
//! value the model's destinations buy, at origin prices, for the model ([`cross_border_shares`]).
//! Each evaluation is stage 2 of ADR 0089 (the scaling that makes every origin sell its output)
//! at stage 1's spending of the data state, held for every trial: a group's share then depends
//! on its own nu only, and the groups are 1-D roots, found together by Brent's method in nu.

use std::path::Path;

use ceteris_reference::io::{Inputs, Mat};
use ceteris_reference::model::Model;

use crate::steady::{self, CalibrationError, Limits};
use crate::write::Table;

pub use ceteris_trade::N_CLASSES;
/// The theta class whose goods take a border friction each (ADR 0276): crude oil, gas and
/// refined fuels, and coal and other mining, whose ICIO cross-border shares differ and whose
/// one class value chokes the second's trade.
pub const PER_GOOD_CLASS: usize = 3;
/// The bracket of nu searched: no friction, and a friction ten times the good's value, which cuts
/// the frictionless kernel to 9e-4 at theta 2.91 and 2e-18 at 16.91.
pub const NU_MAX: f64 = 10.0;
/// Brent ends when every group's bracket is this narrow in nu on the computed shares. The root
/// itself is known only to the scaling's noise in the share over the share's slope in nu: on the
/// fixture about 7e-9 for class 3, whose share moves by 2.7e-7 per unit nu (the scaling stops
/// at 1e-13, which leaves about 1e-15 in that share), and 2e-10 or better for the others.
pub const NU_TOL: f64 = 1e-9;
/// Halvings of the upper end of a group's bracket where the scaling fails there.
pub const MAX_HALVINGS: usize = 10;
/// Rounds of Brent's method before the fit gives up.
pub const MAX_ROUNDS: usize = 200;

/// The goods of each theta class, `theta_class` given per traded good; classes without goods
/// are left out.
pub fn class_groups(theta_class: &[usize]) -> Vec<Vec<usize>> {
    (0..N_CLASSES)
        .map(|c| {
            (0..theta_class.len())
                .filter(|&g| theta_class[g] == c)
                .collect()
        })
        .filter(|goods: &Vec<usize>| !goods.is_empty())
        .collect()
}

/// The friction groups (ADR 0276): [`class_groups`] with class [`PER_GOOD_CLASS`] split into one
/// group per good.
pub fn friction_groups(theta_class: &[usize]) -> Vec<Vec<usize>> {
    class_groups(theta_class)
        .into_iter()
        .flat_map(|goods| {
            if theta_class[goods[0]] == PER_GOOD_CLASS {
                goods.into_iter().map(|g| vec![g]).collect()
            } else {
                vec![goods]
            }
        })
        .collect()
}

/// The P05 of each friction group and the moments at it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    /// The goods of each group.
    pub groups: Vec<Vec<usize>>,
    pub nu: Vec<f64>,
    pub target: Vec<f64>,
    /// The model's share at `nu`, stage 1 at the data state.
    pub share: Vec<f64>,
    /// Evaluations, each one scaling of every good: the bracket's ends, the rounds and the
    /// share at the result.
    pub evaluations: usize,
    /// Groups whose target is above their share at nu 0, held at nu = 0 (ADR 0092, Amendment
    /// 2026-10-30); `share` then falls short of `target`.
    pub clamped: Vec<bool>,
}

impl Fit {
    /// The border friction of every good: its group's nu, `own` for a good in no group.
    pub fn nu_of_goods(&self, own: &[f64]) -> Vec<f64> {
        nu_of_goods(&self.groups, &self.nu, own)
    }
}

/// The border friction of every good, `nu` per group of `groups`, `own` for a good in no group.
pub fn nu_of_goods(groups: &[Vec<usize>], nu: &[f64], own: &[f64]) -> Vec<f64> {
    let mut out = own.to_vec();
    for (goods, &v) in groups.iter().zip(nu) {
        for &g in goods {
            out[g] = v;
        }
    }
    out
}

/// The inputs with every crossing at the border friction `nu` of its good (one per good) in place
/// of the inputs' own (`Inputs::border_nu`): K scaled by ((1 + nu) / (1 + nu_0))^-theta, the share
/// of units arriving by (1 + nu_0) / (1 + nu), and the base buyer prices (sourcing at prices 1)
/// and their selection constants Gamma (ADR 0200) recomputed; the inputs as they are at their
/// own nu.
pub fn at_border_nu(inp: &Inputs, nu: &[f64]) -> Result<Inputs, String> {
    let n_g = inp.n_goods();
    if inp.border_nu.len() != n_g || nu.len() != n_g {
        return Err(format!(
            "{} border frictions in the inputs and {} given, not one per good of {n_g}",
            inp.border_nu.len(),
            nu.len()
        ));
    }
    if nu.iter().any(|v| !(v.is_finite() && *v >= 0.0)) {
        return Err(format!(
            "border friction must be finite and nonnegative: {nu:?}"
        ));
    }
    let mut out = inp.clone();
    if nu == inp.border_nu.as_slice() {
        return Ok(out);
    }
    let cro = &mut out.crossing;
    for j in 0..cro.good.len() {
        let g = cro.good[j];
        let ratio = (1.0 + nu[g]) / (1.0 + inp.border_nu[g]);
        cro.k[j] *= ratio.powf(-inp.theta[g]);
        cro.inv_loss[j] /= ratio;
    }
    out.border_nu = nu.to_vec();
    let ones = Mat::filled(out.n_provinces(), out.n_goods(), 1.0);
    let src = Model::physical(&out)?.sourcing(&ones);
    let mut gamma = Mat::filled(out.n_provinces(), out.n_goods(), 1.0);
    for p in 0..out.n_provinces() {
        for g in (0..out.n_goods()).filter(|&g| out.traded[g]) {
            let (q, ces) = (src.index[(p, g)], src.ces[(p, g)]);
            out.base_price[(p, g)] = q;
            if q.is_finite() && ces.is_finite() {
                gamma[(p, g)] = q / ces;
            }
        }
    }
    out.gamma = Some(gamma);
    Ok(out)
}

/// The group of every good of `n_goods`, if any.
fn group_of(groups: &[Vec<usize>], n_goods: usize) -> Result<Vec<Option<usize>>, String> {
    let mut of = vec![None; n_goods];
    for (k, goods) in groups.iter().enumerate() {
        for &g in goods {
            if g >= n_goods || of[g].is_some() {
                return Err(format!("good {g} is not a good, or in two friction groups"));
            }
            of[g] = Some(k);
        }
    }
    Ok(of)
}

fn ratio(abroad: &[f64], total: &[f64]) -> Vec<f64> {
    abroad
        .iter()
        .zip(total)
        .map(|(&a, &t)| if t > 0.0 { a / t } else { f64::NAN })
        .collect()
}

/// The cross-border share of each group of `groups` over the countries flagged `icio`: the value
/// the destinations of those countries buy from other countries over all they buy, at origin
/// prices (net of tariffs), for spending `spend` (P, G) at posted prices `price` (P, G). A group
/// without purchases in those countries has NaN.
pub fn cross_border_shares(
    inp: &Inputs,
    price: &Mat,
    spend: &Mat,
    icio: &[bool],
    groups: &[Vec<usize>],
) -> Result<Vec<f64>, String> {
    let m = Model::new(inp)?;
    let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
    if icio.len() != n_c {
        return Err(format!("{} ICIO flags for {n_c} countries", icio.len()));
    }
    let of = group_of(groups, n_g)?;
    let src = m.sourcing(price);
    let flows = m.flows(price, spend, &src);
    let (mut abroad, mut total) = (vec![0.0; groups.len()], vec![0.0; groups.len()]);
    for g in (0..n_g).filter(|&g| inp.traded[g]) {
        let Some(k) = of[g] else { continue };
        for e in 0..n_c {
            for i in (0..n_c).filter(|&i| icio[i]) {
                let v = flows.pair_value[(e * n_c + i) * n_g + g];
                total[k] += v;
                if e != i {
                    abroad[k] += v;
                }
            }
        }
        // what each destination buys from its own province, which the pairs leave out
        for p in (0..n_p).filter(|&p| icio[inp.province_country[p]]) {
            let i = g * n_p + p;
            if src.phi[i] > 0.0 {
                total[k] += src.w[i] * spend[(p, g)] / src.phi[i];
            }
        }
    }
    Ok(ratio(&abroad, &total))
}

/// ICIO 2000 cross-border share of each group of `groups` (`econ_home_shares.arrow` of `pack`):
/// absorption weighted over the ICIO economies. Also returns the ICIO flag of every pack country.
pub fn target_shares(pack: &Path, groups: &[Vec<usize>]) -> Result<(Vec<f64>, Vec<bool>), String> {
    let t = Table::open(&pack.join("econ_home_shares.arrow"))?;
    let (country, good) = (t.usize("country")?, t.usize("good")?);
    let (share, absorption, icio) = (t.f64("home_share")?, t.f64("absorption")?, t.bool("icio")?);
    let n_c = country.iter().max().map_or(0, |c| c + 1);
    let n_g = good.iter().max().map_or(0, |g| g + 1);
    let of = group_of(
        groups,
        n_g.max(groups.iter().flatten().max().map_or(0, |g| g + 1)),
    )?;
    let mut flag = vec![false; n_c];
    let (mut abroad, mut total) = (vec![0.0; groups.len()], vec![0.0; groups.len()]);
    for r in 0..country.len() {
        flag[country[r]] = icio[r];
        // an ICIO economy without use of the good has a NaN share and no weight
        if let Some(k) = of[good[r]]
            && icio[r]
            && absorption[r] > 0.0
        {
            total[k] += absorption[r];
            abroad[k] += (1.0 - share[r]) * absorption[r];
        }
    }
    Ok((ratio(&abroad, &total), flag))
}

/// Why an evaluation of the shares failed: the scaling of stage 2 (it did not converge or left
/// f64), which a smaller nu may cure, or anything else.
enum Failure {
    Scaling(CalibrationError),
    Other(CalibrationError),
}

impl From<Failure> for CalibrationError {
    fn from(f: Failure) -> Self {
        match f {
            Failure::Scaling(e) | Failure::Other(e) => e,
        }
    }
}

/// Shares of every group at border frictions `nu` (per good), stage 2 at `spend` and `supply`
/// (P, G), the scaling started from `start`; and the scaling's log weights.
#[allow(clippy::too_many_arguments)]
fn shares_at(
    inp: &Inputs,
    nu: &[f64],
    groups: &[Vec<usize>],
    spend: &Mat,
    supply: &Mat,
    icio: &[bool],
    start: Option<&Mat>,
    limits: Limits,
) -> Result<(Vec<f64>, Mat), Failure> {
    let other = |e: String| Failure::Other(e.into());
    let at = at_border_nu(inp, nu).map_err(other)?;
    let m = Model::new(&at).map_err(other)?;
    let scaling = steady::sinkhorn_limited(&m, spend, supply, start, limits.max_sweeps)
        .map_err(Failure::Scaling)?;
    let (n_p, n_g) = (at.n_provinces(), at.n_goods());
    let lw = &scaling.log_weight;
    let mut top = vec![f64::NEG_INFINITY; n_g];
    for p in 0..n_p {
        for (g, t) in top.iter_mut().enumerate() {
            if at.traded[g] && at.active[p][g] {
                *t = t.max(lw[(p, g)]);
            }
        }
    }
    let price = Mat::from_fn(n_p, n_g, |p, g| {
        if at.traded[g] && at.active[p][g] {
            (-(lw[(p, g)] - top[g]) / at.theta[g]).exp()
        } else {
            1.0
        }
    });
    let share = cross_border_shares(&at, &price, spend, icio, groups).map_err(other)?;
    Ok((share, scaling.log_weight))
}

/// The cross-border share of every group at border frictions `nu` (per good), over the countries
/// flagged `icio`: stage 2 at the stage 1 spending of the data state of `inp`, as the fit
/// evaluates it.
pub fn data_state_shares(
    inp: &Inputs,
    nu: &[f64],
    groups: &[Vec<usize>],
    icio: &[bool],
    limits: Limits,
) -> Result<Vec<f64>, CalibrationError> {
    let (spend, supply) = steady::data_state_spending(inp, limits)?;
    Ok(shares_at(inp, nu, groups, &spend, &supply, icio, None, limits)?.0)
}
/// Brent's method (Brent 1973, `zeroin`) on one bracket, driven from outside: [`Brent::next`]
/// names the point to evaluate, [`Brent::update`] takes f there, so several brackets can share
/// one evaluation.
#[derive(Clone, Debug)]
struct Brent {
    a: f64,
    b: f64,
    c: f64,
    fa: f64,
    fb: f64,
    fc: f64,
    d: f64,
    e: f64,
}

impl Brent {
    /// A bracket [a, b] with f(a) and f(b) of opposite signs (or one of them zero).
    fn new(a: f64, fa: f64, b: f64, fb: f64) -> Brent {
        Brent {
            a,
            b,
            c: a,
            fa,
            fb,
            fc: fa,
            d: b - a,
            e: b - a,
        }
    }

    /// The point to evaluate next, or None once the root is within `tol` (half-width about
    /// `tol` / 2 plus rounding).
    fn next(&mut self, tol: f64) -> Option<f64> {
        if self.fb * self.fc > 0.0 {
            (self.c, self.fc) = (self.a, self.fa);
            self.d = self.b - self.a;
            self.e = self.d;
        }
        if self.fc.abs() < self.fb.abs() {
            (self.a, self.fa) = (self.b, self.fb);
            (self.b, self.fb) = (self.c, self.fc);
            (self.c, self.fc) = (self.a, self.fa);
        }
        let tol1 = 2.0 * f64::EPSILON * self.b.abs() + 0.5 * tol;
        let xm = 0.5 * (self.c - self.b);
        if xm.abs() <= tol1 || self.fb == 0.0 {
            return None;
        }
        if self.e.abs() >= tol1 && self.fa.abs() > self.fb.abs() {
            let s = self.fb / self.fa;
            let (mut p, mut q) = if self.a == self.c {
                (2.0 * xm * s, 1.0 - s)
            } else {
                let (q, r) = (self.fa / self.fc, self.fb / self.fc);
                (
                    s * (2.0 * xm * q * (q - r) - (self.b - self.a) * (r - 1.0)),
                    (q - 1.0) * (r - 1.0) * (s - 1.0),
                )
            };
            if p > 0.0 {
                q = -q;
            }
            p = p.abs();
            if 2.0 * p < (3.0 * xm * q - (tol1 * q).abs()).min((self.e * q).abs()) {
                self.e = self.d;
                self.d = p / q;
            } else {
                self.d = xm;
                self.e = xm;
            }
        } else {
            self.d = xm;
            self.e = xm;
        }
        (self.a, self.fa) = (self.b, self.fb);
        self.b += if self.d.abs() > tol1 {
            self.d
        } else {
            tol1.copysign(xm)
        };
        Some(self.b)
    }

    fn update(&mut self, fb: f64) {
        self.fb = fb;
    }

    fn root(&self) -> f64 {
        self.b
    }
}

/// nu per group of `groups` such that the model's cross-border share of the group equals
/// `target`, over the countries flagged `icio`, at the stage 1 spending of the data state of
/// `inp`. Each group's share falls as its nu rises. A target above the share at nu 0 clamps the
/// group at nu = 0, the corner of nu >= 0 (`Fit::clamped`; ADR 0092, Amendment 2026-10-30); a
/// target below the share at the bracket's upper end ([`NU_MAX`], lower where the scaling fails
/// there) is an error naming the group. A group with a NaN target (no ICIO use) keeps the
/// inputs' nu of its first good. Brent's method per group, its bracket to [`NU_TOL`] in nu (the
/// root's resolution is coarser where a group's share is flat, see [`NU_TOL`]); the groups share
/// each evaluation, and every scaling starts from the scaling at nu 0, so the fit does not depend
/// on the order of its evaluations.
pub fn fit_border_nu(
    inp: &Inputs,
    groups: &[Vec<usize>],
    target: &[f64],
    icio: &[bool],
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<Fit, CalibrationError> {
    let n = groups.len();
    if target.len() != n || inp.border_nu.len() != inp.n_goods() {
        return Err(format!(
            "{} targets for {n} friction groups, {} border frictions for {} goods",
            target.len(),
            inp.border_nu.len(),
            inp.n_goods()
        )
        .into());
    }
    group_of(groups, inp.n_goods())?;
    let (spend, supply) = steady::data_state_spending(inp, limits)?;
    let own: Vec<f64> = groups.iter().map(|goods| inp.border_nu[goods[0]]).collect();
    let mut fitted: Vec<usize> = (0..n).filter(|&k| !target[k].is_nan()).collect();
    let mut evaluations = 0;
    let target_of = fitted.clone();
    let at = |nu: &[f64]| -> Vec<f64> {
        let per_group: Vec<f64> = (0..n)
            .map(|k| {
                if target_of.contains(&k) {
                    nu[k]
                } else {
                    own[k]
                }
            })
            .collect();
        nu_of_goods(groups, &per_group, &inp.border_nu)
    };
    // Every scaling starts from that at nu 0, so an evaluation depends on nu alone, not on the
    // path of the search.
    evaluations += 1;
    let (at_zero, start) = shares_at(
        inp,
        &at(&vec![0.0; n]),
        groups,
        &spend,
        &supply,
        icio,
        None,
        limits,
    )?;
    progress(&format!("border friction: shares at nu 0 {at_zero:?}"));
    let mut shares = |nu: &[f64]| {
        evaluations += 1;
        shares_at(
            inp,
            &at(nu),
            groups,
            &spend,
            &supply,
            icio,
            Some(&start),
            limits,
        )
        .map(|s| s.0)
    };
    let name = |k: usize| format!("group {k} (goods {:?})", groups[k]);
    // f = share - target, falling in nu
    let mut hi = vec![NU_MAX; n];
    let mut brent: Vec<Option<Brent>> = vec![None; n];
    // The upper end, one group at a time: where the scaling fails there (its log-domain weights
    // leave the range of f64 when a group's imports are cut that far), the end comes down by
    // halves until it solves.
    if let Some(&k) = fitted.iter().find(|&&k| at_zero[k].is_nan()) {
        return Err(format!("{}: no cross-border share at nu 0", name(k)).into());
    }
    // a target above the share at nu 0, the largest, has no root: the group stays at the corner
    let clamped: Vec<bool> = (0..n)
        .map(|k| target_of.contains(&k) && at_zero[k] < target[k])
        .collect();
    for k in (0..n).filter(|&k| clamped[k]) {
        progress(&format!(
            "border friction: {} clamped at nu 0, target {} above the model's {}",
            name(k),
            target[k],
            at_zero[k]
        ));
    }
    fitted.retain(|&k| !clamped[k]);
    for &k in &fitted {
        let mut halvings = 0;
        let f_hi = loop {
            let mut nu = vec![0.0; n];
            nu[k] = hi[k];
            match shares(&nu) {
                Ok(share) => {
                    progress(&format!(
                        "border friction: {} share {} at nu {}",
                        name(k),
                        share[k],
                        hi[k]
                    ));
                    break share[k] - target[k];
                }
                Err(Failure::Scaling(e)) if halvings < MAX_HALVINGS => {
                    progress(&format!(
                        "{}: no scaling at nu {} ({e}); halving",
                        name(k),
                        hi[k]
                    ));
                    hi[k] *= 0.5;
                    halvings += 1;
                }
                Err(Failure::Scaling(e) | Failure::Other(e)) => {
                    return Err(format!("{}: {e}", name(k)).into());
                }
            }
        };
        let f_lo = at_zero[k] - target[k];
        if !(f_lo >= 0.0 && f_hi <= 0.0) {
            return Err(format!(
                "{}: target cross-border share {} outside the model's {} (nu 0) to {} (nu {})",
                name(k),
                target[k],
                at_zero[k],
                f_hi + target[k],
                hi[k]
            )
            .into());
        }
        brent[k] = Some(Brent::new(0.0, f_lo, hi[k], f_hi));
    }
    let mut nu = own.clone();
    for k in (0..n).filter(|&k| clamped[k]) {
        nu[k] = 0.0;
    }
    for round in 1..=MAX_ROUNDS {
        let mut open = Vec::new();
        for &k in &fitted {
            let b = brent[k].as_mut().expect("a bracket per fitted group");
            match b.next(NU_TOL) {
                Some(x) => {
                    nu[k] = x;
                    open.push(k);
                }
                None => nu[k] = b.root(),
            }
        }
        if open.is_empty() {
            let share = shares(&nu)?;
            return Ok(Fit {
                groups: groups.to_vec(),
                nu,
                target: target.to_vec(),
                share,
                evaluations,
                clamped,
            });
        }
        let share = shares(&nu)?;
        for &k in &open {
            brent[k]
                .as_mut()
                .expect("a bracket per fitted group")
                .update(share[k] - target[k]);
        }
        progress(&format!("border round {round}: nu {nu:?}, share {share:?}"));
    }
    Err(format!("the border friction fit did not converge in {MAX_ROUNDS} rounds").into())
}
