# 0145. War architecture: clocks, grains and profiles

Status: proposed
Date: 2026-10-28
Design sections: 4.3, 5.3, 11, 12.6, 19.2, 23.5, 23.8; ADRs 0048, 0110, 0115, 0136
Phase: P7 (2.0-beta)

## Context

The owner asked for a war and military system in the Hearts of Iron line, closer to HOI3 than
HOI4 (deep chain of command, order of battle, supply, delegation of theatres to the AI), but more
detailed and realistic, with several interaction surfaces and customization. Breakdown, with
diagrams and estimates: https://claude.ai/artifact/AJ5TQdDUAbesAPsNhYPwEg

The design has the pieces spread out: aggregate capability and Lanchester combat (11.4, 11.5), a
command layer in the extended profile (23.5), a daily sub-tick while at war (4.3, 19.2), a
reduced-form hazard as the lab default (0110), and territory as cell partitions (0115). It has no
single statement of how the war systems fit together, in what order they run, or what each costs.
ADRs 0146 to 0154, 0165 and 0166 fill in the parts.

## Options

1. Keep the aggregate model of 11.5 only; the command layer as later game content.
2. One model at full detail for both lab and game.
3. Three clocks and two profiles: the lab keeps the hazard onset and an aggregate exchange; the
   extended profile adds the full operational model, bound to the lab by a nesting test.

## Decision

Option 3.

- **Clocks.** The monthly tick runs the strategic layer: crisis bargaining, war entry and peace
  (11.2, 11.8), mobilization law, budgets, procurement, production lines, training pipelines, war
  finance and war weariness. A daily sub-tick runs while any war is active. Below it, each front
  segment resolves in one closed-form step per day (0149); nothing is simulated per shot or
  vehicle.
- **Grains.** Control, positions, fronts, fortification and mines live on the H3 resolution 6
  cells of 0115. The economy stays on provinces and overlay units and reads control shares.
  Supply moves on a military overlay of the transport graph (0148). Naval forces use the 725 sea
  zones of 5.3; air forces use air zones drawn per theatre (0150).
- **Daily order**, fixed: release orders whose latency has elapsed and run delegated planners;
  sensor search and belief updates; air tasking; naval tasking; supply flow and consumption;
  movement; engagements and control relabels; encirclement by flood fill; morale, cohesion,
  availability, repairs and replacements; events and accumulators for the monthly tick.
- **Determinism.** Every draw comes from a stream keyed by (run seed, war, entity, day, purpose).
  Iteration is by integer ID. Solvers and planners run fixed iteration counts. Theatres that share
  no supply edge and no front may run in parallel; shared quantities are reduced in theatre-ID
  order.
- **Lab and game boundary.** The game writes state only through the intervention list. Every
  order (0147) is an action in the Tier 1a action space (12.6); AI planners write the same order
  objects.
- **Profiles.** Lab: hazard onset and termination (0110), aggregate capability stocks and an
  aggregate exchange per dyad and theatre at the monthly tick, and the war economy (0165).
  Extended: the operational model of 0146 to 0154. The command layer may enter the lab only by
  beating the lab model on the 0107 hold-outs.
- **Nesting test.** A theatre delegated to an average general with default doctrine reproduces
  the lab profile's outcome distribution (23.5).
- **Crate.** `ceteris-war`, with modules force, oob, command, supply, front, combat, air, naval,
  strategic, intel, irregular and mobilize. It reads econ state through declared interfaces; no
  econ crate depends on it.
- **Budget** (estimates; benchmarks replace them): about 7 to 9 ms per day for a large two-front
  war at the 2022 scale and 25 to 30 ms for a world war, so 0.2 to 0.9 s per simulated month at
  war. The supply solver is the only line at risk (0148). The lab profile costs under 1 ms per
  month.

## Migration

- P7, in `ceteris-war`; 0115's cell labels and weight fields land in the pack before P7.
- Build order (stages W0 to W10 in the breakdown): pack preparation; lab war and war economy;
  fronts and irregular war; logistics; order of battle; command; air; naval; intelligence; strike
  and nuclear; modding. W1 and W2 make the 2.0-beta; W4 onward can ship as 2.x minors in the
  extended profile.

## Tests

- Same seed and order list give the same state hash with one thread and with many.
- A run with no war never enters the daily sub-tick, and its state hash equals a build without
  `ceteris-war`.
- Nesting test, as above, within Monte Carlo error.

## Consequences

The war systems have one frame, a cost and an order of work. Interactive speed falls to about one
simulated month per second in a world war, still faster than HOI at top speed. Lab ensembles do
not pay for operational detail.
