# 0254. Organic conflict onset: crisis hazard, actors decide

Status: accepted (owner, 2026-10-30)
Date: 2026-10-30
Design sections: 6.5, 10.8, 11.1, 11.2, 11.6, 11.8, 13.5, 13.6, 15, 23.6; ADRs 0051, 0069, 0073, 0110, 0136, 0145, 0189, 0241, 0242
Phase: P5 pack (t0 conflict state), P8 and P9 (1.0 conflict incidence), P6 (cohesion, attribution), P7 (crisis generator, intervention, cross-border groups)

## Context

ADR 0241 makes default runs calendar-blind, so every war after 1 January 2000 must emerge from
simulated state. A survey of conflicts from 2000 to 2025 (research page finds ten causal pathways: opportunistic
revision (Georgia 2008, Ukraine 2014 and 2022, Karabakh), revolution to civil war (Syria, Libya,
Yemen, Myanmar), splits inside the security forces (Sudan 2023, South Sudan 2013), exclusion
insurgency (Darfur, ISIS 2014, Tigray), cross-border diffusion (Libya to Mali), attack to
retaliation (Afghanistan 2001, India-Pakistan 2019 and 2025), third-party intervention (Syria,
Yemen, Mali, Libya), frozen-conflict thaw (Karabakh 2020), criminal war (Mexico, Haiti) and
preventive war (Iraq 2003, Israel-Iran 2025). Onsets almost always pair a fast political trigger
with slow structural risk; most are recurrences; sponsors set duration; collapses are sudden.

The design covers some of this (11.2, 11.6, 10.8, 0069, 0073, 0110) and has seven gaps:

1. No cohesion or factions in security forces, so defection cascades, praetorian splits and sudden
   collapses (Kabul 2021, Damascus 2024) cannot happen.
2. No rule for third-party intervention; sponsors exist only as money in 0069.
3. Armed groups belong to one state; no branches across borders, no arms leakage after collapse.
4. Attacks (0069) carry no origin, so they cannot cause retaliation.
5. 0110's hazard says where violence starts but not who fights or why; 11.2 says why but has no
   rate at which disputes arise.
6. The pack holds no conflict state at t0.
7. War is 2.0 (0136), so 1.0 runs have no war shocks to oil, refugees and capital.

## Options

D1, conflict in 1.0:

1. Reduced-form conflict incidence with economic effects only.
2. None in 1.x; wars only through replay.
3. Move P7's lab profile into 1.0.

D2, onset in 2.0:

1. A crisis hazard creates crises with actors and a stake; actors decide escalation; aggregate
   onset rates nest the 0110 hazard.
2. The 0110 hazard places wars; actors are assigned afterwards.
3. Structural bargaining (11.2) only.

D3, recording: one ADR for all mechanisms; an ADR for t0 state and 1.0 only; research page only.

## Decision

Option 1 for D1, D2 and D3 (owner, 2026-10-30).

**Layers.** L0 structural risk is state already computed (income, growth, exclusion, state
capacity and reach, terrain, rents, youth unemployment, peace-years, neighbourhood conflict; per
dyad claims, capability ratio and trend, alliance credibility, trade dependence, nuclear status,
regime). L1 triggers are events from other hazards (coups and protest cascades of 10.8, leader
death of 0066, subsidy cuts forced by fiscal stress, attributed attacks, a neighbour's collapse, a
sponsor's withdrawal). L2 is the crisis generator below. L3 is escalation. L4 is conduct and
termination as in 0145 to 0154 and 11.8. L5 is aftermath: peace-years reset, ceasefire lines
become frozen conflicts with 0073's thaw hazard, groups keep their rents, and grievance, destroyed
capital and refugees feed L0. No country, group or date is named anywhere in the rules.

**Crisis generator (L2).** One hazard in the 0051 library creates crises, not wars. A crisis has
typed actors (state, faction, armed group, sponsor) and a stake (territory, regime, autonomy,
policy, retaliation). Its rate is the L0 risk times multipliers for L1 triggers in the past
months, fitted on MID and ICB crises (interstate) and UCDP minor-conflict onsets (domestic), drawn
by thinning on shared uniforms (0241).

**Escalation (L3).** In the extended profile actors bargain under 11.2; third parties run the
intervention rule; factions run the cohesion rule. Outcomes are settlement, standoff, limited clash
or war. In the lab profile an escalation logit on the same covariates replaces bargaining.

**Nesting.** With default parameters, onset rates by type and region from L2 and L3 together
match the 0110 hazard within its interval. 0110 stays the benchmark the structural layers must not
move.

**M1, security-force cohesion (P6).** Each state's forces are one to a few factions (regular army,
guard, paramilitary), each with an identity profile and a loyalty stock. Loyalty falls with pay
arrears, orders to repress the faction's own group, losses and leader illegitimacy, and rises with
shared rents and sponsor support. Defection is a threshold cascade, the 10.8 protest model. A split
creates an armed actor with its units and the cells it holds (0245). Coup-proofing lowers coup risk
and raises split risk (Roessler 2016). Calibrated on Powell-Thyne and Cline coups and defection
episodes.

**M2, third-party intervention (P7).** Each month, states with a stake in a crisis or war score
modes (arms and money, advisers or private forces, air power, ground troops) by value (client tie
of 23.6, basing, kin, refugee inflow, attacks originating there, proliferation risk, commodity
exposure) against cost (projection distance, casualty sensitivity by regime type, sanctions,
other commitments). Sponsor budgets are shared across theatres, so overstretch in one war can
collapse a client in another. Calibrated on UCDP External Support and International Military
Intervention data.

**M3, cross-border armed groups (P7).** A group may open a branch in an adjacent province with low
state reach (0163) and a kin or ideological link. When a state collapses, a share of its arms
stock (0146) leaks to groups within reach. Splits and mergers follow 0189. Calibrated on UCDP
actor histories.

**M4, attribution (P6).** Each 0069 attack records its group and host state. A mass-casualty
attack multiplies the target's crisis rate against the host for some months, more with deaths and
rivalry, less under nuclear deterrence.

**M5, conflict state at t0 (pack, P5).** The 2000 pack records conflicts active in 1999 (UCDP),
armed groups and the cells they hold, foreign deployments, ceasefire lines and claims (ICOW). This
is 0241's commitments-in-force rule applied to war. 1.0 reads only what M6 needs (active conflict
and intensity by province).

**M6, conflict incidence in 1.0 (P8, P9).** A reduced-form 0110 hazard per province (onset,
intensity class, duration) with economic incidence only: capital destruction, lost output and
labour, forced displacement under 6.5, oil and mineral supply cuts in affected provinces, freight
premia on nearby chokepoints (0242). No actors, territory or borders change, and the hazard is off
when a 2.0 war module is active. 2.0 replaces it with L2 to L5 and must reproduce its marginal
frequencies.

## Migration

- DESIGN 11.2 and 11.6 gain the layers; 11.1 gains factions and branches; 13.6 notes 1.0 conflict
  incidence; 0136's 1.0 row reads "war conduct" rather than "war" when accepted.
- ADR 0110: the hazard becomes the nesting target and the 1.0 incidence model.
- Pack: UCDP active conflicts 1999 and actor list (already in sources.toml), ICOW claims (licence
  to audit), ceasefire lines, UCDP External Support, IMI, Cline and Powell-Thyne coups (listed).
- `ceteris-war` (0145) gains `crisis`, `cohesion` and `intervene` modules; M6 lives in the 0051
  library in 1.0.

## Tests

- Nesting: simulated onset rates by type and region match the 0110 hazard within its interval.
- Pathway coverage: over 32 members and 25 years from the 2000 pack, each of the ten pathways
  fires at least once and none exceeds three times its historical share.
- Ablation: with attribution off no retaliation onset occurs; with cohesion off no faction split
  occurs; other pathways stay within error.
- With M6 rates at zero, 1.0 golden hashes equal the conflict-free run.
- Frequencies, never dates: onsets per decade by type, internationalized share, duration and
  termination type shares, recurrence share, fatality tail (0051), displacement per battle death,
  scored on 2011-2024 from the 2000, 2005 and 2010 origins.

## Consequences

Every simulated war carries a causal chain from state to trigger to crisis to decision, which the
chronicle (0049) can show. Onset frequencies stay anchored to an empirically scored hazard. 1.0
gains war's economic footprint without politics or territory, at about two to three owner-weeks in
P8 and P9. The pack grows a t0 conflict layer. Cohesion, intervention and diffusion add P6 and P7
scope, estimated at four to six owner-weeks across both.
