# 0131. Deep history: economic reanalysis, agrarian worlds, period advisers, long runs

Status: proposed
Date: 2026-10-28
Design sections: 15, 23.3, 23.7, 23.9; ADRs 0086, 0103, 0115
Phase: 3.x (reanalysis may start after P9)

## Context

Pre-1995 packs are planned for 3.x and need a historical hindcast before they leave the
extended profile (23.3). Weather science built its hindcasts by reanalysis: assimilating sparse
observations into a model to rebuild a consistent past (ERA5, the 20th Century Reanalysis). The
same approach gives Ceteris its pre-1995 world and a dataset of independent value. Brainstorm:
https://claude.ai/artifact/N9zi5kznSaqvjg9ztw4WMT (ideas 48, 49, 54, 55 and 59).

## Options

1. As planned in 23.3: imputed pre-1995 packs, extended profile only.
2. Reanalysis first, then historical packs built on it, with the mechanisms earlier eras need.

## Decision

Option 2.

- Economic reanalysis 1870-2000: an ensemble Kalman or particle filter (15) assimilates
  Maddison, HYDE, Clio-Infra, trade and price series into the model and outputs a gridded,
  consistent reconstruction at province grain on the H3 cells of ADR 0115, with uncertainty
  bands. Released as open data with its manifests.
- Agrarian packs: start dates before industrialization (for example 100 CE or 1100) with
  Malthusian feedback, tribute and corvée, slavery as a labour institution, caravan and sail
  trade and plague. Structural-demographic dynamics (Turchin) are a module tested against the
  Seshat databank (licence to check).
- Period-correct advisers: in historical starts, adviser forecasts use the economic doctrine of
  the date (no Keynesian stabilization before 1936, no inflation targeting before 1990). The
  world runs on the model regardless; heterodox choices cost political capital.
- Collapse and recovery: capability milestones (23.7) can decay when the institutions or
  capital holding them fall below thresholds; rediscovery is slower than first discovery.
  Enables runs to 2500 that test long-horizon stability.
- Era-spanning campaign: one save from a historical start through 2000 into the open future,
  with the 2000 data pack as a checkpoint that reports drift from observed history.

## Migration

- After P9: reanalysis prototype on 1950-2000. 3.x: packs, modules and campaign.

## Tests

- Reanalysis held-out series (withheld from assimilation) fall within its reported bands at
  nominal coverage.
- A run that crosses the 2000 seam conserves all ledgers.

## Consequences

Earlier eras get a defensible empirical base, and the project produces a reconstruction
historians can use without playing. It is the largest item in this batch.
