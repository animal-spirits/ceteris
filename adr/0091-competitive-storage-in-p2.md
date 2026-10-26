# 0091. Competitive storage in P2 with per-market price expectations

Status: proposed (owner chose the options, 2026-10-25)
Date: 2026-10-25
Design sections: 8.2, 12.2, 17 (P02, P56, S04); trade SPEC Origin market; ADRs 0016, 0019, 0029

## Context

ADR 0029 (design branch, proposed) adds competitive storage for grains, other storable crops,
crude oil and metals, with storage cost and convenience yield in P56. P2's origin markets
already hold merchant stock targeted at S* = s_g D, so taking 0029 later would leave two
inventory models for the same goods. The owner chose to bring competitive storage into P2
(2026-10-25). Storage needs an expected price per good; P2 has only the world pi^e (ADR 0019).

## Options

Where storage applies: (1) the 0029 goods only, with ADR 0090's kappa2 rule for the rest;
(2) every traded good. Which expectation it reads: (a) a per-market adaptive expectation;
(b) a model-consistent price within the tick; (c) world pi^e plus mean reversion.

## Decision

Options (1) and (a) (owner, 2026-10-25).

**Goods.** Competitive storage replaces the stock rule for agriculture (0), crude oil, gas and
refined fuels (1, with C19 per ADR 0092), coal and other mining (2), and chemicals, plastics,
minerals and metals (5). Food (3), light manufacturing (4) and machinery (6) keep ADR 0090's
kappa2 rule. Good 5 is the closest P2 has to "metals"; the list is revisited when goods split
in P8.

**Expectation.** Each origin market (o, g) keeps an expected log-price change

```
pi^e_og <- pi^e_og + g_s ( dlnp_og - pi^e_og )        (P146, prior as P45)
```

updated after T4. S04 stays adaptive-only.

**Storage rule.** Merchants at the origin market buy into or sell out of stock at the posted
price. The expected net return of holding one more unit for a month is

```
x_og = pi^e_og + y_g(S / (s_g D)) - k_g - delta_g
y_g(z) = (k_g + delta_g) z^(-zeta_g)
```

with k_g the money cost and delta_g the physical loss per month (P56), and y_g the convenience
yield (Working-Kaldor), falling in cover z. Storage demand added to orders at T3:

```
dS_og = delta_g S_og + psi_g D_og x_og,   floored so that S' >= 0
```

psi_g (P144) is the speculation intensity, zeta_g (P145) the convenience-yield elasticity.
At a steady state pi^e = 0, so x = 0 at z = 1: stock sits at P02 cover, and P02 keeps its
meaning. A rising price expectation pulls stock in, and a stockout (S = 0) lets the price spike,
the asymmetry of Deaton and Laroque (1992). The kappa2 term of ADR 0090 is off for these goods;
the relative and aggregate terms apply unchanged.

## Tests

- Steady state: at pi^e = 0 and z = 1, storage orders equal spoilage delta_g S; ADR 0089's fixed-point test holds with storage.
- Linearized stability: the storage loop adds one state per market; re-derive the region with
  psi and zeta and check it on the toy market.
- Stockout asymmetry: a 30% harvest cut in agriculture raises the price more than a 30% surplus
  lowers it, and stocks hit zero only on the cut side.
- Conservation: storage purchases and releases pass through the ledger; spoilage is tallied.

## Consequences

- One inventory model per good; ADR 0029's storage part is implemented in P2, its export
  restrictions and famine parts stay in their phases.
- Three new register entries (P144 to P146); P56 is used from P2.
- psi, zeta and g_s are not identified in P2 (no shocks); P9 fits them to commodity price and
  stock dynamics (FAO, EIA, LME).
- ADR 0028's OPEC rule reads the same stocks for crude.
