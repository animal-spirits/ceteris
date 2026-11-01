# 0202. Firm entry, exit and the zombie band

Status: proposed
Date: 2026-10-28
Design sections: 7.2, 7.4, 15.4, 15.5, 17.1; ADRs 0026, 0095, 0172
Phase: with the quarterly investment step (7.4)

## Context

ADR 0026 makes the quarterly step "free entry and exit in the mass of firms" without a rule.
DESIGN 7.4 exits a whole cell after persistent losses with negative equity, so exit is all or
nothing. ADR 0172 lets banks roll over loans to insolvent borrowers, but in a cell with one
enterprise that keeps the whole cell alive or none of it. Zombie firms are a share of firms:
about 2% of listed firms in 14 advanced economies in the late 1980s and about 12% in 2016 on
the broad measure (Banerjee and Hofmann 2018), and far more in Japan around 2000 (Caballero,
Hoshi and Kashyap 2008).

The register entry for sigma_g (P50) has a literature prior but no identifying moment. With
k = theta (sigma - 1) / sigma (0095), the Pareto exponent of firm sales is
k / (sigma - 1) = theta / sigma. Firm sales are close to Zipf, with an exponent near 1.05
(Axtell 2001).
Deep dive: https://claude.ai/artifact/J7jLsKygA6qyLfLmvNXhNa

## Options

1. Whole-cell exit (7.4 as written).
2. Hopenhayn-style entry and exit in the Pareto mass, with forbearance as a lowered exit cutoff.
3. Explicit firm cohorts.

## Decision

Option 2. Option 3 is an extended-profile follow-up (startup cohorts after 2008, Sedlacek and
Sterk 2017), reusing the putty-clay vintage arrays.

Quarterly, per cell:

```
entry   E = eta_e M max(0, V_e / (w f_e) - 1)          V_e: expected value of a draw from the tail
exit    X = delta M + M [1 - (phi*_old / phi*_new)^k]+   death plus cutoff exit
zombie  phi_z = phi*_dom (1 - z_b)                       z_b from the bank capital gap (0172)
        zombie share = 1 - (phi_z / phi*_dom)^k
```

- Zombies hold inputs and labour and earn below cost; their losses sit on bank balance sheets as
  0172 states. The whole-cell exit of 7.4 is the limit of this rule when the cutoff passes the
  top of the tail.
- sigma_g (P50) is set per theta class by sigma = theta / zeta with zeta = 1.05 (Zipf), checked
  against markup levels. This gives about 2.5, 4.9 and 10.1 for the three classes, so P50's range
  of 3 to 8 widens to 2 to 11. The first class sits close to k = sigma - 1 and is checked first.
- f_e (P52) is identified by firms per worker by sector (OECD SDBS, Eurostat SBS, US SUSB, around
  2000), with World Bank Doing Business start-up costs as a prior.
- delta is set from published firm death rates (US BDS, OECD DynEmp); eta_e from the 2008-2010
  start-up rate response.

## Tests

- In steady state with free entry, M scales with cell labour and is independent of trade costs
  (equilibrium nesting).
- A rise in the domestic cutoff removes exactly the share 1 - (phi*_old / phi*_new)^k.
- With z_b = 0, no zombies; a toy undercapitalized bank produces a zombie share that falls when
  its capital is restored.
- With entry and exit off, hashes equal the fixed-mass path.

## Consequences

Firm counts, entry, exit and the zombie share become outputs with published targets. Zombie
congestion lowers cell productivity without a new mechanism beyond 0172. Register: 2 entries
(delta, eta_e); P50 gets an identifying rule and a wider range.
