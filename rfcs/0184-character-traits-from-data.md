# 0184. Character traits from data, and the ablation gate

Status: proposed
Date: 2026-10-28
Design sections: 10.5, 12.3, 15.7, 17, 23.5; ADRs 0066, 0107, 0136, 0183
Phase: P6 (2.0-alpha); gate for the 4.0 lab default

## Context

23.5 lists latent traits per role but not where they come from. Hand-set traits for named
people would be defamatory and would leak the holdout (ADR 0107).

## Options

1. Traits drawn from role priors only.
2. Observables from data; latent traits drawn from priors conditioned on observables known
   before 2000; every effect bounded by a published estimate.

## Decision

Option 2.

| Trait | At start from | Enters as | Bounded by |
| --- | --- | --- | --- |
| Age, sex, birthplace | Wikidata; PLAD | Mortality; spending tilt to the birth region | Hodler and Raschky (2014) |
| Military and rebel past | LEAD (Ellis, Horowitz and Stam 2015) | Risk weight in war and crisis choices | Horowitz and Stam (2014) |
| Education, profession | Wikidata, LEAD | Implementation slippage; reform propensity | Besley, Montalvo and Reynal-Querol (2011); Dreher et al. (2009) |
| Lived inflation | CPI history over the person's life | Governor hawkishness within the regime's band | Malmendier, Nagel and Yan (2021) |
| Entry manner, tenure, fate | Archigos, REIGN | Exit hazard and value of staying (0066) | Archigos distributions |
| Ideology | Party position (V-Party, ParlGov, Manifesto) | Planner weights (10.5) | Party positions, small offset |
| Competence, loyalty | Latent; prior shifted by selection (Egorov and Sonin 2011) | Planner noise (12.3), slippage | Jones and Olken (2005); Easterly and Pennings (2020) |

- Nobody hand-sets a trait for a named person. Post-2000 knowledge never enters a prior.
- Latent traits stay hidden; the player sees dossier signals (23.5).
- Each effect is a P-number in the register (17).
- Gate for the 4.0 lab default (15.7): simulated leader deaths change growth as in Jones and
  Olken (clear in autocracies, near zero in democracies); the spread of leader effects stays
  within Easterly and Pennings; birth-region night lights, dispute initiation by background,
  and governors' rate deviations by lived inflation match the studies above in sign and rough
  size; out-of-sample macro scores are no worse than without characters.
- Nesting: traits at role means reproduce ADR 0066 bit for bit.

## Tests

- The nesting golden hash.
- The gate tests above, run as a lab study before 4.0.

## Consequences

Leader effects are as large as the evidence allows and no larger. Licences of LEAD, WhoGov,
PLAD and REIGN are unchecked; under the 28 October 2026 decision each user downloads them.
Cost about 3-4 part-time weeks, plus 3-4 for the gate study.
