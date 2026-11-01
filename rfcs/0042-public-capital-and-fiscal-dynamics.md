# 0042. Public capital and fiscal dynamics

Status: proposed; amended per ADR 0095 (2026-10-26)
Date: 2026-10-24
Design sections: 5.5, 7.2, 9.4, 9.6, 15.4
Phase: P5

## Context

Public investment is a spending category (9.6) and infrastructure lowers transport costs (5.5),
but there is no public capital stock in production and no efficiency of public investment.
Meta-analysis puts the output elasticity of public capital around 0.08, higher for core
infrastructure (Bom and Ligthart 2014); how much spending becomes capital differs widely by
state (IMF Public Investment Management Assessments; Gupta et al. 2014). Fiscal multipliers
depend on slack and monetary accommodation (Ramey and Zubairy 2018) and debt dynamics on r
minus g. These drive the fiscal counterfactuals that a Steward or lab user will run first.

## Options

1. Status quo.
2. A public capital stock per province in the value-added function with an efficiency factor
   per state; multipliers emerge from the model and are validated, not imposed.
3. Imposed multipliers.

## Decision

Option 2.

- Public capital enters A in 7.2 as A = A_cal (K_g / K_g0)^P77, with A_cal the calibrated value
  of ADR 0089 (ADR 0095) and elasticity P77 (prior centred on 0.08, by type: core
  infrastructure, other). Public investment adds to the stock times an efficiency factor from
  PIMA scores and governance (P78); the rest is lost (corruption leakage from 0046 adds to the
  loss).
- Debt dynamics come from the fiscal and finance blocks already in the design; r minus g is
  reported per state.
- Multipliers are measured on the model (impulse responses to spending shocks) and validated
  against Ramey-Zubairy state-dependent estimates.

## Migration

- P5: public capital state and its production term; efficiency data in the pack.
- Data: IMF Investment and Capital Stock Dataset and PIMA scores (licences to audit).

## Tests

- With P77 zero, results equal the status quo.
- Spending multipliers in slack and non-slack states have the ordering and rough size of
  Ramey-Zubairy.

## Consequences

Public investment, its quality and fiscal space become the levers they are in practice.
