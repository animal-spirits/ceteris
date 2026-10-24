# Core SPEC

Status: draft (P0 needs no human approval; P2, P5 and P6 specs do)
Phase: P0
Design sections: 4.1, 4.4, 4.5, 4.6, 14.3, 14.4, 19.4, 21.1 (P0)

## Purpose

`ceteris-core` owns the machinery every module runs on: stable entity IDs, the
system scheduler, counter-based random streams, the double-entry ledger for money
and goods, the event log, deterministic reductions, and snapshot and restore. It
holds no economics. The toy world in `ceteris_core::toy` is the P0 fixture that
exercises all of it; P2 replaces it with the economic core.

## State components

| Component | Owner | Type | Notes |
|---|---|---|---|
| Entity IDs | per entity kind | `EntityId(u32)` from `IdAlloc` | Monotonic, never reused, part of the snapshot |
| Money balances | `Ledger` | `i64` minor units per account | Negative balance = net liability (issuers) |
| Goods holdings | `Ledger` | `f64` per (account, good), goods innermost | Never negative |
| Account open flags | `Ledger` | `bool` per account | Closed accounts must be empty; IDs are not reused |
| Event log | `EventLog` | `Vec<Event>` | Discrete decisions; part of the snapshot |
| Tick | `Sim` | `u32` | Number of completed ticks; tick 0 is January of the start year |

Per-tick scratch (ledger journal, goods created and destroyed tallies, opening
goods totals) is reset by `begin_tick` and is not part of the snapshot.

## Random streams (Section 4.5)

Philox 4x32-10 (Salmon et al. 2011). Every draw is a pure function of

- key = master seed (`u64`, split into two `u32` words),
- counter = `[tick, system << 16 | kind, entity, block]`.

`system` is the `SystemId` drawing, `kind` the entity kind tag (provinces and firms
may share numeric IDs, so the kind separates their streams), `entity` the stable ID
and `block` the draw index. Each block yields four `u32`; a stream advances `block`
when its buffer is exhausted. There is no global RNG state, so adding an entity or
a system never shifts another stream (common random numbers, Section 14.4).

- `uniform()`: `(next_u64() >> 11) * 2^-53`, in `[0, 1)`.
- `normal()`: Box-Muller cosine branch with `u1 = 1 - uniform()` in `(0, 1]`; uses
  `libm` so results are identical on every platform.

## Scheduler

A `Schedule<W>` is an ordered list of `SystemDef { id, name, cadence, run: fn(&mut W, &Ctx) }`.
System IDs are unique (checked at construction). Systems are stateless: everything
they need lives in `W`, so a snapshot of `W` is the complete state.

| Cadence | Due when |
|---|---|
| Monthly | every tick |
| Quarterly | `tick % 3 == 2` (last month of each quarter) |
| Annual { month } | `tick % 12 == month` |

Tick `t` runs: `world.begin_tick(t)`, each due system in order, `world.end_tick(t)`
(invariants), then `tick = t + 1`, then a snapshot if `tick % snapshot_every == 0`.
A snapshot is also taken at tick 0. Declared read and write sets for concurrent
systems (19.4) arrive with the first module that needs them.

## Ledger (Section 4.6)

- `transfer(flow, from, to, amount)` moves money between two accounts with checked
  `i64` arithmetic; overflow halts the run. Every transfer is journaled with its
  `Flow` tag for later sector flow matrices.
- `ship(from, to, good, qty)` moves goods between accounts.
- `create(account, good, qty)` and `destroy(account, good, qty)` are the only ways
  goods appear or disappear (production, consumption, spoilage); both are tallied.
- Money scale (minor units per currency unit) is a P5 decision; P0 uses one numeraire.

## Deterministic reduction (Section 19.4, parallelism)

`fixed_sum(xs)` splits `xs` into chunks of 1,024 (independent of thread count), sums
each chunk with a pairwise tree, then combines chunk sums with a pairwise tree. The
tree shape depends only on `xs.len()`, so 1 and 8 threads give identical bits.

## Snapshots (Sections 4.1, 14.3)

Canonical encoding: little-endian, `f64` by bit pattern, slices length-prefixed,
prefixed with magic `CETS` and a format version. State hash = BLAKE3 of
(tick, canonical bytes). Stored bytes are zstd-compressed and content-addressed by
the hash, so identical states share one blob. `Sim::seek(t)` restores the nearest
snapshot at or before `t` and replays. See ADR 0002 for why this is not Arrow yet.

`State::restore` rebuilds a world from its encoding by calling the live world, so a
world may keep static inputs (kernels, data) in an `Arc` outside the snapshot and
encode only a digest of them; restore reuses the live handle.

## Invariants

| Invariant | Statement | Tolerance | Checked where |
|---|---|---|---|
| Money | Sum of all balances is zero | exact | `Ledger::check`, every tick |
| Goods | For each good: closing total = opening + created - destroyed | 1e-9 relative | `Ledger::check`, every tick |
| Goods sign | No holding is negative | exact | `Ledger::check`, every tick |
| Closed accounts | Closed accounts hold no money or goods | exact | `Ledger::close` |
| Determinism | Same inputs give the same state hash at every snapshot | exact | `tests/p0_acceptance.rs` |
| Thread independence | 1 and 8 threads give the same hashes | exact | `tests/p0_acceptance.rs` |
| Replay | Restore from any snapshot and replay reproduces later hashes | exact | `tests/p0_acceptance.rs` |

## Toy world (P0 fixture)

`n` provinces on a ring, each with a household and a merchant account, plus a
dynamic table of firms; one good; a mint account issues the initial money. Monthly:
produce (stochastic, normal draws), wholesale, wages, retail, ring trade, prices by
the stock-flow rule of Section 8.2, merchant spoilage. Quarterly: firm exit and
entry (entity churn, account open and close). Annually: a census event using
`fixed_sum`. Constants are fixture values, not parameters, and are not in the
register.

## Parameters

None registered. Toy constants live in `toy.rs` and die with it in P2.

## Equilibrium nesting

n/a: the core holds no behaviour.

## Test plan

- Philox known-answer vectors from Random123.
- Property tests: ledger operations keep money summing to zero; snapshot encode and
  decode round-trip; `fixed_sum` is identical across thread pools; `compact`
  preserves order; random seeds never violate toy invariants.
- Acceptance (P0 gate): 10-province toy, 1,200 ticks, two runs identical; 1 vs 8
  threads identical; restore-and-replay from every snapshot matches.
- Golden: `tests/golden/p0-toy.txt` stores the snapshot hashes; set
  `CETERIS_BLESS=1` to rewrite deliberately, and say why in the commit.
- Benchmarks (criterion): Philox throughput, `fixed_sum`, toy tick.

## Open questions

- Money scale per currency and overflow headroom (P5).
- Journal retention beyond one tick, and the on-disk run store (P3).
