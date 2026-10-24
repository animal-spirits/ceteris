# 0050. Research and release pack profiles, and sources to audit

Status: proposed
Date: 2026-10-24
Design sections: 16, 16.1, 2.5, 15.4, 22
Phase: pipeline, now

## Context

ADR 0005 and `python/ceteris/packs/sources.toml` already record each source's licence,
commercial use, share-alike terms and release policy, and `check_sources.py` blocks shipping
anything that does not allow commercial use. Two things are missing. First, one pack serves two
purposes that the licences pull apart: research validation, where the best data are often
non-commercial, and a public or commercial release. Second, the gap analysis of 2026-10-24 adds
sources the manifest does not yet list, several of them non-commercial:

- GLORIA multi-regional input-output tables (164 regions, 120 sectors): the best check on the
  imputed input-output structure of the ~115 economies outside OECD ICIO, but CC BY-NC-SA 4.0.
- Jorda-Schularick-Taylor Macrohistory: CC BY-NC-SA 4.0 (the manifest's macro-nc-datasets entry
  may already cover it; confirm).
- ACLED: requires a commercial licence for for-profit use and forbids using it to train systems
  that substitute for it, for any purpose.
- IHME Global Burden of Disease: non-commercial; WHO Global Health Estimates are the
  alternative (ADR 0033).
- Global Terrorism Database: non-commercial.
- renewables.ninja capacity factors: non-commercial (ADR 0034).
- V-Dem v16 (March 2026) supersedes v15; CC BY-SA 4.0, commercial use allowed with share-alike
  on redistributed derivatives.

## Options

1. One pack, excluding every non-commercial source.
2. Two pack profiles built by the same pipeline: research (may read closed-source-only and
   non-commercial sources for calibration and validation, never distributed) and release (only
   sources whose licence allows the planned distribution).
3. Separate pipelines.

## Decision

Option 2.

- `check_sources.py` gains `--profile research|release`. In the release profile, any source
  with `commercial_use = false` or `closed_source_only = true` fails the build if the pack
  reads it; in the research profile it is allowed with `use` in {validation, complement,
  fallback}.
- The pack manifest records its profile; the scorecard records which profile produced it, and
  results that need research-only data are labelled as such.
- Imputation for non-ICIO economies stays on releasable sources (ICIO, BACI, FIGARO, own
  imputation); GLORIA validates it in the research profile, and the imputation error by economy
  is published with the scorecard.
- ACLED is excluded in both profiles because of its machine-learning clause; UCDP GED (CC BY
  4.0) remains the conflict backbone.

Sources to audit and add, each with its provider page checked (the gap analysis flagged several
licences as unverified): GLORIA, ACLED (to record the exclusion), IHME GBD, WHO GHE, GTD,
renewables.ninja, V-Dem v16, WCDE v3, MESMER and its parameter sets, AR6 sea-level projections,
WRI Aqueduct, FAOSTAT, AMIS, IDMC, CShapes 2.0, DESTA, ICOW, Romelli CBIE, Garriga CBI, Global
Sanctions Database, USGS Mineral Commodity Summaries, Smithsonian Global Volcanism Program,
Baumeister oil shock series, Cruces-Trebesch haircuts, Horn-Reinhart-Trebesch China lending,
IMF Investment and Capital Stock Dataset and PIMA, Missing Profits of Nations, OECD AMNE,
FIGARO, EXIOBASE (newer versions may restrict commercial use), ARDECO, Economic Transformation
Database, Ember, IRENA statistics, OECD Pensions at a Glance, Kummu et al. 2025 subnational GDP
(Zenodo; supersedes or complements kummu-gdp), ALFRED vintages, OECD Economic Outlook and IMF
WEO vintages, ITU indicators, van Donkelaar PM2.5, published AI exposure indices. Global Energy
Monitor's statement of CC BY 4.0 conflicts with a non-commercial checkbox on its download form;
get written confirmation before relying on it for release.

## Migration

- Pipeline: the profile flag in `check_sources.py` and the pack build; a CI job builds the
  manifest check for both profiles.
- Manifest entries for the sources above, each with `checked` set on the day the provider page
  is read.

## Tests

- A fixture manifest with a non-commercial source fails the release check and passes the
  research check.
- The release pack's provenance lists no source with `commercial_use = false`.

## Consequences

Research quality and release legality stop competing. The Steam release plan inherits a
mechanical guarantee instead of a manual review.
