# 0183. Characters and the real 2000 roster

Status: proposed; amended by ADR 0253 (kin edges, houses, claims)
Date: 2026-10-28
Design sections: 2.4, 10.6, 13.5, 14.4, 15.7, 23.5; ADRs 0066, 0078, 0107, 0132, 0136, 0147, 0154
Phase: P6 (2.0-alpha), extended profile; lab candidate at 4.0 (ADR 0136)

## Context

DESIGN 23.5 makes characters generated people with generic names, and ADR 0066 generates
leaders without names. A start on 1 January 2000 reads far better with the real incumbents,
and their records are public (Wikidata, WhoGov, Archigos, REIGN). The owner chose real names
for the 2000 roster on 28 October 2026. Proposal page with a portrait demo

## Options

1. Fully generated characters; real records only as validation.
2. Real public facts in 2000, generated names (DESIGN 23.5 as written).
3. A real 2000 roster with real names, and generated people after.

## Decision

Option 3.

- **Record.** One character record for every person the model tracks: birth date, sex, birth
  cell and ancestral cell, offices held, observables and latent traits (ADR 0184), memory
  (ADR 0185) and career state (ADR 0186). Generals of ADR 0147 use the same record.
- **Roster.** Incumbents on 1 January 2000: heads of state and government, cabinets, central
  bank governors, party leaders and service chiefs. Positions and dates from Wikidata (CC0),
  cabinets from WhoGov, entry manner and earlier tenures from Archigos and REIGN.
- **Bench.** People in public life in 2000 who could rise: national legislators, governors,
  capital mayors, deputy ministers. Every state's bench is filled to the same size with
  generated people, so better documentation does not become political advantage.
- **After 2000.** No real dates fire. Real people age under the life tables of 13.5 and leave
  office by the hazards of ADR 0066. Real post-2000 careers are validation targets (ADR 0107),
  never inputs. Everyone who enters public life later is generated (ADR 0186).
- **Display.** The extended profile shows real names. The lab shows role labels ("Head of
  government, Brazil"). A run-level "Real names" switch replaces every real person with a
  generated stand-in carrying the same public facts; shared timelines (ADR 0132) carry the
  setting and viewers can change it.
- **Safeguards.** Real people get no generated likeness (portrait ADR to follow). Chronicle
  exports and postcards (ADR 0144) carry a counterfactual-run line. Acts under ADR 0154 by a
  real person are worded as events of this run.

## Tests

- Every state has a head of government, a head of state and a cabinet on the start date.
- The roster matches Wikidata and WhoGov for a fixed list of 30 states.
- Switching "Real names" off changes no state hash.

## Consequences

The start is recognisable and the leader time scale of 0066 gets real people. The risk is
showing real living people doing things they never did; the switch, the lab labels and the
no-likeness rule are the guard. Cost about 5-7 part-time weeks, mostly the roster import.
