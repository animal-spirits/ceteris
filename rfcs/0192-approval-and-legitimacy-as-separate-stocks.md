# 0192. Approval and legitimacy as separate stocks

Status: proposed
Date: 2026-10-28
Design sections: 10.1, 10.5, 10.7, 10.8, 11.5, 13.3; ADRs 0046, 0056, 0065, 0066, 0068, 0071, 0139, 0189, 0191
Phase: P6 (2.0-alpha)

## Context

Section 10.7 lists what enters pop approval (material conditions against expectations, policy
distance, exclusion, corruption, war weariness, repression) but gives no equation, no dynamics
and no calibration target, and 10.8 uses the same quantity for protest and regime change. The
literature separates two things that move on different clocks (Easton 1965). Specific support
for the incumbent moves in months: it follows the economy (Mueller 1970; Nannestad and Paldam
1994), more so where responsibility is clear (Powell and Whitten 1993; Duch and Stevenson 2008),
decays from a honeymoon, carries a cost of ruling (Paldam and Skott 1995), and jumps in rallies
(Mueller 1970; Baker and Oneal 2001). Diffuse support for the regime moves in years: losers in
democracies keep consenting to the system (Anderson et al. 2005), while autocracies with no
electoral outlet absorb discontent into the regime itself. One stock cannot do both: either
unpopular democratic governments trigger regime crises, or autocracies never do.

## Options

1. Status quo: one approval stock for elections and unrest.
2. Two stocks per pop: approval of the incumbent, and legitimacy of the regime fed by approval
   at a rate set by electoral competition.

## Decision

Option 2.

**Approval target**, per pop and incumbent:

```
A*_i = logistic( a0 + g PID_i,inc + b_e kappa_c dP_i - b_d D_i - b_k K_i
                 - b_w log(1 + casualties_c) - b_r R_i - b_x X_i - b_h tenure_c )
```

- PID: attachment to the governing party or parties; it also biases how partisans read the
  economy (Bartels 2002; Evans and Andersen 2006).
- dP: perceived performance, the change in the pop's life evaluation (ADR 0191) and in national
  unemployment, inflation and growth against expectation (ADR 0139 kernel); losses weigh about
  twice gains (Nannestad and Paldam 1997; Soroka 2006). b_e is register entry P33.
- kappa_c: clarity of responsibility, lower for coalitions, cohabitation, minority governments,
  federal states and very open economies (Hellwig 2001; Duch and Stevenson 2008).
- D: salience-weighted distance from the pop's ideal point to enacted policy (10.6).
- K: perceived corruption after the information filter of ADRs 0046 and 0071 (Ferraz and Finan
  2008). R and X: repression felt and group exclusion (EPR). Casualties: Mueller (1973); Gartner
  and Segura (1998).

**Approval dynamics**, monthly: `A_t = A_t-1 + lambda (A*_t - A_t-1) + H_t + Q_t`. H is a
honeymoon bonus at a change of government that decays over the first year. Q is a rally after
an attack on the state, a war onset or a pandemic onset, with a jump drawn from a keyed stream
per (state, event) and a half-life of a few months, smaller where polarization is high
(Hetherington and Nelson 2003; Bol et al. 2021).

**Legitimacy**, per pop and regime, monthly with a slow rate mu:

```
G_t+1 = G_t + mu [ (1 - omega_c) mean5y(A_i) + omega_c mean5y(A_system) - G_t ]
        + phi (fairness_c - fairness_ref) - rho repression_shock_i
```

omega_c is electoral competition from the institution vector: where governments can be voted
out, dislike of the incumbent mostly stays with the incumbent. Fairness combines clean
elections, rule of law and corruption from 10.1.

**Uses.** Approval drives vote intention (ADR 0193), party discipline and partner exit (ADR
0194), snap elections and ADR 0066's leader hazards in democracies. Legitimacy replaces approval
in the 10.8 protest threshold, in the security forces' defection calculus for coups, and in the
trust and compliance links of ADR 0056. Group, organization (ADR 0189) and party approval are
membership-weighted means of pop approval; there is no separate group happiness.

**Autocracies.** The same equations run (Treisman 2011). Reported approval differs from true
approval by a falsification bias rising with repression (Kuran 1991; Frye et al. 2017), which
lives in the observed layer of ADR 0065, not in the state.

**Parameters.** Free: lambda, honeymoon size and decay, rally scale, mu (four or five). P33 is
calibrated; clarity loadings come from published estimates.

## Migration

- P6, in ceteris-polity, after ADR 0191.
- Data: Executive Approval Project quarterly series (Carlin, Hartlyn, Hellwig, Love,
  Martinez-Gallardo and Singer; terms to audit, ADR 0050); Eurobarometer and Latinobarometro
  trust in government; Claassen (2020) latent support for democracy; V-Dem; WVS confidence in
  institutions (derived parameters only).

## Tests

- Approval and legitimacy stay in [0, 1].
- With omega at zero, legitimacy equals five-year mean approval.
- With rally, honeymoon, clarity and the life-evaluation term off, approval reproduces the 10.7
  baseline used by the P6 golden hash.
- Calibrated on 2000-2010, simulated approval paths 2011-2024 show honeymoon decay, crisis drops
  and rally sizes within tolerance of the Executive Approval Project, including 9/11, Crimea 2014
  and March 2020 as in-sample and holdout patterns.
- A democracy with an unpopular government shows a lower protest hazard than an autocracy with
  the same approval path.

## Consequences

Governments can fall without regimes falling, and regimes without an electoral outlet carry
their discontent until it breaks. The cost is two floats per pop, negligible compute, and about
3-4 part-time weeks.
