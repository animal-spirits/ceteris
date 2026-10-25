# 0046. Political economy channels: populism, information, polarization and institutions

Status: proposed
Date: 2026-10-24
Design sections: 10.1, 10.4, 10.5, 10.7, 10.8, 9.2, 9.6, 18 (H7)
Phase: P6

## Context

Section 10 builds politics from interest groups, parties, legitimacy and opinion dynamics.
Several of the best-identified links between economic shocks and politics have no explicit
channel:

- Trade shocks raise polarization and support for more extreme candidates (Autor, Dorn, Hanson
  and Majlesi 2020).
- After financial crises, far-right vote shares rise by about 30% on average and fragmentation
  increases (Funke, Schularick and Trebesch 2016, 800+ elections since 1870).
- Refugee arrivals raise far-right support locally (Dinas et al. 2019, Greek islands).
- Information technology changes accountability: mobile internet lowered government approval
  where corruption was high (Guriev, Melnikov and Zhuravskaya 2021).
- Central bank independence is eroded by governments under pressure (Turkey, Argentina), with
  inflation consequences; it can be tracked with de jure indices (Garriga 2016; Romelli 2022).
- Corruption leaks public spending and revenue; judicial independence limits executive moves.
  Both are V-Dem dimensions already in the state vector.

## Options

1. Status quo: these effects emerge, or not, from generic legitimacy and voting rules.
2. Explicit, separately parameterized channels, each with its natural experiment as the
   validation target, and each subject to the ablation rule.
3. Learned policies only (Tier 1b).

## Decision

Option 2, each channel behind its own flag:

- Grievance from economic shocks: local import-competition losses and crisis episodes raise the
  salience of the nationalism and anti-establishment dimensions of the policy space (10.5) for
  affected pops (P84, P85).
- Information environment: each pop has an information-exposure level (media freedom, internet
  penetration) that scales how fast perceived corruption and performance enter approval and the
  Hegselmann-Krause updates (P86).
- Refugee exposure: arrivals relative to local population raise the salience of the immigration
  and identity dimensions.
- Central bank independence is a state variable initialized from CBIE, eroded by governments
  with small winning coalitions or high debt, restored by reform laws (P87); policy-rule
  coefficients (P35) depend on it.
- Corruption leakage on spending and tax collection, and judicial independence as a veto on
  executive policy changes, both from V-Dem.

Channels that fail ablation stay in the extended profile (23.1).

## Migration

- P6, in ceteris-polity.
- Data: V-Dem v16 (supersedes v15; CC BY-SA 4.0, commercial use allowed), Romelli CBIE and
  Garriga CBI (licences to audit), ITU internet penetration.
- Validation: H7 (crises and populism) plus event studies of the China shock on US vote shares
  and of refugee exposure.

## Tests

- With all channels off, golden hashes equal the P6 baseline.
- Event studies on simulated crises and trade shocks recover effects with the signs and rough
  magnitudes of the source papers.

## Consequences

Economic shocks reach politics through measured channels, so policy endogeneity in
counterfactuals rests on evidence rather than on generic approval rules.
