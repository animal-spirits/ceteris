# 0216. Platforms, signing, updates and minimum hardware

Status: proposed
Date: 2026-10-28
Design sections: 1, 19.1, 19.2, 22.1 (Hardware, First release); ADRs 0088, 0128, 0207, 0218
Phase: R

## Context

Phase R promises "a signed macOS build". ADR 0128 wants the golden hash on macOS, Linux and
Windows. Nothing settles which platforms 1.0 ships on, who holds the Apple Developer account
(99 USD a year on a non-commercial project), the distribution channel, how updates reach users,
or what happens on a 16 GB Mac when 19.2 assumes 18 to 36 GB. Audit gap 8.

## Options

1. macOS app only.
2. macOS app, plus the headless `ceteris` CLI for macOS and Linux.
3. App on macOS, Linux and Windows.

## Decision

Option 2.

- **App:** macOS on Apple silicon, signed and notarized. Intel Macs are unsupported.
- **CLI:** macOS and Linux (x86_64 and aarch64), for batch runs, calibration and servers. CI
  already runs Linux. Windows runs only the cross-platform golden test of ADR 0128.
- **Channel:** GitHub Releases (the repository is public from 1.0, ADR 0218) and a Homebrew cask
  for the app and a formula for the CLI.
- **Updates:** the app asks GitHub Releases for the latest version at launch and shows a notice;
  it never installs by itself. The check can be switched off and sends nothing but the request.
- **Minimum:** Apple silicon, 16 GB, macOS from the current version minus two. Below 18 GB the
  defaults drop to 8 ensemble lanes and a longer history interval (ADR 0212).
- **Signing:** the owner's Apple Developer account; the yearly fee is the project's one fixed cost
  besides the collector of ADR 0214.

## Migration

- R: release workflow (build, sign, notarize, checksums, cask, formula), update notice.

## Tests

- The release workflow produces a notarized app that opens on a clean Mac.
- The Linux CLI reproduces the release golden hashes.
- A 16 GB machine runs the default scenario to 2050 within memory.

## Consequences

One app platform keeps the UI work small; researchers on Linux still get the full engine.
Windows users wait for demand.
