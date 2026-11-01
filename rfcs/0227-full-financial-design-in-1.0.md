# 0227. The full financial design in 1.0

Status: proposed (owner chose the option, 2026-10-28)
Date: 2026-10-28
Design sections: 8.8, 9.7-9.14, 15.4, 17, 19.2, 21.1; ADRs 0207, 0210
Phase: P5b

## Context

ADR 0207 cut the finance ADRs to a lean 1.0 but left DESIGN 9.7 to 9.14 untouched: a monthly
Koijen-Yogo demand system over about 2,000 assets and 2,000 investor classes, five bank archetypes,
four contagion channels, global risk appetite and endogenous crises. P5b's 9 to 14 owner-weeks,
which also hold the Rust HANK reference, did not price that design. The review of 28 October 2026
also found that the finance parameters come to about 14 against the cap of 12 in 9.14, that the
posted export-price layer of 8.8 is in no phase, that the 8 ms finance budget of 19.2 predates the
demand system, that holdings data for 2000 is thin outside the OECD, and that the banking-crisis
database ends in 2017. Review: https://claude.ai/artifact/X8c2Ah1MkJPnViYcku6zyk

## Options

1. Full DESIGN 9.7 to 9.14 in 1.0.
2. Lean full chain: about 6 investor classes, 3 bank archetypes, no direct interbank network.
3. Prices without crises; contagion and crisis validation in 1.1.
4. Reduced form; no demand system in 1.0.

## Decision

Option 1 (owner, 2026-10-28). The four build stages of 9.14 all ship in 1.0 with every investor
class, all five bank archetypes and all four contagion channels.

- **Estimate.** P5b grows by about 6 to 10 owner-weeks, to 15 to 24. The 1.0 total of ADR 0207
  becomes about 58 to 92 owner-weeks, roughly five to eight months later at 12 hours a week.
- **Parameter budget.** The cap of 12 free finance parameters holds. The risk-appetite innovation
  process (persistence, volatility of volatility, tail) is estimated on VIX 1990 to 1999, before
  the calibration window, and counts as data. Restructuring haircut and exclusion (0043 P79, P80)
  take Cruces-Trebesch estimates as fixed priors. The SPEC keeps the ledger of what remains free.
- **Posted export prices.** The Calvo posted-price layer of 8.8 is built in P5b with FX clearing,
  so the pass-through targets of 15.7 stay.
- **Initialization.** A finance stage of the data-state calibration (as 0089 is for trade)
  imputes 2000 sector balance sheets and who-to-whom holdings where financial accounts, CPIS,
  BIS and Lane-Milesi-Ferretti leave gaps, and reports the fit per state. Latent demand is backed
  out after imputation.
- **Crisis targets.** The Laeven-Valencia database is extended past 2017 for the 2011 to 2024
  holdout (Lebanon 2020, Sri Lanka 2022, the 2023 bank runs) from IMF and central bank reports,
  listed in the manifest with its sources.
- **Budget.** The 8 ms finance line of 19.2 is replaced by a measurement at the P5b gate; the
  inner fixed-point loop's round count and Newton iterations are set from it.

## Migration

- `specs/finance/SPEC.md` (owner-approved before code, 21.2) covers all of 9.7 to 9.14 and the
  points above.
- ADR 0207's estimate table is read with P5b at 15 to 24 weeks.

## Tests

- The P5b gate of 21.1 as written, plus the crisis-frequency score of 9.13 against the extended
  database.
- Each contagion channel passes the ablation test of 15.7 or ships off by default.

## Consequences

1.0 carries the full crisis machinery for 2008, the euro crisis and the 2013 taper. The release
moves later by several months and P5b becomes the largest phase.
