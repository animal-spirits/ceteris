# 0189. Emergent organizations

Status: proposed
Date: 2026-10-28
Design sections: 10.4, 10.5, 10.7, 10.8, 11.6; ADRs 0046, 0056, 0058, 0060, 0069, 0159, 0186
Phase: P6 (2.0-alpha)

## Context

Section 10.4 fixes the interest groups at about ten per state, from a closed list (business by
sector, finance, organized labour, farmers, security forces and so on). The list is right for
2000 and wrong for 2020: unions, movements, churches, militias and lobbies appear, split, merge
and die after 2000, and the ones that matter are often ones no list anticipated (the Brazilian
MBL, the gilets jaunes, Hizb ut-Tahrir's local branches, the Wagner Group). With a closed list,
a fuel-price shock can raise grievance and protest (10.8) but cannot produce a transport union
the government must then repress, co-opt or concede to. Parties (10.5), protest leaders in the
career pools (0186), criminal groups (0060) and armed actors (0069) each need a source of new
collective actors, and today each would invent its own.

## Options

1. Status quo: the closed list of 10.4, with membership shares drifting.
2. One organization primitive. The 2000 interest groups are its initial instances; new ones
   spawn when latent potential in a population slice crosses a threshold, and then grow, split,
   merge and decay under one set of rules. Parties, criminal groups and armed actors are
   organization kinds.
3. Scripted spawns: data files that name organizations and the conditions that create them.

## Decision

Option 2. Option 3 puts composition by hand, which the design rules out (0186 applies the same
argument to people).

**Latent potential, always on.** Per (state, pop cell, policy axis of 10.5), vectorized:

```
potential = grievance * homogeneity * density * (1 - mobilization_cost)
mobilization_cost = f(trust deficit (0056), repression, civil society density (0056),
                      prior organization on the axis, 1 / information exposure (0046))
```

Grievance is the axis share of 10.7's approval gap, weighted by identity salience (0058).
Homogeneity is the inverse of preference variance within the slice; density is the slice's
share of its provinces' population, urban cells counting more.

**Spawn.** The crossing of `potential > theta_spawn` is solved analytically within the tick and
queued as an event, so only cells near the threshold cost anything. At the crossing a leader is
drawn from the slice's protest-leader or business-owner pool (0186), and the kind follows the
slice's dominant resource base:

| Resource base | Kinds |
| --- | --- |
| Selective incentives | Union, patronage network, guild |
| Material interest | Lobby, business association |
| Ideology or identity | Movement, church, party precursor |
| Coercion | Militia, criminal group (0060, 0069) |

Initial resources are `dues * members * compliance`.

**State.** Struct-of-arrays per state, typically 50 to 500 organizations:

```rust
pub struct Orgs {
    pub kind: Vec<OrgKind>,
    pub members: Vec<CellShares>,  // share of each source pop cell
    pub ideology: Vec<AxisVec>,    // position on the 10.5 axes
    pub demands: Vec<LeverVec>,    // target policy settings
    pub money: Vec<f64>,
    pub arms: Vec<f64>,
    pub legitimacy: Vec<f64>,
    pub cohesion: Vec<f64>,        // 1 / internal preference variance
    pub relation: Vec<StateRelation>, // tolerated, repressed, co-opted, captured
    pub leader: Vec<PersonId>,
    pub visible: Vec<bool>,
}
```

**Dynamics, monthly.**

- Growth: `d members = recruit * (appeal - cost) * (1 - saturation)`. Appeal is ideological
  alignment plus the value of selective benefits; cost rises with repression and falls with the
  organization's past wins.
- Free riding: organizations providing only public goods grow only while identity salience is
  high or they offer selective benefits, so unions persist and diffuse movements burn out
  (Olson 1965).
- Resources: `d money = dues * members - spending - seizure`. The leader splits spending across
  lobbying, protest, services, arms and saving by one-step lookahead on its utility.
- Splits: when cohesion falls below `theta_split`, a child spawns along the principal axis of
  internal variance; splinters start more radical than the parent.
- Coalitions: organizations closer than `d_merge` with a shared opponent form an alliance with
  the same cross-body primitive as treaties (0047); sustained alliances merge.
- Decay: when grievance on its axis goes stale or its leader exits without a successor, an
  organization returns to latent potential; seizure can also end it.

**Interfaces.**

- Power: organizations replace the fixed list in 10.4's contest success function, with
  `x_g = money^a * (members * cohesion)^b * arms^c`.
- Parties: a party precursor enters elections when the unrepresented-mass test of 10.5 holds
  and ballot access allows; this is 10.5's party entry, not a second rule.
- Protest: an organized slice lowers the Granovetter-Kuran threshold of 10.8 by giving a known
  coordination point, so protests become rarer but larger and more sudden.
- State response: ignore, concede (policy edit toward `demands`), co-opt (patronage transfer,
  raising corruption per 0046) or repress (coercion capacity, legitimacy cost, defection hazard
  in the security services). Each choice updates `relation` and is a logged event with causes,
  so situations (0159) and the chronicle can cite it.
- Observation: an organization starts hidden when civil liberties are low and surfaces when
  its resources or actions cross a visibility threshold, through the observed layer (0065).

**Initial state.** The 2000 interest groups of 10.4 are the initial organizations, with kind
from the closed list and membership from the same pop rules. Movements and armed groups active
in 2000 are seeded from V-Dem's civil society and party data, UCDP actors and EPR group
organizations where they exist, and are absent elsewhere.

**Determinism.** Spawn and leader draws use streams keyed by (state, cell, axis, purpose), per
14.4, so branches keep the same organizations until their histories differ.

## Migration

- P6, in ceteris-polity, after pops carry grievance and ideology. Criminal and armed kinds
  attach in P7 with 0060's capture link and 0069.
- Data: V-Dem civil society and V-Party (CC BY-SA 4.0), UCDP actor lists (CC BY 4.0), EPR (CC
  BY-NC-SA, acceptable under the non-commercial decision). Calibration targets: union density
  (ILOSTAT, OECD/AIAS ICTWSS), counts of new parties reaching parliament (P34), NAVCO campaign
  onsets and their violent share (Chenoweth).

## Tests

- With `theta_spawn` at infinity and splits off, results equal the fixed interest-group list.
- Fuel-price shock: a toy state with a transport-worker cell and a fuel price raised 50%
  spawns a union within 12 months; conceding ends it within 24, co-opting raises measured
  corruption, repressing raises the splinter rate.
- A public-goods grievance with low salience and no selective benefits does not sustain an
  organization past 24 months.
- No explosion: across the 2000-2020 ensemble, organizations per state stay within a band set
  from the calibration targets, and none persists after its axis grievance has been zero for
  ten years.
- Historical face validity, scored on hazards, not events: in branches started from 2000,
  organizations appear on the axes and in the countries of Brazil 2013-2016, Tunisia and Egypt
  2010-2011 and France 2018 at above-baseline rates.

## Consequences

New collective actors come from the population instead of a list, so a run in 2030 has
organizations nobody wrote. Parties, protest, crime and insurgency share one source of actors,
which removes four ad hoc spawn rules but makes politics depend on one more threshold to
calibrate. The 50 to 500 organizations per state are cheap; the cost is calibration. Cost about
8-12 part-time weeks after P6's pops and approval.
