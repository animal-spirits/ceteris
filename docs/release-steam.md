# Steam release path (P8, issue #411)

Free 1.0 on Steam, no Early Access (decisions #262, #263). Platforms: Apple silicon macOS,
Linux x86_64, Linux aarch64 (RFC 0334, proposed). Windows waits for demand.

## Repo side (this repo)

- `scripts/package_app.sh` stages `dist/<os>-<arch>/` (app, CLI, licences, `SHA256SUMS`) on the
  host platform. Run it once per platform; the macOS build still needs signing and notarisation (#228).
- `release/steam/*.vdf` are steamcmd build scripts with placeholder ids. The depot id placeholders
  and `APP_ID` are filled in after Steamworks assigns them.
- `docs/pack-distribution.md` covers pack size measurement and the Zenodo archive step.
- `docs/privacy.md` is the privacy statement draft the store page links to.

Upload: `steamcmd +login <builder> +run_app_build release/steam/app_build.vdf +quit`.

## Owner actions (not doable in the cloud)

1. Steamworks partner account, app fee, tax and banking forms; create the app and three depots.
2. Fill the ids in `release/steam/`; set launch options per OS in the Steamworks app config.
3. Signed, notarised macOS build (#228); upload from a machine with the builder credentials.
4. Store page and its review lead time; P9 trailer and capsule art need a ruling first.
5. Install and launch check from Steam on a clean Mac and a clean Linux machine (acceptance of #411).
6. Linux: measure minimum hardware and Vulkan driver floor (RFC 0334 open point); verify Steam
   requirements against current Steamworks docs.
7. RFC 0334 merge (Antonio alone); the cross-platform hash job (Branch H) touches `.github/`.
