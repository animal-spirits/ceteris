# RFC 0334. Linux app for the 1.0 Steam release (amends ADR 0216)

Status: proposed
Date: 2026-11-09
Design sections: 19 (platforms)
Release: 1.0, R
Research: `reports/pre-1.0-polish-plan-2026-11-09.md` (P8)

## Summary

ADR 0216 ships the 1.0 app on macOS (Apple silicon) only, with the CLI on macOS and Linux. For a
Steam release the app also ships on Linux (x86_64 and aarch64), which the Slint and wgpu stack
supports. Windows stays out until there is demand. Estimate: about 1 owner-week beyond the Steam
path (issue #411).

## Decisions

### D1. Which platforms does the Steam 1.0 app ship on?

Options:

- A. macOS only (ADR 0216): smallest scope.
- B. macOS plus Linux app: about 1 owner-week; Steam Deck reachable.
- C. macOS, Linux and Windows: largest audience; 2 to 3 owner-weeks plus Windows signing cost.

Recommendation: B, because it is a cheap gain in reach and Windows can wait for demand.

Decision: B, Antonio, 2026-11-09 (decision card in the polish-plan thread).
Row: to be taken when this PR writes the spec.

## Spec changes

- 19, platforms: app on macOS and Linux; Windows runs only the cross-platform golden test (ADR 0128).
- ADR 0216 status line notes the amendment; its tests gain a Linux app launch on a clean machine.

## Open points for the review

- Linux minimum hardware and GPU drivers (Vulkan) are not yet specified; measure on a Linux box.
- The cross-platform hash job (Branch H) must cover the Linux app build.
