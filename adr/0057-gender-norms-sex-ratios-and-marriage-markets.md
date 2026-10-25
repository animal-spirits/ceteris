# 0057. Gender norms, sex ratios at birth and marriage markets

Status: proposed
Date: 2026-10-24
Design sections: 6.1, 6.2, 6.3, 10.8
Phase: P4

## Context

Female labour force participation is shaped by norms as well as wages and education, and those
norms move with values and schooling. Sex ratios at birth rose sharply in China, India and
South Korea with son preference and prenatal sex selection, then fell in South Korea and partly
in China as development and policy changed (Guilmoto 2009; Jayachandran 2015). Surplus young
men tighten marriage markets, and Edlund et al. (2013) link rising sex ratios to higher crime
in China. Pops already carry age and sex, so these mechanisms are cheap.

## Options

1. Status quo: participation by labour status only; sex ratio at birth fixed.
2. A participation norm per pop, a sex-ratio-at-birth path per state, and a marriage-squeeze
   index from cohort sex ratios.

## Decision

Option 2.

- Female participation norm per pop, moved by the emancipative value score (0055), female
  education and urbanization (P98); it scales female labour supply in 8.7.
- Sex ratio at birth per state declines with female education, development and policy (bans on
  sex-selective abortion) from its 2000 level (P96).
- Marriage-squeeze index per province from the sex ratio across the usual age gap; it feeds the
  crime model (0060) and protest propensity as a small, uncertain effect (P97), and lowers
  marriage and fertility.

## Migration

- P4: in the demography module.
- Data: UN WPP sex ratios at birth (CC BY 3.0 IGO); ILO participation by sex (already in the
  manifest).

## Tests

- Simulated sex ratios at birth reproduce the rise and fall in South Korea, the rise and
  partial decline in China, and India's slow decline, within tolerance.
- Female participation paths 2000-2024 by region within tolerance.

## Consequences

Labour supply, fertility and some crime and stability dynamics respond to gender norms and to
the sex-ratio legacy of 1990-2010.
