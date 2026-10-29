# 0215. First-run data fetch and the source archive

Status: proposed (owner chose the archive option, 2026-10-28)
Date: 2026-10-28
Design sections: 1, 16, 16.1, 21.1 (R); ADRs 0005, 0050, 0113, 0207, 0211, 0217
Phase: R (fetch screen, archive); P3 (pack schema check, rebuild)

## Context

Each user downloads their own data (owner, 28 Oct 2026). fetch.toml pins 50 files, about 6 GB,
by sha256; 13 are manual downloads from three providers (OECD ICIO, Global Energy Monitor, Kummu
GDP). The README still says 920 MB. When a provider revises or removes a pinned file, every new
user is blocked and the R gate ("a clean machine builds the pack and reproduces the published 1.0
hashes") fails. Nothing designs the first-run experience, manual downloads, resuming, disk checks,
or rebuilding when an update changes the pack schema. Audit gap 7.

## Options

1. Archive every redistributable source on Zenodo at each release; fetch tries upstream, then the
   archive.
2. Upstream only.
3. Re-pin at each release and accept that older releases stop reproducing.

## Decision

Option 1 (owner, 2026-10-28).

**Archive.** At each release the maintainer uploads every pinned file whose licence allows
redistribution to a Zenodo record versioned with the release (one DOI per release). fetch.toml
entries gain a `mirror` URL. `fetch` tries upstream, verifies the hash, and falls back to the
mirror on a missing file or a hash mismatch. Files that may not be redistributed stay upstream
only; if one changes, the release notes say which releases no longer reproduce, and the next
release re-pins it. A scheduled CI job checks every pinned URL monthly and reports drift.

**First-run screen.** In the app and as `ceteris setup`:
- A disk check up front (downloads plus build space).
- One row per source: size, licence, status, and progress; downloads resume.
- Manual files: the provider's page, the exact file name and size, and a drop zone or watched
  folder; a file is accepted when its hash matches.
- The pack builds with a progress bar; the app opens on the built pack.

**Schema changes.** The app knows the pack schema version it needs. On a mismatch it rebuilds from
the cached sources without downloading again, and says how long it will take.

## Migration

- `sources.toml` gains `mirror_ok` (licence allows redistribution); `fetch.toml` gains `mirror`.
- R: archive upload script, setup screen and CLI, drift job. P3: schema check and rebuild.
- README: sizes and steps follow the pack when this lands.

## Tests

- With upstream blocked, `fetch` completes from the archive for every `mirror_ok` file.
- A manual file with a wrong hash is refused with the expected name and size shown.
- An interrupted download resumes.
- A clean machine builds the pack from the archive and reproduces the release hashes (R gate).

## Consequences

Releases stay reproducible for most of the data at no hosting cost, with citable DOIs. The
maintainer uploads about the redistributable share of 6 GB per release.
