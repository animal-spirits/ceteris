# 0149. Land combat resolver

Status: proposed
Date: 2026-10-28
Design sections: 11.5, 17.2, 23.5; ADRs 0048, 0110, 0115, 0132, 0145, 0146
Phase: P7 (2.0-beta)

## Context

11.5 uses Lanchester attrition with modifiers; 0048 moves to Dupuy-style multipliers; 0110 notes
that Lanchester laws fit battles poorly and that force employment matters more than numbers
(Biddle 2004). The resolver's shape, its modern effects and its time model are open.

## Options

Shape: Lanchester square; fitted Helmbold exchange; a Dupuy QJM/TNDM-style combat power ratio with
Biddle's force employment inside the quality term.

Time: daily ticks with speed control; a separate continuous-time battle view.

## Decision

The Dupuy-style ratio with Biddle multipliers as the default, Helmbold as the lab comparison lane
(owner, 2026-10-28). Daily ticks with speed control, no continuous battle view (owner,
2026-10-28); crises may still switch to the real-time pacing of 0132.

- **Segments.** A front segment is a run of contested cell edges matched to the frontage of the
  formations engaged, about 5 to 20 km. Each resolves once per day.
- **Combat power.** P = S · V · Q · E. S sums equipment firepower by class, weighted by
  availability and the target mix. V carries posture, terrain, weather, season, rivers, urban
  share (GHSL), fortification and mines. Q carries training, cohesion, morale, leadership, supply
  state and force employment. E carries air superiority, ISR advantage, EW and fires integration.
- **Exchange.** With R = P_att / P_def: attacker loss rate a₀ · R^−γ · intensity, defender loss
  rate d₀ · R^δ · intensity, advance v₀ · max(0, R − R*) · mobility · (1 − obstacle), and a
  breakthrough hazard in R, depth of defence and reserves that opens a pursuit state. Noise is a
  keyed lognormal draw; setting it to zero gives the expected-value path used by the nesting test.
- **Modern effects.** Battlefield transparency (a fires penalty on concentrations under high enemy
  ISR, lifted by EW or weather); precision munitions and counter-battery as stocks; mine density
  and fortification as cell fields built by engineers; urban terrain slowing advance to hundreds of
  metres a day; encirclement by flood fill with a surrender hazard; exhaustion lowering cohesion
  until formations refuse attacks or rout.
- **Switch.** A new 17.2 entry selects square, Helmbold, Dupuy-style or stochastic (Markov)
  resolvers; all write the same losses, advance and events.
- **Calibration.** Coefficients are parameter lanes fitted to CDB90 outcomes and COW durations,
  with Karabakh 2020 and Ukraine 2022 (front movement and consumption) as hold-outs under 0107.

## Migration

- P7 stage W4, in `ceteris-war` (combat, front). CDB90 licence to check (0005).

## Tests

- Each resolver reproduces its analytic case on a toy segment.
- The expected-value path is the mean of the noisy path within Monte Carlo error.
- Hold-out cases: simulated front movement within the published range.

## Consequences

Terrain, posture and quality are visible in every result and in the explainability panel (0049).
Modern battle data is sparse and contested, so coefficients carry wide intervals, and the lab keeps
the hazard model until this resolver beats it (0110).
