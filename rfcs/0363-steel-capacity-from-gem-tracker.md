# RFC 0363. Steel capacity comes from the GEM Global Iron and Steel Tracker

Status: accepted
Date: 2026-11-20
Design sections: 5.2, 7.1, 22
Release: 1.0 (P2 data)
Research: none; follows issue #175 (SFI steel request unanswered)

## Summary

Steel plant capacity and start year seed the C24A province spread (ADR 0275). The SFI steel
database sits behind a request form that has had no reply since before 2026-11-04. Global Energy
Monitor's Global Iron and Steel Tracker (September 2026 V1) is public, covers plants of 0.5 Mtpa
and above with unit-level furnace data, and is CC BY 4.0, the licence already recorded for the
other GEM trackers. It allows a paid build with attribution. SFI stays the cement source.

## Decisions

### D1. Which source supplies steel capacity?

Options:

- A. GEM Global Iron and Steel Tracker (plant-level and steel unit workbooks), as a manual
  download in the `gem-trackers` source. SFI steel is dropped.
- B. Keep waiting for SFI and calibrate steel on proxies meanwhile.

Recommendation: A, because the data is in hand, openly licensed and more recent than the 2021 SFI
release, and SFI has not replied.

Decision: A, Antonio, 2026-11-20 (thread request to switch the steel source to GEM).
Row: ADR 0397

## Spec changes

- 5.2, 7.1, 22: steel plant seeds come from GEM's Iron and Steel Tracker, not SFI (ADR 0397). Code and spec text did not name SFI for steel, so only `sources.toml` and `fetch.toml` change.

## Work

- `sources.toml`: steel added to `gem-trackers` with the licence check of 2026-11-20; `sfi-geoasset` narrowed to cement.
- `fetch.toml`: the two GEM steel workbooks pinned by sha256; the SFI steel entry removed.
- No reader consumes steel seeds yet, so there is no wiring to test; the kernel freeze holds.
