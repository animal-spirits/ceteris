## 19. Technology Stack, Performance and Repository

A Rust core gives deterministic, parallel speed; Rust crates also own the data pipeline and calibration, and analysis reads the outputs from outside; the desktop app renders the map, in its webview today and natively in Rust (wgpu map, Slint panels; ADR 0088) later; the three meet on Apache Arrow.

### 19.1 Stack

| Layer | Choice | Why |
| --- | --- | --- |
| Simulation core | Rust, plain structure-of-arrays, rayon for parallelism | Determinism control, speed, memory safety for a long-lived codebase |
| Graphs | petgraph plus a sparse least-cost-path cache | Transport costs recomputed only when edges change |
| GPU fallback | wgpu compute on Metal | Only if the trade solver misses its CPU budget |
| App shell | One native Rust process: winit window, Slint panels, the economy on a sim thread publishing views through a run-ahead ring (ADRs 0088, 0208) | One native macOS app, no server |
| Map UI | wgpu globe drawn into a texture shown in Slint: fills coloured from a per-feature texture, borders from an id target, labels as Slint text (ADRs 0010, 0088, 0244); globe only, with Equal Earth for flat exports (ADR 0250) 120 fps on the owner's Mac (frame p50 8.33 ms); a full province recolour uploads in 0.06 ms |
| Pipeline, reference model, calibration | Rust crates `ceteris-pack`, `ceteris-reference`, `ceteris-calibrate`, driven by the `ceteris` CLI; GEOS only on the owner-side map build (ADR 0113) | One toolchain; the restricted part of the pack builds on the user's machine with no Python runtime |
| Analysis | Outside the repo: Polars, DuckDB or any language reading the Arrow and Parquet outputs | No bindings to maintain; runs are driven by scenario files |
| Run store | Parquet partitioned by experiment / branch / seed / table; DuckDB queries | Cheap ensemble analysis on one disk |
| Snapshots | zstd-compressed Arrow, content-addressed | Branch sharing |
| Licence | Open source, non-commercial: Apache-2.0 / MIT dual for code; data under its source licences, downloaded by each user (ODbL and other share-alike tables shipped separately) | Non-commercial sources usable; restricted sources fetched, never bundled |

An alternative worth noting: the Julia ecosystem now hosts a data-driven macro ABM framework (BeforeIT.jl, associated with the Bank of Italy), useful as a reference implementation for the economic core even though Ceteris Paribus uses Rust.

### 19.2 Performance budget (single run, 8-core desktop)

Hardware: MacBook Pro with an M-series Pro chip (about 12 cores) and 18-36 GB of unified memory. Target 6 to 12 ticks per second (0.5 to 1 simulated year per second), about 80 to 160 ms per tick. Memory target: under 6 GB for one interactive run so ensembles can run alongside. On the 10-goods P2 pack the sim-loop tick measured about 34 ms on a loaded Mac after the speed stack, against its 35 ms budget; the 160 ms ceiling stands (ADR 0325).

| System | Work per tick | Budget |
| --- | --- | --- |
| Demand and consumption | 100k pops x 8 categories x goods | 10 ms |
| Production and labour | 150k enterprises | 12 ms |
| Province markets | 4,000 provinces x 50 goods | 2 ms |
| Trade (one gravity normalization per tick, gateway-factorized kernel) | 45 goods in 3 to 5 theta classes, 600 gateways + 4,594 provinces x k gateways | 30 ms, of which source\_dispatch 8 ms (P2, full pack) |
| Finance, fiscal, monetary | 1M claims, mostly passive | 8 ms |
| Politics, migration, diplomacy | Pops, parties, states | 8 ms |
| Climate (annual) | Emulator + pattern scaling | under 1 ms amortized |
| Accounting assertions | Full ledger check | 5 ms |
| Headroom |  | to 160 ms |

Goods-indexed cost (ADR 0235, measured 2026-11-07 on the calibrated pack, release, one rayon thread, wall time on a Mac 14-core host at load average 40 to 111, so values are upper bounds). Synthetic worlds of 10, 20 and 45 goods; per-system medians in ms at 10 / 20 / 45 goods and the least-squares per-good slope: dispatch 97 / 248 / 823 (21.2 per good), deliver 66 / 99 / 387 (9.6), produce 32 / 41 / 226 (5.9), clear 6 / 15 / 100 (2.8), labour 18 / 22 / 84 (2.0), consume 19 / 36 / 80 (1.7), source 14 / 34 / 66 (1.5), distribute 1.3 / 2.8 / 4.3 (0.08), reprice 0.3 / 0.6 / 1.3 (0.03); the nine systems sum to 255 / 497 / 1,774 ms. Dispatch, deliver, produce and clear are superlinear in goods (negative fitted fixed term), so a linear fixed plus per-good form holds only for the other five and the 45-goods value is read from the measurement, not the fit. With default threads the sum is 90 / 133 / 352 ms at the same load; the 10-goods figure is 2.6 times the quiet-host 34 ms tick, so the 45-goods tick on a quiet host is an estimate near 135 ms and is not yet confirmed against the 160 ms ceiling. The synthetic 45-goods world runs 100 years with no invariant violation (CPU per tick 1,827 ms in years 1 to 5 and 1,969 ms in years 96 to 100, one thread, ratio 1.078), at 10.5 GB peak resident memory, above the 6 GB interactive target. If the quiet-host tick exceeds 160 ms, the levers in order are gateway count and crossings (0092), the dispatch plan (0097), then the ceiling by new ADR. Chunk store (ADR 0212, 0326): one live run to 2150 is 1.59 GB of the 20 GB budget; an ensemble of 32 to 2050 is 13.0 GB of 50 GB.

During wars the daily sub-tick adds about 30 cheap steps per month, budgeted separately. All timings are estimates; P0 and P2 benchmarks replace them with measurements. The ceiling stays 160 ms; from P3 each goods-indexed line is restated as a fixed cost plus a cost per good fitted to measurements, with its value at 45 goods (ADR 0235). Ensembles run headless across the desktop's cores; with desktop-only hardware, calibration depends on emulators and modular calibration to stay within one machine's compute.

Disk (first estimates, replaced by the P3 retention benchmark; ADR 0212): one interactive run to 2150 within 20 GB and an ensemble of 32 within 50 GB, by tiered history retention.

### 19.3 Repository layout

```
ceteris/
  specs/                  module SPEC.md files: state, equations, order, invariants, parameter IDs
  adr/                    architecture decision records
  crates/
    ceteris-core/         entity storage, scheduler, RNG, ledger, snapshots, events
    ceteris-geo/          provinces, transport graph, trade costs
    ceteris-econ/         production, enterprises, labour, demand, productivity
    ceteris-trade/        province markets, two-level entropic OT, shipments
    ceteris-finance/      money, central banks, banks, sovereign debt, FX, fiscal
    ceteris-demo/         demography, education, identity, migration
    ceteris-polity/       institutions, interest groups, parties, elections, unrest
    ceteris-war/          diplomacy, forces, combat, civil conflict
    ceteris-climate/      emissions, emulator, damages
    ceteris-ai/           state planners, policy interfaces
    ceteris-pack/         data pipeline (any start year 1995-2022); config/ holds the licence manifest
    ceteris-geos-coverage/  GEOS coverage simplification, owner-side map build only
    ceteris-reference/    GE hat-algebra reference model
    ceteris-calibrate/    calibration, estimation, SBI, MSM, emulators
    ceteris-cli/          the ceteris binary: pack, reference, calibrate, runs, branch and ensemble runners
  app/                    native app (winit, Slint, wgpu), its own cargo workspace (ADR 0088)
  tests/golden/           golden runs with state hashes
```

### 19.4 Performance directives

These are binding rules for the coding agent: the speed comes from layout and algorithm choices, never from relaxing determinism.

**Data layout**

- Structure-of-arrays columns indexed by `u32`; no hash maps in hot loops.
- Entities sorted by province (CSR-style offsets), so per-province work is a contiguous slice; re-sort only at quarter boundaries when entities are created or removed.
- Sparse relations (trade partners, transport edges, financial claims by holder and issuer) in compressed-row form; 45x45 input-output blocks stay dense.
- Goods are the innermost dimension of every goods-indexed array, padded to a multiple of 4 (f32) or 2 (f64), aligned to 128 bytes (the M-series cache line).
- Ledger goods holdings are account-major (an account's goods side by side); accounts are opened by kind and each kind in province order, so a province's accounts are contiguous ranges (ADR 0328).
- Goods that share a kernel (theta class and kappa group, in good order) are stored for batched kernels in goods-innermost blocks of B = 4 or 8 goods, `[row][B]`, padded and aligned; ensemble lanes sit outside the block, `[row][lane][B]`. Each good keeps its scalar summation order (ADR 0330).
- Ledger balances in fixed-point `i64`; prices and quantities in `f64`; kernel values in `f32` where the spec allows.
- Economic kernels are generic over a Real trait implemented by f64 and by forward-mode dual numbers (ADR 0096). Release builds instantiate f64 only, so derivatives cost nothing outside calibration.
- Ensemble members are lanes, the innermost array dimension (ADR 0024); interactive play runs one lane.

**State tables**

- Every state table is fixed-size for the run or names a bound and the rule that keeps it (a cap, an aggregation or an archive); its module SPEC states the bound and its expected size at 2150. Rows no system reads any more move to an archive table outside the tick's working set, read by panels and never by a system. Aggregation and archiving run at quarter boundaries in stable ID order (ADR 0236).

**Tick execution**

- Double-buffered state: each system reads tick t and writes t+1; no locks in hot paths.
- Zero allocation in hot loops: preallocated scratch per thread (rayon `map_init`), arenas for transient objects.
- A speed change that claims no output change shows the state hash and the ledger journal identical over 24 full-pack ticks at 1 thread and at all threads, on the Mac and on Linux x86, and reports the criterion tick and a per-system sim-loop timing (ADR 0294); under netted T2b entries (ADR 0331) the gate is the state hash identical, and the netted journal of a detail run equal to the netted run's journal. Ledger money sums are integers, exact in any order, so posting may sum by account bucket in any partition; only an overflow check needs an order (ADR 0294).
- Long single-good reductions named here (dispatch and arrivals row sums of unblocked goods, kernel row normalisers, and any loop a stage shows at 2% of the tick or more) sum in four interleaved accumulators: element j goes to accumulator j mod 4, each taken in index order, combined as (a0 + a1) + (a2 + a3). Fixed k, never the machine's width; scalar code emulates it exactly. Adopting it is one golden move with its evidence stated in RFC 0317 (ADR 0327).
- Shipments in a timing wheel (ring buffer indexed by arrival tick): O(1) insert and delivery, no heap.
- Multi-rate scheduling; war sub-ticks only while a war is active; rare events by sampled next-event times, not per-tick coin flips.
- Closed-form PIGL demand (8.6) and a single kernel product for trade (8.3): goods markets have no within-tick iterative solver. Asset-market clearing (9.9) is the only within-tick iteration.

**Linear algebra**

- The trade kernel factorizes through about 600 gateways: O(P x k + G^2) per tick instead of O(P^2), exact under route choice because route sums decompose through gateways (5.4, 8.3), and an approximation of the path minimum under the least-cost legs P2 keeps (S05).
- Goods sharing a (theta class, weight class) pair share a kernel, so kernel products are matrix-matrix (provinces x goods), not one matrix-vector product per good; 3 to 5 theta classes (register P01).
- Tariffs are constant within a country pair: apply them as blockwise scaling of per-block products; never materialize per-good kernels.
- The gravity kernel tau^(-theta) is bounded and dimensionless, so no log-domain stabilization is needed. Route-choice solves (I - A)x = b, off in P2 (5.4), use sparse iterative methods warm-started from the previous solve and run only when A changes. Anderson acceleration on every remaining fixed-point loop (Walrasian mode, full within-tick Sinkhorn test mode, GE reference model, Koijen-Yogo clearing).
- Never invert the Leontief matrix in the agent model: shortages propagate one sparse matrix-vector product per tick. The GE reference model applies the global IO system matrix-free and solves with Krylov methods.
- Exponential distributed lags (climate boxes, adaptive expectations, depreciation) are O(1) state updates; cohort aging is an array shift.
- Correlated weather from k spatial principal components (O(P x k) per draw); route-choice solves from gateways (5.4), updated only when edges change; contagion by fictitious-default rounds on sparse exposures; bounded-confidence neighbours by sort-and-sweep.

**SIMD on Apple Silicon (NEON, 128-bit)**

- Write inner loops to auto-vectorize: slices with lengths asserted up front, `chunks_exact` and `zip` instead of indexing, no branches (min/max and select for clamps), separate input and output slices. Hand-written kernels use stable crates (`pulp` or `wide`), not nightly `std::simd`.
- NEON has no gather instruction, so sparse kernels vectorize across goods (contiguous) while iterating partners serially; never vectorize across partner indices.
- `exp` and `ln` in logit and migration choices use fixed-coefficient polynomial approximations implemented once in SIMD; utilities that change by small additive amounts update multiplicatively.
- Pops are already partitioned by labour status, so status-dependent logic runs as separate branch-free loops per partition.
- FMA is used explicitly and always (`mul_add`), never left to the compiler, so results match across machines.
- `unsafe` is forbidden except unchecked indexing (`get_unchecked`, `get_unchecked_mut`) in a crate's `hot` module, after safe restructuring was tried: each block has a `SAFETY:` comment and a `debug_assert!` of its bound, a safe twin behind a cargo feature is compared bit for bit in CI, and the PR shows at least 2% of the tick (ADR 0295).

**Parallelism**

- M-series chips mix performance and efficiency cores, so equal static chunks leave fast cores idle. Split work into many small chunks with fixed boundaries independent of thread count, let rayon work-steal, write results to fixed slots, and reduce in fixed order: scheduling varies, results do not.
- Per-province systems receive disjoint mutable slices of the ledger (the province's pops, enterprises and market) and a journal block each; blocks are appended in province order. Float tallies that cross provinces (goods created and destroyed per good) sum per province and add in province order (ADR 0328, ADR 0329).
- Country-local systems (fiscal, politics, labour) run one task per country, sorted largest first for load balance.
- Systems with disjoint read and write sets run concurrently, from a static dependency graph built from declared read/write sets.
- Scatter accumulators whose fixed order blocks a decomposition (first the T2b dispatch accumulators) sum through an in-house exact accumulator (pure Rust, no FMA, one rounding at the end), so any order of adds gives the same correctly rounded sum; adopting it moves goldens once (ADR 0298).
- Snapshots, metrics and UI extraction run on a background thread from the frozen buffer, overlapping the next tick.
- macOS cannot pin threads to cores; use quality-of-service classes instead: user-interactive for the interactive run, background for ensemble processes, so the UI stays smooth during batch work.

**Build**

- Release and bench builds are profile-guided (a 24-tick full-pack training run) with a fixed CPU baseline per platform (`apple-m1` on macOS arm64, none elsewhere), never `native`; the state hash is checked identical against a plain build and across architectures (ADR 0296). `cargo test` uses plain builds.
- The `ceteris` binary and the benches use mimalloc as global allocator once a sim-loop measurement shows at least 1 ms per tick (ADR 0297).

**Ensembles and calibration**

- Ensemble members run as 8 to 32 lanes of one process, advancing in lockstep (ADR 0024); process count about equal to performance cores.
- Data packs are immutable Arrow files memory-mapped by every process, so ensembles share one physical copy through the page cache.
- Lanes share packs, kernels and route-choice factors; trade products widen from (provinces x goods) to (provinces x goods x lanes).
- Copy-on-write snapshots in about 64k-row content-hashed chunks with low-level zstd.
- Emulate principal components of output series; reuse Saltelli sample matrices; random Fourier features keep emulators linear in run count; analysis tools read runs only through Arrow and Parquet outputs, never per-entity calls.

**GPU (fallback only)**

- Metal via wgpu shares unified memory (no copies) but each dispatch costs tens of microseconds, so only large batched kernels qualify (gateway Sinkhorn, emulator evaluation).
- Shader fast-math must be disabled and reductions must use fixed trees; verify the wgpu Metal backend's math mode before enabling any GPU path.

**Determinism rules**

- Pure-Rust `libm` for transcendental functions; fixed reduction trees; no float atomics; fixed target CPU in release builds.
- Dense linear algebra uses the pure-Rust `faer` crate with fixed parallelism by default. Apple Accelerate (faster matrix units, but multithreaded reduction order not guaranteed) is allowed only behind a feature flag for runs that need statistical, not bitwise, reproducibility.

**Tooling**

- criterion benchmark per system in CI; regressions over 20% block merges.
- Profile with samply and Instruments; release builds with `lto = "fat"` and `codegen-units = 1`.

