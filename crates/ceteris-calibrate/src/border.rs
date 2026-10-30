//! The border friction per theta class (P05, ADR 0092; `specs/econ/SPEC.md`, Calibration): nu_c
//! solves, given theta_c, the cross-border share of the class's goods in ICIO 2000.
//!
//! The moment of class c is the share of the ICIO economies' use of the class's goods that comes
//! from abroad, sum over those economies and goods of (1 - home share) absorption over sum of
//! absorption: from `econ_home_shares.arrow` for the target ([`target_shares`]), and from the
//! value the model's destinations buy, at origin prices, for the model ([`cross_border_shares`]).
//! Each evaluation is stage 2 of ADR 0089 (the scaling that makes every origin sell its output)
//! at stage 1's spending of the data state, held for every trial: a
//! class's share then depends on its own nu only, and the four classes are four 1-D roots,
//! found together by Brent's method in nu.

use std::path::Path;

use ceteris_reference::io::{Inputs, Mat};
use ceteris_reference::model::Model;

use crate::steady::{self, CalibrationError, Limits};
use crate::write::Table;

pub use ceteris_trade::N_CLASSES;
/// The bracket of nu searched: no friction, and a friction ten times the good's value, which cuts
/// the frictionless kernel to 9e-4 at theta 2.91 and 2e-18 at 16.91.
pub const NU_MAX: f64 = 10.0;
/// Brent ends when every class's bracket is this narrow in nu on the computed shares. The root
/// itself is known only to the scaling's noise in the share over the share's slope in nu: on the
/// fixture about 7e-9 for class 3, whose share moves by 2.7e-7 per unit nu (the scaling stops
/// at 1e-13, which leaves about 1e-15 in that share), and 2e-10 or better for the others.
pub const NU_TOL: f64 = 1e-9;
/// Halvings of the upper end of a class's bracket where the scaling fails there.
pub const MAX_HALVINGS: usize = 10;
/// Rounds of Brent's method before the fit gives up.
pub const MAX_ROUNDS: usize = 200;

/// The P05 of each class and the moments at it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    pub nu: [f64; N_CLASSES],
    pub target: [f64; N_CLASSES],
    /// The model's share at `nu`, stage 1 at the data state.
    pub share: [f64; N_CLASSES],
    /// Evaluations, each one scaling of every good: the bracket's ends, the rounds and the
    /// share at the result.
    pub evaluations: usize,
    /// Classes whose target is above their share at nu 0, held at nu = 0 (ADR 0092, Amendment
    /// 2026-10-30); `share` then falls short of `target`.
    pub clamped: [bool; N_CLASSES],
}

/// The inputs with every crossing at the border friction `nu` of its good's class in place of
/// the inputs' own (`Inputs::border_nu`): K scaled by ((1 + nu) / (1 + nu_0))^-theta, the share of
/// units arriving by (1 + nu_0) / (1 + nu), and the base buyer prices (sourcing at prices 1)
/// recomputed; the inputs as they are at their own nu.
pub fn at_border_nu(inp: &Inputs, nu: &[f64; N_CLASSES]) -> Result<Inputs, String> {
    if inp.border_nu.len() != N_CLASSES {
        return Err(format!(
            "the inputs have {} border frictions, not {N_CLASSES}",
            inp.border_nu.len()
        ));
    }
    if nu.iter().any(|v| !(v.is_finite() && *v >= 0.0)) {
        return Err(format!(
            "border friction must be finite and nonnegative: {nu:?}"
        ));
    }
    let mut out = inp.clone();
    if nu.as_slice() == inp.border_nu.as_slice() {
        return Ok(out);
    }
    let cro = &mut out.crossing;
    for j in 0..cro.good.len() {
        let g = cro.good[j];
        let c = inp.theta_class[g];
        let ratio = (1.0 + nu[c]) / (1.0 + inp.border_nu[c]);
        cro.k[j] *= ratio.powf(-inp.theta[g]);
        cro.inv_loss[j] /= ratio;
    }
    out.border_nu = nu.to_vec();
    let ones = Mat::filled(out.n_provinces(), out.n_goods(), 1.0);
    let index = Model::new(&out)?.sourcing(&ones).index;
    for p in 0..out.n_provinces() {
        for g in (0..out.n_goods()).filter(|&g| out.traded[g]) {
            out.base_price[(p, g)] = index[(p, g)];
        }
    }
    Ok(out)
}

/// The cross-border share of each class over the countries flagged `icio`: the value the
/// destinations of those countries buy from other countries over all they buy, at origin prices
/// (net of tariffs), for spending `spend` (P, G) at posted prices `price` (P, G). A class without
/// purchases in those countries has NaN.
pub fn cross_border_shares(
    inp: &Inputs,
    price: &Mat,
    spend: &Mat,
    icio: &[bool],
) -> Result<[f64; N_CLASSES], String> {
    let m = Model::new(inp)?;
    let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
    if icio.len() != n_c {
        return Err(format!("{} ICIO flags for {n_c} countries", icio.len()));
    }
    let src = m.sourcing(price);
    let flows = m.flows(price, spend, &src);
    let (mut abroad, mut total) = ([0.0; N_CLASSES], [0.0; N_CLASSES]);
    for g in (0..n_g).filter(|&g| inp.traded[g]) {
        let c = inp.theta_class[g];
        for e in 0..n_c {
            for i in (0..n_c).filter(|&i| icio[i]) {
                let v = flows.pair_value[(e * n_c + i) * n_g + g];
                total[c] += v;
                if e != i {
                    abroad[c] += v;
                }
            }
        }
        // what each destination buys from its own province, which the pairs leave out
        for p in (0..n_p).filter(|&p| icio[inp.province_country[p]]) {
            let i = g * n_p + p;
            if src.phi[i] > 0.0 {
                total[c] += src.w[i] * spend[(p, g)] / src.phi[i];
            }
        }
    }
    Ok(std::array::from_fn(|c| {
        if total[c] > 0.0 {
            abroad[c] / total[c]
        } else {
            f64::NAN
        }
    }))
}

/// ICIO 2000 cross-border share of each class (`econ_home_shares.arrow` of `pack`): absorption
/// weighted over the ICIO economies, for the classes of `theta_class` (per traded good). Also
/// returns the ICIO flag of every pack country.
pub fn target_shares(
    pack: &Path,
    theta_class: &[usize],
) -> Result<([f64; N_CLASSES], Vec<bool>), String> {
    let t = Table::open(&pack.join("econ_home_shares.arrow"))?;
    let (country, good) = (t.usize("country")?, t.usize("good")?);
    let (share, absorption, icio) = (t.f64("home_share")?, t.f64("absorption")?, t.bool("icio")?);
    let n_c = country.iter().max().map_or(0, |c| c + 1);
    let mut flag = vec![false; n_c];
    let (mut abroad, mut total) = ([0.0; N_CLASSES], [0.0; N_CLASSES]);
    for r in 0..country.len() {
        flag[country[r]] = icio[r];
        let g = good[r];
        if g >= theta_class.len() {
            return Err(format!("econ_home_shares.arrow names good {g}"));
        }
        // an ICIO economy without use of the good has a NaN share and no weight
        if icio[r] && absorption[r] > 0.0 {
            let c = theta_class[g];
            total[c] += absorption[r];
            abroad[c] += (1.0 - share[r]) * absorption[r];
        }
    }
    let target = std::array::from_fn(|c| {
        if total[c] > 0.0 {
            abroad[c] / total[c]
        } else {
            f64::NAN
        }
    });
    Ok((target, flag))
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

/// Shares of every class at border frictions `nu`, stage 2 at `spend` and `supply` (P, G), the
/// scaling started from `start`; and the scaling's log weights.
fn shares_at(
    inp: &Inputs,
    nu: &[f64; N_CLASSES],
    spend: &Mat,
    supply: &Mat,
    icio: &[bool],
    start: Option<&Mat>,
    limits: Limits,
) -> Result<([f64; N_CLASSES], Mat), Failure> {
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
    let share = cross_border_shares(&at, &price, spend, icio).map_err(other)?;
    Ok((share, scaling.log_weight))
}

/// The cross-border share of every class at border frictions `nu`, over the countries flagged
/// `icio`: stage 2 at the stage 1 spending of the data state of `inp`, as the fit evaluates it.
pub fn data_state_shares(
    inp: &Inputs,
    nu: &[f64; N_CLASSES],
    icio: &[bool],
    limits: Limits,
) -> Result<[f64; N_CLASSES], CalibrationError> {
    let (spend, supply) = steady::data_state_spending(inp, limits)?;
    Ok(shares_at(inp, nu, &spend, &supply, icio, None, limits)?.0)
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

/// nu per class such that the model's cross-border share of the class equals `target`, over the
/// countries flagged `icio`, at the stage 1 spending of the data state of `inp`. Each class's
/// share falls as its nu rises. A target above the share at nu 0 clamps the class at nu = 0, the
/// corner of nu >= 0 (`Fit::clamped`; ADR 0092, Amendment 2026-10-30); a target below the share at
/// the bracket's upper end ([`NU_MAX`], lower where the scaling fails there) is an error naming
/// the class. A class with a
/// NaN target (no ICIO use) keeps the inputs' nu. Brent's method per class, its bracket to
/// [`NU_TOL`] in nu (the root's resolution is coarser where a class's share is flat, see
/// [`NU_TOL`]); the classes share each evaluation, and every scaling starts from the scaling at
/// nu 0, so the fit does not depend on the order of its evaluations.
pub fn fit_border_nu(
    inp: &Inputs,
    target: &[f64; N_CLASSES],
    icio: &[bool],
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<Fit, CalibrationError> {
    if inp.border_nu.len() != N_CLASSES {
        return Err(format!(
            "the inputs have {} border frictions, not {N_CLASSES}",
            inp.border_nu.len()
        )
        .into());
    }
    let (spend, supply) = steady::data_state_spending(inp, limits)?;
    let own: [f64; N_CLASSES] = std::array::from_fn(|c| inp.border_nu[c]);
    let mut fitted: Vec<usize> = (0..N_CLASSES).filter(|&c| !target[c].is_nan()).collect();
    let mut evaluations = 0;
    let target_of = fitted.clone();
    let at = |nu: &[f64; N_CLASSES]| -> [f64; N_CLASSES] {
        std::array::from_fn(|c| {
            if target_of.contains(&c) {
                nu[c]
            } else {
                own[c]
            }
        })
    };
    // Every scaling starts from that at nu 0, so an evaluation depends on nu alone, not on the
    // path of the search.
    evaluations += 1;
    let (at_zero, start) = shares_at(
        inp,
        &at(&[0.0; N_CLASSES]),
        &spend,
        &supply,
        icio,
        None,
        limits,
    )?;
    progress(&format!("border friction: shares at nu 0 {at_zero:?}"));
    let mut shares = |nu: &[f64; N_CLASSES]| {
        evaluations += 1;
        shares_at(inp, &at(nu), &spend, &supply, icio, Some(&start), limits).map(|s| s.0)
    };
    // f = share - target, falling in nu
    let mut hi = [NU_MAX; N_CLASSES];
    let mut brent: Vec<Option<Brent>> = vec![None; N_CLASSES];
    // The upper end, one class at a time: where the scaling fails there (its log-domain weights
    // leave the range of f64 when a class's imports are cut that far), the end comes down by
    // halves until it solves.
    if let Some(&c) = fitted.iter().find(|&&c| at_zero[c].is_nan()) {
        return Err(format!("class {c}: no cross-border share at nu 0").into());
    }
    // a target above the share at nu 0, the largest, has no root: the class stays at the corner
    let clamped: [bool; N_CLASSES] =
        std::array::from_fn(|c| target_of.contains(&c) && at_zero[c] < target[c]);
    for c in (0..N_CLASSES).filter(|&c| clamped[c]) {
        progress(&format!(
            "border friction: class {c} clamped at nu 0, target {} above the model's {}",
            target[c], at_zero[c]
        ));
    }
    fitted.retain(|&c| !clamped[c]);
    for &c in &fitted {
        let mut halvings = 0;
        let f_hi = loop {
            let mut nu = [0.0; N_CLASSES];
            nu[c] = hi[c];
            match shares(&nu) {
                Ok(share) => {
                    progress(&format!(
                        "border friction: class {c} share {} at nu {}",
                        share[c], hi[c]
                    ));
                    break share[c] - target[c];
                }
                Err(Failure::Scaling(e)) if halvings < MAX_HALVINGS => {
                    progress(&format!(
                        "class {c}: no scaling at nu {} ({e}); halving",
                        hi[c]
                    ));
                    hi[c] *= 0.5;
                    halvings += 1;
                }
                Err(Failure::Scaling(e) | Failure::Other(e)) => {
                    return Err(format!("class {c}: {e}").into());
                }
            }
        };
        let f_lo = at_zero[c] - target[c];
        if !(f_lo >= 0.0 && f_hi <= 0.0) {
            return Err(format!(
                "class {c}: target cross-border share {} outside the model's {} (nu 0) to {} \
                 (nu {})",
                target[c],
                at_zero[c],
                f_hi + target[c],
                hi[c]
            )
            .into());
        }
        brent[c] = Some(Brent::new(0.0, f_lo, hi[c], f_hi));
    }
    let mut nu = own;
    for c in (0..N_CLASSES).filter(|&c| clamped[c]) {
        nu[c] = 0.0;
    }
    for round in 1..=MAX_ROUNDS {
        let mut open = Vec::new();
        for &c in &fitted {
            let b = brent[c].as_mut().expect("a bracket per fitted class");
            match b.next(NU_TOL) {
                Some(x) => {
                    nu[c] = x;
                    open.push(c);
                }
                None => nu[c] = b.root(),
            }
        }
        if open.is_empty() {
            let share = shares(&nu)?;
            return Ok(Fit {
                nu,
                target: *target,
                share,
                evaluations,
                clamped,
            });
        }
        let share = shares(&nu)?;
        for &c in &open {
            brent[c]
                .as_mut()
                .expect("a bracket per fitted class")
                .update(share[c] - target[c]);
        }
        progress(&format!("border round {round}: nu {nu:?}, share {share:?}"));
    }
    Err(format!("the border friction fit did not converge in {MAX_ROUNDS} rounds").into())
}
