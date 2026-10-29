//! The same-tick chain of local goods (ADR 0020, Mapping to P2 as built; ADR 0093, Amendment;
//! `specs/econ/SPEC.md`, Production). Utilities, construction and services have no transit, so
//! their production, input use and rationing run as one pass inside E2: a shortage of a local
//! good cuts its users' output in the same tick. Traded inputs keep their stocks and transit lags.

use rayon::prelude::*;

use ceteris_core::ledger::Ledger;
use ceteris_trade::N_GOODS;

use crate::inputs::EconInputs;
use crate::world::{EconWorld, Layout};

/// Local goods, in input-output order.
pub const LOCAL: [usize; 3] = [7, 8, 9];

/// Splits `rest` units of output among users with needs n and stocks b, writing each user's
/// draw from output into `out`: output first, so with enough output every need is met from
/// it; short, each user gets what its stock cannot cover, max(n - b, 0), and the rest of the
/// output pro rata to min(b, n), its stock covering the remainder. Where even the uncovered
/// needs exceed the output (never at the chain's fills), they share it pro rata.
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
#[derive(Clone, Debug)]
pub struct Province {
    /// Output capped by capacity and traded input stocks.
    pub cap: Vec<f64>,
    /// Coefficients of the local goods; 0 for an enterprise's own good, which its own
    /// output serves first.
    pub coef: Vec<[f64; 3]>,
    pub stock: Vec<[f64; 3]>,
    pub producer: [Option<usize>; 3],
    /// The producer's own coefficient of each local good.
    pub own: [f64; 3],
}

impl Province {
    /// The province's enterprises from the ledger, `cap` their capped outputs.
    pub fn new(inp: &EconInputs, lay: &Layout, ledger: &Ledger, i: usize, cap: &[f64]) -> Self {
        let ents = lay.enterprises(i);
        let first = ents.start;
        let good = |e: usize| usize::from(inp.enterprises.good[e]);
        let producer = LOCAL.map(|k| ents.clone().find(|&e| good(e) == k).map(|e| e - first));
        Province {
            cap: cap.to_vec(),
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
                .map(|e| LOCAL.map(|k| ledger.goods(lay.accounts.enterprise[e], k)))
                .collect(),
            producer,
            own: std::array::from_fn(|j| {
                producer[j].map_or(0.0, |p| lay.coef[p + first][LOCAL[j]])
            }),
        }
    }

    /// Smallest fill of enterprise `u`'s local inputs, 1 if it uses none.
    fn least(&self, u: usize, f: &[f64; 3]) -> f64 {
        (0..3)
            .filter(|&j| self.coef[u][j] > 0.0)
            .fold(1.0, |m, j| m.min(f[j]))
    }

    /// Outputs at fills `f`: Y_e = cap_e min_k F_k over the local goods e uses.
    pub fn outputs(&self, f: &[f64; 3], y: &mut Vec<f64>) {
        y.clear();
        y.extend((0..self.cap.len()).map(|u| self.cap[u] * self.least(u, f)));
    }

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

    /// What good j's users' stocks cannot cover, sum max(n - b, 0), and the output its
    /// producer leaves, at outputs y.
    fn balance(&self, j: usize, y: &[f64], users: &mut Vec<(f64, f64)>) -> (f64, f64) {
        let rest = self.users(j, y, users);
        (users.iter().map(|&(n, b)| (n - b).max(0.0)).sum(), rest)
    }

    /// Whether good j fits at outputs y: the first of [`Province::balance`] is at most the
    /// second.
    fn fits(&self, j: usize, y: &[f64], users: &mut Vec<(f64, f64)>) -> bool {
        let (uncovered, rest) = self.balance(j, y, users);
        uncovered <= rest
    }
}

/// Working buffers of [`fills`].
#[derive(Default)]
struct Work {
    y: Vec<f64>,
    users: Vec<(f64, f64)>,
    kinks: Vec<f64>,
}

/// A fill as an affine function of the fills at the start of a pass: c + sum_k d_k F_k.
#[derive(Clone, Copy, Debug)]
struct Affine {
    c: f64,
    d: [f64; 3],
}

impl Affine {
    fn of(j: usize) -> Self {
        let mut d = [0.0; 3];
        d[j] = 1.0;
        Affine { c: 0.0, d }
    }

    fn at(&self, f: &[f64; 3]) -> f64 {
        self.c + (0..3).map(|k| self.d[k] * f[k]).sum::<f64>()
    }

    /// This function of the fills `inner` gives, each affine in the same fills.
    fn after(&self, inner: &[Affine; 3]) -> Affine {
        let mut row = Affine {
            c: self.c,
            d: [0.0; 3],
        };
        for (dk, ik) in self.d.iter().zip(inner) {
            row.c += dk * ik.c;
            for (r, i) in row.d.iter_mut().zip(&ik.d) {
                *r += dk * i;
            }
        }
        row
    }
}

/// How a user of the good being stepped enters U on the step's piece.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    /// Its need exceeds its stock and its smallest fill is the step's own.
    OnStep,
    /// Its need exceeds its stock and its smallest fill is good k's.
    On(usize),
    /// Its stock covers its need.
    Covered,
}

/// The piece on which a step found its root: the role of each user and the good whose fill
/// bounds the producer's output (None where it uses no local good).
#[derive(Clone, Debug, Default)]
struct Piece {
    roles: Vec<(usize, Role)>,
    producer: Option<usize>,
}

/// The good whose fill is user `u`'s smallest at fills `f` among its local inputs other than
/// `but`, the lowest on a tie; None if it uses none.
fn binding(p: &Province, u: usize, f: &[f64; 3], but: Option<usize>) -> Option<usize> {
    (0..3)
        .filter(|&k| p.coef[u][k] > 0.0 && Some(k) != but)
        .fold(None, |best: Option<usize>, k| match best {
            Some(b) if f[b] <= f[k] => Some(b),
            _ => Some(k),
        })
}

/// Whether the step of good j that sets its fill to `x` from fills `f` (the other goods'
/// current values) lies on `piece`. Comparisons are not strict: on a tie or at a stock
/// threshold both sides give the same U.
fn on_piece(p: &Province, j: usize, piece: &Piece, f: &[f64; 3], x: f64) -> bool {
    let smallest =
        |u: usize, k: usize| (0..3).all(|i| i == j || p.coef[u][i] == 0.0 || f[k] <= f[i]);
    let producer = match (p.producer[j], piece.producer) {
        (Some(q), Some(k)) => smallest(q, k),
        _ => true,
    };
    producer
        && piece.roles.iter().all(|&(u, role)| {
            let (a, b) = (p.coef[u][j] * p.cap[u], p.stock[u][j]);
            let m = p.least(u, &{
                let mut o = *f;
                o[j] = 1.0;
                o
            });
            match role {
                Role::OnStep => m >= x && a * x >= b,
                Role::On(k) => f[k] <= x && smallest(u, k) && a * f[k] >= b,
                Role::Covered => a * x.min(m) <= b,
            }
        })
}

/// One step of the fill iteration for good j: the largest x <= f_j at which j fits, the other
/// fills as they stand. What the users' stocks cannot cover, U(x) = sum_e max(a_e cap_e
/// min(x, m_e) - b_e, 0) with m_e the user's smallest other fill, is continuous,
/// nondecreasing and piecewise linear in x, and the producer's output does not depend on x
/// (its own good is not its input): the root lies on one piece, found from its kinks.
///
/// Returns None where j already fits. Otherwise the piece and the root on it as an affine
/// function of the other fills, x = (R - level) / slope; the affine has NaN coefficients where
/// rounding put the root on a kink.
fn step(p: &Province, j: usize, f: &mut [f64; 3], w: &mut Work) -> Option<(Affine, Piece)> {
    p.outputs(f, &mut w.y);
    if p.fits(j, &w.y, &mut w.users) {
        return None;
    }
    let mut others = *f;
    others[j] = 1.0;
    let users = || (0..p.cap.len()).filter(|&u| p.coef[u][j] > 0.0);
    let uncovered = |x: f64| -> f64 {
        users()
            .map(|u| {
                let m = p.least(u, &others);
                (p.coef[u][j] * p.cap[u] * x.min(m) - p.stock[u][j]).max(0.0)
            })
            .sum()
    };
    w.kinks.clear();
    for u in users() {
        let slope = p.coef[u][j] * p.cap[u];
        w.kinks.push(p.least(u, &others));
        if slope > 0.0 {
            w.kinks.push(p.stock[u][j] / slope);
        }
    }
    w.kinks.retain(|&x| x > 0.0 && x < f[j]);
    w.kinks.sort_by(|a, b| b.total_cmp(a));
    // the piece (lo, hi) on which U crosses R: lo the largest kink with U(lo) <= R
    let rest_now = p.producer[j].map_or(0.0, |q| (1.0 - p.own[j]) * w.y[q]);
    let mut hi = f[j];
    let mut lo = 0.0;
    for &x in &w.kinks {
        if uncovered(x) <= rest_now {
            lo = x;
            break;
        }
        hi = x;
    }
    // R - level on the piece, as an affine function of the other fills
    let mut num = Affine {
        c: 0.0,
        d: [0.0; 3],
    };
    let mut piece = Piece::default();
    if let Some(q) = p.producer[j] {
        let share = (1.0 - p.own[j]) * p.cap[q];
        piece.producer = binding(p, q, f, None);
        match piece.producer {
            Some(k) => num.d[k] += share,
            None => num.c += share,
        }
    }
    let mid = 0.5 * (lo + hi);
    let mut slope = 0.0;
    for u in users() {
        let (a, b) = (p.coef[u][j] * p.cap[u], p.stock[u][j]);
        let m = p.least(u, &others);
        let role = if m > mid && a * mid > b {
            slope += a;
            num.c += b;
            Role::OnStep
        } else if m <= mid && a * m > b {
            let k = binding(p, u, f, Some(j)).expect("a smaller fill than x");
            num.d[k] -= a;
            num.c += b;
            Role::On(k)
        } else {
            Role::Covered
        };
        piece.roles.push((u, role));
    }
    let root = num.at(f) / slope;
    let inside = slope > 0.0 && root >= lo && root <= hi;
    // U is flat on the piece only where rounding put the crossing at its upper kink
    let mut x = if slope > 0.0 { root.clamp(lo, hi) } else { hi };
    // a fill falling geometrically to 0 would otherwise crawl through the subnormals
    if x < f64::MIN_POSITIVE {
        x = 0.0;
    }
    // the closed form rounds: step down until the fit holds as `Province::fits` evaluates it
    loop {
        f[j] = x;
        p.outputs(f, &mut w.y);
        if x == 0.0 || p.fits(j, &w.y, &mut w.users) {
            break;
        }
        x = x.next_down().max(0.0);
    }
    let map = if inside {
        Affine {
            c: num.c / slope,
            d: num.d.map(|d| d / slope),
        }
    } else {
        Affine {
            c: f64::NAN,
            d: [f64::NAN; 3],
        }
    };
    Some((map, piece))
}

/// What a pass did: each good's fill at its end as an affine function of the fills at its
/// start, and the piece of each step that cut.
struct Pass {
    map: [Affine; 3],
    pieces: [Option<Piece>; 3],
}

/// One pass of the fill iteration over the goods in order.
fn pass(p: &Province, f: &mut [f64; 3], w: &mut Work) -> Pass {
    let mut map = [0, 1, 2].map(Affine::of);
    let mut pieces = [None, None, None];
    for j in 0..3 {
        if let Some((s, piece)) = step(p, j, f, w) {
            map[j] = s.after(&map);
            pieces[j] = Some(piece);
        }
    }
    Pass { map, pieces }
}

/// The longest cycle of passes [`jump`] looks for.
const PERIOD: usize = 3;
/// Relative rounding of the affine maps that [`jump`] allows where a good fits with equality.
const ROUNDING: f64 = 1e-12;

/// Where [`jump`] takes the passes.
enum Jump {
    /// The limit, confirmed by running the cycle's passes from it.
    Limit([f64; 3]),
    /// The fills after the last whole cycle that stays on the cycle's pieces.
    Ahead([f64; 3]),
}

/// The passes repeating the cycle of the last passes `cycle` (oldest first), from the fills
/// `f` at the end of the last, as the cycle's affine maps give them: each step on the same
/// piece, every fill not raised, every good the cycle does not lower still fitting. Their
/// limit is the fixed point g of the composed map on the goods the cycle lowers, the others
/// held: taken where the passes reach it on the pieces and running the cycle from g returns g.
/// Where they leave a piece first, the fills after the last whole cycle before that, which
/// the passes reach in exact arithmetic. None where the first cycle already leaves.
fn jump(p: &Province, cycle: &[Pass], f: &[f64; 3], w: &mut Work) -> Option<Jump> {
    let cut: Vec<usize> = (0..3)
        .filter(|&j| cycle.iter().any(|q| q.pieces[j].is_some()))
        .collect();
    let map = cycle.iter().fold([0, 1, 2].map(Affine::of), |acc, q| {
        q.map.map(|o| o.after(&acc))
    });
    let n = cut.len();
    // (I - M_CC) g_C = c_C + M_CN f_N
    let mut a = [[0.0f64; 4]; 3];
    for (r, &j) in cut.iter().enumerate() {
        let row = &map[j];
        if !row.c.is_finite() || !row.d.iter().all(|d| d.is_finite()) {
            return None;
        }
        a[r][3] = row.c
            + (0..3)
                .filter(|k| !cut.contains(k))
                .map(|k| row.d[k] * f[k])
                .sum::<f64>();
        for (s, &k) in cut.iter().enumerate() {
            a[r][s] = f64::from(u8::from(r == s)) - row.d[k];
        }
    }
    for c in 0..n {
        let pivot = (c..n).max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))?;
        if a[pivot][c] == 0.0 {
            return None;
        }
        a.swap(c, pivot);
        let top = a[c];
        for row in a.iter_mut().take(n).skip(c + 1) {
            let ratio = row[c] / top[c];
            for (x, t) in row.iter_mut().zip(&top).skip(c) {
                *x -= ratio * t;
            }
        }
    }
    let mut x = [0.0; 3];
    for c in (0..n).rev() {
        let s: f64 = (c + 1..n).map(|k| a[c][k] * x[k]).sum();
        x[c] = (a[c][3] - s) / a[c][c];
    }
    // the solve rounds: a limit at 0 or at f may come out just beyond it
    let mut g = *f;
    let mut limit = true;
    for (v, &j) in cut.iter().enumerate() {
        limit &= x[v] >= -1e-14 && x[v] <= f[j] + 1e-14;
        g[j] = x[v].clamp(0.0, f[j]);
    }
    // the passes from f, as the maps give them, until they are within rounding of g
    let mut now = *f;
    let ahead = |now: [f64; 3]| (now != *f).then_some(Jump::Ahead(now));
    let scale = cut
        .iter()
        .fold(f64::MIN_POSITIVE, |s, &j| s.max((f[j] - g[j]).abs()));
    for _ in 0..100_000 {
        if limit && cut.iter().all(|&j| (now[j] - g[j]).abs() <= 1e-15 * scale) {
            let mut h = g;
            for _ in cycle {
                pass(p, &mut h, w);
            }
            let size = g.iter().fold(0.0f64, |s, &v| s.max(v));
            let still = (0..3).all(|k| (h[k] - g[k]).abs() <= 1e-12 * size);
            return if still {
                Some(Jump::Limit(h))
            } else {
                ahead(now)
            };
        }
        let mut through = now;
        for q in cycle {
            let mut state = through;
            for j in 0..3 {
                // a good the pass leaves fits where its step comes, one it lowers does not, up
                // to the rounding of the maps
                p.outputs(&state, &mut w.y);
                let (uncovered, rest) = p.balance(j, &w.y, &mut w.users);
                let slack = ROUNDING * (uncovered + rest);
                let on = match &q.pieces[j] {
                    None => uncovered <= rest + slack,
                    Some(piece) => {
                        let next = q.map[j].at(&through);
                        let on = uncovered + slack > rest
                            && next <= state[j]
                            && next >= 0.0
                            && on_piece(p, j, piece, &state, next);
                        state[j] = next;
                        on
                    }
                };
                if !on {
                    return ahead(now);
                }
            }
            through = state;
        }
        if through == now {
            return ahead(now);
        }
        now = through;
    }
    ahead(now)
}

/// The fill of each local good (ADR 0093, Amendment): the limit of the fill iteration from
/// F = 1, passes over the goods in order, each setting F_k to the largest value at most F_k
/// at which k fits ([`step`]). Where the last passes, repeated, would fall to a fixed point
/// without leaving their pieces, the passes jump there ([`jump`], cycles of up to [`PERIOD`]
/// passes). Returns the fills and the passes run, a jump's confirming passes included.
pub fn fills(p: &Province) -> ([f64; 3], usize) {
    let mut w = Work::default();
    let mut f = [1.0; 3];
    let mut passes = 0;
    let mut recent: Vec<Pass> = Vec::with_capacity(PERIOD);
    loop {
        passes += 1;
        let start = f;
        let last = pass(p, &mut f, &mut w);
        if f == start {
            return (f, passes);
        }
        if recent.len() == PERIOD {
            recent.remove(0);
        }
        recent.push(last);
        // a limit from the shortest cycle that reaches one, else the first cycle that runs ahead
        let mut ahead = None;
        for period in 1..=recent.len() {
            let cycle = &recent[recent.len() - period..];
            match jump(p, cycle, &f, &mut w) {
                Some(Jump::Limit(h)) => {
                    passes += period;
                    ahead = Some(h);
                    break;
                }
                Some(Jump::Ahead(h)) => {
                    ahead.get_or_insert(h);
                }
                None => {}
            }
        }
        if let Some(h) = ahead {
            f = h;
            recent.clear();
        }
    }
}

/// Runs the chain for the enterprises of province `i`. On entry `Scratch::produced` holds
/// each enterprise's output capped by capacity and traded input stocks; on return it holds
/// the output at the province's [`fills`], and `Scratch::from_output` the units of each local
/// good the enterprise draws from this tick's output ([`split`]; the rest of its use comes from
/// its stock, the buffer). Returns the passes run.
///
/// `produced` and `from_output` are the province's rows of `Scratch::produced` and
/// `Scratch::from_output`; no other province's state is read or written.
pub fn chain(
    inp: &EconInputs,
    lay: &Layout,
    ledger: &Ledger,
    i: usize,
    produced: &mut [f64],
    from_output: &mut [f64],
) -> usize {
    let province = Province::new(inp, lay, ledger, i, produced);
    let (f, passes) = fills(&province);
    let mut y = Vec::new();
    province.outputs(&f, &mut y);
    let mut users = Vec::new();
    let mut draws = Vec::new();
    for (j, &k) in LOCAL.iter().enumerate() {
        let rest = province.users(j, &y, &mut users);
        draws.resize(users.len(), 0.0);
        split(rest, &users, &mut draws);
        let mut d = draws.iter();
        for (u, &yu) in y.iter().enumerate() {
            from_output[u * N_GOODS + k] = if Some(u) == province.producer[j] {
                province.own[j] * yu
            } else if province.coef[u][j] * yu > 0.0 {
                *d.next().expect("one draw per user")
            } else {
                0.0
            };
        }
    }
    produced.copy_from_slice(&y);
    passes
}

/// [`chain`] for every province, in parallel: `Scratch::produced` from the capped outputs to
/// the outputs, `Scratch::from_output` and `Scratch::chain_iterations`.
pub fn chain_all(w: &mut EconWorld) {
    let (inp, lay, ledger) = (&*w.inputs, &*w.layout, &w.ledger);
    let sc = &mut w.scratch;
    let mut produced: &mut [f64] = &mut sc.produced;
    let mut from_output: &mut [f64] = &mut sc.from_output;
    let mut rows = Vec::with_capacity(inp.n_provinces());
    for i in 0..inp.n_provinces() {
        let n = lay.enterprises(i).len();
        let (p, rest) = std::mem::take(&mut produced).split_at_mut(n);
        let (f, rest_f) = std::mem::take(&mut from_output).split_at_mut(n * N_GOODS);
        (produced, from_output) = (rest, rest_f);
        rows.push((p, f));
    }
    (sc.chain_iterations.par_iter_mut(), rows)
        .into_par_iter()
        .enumerate()
        .for_each(|(i, (iterations, (p, f)))| {
            *iterations = chain(inp, lay, ledger, i, p, f) as u32;
        });
}

#[cfg(test)]
mod tests {
    use super::*;

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
