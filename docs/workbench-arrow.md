# Reading workbench outputs

`ceteris ensemble`, `effects`, `explain` and `blame` write Arrow IPC stream files into the
verb's `--out` directory: `outcomes.arrow`, `effects.arrow` and `channel_shares.arrow`.
Columns and types are the `workbench/v1` schema in `crates/ceteris-run/src/outputs.rs`
(`TABLES`); there is no Parquet writer in 1.0 (issue #201). Polars and DuckDB read the files
directly.

## Polars

```python
import polars as pl

effects = pl.read_ipc_stream("out/effects.arrow")
print(effects.filter(pl.col("sign_share") > 0.95).select("outcome", "mean", "lo", "hi"))

outcomes = pl.read_ipc_stream("out/outcomes.arrow")
print(outcomes.group_by("branch", "outcome").agg(pl.col("value").mean()))
```

## DuckDB

```sql
INSTALL arrow FROM community;
LOAD arrow;

SELECT outcome, mean, lo, hi
FROM read_arrow('out/effects.arrow')
WHERE sign_share > 0.95;
```

`pyarrow` also works: `pyarrow.ipc.open_stream(open("out/effects.arrow", "rb")).read_all()`.
The files are streams, not Arrow files, so use the `ipc_stream` readers.
