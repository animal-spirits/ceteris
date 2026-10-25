# 0063. Addiction epidemics

Status: proposed
Date: 2026-10-24
Design sections: 6.2, ADR 0033, ADR 0060
Phase: P6 (with the drug-route model)

## Context

The US overdose epidemic moved in supply waves: prescription opioids, then heroin, then
fentanyl. Deaths reached about 110,000 in 2023 and then fell by about 27% to about 80,000 in
2024 (CDC/NCHS provisional counts), a drop whose causes are still debated (supply changes,
naloxone, treatment, cohort depletion). ADR 0033 holds "deaths of despair" as a scenario
hazard; ADR 0060 adds drug routes. An epidemic model connects them.

## Options

1. Keep the scenario hazard only.
2. An addiction stock per province with initiation, recovery and death, driven by supply
   potency and prices from the drug-route model and by treatment and harm-reduction policy.

## Decision

Option 2, as an SIR-like model per province and age band: initiation rises with availability
and falls with price; potency shocks (a fentanyl-type wave) raise death rates; naloxone,
treatment and prescribing rules are levers (P108). Deaths enter the injury and despair
components of 0033. Kept only if it passes ablation; otherwise it moves to the extended
profile.

## Migration

- P6, alongside 0060.
- Data: CDC WONDER mortality (public domain); UNODC World Drug Report estimates (terms to
  audit).

## Tests

- A replay of the US 1999-2024 series reproduces the three waves and a decline after 2023 when
  supply shocks reverse; provisional counts are revised, so final counts are the target.

## Consequences

Drug policy becomes a lever with mortality, labour supply and crime effects.
