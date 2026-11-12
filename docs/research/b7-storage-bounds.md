# B7 follow-up: storage values that keep the fixture and pack stable

Status: proposal for the owner. No defaults changed, nothing committed. Analysis at the code of
edc2e8d / ee79611 (local branch claude/storage-bounds, sleepy-benz-72208e worktree; probe edits
reverted).

## Proposal

**Set P146 g_s to 0.006 a month (from 0.08) and keep every other approved storage value**
(psi 20, 1.0, 3.571, 5.714; zeta, delta and k unchanged).

- Source: Milani (2007, JME 54, "Expectations, learning and macroeconomic persistence") estimates
  the constant learning gain at a posterior mean of 0.0183 a quarter, which is 0.0061 a month
  (verified in the working paper text). That is P45's start, and ADR 0091 already gives P146
  "prior as P45". The 0.08 came from GHR's 12-month signal window. With unit pass-through, though,
  pi^e_og is a forecast of the full trend, not a signal, and a 12-month window at full weight is
  what drives the momentum loop.
- Largest values with margin. The margin is the project's m = 0.5 (P03d): each parameter at most
  half its limit. On a single market, over E in [0.3, theta + 1] (0.3 is the island's effective E)
  and the pack's eps range (0.09 to 30.4), in both modes, with the per-market cap applied:

| g_s | psi_ag limit | largest psi_ag at m = 0.5 |
|---|---|---|
| 0.08 (approved) | 14.7 | 7 |
| 0.04 | 17.0 | 8.5 |
| 0.02 | 27.0 | 13.5 |
| 0.01 | 42.4 (38.1 at E 0.2) | 21 |
| 0.006 | 43.5 (42.5 at E 0.2) | 21.5 |

  So psi_ag 20 needs g_s at or below about 0.01, and 0.006 is the sourced value inside that.

- The other goods at g_s 0.006: the psi limits are 2.1 (oil), 22.3 (coal) and 26.5 (chemicals).
  The approved 1.0, 3.571 and 5.714 sit at 48%, 16% and 22% of them, so no change is needed.
- `ceteris calibrate stability --storage`, kappa1 limits at g_s 0.006 on the fixture (default /
  frictionless): agriculture 0.168 / 0.151 (it was 0.054 / 0.170), oil 0.103 / 0.096, coal
  0.102 / 0.095, chemicals 0.424 / 0.295. The default kappa1 0.05 is at most 0.52 of any of
  these. The worst is oil frictionless, on the -1 side, which the per-market cap already bounds.
- Fixture world (`small_perturbation_decays`, with the fixture export and GE reference
  regenerated with storage on): it **passes**, decaying 0.991 a tick frictionless and 0.992 in
  the default mode. At the approved values it reproduces the B7 stop.

Alternative, if g_s should stay at 0.08: psi_ag 7. The fixture also passes (0.9925 / 0.9923),
but the stock-persistence target is lost (below) and the overshoot is larger.

## Why it fails at 0.08 (derivation)

det(I - M) of the storage loop's 4 x 4 map comes out to c eta g_s kappa psi (E + eps) / s,
which is positive for any positive parameters. So no real root ever crosses +1: there is no
monotone bubble root. The failure is a slow complex pair, a stock-price cycle driven by the
momentum, crossing the unit circle. The loop gain kappa rho psi is 0.99 for agriculture.
Lowering g_s weakens that feedback without touching the stock pull psi c. Lowering psi weakens
both. The determinant also shows that at psi = 0 or g_s = 0 the loop has a unit root, as B7
found.

## Checking the psi derivation (stock persistence)

storage-params.md takes the stock-gap decay rate to be lambda = psi c / s. As built, the order
is filled through the price, and the quasi-static decay is

lambda = delta + (E + eps/rho) rho (psi c - delta s) / (s (rho E (1 + psi c) + eps)),

which is about psi c / (s (1 + psi c)) in elastic markets and psi c / s as E goes to 0. The
eigenvalues agree (psi 20: formula 0.893, eigenvalue 0.868 at E 8.7). For agriculture at
psi 20, lambda is 0.11 to 0.13 against the GHR target of 0.14. That is close, so psi 20 stays.
In inelastic markets the stock-price pair is complex, with modulus about 0.96, which is slower.
Hitting 0.14 exactly would take psi of about 33, which is beyond the margin. At psi 7, lambda
is about 0.04, a stock AR(1) of 0.96 against 0.82 to 0.90 in the data.

## Stockout and smoothing claims (toy agriculture market, `harvest` of the trade tests)

| set | 30% cut x12: peak ln p | lowest S/S* | highest S/S* after | lowest ln p after | ln p at month 60 | 30% surplus: peak |
|---|---|---|---|---|---|---|
| storage off (stock rule) | 0.206 | 0.037 | 1.36 | 0.000 | 0.050 | 0.122 |
| approved (psi 20, g_s 0.08) | 0.733 | 0.296 | 4.68 | -0.765 | -0.312 | 0.342 |
| **psi 20, g_s 0.006** | 0.641 | 0.234 | 2.86 | -0.225 | 0.008 | 0.198 |
| psi 20, g_s 0.02 | 0.654 | 0.246 | 3.17 | -0.343 | 0.015 | 0.230 |
| psi 7, g_s 0.08 | 0.528 | 0.149 | 2.76 | -0.094 | 0.067 | 0.151 |

- Asymmetry: it holds in every set.
- "Stocks hit zero only on the cut side": **this fails for every set.** Even a 24-month cut stops
  at 0.23 of target at g_s 0.006.
- Smoothing: **storage amplifies the harvest shock** (peak 0.53 to 0.73 against 0.21 without
  storage). In Deaton and Laroque (1992) and Williams and Wright (1991), storage damps it. The
  cause is in x. Nothing in it says that a high price now means a lower price later. Both of its
  terms push merchants to buy as the stock falls: the adaptive pi^e follows the rise, and the
  convenience yield rises as cover falls. No psi or g_s value changes that sign. g_s 0.006 does
  remove most of the overshoot after the shock (the stock peaks at 2.9 times target instead of
  4.7, and the price is back by month 60).
- Getting the smoothing and the stockout needs ADR 0091 option (c), a mean-reverting term in
  the expectation, which is a new ADR. This is separate from stability.

## New finding: V1 fails for a different reason than B7 thought

`v1_fixture_frictionless_matches_reference` fails at 1,200 ticks for every set tested,
including those that pass the perturbation test. The gap is 1.5% to 1.65% on prices, mostly
coal; at g_s 0.006 it is 1.65%. Run to 3,600 ticks (g_s 0.006), prices come within 0.06% and
wages within 0.008%, with flows at 1.5%. So this is slow convergence from the data state (about
0.9986 a tick), not instability, and not the momentum loop. Tripling psi for goods 1, 2 and 5
halves the price gap (0.8%), but flows are still 6% off. Before storage is turned on, the owner
needs to pick one of two options: a longer V1 horizon for stored goods, or a stronger stock pull
for goods 1, 2 and 5.

## Not done

- No pack world run. The pack-side check is the single-market region over the pack's eps range
  above. A pack run needs B8's pack.
- No change to defaults, the register or the ADR. Approving g_s 0.006 means updating DESIGN 17
  row P146 (value and source) and the trade SPEC tables, then re-running the storage tests,
  with storage still off by default until V1 is settled.

## Sources

- Milani, F. (2007), JME 54(7) 2065-2082; WP https://econwpa.ub.uni-muenchen.de/econ-wp/mac/papers/0510/0510022.pdf
  ("The posterior mean estimate for the gain equals 0.0183", quarterly US data).
- Gorton, Hayashi and Rouwenhorst (2013), as cited in storage-params.md (stock persistence
  target, the 12-month window).
- Single-market scans: scratchpad scan.py, run2.py to run5.py (numpy copy of
  `storage_matrix`, checked against the B7 numbers: radius 1.0032 at E 0.6, pack eps).
