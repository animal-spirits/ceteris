# Pack distribution (P8/P14, ADR 0215)

Decision (docs #457): the prebuilt redistributable pack ships inside the Steam depot; a Zenodo
record is the archive of record for redistributable sources, one record (one DOI) per release.

## Size measurement

`scripts/pack_size.sh [PACK_DIR...]` prints files, uncompressed and compressed MiB per pack
(default: each directory under `data/packs/`). Compression is `xz -6` as a stand-in for Steam's
chunk compression (override with `COMPRESS`); treat the compressed figure as an estimate and
confirm with a steamcmd upload.

The pack is built owner-side (Mac), so no figures are recorded yet. Owner action: build the release
profile pack, run the script, paste the table here, and check it against the depot size you can
upload (Steamworks docs, verify).

| pack | files | uncompressed MiB | compressed MiB | ratio |
|---|---:|---:|---:|---:|
| 2000 (release profile) | pending | pending | pending | pending |

Fallbacks if the pack is too large for a depot: a separate Steam depot or DLC-less optional depot
for the pack, or the Zenodo record as first-run download (50 GB per record, verify).

## Zenodo archive

`scripts/publish_pack_zenodo.sh --version V DIR` archives the files in `DIR`, which the owner
filters beforehand to sources whose licence allows redistribution (`pack check-sources --profile
release`; non-commercial and no-redistribution sources never go in).

- Default is a dry run: no network, no token. It prints the target, file count, bytes and the
  deposition metadata.
- `--execute` creates a draft deposition, uploads `DIR` plus a `SHA256SUMS`, sets metadata, and
  leaves the draft unpublished for review.
- `--publish` also publishes. Publishing is irreversible and mints the DOI.
- `--sandbox` targets sandbox.zenodo.org; rehearse there first.
- The token is read from `ZENODO_TOKEN` only. Keep it in the keychain, never in the repo.

After publishing, add each file's record URL as the `mirror` in `fetch.toml` (ADR 0215).
Not covered yet: new versions under an existing concept DOI, and splitting beyond 100 files per
record (tar the files first).
