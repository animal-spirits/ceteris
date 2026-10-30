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
    /// Cycles followed and maps applied by [`jump`].
    work: usize,
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
    // the closed form rounds: step down until the fit holds as `Province::fits` evaluates it
    loop {
        f[j] = x;
        p.outputs(f, &mut w.y);
        if x == 0.0 || p.fits(j, &w.y, &mut w.users) {
            break;
        }
        x = x.next_down().max(0.0);
    }
    if x < f64::MIN_POSITIVE {
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
const PERIOD: usize = 3;
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
/// Passes after which [`fills`] ends on its fallback ([`capped`]), the stall reported: the
/// bound on its passes (a jump's confirming passes count).
pub const PASS_CAP: usize = 4 * STALL_PASSES;
/// The fit the fills keep where the passes stop short of their end: 1e-12 of a good's flows,
/// the Leontief invariant's tolerance (ADR 0093, bounded work).
const FIT: f64 = 1e-12;
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
    /// The fills after the last whole cycle that stays on the cycle's pieces.
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
/// Where they leave a piece first, the fills after the last checked point before that, which
/// the passes reach in exact arithmetic. None where the first cycle already leaves.
fn jump(
    p: &Province,
    cycle: &[Pass],
    f: &[f64; 3],
    budget: &mut usize,
    w: &mut Work,
) -> Option<Jump> {
    if *budget == 0 {
        return None;
    }
    let FixedPoint { cut, map, g, limit } = fixed_point(cycle, f)?;
    // a fall the map gives only to rounding is no fall: no jump extrapolates it (its first
    // cycle allows the maps' rounding on shortages, as the cycles followed one at a time do)
    *budget -= 1;
    let next = follow(p, cycle, f, ROUNDING, w)?;
    if !cut.iter().all(|&j| resolvable(f[j], next[j])) {
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
    let mut power = map;
    let mut last = now;
    for _ in 0..DOUBLINGS {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        w.work += 1;
        let mut at = now;
        for &j in &cut {
            at[j] = power[j].at(&now);
        }
        let valid = cut.iter().all(|&j| at[j] >= 0.0 && at[j] <= last[j])
            && follow(p, cycle, &at, 0.0, w).is_some();
        if !valid {
            break;
        }
        last = at;
        if reached(&at) {
            return confirm(w).or_else(|| ahead(last));
        }
        power = power.map(|o| o.after(&power));
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
            };
        }
        if passes >= PASS_CAP {
            return Fills {
                fills: capped(p, f, &mut w),
                passes,
                work: w.work,
                stalled: true,
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
        if rounding || (since >= STALL_PASSES && !stalled) {
            since = 0;
            recent.clear();
            moves += 1;
            let to = (moves <= MAX_MOVES)
                .then(|| settle(p, &tail, &f, rounding, &mut w))
                .flatten();
            tail.clear();
            if let Some(to) = to {
                f = to;
                continue;
            }
            if rounding {
                return Fills {
                    fills: f,
                    passes,
                    work: w.work,
                    stalled,
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
}
