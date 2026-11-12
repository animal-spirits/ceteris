# RFC 0346. Storage-on calibration of a pack for the RFC 0286 screen

Status: accepted
Date: 2026-11-12
Design sections: 8.2
Release: 1.0, P5 (task E2)
Research: RFC 0286 (screen); RFC 0280, As built (storage-on calibration of the 2000 pack)

## Summary

RFC 0286's screen runs V1 from sixteen starts, eight of them with competitive storage on. Storage
is structural (ADRs 0091, 0267): a calibrated start refuses storage parameters other than those
its steady state was solved under, and the loader accepts only tables whose static digest is that
of the inputs under the default parameters, where storage is off. The storage-on runs of 1
November used a local switch that was never committed, so the screen could not be rerun. This RFC
lets a calibration solved under the approved storage values (`TradeParams::with_storage`) load:
its static digest already encodes those values, so the loader can tell it apart without a format
change, and the calibrated inputs then run with storage on. Storage stays off by default.

## Decisions

### D1. How does the storage-on half of the RFC 0286 screen get its calibrated pack?

Options:

- A. Accept the storage-on digest: `ceteris calibrate --pack DIR --storage` exports and calibrates
  under the approved storage values; the loader accepts that digest, and `EconInputs::trade_params`
  then turns storage on. Same file format.
- B. Storage-off half only: the screen covers the eight storage-off starts; the storage-on pack
  waits for its own RFC.
- C. Local switch, not merged: a patch the Mac applies to a scratch checkout, as on 1 November.

Recommendation: A, because the static digest already includes the storage parameters, so this
needs no format change and keeps the guard against stale calibrations.

Decision: A, Antonio, 2026-11-12 (question card).
Row: ADR 0356

## Spec changes

- econ SPEC, Calibration (loading the tables): a digest under the approved storage values also
  loads, and the inputs then run with storage on (ADR 0356).
