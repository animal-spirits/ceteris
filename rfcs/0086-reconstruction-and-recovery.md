# 0086. Reconstruction and recovery

Status: proposed
Date: 2026-10-24
Design sections: 7.4, 11.7, 13.4, ADR 0075
Phase: P7, P8

## Context

Disasters and wars destroy capital and housing (11.7, 13.4); what happens next decides the
long-run cost. Recovery is often incomplete: tropical cyclones lower national income for more
than a decade with no rebound to trend (Hsiang and Jina 2014), while some reconstructions are
fast when construction capacity, finance and aid are available. Reconstruction competes for
construction labour and materials, raises local prices, and depends on insurance (0051) and
aid. The design destroys capital but leaves rebuilding to ordinary investment, without its
bottlenecks.

## Options

1. Status quo: rebuilding through ordinary investment.
2. Reconstruction demand as a priority claim on construction capacity, financed by insurance,
   public budgets, aid and borrowing, with recovery speed limited by construction capacity,
   repair crews (shared with 0075) and governance.

## Decision

Option 2. Destroyed capital and housing create a reconstruction backlog per province; the
construction sector serves it first, at rising costs when its capacity binds, and imports
construction services where possible (P141). Financing comes from insurance payouts, public
budgets, aid (0070) and borrowing. Governance and capacity (0067) set leakage and speed.

## Migration

- P7 and P8, in ceteris-econ.
- Validation: post-disaster income paths (Hsiang and Jina 2014) and post-war reconstruction
  episodes.

## Tests

- A simulated large cyclone reproduces a persistent income gap of the published order when
  finance and capacity are limited, and a faster recovery when they are not.

## Consequences

The long-run cost of disasters and wars depends on preparedness, finance and capacity, as
observed.
