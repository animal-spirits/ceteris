# 0069. Non-state armed actors, insurgency finance and terrorism

Status: proposed
Date: 2026-10-24
Design sections: 11.6, 11.7, roadmap P7, ADR 0060
Phase: P7

## Context

Rebel factions in 11.6 control provinces and are financed by resources, diaspora and sponsors,
without a model of how revenue scales with territory and rents. Mineral price booms raise
conflict in mining areas (Berman, Couttenier, Rohner and Thoenig 2017); extortion, kidnapping
and illicit trade finance insurgencies in the Sahel, Colombia and the Middle East. Terrorist
attacks are rare, fat-tailed events with outsized effects on legitimacy and identity salience.
Private military companies (the Wagner arc) are a contractable force with low political cost
and loyalty risk.

## Options

1. Status quo: rebel finance as fixed parameters.
2. Armed groups as non-state actors with a revenue function over the rents in the territory
   they control or contest (minerals, drug routes, extortion, diaspora, sponsors), attack
   hazards, and contractable private forces.

## Decision

Option 2.

- Revenue per armed group from rents in controlled provinces (mineral output at world prices,
  drug-route flows from 0060, extortion of local output, diaspora and state sponsors) funds
  recruitment and operations (P118).
- Attacks (terrorism, raids) are drawn from the fat-tailed hazard library (0051) with rates
  rising in group resources and grievance; effects run through legitimacy, identity salience
  (0058) and tourism and investment.
- Private military companies: states can contract forces with low casualty cost to the
  principal and a loyalty hazard (principal-agent), as a formation subtype in P7.

## Migration

- P7, in ceteris-war.
- Data: UCDP GED (CC BY 4.0) as the backbone; GTD and ACLED are non-commercial or restricted
  (ADR 0050).

## Tests

- In simulation, a mineral price boom raises conflict incidence in mining provinces with the
  sign and rough size of Berman et al. (2017).

## Consequences

Civil conflict responds to commodity prices, drug policy and sanctions, as it does.
