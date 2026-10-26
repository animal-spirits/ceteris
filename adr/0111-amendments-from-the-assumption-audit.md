# 0111. Amendments from the assumption audit

Status: proposed
Date: 2026-10-26
Design sections: 6.2, 8.2, 8.3, 8.6, 13.2; amends ADRs 0027, 0033, 0036, 0091 (proposed, on
`design/p2-review-adrs`)

## Context

The 2026-10-26 assumption audit found some distortions already addressed by proposed ADRs that
need a change of default, timing or a small extension rather than a new decision. This ADR
records those amendments so the original ADRs stay readable. Items the audit raised that need
no ADR now are listed at the end.

## Decision

- **0027, short- and long-run trade elasticities.** The audit ranks month-to-month application
  of the long-run theta among the top three distortions for P3: tariff experiments show trade
  diversion years too early. Amendment: 0027 is a prerequisite for any P3 tariff or sanctions
  experiment, and S12 defaults to partial adjustment from P3 onward. Until rho is estimated,
  the P53 prior centre (half-life 24 months, Boehm, Levchenko and Pandalai-Nayar 2023) is the
  default.
- **0033, mortality.** The Preston curve shifts upward over time at a given income (health
  technology), by about 0.2 years of life expectancy per year historically. Amendment: the
  Brass level's income response includes a time shift, estimated on WPP 1950-2000 with the
  rest of 0033's offline fit (ADR 0108 window), and the shift's future rate is a lane (low,
  central, high).
- **0036, climate damages.** Add a lane following Kotz, Levermann and Wenz (2024) beside the
  existing ones, so the lane set spans the post-2020 empirical estimates.
- **0091, competitive storage.** Per-market adaptive expectations make stocks chase past prices.
  Amendment: when 0091 is accepted, each storable good's storage rule is a rational-expectations
  policy function (Deaton and Laroque 1992) solved offline on a grid of availability and
  stored as a lookup table in the pack; the run interpolates it, which is deterministic and
  costs one table read per market. Adaptive expectations stay as the fallback switch.

## Raised, no ADR now

- An energy category in PIGL (8.6): wait for 0028's oil and gas structure.
- Nested CES on intermediates with a rising elasticity (7.2): follow 0020 and 0027's
  partial-adjustment mechanism once P4 adds investment.
- Services trade across borders: a gravity kernel without gateways; revisit when goods grow
  toward 45.
- Koijen-Yogo latent demand (9.9): fewer classes and mean-reverting latent demand; take up in
  P5 design.
- Informality and u* (0031, decision D1): no change.

## Consequences

- 0027 moves from optional to required before P3 policy work.
- No change to P2 golden hashes.
