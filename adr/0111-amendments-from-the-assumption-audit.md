# 0111. Amendments from the assumption audit

Status: proposed
Date: 2026-10-26
Design sections: 6.2, 8.2, 8.3, 8.6, 13.2; amends ADRs 0027, 0033, 0091 (proposed, on
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
- **0091, competitive storage.** Per-market adaptive expectations make stocks chase past prices.
  They stay the P2 rule (owner choice D5b, 2026-10-25; confirmed 2026-10-26). Amendment: a
  rational-expectations policy function (Deaton and Laroque 1992), solved offline on a grid of
  availability and stored as a lookup table in the pack, is added as an alternative switch; the
  run interpolates it, which is deterministic and costs one table read per market. P9 makes it
  the default only if it beats adaptive expectations on commodity price and stock moments (FAO,
  EIA, LME).

## Raised, no ADR now

- An energy category in PIGL (8.6): wait for 0028's oil and gas structure.
- Nested CES on intermediates with a rising elasticity (7.2): follow 0020 and 0027's
  partial-adjustment mechanism once P4 adds investment.
- Services trade across borders: a gravity kernel without gateways; revisit when goods grow
  toward 45.
- Koijen-Yogo latent demand (9.9): fewer classes and mean-reverting latent demand; take up in
  P5 design.
- Informality and u* (0031, decision D1): no change.
- A Kotz, Levermann and Wenz damage lane for 0036: wait for the corrected paper. The 2024 version
  was retracted in December 2025 and cannot anchor a lane (ADR 0036; owner, 2026-10-26).

## Consequences

- 0027 moves from optional to required before P3 policy work.
- No change to P2 golden hashes.
