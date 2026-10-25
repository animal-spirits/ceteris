# 0044. Pensions, life-cycle saving and wealth dynamics

Status: proposed
Date: 2026-10-24
Design sections: 6.1, 8.6, 9.1, 9.6, 15.7
Phase: P5; needs ADR 0032

## Context

Ageing is the largest predictable fiscal pressure of 2025-2100, and pension rules are among the
most common policy questions. The design has pension spending (9.6) and retirement by age and
pension law (6.3) but no pension system and no life-cycle saving. Wealth inequality is
initialized from distributional national accounts (15.7) but has no dynamics: Gabaix, Lasry,
Lions and Moll (2016) show that standard random-growth models cannot produce the speed of
observed changes in top wealth shares without heterogeneity in returns or scale dependence.

## Options

1. Status quo.
2. Pension systems per state (pay-as-you-go defined benefit, funded defined contribution,
   mixes) with contribution rates, benefit formulas and retirement ages as laws; age-dependent
   targets in the buffer-stock consumption rule; inheritance at death; a two-type return
   process for wealth.
3. Overlapping-generations households with per-agent optimization (excluded by 15.7).

## Decision

Option 2.

- Pension systems: contributions from wages, benefits by formula, funded systems holding assets
  in the demand system (9.9); parameters from OECD Pensions at a Glance and national laws.
- Life cycle: the consumption rule's target wealth ratio varies by age band, which turns the
  cohort structure of 0032 into life-cycle saving without optimization.
- Inheritance: deaths transfer wealth within the pop's ethnic and province group, by the age
  gap of a generation.
- Returns: wealth brackets earn a passive return or, for an entrepreneurial share, a
  higher-mean, higher-variance return (P81); bequest motive strength P82. Top shares then move
  at the speed found in WID data.

## Migration

- P5: pension accounts in ceteris-finance; age-dependent targets in the consumption rule.
- Data: OECD Pensions at a Glance and ISSA country profiles (licences to audit); WID top shares
  as validation (terms to audit).

## Tests

- With one return type and no pensions, results equal the status quo.
- Top 1% wealth share paths 2000-2024 in the United States, France and China within tolerance.
- A retirement-age increase raises labour supply and lowers pension spending with the sign and
  timing of its cohort arithmetic.

## Consequences

Ageing, pension reform and wealth concentration become central, testable counterfactuals.
