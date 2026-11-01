# 0226. Atrocity risk, onset and consequences

Status: proposed
Date: 2026-10-28
Design sections: 6.2, 6.4, 6.5, 10.1, 11.6, 13.5, 15.3; ADRs 0052, 0056, 0058, 0065, 0069, 0072, 0110, 0115, 0129, 0132, 0141, 0154, 0201, 0222, 0225
Phase: P6 (2.0-alpha) risk and map mode, P7 (2.0-beta) acts and consequences
Amends: ADR 0154

## Context

ADR 0154 makes atrocities and banned weapons orders open to player and AI, and has the AI order
them when estimated payoff exceeds costs. Mass atrocities are rare; a payoff planner is likely to
produce them far too often or never, so onset rates would not be credible in the lab. Genocide
studies give a structural account: Harff (2003) finds prior genocide or politicide, political
upheaval, an ethnic-minority ruling elite, exclusionary ideology, autocracy and low trade openness
predict onset; the Early Warning Project publishes annual out-of-sample risk forecasts; Straus
(2015) adds wartime threat, founding narratives and restraints; Valentino (2004) and Valentino,
Huth and Balch-Lindsay (2004) the strategic use of mass killing against insurgencies that rely on
civilian support. Consequences last generations: Acemoglu, Hassan and Robinson (2011), Rohner,
Thoenig and Zilibotti (2013), Lupu and Peisakhin (2017).

## Options

1. Lab AI uses a calibrated onset hazard; the payoff planner runs in game-seat lanes.
2. Payoff planner only, as 0154 is written.

## Decision

Option 1 (owner, 2026-10-28).

- **Structural risk.** Logit hazard per (state, target group, year), evaluated monthly, from state
  the model already holds: prior episodes 1945-1999 (initial state), war and irregular transitions
  (10.8, 11), EPR status of the executive's group and of the target, ruling platform exclusion
  (10.5, 0189), V-Dem vector (10.1), trade openness (8.3), insurgent recruitment from the group
  (0154, 0069), state media control and cueing (0065, 0058). Restraints: intermarriage, group share
  in the armed forces, trade and aid dependence on democracies. Estimated like ADR 0110.
- **Escalation and duration.** Once begun, intensity follows a persistence process ending by
  military outcome, intervention, regime change or exhaustion, calibrated on UCDP one-sided
  violence.
- **Who decides.** In the lab default the hazard draws onset for AI actors. In game-seat lanes the
  0154 payoff planner decides, with the hazard as its prior. The player may order acts in any seat
  (0154).
- **Acts.** Mass killing, expulsion, internment, forced assimilation (0225), birth prevention and
  child transfer, each with an age and sex profile of harm.
- **Consequences.** Deaths and lost cohorts (6.2); displacement into the hazard flow (6.5); target
  group assets transferred as an ownership edit (0115, 0201) and human capital lost; lower
  generalized trust (0056) and higher salience for survivors and descendants (0058), decaying over
  generations; legitimacy, sanctions and alliance hazards, court cases (0072); settlement weights
  updated from pops, so the group map changes.
- **Labels.** The engine records acts, counts and evidence of intent. It never applies the term
  genocide; legal labels come from simulated court outcomes or, for history, the cited body, as
  in ADR 0222.
- **Presentation.** No score, objective, achievement, replay hook (0144) or music cue rewards or
  highlights an atrocity; the run score counts its deaths (0129, 0132). Orders show the cost
  screen before confirmation. A read-only atrocity-risk map mode shows the hazard by state and
  group with its factors. Identity stays statistics only (0141).
- **Parameters.** About twelve register entries: hazard coefficients, intensity persistence,
  consequence decay rates.

## Migration

- P6: hazard, risk map mode, initial prior-episode state. P7 stage W5: acts and consequences.
- Data: UCDP one-sided violence (CC BY 4.0); PITF genocide and politicide list and Early Warning
  Project data (terms to audit, ADR 0005); EPR/GeoEPR (accepted NC licence).
- Calibration cases 2000-2025: Darfur, Sri Lanka 2009, Yazidis 2014, Rohingya 2017, Xinjiang,
  Tigray, Ukraine child deportations, Nagorno-Karabakh 2023, Gaza, Sudan 2023-.

## Tests

- Simulated onsets per decade match the episode rate on the selection window.
- Out-of-sample risk ranking accuracy is scored against the Early Warning Project forecasts
  (0052).
- Every listed consequence applies after an act and none without it (as 0154).
- With the mechanism off, golden hashes are unchanged.

## Consequences

Atrocity onset in the lab is anchored to observed rates and becomes a forecastable,
counterfactual-testable outcome, so prevention levers can be studied; the slow consequences carry
into demography, economy and identity for the rest of the run.
