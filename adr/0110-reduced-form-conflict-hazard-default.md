# 0110. Reduced-form conflict hazard as the lab default

Status: proposed
Date: 2026-10-26
Design sections: 10.1, 10.8, 11.2, 11.5, 11.6, 13.5, 15.4, 17.2; amends ADRs 0048, 0051, 0052

## Context

The war module (11) is structural: a contest function, crisis bargaining with information and
commitment problems, and a combat model (Lanchester in 11.5; Dupuy-style multipliers under
0048). Political state comes from the V-Dem institution vector (10.1).

- **Identification.** Structural parameters are not identified on 2000-2010, and only weakly on
  long panels. Reduced-form conflict forecasts are hard to beat (ViEWS; Hegre et al. 2019),
  and 0052 already scores against them. Until the structural model beats a hazard model, its
  probabilities are priors presented as mechanisms.
- **Conflict persistence.** Recurrence after war is one of the strongest regularities in
  conflict data (the conflict trap), and the structural model has no term for it.
- **Measurement error in V-Dem.** V-Dem values are posterior means of latent quantities with
  wide intervals; using point values as exact states understates uncertainty.
- **Combat.** Lanchester laws fit historical battles poorly; force employment matters more than
  numbers (Biddle 2004). 0048 already moves to Dupuy-style multipliers; outcome and duration
  calibration remains open.

## Options

1. Structural model as default, hazard as benchmark only.
2. Reduced-form hazard as the lab default; structural model in the extended profile, promoted
   when it beats the hazard on the 2011-2018 selection period (0107).
3. Hybrid: hazard for onset, structural model for conduct and termination.

## Decision

Option 2, with option 3's split once the structural model qualifies on conduct before onset.

- **Hazard.** Monthly onset hazards for civil conflict (UCDP, 25 deaths) and interstate disputes
  (MID or COW), a logit or cloglog in: log GDP per capita, growth, population, V-Dem
  electoral democracy and its square, ethnic exclusion (EPR), neighbourhood conflict, and a
  peace-years spline (Beck, Katz and Tucker 1998). Estimated offline on 1946+ (civil) and
  1816+ (interstate) per ADR 0108. Termination by a duration model on the same data.
- **Selection.** Switch (new 17.2 entry): hazard (lab default) or structural (extended). The
  structural model becomes default when its onset log score on 2011-2018 exceeds the hazard's
  (Diebold-Mariano at 5%).
- **V-Dem draws.** Each ensemble member draws its initial institution vector from the V-Dem
  posterior (V-Dem publishes draws), deterministic per member.
- **Combat.** Battle outcome and attrition from 0048's model are calibrated to CDB90 outcomes
  and COW war durations; exchange ratios enter as a parameter lane.
- **Nuclear deterrence** is a lane (ADR 0108).

## Migration

- war module: a hazard path beside the structural path, behind the switch; both write the same
  events to the shock tape.
- Pack: UCDP, COW, MID (licences to audit in 0005), EPR, V-Dem posterior draws.

## Tests

- The hazard reproduces its offline estimates on the historical panel (coefficients within one
  standard error when re-estimated on simulated data).
- Simulated recurrence rate after war termination matches the historical rate within its
  interval.

## Consequences

- Conflict probabilities in default runs are empirically grounded and scored; mechanism
  analysis stays available in the extended profile.
- The structural model has a measurable bar to clear.
