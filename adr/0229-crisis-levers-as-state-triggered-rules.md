# 0229. Crisis levers as state-triggered rules in 1.0

Status: proposed (owner chose the option, 2026-10-28); amended per ADR 0241, 0242 (2026-10-29)
Date: 2026-10-28
Design sections: 9.2, 9.4, 9.11, 9.12, 15.4; ADRs 0043, 0210
Phase: P5b (rules), P9 (estimation)

## Context

ADR 0210 gives 1.0 a rule per lever for the policy rate, primary balance, default and pegs. The
safety-net levers of 9.11 and the IMF of 9.4 have none: lender-of-last-resort lending, deposit
insurance, guarantees, recapitalization and bail-in, asset purchases, swap lines, macroprudential
caps and IMF programmes. Crises are endogenous (13.6), so a 2000 to 2024 tape of rescues would fire
the 2008 rescues in a branch where 2008 never happens.

## Options

1. Rules that fire on the simulated state; macroprudential settings from a tape.
2. Tape to 2024, then rules, as for 0210's other levers.
3. Off in 1.0, except lender of last resort at a fixed haircut.

## Decision

Option 1 (owner, 2026-10-28).

| Lever | Trigger | Action |
| --- | --- | --- |
| Lender of last resort | A bank archetype's liquid assets fall short of outflows | Lends against collateral at published haircuts per collateral class |
| Deposit insurance | A bank fails | Pays insured deposits up to the limit (limit on the tape, held after 2024) |
| Guarantees | Uninsured and wholesale outflows of systemic archetypes pass a threshold | Blanket guarantee of bank liabilities; contingent fiscal liability |
| Recapitalization, bail-in | A systemic archetype's capital falls below the minimum after LOLR | Bail-in up to the resolution regime's share (tape: BRRD from 2016), then public capital to minimum plus buffer |
| Asset purchases | Policy rate at the lower bound and the Taylor rule below it | Purchases sized by the shortfall (shadow-rate mapping, Wu and Xia 2016) |
| Swap lines | Dollar funding gap of non-US banks passes a threshold | Fed lines to states on the standing list (tape) |
| IMF programme | Reserve cover below threshold or market access lost | Lends a multiple of quota; programme primary-balance target replaces the 0210 fiscal rule while it runs |
| Countercyclical buffer | Credit-to-GDP gap | Tape to 2024; Basel gap guide after |
| LTV, DTI caps | None | Tape to 2024 (IMF iMaPP), held after |

Which response a crisis gets (liquidity only, guarantee, recapitalization, nationalization, IMF)
is a logit on state variables (fiscal space, bank size over GDP, union membership, reserve cover),
estimated on the policy responses recorded in Laeven and Valencia (2018). Thresholds are fitted to
the same data. Costs flow to the fiscal state (9.6) and spreads (9.4). In 2.0 the Tier 1a planner
replaces these rules, as for 0210.

## Migration

- P5b: rules in `ceteris-ai` beside 0210's; levers in `ceteris-finance`. P9: estimation.
- `specs/policy/SPEC.md` gains the levers; sources to audit for the manifest: Laeven-Valencia,
  IMF iMaPP, IMF lending arrangements, Fed swap-line history.

## Tests

- A toy bank run triggers LOLR, then guarantees, in that order, and no rule fires without stress.
- In a branch that avoids the 2008 bust, no 2008 rescue occurs.
- Simulated fiscal costs of banking crises lie within the Laeven-Valencia distribution.

## Consequences

Rescues follow simulated crises rather than dates, so counterfactuals stay coherent. Register:
thresholds and logit coefficients are estimated on data, with IDs assigned in the SPEC.
