# CI pack subset (RFC 0353, ADRs 0370, 0374)

`ceteris pack subset --pack DIR --out tests/fixtures/pack-ci` cuts the small uncalibrated pack
from a full uncalibrated pack. Config: `crates/ceteris-pack/config/subset_ci.toml` (country list,
rest-of-world code, 25 MB cap, excluded sources, per-column aggregation rules).

- Kept countries keep provinces, cells, counties, graph nodes and edges, gateways, econ rows.
- Every other country becomes one rest-of-world economy: no provinces, no gateways, columns
  combined by the rules (`sum`, GDP-weighted `mean`, `any`, `first`, `const:`). `owner_k` columns
  are reindexed; dropped owners sum into the rest-of-world owner.
- A column without a rule, a calibrated pack, or output over the cap is an error.
- `manifest.json` lists the sources less `exclude_sources` (EPR, EXIOBASE), the config hash and
  the table digests. Output is byte-identical across runs.

Check the rules against the real columns (Mac, full uncalibrated pack):

```
cargo run --release --bin ceteris -- pack subset --pack data/packs/2000 --schema > /tmp/schema.txt
```

It prints `table.column type rows=N` and exits 1 listing columns with no rule. Then generate:

```
cargo run --release --bin ceteris -- pack subset --pack data/packs/2000 --out tests/fixtures/pack-ci
```

The full pack on the Mac holds calibration tables after `calibrate`; subset from a pack built
with `pack build` before calibrating, or copy the uncalibrated tables to a clean directory.
