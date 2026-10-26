# 0095. Consistency amendments to proposed design ADRs

Status: proposed
Date: 2026-10-25
Design sections: 7.2, 8.3, 13.4, 17; ADRs 0026, 0027, 0028, 0029, 0032, 0034, 0042, 0045 (design branches)

## Context

Eight ADRs proposed on the `design/*` branches conflict with P2 as built or with ADRs 0089 to
0094. Each must be amended before it is accepted.

## Decision

| ADR | Conflict | Amendment |
| --- | --- | --- |
| 0026 Pareto firms | Reads theta as Pareto shape k but keeps tariff-based estimates. Under Melitz-Chaney with ad valorem tariffs the tariff elasticity is k sigma/(sigma - 1), not k (Costinot and Rodriguez-Clare 2014; Felbermayr, Jung and Larch 2015) | k = theta (sigma - 1)/sigma, or estimate k from freight-cost variation |
| 0027 short- and long-run elasticities | Cites a long run near 2 (Boehm, Levchenko and Pandalai-Nayar) yet keeps theta_c up to 12.55; its lagged kernel adds a state to every market | Long run = theta_c (ADR 0092); short-run order elasticity rho_c theta + 1 enters ADR 0090's E_o; re-derive the stability region with rho_c |
| 0028 OPEC rule | Quotas on good 1, which merges crude, gas and (ADR 0092) refined fuels | Apply to good 1 in P2 with a crude share; split crude, gas and refined fuels in P8 before the rule binds |
| 0029 food system | Its storage part now lives in ADR 0091 | Keep export restrictions and famine; point storage to 0091; P56 used from P2 |
| 0032 cohort demography | Says P2 reads age bands; P2 pops are province by education | Correct the text: age bands arrive in P4 |
| 0034 power sector | Load duration curves per state, while electricity is a province-local good | Add a state grid market (or trade in good 7 within a state) before P8 |
| 0042 public capital | Public capital enters A, while ADR 0089 sets A = 1/p_VA | A = A_cal (K_g / K_g0)^P77, with A_cal from ADR 0089 |
| 0045 sanctions evasion | Transshipment links do not exist in the two-level kernel (only direct A-to-B gateway pairs) | Add a transit hop a to c to b in the gateway layer with wedge P83, still low rank |

## Consequences

The amended ADRs stay consistent with P2's calibration and stability analysis. No P2 code changes.
