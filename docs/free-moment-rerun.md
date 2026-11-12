# Free-moment rerun and RFC 0286 screen (Mac)

The pack runs behind the 12 November retrospective's first action and specs/p5/PLAN.md tasks E1
to E3. Every command runs from the repository root in release, detached under `caffeinate -imsu`,
logs in `~/ceteris-logs/`. Each column of the result table is one setting of
`CETERIS_PRICE_ANCHOR`: `off` (status quo), `e1` (anchored rule, ADRs 0338, 0340) and `e3`
(ADR 0343's package). `ceteris run --price-anchor` takes the same names.

## Packs

- `P` = the built 2000 pack with its calibration (`data/packs/2000`).
- `P_NU` = a copy of `P` calibrated with `ceteris calibrate --pack P_NU --no-border-fit --out T`,
  then `T`'s tables copied into `P_NU` (border nu reverted to the default, ADR 0092).
- `P_ST` = a copy of `P` calibrated with `ceteris calibrate --pack P_ST --storage --out T`, then
  `T`'s tables copied into `P_ST` (storage on, ADR 0356). RFC 0280 timed this at about 64 min.
- The V1 reference needs `CETERIS_PACK_EXPORT`: `ceteris export --pack P --out E` for `P` and
  `P_NU`, `ceteris export --pack P_ST --storage --out E_ST` for `P_ST`.

## Free moments (H2 distance, Brexit, ACR-IO, US-China #245, x10)

For each `A` in `off e1 e3`, on `P` (and `P_NU` for the border-nu column):

```
CETERIS_PACK=P CETERIS_PRICE_ANCHOR=A cargo test --release -p ceteris-calibrate --test benchmark \
  -- --include-ignored --nocapture
CETERIS_PACK=P CETERIS_PACK_EXPORT=E CETERIS_PRICE_ANCHOR=A cargo test --release -p ceteris-cli \
  --test p2_gate h2_distance_elasticity -- --nocapture
```

`benchmark_cases_are_reported` prints openness, ACR x1.1 (three forms), hard Brexit and x10;
`eu_loses_from_soft_brexit` soft Brexit; `us_china_2018_is_reported` US-China. Pass numbers are
RFC 0257's, fixed in advance.

## RFC 0286 screen (16 starts, 6,000 ticks)

For each `A` in `off e1 e3`:

```
CETERIS_PACK=P    CETERIS_PACK_EXPORT=E    CETERIS_PRICE_ANCHOR=A cargo test --release \
  -p ceteris-cli --test p2_gate rfc0286_screen_is_reported -- --ignored --nocapture
CETERIS_PACK=P_ST CETERIS_PACK_EXPORT=E_ST CETERIS_PRICE_ANCHOR=A cargo test --release \
  -p ceteris-cli --test p2_gate rfc0286_screen_is_reported -- --ignored --nocapture
```

Each prints RFC 0280's table: per start the largest price gap at 1,200, 3,600, 4,800 and 6,000
ticks, the wage and flow gaps at 6,000 and whether V1 holds; a halted run prints its stderr. The
gate RFC 0286 sets is all eight storage-on starts converging and the storage-off ones no worse
than the B8e baseline.
