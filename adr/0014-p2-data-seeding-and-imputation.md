# 0014. P2 data seeding, downscaling and imputation

Status: accepted (owner, 2026-10-23, with `specs/econ/SPEC.md`)
Date: 2026-10-23
Design sections: 5.2, 6.1, 7.1, 7.2, 8.5, 15.1, 16, 16.1, 22.2

## Context

The owner chose full 2000 data for P2 (ADR 0012: full globe). Section 16.1 sketches
downscaling national ICIO output to provinces and imputing ICIO coefficients for economies
outside ICIO; 22.2 leaves the imputation model and sector splits to the agent. Constraints
from the licence audit (ADR 0005): GADM geometries may not be used, Teti's tariff database is
rejected, WTO tariff data is closed-source only.

## Options

Imputation of IO structure outside ICIO: (1) ICIO's rest-of-world block; (2) nearest ICIO
economy in region and log GDP per capita; (3) regional averages adjusted by income (16.1).

Province output: (1) gridded GDP only; (2) sector-specific proxies with gridded GDP and
population as fallbacks.

Tariffs: (1) none in P2; (2) importer x {primary, manufactured} applied rates from WDI, zero
inside agreements; (3) WTO IDB product detail.

## Decision

- Goods: 10 energy-forward goods aggregated from ICIO 2025 edition industries
  (`specs/econ/SPEC.md`); coal split from oil and gas by the edition's own split if present,
  else by EIA 2000 production values at Pink Sheet prices.
- IO outside ICIO: option 2, nearest donor; national VA by good from WDI sector shares, split
  within groups by the donor's proportions.
- Province VA for traded goods: option 2. Agriculture by MapSPAM 2000; oil and gas by GEM
  extraction assets; coal and mining by GEM coal mines and P1 mineral sites; manufacturing by
  DOSE where a region matches a province by ISO 3166-2 code or exact name (no GADM geometry),
  else Kummu gridded GDP; population as the last fallback. Local goods get a population-based
  base point only. Provinces under 1,000 working-age people get no VA.
- Population from WorldPop 2000 through the H3 cells; education from the WIC reconstruction
  (Barro-Lee as cross-check); participation and unemployment from ILOSTAT; labour share from
  PWT 11; weight per value from BACI HS92 2000.
- Tariffs: option 2, from WDI weighted applied rates and CEPII gravity and WB DTA agreement
  flags.
- Trade imbalances are not seeded: P2 has balanced trade until finance (P5).
- Productivity is backed out by one joint GE solve against the downscaled VA
  (`econ_calib.arrow`), with residuals reported.

## Consequences

- Every pack row carries an imputation flag; P9 down-weights imputed rows.
- The donor rule is coarse for economies far from any ICIO member (small islands, some of
  Sub-Saharan Africa); regional averaging (option 3) remains the upgrade path.
- Tariff detail by product and preferential margins wait for an open bilateral source; until
  then tariff studies (P3) use the two-group rates.
- DOSE coverage depends on the code and name match rate, reported by the pipeline.
