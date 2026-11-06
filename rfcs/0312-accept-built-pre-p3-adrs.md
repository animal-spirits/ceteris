# RFC 0312. Accept the pre-P3 ADRs that are built but still read proposed

Status: proposed
Date: 2026-11-06
Design sections: none
Release: 1.0, pre-P3
Research: `reports/plans-from-review-2026-11-04.md` item 5a (project files); each ADR's own "As built" section

## Summary

Fifteen ADRs from the P2 and pre-P3 work are built and on main, with C6 and C7 merged, but their
status lines and the RFC index still read proposed, so the index understates what is settled
(the 2026-11-04 review counted at least 17; ADRs 0099 and 0104 have since been accepted by RFCs
0301 and 0302). This RFC accepts the fifteen as built and as amended, in one decision. It changes
no behaviour and no number.

## Decisions

### D1. Accept ADRs 0092, 0093, 0094, 0098, 0100, 0101, 0103, 0106, 0247, 0256, 0266, 0267, 0276, 0280 and 0281 as built?

Each line says what main holds. The ADR's "As built" section has the detail and the measured
numbers.

| ADR | As built on main | Amended by |
| --- | --- | --- |
| 0092 Trade frictions consistent with theta | Per-class and per-good border friction nu fitted to ICIO 2000 cross-border shares (B8b); four theta classes from the C19 move; class 3 clamped at nu = 0. Gateway mass weights were removed and stay deferred; the H2 per-class gate is reported . | 0276 |
| 0093 Numerical robustness of the P2 core | Log-domain weights and the kernel range with per-country shifts; one fill per local good; bounded work. | 0266 (labour targets) |
| 0094 PIGL aggregation across pops | `invert_pigl_pops` and `within_sigma` in the simulator, `invert_pops` in the reference and calibration stage 4, with s_w stored in the pack. | 0313, 0316 |
| 0098 Price levels and source corrections | `econ::price_level`, the 1999 to 2001 rate rescale, SWIID Gini with fallback, RAS-balanced donor tables (`econ::ras`), real GDP check. | 0282 to 0286, 0289, 0290, 0317 |
| 0100 Trade imbalances as transfers | `econ::transfers`: current accounts as transfers, balanced final demand and the observed 2000 to 2024 path. | 0315, 0318, 0319, 0300 |
| 0101 Harmonic aggregation of trade elasticities | Harmonic class means on the Caliendo-Parro vector; four classes 2.91, 4.14, 7.68, 16.91; theta scale P150; non-uniform welfare test. | |
| 0103 Initial drift | `econ::drift` from PWT 1995 to 2000; `EconParams::drift` and the half-life P153 in the tick; P153 is untuned (issue #172). | |
| 0106 Price clip on the gap, money width | Clip on the gap only; calibration pinned by rescaling the traded r each pass; wide (`i128`) ledger accounts for accounts that aggregate many others; the full-width retrofit is P4 (#211). | |
| 0247 Bulk freight rate per theta class | `TransportParams::bulk`, P154 and P155, `BulkLegs`; on by default for class 3. | |
| 0256 World technology for oil and gas from rents | Rents path in the pack build, crude oil and gas VA from rents. | 0270, 0281 |
| 0266 Labour capped at what inputs allow | Input-capped labour targets, the rationed frictionless wage rule, the E3 split order rule. | |
| 0267 Price gain capped per coupled block | `ceteris_calibrate::block`, block spectral-radius limits, `price_gain` in the world. | |
| 0276 Border friction per good in class 3 | `TradeParams::border_nu` per good; fit on the 2000 pack. | |
| 0280 Storage order by partial adjustment | Partial-adjustment storage order with lambda and a target stock; storage off by default; the storage-on frictionless misses are recorded in RFC 0286. | |
| 0281 Rents-to-VA factor from extraction | `icio::extraction_va`; installed pack rebuilt at B10. | |

Options:

- A. Accept all fifteen as built and as amended. Each ADR keeps its text; this RFC is the
  acceptance record, and the RFC index shows them accepted.
- B. Accept all except those the owner names, which stay proposed.
- C. Leave them proposed.

Recommendation: A, because they are built, tested and gated by merged C6 and C7, and a proposed
status on built work misleads every reader of the index.

Decision: pending, Antonio.
Row: none new; ADRs 0092, 0093, 0094, 0098, 0100, 0101, 0103, 0106, 0247, 0256, 0266, 0267, 0276, 0280 and 0281 keep their rows

## Spec changes

- `specs/trade/SPEC.md` (Two-level kernel) and `specs/econ/SPEC.md` (Pack additions; Goods): the
  "proposed" marks beside ADRs 0276 and 0281 go.
- `rfcs/README.md`: the fifteen show `accepted (RFC 0312)`.
- ADR files are not rewritten.

## Consequences

- Open items against these ADRs stay open under their own issues: the gateway mass weights and
  H2 per class (ADR 0092), the P153 half-life (#172), storage-on misses (#216).
- ADRs still proposed after this are the post-1.0 and later-phase ones, which the phase table of
  ADR 0207 already lists.
