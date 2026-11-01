# 0085. Strategic reserves and stockpiles

Status: proposed
Date: 2026-10-24
Design sections: 9.2, 9.6, 12.3, ADR 0028, ADR 0029, ADR 0048
Phase: P5 (oil and grain), P7 (munitions), P8 (medical)

## Context

States hold stocks for crises: strategic petroleum reserves under IEA 90-day obligations
(released jointly in 2022), grain reserves (China, India's Food Corporation), munitions
stockpiles whose depth decided much of 2022-2024 in Ukraine, and medical stockpiles that proved
thin in 2020. Foreign-exchange reserves already exist (9.5). Competitive storage (0029) models
private stocks; public stocks follow different rules and are policy levers with fiscal costs.

## Options

1. Status quo: only private storage and FX reserves.
2. State-held stocks of oil, grain, munitions and medical supplies with acquisition and release
   rules chosen by the planner, as policies with fiscal costs.

## Decision

Option 2. Each state can hold stocks of crude and products, staple grains, munitions and
medical supplies. The planner sets target levels and release rules (release when prices spike
or in war or pandemic), with purchase and carrying costs in the budget (P140). Releases enter
markets as supply; coordinated releases among IEA members are a treaty obligation.

## Migration

- P5 (oil and grain), P7 (munitions, sharing 0048's consumption model), P8 (medical, with
  0061).
- Data: IEA and EIA stock levels (EIA public domain; IEA terms to audit), USDA PSD stocks
  (public domain).

## Tests

- A simulated 2022 oil shock with and without the coordinated release shows the release
  lowering the price peak by an amount within published estimates.

## Consequences

Preparedness becomes a choice with a cost, and long wars and pandemics test it.
