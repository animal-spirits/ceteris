# Econ numbers against the literature, 4 November 2026

Sources: `specs/econ/SPEC.md` (P2 gate Measured, Benchmark, Parameters; main and pre-p3/audit),
`specs/trade/SPEC.md` (stability table), ADR 0200 As built (benchmark on the k263 scratch pack),
memory notes for the 28 Oct benchmark and the 30 Oct fidelity audit (marked "memory"; not
re-measured). Nothing was rerun today; the C7 rerun on the final pack will replace the
benchmark rows.

Legend: **Agrees**, **Disagrees**, **Agrees by construction** (the value is borrowed or fitted to
the target, so agreement tests nothing), **Untested**.

## 1. Trade

| Number | Model | Literature / data | Verdict |
| --- | --- | --- | --- |
| Trade elasticity per class (P01) | 2.91, 4.14, 7.68, 16.91 (harmonic class means of the OECD key) | Simonovska-Waugh ~4; Head-Mayer median ~5; Caliendo-Parro sector range ~1 to 50 (petroleum and mining highest) | Agrees by construction |
| Theta estimated directly on BACI by PPML (memory, 30 Oct audit) | 10.9 to 14.1 for every class | Should spread like the classes above | Disagrees: the data do not reproduce the class spread; the borrowed values are what we run on |
| H2 per class (model recovers its own theta) | within 2e-8 relative | n/a | Passes, but it checks the code, not the literature (ADR 0264 made it the gate) |
| H2 distance elasticity | -0.454 (s.e. 0.011, B8 pack); was -0.753 at the P2 gate, then -0.536, -0.509, -0.463 | Data -1.1 (SPEC's own target); Head-Mayer meta-analysis ~-0.9 to -1.1 | **Disagrees**, and each fix moved it further away. Cause in SPEC: with iceberg legs each class's coefficient is about -0.09 theta_c against about -0.8 in the data. Now only reported (0264); log-distance crossing friction (0272) is the fix in progress |
| Openness, foreign share by theta class vs ICIO 2000 | 1.11, 1.04, 1.13, 1.00 (k263); 1.151, 1.043, 1.144, 0.973 (B8, memory) | ICIO 2000 = 1 | Agrees by construction: border nu is fitted to the ICIO cross-border share. On 28 Oct, before the fit, openness was ~3x the data (goods exports 45% of income vs ICIO 14%) |
| Short-run vs long-run trade elasticity | Long-run theta applied every month | Boehm, Levchenko and Pandalai-Nayar (2023): about 0.8 in the short run, about 2 in the long run | **Disagrees**: the monthly response is too elastic. ADR 0027 partial adjustment is the fix, still proposed and off |
| Distance elasticity of air and sea mode costs (P04) | Fit never converged; borrowed ratios (0269) | Hummels and others | Agrees by construction |

## 2. Welfare and counterfactuals

| Number | Model | Literature | Verdict |
| --- | --- | --- | --- |
| ACR with input-output links, border tau x1.1 (B8 pack, country form) | median model/ACR 1.077 (p10 0.93, p90 1.33), correlation 0.862, 0 wrong signs out of 80 | Costinot and Rodriguez-Clare (2014): welfare = ACR-IO, so median 1 and correlation near 1; our bar is median in [0.8, 1.25] and correlation >= 0.9 | Median agrees; **correlation misses** (0.862 < 0.9). The province-graded form (0268) is not yet measured on the final pack (C7) |
| Plain ACR, no IO | median 2.42, correlation 0.855 | With Leontief cost share a ~0.55, IO amplifies by ~1/(1-a) ~2.2 | Agrees: the gap is the IO multiplier the literature predicts |
| Hard Brexit, UK welfare | -0.63% (k263); was -2.2% on 28 Oct | Dhingra et al. (2017) static: -2.66% | **Disagrees**: about 4x too small since buyers pay the EK index (0200) and nu was fitted. Tariffs plus iceberg only; Dhingra also include non-tariff barriers, so part of the gap is the shock file |
| Soft Brexit, UK welfare | -0.24% (k263); was -0.95% | Dhingra et al.: -1.34% | **Disagrees**, about 5x too small |
| Brexit, EU27 | -0.04% soft, -0.11% hard | Small negative (Dhingra; Felbermayr et al.) | Agrees in sign and order; on 28 Oct the EU gained, now fixed |
| Japan under costlier borders | -0.53% (was +0.6%) | Must lose | Agrees since 0200 |
| US-China 2018 tariffs (28 Oct, memory; not rerun since 0200) | pass-through 0.97, US welfare -0.04% | Amiti-Redding-Weinstein, Fajgelbaum et al.: near-complete pass-through, US loss ~-0.04% | Agreed on 28 Oct; **untested** on current code |
| x10 border shock, autarky gains | Run panics (i64 overflow in write-off) or stops at tick 45 | ACR gains from trade are a few percent to tens of percent | **Untested / fails**: no autarky number exists; deferred to P5 |

## 3. Prices, wages, production, demand

| Number | Model | Literature | Verdict |
| --- | --- | --- | --- |
| Price levels 2000 (PWT vs WDI) | 31 of 31 ICP benchmark economies within 10% | ICP 1996/2005 benchmarks | Agrees (data check). Interpolated and extrapolated economies disagree by 12.7% and 50.7% median; not gated |
| PIGL eps, gamma, sigma_B, weights | 0.71, 0.71, 0.26, (0.634, 0.082, 0.284) | Alder, Boppart and Mueller (2022) | Agrees by construction (their estimates) |
| Goods substitution within a category (P06) | 0.6 | 0.3 to 0.9 | Agrees, untested in any moment |
| F-L substitution sigma_FL (P08) | 0.8 | Oberfield-Raval ~0.7; Chirinko 0.4 to 0.6 | Agrees, at the high end |
| Education efficiency (P42) | secondary 1.4 to 1.6, tertiary 2.0 to 2.6 | Mincer returns 8 to 12% a year | Agrees by construction |
| Price speed kappa1 | 0.05 per month (design 0.2) | Set by stability limits (0.11 to 0.47), not by price data | Untested; no micro-price moment (e.g. Nakamura-Steinsson frequencies) is used |
| Inventory term kappa2 | 0 (design 0.1) | Inventories untargeted, unit root in the stock | Departs from the design; untested |
| Wage speed kappa_w 0.1, rigidity 0.5, employment speed eta 0.3 | Free parameters | No moment yet (Phillips-curve slope, Okun) | Untested |
| Storable clip 0.3 a month | 0.3 | Oil fell ~30% in a month in late 2008 | Agrees with the one episode cited |
| Steady state in 2000 | Data state = steady state (0089) | 2000 is not a steady state | **Disagrees** with reality by design; ADR 0103's drift and P4's balanced growth (0243) are the corrections |

## 4. Distribution and growth (audit branch)

| Number | Model | Literature / data | Verdict |
| --- | --- | --- | --- |
| US top-10% wealth share | 0.638 | Fed DFA 2000Q1 0.638 | Agrees by construction (tau is fitted to it) |
| Other countries' top-10% wealth and income vs WID | Report written (#143), table not run | WID shweal, sptinc | Untested until the C6 Mac run |
| Drift half-life (P153) | 7 years (K/Y gap closing) | Barro cross-country convergence ~2% a year (half-life ~35 y); panel estimates 6 to 10% (7 to 12 y); Solow-implied ~4 to 6% (12 to 17 y) | Fast end of the range; registered as fitted on the 2000-2010 holdout, not yet fitted |
| Sector labour shares | WIOD SEA where cached | SEA | Agrees by construction; coverage incomplete until the SEA fetch |

## 5. Summary

- **Agree on evidence:** plain-vs-IO ACR ratio, ACR-IO median, signs of welfare effects
  (Japan, EU), 2000 price levels of the benchmark economies, US-China pass-through (as of
  28 Oct).
- **Agree only by construction:** thetas, PIGL, openness, US wealth share, labour shares,
  education returns, mode costs. These are borrowed or fitted to the very target.
- **Disagree:** distance elasticity (-0.45 vs about -1.1, worsening), Brexit magnitudes (4 to
  5x too small), ACR-IO correlation (0.86 vs 0.9 bar), theta spread on BACI, monthly use of
  long-run theta, no autarky result.
- **Untested:** price, wage and employment speeds, x10, WID shares for other countries, the
  drift half-life.

Main risk: the trade block now hits its fitted targets (openness, own theta) while the free
moments (distance, Brexit size) moved away from the literature. That pattern points to the
crossing friction doing the work that distance should do. The fix in progress is ADR 0272,
and C7 is the check.
