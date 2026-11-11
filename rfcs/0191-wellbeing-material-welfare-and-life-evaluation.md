# 0191. Wellbeing: material welfare and life evaluation

Status: proposed
Date: 2026-10-28
Design sections: 6.1, 8.6, 10.7, 10.8, 20.1; ADRs 0021, 0046, 0056, 0060, 0129, 0139, 0189
Phase: 1.x (read-only map mode), P6 (2.0-alpha) for political uses

## Context

Section 10.7 builds approval from "material conditions relative to expectations" without saying
what a pop's material conditions are, and ADR 0129 scores runs in consumption-equivalent welfare
(Jones and Klenow 2016) without a per-pop definition the live model can read. People also report
caring about things consumption misses. The World Happiness Report finds that six national
factors (income, social support, healthy life expectancy, freedom, generosity, perceived
corruption) explain about three quarters of cross-country variance in the Cantril ladder, and
micro studies add large, non-adapting costs of unemployment (Winkelmann and Winkelmann 1998;
Clark et al. 2008), a trade-off between unemployment and inflation (Di Tella, MacCulloch and
Oswald 2001; Blanchflower et al. 2014), comparison with a reference group (Clark, Frijters and
Shields 2008) and adaptation to income gains (Di Tella, Haisken-DeNew and MacCulloch 2010).

Design note with the full chain from wellbeing to laws

## Options

1. Status quo: approval reads income and prices directly; wellbeing is undefined.
2. One subjective score per pop, fitted to survey data.
3. Two scores per pop: an objective material welfare implied by the demand system, and a
   subjective life evaluation layered on it.

## Decision

Option 3.

**Material welfare W.** The money-metric of each pop's PIGL indirect utility (ADR 0021) at 2000
prices, plus the Jones-Klenow terms consumption misses. With lognormal income (mu, sigma^2) the
inequality term is closed form:

```
W_i = log e*_i(p_2000) + u_bar + v(leisure_i) - sigma_i^2 / 2 + log LE_i
```

ADR 0129's veil score uses this W, so the score and the live model share one definition.

**Life evaluation L (0 to 10).**

```
L_i = alpha_c + b_y W_i + b_r (W_i - W_ref(i)) + b_a (W_i - W_exp_i) - b_u u_i - b_U U_c
      - b_pi pi_c + b_t trust_i + b_f freedom_c - b_k corruption_perceived_c - b_s insecurity_i
```

- alpha_c is a country constant frozen at its 2005-2010 value, so response styles stay put and
  only changes are modelled.
- W_ref is the pop's reference group (region and ethnic group); W_exp is the expectation from
  the experience kernel of ADR 0139, with losses weighted about twice gains (Boyce et al. 2013).
- u_i is the pop's own unemployment share; U_c and pi_c are national unemployment and inflation.
- Trust is ADR 0056's stock, freedom is V-Dem civil liberties, perceived corruption passes the
  information filter of ADR 0046, insecurity is homicide (ADR 0060) and conflict exposure.

**Uses, each behind the ablation rule (15.7).** Changes in L enter approval (ADR 0192), since
life satisfaction predicts the incumbent vote beyond the economy (Liberini, Redoano and Proto
2017; Ward 2020). The gap between L and its expectation is the grievance input to ADR 0189's
organization potential and to the 10.8 protest threshold. A map mode shows both W and L.
Migration intentions are a plausible further use and stay off until tested.

**Parameters.** Two free (b_r and the adaptation speed). The other coefficients are estimated
offline from survey micro data and enter as literature priors; register entries are assigned
with the P6 spec.

## Migration

- 1.x: W and L computed after step 11 of 4.4 and shown as map modes; nothing reads them, so no
  golden hash moves.
- P6: approval and grievance read L.
- Data: World Happiness Report national ladder averages 2005-2024 (published Gallup World Poll
  aggregates; microdata are proprietary; terms to audit, ADR 0050); WVS and EVS (derived
  parameters only, as in ADR 0055); ESS, Eurobarometer, Latinobarometro, Afrobarometer, Asian
  and Arab Barometer for group and subnational coefficients (terms to audit).

## Tests

- W equals Monte Carlo integration of PIGL utility over the pop's lognormal income to 1e-6.
- With every pop identical, ADR 0129's veil score equals the state mean of W.
- Calibrated on 2005-2010, national changes in L over 2011-2024 match the WHR in sign for the
  large movers (for example Greece and Venezuela) and in rank correlation overall (ADR 0107).
- Computing W and L without their P6 uses changes no state hash.

## Consequences

Politics reads a measured notion of how lives are going, and the veil score and the live model
agree. The cost is about 12 terms for 100,000 pops a month, under a millisecond, and about 2-3
part-time weeks.
