---
paths:
  - "python/**"
---

# Python (being removed, ADR 0113)

The pipeline, reference model and calibration are moving to `ceteris-pack`,
`ceteris-reference` and `ceteris-calibrate` (plan: `specs/rust-migration/PLAN.md`).

- No new features in Python. Fixes only, and only when the Rust port is not yet in place.
- Python output is the parity baseline for the port: do not change it during a migration step
  unless the plan says so.
- Run through uv: `uv run --project python --locked pytest python/tests`.
