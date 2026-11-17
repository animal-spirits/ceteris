# Non-commercial data audit for any paid build (#231)

Date: 2026-11-16. Research, not legal advice. Source of truth: `crates/ceteris-pack/config/sources.toml`
(`checked` dates there; licences were not re-read on the providers' pages for this audit).

## Question

Which sources are non-commercial, share-alike-incompatible or unlicensed, and what does each
verdict mean for a paid build? Per #262 and #263 the sale is the paid 2.0 game; 1.0 is free.
The rule the audit applies: no non-commercial or unlicensed data in a paid build, and none in
the prebuilt pack (RFC 0333, ADR 0379).

## Result

- 131 sources; of the 114 not `rejected`, 24 are non-commercial or unlicensed. Every one has
  `policy = "fetch"`, so none is bundled in any artifact today. Six already carried a verdict
  (iea-balances, gennaioli-regions, lis, exiobase, acled, polity5); this audit adds one to the
  other 18 (`Verdict:` at the start of `reason`).
- One source blocks a paid build: `epr-v1` (ETH ICR, CC BY-NC-SA 4.0, `use = primary`, ethnic
  group status and settlements in 2000, sections 6.4 and 11.6). Every other excluded source is
  `fallback` or `validation`, so excluding it removes a check or a fallback, not a model input.
- Open substitutes exist for gaez-v4, wto-tariffs, irr-regimes, pink-sheet and (partly) bis-stats
  and imf-data; their `notes` say which. None has one for unctad-trains, gdl-shdi, em-dat,
  global-flood-db, cow, atop-5, views, laeven-valencia (after 2017).
- Share-alike, not non-commercial, and out of scope here: osm and speibase (ODbL-1.0) and vdem
  (CC-BY-SA-4.0) bind the derived table they feed (ADR 0182); the licence audit of the prebuilt
  pack (#408) has to place them.

## Timeline check (sale before February 2027)

The only dated dependency is EPR: either ETH ICR grants a commercial licence or EPR leaves the
paid build. EPR feeds ethnic-group status for the politics and conflict modules, which are 2.0
and later work (no railroading, ADRs 0241 and 0242), so the exclusion costs nothing in 1.0.
Whether to ask ETH is for Antonio. The other exclusions need no action before a sale.

## Not done

- Licences were not re-read at the providers; entries keep their `checked` dates.
- No lawyer review (RFC 0344 requires one before 1.0 for the game licence and the CLA).
- Whether any NC value leaks into a calibrated number shipped in the prebuilt pack (derived
  data) is a pack-build check, to be done with the redistributable-source set in #408.
