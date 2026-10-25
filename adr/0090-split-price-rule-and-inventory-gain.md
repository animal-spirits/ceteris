# 0090. Split price rule and inventory gain

Status: proposed (owner chose the options, 2026-10-25)
Date: 2026-10-25
Design sections: 8.2, 17 (P02, P03a, P03b); trade SPEC Origin market, Stability region; ADRs 0016, 0019

## Context

The price rule `dlnp = pi^e + kappa1 (D - Q)/Q - kappa2 (S' - S*)/S*` uses one gain for two
modes of very different speed:

- **Relative prices.** A small origin faces an order elasticity E = theta_g + 1, up to 13.55.
  J2 caps kappa1 at 0.14 for the theta-12.55 goods and 0.51 for food, so one global kappa1 makes
  food and manufactures adjust 2 to 4 times slower than they could.
- **Price level.** The world price level faces only household demand (input orders are in
  units, money is fixed) and returns at kappa1 h, with h about 0.24. With pi^e the mode has a
  modulus of 0.994 to 0.996 per tick, a half-life of about 190 months.

With kappa2 = 0 the stock has a unit root (4.7 to 47 times target cover on the fixture), and
P02 has no effect.

## Options

(1) Keep one kappa1. (2) Split the excess-demand term into an elasticity-normalized relative
term and a world aggregate term, each with its own gain.

For the stocks of goods outside competitive storage (ADR 0091): (a) kappa2 = 0; (b) storage
loss only; (c) kappa2 > 0 plus a storage cost.

## Decision

Option 2, with inventory option (c) (owner, 2026-10-25).

With l_o = ln(D_o / Q_o), lbar its value-weighted mean over all markets, E_o = theta_g + 1 for
traded goods and 1 for local goods, and h the steady-state household share of orders:

```
dlnp_o = clip( pi^e + (kappa_r / E_o)(l_o - lbar) + (kappa_n / h) lbar
               - (kappa2 / E_o)(S'_o - S*_o)/S*_o,  -c_max, c_max )
```

- kappa_r (P03a, redefined): start 0.5. For a small origin kappa_r = 1 is one exact Sinkhorn
  step, since orders move with p^-(theta+1).
- kappa_n (P143): start 0.2. The price level returns at kappa_n per tick instead of kappa1 h.
- kappa2 (P03b): normalized like kappa_r; start set inside the re-derived region.
- lbar is one world reduction per tick, summed in a fixed order (determinism).
- h is fixed at its steady-state value, so the rule stays homogeneous of degree zero.

The stock of goods outside ADR 0091 (food 3, light manufacturing 4, machinery 6) also loses a
carrying cost: S' = (1 - delta_g) S + Q - sales, with delta_g the storage cost of P56 (ADR
0029), shared with ADR 0091. The loss is physical (destroyed, tallied as spoilage). Merchants
replace it, so the excess-demand ratio reads l_o = ln((D_o + delta_g S*_o) / Q_o), and at the
steady state output covers sales plus delta_g S* (ADR 0089 stage 1 adds it to V_o). Local
goods keep s_g = 0.

The as-built rule is the special case kappa_r / E = kappa_n / h = kappa1, kappa2 = 0, delta = 0.

## Tests

- Jury conditions re-derived for the normalized rule. J2 becomes
  kappa_r (1 + eta eps / ((2 - eta) E)) < 2 when kappa2 = 0: the same for every theta.
  `price_rule_stability.rs` checks 10 pairs inside and 10 outside per good, as now.
- Nominal mode: a 5% shock to the money stock returns the world index within 1% in under 30
  ticks at the defaults.
- Special case: with the gains mapped as above, state hashes equal the as-built rule over 120
  ticks.
- Stocks: from a 3x stock shock, cover returns to P02 within its half-life prediction.

## Consequences

- Stability no longer depends on theta, and the nominal half-life falls from about 190 months
  to about 3 ticks.
- Every market reads one world mean per tick: a global coupling, cheap but serial.
- "Merchants see world excess demand" is a modelling shortcut. P5 money and expectations
  (12.2) can replace the aggregate term with a micro-founded anchor.
- Register: P03a redefined, P143 added; trade SPEC Stability region rewritten.
