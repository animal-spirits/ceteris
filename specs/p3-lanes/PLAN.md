# P3 Branch L: ensemble lanes

> Draft for the owner. Refines Branch L of `specs/p3/PLAN.md` against the kernel signatures on main
> after Branches A, B, D and E. Nothing here is built; ADR 0024 and the requirements of pre-P3
> Branch E (kept in `specs/pre-p3/PLAN.md`) are the contract.

**Goal.** Run K ensemble members (and a common-random-number pair) as one block of `L` lanes in one
process, so read-only data is stored once and the hot kernels read it once per block. C1 consumes
the block; the block size comes from a benchmark here.

**Fixed requirements (ADR 0024, ADR 0288).** Lane l of an `L = 8` run reproduces bit for bit the
`L = 1` run with lane l's parameters and seed. Release builds at `L = 1` reproduce the goldens at
1, 3 and all threads. Lanes differ only in parameters and random streams. No `Real` trait; the
layout must not preclude one (ADR 0096, P9).

## What state is read-only and what is per lane

Read from the code on main (`crates/ceteris-econ/src/world.rs`, `crates/ceteris-trade`):

| Item | Today | In a block |
|---|---|---|
| `EconInputs` (pack, network, enterprises, pops) | `Arc`, shared | shared by all lanes |
| `Kernel` (623k nonzeros per traded good) | `Arc`, rebuilt when network, trade params or inputs change | one per distinct (network, `TradeParams`, wedges); lanes with equal trade parameters share it |
| `EconParams` (kappa_w, eta, sigma, P-codes) | by value in the world | per lane |
| `TradeParams` (theta, border_nu, rho of ADR 0027) | by value | per lane; a differing lane gets its own kernel |
| Prices, wages, markets, pipeline, ledger, lagged crossings, smoothed prices | world fields | per lane |
| `Scratch` | per world | per lane |

A lane whose parameters alter the kernel therefore costs one kernel build (about 2 s on the pack,
`kernel_build` budget) and memory for a second kernel; this is the copy-on-write of ADR 0024.
Member draws that touch only `EconParams` cost nothing extra. The table of drawn parameters (C1)
decides how often kernels are shared; theta, border_nu and rho draws do not share.

## Two stages, because the first one is useful alone

The ADR asks for the lane as the innermost array dimension. That rewrites about 3,400 lines of
`world.rs` and 1,860 of `systems.rs` and the 2,400-line `source.rs`. Doing it all before C1
starts puts C1 behind the largest refactor in P3. So the plan separates what C1 needs (a correct
block runner) from what the throughput budget needs (vectorized kernels).

### Task L1: Block runner over shared inputs

**Files:** `crates/ceteris-econ/src/lanes.rs` (new), `crates/ceteris-core/src/sim.rs` (a
`Block<W, const L: usize>` beside `Sim`), tests beside them.

**Interfaces:** `Block<const L: usize>` holds `[EconWorld; L]` built from one `Arc<EconInputs>`
and `L` (params, trade params, seed) triples; `step()` advances all lanes one tick, lanes in
parallel on rayon with a fixed lane-to-task mapping; `state_hashes()` returns one hash per lane.
A kernel cache keyed by the kernel id (`Kernel::id()`) shares kernels between lanes with equal
trade parameters.

- [ ] Failing tests: `lane_k_equals_scalar_run_with_seed_k` at every snapshot on the fixture
  (the C1 test, here at the block level); `lanes_with_equal_trade_params_share_one_kernel`;
  `block_hashes_do_not_depend_on_thread_count` (1, 3, all); `block_of_one_equals_sim`.
- [ ] Implement; commit `feat(econ): lane blocks over shared inputs (adr 0024)`.

L1 is bit-exact by construction (each lane is an ordinary world), so it can land and unblock C1
while L2 is measured.

### Task L2: Lane benchmark and the block size

- [ ] Criterion benchmarks of `Block<1, 8, 16, 32>` against `L` separate processes (the status quo) on the fixture and on the full pack (Mac): run-ticks per second and peak memory. Records the ensemble budget in the econ SPEC and the block size C1 uses.
- [ ] Decision gate (owner): if L1 reaches at least 80% of the throughput the ADR expects from vectorization, L3 is not built; otherwise L3.

### Task L3: Vectorized kernels (only if L2 says so)

Scope is the three places where lanes share read-only data and the arithmetic dominates:
`source` (origin shares and the second access pass of ADR 0027), `dispatch` (crossing pass) and
`price_refresh`. Everything else stays per-lane worlds.

- [ ] Kernel entry points generic over `const L: usize` with the lane innermost after goods: `source_lanes::<L>(k, g, price: &[[f64; L]], ...)`. Lane arithmetic is the scalar arithmetic repeated per lane in the same order, so each lane's bits equal the scalar result (no cross-lane reductions, no FMA contraction changes).
- [ ] Failing tests: per-kernel `lane_kernel_equals_scalar_kernel` over random prices and active masks; then `lane_k_equals_scalar_run_with_seed_k` still passes with `Block` using them.
- [ ] Benchmark each kernel; keep a lane kernel only if it beats L1 for that kernel. Commit per kernel.

## Route-choice solve

The route-choice solve of the pack build is not in the tick and stays scalar; lanes with changed
edges get a copy-on-write factor only through a kernel rebuild (L1's cache). Vectorizing the
solve is out of scope unless a later benchmark shows rebuilds dominate ensembles.

## Not in this branch

Per-lane existence masks (enterprise entry, wars) are P4 work (ADR 0024 Migration). `Dual<N>`
tangents (ADR 0096) are P9. Snapshots store lanes as separate worlds, so `Block` adds no new
snapshot format.

## Open decisions for the owner

1. Build L1 first and gate L3 on the benchmark (recommended), or build the vectorized layout directly as ADR 0024 words it.
2. Lane count the benchmark targets: 8 and 32 only, or 8, 16 and 32.
3. Memory ceiling for an ensemble on the 16 GB target: a second kernel per differing lane is the main cost.

## Estimate

L1 about 0.5 owner-week, L2 about 0.25 (Mac time), L3 about 1 to 1.5 if built. Branch L's
1.5 to 2.5 in `specs/p3/PLAN.md` holds if L3 is built; without it, L is about 0.75.
