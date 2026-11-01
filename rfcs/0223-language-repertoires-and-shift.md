# 0223. Language repertoires and shift

Status: proposed
Date: 2026-10-28
Design sections: 6.1, 6.4, 6.5, 8.7, 17.1; ADRs 0055, 0058, 0189, 0207
Phase: P4 (fields reserved), P6 (2.0-alpha) dynamics

## Context

Section 6.1 gives each pop "language shares" and 6.4 names Bisin-Verdier transmission, with no
definition of a share, no bilingual state, no levers and no targets. Observed language shift runs
through bilingualism over two or three generations (Fishman 1991; Alba et al. 2002 for Spanish in
the United States), and stable bilingualism exists where the minority language has status or
schooling (Catalan, Welsh, Basque). Abrams and Strogatz (2003) fit adult switching between two
languages with exponent a of about 1.31 across 42 languages, but their model always ends in the
extinction of one language; the bilingual extensions (Mira and Paredes 2005; Minett and Wang 2008)
allow coexistence. Ethnologue, the usual speaker source, is subscription-only.

## Options

1. Heritage repertoire per pop: shares of heritage only (H), bilingual (HD) and dominant only (D).
2. A share vector over every language spoken in the province.
3. Language equals ethnic group; no separate state.

## Decision

Option 1 (owner, 2026-10-28).

- **State.** For each pop whose group has a heritage language H distinct from the state's dominant
  language D: shares h, b, d summing to one. Groups with no distinct heritage language skip the
  field. Where a state has more than one official language, D is the one dominant in the province.
- **Transmission at birth.** P(child acquires H) = h + b * tau, with
  tau = sigma(k0 + k_s (s_H - s_D) + k_x logit(x_H) + k_e school_H + k_m homogamy + k_f backlash).
  s is language status (official use, share of income earned in the language, media), x_H the
  provincial share of H speakers, school_H whether H is a medium of instruction, homogamy the
  in-group marriage rate, backlash the salience term of ADR 0225. Children acquire D from schooling
  and exposure.
- **Adult change.** H to HD at rate alpha (1 - x_H) times the wage premium of D (Chiswick-Miller);
  HD to D at rate beta (1 - s_H) (1 - x_H)^a. With no bilinguals and fixed status this reduces to
  Abrams-Strogatz.
- **Uses.** Language distance and repertoire enter migration moving costs (6.5), the D wage premium
  in labour matching (8.7), recruitment of language movements (0189) and vote choice through
  salience (0058, 0193).
- **Parameters.** k0, k_s, k_x, k_e, k_m, k_f, alpha, beta, a: nine register entries (17.1), each
  with a literature prior; a starts at 1.31.
- Off by default until it passes the ablation test (15.7); with it off, shares stay at 2000 values.

## Migration

- P4: fields reserved and filled from 2000 data, no dynamics (ADR 0207). P6: dynamics, yearly.
- Data: Unicode CLDR territory language populations (Unicode License v3) for national speaker
  shares, Glottolog (CC BY 4.0) for codes and family distance, IPUMS International census
  microdata (fetch only) for subnational shares and intermarriage. Ethnologue is excluded.
- Validation: Welsh and Irish censuses, Basque sociolinguistic surveys 1991-2021, Catalan, Quebec
  French, Russian in Latvia and Estonia, Kazakh, US ACS Spanish by generation, Ukraine 2001-2023.

## Tests

- With no bilinguals and constant status, the share path matches the Abrams-Strogatz solution.
- With the mechanism off, golden hashes are unchanged.
- Shares h, b, d stay in [0, 1] and sum to one (proptest).
- Simulated Welsh and Basque speaker shares by age 2001-2021 within tolerance.

## Consequences

Language shift, revival and stable bilingualism become reachable outcomes of policy and
economics, and later modules (migration, labour, organizations) read one consistent field.
