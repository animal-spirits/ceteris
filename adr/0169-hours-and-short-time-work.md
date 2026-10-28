# 0169. Hours worked and short-time work

Status: proposed
Date: 2026-10-28
Design sections: 6.3, 8.7, 15.3 (V2), 15.4; ADR 0105
Phase: P4 or P5

## Context

Labour adjusts only through headcount and labour status (6.3, P41). There is no hours margin,
so labour hoarding and short-time work cannot happen. In 2009 Germany kept unemployment nearly
flat through Kurzarbeit while output fell about 5% (Balleer, Gehrke, Lechthaler and Merkl
2016), and in 2020 European job-retention schemes kept unemployment low while US unemployment
reached about 15%. Unemployment is a headline 15.4 target, and Okun's law is a V2 fact.
Gap audit (2026-10-28): https://claude.ai/artifact/HRDQVbjJz8MrcDuPphPyYs

## Options

1. Status quo: headcount only.
2. Hours per worker as a second adjustment margin, with a short-time-work subsidy lever.

## Decision

Option 2.

- Each (province, sector) segment carries hours per worker, starting at the country's PWT
  average hours (already in the manifest). Enterprises adjust hours first, at an adjustment
  speed faster than P41, and headcount when the hours gap persists.
- Short-time work: a state lever subsidizes a share of lost hours' wages, paid from the fiscal
  account (9.6), with eligibility rules taken from the 2009 and 2020 schemes. It lowers the cost
  of keeping workers relative to layoffs.
- The labour income of a pop is wage times hours times employment, so PIGL and the consumption
  rule see the loss.

## Migration

- P4 or P5: an hours array per wage segment (0105) and the subsidy lever in the fiscal block.
- Pack: PWT average hours by country.

## Tests

- With hours fixed at their initial value and no subsidy, golden hashes are unchanged.
- A toy demand shock with the subsidy on lowers unemployment and hours together, with the
  ordering Germany 2009 versus the US 2009.
- Okun's coefficient by country 2000-2010 within the published range.

## Consequences

The 2009 and 2020 labour-market divergences become reachable, and job-retention schemes become a
policy experiment. Register: 1 entry (hours adjustment speed).
