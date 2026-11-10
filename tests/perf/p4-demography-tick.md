# P4 demography tick cost (spec 19.2, Branch A)

Bench: `cargo bench -p ceteris-econ --bench pop` (`crates/ceteris-econ/benches/pop.rs`). Cohorts are
`pop::toy` (all three mechanisms on, 176 cells per province), a cost fixture and never a
calibration input. Estimate, not a Mac measurement: x86 cloud host (4 cores), no calibrated pack,
so the fixture (25 provinces) was timed, release, criterion median of 10 samples.

| Case | Median |
|---|---|
| `demography` system, monthly tick | 180 us |
| `demography` system, ageing tick (ageing + education progression) | 186 us |
| whole tick, population off | 1.72 ms |
| whole tick, population on | 2.12 ms |

Cost per cell is about 41 ns (180 us / 4,400 cells), the same on monthly and ageing ticks, so
mortality and fertility dominate; ageing and progression add under 4%.

Linear extrapolation to the 4,368-province pack (768k cells): about 31 ms per tick, single thread,
against a 160 ms ceiling and no demography line in the 19.2 table (migration, politics and
diplomacy share 8 ms). Per-province work is independent, so rayon over provinces is available.

Status: the system is off by default (#434). Per spec 21.2 the number to act on is the Mac pack
measurement with `scripts/bench_quiet.sh`; until then 31 ms is an upper-bound planning figure.
Per Development laws, no optimisation before the Right gates: the per-cell cost (likely a
transcendental per cell in `monthly_q`) is recorded here, not changed.
