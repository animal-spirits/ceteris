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
    /// The cycle's composed map squared k times, k = 0, 1, ..., in [`jump`].
    powers: Vec<[Affine; 3]>,
    /// Cycles followed and maps applied by [`jump`].
    work: usize,
    /// Fits evaluated by the last [`descend`].
    descent: usize,
    /// Whether a [`jump`] since this was cleared did not apply: the cycle's map undefined or
    /// singular, its fall within rounding, or the budget spent.
    inapplicable: bool,
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
    // No kink lies inside (lo, hi), so each user's smallest other fill m and stock threshold
    // b / a lie at or below lo or at or above hi: its role on the piece follows from comparing
    // them with the ends, with no midpoint to round onto an end.
    let mut slope = 0.0;
    for u in users() {
        let (a, b) = (p.coef[u][j] * p.cap[u], p.stock[u][j]);
        let m = p.least(u, &others);
        let role = if a > 0.0 && m >= hi && b / a <= lo {
            slope += a;
            num.c += b;
            Role::OnStep
        } else if a > 0.0 && m <= lo && a * m > b {
            // m <= lo < hi <= 1, so another of the user's fills is below 1
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
    f[j] = descend(p, j, f, x, w);
    if f[j] < f64::MIN_POSITIVE {
        f[j] = 0.0;
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

/// Units in the last place [`descend`] steps down one at a time before it searches.
const DESCENT: usize = 8;

/// The largest fill at most `x` at which good j fits as [`Province::fits`] evaluates it, the
/// other fills `f` as they stand; leaves f\[j\] at an unspecified value. The closed form of
/// [`step`] rounds, so its root may not fit: the fill steps down a unit in the last place at a
/// time, and after [`DESCENT`] units searches the bit patterns of [0, x] by bisection. The
/// search returns what stepping down would: the evaluated fit is monotone in the fill (each
/// user's need is a product of a min over the fills, and rounded products, maxima and sums are
/// monotone; the producer's output does not depend on its own good), 0 always fits (every
/// uncovered need is then 0) and positive f64 order as their bits. A root above a flat piece,
/// where U - R is rounding at a stock threshold, could otherwise take up to 2^62 steps.
fn descend(p: &Province, j: usize, f: &mut [f64; 3], x: f64, w: &mut Work) -> f64 {
    debug_assert!(
        p.producer[j].is_none_or(|q| p.coef[q][j] == 0.0),
        "good {j}'s producer takes its own good as an input: the fit is not monotone"
    );
    w.descent = 0;
    let fits = |x: f64, f: &mut [f64; 3], w: &mut Work| {
        w.descent += 1;
        debug_assert!(w.descent <= DESCENT + 65, "descent unbounded");
        f[j] = x;
        p.outputs(f, &mut w.y);
        x == 0.0 || p.fits(j, &w.y, &mut w.users)
    };
    let mut x = x;
    for _ in 0..DESCENT {
        if fits(x, f, w) {
            return x;
        }
        x = x.next_down().max(0.0);
    }
    if fits(x, f, w) {
        return x;
    }
    // invariant: lo fits, hi does not
    let (mut lo, mut hi) = (0u64, x.to_bits());
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if fits(f64::from_bits(mid), f, w) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::from_bits(lo)
}

/// What a pass did: each good's fill at its end as an affine function of the fills at its
/// start, and the piece of each step that cut.
#[derive(Clone)]
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

/// A fall of a fill, per pass or cycle, of more than [`RESOLVABLE`] units in the last place of
/// the fill. A smaller fall is taken as no fall, whatever its source: rounding, or a real loop
/// whose gain is within about 9e-13 of 1.
fn resolvable(before: f64, after: f64) -> bool {
    before - after > RESOLVABLE * f64::EPSILON * before
}

/// Units in the last place a fill must fall by, per pass or cycle, for [`jump`] or the stall
/// rule of [`fills`] to extrapolate the fall: the maps sum a few rounded terms and a step's
/// descent moves its fill by a few units, so 4,096 (relative 9.1e-13) is three orders above
/// what rounding gives and still below the 1e-12 the fills are compared at.
const RESOLVABLE: f64 = 4096.0;

/// How far below the fills, relative, a stop moves to a cycle's fixed point: the rest of a fall
/// below the margin per pass at a gain up to 1 - 1/1024 (1,024 margins, relative 9.3e-10).
const TAIL: f64 = 1024.0 * RESOLVABLE * f64::EPSILON;

/// The longest cycle of passes [`jump`] looks for.
pub const PERIOD: usize = 3;
/// Relative rounding of the affine maps that [`jump`] allows where a good fits with equality.
const ROUNDING: f64 = 1e-12;
/// How far beyond [0, f] the solve of [`jump`] may put a fixed point by rounding.
const SOLVE_ROUNDING: f64 = 1e-14;
/// Distance to the fixed point, relative to the distance from the start, at which the
/// trajectory of [`jump`] has reached it.
const REACHED: f64 = 1e-15;
/// How close, relative to the largest fill, the cycle's passes from a fixed point must return
/// it for [`jump`] to take it.
const CONFIRMED: f64 = 1e-12;
/// Passes after which [`fills`] moves the goods still falling to the last pass's fixed point
/// (ADR 0093, bounded work).
pub const STALL_PASSES: usize = 256;
/// The bound on the passes of [`fills`], a jump's confirming passes included: at
/// `PASS_CAP - PERIOD` passes it ends on its fallback ([`capped`]), the stall reported.
pub const PASS_CAP: usize = 4 * STALL_PASSES;
/// The fit the fills keep where the passes stop short of their end: 1e-12 of a good's flows,
/// which the Leontief invariant allows for local draws (ADR 0093, bounded work and amendment of
/// 2026-10-30).
pub(crate) const FIT: f64 = 1e-12;
/// Moves to a pass's fixed point after which [`fills`] takes the fills as they are.
const MAX_MOVES: usize = 3;
/// Cycles followed and maps applied by the jumps within one [`fills`], all jumps together.
const CYCLE_BUDGET: usize = 4096;
/// The most work of one [`fills`] call ([`Fills::work`]): a doubling applies a map and follows
/// one cycle for one unit of the budget.
pub const WORK_BOUND: usize = 2 * CYCLE_BUDGET;
/// Cycles one [`jump`] follows one at a time before it doubles.
const SEQUENTIAL: usize = 64;
/// Doublings of the cycles [`jump`] takes at once: 2^64 cycles is beyond any fall.
const DOUBLINGS: usize = 64;

/// Where [`jump`] takes the passes.
enum Jump {
    /// The limit, confirmed by running the cycle's passes from it.
    Limit([f64; 3]),
    /// The fills after the last whole cycle that stays on the cycle's pieces (short of it where
    /// the jumps' shared budget runs out).
    Ahead([f64; 3]),
}

/// The fixed point g of the cycle of passes `cycle` (oldest first) from the fills `f` at the
/// end of its last pass: on the goods the cycle lowers, the composed map's fixed point, the
/// others held at f. None where a step's map is undefined or the system is singular.
fn fixed_point(cycle: &[Pass], f: &[f64; 3]) -> Option<FixedPoint> {
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
        limit &= x[v] >= -SOLVE_ROUNDING && x[v] <= f[j] + SOLVE_ROUNDING;
        // + 0.0 turns a -0.0 into 0.0
        g[j] = x[v].clamp(0.0, f[j]) + 0.0;
    }
    Some(FixedPoint { cut, map, g, limit })
}

/// What [`fixed_point`] returns.
struct FixedPoint {
    /// The goods the cycle lowers.
    cut: Vec<usize>,
    /// The cycle's composed map.
    map: [Affine; 3],
    g: [f64; 3],
    /// Whether g lies in [0, f] up to the solve's rounding.
    limit: bool,
}

/// The passes repeating the cycle of the last passes `cycle` (oldest first), from the fills
/// `f` at the end of the last, as the cycle's affine maps give them ([`follow`]): each step on
/// the same piece, every fill not raised, every good the cycle does not lower still fitting.
/// Their limit is the fixed point g of the composed map on the goods the cycle lowers, the
/// others held: taken where the passes reach it on the pieces and running the cycle from g
/// returns g. The passes are followed one cycle at a time up to [`SEQUENTIAL`] cycles (and
/// `budget` within the call to [`fills`]), then 2^k cycles at once, checked at those points.
/// Where they leave a piece first, the fills after the last whole cycle on the pieces, which the
/// passes reach in exact arithmetic: between the last two checked points it is found by
/// bisection of the cycles (as the doubling, the cycles that stay on the pieces taken to be the
/// first ones), so the next pass crosses a kink. None where the first cycle already leaves.
fn jump(
    p: &Province,
    cycle: &[Pass],
    f: &[f64; 3],
    budget: &mut usize,
    w: &mut Work,
) -> Option<Jump> {
    if *budget == 0 {
        w.inapplicable = true;
        return None;
    }
    let Some(FixedPoint { cut, map, g, limit }) = fixed_point(cycle, f) else {
        w.inapplicable = true;
        return None;
    };
    // a fall the map gives only to rounding is no fall: no jump extrapolates it (its first
    // cycle allows the maps' rounding on shortages, as the cycles followed one at a time do)
    *budget -= 1;
    let next = follow(p, cycle, f, ROUNDING, w)?;
    if !cut.iter().all(|&j| resolvable(f[j], next[j])) {
        w.inapplicable = true;
        return None;
    }
    let mut now = *f;
    let ahead = |now: [f64; 3]| (now != *f).then_some(Jump::Ahead(now));
    let scale = cut
        .iter()
        .fold(f64::MIN_POSITIVE, |s, &j| s.max((f[j] - g[j]).abs()));
    let reached =
        |s: &[f64; 3]| limit && cut.iter().all(|&j| (s[j] - g[j]).abs() <= REACHED * scale);
    let confirm = |w: &mut Work| {
        let mut h = g;
        for _ in cycle {
            pass(p, &mut h, w);
        }
        let size = g.iter().fold(0.0f64, |s, &v| s.max(v));
        (0..3)
            .all(|k| (h[k] - g[k]).abs() <= CONFIRMED * size)
            .then_some(Jump::Limit(h))
    };
    // the passes the maps give, one cycle at a time, up to the shared budget
    for _ in 0..SEQUENTIAL {
        if reached(&now) {
            return confirm(w).or_else(|| ahead(now));
        }
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        match follow(p, cycle, &now, ROUNDING, w) {
            Some(next) if next != now => now = next,
            _ => return ahead(now),
        }
    }
    // then 2^k cycles at once (the composed map squared k times), checked at those points
    // with no rounding allowance on the shortages: a fall too slow to follow cycle by cycle,
    // typically a collapse towards 0 at a rate near 1
    // the fills `power` gives from `from`, where they lie in [0, below] and on the pieces
    let on = |from: &[f64; 3], power: &[Affine; 3], below: &[f64; 3], w: &mut Work| {
        let mut at = *from;
        for &j in &cut {
            at[j] = power[j].at(from);
        }
        let valid = cut.iter().all(|&j| at[j] >= 0.0 && at[j] <= below[j])
            && follow(p, cycle, &at, 0.0, w).is_some();
        valid.then_some(at)
    };
    w.powers.clear();
    w.powers.push(map);
    let mut last = now;
    let mut left = false;
    for k in 0..DOUBLINGS {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        w.work += 1;
        let power = w.powers[k];
        let Some(at) = on(&now, &power, &last, w) else {
            left = true;
            break;
        };
        last = at;
        if reached(&at) {
            return confirm(w).or_else(|| ahead(last));
        }
        w.powers.push(power.map(|o| o.after(&power)));
    }
    // The passes left the pieces between the last two checked points, 2^(k-1) and 2^k cycles
    // on: the last point before from which a cycle stays on them, by bisection of the cycles in
    // between (adding 2^i cycles for i from k - 2 down to 0 where the cycle from there stays on
    // the pieces; the cycles that do are the first ones), then that last whole cycle.
    if left {
        let checked = w.powers.len() - 1;
        for i in (0..checked.saturating_sub(1)).rev() {
            if *budget == 0 {
                return ahead(last);
            }
            *budget -= 1;
            w.work += 1;
            let power = w.powers[i];
            if let Some(at) = on(&last, &power, &last, w) {
                last = at;
                if reached(&at) {
                    return confirm(w).or_else(|| ahead(last));
                }
            }
        }
        if *budget > 0 {
            *budget -= 1;
            if let Some(next) = follow(p, cycle, &last, 0.0, w) {
                last = next;
            }
        }
    }
    ahead(last)
}

/// One cycle of passes from fills `from` as the cycle's maps give them: the fills at its end,
/// or None where a step leaves its piece, a fill would rise or fall below 0, a good a pass
/// leaves does not fit where its step comes (up to [`ROUNDING`] of its flows, the maps'
/// rounding), or a good it lowers is not short there by more than `short` of its flows.
fn follow(
    p: &Province,
    cycle: &[Pass],
    from: &[f64; 3],
    short: f64,
    w: &mut Work,
) -> Option<[f64; 3]> {
    w.work += 1;
    let mut through = *from;
    for q in cycle {
        let mut state = through;
        for j in 0..3 {
            p.outputs(&state, &mut w.y);
            let (uncovered, rest) = p.balance(j, &w.y, &mut w.users);
            let slack = ROUNDING * (uncovered + rest);
            let on = match &q.pieces[j] {
                None => uncovered <= rest + slack,
                Some(piece) => {
                    let next = q.map[j].at(&through);
                    let on = uncovered + short * (uncovered + rest) > rest
                        && next <= state[j]
                        && next >= 0.0
                        && on_piece(p, j, piece, &state, next);
                    state[j] = next;
                    on
                }
            };
            if !on {
                return None;
            }
        }
        through = state;
    }
    Some(through)
}

/// The fill of each local good (ADR 0093, Amendment): the limit of the fill iteration from
/// F = 1, passes over the goods in order, each setting F_k to the largest value at most F_k
/// at which k fits ([`step`]). Where the last passes, repeated, would fall to a fixed point
/// without leaving their pieces, the passes jump there ([`jump`], cycles of up to [`PERIOD`]
/// passes) where the fall is resolvable. The work is bounded (ADR 0093, bounded work): a pass
/// whose falls are all within rounding, or the [`STALL_PASSES`]th pass, settles at a fixed
/// point of the last passes ([`settle`]), or stops there (where it is at what f64 resolves,
/// every good fitting to 1e-12 of its flows), or reports a stall; a fill below the smallest
/// normal f64 is 0.
pub fn fills(p: &Province) -> Fills {
    let mut w = Work::default();
    let mut f = [1.0; 3];
    let mut passes = 0;
    let mut recent: Vec<Pass> = Vec::with_capacity(PERIOD);
    // the last passes, kept across jumps: the cycle whose fixed point a stop moves to
    let mut tail: Vec<Pass> = Vec::with_capacity(PERIOD);
    let mut budget = CYCLE_BUDGET;
    let mut since = 0;
    let mut moves = 0;
    let mut stalled = false;
    let mut uncovered = 0;
    // `uncovered` at the last move: a stall charges every pass since
    let mut mark = 0;
    loop {
        passes += 1;
        since += 1;
        let start = f;
        let last = pass(p, &mut f, &mut w);
        if f == start {
            return Fills {
                fills: f,
                passes,
                work: w.work,
                stalled,
                uncovered,
            };
        }
        // a jump's confirming passes come after this check, at most PERIOD of them
        if passes + PERIOD >= PASS_CAP {
            return Fills {
                fills: capped(p, f, &mut w),
                passes,
                work: w.work,
                stalled: true,
                uncovered,
            };
        }
        if tail.len() == PERIOD {
            tail.remove(0);
        }
        tail.push(last.clone());
        // A pass whose falls are all within rounding has reached what f64 resolves, and a pass
        // past the stall bound is falling too slowly to follow: either way the cycle of the
        // last one to three passes ends at its fixed point, clamped to [0, f], and the goods it
        // would lower by more than rounding move there, where the cycle's passes from there
        // return it. Where none would after a rounding pass (the fills are at the fixed point
        // to rounding, or the map falls to 0 by less than the margin: no constant, so the fixed point
        // is 0, and no resolvable fall), the fills are taken as they are; after a stall, the
        // plain passes go on and the stall is reported.
        let lowered: Vec<usize> = (0..3).filter(|&j| f[j] != start[j]).collect();
        let rounding = lowered.iter().all(|&j| !resolvable(start[j], f[j]));
        // what this pass adds to `uncovered`, None once counted: a good cut by rounding alone
        // while another falls changes the pieces where no kink lies, and costs two passes as a
        // kink does (this one and the one whose jump it ends)
        let mut charge = if !rounding && lowered.iter().any(|&j| !resolvable(start[j], f[j])) {
            Some(2)
        } else {
            Some(usize::from(stalled))
        };
        if rounding || (since >= STALL_PASSES && !stalled) {
            if !rounding {
                let this = charge.take().unwrap_or(0);
                uncovered = mark + since.max(uncovered + this - mark);
            }
            since = 0;
            recent.clear();
            moves += 1;
            let to = (moves <= MAX_MOVES)
                .then(|| settle(p, &tail, &f, rounding, &mut w))
                .flatten();
            tail.clear();
            if let Some(to) = to {
                uncovered += usize::from(rounding);
                mark = uncovered;
                f = to;
                continue;
            }
            mark = uncovered;
            if rounding {
                return Fills {
                    fills: f,
                    passes,
                    work: w.work,
                    stalled,
                    uncovered,
                };
            }
            stalled = true;
        }
        if recent.len() == PERIOD {
            recent.remove(0);
        }
        recent.push(last);
        // a limit from the shortest cycle that reaches one, else the first cycle that runs ahead
        let mut ahead = None;
        w.inapplicable = false;
        for period in 1..=recent.len() {
            let cycle = &recent[recent.len() - period..];
            match jump(p, cycle, &f, &mut budget, &mut w) {
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
        if let Some(c) = charge {
            let missed = (ahead.is_none() && w.inapplicable) || budget == 0;
            uncovered += c.max(usize::from(missed));
        }
        if let Some(h) = ahead {
            f = h.map(|x| if x < f64::MIN_POSITIVE { 0.0 } else { x });
            recent.clear();
        }
    }
}

/// Where the passes stop or stall at fills `f`, the fixed point of a cycle of the last passes
/// `tail` (oldest first), clamped to [0, f], where it lowers some good resolvably and the
/// cycle's passes from it return it to [`CONFIRMED`]. The last pass's fixed point is taken
/// wherever it lies; at a stop (`rounding`) the cycles of two and three passes are tried next,
/// their fixed points only within [`TAIL`] of f (the rest of a fall already below the margin per
/// pass), since a cycle across pieces may have fixed points the passes never reach. A map that
/// falls to 0 only by rounding at a stop (no constant: every fixed point of the goods it lowers
/// is 0) is not followed. None where no cycle moves.
fn settle(
    p: &Province,
    tail: &[Pass],
    f: &[f64; 3],
    rounding: bool,
    w: &mut Work,
) -> Option<[f64; 3]> {
    let periods = if rounding { 1..=tail.len() } else { 1..=1 };
    for period in periods.filter(|&n| n <= tail.len()) {
        let cycle = &tail[tail.len() - period..];
        // a fixed point outside [0, f] beyond the solve's rounding is not where the passes go
        let Some(fp) = fixed_point(cycle, f).filter(|fp| fp.limit) else {
            continue;
        };
        if rounding && fp.cut.iter().all(|&j| fp.g[j] == 0.0) {
            continue;
        }
        let mut to = *f;
        for &j in &fp.cut {
            let g = fp.g[j].clamp(0.0, f[j]) + 0.0;
            let near = period == 1 || f[j] - g <= TAIL * f[j];
            if g.is_finite() && resolvable(f[j], g) && near {
                to[j] = g;
            }
        }
        if to == *f {
            continue;
        }
        let mut h = to;
        for _ in cycle {
            pass(p, &mut h, w);
        }
        let size = to.iter().fold(0.0f64, |s, &v| s.max(v));
        if (0..3).all(|k| (h[k] - to[k]).abs() <= CONFIRMED * size) {
            return Some(to);
        }
    }
    None
}

/// The fills [`fills`] returns at [`PASS_CAP`]: the current ones, where every good fits to
/// [`FIT`] of its flows, else with the goods that do not set to 0 one at a time in good order
/// until every good does (all fills 0 always fit: no user of a local good then runs).
fn capped(p: &Province, mut f: [f64; 3], w: &mut Work) -> [f64; 3] {
    loop {
        p.outputs(&f, &mut w.y);
        let short = (0..3).find(|&j| {
            let (uncovered, rest) = p.balance(j, &w.y, &mut w.users);
            uncovered > rest + FIT * (uncovered + rest)
        });
        match short {
            Some(j) => f[j] = 0.0,
            None => return f,
        }
    }
}

/// What [`fills`] returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fills {
    pub fills: [f64; 3],
    /// Passes run, a jump's confirming passes included.
    pub passes: usize,
    /// Cycles followed and maps applied by the jumps.
    pub work: usize,
    /// Whether the stall rule found no fixed point to move to and left the fall to the plain
    /// passes (a diagnostic).
    pub stalled: bool,
    /// Passes outside the kink accounting of the pass bound (ADR 0093; `specs/econ/SPEC.md`,
    /// Production), which holds where every jump applies: a pass after which no jump moved and
    /// one did not apply (its cycle's map undefined or singular, its fall within rounding, or
    /// the jumps' budget spent); two for a pass that cuts a good by rounding alone while another
    /// falls resolvably; a pass a stop moves on from; every pass since the last move when the
    /// stall rule acts, and every pass after a stall (a diagnostic).
    pub uncovered: usize,
}

/// Runs the chain for the enterprises of province `i`. On entry `Scratch::produced` holds
/// each enterprise's output capped by capacity and traded input stocks; on return it holds
/// the output at the province's [`fills`], and `Scratch::from_output` the units of each local
/// good the enterprise draws from this tick's output ([`split`]; the rest of its use comes from
/// its stock, the buffer). Returns the passes run and whether the stall rule left a fall
/// unresolved ([`Fills::stalled`]).
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
) -> (usize, bool) {
    let province = Province::new(inp, lay, ledger, i, produced);
    let Fills {
        fills: f,
        passes,
        stalled,
        ..
    } = fills(&province);
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
    (passes, stalled)
}

/// [`chain`] for every province, in parallel: `Scratch::produced` from the capped outputs to
/// the outputs, `Scratch::from_output`, `Scratch::chain_iterations` and `Scratch::chain_stalled`.
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
    (
        sc.chain_iterations.par_iter_mut(),
        sc.chain_stalled.par_iter_mut(),
        rows,
    )
        .into_par_iter()
        .enumerate()
        .for_each(|(i, (iterations, stalled, (p, f)))| {
            let (passes, stall) = chain(inp, lay, ledger, i, p, f);
            (*iterations, *stalled) = (passes as u32, stall);
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

    /// A province of the installed pack under the B6 split rule, frictionless, where good 8's
    /// producer has no capacity (R = 0) and, at fills [0.9776, 1, 1], user 3 sits exactly at its
    /// stock threshold of good 8: (coef cap) F_7 - b is 0, while coef (cap F_7) - b, as
    /// `Province::fits` evaluates it, is 1.8e-12. The closed form puts the root at 1 on a piece
    /// where U is flat at 0 and the evaluated fit fails by rounding, so the fill must cross the
    /// whole piece down to F_7. Stepping down one unit at a time took about 2e14 steps (over 40 minutes).
    fn flat_piece_province() -> Province {
        let b = f64::from_bits;
        Province {
            cap: vec![
                b(0x412ca1b14416c37a),
                b(0x0),
                b(0x0),
                b(0x413b6304bf9fe3d9),
                b(0x0),
                b(0x0),
                b(0x0),
                b(0x41843e5a458416d9),
            ],
            coef: vec![
                [
                    b(0x3f85a6f0b9b3de98),
                    b(0x3f7e92ae222c96b8),
                    b(0x3fc63815566005fa),
                ],
                [
                    b(0x3f9598f71129cc52),
                    b(0x3f79d47b198be467),
                    b(0x3fdfab0aed7215a1),
                ],
                [
                    b(0x3f8ce1e978c9b3c6),
                    b(0x3f817caeb6d4c865),
                    b(0x3fcfe8ac55ea2841),
                ],
                [
                    b(0x3f9fc40dc68be582),
                    b(0x3f7f8bd01844bef0),
                    b(0x3fdc141a3f614221),
                ],
                [
                    b(0x3f95d4d30cb01947),
                    b(0x3f8a7e4722eaf7c8),
                    b(0x3fe43550d69303c6),
                ],
                [b(0x0), b(0x3f9f7ffeb7ac1bc6), b(0x3fbd7e85874df7ef)],
                [b(0x3f6e4a140d81724a), b(0x0), b(0x3fc4d72654a714d9)],
                [b(0x3f890a0d967c814b), b(0x3f8e3b0f260305de), b(0x0)],
            ],
            stock: vec![
                [
                    b(0x40d421b1e80faeb2),
                    b(0x40f18cd85b25b322),
                    b(0x414163a049f2f949),
                ],
                [
                    b(0x40e78644fc76e453),
                    b(0x40cc22755f56162d),
                    b(0x41313f2f15bbe47f),
                ],
                [
                    b(0x40e81a6cb4b96176),
                    b(0x40e6538a32796eb6),
                    b(0x41345e9e2d5b1bd6),
                ],
                [
                    b(0x40ea93e85f71d36e),
                    b(0x40ca64da27b1b2a8),
                    b(0x4149df4744fbe9dd),
                ],
                [
                    b(0x40cec03480ddd4b8),
                    b(0x40c2a8a10b6fffb4),
                    b(0x411c76ef7a254b22),
                ],
                [
                    b(0x4132e6633c80f92c),
                    b(0x411b5a116c5dd448),
                    b(0x41399c3671587619),
                ],
                [
                    b(0x41043dfd0a783826),
                    b(0x41611b52aecc713f),
                    b(0x4169c4178bd0d50e),
                ],
                [
                    b(0x41406b3fc97b9847),
                    b(0x415a6620efa51dfa),
                    b(0x41a0c1d444f73464),
                ],
            ],
            producer: [Some(5), Some(6), Some(7)],
            own: [
                b(0x3fb5c42c41795521),
                b(0x3fbbac4c897bd664),
                b(0x3fcf61ba23120be3),
            ],
        }
    }

    /// Utilities and construction inputs of each other at a loop gain of 0.995 per pass, and a
    /// third user of utilities whose stock covers its need below fill 1e-3: the fall reaches
    /// that kink after about 1,500 cycles, past the 64 followed one at a time and between two
    /// points the doubling checks (1,088 and 2,112 cycles).
    fn slow_fall_province() -> Province {
        Province {
            cap: vec![1.0, 1.0, 1.0],
            coef: vec![[0.0, 0.5, 0.0], [0.50151, 0.0, 0.0], [0.001, 0.0, 0.0]],
            stock: vec![[0.0; 3], [0.0; 3], [1e-6, 0.0, 0.0]],
            producer: [Some(0), Some(1), None],
            own: [0.5, 0.5, 0.0],
        }
    }

    /// `Jump::Ahead` ends after the last whole cycle on the cycle's pieces: one more cycle from
    /// there leaves them, and the next pass crosses the kink. Ending at the doubling's last
    /// check, about 400 cycles short, it took 7 passes.
    #[test]
    fn a_jump_ahead_ends_after_the_last_whole_cycle_on_the_pieces() {
        let p = slow_fall_province();
        let mut w = Work::default();
        let mut f = [1.0; 3];
        let cycle = [pass(&p, &mut f, &mut w), pass(&p, &mut f, &mut w)];
        let cycle = &cycle[1..];
        let mut budget = CYCLE_BUDGET;
        let Some(Jump::Ahead(h)) = jump(&p, cycle, &f, &mut budget, &mut w) else {
            panic!("no jump ahead from {f:?}");
        };
        let kink = 1e-6 / 0.001;
        assert!(h[0] > kink, "{h:?} past the kink");
        assert!(
            follow(&p, cycle, &h, 0.0, &mut w).is_none(),
            "a whole cycle from {h:?} stays on the pieces"
        );
        // the pass from F = 1 (both goods short), a jump to the kink, the pass across it, a
        // jump to the limit and its confirming pass, and the pass that changes nothing
        let out = fills(&p);
        assert_eq!((out.fills, out.passes), ([0.0, 0.0, 1.0], 4), "{out:?}");
    }

    #[test]
    fn a_root_above_a_flat_piece_descends_in_bounded_work() {
        let p = flat_piece_province();
        let mut w = Work::default();
        let lo = 0.9776130665525171;
        let mut f = [lo, 1.0, 1.0];
        let (_, piece) = step(&p, 1, &mut f, &mut w).expect("good 8 is short at fill 1");
        assert!(
            piece.roles.iter().all(|&(_, r)| r != Role::OnStep),
            "{piece:?}"
        );
        // the largest f64 at most 1 at which good 8 fits: below the piece, the next one up not
        let x = f[1];
        let fits_at = |x: f64| {
            let mut g = f;
            g[1] = x;
            let mut y = Vec::new();
            p.outputs(&g, &mut y);
            p.fits(1, &y, &mut Vec::new())
        };
        assert!(x < lo && fits_at(x) && !fits_at(x.next_up()), "{x}");
        // DESCENT steps and the check after them, then at most 64 bisections; one unit at a time
        // took about 2e14
        assert!(
            w.descent <= DESCENT + 1 + 64,
            "{} fits evaluated",
            w.descent
        );
        let out = fills(&p);
        assert!(out.passes <= PASS_CAP, "{out:?}");
    }
}
