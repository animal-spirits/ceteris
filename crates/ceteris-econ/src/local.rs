//! The same-tick chain of local goods (ADR 0020, Mapping to P2 as built; `specs/econ/SPEC.md`,
//! Production). Utilities, construction and services have no transit, so their production,
//! input use and rationing run as one pass inside E2: a shortage of a local good cuts its
//! users' output in the same tick. Traded inputs keep their stocks and transit lags.

use std::sync::Arc;

use ceteris_trade::N_GOODS;

use crate::world::EconWorld;

/// Iteration bound of the chain.
pub const N_LOCAL: usize = 32;
/// Local goods, in input-output order.
pub const LOCAL: [usize; 3] = [7, 8, 9];
/// Relative change of every output at which the chain has converged.
const CONVERGED: f64 = 1e-12;
/// Shortfall of a good, relative to the needs its users' stocks cannot cover, that the last
/// step of the chain leaves as rounding.
const ROUNDING: f64 = 1e-13;

/// Common fill of the users of one local good, with `rest` units of output and users with
/// needs n and stocks b: the largest x <= 1 at which the needs fit the output and the stocks,
/// x sum n <= rest + sum min(b, x n). It is the limit of applying
/// f = min(1, (rest + sum min(b, n)) / sum n) to every user until it stops cutting, found
/// exactly: on the users whose stock binds (b < x n) the condition is linear, so x moves to
/// (rest + sum b) / sum n over them, and that set only shrinks.
fn fill(rest: f64, users: &[(f64, f64)]) -> f64 {
    let need: f64 = users.iter().map(|u| u.0).sum();
    if need <= rest {
        return 1.0;
    }
    let mut x: f64 = 1.0;
    for _ in 0..=users.len() {
        let (mut n, mut b) = (0.0, 0.0);
        for &(ne, be) in users.iter().filter(|&&(ne, be)| be < x * ne) {
            n += ne;
            b += be;
        }
        if n == 0.0 {
            break;
        }
        let next = ((rest + b) / n).max(0.0);
        if next >= x {
            break;
        }
        x = next;
    }
    x
}

/// Splits `rest` units of output among users with needs n and stocks b, writing each user's
/// draw from output into `out`: output first, so with enough output every need is met from
/// it; short, each user gets what its stock cannot cover, max(n - b, 0), and the rest of the
/// output pro rata to min(b, n), its stock covering the remainder. Where even the uncovered
/// needs exceed the output (only before the chain converges), they share it pro rata.
fn split(rest: f64, users: &[(f64, f64)], out: &mut [f64]) {
    let need: f64 = users.iter().map(|u| u.0).sum();
    if need <= rest {
        for (o, u) in out.iter_mut().zip(users) {
            *o = u.0;
        }
        return;
    }
    let uncovered: f64 = users.iter().map(|&(n, b)| (n - b).max(0.0)).sum();
    if uncovered >= rest {
        // uncovered = 0 here means rest = 0: stocks cover every need and nothing is drawn
        let share = if uncovered > 0.0 {
            rest / uncovered
        } else {
            0.0
        };
        for (o, &(n, b)) in out.iter_mut().zip(users) {
            *o = (n - b).max(0.0) * share;
        }
        return;
    }
    let usable: f64 = users.iter().map(|&(n, b)| b.min(n)).sum();
    let extra = (rest - uncovered) / usable;
    for (o, &(n, b)) in out.iter_mut().zip(users) {
        *o = (n - b).max(0.0) + extra * b.min(n);
    }
}

/// A province's enterprises as the chain sees them, by position in the province.
struct Province {
    /// Output capped by capacity and traded input stocks.
    cap: Vec<f64>,
    /// Coefficients of the local goods; 0 for an enterprise's own good, which its own
    /// output serves first.
    coef: Vec<[f64; 3]>,
    stock: Vec<[f64; 3]>,
    producer: [Option<usize>; 3],
    /// The producer's own coefficient of each local good.
    own: [f64; 3],
}

impl Province {
    /// Output left for the users of good j, its users as (need, stock), at outputs y.
    fn users(&self, j: usize, y: &[f64], out: &mut Vec<(f64, f64)>) -> f64 {
        out.clear();
        for (u, &yu) in y.iter().enumerate() {
            let need = self.coef[u][j] * yu;
            if need > 0.0 {
                out.push((need, self.stock[u][j]));
            }
        }
        self.producer[j].map_or(0.0, |p| (1.0 - self.own[j]) * y[p])
    }

    /// The output left for the users of good j at outputs y and what their stocks cannot
    /// cover, sum max(n - b, 0): the good fits while the first is at least the second.
    fn uncovered(&self, j: usize, y: &[f64], users: &mut Vec<(f64, f64)>) -> (f64, f64) {
        let rest = self.users(j, y, users);
        (rest, users.iter().map(|&(n, b)| (n - b).max(0.0)).sum())
    }
}

/// Runs the chain for the enterprises of province `i`. On entry `Scratch::produced` holds
/// each enterprise's output capped by capacity and traded input stocks; on return it holds
/// the output, and `Scratch::from_output` the units of each local good the enterprise draws
/// from this tick's output (the rest of its use comes from its stock, the buffer). Returns
/// the iterations run.
///
/// Each iteration, from outputs Y and for each local good k: its producer's own use,
/// coef_kk Y_k, comes out of its output first; the other users, with needs n_ek = coef_ek
/// Y_e and stocks b_ek, share the rest R_k = (1 - coef_kk) Y_k and their stocks at the common
/// fill f_k of [`fill`], the fixed point of f_k = min(1, (R_k + sum_e min(b_ek, n_ek)) /
/// sum_e n_ek), and each enterprise runs at the smallest fill of its local inputs:
/// Y'_e = Y_e min_k f_k. Y never rises; the chain stops when no output moves by more than
/// 1e-12 of itself, or after [`N_LOCAL`] iterations, with the last fills applied. A cycle
/// among local goods converges geometrically and may leave a shortage at the bound: it is
/// then cleared, good by good in order 7, 8, 9, by cutting the users that produce no local
/// good by the fill of what their producers leave, which changes no local output; where the
/// producers alone do not fit, the users of the good that their stock cannot cover stop, and
/// so on, so every good fits exactly. The draws are then split at the final outputs
/// ([`split`]).
pub fn chain(w: &mut EconWorld, i: usize) -> usize {
    let (inp, lay) = (Arc::clone(&w.inputs), Arc::clone(&w.layout));
    let ents = lay.enterprises(i);
    let first = ents.start;
    let good = |e: usize| usize::from(inp.enterprises.good[e]);
    let producer = LOCAL.map(|k| ents.clone().find(|&e| good(e) == k).map(|e| e - first));
    let province = Province {
        cap: w.scratch.produced[ents.clone()].to_vec(),
        coef: ents
            .clone()
            .map(|e| {
                let mut c = LOCAL.map(|k| lay.coef[e][k]);
                if let Some(j) = LOCAL.iter().position(|&k| k == good(e)) {
                    c[j] = 0.0;
                }
                c
            })
            .collect(),
        stock: ents
            .clone()
            .map(|e| LOCAL.map(|k| w.ledger.goods(lay.accounts.enterprise[e], k)))
            .collect(),
        producer,
        own: std::array::from_fn(|j| producer[j].map_or(0.0, |p| lay.coef[p + first][LOCAL[j]])),
    };

    let mut users = Vec::new();
    let mut y = province.cap.clone();
    let mut cut = vec![1.0f64; y.len()];
    let mut iterations = 0;
    loop {
        iterations += 1;
        cut.fill(1.0);
        for j in 0..3 {
            let rest = province.users(j, &y, &mut users);
            let f = fill(rest, &users);
            for (u, c) in cut.iter_mut().enumerate() {
                if province.coef[u][j] * y[u] > 0.0 {
                    *c = c.min(f);
                }
            }
        }
        let mut converged = true;
        for (yu, &c) in y.iter_mut().zip(&cut) {
            let next = *yu * c;
            converged &= (*yu - next).abs() <= CONVERGED * *yu;
            *yu = next;
        }
        if converged || iterations == N_LOCAL {
            break;
        }
    }
    // what is left short: cut the users that produce no local good, which leaves every local
    // output as it is; where the producers alone do not fit, stop the users of the good, and
    // so on down the chain (each round stops at least one more enterprise)
    let makes_local = |u: usize| province.producer.contains(&Some(u));
    let mut others = Vec::new();
    for j in 0..3 {
        let (mut rest, uncovered) = province.uncovered(j, &y, &mut users);
        if uncovered <= rest {
            continue;
        }
        others.clear();
        for (u, &yu) in y.iter().enumerate() {
            let (need, stock) = (province.coef[u][j] * yu, province.stock[u][j]);
            if need > 0.0 && makes_local(u) {
                rest -= (need - stock).max(0.0);
            } else if need > 0.0 {
                others.push((need, stock));
            }
        }
        let z = fill(rest, &others);
        for (u, yu) in y.iter_mut().enumerate() {
            if province.coef[u][j] > 0.0 && !makes_local(u) {
                *yu *= z;
            }
        }
    }
    for _ in 0..=y.len() {
        let short = |j: usize, users: &mut Vec<(f64, f64)>| {
            let (rest, uncovered) = province.uncovered(j, &y, users);
            uncovered * (1.0 - ROUNDING) > rest
        };
        let Some(j) = (0..3).find(|&j| short(j, &mut users)) else {
            break;
        };
        for (u, yu) in y.iter_mut().enumerate() {
            if province.coef[u][j] * *yu > province.stock[u][j] {
                *yu = 0.0;
            }
        }
    }

    let sc = &mut w.scratch;
    let mut draws = Vec::new();
    for (j, &k) in LOCAL.iter().enumerate() {
        let rest = province.users(j, &y, &mut users);
        draws.resize(users.len(), 0.0);
        split(rest, &users, &mut draws);
        let mut d = draws.iter();
        for (u, &yu) in y.iter().enumerate() {
            let at = (first + u) * N_GOODS + k;
            sc.from_output[at] = if Some(u) == producer[j] {
                province.own[j] * yu
            } else if province.coef[u][j] * yu > 0.0 {
                *d.next().expect("one draw per user")
            } else {
                0.0
            };
        }
    }
    for (u, &yu) in y.iter().enumerate() {
        sc.produced[first + u] = yu;
    }
    iterations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_is_the_limit_of_the_cut() {
        // enough output: no cut
        assert_eq!(fill(12.0, &[(6.0, 0.0), (6.0, 0.0)]), 1.0);
        // no stocks: output over need
        assert!((fill(10.0, &[(6.0, 0.0), (6.0, 0.0)]) - 10.0 / 12.0).abs() < 1e-15);
        // one user covers itself from stock: x 12 <= 4 + min(6, 6 x) gives x = 2 / 3
        let users = [(6.0, 6.0), (6.0, 0.0)];
        let x = fill(4.0, &users);
        assert!((x - 2.0 / 3.0).abs() < 1e-15);
        // the brief's f = min(1, (rest + sum min(b, n)) / sum n) repeated converges there
        let mut f = 1.0;
        for _ in 0..200 {
            let need: f64 = users.iter().map(|u| f * u.0).sum();
            let usable: f64 = users.iter().map(|&(n, b)| b.min(f * n)).sum();
            f *= ((4.0 + usable) / need).min(1.0);
        }
        assert!((f - x).abs() < 1e-12);
        // more stock than needed counts only up to the need; nothing left over is nil
        assert_eq!(fill(0.0, &[(2.0, 5.0)]), 1.0);
        assert_eq!(fill(0.0, &[(2.0, 0.0)]), 0.0);
    }

    #[test]
    fn split_draws_output_first_and_stock_for_the_rest() {
        let mut out = [0.0; 2];
        split(20.0, &[(6.0, 3.0), (4.0, 0.0)], &mut out);
        assert_eq!(out, [6.0, 4.0]);
        // short: the user without stock gets its whole need, the other the rest of output
        split(5.0, &[(4.0, 4.0), (4.0, 0.0)], &mut out);
        assert_eq!(out, [1.0, 4.0]);
        // stocks cover the residues exactly
        split(4.0, &[(4.0, 6.0), (4.0, 0.0)], &mut out);
        assert_eq!(out, [0.0, 4.0]);
        // no output and stocks covering every need: nothing drawn, not 0 * (0 / 0)
        split(0.0, &[(4.0, 6.0), (2.0, 2.0)], &mut out);
        assert_eq!(out, [0.0, 0.0]);
    }
}
