# 0074. Settlement hierarchy and urban systems

Status: proposed
Date: 2026-10-24
Design sections: 5.1, 5.2, 6.1, 7.2 (agglomeration), 8.7 (housing)
Phase: P4

## Context

Urbanization of Africa and South Asia is one of the defining processes of 2000-2100: it moves
fertility, crime, disease, emissions, heat exposure, protest geography and the cost of
services. The design has an urban share per province and agglomeration in productivity, but no
settlement classes inside provinces, so a province of one megacity and a rural hinterland looks
like a uniform average. City sizes follow Zipf's law, congestion offsets agglomeration, and
slums form where housing supply and tenure fail (Marx, Stoker and Suri 2013). The EU's Global
Human Settlement Layer classifies settlements by the UN Degree of Urbanisation (cities, towns
and semi-dense areas, rural areas) from 1975 to 2030, with projections to 2100, openly.

## Options

1. Status quo: one urban share per province.
2. Three settlement classes per province (Degree of Urbanisation), with pops split by class,
   Zipf-consistent city growth with congestion, and a slum share in cities.
3. Explicit city agents.

## Decision

Option 2.

- Each province's pops are split across the three classes; rural-to-urban moves within a
  province run through the migration model with short distances.
- Cities grow with agglomeration (A5) net of congestion costs (housing, commuting, pollution)
  (P126); the province's largest city is tracked as a named feature for the map.
- Slum share in cities rises when housing supply (0040) lags urban growth and tenure is
  insecure (0068) (P127); slums raise disease, crime and heat exposure and lower service
  coverage.
- Fertility, mortality, crime, heat (0077) and pollution (0038) read the class mix.

## Migration

- P4; class shares for 2000 come from GHSL in the pipeline.
- Data: GHSL settlement layers and Urban Centre Database (free and open; confirm terms in ADR
  0050), UN-Habitat slum estimates (terms to audit).

## Tests

- Simulated urban shares and built-up growth 2000-2020 by country, out of sample after 2010,
  within tolerance.
- The city-size distribution stays close to Zipf in ensembles.

## Consequences

Urbanization becomes a mechanism with its consequences, at the cost of three classes per
province.
