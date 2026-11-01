# 0099. Split final demand and a fixed saving rate in P2

Status: proposed
Date: 2026-10-26
Design sections: 7.4, 8.6, 9.6, 15.3; econ SPEC Deviations; amends ADRs 0021, 0025, 0089

## Context

The P2 core has one final demander per pop, with country shares from all ICIO final demand:
household consumption (HFCE), government (GGFC and NPISH) and gross fixed capital formation
(GFCF) together. Pops spend all income (0021: m = 1, lambda = 0 until P5). Two distortions
follow.

- PIGL weights are inverted from a budget that includes machinery and construction bought for
  investment and services bought by government. Engel and price elasticities that belong to
  households are applied to all of it, so structural change is mis-measured from t = 0.
- Investment, the most volatile component of demand, responds to income exactly like
  consumption. There is no accelerator, so downturns are too mild and capital-goods sectors
  are too stable. With no saving there is also no wedge between income and spending, so any
  shock passes one for one into demand.

These are stage stand-ins, but they shape every P2 and P3 result, and the data to fix them is
already in the ICIO pack.

## Options

1. Keep one demander until P5.
2. Three demanders per country from ICIO columns: households (PIGL), government (fixed income
   share, fixed composition) and investment (fixed saving rate, fixed composition).
3. Full fiscal block and investment from capital accumulation (9.6, 7.4).

## Decision

Option 2. Option 3 replaces the two fixed rules when P5 and P4 arrive.

- **Households.** PIGL is inverted on HFCE shares only. Pops spend (1 - s_c - g_c) of disposable
  income on PIGL categories.
- **Government.** A country-level demander spends g_c, the 2000 GGFC plus NPISH share of GDP, of
  national income, with the ICIO GGFC composition by good at 2000 prices. Funded by a
  proportional tax on pop income at rate g_c, held in a per-country fiscal account.
- **Investment.** A country-level demander spends s_c, the 2000 GFCF share of GDP, with the ICIO
  GFCF composition. Funded from pop income at rate s_c. With the fixed factor F constant in P2,
  investment is demand only; P4 links it to capital.
- **Accelerator, optional switch.** Investment spending follows s_c times a smoothed income
  ratio raised to an elasticity beta (new register entry, prior 1 to 3, Ramey 2016 style
  responses); beta = 1 is the default and reproduces the fixed share.
- Inventory change in ICIO stays dropped, as now.

## Migration

- Pack: `econ_countries.arrow` final demand shares become three vectors of 10 (HFCE, GGFC,
  GFCF) plus g_c and s_c.
- Ledger: one fiscal and one investment account per country; both spend in full each tick so
  the money stock is unchanged.
- 0089 stage 4: the PIGL inversion uses the HFCE vector; government and investment spending are
  fixed values in the Sinkhorn targets.

## Tests

- t = 0 world spending by good equals the ICIO final-demand total by good within 1e-10.
- With beta = 1, a 1% permanent income shock raises spending on each column by 1% of its base
  in the new steady state.
- V1 holds from the perturbed starts.
- ADR 0025's harness: with m = 1 and lambda = 0 the households' intertemporal MPC matrix is
  (1 - s_c - g_c) times the identity, and aggregate spending out of income is 1.

## Consequences

- Structural-change dynamics are fitted on household data, the only data PIGL is estimated on.
- Investment goods respond to income at the share, not at household elasticities; with the
  accelerator on, downturns hit machinery and construction first, as in the data.
- The fiscal and investment accounts are the hooks that 9.6 and 7.4 attach to later.
