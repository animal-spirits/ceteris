# Can the epsilon fit be made faster? (2026-11-20)

Short answer: yes, probably by one to two orders of magnitude, but not by tuning the root
finders. The time is in the nested equilibrium solves under them. Nothing here was run; the
profile is read from existing logs and the code on main (5eedd97). Every speed-up number below
is an estimate unless marked measured.

Scope: research only. No kernel or fit code changes (freeze holds). Read:
`crates/ceteris-calibrate/src/{epsilon,border,steady}.rs`, RFC 0360,
`reports/warm-start-spike-2026-11-16.md`, project reports `parallel-refit-scope-2026-11-18.md`
and `tick-profile-2026-11-17.md`, Mac logs in `~/ceteris-logs/` (read-only). Not read: PR #600
(`fit_theta`, Anderson on the data-state passes), so the theta fit is covered only through its
log lines.

## 1. Where the 11 hours go

### The nest (main)

| Level | Code | Method | Tolerance | Count |
|---|---|---|---|---|
| 1 epsilon | `fit_epsilon_inner` | Brent on [0, 2] | 1e-3 | 2 ends + 8 to 12 trials |
| 2 scale passes | `trial` loop | Gauss-Seidel over classes | 1e-5 | up to 10; 2 to 3 seen |
| 3a data-state rounds | `fit_border_nu_seeded` | fixed point on spending | 1e-8 / 1e-13 | up to 20; 2 to 5 seen |
| 4a nu per group | `fit_at_spending` | Brent, groups share an evaluation | 1e-9 | 4 to 8 rounds + about 6 fixed |
| 3b s_c per class | `solve_scale` | Brent on [0, 1] | 1e-6 | 2 ends + about 8 to 12, per class |
| 5 data state | `data_state_spending` | plain fixed point on rebates and PIGL weights | 1e-13 | passes unknown; `calibrate` needs 40 to 178 |
| 6 scaling | `sinkhorn_with` | log-domain Sinkhorn, Anderson m = 8 | 1e-13 | 7 to 4,000 sweeps per call |

Every evaluation at level 4a is one scaling (level 6), started from the nu 0 scaling. Every
evaluation at level 3a and 3b is one full data state (level 5, a scaling per pass) plus one
more scaling, and in `solve_scale` that last scaling starts cold (`data_state_parts` passes
`start: None`).

### Counts from the last full log

`epsilon-d5-2026-11-18.log` (PR #600 branch, 5 classes, full 2000 pack): 40,021 s for two
trials (epsilon 0 and the failing epsilon 2). No line carries a timestamp, so the split between
levels is not known. Counted from the lines:

- 21 `fit_at_spending` calls (4 at epsilon 0, 17 at epsilon 2), 133 Brent rounds. About 260 to
  300 scalings at level 4a.
- 8 `fit_border_nu` calls, 21 data-state rounds: 29 data states.
- 7 completed `solve_scale` calls plus the failing one, about 10 evaluations each (not logged):
  about 80 data states plus 80 cold scalings. 4 `fit_theta` calls of unknown cost.

So two trials cost roughly 110 to 150 data-state fixed points and 350 to 400 further scalings.
A full fit is 10 to 14 trials.

### What one scaling costs

- `calibrate-time.log`: one steady calibration, 40 passes, 88,169 sweeps, 2.06e12 cycles. If
  sweeps dominate, that is about 23 M cycles, roughly 7 ms, per sweep (estimate; the host load
  was 87 and the counters may cover more than that run).
- `val-calib-full-P.log`: 178 passes, 16,938 sweeps. Early passes stop at 1e-6 (the adaptive
  inner tolerance of `calibrate_from`), late ones at 1e-13.
- Sweeps per scaling range from 7 to 4,027. The cause is documented in `steady.rs`
  (`MAX_SWEEPS`): one near-closed block (Brazil's 48 coal origins) contracts by 0.99908 per
  plain sweep; an earlier pack had 0.99995. Anderson does not remove it reliably: passes 120 to
  133 of one run needed 300 to 4,000 sweeps each at a pass change of 1e-7 to 1e-9.
- The sweep computes every good every time, with one joint Anderson history. Goods are
  independent in the scaling, so six goods keep sweeping until the slowest one converges.
- At 7 ms a sweep, 40,021 s is about 5.7 M sweep-equivalents for two trials (estimate).

### Reading

1. The search layers (levels 1 to 4) multiply about 500 expensive solves per trial. Fewer
   evaluations per Brent call is a small lever (measured: warm brackets 1.66x, no-go).
2. The solves are slow for one identifiable reason: a linearly convergent iteration on a
   problem with a contraction factor near 1, run to 1e-13, hundreds of times.
3. Everything is single-threaded inside a trial.
4. Unknown: the share of time in `Setup::settle` (income fixed point to 1e-14), the RAS in
   `demand_weights`, `Model::new` per evaluation, and allocation in the sweep. No log shows it.

One caution that speed does not fix: the last four full refits failed on a structural
infeasibility (class 3 short at scale 0), not on time. A faster fit returns that answer sooner,
which is worth having, but it does not produce an epsilon.

## 2. Candidates

Determinism column: "bit-identical" means `epsilon.json` and goldens do not move; "moves
within tolerance" means deterministic and thread-independent, but a reviewed golden move.

| # | Candidate | Expected gain (estimate) | Determinism | Cost | Under the freeze |
|---|---|---|---|---|---|
| A | Timestamps and counters on the fit | none; unblocks the rest | bit-identical | hours | safe |
| B | Parallel sweep across goods, fixed-order combine | 3 to 6x on scalings on 8+ cores | bit-identical | medium (core solver, golden gates) | needs your go: solver code |
| C | Newton solve for the scaling, per good, Sinkhorn fallback | 5 to 20x on slow scalings, about 1 to 3x on easy ones | moves within 1e-13 | medium to large, RFC | no, after flagship or by override |
| D | Per-good stop and per-good Anderson (`Accel::PerGood` exists) | up to n_goods x on slow scalings | moves within 1e-13 | small | RFC-light; changes fit bits |
| E | Nest hygiene: path-independent start for `solve_scale` scalings, adaptive inner tolerance in `data_state_spending`, Anderson on its passes | 1.5 to 3x on level 3b and 5 | moves within 1e-12 | small | partly on PR #600 already |
| F | One Newton system for (nu, s_c) with implicit derivatives, in place of levels 2 to 4 | 5 to 10x fewer equilibrium solves per trial | moves within tolerance | large, RFC (ADRs 0092, 0376) | no |
| G | Epsilon by derivative (Newton/secant with df/d epsilon) or ITP | 10 to 14 trials down to 4 to 7 sequential | moves within 1e-3 | small after F | superseded by RFC 0360 if a 16 vCPU box exists |
| H | RFC 0360 grid | 2 to 3.5x wall on 16 vCPU, 3 to 4x the CPU | moves within 1e-3 | accepted, needs the box | accepted |
| I | Warm start across epsilon trials | under 2x; measured 1.66x on brackets | not bit-identical; conflicts with ADR 0097 | small | no-go already recorded |
| J | Bayesian or surrogate optimisation over epsilon | negative | - | - | reject |
| K | f32 or loose inner solves with f64 polish | under 2x | breaks the f64 law, or risks wrong roots | - | reject |

### A. Measure first

A timestamp per progress line and four counters (scalings, sweeps, data-state passes, wall
time per level). Without it every number above stays an estimate. Development law: measure
before Fast.

### B. Parallel sweep (scoping item F of 2026-11-18)

Goods are independent in `origin_sums` and `destination_sums`; only the per-good `r` and the
joint Anderson least squares couple them, and those are small. Compute goods in parallel into
fixed slots and combine in index order (rust.md). Bit-identical, so no golden moves, and it
stacks with everything else. Limit: 7 to 10 traded goods, so at most that many cores help;
memory bandwidth is not a concern at about 28,000 unknowns.

### C. Newton for the scaling

The scaling is the dual of an entropy-regularised transport problem: convex, unique up to one
level per good, with a symmetric positive semidefinite Hessian. Sinkhorn is block coordinate
ascent on it and converges linearly at the second eigenvalue, which is the 0.99908. Newton
converges quadratically regardless of that eigenvalue.

- Knight and Ruiz (2013), "A fast algorithm for matrix balancing", IMA J. Numer. Anal. 33:
  inexact Newton with conjugate gradients, matrix-free, each product about one sweep.
- Brauer, Clason, Lorenz and Wirth (2017), "A Sinkhorn-Newton method for entropic optimal
  transport", arXiv:1710.06635: Newton in the log domain, the form `sinkhorn_with` uses.
- Tang, Shavlovsky, Rahmanian, Tardini, Thekumparampil, Xiao and Ying (2024), "Accelerating
  Sinkhorn algorithm with sparse Newton iterations", ICLR 2024, arXiv:2401.12253: Sinkhorn
  warm-up, then Newton on a sparsified Hessian; super-exponential convergence observed. The
  newest practical variant.
- Cohen, Madry, Tsipras and Vladu (2017), "Matrix scaling and balancing via box constrained
  Newton's method and interior point methods", FOCS; Allen-Zhu, Li, Oliveira and Wigderson
  (2017), "Much faster algorithms for matrix scaling", FOCS: complexity results, not code to
  copy.

Here each good is about 4,000 unknowns, so the per-good Hessian can even be formed and
factored densely (16 M entries, a fraction of a second) in a fixed order, which is fully
deterministic. Expected: 5 to 10 Newton steps where Anderson needs 300 to 4,000 sweeps.
Unknowns: the cost of assembling the Hessian through the domestic, access and crossing legs;
whether Newton's last step reaches the 1e-13 stop inside the rounding floor that
`summation_depth` documents (the residual is evaluated the same way, so it should). Keep the
current sweep as the globalising fallback and as the oracle in tests.

Alternatives with less evidence for this problem: over-relaxed Sinkhorn (Thibault, Chizat,
Dossal and Papadakis 2021, Algorithms 14; Lehmann, von Renesse, Sambale and Uschmajew 2022,
Optim. Lett.), which gives about 2 to 20x when the factor is near 1 but needs a tuned
parameter; accelerated gradient variants (Dvurechensky, Gasnikov and Kroshnin 2018, ICML; Lin,
Ho and Jordan 2022, JMLR) and Greenkhorn (Altschuler, Weed and Rigollet 2017, NeurIPS), which
target low accuracy and are the wrong tool at 1e-13; aggregation-disaggregation for nearly
decomposable blocks (Koury, McAllister and Stewart 1984, SIAM J. Alg. Disc. Meth.), which
attacks the Brazil block directly but is more bespoke than Newton.

### D. Per-good stop

Cheaper than C and attacks the same waste: stop sweeping a good once it is within tolerance
and mix each good with its own history. `Accel::PerGood` already exists as a mode. The result
differs from the joint mixing in the last places, so fit outputs move within 1e-13.

### E. Nest hygiene

- `solve_scale` evaluations start their final scaling cold. `fit_at_spending` shows the
  path-independent pattern: start every evaluation from one reference scaling, so an
  evaluation still depends on its argument alone (ADR 0097 holds).
- `data_state_spending` runs every pass's scaling to 1e-13, although `calibrate_from` already
  uses an inner tolerance tied to the outer change. That is the Eisenstat and Walker (1996,
  SIAM J. Sci. Comput. 17) forcing-term rule and is safe when the last passes run tight.
- Its passes are a plain fixed point on main. Anderson on them (Walker and Ni 2011, SIAM J.
  Numer. Anal. 49; Evans, Pollock, Rebholz and Xiao 2020, SIAM J. Numer. Anal. 58) is on PR
  #600; the 11-03 study measured 28 down to 8 iterations on the income loop.
- A Newton-Krylov solve of the pass fixed point (Knoll and Keyes 2004, J. Comput. Phys. 193)
  is the stronger form; only worth it if A shows passes, not sweeps, dominate.

### F. One system instead of four nested searches

Per trial the unknowns are 5 nu, up to 5 s_c and their corner conditions (nu at 0, s_c at 0 or
1): about ten numbers. Today they are found by Brent inside a fixed point inside Gauss-Seidel,
about 500 equilibrium solves. A Newton iteration on the ten equations needs the equilibrium
and its sensitivities: one nonlinear solve plus about ten linear solves with the same Jacobian
per step (implicit function theorem), and 4 to 6 steps. With C in place the Jacobian
factorisation is already there.

- Su and Judd (2012), "Constrained optimization approaches to estimation of structural
  models", Econometrica 80: the nested fixed point against the joint formulation.
- Dubé, Fox and Su (2012), Econometrica 80: nested fixed points with loose inner tolerances
  give wrong estimates; tight inner solves or the joint system. This is also the argument
  against K.
- Iskhakov, Lee, Rust, Schjerning and Seo (2016), Econometrica 84: a nested fixed point with a
  Newton inner solve is competitive with the joint system. This supports doing C before F.
- Blondel et al. (2022), "Efficient and modular implicit differentiation", NeurIPS;
  Eisenberger et al. (2022), "A unified framework for implicit Sinkhorn differentiation",
  CVPR: derivatives of a scaling's solution without unrolling it.
- Corners: semismooth Newton for complementarity (De Luca, Facchinei and Kanzow 1996, Math.
  Program. 75) or an explicit active set with the current pin rules.

Risks: the corner logic (`is_pinned`, clamps, held groups, typed infeasibility of RFC 0360) is
where the fit's correctness lives today, and a Newton iteration must reproduce each case
exactly. ADRs 0092 and 0376 name Brent, so this is an RFC. Largest gain, largest risk, and it
should wait for the class 3 structure decision, which may change the system being solved.

### G, H. Epsilon

f(epsilon) is one-dimensional, monotone by design, and needed to 1e-3 only. With F, df/d
epsilon comes from one more linear solve, and Newton or secant reaches 1e-3 in 3 to 5 trials.
Without derivatives, the ITP method (Oliveira and Takahashi 2020, ACM Trans. Math. Softw. 47)
keeps bisection's worst case with fewer evaluations on average than Brent; a modest gain, 10
to 14 trials down to perhaps 7 to 9. RFC 0360's grid is already accepted and is the better
choice once a 16 vCPU host exists. On the Mac alone, G is the alternative.

### I, J, K. Not recommended

- I: measured no-go (1.66x against a 3x bar). Warm state across trials would also make a
  trial depend on the search path, against ADR 0097.
- J: Bayesian optimisation is for expensive black-box objectives without structure. A monotone
  one-dimensional root has a bracket; bisection-type methods are provably better.
- K: f32 is excluded by law. Loose f64 solves with a polish save only the cheap early
  iterations of a linearly convergent method, and Dubé, Fox and Su document the failure mode.

## 3. Ranked recommendation

Most correct and robust first.

1. A, the measured profile. Hours of work, no behaviour change. Everything else is sized by it.
2. B, the bit-identical parallel sweep, and H, the accepted grid when the box exists. No
   answer changes, so no correctness risk beyond the golden gates.
3. C with D as its cheap first step: remove the slow mode at its root with a per-good Newton
   solve, the sweep as fallback and oracle. Convex problem, unique solution, standard method.
4. E, the nest hygiene items not already on PR #600.
5. F then G: the joint system. After the class 3 decision and after C.

Stacked estimate for one trial on the Mac, 5.5 h today: B and C with D and E give roughly 10
to 30x (10 to 30 minutes); F on top brings a trial to minutes. Not measured, and the unknown
share of non-scaling time in section 1 caps it.

Under the freeze only A is clearly allowed. B to F change solver or fit code and need your go;
C, D, E and F move fit outputs within tolerance, so each needs an RFC, a switch-on behaviour
test and a reviewed golden move.

## 4. First one-hour spike

Goal: replace the estimates in section 1 with numbers, and get a go/no-go for C.

1. On a branch, not merged: wall-clock stamps on the progress lines and counters for
   scalings, sweeps per scaling per good, data-state passes, and time in `settle`, RAS,
   `Model::new` and the sweep. Run `fit_at_epsilon` once at epsilon 0 on the CI small pack in
   the cloud (no Mac run).
2. Dump the inputs of the slowest scaling of that run. Offline, solve it with a dense per-good
   Newton iteration from the same start and compare with the sweep.

Go for C if Newton reaches the 1e-13 stop in at most 20 steps on a scaling where the sweep
needs more than 500, and agrees with the sweep's log weights to 1e-11. Go for B if sweeps are
at least 60% of the trial's wall time. If passes or `settle` dominate instead, E moves to the
top and C waits. Whatever the outcome, record it here.

Caveat: the small pack may not contain a near-closed block like Brazil's coal, in which case
step 2 needs one scaling dumped from the full pack on the Mac, about a minute of Mac time.
