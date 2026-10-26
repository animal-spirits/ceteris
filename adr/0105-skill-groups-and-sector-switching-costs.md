# 0105. Skill groups and sector switching costs

Status: proposed
Date: 2026-10-26
Design sections: 6.3, 7.2, 8.7, 15.5; econ SPEC Labour market; amends ADRs 0089, 0093

## Context

The P2 labour market has one wage per province. Education levels are perfect substitutes in
effective labour (L^s = sum_k e_k LF_k), and labour moves between sectors within a province in
the same tick.

- **Perfect substitution** removes the skill premium. Trade and technology cannot change wage
  inequality, skilled migration has no wage incentive of its own, and DESIGN 6.3's education
  dynamics have no price signal.
- **Instant sector mobility** sets reallocation costs to zero. The China-shock evidence (Autor,
  Dorn and Hanson 2013) and sector-switching estimates (Artuc, Chaudhuri and McLaren 2010) show
  large, persistent local losses from import competition because workers do not move between
  sectors freely. The model understates the losses and the persistence of trade shocks, and
  adjusts to any policy experiment too fast. This is a P3 problem: P3 natural experiments
  (15.5) include the China shock.

## Options

1. Keep one wage.
2. Two skill groups with a CES between them, and sector-specific wages with a logit
   reallocation rate.
3. Dynamic discrete choice over sector and location (Caliendo, Dvorkin and Parro 2019).

## Decision

Option 2, as two independent switches so ablation can drop either.

- **Skills.** Two groups: high (tertiary) and low (the rest). Effective labour in VA is
  L = (alpha L_h^((s-1)/s) + (1 - alpha) L_l^((s-1)/s))^(s/(s-1)) with s = 1.6 (Katz and Murphy
  1992; Ciccone and Peri 2005; literature, not free) and alpha calibrated per country to the
  2000 skill premium (ILOSTAT or SEDLAC earnings by education, regional means where missing).
  Each province has two wages.
- **Sector switching.** Workers belong to a sector. Each month a share m of workers in each
  sector re-chooses by logit over sectors in the province, on expected wages, with dispersion
  omega_s (not nu, the border friction P05); m and omega_s from Artuc, Chaudhuri and
  McLaren (2010), moved to monthly frequency
  (literature, not free; one lane over their confidence range). Sector wages clear sector
  labour markets; the wage rule of the econ SPEC applies per (province, sector, skill).
- **Natural rate.** ADR 0089's wedge applies per segment with the country's u*: the wage rule per
  (province, sector, skill) reads x + u*_i, and the frictionless search (ADR 0093) finds each
  segment's root of L*(w) = (1 - u*_i) L^s on uncapped targets.
- **Education efficiency.** P42's e_k weights the education levels inside each skill group
  (secondary against below secondary in the low group); the CES applies between the groups.
- With both switches off, the current model is reproduced exactly.

## Migration

- Econ: wage state widens from province to (province, sector, skill); the closed-form L* per
  enterprise is unchanged given its wage.
- Pack: skill premium per country; tertiary shares from WIC are already there.
- Budget: wage state grows 2 x 10 times; the labour step is not on the critical path (0097).

## Tests

- Switches off: golden hashes unchanged.
- A sector demand shock in a toy province: wage gap closes at the closed-form logit rate.
- In P3, the China-shock replication (15.5) reports local employment effects with the switch on
  against Autor-Dorn-Hanson.

## Consequences

- Trade shocks have local, persistent incidence; the skill premium responds to trade and
  technology.
- Two literature parameter sets enter; parameter lanes (ADR 0107) carry their uncertainty.
