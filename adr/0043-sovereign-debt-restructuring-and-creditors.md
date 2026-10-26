# 0043. Sovereign debt restructuring and creditor structure

Status: proposed
Date: 2026-10-24
Design sections: 9.4, 9.10, 9.11, 11.8
Phase: P5

## Context

The design makes default a strategic choice priced consistently with the state rule (9.4), with
market exclusion and haircuts as consequences, but does not say how a default is resolved.
Outcomes after default depend on the resolution: haircuts range widely and higher haircuts
predict longer exclusion and higher later spreads (Cruces and Trebesch 2013); pre-emptive
restructurings are shorter and costlier for creditors than post-default ones (Asonuma and
Trebesch 2016); creditor composition has shifted toward official and Chinese lenders, much of
it unreported (Horn, Reinhart and Trebesch 2021), which changed resolution under the G20 Common
Framework (Zambia, Sri Lanka, Ghana).

## Options

1. Status quo: a fixed haircut and exclusion period.
2. An event-driven restructuring negotiation between the state and creditor classes, with
   haircut and timing from a bargaining rule, official and Chinese creditors as separate
   classes, collective action clauses, and IMF programmes with conditionality.
3. A full dynamic bargaining model.

## Decision

Option 2.

- Creditor classes per state: bondholders by currency and law, domestic banks, Paris Club,
  China, multilaterals. Stocks come from the who-to-whom matrix of 9.8.
- After default (or pre-emptively when the default hazard is high), a negotiation runs monthly:
  the haircut follows a bargaining rule on the state's capacity to pay and creditor outside
  options (P79), with holdouts reduced by collective action clauses. Multilaterals are senior
  and take no haircut.
- Market exclusion lasts until agreement plus a reputation period that rises with the haircut
  (P80).
- IMF programmes lend into arrears under conditionality (fiscal targets), already sketched in
  9.4.

## Migration

- P5: the negotiation as an event-driven system in ceteris-finance.
- Data: Cruces-Trebesch haircut data and Horn-Reinhart-Trebesch Chinese lending data (licences
  to audit, ADR 0050); World Bank International Debt Statistics (already in the manifest,
  wb-ids).

## Tests

- Simulated haircut distribution and default durations within tolerance of the Cruces-Trebesch
  database.
- Post-default output paths follow the published pattern (deeper losses with larger haircuts).

## Consequences

Default counterfactuals (Argentina 2001, Greece 2012, Zambia 2020) gain their resolution stage,
and creditor geopolitics becomes visible.
