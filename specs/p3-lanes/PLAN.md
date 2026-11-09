# P3 Branch L: ensemble lanes

> Draft for the owner; decisions of 2026-11-07 applied (section at the end). Refines Branch L of `specs/p3/PLAN.md` against the kernel signatures on main
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

## Tasks, in order

The owner chose the vectorized layout directly (ADR 0024 as worded), so C1 waits for Branch L.
Every task is gated the same way: lane l of an `L`-lane run equals the `L = 1` run with lane l's
parameters and seed, bit for bit, and release builds at `L = 1` reproduce the goldens at 1, 3 and
all threads. Lane arithmetic is the scalar arithmetic repeated per lane in the same order: no
cross-lane reductions, no changed association, no FMA contraction.

### Task L1: Trade kernels generic over lanes

**Files:** `crates/ceteris-trade/src/{source,kernel,pipeline}.rs`.

**Interfaces:** `source`, `dispatch`, `arrivals` and the access passes take columns as
`&[[f64; L]]` (lane innermost after goods), read the `Kernel` once per call and loop lanes in
the innermost position; the scalar entry points stay as `L = 1` wrappers so callers compile
unchanged. Per-lane active masks and per-lane `weight_price` (ADR 0027) are lane columns.

- [ ] Failing tests: per kernel `lane_kernel_equals_scalar_kernel` over random prices and masks for `L` in 1, 2, 8; `l1_kernels_reproduce_goldens`.
- [ ] Implement per kernel, commit per kernel (`feat(trade): lane-generic source`, and so on).

### Task L2: Lane-wide world state

**Files:** `crates/ceteris-econ/src/{world,systems,lanes}.rs`.

**Interfaces:** `EconWorld<const L: usize = 1>`: price, wage, market, pipeline, ledger and
smoothed-price columns gain the lane dimension; `inputs: Arc<EconInputs>` and the `Kernel` are
stored once. Per-lane `EconParams` and `TradeParams`; lanes with equal trade parameters share a
kernel through a cache keyed by `Kernel::id()`, a differing lane gets its own (copy-on-write,
ADR 0024). The ledger journal and invariants run per lane. Systems become lane loops except
where L1 supplies a lane kernel.

- [ ] Failing tests: `lane_k_equals_scalar_run_with_seed_k` at every snapshot on the fixture; `lanes_with_equal_trade_params_share_one_kernel`; `world_hashes_do_not_depend_on_thread_count`; `l1_world_reproduces_goldens`.
- [ ] Implement system by system, each with its lane equality test; commit per system.

### Task L3: Block API for C1

- [ ] `Sim` over `EconWorld<L>`: `step()` advances all lanes; `state_hashes()` returns one hash per lane; snapshots store lanes as separate branches (ADR 0024 Consequences), so no new snapshot format; restore of one lane into an `L = 1` world.
- [ ] Failing tests: `crn_pair_in_two_lanes_equals_two_scalar_runs`; `snapshot_of_lane_k_restores_into_scalar_world`. Commit.

### Task L4: Benchmark and the ensemble budget

- [ ] Criterion benchmarks at `L` = 1, 8, 16 and 32 against the process-per-run baseline on the fixture and the full pack (Mac): run-ticks per second, peak resident memory, memory per differing-kernel lane.
- [ ] Record the ensemble budget in the econ SPEC and the block size C1 uses; propose a memory ceiling from the measured peaks (decided: measure first, no ceiling before).

## Route-choice solve

The route-choice solve of the pack build is not in the tick and stays scalar; lanes with changed
edges get a copy-on-write factor through a kernel rebuild (L2's cache). Vectorizing the solve
is out of scope unless L4 shows rebuilds dominate ensembles.

## Not in this branch

Per-lane existence masks (enterprise entry, wars) are P4 work (ADR 0024 Migration).
`Dual<N>` tangents (ADR 0096) are P9; the lane layout takes a `Real` parameter later without
reshaping. Member parameter draws are C1's table.

## Decided (owner, 2026-11-07, question cards)

1. Vectorized layout directly, not a block runner first. The plan above follows it; the cost is that C1 starts after L2 and L3.
2. The benchmark covers `L` = 8, 16 and 32 (plus 1 as the baseline).
3. No memory ceiling now; L4 measures peak memory and proposes one.

## Estimate

L1 about 0.5 owner-week, L2 about 1 to 1.5, L3 about 0.25, L4 about 0.25 (Mac time). The
1.5 to 2.5 in `specs/p3/PLAN.md` holds.
