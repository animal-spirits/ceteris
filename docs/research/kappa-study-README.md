# kappa-study.patch

Diagnostic harness from the ADR 0263 price-gain study. Not for merge.

- Base: main at 6edcd94 (merge of PR #108); `git apply --check` passes there.
- This copy came over as text. It differs from the Mac original by 5 bytes of whitespace (original sha256 a2697555...0e80, 25094 bytes), and it still applies cleanly.
- Env hooks:
  - `CETERIS_KAPPA1` / `CETERIS_KAPPA2`: override the gains (CLI and `EconWorld`).
  - `CETERIS_KAPPA_SCALE`: a file of per-cell f64 LE factors, read in `market.rs`.
  - `V1_SEED` / `V1_TICKS`: set the V1 seed and length.
- Tests that run only when their env var is set:
  - `j2_map` (`J2_MAP`): prints the J2 map. With `J2_SCALE_OUT` and `J2_K` it also writes the scale file.
  - `alb_debug` (`ALB_RUN_DIR`) and `country_map` (`MAP_RUN_DIR`): read a run's output directory.
  - `gain_cost` (`GAIN_COST`, plus `GAIN_FRICTIONLESS` for frictionless): convergence and the Suez response.
- Caveat: `j2_map`'s eps per cell is the enterprise max. ADR 0263 specifies the output-weighted mean (`eps_out` holds both).
