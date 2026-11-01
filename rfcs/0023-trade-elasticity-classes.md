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

Each HS 1992 6-digit line takes one CPA 2.1 activity from the OECD BTiGE key, adapted locally (gold
to basic metals, books and recorded media to printing; ADR 0050 amendment), which fixes its good
(`goods.hs_goods_shares`) and, through the ISIC Rev 3 division of that activity (`theta.REV3_OF_CPA`),
its Caliendo-Parro estimate. Theta of good g is the arithmetic mean of the line estimates weighted by
the line's 2000 BACI trade value (all flows, v in thousands of USD), `theta.caliendo_parro_by_good`.

The values below were drawn on 2026-10-24 through the route the key replaced: the ISIC Rev 3 division
of the WITS HS 1988/92 to ISIC Rev 3 concordance, and good shares split equally over the ISIC Rev 4
divisions the UNSD Rev 3.1 to Rev 4 table links (0.6% of world trade sat on lines outside it). The
adapted key gives 9.11, 13.53, 13.53, 2.62, 9.15, 10.57, 5.19 (amendment below).

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

Result, stored to two decimals (the precision of the source) in `TradeParams::default()`, on the
thetas of the OECD key with its adaptations (amendment below):

| Class | Goods | theta |
|---|---|---|
| 0 | 3 Food | 2.62 |
| 1 | 6 Machinery, electronics and vehicles | 5.19 |
| 2 | 0 Agriculture, 1 Crude oil and gas, 2 Coal and other mining, 4 Light manufacturing, 5 Chemicals, refined fuels and metals | 10.57 |

Class 2 spans 9.11 to 13.53, a factor 1.485. On the original WITS and UNSD thetas the rule drew
2.62 (food), 5.88 (goods 4 and 6) and 12.55 (goods 0, 1, 2, 5), the values of the first
implementation. On that route the full-sample column and the replication vector gave the same classes
except that Agriculture moved to class 1 (class values 2.55, 5.64, 13.33 and 2.62, 7.39, 12.91).

### Prior band (H2 per class, PLAN Task 20)

ADR 0018 asks that PPML on simulated flows recover each class's theta "within its prior band" and
defines none. The band, fixed before the gate ran (controller ruling, 2026-10-26): theta_c plus or
minus 2 standard errors of the class's estimate. A class's theta is the mean of the Caliendo-Parro
sector estimates weighted by the class's 2000 BACI trade value per sector (the weights of the rule
above, `theta.sector_values`); its standard error is that of the same weighted mean of independent
sector estimates, sqrt(sum_s w_s^2 se_s^2), with the robust standard errors of the table above
(`theta.class_bands`). The sectors are estimated in separate regressions, hence independence; with
perfectly correlated errors the band would be wider.

| Class | theta | s.e. | Band (theta +/- 2 s.e.) |
|---|---|---|---|
| 0 Food | 2.62 | 0.610 | 1.40 to 3.84 |
| 1 Machinery, electronics and vehicles | 5.19 | 0.917 | 3.36 to 7.02 |
| 2 Agriculture, mining, light manufacturing, chemicals and metals | 10.57 | 1.211 | 8.15 to 12.99 |

The band is not widened after a result (Task 20 stop rule): a miss goes to the owner.

## Consequences

- The P01 prior widens from 4 to 8 to the range of the class values, 2.62 to 10.57 (12.55 on
  the original thetas; `specs/trade/SPEC.md`, P01).
- Kernels stay per good in P2 (weight classes are goods, ADR 0018); `TradeParams.theta_class`
  carries the map so goods of one class can share kernels once weight classes merge.
- H2 per class (`test_h2_theta_by_class`, PLAN Task 20) estimates each class's theta by PPML on
  simulated flows (`ppml.trade_elasticity`).
- At theta 10.57 the sourcing weight floor (1e-100) drops origins priced 3e9 times above the best
  (1e8 at 12.55), against 1e20 at theta 5.
- Direct estimation on BACI (ADR 0022) replaces these values and may redraw the classes by the
  same rule.
- Amended by ADR 0050 (2026-10-25): the WITS and UNSD concordance gave way to the OECD BTiGE HS to CPA
  key. Through the unadapted key the rule drew four classes (2.62, 5.19, 9.94, 13.53), and the owner
  adopted the key's classes on 2026-10-25. With the local adaptations (gold to basic metals, books
  and recorded media to printing) light manufacturing rises to 9.15 and chemicals and metals falls to
  10.57, so the rule draws three classes again: 2.62 (food), 5.19 (machinery, electronics and
  vehicles) and 10.57 (agriculture, crude oil and gas, coal and other mining, light manufacturing,
  chemicals, refined fuels and metals; 9.11 to 13.53, a factor 1.485). `TradeParams::default()`
  holds the adopted classes since the theta task (2026-10-25), and `test_rust_theta_defaults_follow_the_estimator`
  ties the fixture export of those defaults to `theta.classes`.
- Amended by ADR 0101 (owner, 2026-10-26): class theta becomes the trade-weighted harmonic mean
  over the HS lines of the class, (sum_l w_l / theta_l)^-1, inside the same class rule. It is
  computed after ADR 0092 moves refined petroleum (C19) into good 1, and ADR 0092's nu_c fit then
  runs at the new values. Tariff experiments report a theta-scale lane {0.6, 1, 1.4} until the
  ADR 0022 estimates replace the literature values. Built on 2026-10-29 with the replication
  vector (owner ruling); see Amendment (2026-10-29) below.

## Amendment (2026-10-29): harmonic class means on the replication vector

Owner ruling (2026-10-29, option B of the first attempt at the pre-P3 task). ADRs 0092 and 0101
are applied together: refined petroleum (CPA C19, ISIC Rev 3 division 23, Caliendo-Parro
Petroleum) moves from good 5 into good 1, and every mean becomes the trade-weighted harmonic
mean over HS lines.

**Source column.** The sector thetas are those of the authors' replication vector
(`Equilibrium/T.txt`): the printed table above, with Auto and Other Transport set to 8.22. Basic
metals and Machinery n.e.c. keep their printed values, as the vector does. Reason: on the printed
column, the harmonic mean of good 6 is 1.90, and 38% of its 1/theta mass comes from Other
Transport, theta 0.39 with a standard error of 1.08. That estimate is not distinguishable from
zero, and the paper itself reports it as changing sign in the 97.5% sample. A harmonic mean
weights the smallest values most, so one insignificant estimate set the class. The printed table
stays above as transcribed; the substitution is `theta::REPLICATION` in the code.

**Rule.** Theta of good g is (sum_l w_l / theta_l)^-1 over the good's HS lines l, with w_l the
line's share of the good's 2000 BACI trade value and theta_l the replication-vector theta of the
line's sector. The class rule above is unchanged: the fewest classes, 3 to 5, of goods adjacent
in theta within a factor 1.5. Class theta is the harmonic mean over the class's lines, which equals
the harmonic mean of its goods' thetas weighted by their trade value. Values are stored to two
decimals in `TradeParams::default()`.

Good thetas on this rule, goods 0 to 6: 9.11, 17.64, 13.53, 2.62, 7.46, 3.00, 4.14. Classes:

| Class | Goods | 2000 trade, USD bn | theta harmonic | theta arithmetic |
|---|---|---|---|---|
| 0 | 3 Food, 5 Chemicals and metals | 1,741 | 2.91 | 3.23 |
| 1 | 6 Machinery, electronics and vehicles | 2,891 | 4.14 | 7.20 |
| 2 | 0 Agriculture, 4 Light manufacturing | 1,130 | 7.68 | 9.15 |
| 3 | 1 Crude oil, gas and refined fuels, 2 Coal and other mining | 734 | 16.91 | 26.51 |

The arithmetic column is the trade-weighted arithmetic mean over the same lines, on the same
vector. It is recorded in `econ_goods.arrow` (`theta_arithmetic`, next to `theta_harmonic`) and
is not used by the model. No split into 3 classes keeps the bound: food at 2.62 and machinery at
4.14 differ by a factor 1.58. Among the splits into 4, the one above has the smallest largest
spread, 1.30 (class 3).

Outcomes against ADRs 0092 and 0101, as expected there:

- Four classes, not three. P05 (ADR 0092) becomes four values, one per class.
- Good 5 leaves the top class for class 0 at 2.91, as ADR 0092 expected ("about 3"), but class 0
  is food and good 5 together, and machinery is class 1 on its own.
- Good 6 is 4.14, against "roughly 3" in ADR 0101. Machinery n.e.c. (1.45, 18% of the good's
  trade value) pulls it below the arithmetic 7.20.
- Good 1 rises to 17.64 once refined petroleum (64.85) joins crude oil and gas (13.53), and class
  3 is at 16.91. At that theta the sourcing weight floor (1e-100) drops origins priced about
  8e5 times above the best.
- Good 5 keeps its index and is renamed "Chemicals and metals", since refined fuels left it.

The P01 prior becomes 2.91 to 16.91, the range of the class values.

**Prior band (H2 per class).** The band stays theta_c plus or minus 2 standard errors. For a
harmonic mean H = (sum_s w_s / theta_s)^-1 of independent sector estimates, the delta method
gives se(H) = H^2 sqrt(sum_s w_s^2 se_s^2 / theta_s^4). The replication vector prints no standard
errors, so Auto and Other Transport keep their printed ones (0.92 and 1.08) at theta 8.22, which
understates their uncertainty.

| Class | theta | s.e. | Band |
|---|---|---|---|
| 0 Food, chemicals and metals | 2.91 | 0.948 | 1.02 to 4.81 |
| 1 Machinery, electronics and vehicles | 4.14 | 4.058 | -3.97 to 12.26 |
| 2 Agriculture, light manufacturing | 7.68 | 0.870 | 5.94 to 9.42 |
| 3 Crude oil, gas and refined fuels, coal and other mining | 16.91 | 4.294 | 8.33 to 25.50 |

Class 1's band does not bind: Machinery n.e.c. (1.45, s.e. 2.80) enters through 1/theta^2. ADR
0092 replaces these bands with plus or minus 2 standard errors of each class's PPML estimate once
its nu fit lands.

**ADR 0101's welfare test.** As first written (a uniform cost shock on a two-line class), the test
holds at any class theta: d ln lambda_l = -theta_l (1 - lambda_l) d ln tau, so theta cancels from
the line welfare -(1/theta_l) d ln lambda_l. The test used instead is a non-uniform shock: line
costs move so that every line's domestic share falls by the same log amount, the setting in which
the welfare change is read from an observed share change. The line-level change is
-sum_l w_l (1/theta_l) d ln lambda and the class-level change is -(1/theta_C) d ln lambda; they
agree only at the harmonic mean. With line thetas 2 and 8 at equal weight, the harmonic 3.2
matches the line sum and the arithmetic 5 misses it by 36%
(`welfare_first_order_at_harmonic_theta`). The response of the class's trade share to a uniform
tariff runs through the weighted arithmetic mean instead, so a class model at the harmonic theta
understates that response when lines keep their own thetas. ADR 0101's statement that the
arithmetic mean overstates trade diversion does not hold for that response.

**Theta scale (P150).** `TradeParams.theta_scale`, default 1, multiplies every good's theta where
the kernel is built. Tariff experiments run it at {0.6, 1, 1.4} (ADR 0101).
