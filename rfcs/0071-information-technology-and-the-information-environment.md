# 0071. Information technology and the information environment

Status: proposed
Date: 2026-10-24
Design sections: 10.7, 10.8, 9.6, ADR 0046
Phase: P4 (diffusion), P6 (political uses)

## Context

ADR 0046 gives each pop an information-exposure level. What drives it, and what it does, need
specifying. Internet and mobile adoption follow S-curves (ITU, 2000-2024), among the cleanest
out-of-sample targets available. Mobile phones raise mass mobilization in bad times by easing
coordination (Manacorda and Tesei 2020); mobile money raises financial inclusion and
consumption smoothing (Jack and Suri 2014); digital ID and payments cut leakage in transfers
(Muralidharan, Niehaus and Sukhtankar 2016). States answer with censorship (King, Pan and
Roberts 2013) and surveillance technology, which China exports and which correlates with
suppressed unrest (Beraja et al. 2023). The polarization effect of social media is smaller than
folk accounts: a large deactivation experiment before the 2020 US election found no significant
effect on polarization (Allcott et al. 2024).

## Options

1. Status quo: exposure as a fixed parameter.
2. Diffusion stocks for internet, mobile and mobile money per province; digital public
   infrastructure as policy projects; censorship and surveillance capacity as state stocks;
   modest, ensembled effects of the information environment on opinion dynamics.

## Decision

Option 2.

- Diffusion per province by logistic S-curves with income, electricity access, urbanization and
  price covariates (P120 holds the covariate loadings).
- Uses: coordination (protest thresholds fall with mobile penetration in bad times), mobile
  money (lower transaction costs, better smoothing in the consumption rule), digital public
  infrastructure (projects that cut leakage in transfers and raise formalization).
- Censorship and surveillance capacity per state, built or imported, raise the effective
  protest threshold and lower the information reaching approval (P121). They also degrade the
  leader's own information (the dictator's information problem, with 0065).
- Information fragmentation slightly raises opinion-update volatility and the speed of protest
  diffusion, with small effects ensembled around zero (P122), consistent with the experimental
  evidence.

## Migration

- P4 (diffusion), P6 (uses).
- Data: ITU indicators (terms to audit), GSMA mobile money (terms to audit), Freedom on the Net
  (terms to audit).

## Tests

- Simulated internet adoption 2011-2024 out of sample by country, scored against ITU.
- The Manacorda-Tesei interaction (coverage times downturn raises protest) is recovered on
  simulated data.

## Consequences

The Arab Spring's coordination channel and digital authoritarianism's suppression channel
become mechanisms, with honest effect sizes.
