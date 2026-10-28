# 0146. Forces, order of battle and the forces of 2000

Status: proposed
Date: 2026-10-28
Design sections: 11.4, 23.5, 23.7; ADRs 0032, 0048, 0105, 0145
Phase: P7 (2.0-beta)

## Context

11.4 holds forces as aggregate capability stocks; 23.5 organizes them into formations with HQs and
templates. Neither fixes what a formation holds, which unit is the smallest that moves and fights,
or where the forces of 1 January 2000 come from.

## Options

Smallest unit: brigade everywhere; battalion everywhere (about three times the formations and
planner cost); brigade with battalions for small actors.

Initial forces: open sources and priors only; an optional importer for IISS Military Balance.

## Decision

Brigade, with battalions for small states and armed groups (owner, 2026-10-28). Open sources and
priors only, with no IISS importer (owner, 2026-10-28).

- **Equipment catalog.** About 45 classes in seven families: ground manoeuvre, fires, air
  defence, air, uncrewed, naval, enablers. Each class is a data row: firepower against soft,
  armoured, air and naval targets; protection; mobility by road class and terrain; fuel and
  ammunition per day by posture; crew; mean time between failures and repair hours; signature;
  unit cost and input vector into the IO sectors; vintage band and producer. Counts are held per
  formation by class and vintage band, not per item.
- **People.** Five tiers (conscript, contract, NCO, officer, specialist) with training times and
  pop sources (0165). Specialists such as pilots and air-defence crews bind before equipment does.
- **Echelons.** Battalion (small actors only), brigade, division, corps, army, front or theatre.
- **Templates.** Authorized counts per class and tier plus enablers. Fill rate against the
  template drives effectiveness. Restructuring costs readiness for months.
- **Readiness.** Cadre, reserve, ready, deployed; moving up takes weeks to months and needs
  people, equipment and fuel to arrive.
- **Derived values.** Combat power, supply need, lift need, signature and frontage are computed
  from contents, never stored.
- **Forces of 2000.** For about 190 states, from national personnel and spending (COW, SIPRI),
  inventories from SIPRI arms transfers, and published orders of battle where they exist; the
  rest imputed by template priors conditional on spending and doctrine family. About 30 large
  militaries are curated by hand. Wars running on 1 January 2000 (Chechnya, the Congo, Ethiopia and
  Eritrea, Sri Lanka, Colombia, Sierra Leone, Kashmir and others) start with forces in contact.

## Migration

- P7 stage W4, in `ceteris-war` (force, oob) and the pack (catalog, templates, OOBs).
- Data: SIPRI milex and arms transfers, COW NMC (terms to audit in 0005); citation-only sources
  enter as priors with ranges.

## Tests

- Every state's 2000 forces load; national personnel and spending totals match their sources
  within tolerance.
- A template edit changes derived values and no stored state.

## Consequences

Forces are explicit and moddable without per-item tracking. The 2000 order of battle is the
largest data task of the war module; its accuracy is bounded by open sources.
