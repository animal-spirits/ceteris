# 0023. Trade elasticity classes

Status: accepted (agent default under 22.2, MIGRATION Task 5); implemented on p2-econ (MIGRATION Task 5)
Date: 2026-10-24
Design sections: 8.3, 17.1 (P01), 18 (H2), 22.2; ADR 0018

## Context

ADR 0018 makes P01 a theta per traded good, grouped into 3 to 5 classes that share kernels, with
starting values from Caliendo and Parro (2015) mapped to the P2 goods by 2000 BACI trade weights.
It leaves the class boundaries to an agent default. This record fixes the source values, the
mapping, the rule that draws the boundaries, and the resulting P01 values.

### Source values

Caliendo, L. and F. Parro (2015), "Estimates of the Trade and Welfare Effects of NAFTA", Review of
Economic Studies 82(1), 1-44. Transcribed from the Advance Access version (published 14 November
2014; the PDF on the first author's Yale page): Table 1, "Dispersion-of-productivity estimates",
p. 18, the 99% sample columns, theta and heteroskedasticity-robust standard error; sectors and ISIC
Rev 3 codes from Table A.1, p. 42. The 99% sample is the paper's benchmark (p. 19), the column its
counterfactuals use (p. 20, Table 1 column 5).

| # | Sector | ISIC Rev 3 | theta | s.e. |
|---|---|---|---|---|
| 1 | Agriculture | 1-5 | 9.11 | (2.01) |
| 2 | Mining | 10-14 | 13.53 | (3.67) |
| 3 | Food | 15-16 | 2.62 | (0.61) |
| 4 | Textile | 17-19 | 8.10 | (1.28) |
| 5 | Wood | 20 | 11.50 | (2.87) |
| 6 | Paper | 21-22 | 16.52 | (2.65) |
| 7 | Petroleum | 23 | 64.85 | (15.61) |
| 8 | Chemicals | 24 | 3.13 | (1.78) |
| 9 | Plastic | 25 | 1.67 | (2.23) |
| 10 | Minerals | 26 | 2.41 | (1.60) |
| 11 | Basic metals | 27 | 3.28 | (2.51) |
| 12 | Metal products | 28 | 6.99 | (2.12) |
| 13 | Machinery n.e.c. | 29 | 1.45 | (2.80) |
| 14 | Office | 30 | 12.95 | (4.53) |
| 15 | Electrical | 31 | 12.91 | (1.64) |
| 16 | Communication | 32 | 3.95 | (1.77) |
| 17 | Medical | 33 | 8.71 | (1.56) |
| 18 | Auto | 34 | 1.84 | (0.92) |
| 19 | Other Transport | 351-359 | 0.39 | (1.08) |
| 20 | Other | 36-37 | 3.98 | (1.08) |

The paper replaces Basic metals, Machinery n.e.c. and Auto, which change sign in the 97.5% sample,
by "the mean estimate for the manufacturing sector" (p. 19) without printing that mean. The
authors' replication files (`Data_and_Codes_CP.zip`, `Equilibrium/T.txt`) instead keep Basic metals
and Machinery and set Auto and Other Transport to 8.22. Neither replacement is in the table, so
the table is used as printed. With the replication vector the goods change as noted below.

### Mapping to the P2 goods

Each HS 1992 6-digit line takes the estimate of its ISIC Rev 3 division (WITS HS 1988/92 to ISIC
Rev 3 concordance) and its good shares from the Task 13 concordance (`goods.hs_goods_shares`).
Theta of good g is the arithmetic mean of the line estimates weighted by share times the line's
2000 BACI trade value (all flows, v in thousands of USD), `theta.caliendo_parro_by_good`. Every
traded good's value is covered; 0.6% of world trade sits on lines outside the concordance.

| Good | theta | 2000 trade, USD bn |
|---|---|---|
| 0 Agriculture | 9.11 | 187 |
| 1 Crude oil and gas | 13.53 | 444 |
| 2 Coal and other mining | 13.53 | 101 |
| 3 Food | 2.62 | 315 |
| 4 Light manufacturing | 6.73 | 1,563 |
| 5 Chemicals, refined fuels and metals | 12.64 | 1,276 |
| 6 Machinery, electronics and vehicles | 5.33 | 2,421 |

Goods 1 and 2 both lie in Mining. Good 5 mixes Petroleum (64.85) with Chemicals, Plastic, Minerals
and Basic metals (1.67 to 3.28); good 6 mixes Office and Electrical (about 13) with Machinery n.e.c.,
Auto and Other Transport (below 2). The full-sample column gives 8.11, 15.72, 15.72, 2.55, 5.65,
12.31, 5.45; the replication vector 9.11, 13.53, 13.53, 2.62, 7.33, 12.64, 7.29.

## Options

(1) One class per good: 7 classes, above the 5 that ADR 0018 and the 19.2 budget allow. (2) Fixed
cut points on theta (for example 4 and 8): simple, but blind to where the values cluster. (3) The
fewest classes of goods adjacent in theta whose thetas lie within a fixed factor.

## Decision

Option 3. Sort the seven traded thetas (equivalently their logs) and split them into the fewest
classes, 3 to 5, of consecutive goods whose largest theta is at most 1.5 times their smallest
(spread in log theta at most ln 1.5). Among splits with that count, take the one with the smallest
largest spread, then the first in order. If 5 classes cannot meet the bound, the rule fails and the
owner decides. Class theta is the mean of its goods' thetas weighted by their 2000 trade value;
classes are numbered from the lowest theta. `theta.classes` implements the rule.

Result, stored to two decimals (the precision of the source) in `TradeParams::default()`:

| Class | Goods | theta |
|---|---|---|
| 0 | 3 Food | 2.62 |
| 1 | 4 Light manufacturing, 6 Machinery, electronics and vehicles | 5.88 |
| 2 | 0 Agriculture, 1 Crude oil and gas, 2 Coal and other mining, 5 Chemicals, refined fuels and metals | 12.55 |

Class 2 spans 9.11 to 13.53, a factor 1.485. The full-sample column and the replication vector
give the same classes except that Agriculture moves to class 1 (class values 2.55, 5.64, 13.33 and
2.62, 7.39, 12.91).

## Consequences

- The P01 prior widens from 4 to 8 to the range of the class values, 2.62 to 12.55
  (`specs/trade/SPEC.md`, P01).
- Kernels stay per good in P2 (weight classes are goods, ADR 0018); `TradeParams.theta_class`
  carries the map so goods of one class can share kernels once weight classes merge.
- H2 per class (`test_h2_theta_by_class`, PLAN Task 20) estimates each class's theta by PPML on
  simulated flows (`ppml.trade_elasticity`).
- At theta 12.55 the sourcing weight floor (1e-100) drops origins priced 1e8 times above the best,
  against 1e20 at theta 5.
- Direct estimation on BACI (ADR 0022) replaces these values and may redraw the classes by the
  same rule.
- Amended by ADR 0050 (2026-10-25): the WITS and UNSD concordance gave way to the OECD BTiGE HS to CPA
  key, through which the same rule draws four classes (2.62, 5.19, 9.94, 13.53). The values above stay
  in `TradeParams::default()` until the owner decides.
