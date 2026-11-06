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
| Money balances | `Ledger` | `i64` minor units per narrow account, `i128` per wide account (ADR 0106) | Negative balance = net liability (issuers); the kind is chosen at `open` |
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

- Accounts are narrow (`open`, `i64` balances: pops, enterprises, firms, households) or wide
  (`open_wide`, `i128` balances: accounts that aggregate many others, such as pools, the mint,
  markets and gateways; ADR 0106). `money(a)` reads a balance as `i64` and panics where it does
  not fit; `money_wide(a)` reads any balance as `i128`.
- `transfer(flow, from, to, amount)` moves an `i64` amount between two accounts of either
  kind with checked arithmetic; a narrow balance leaving the `i64` range, or a wide one the
  `i128` range, halts the run (`ledger overflow`). Every transfer is journaled with its
  `Flow` tag for later sector flow matrices.
- `ship(from, to, good, qty)` moves goods between accounts.
- `create(account, good, qty)` and `destroy(account, good, qty)` are the only ways
  goods appear or disappear (production, consumption, spoilage); both are tallied.
- `take_columns(n, cols)` and `put_columns(cols)` lend goods `0..n` out as one `GoodColumn` per
  good (holdings of every account and the good's created and destroyed tallies), whose
  `ship`, `create` and `destroy` are the ledger's own on that good, so per-good moves run in
  parallel with the same arithmetic in the same order (RFC 0287 item 2; test
  `columns_are_goods_operations_in_turn`).
- `track(flows)` (flow ids below 64) makes the ledger keep, per account, the net amount of this
  tick's entries of those flows as it posts them, from `transfer` and `post_all` alike;
  `tally(a)` reads it and `begin_tick` zeroes it. It equals the journal's sum over those flows
  (test `tally_is_the_journal_sum_of_tracked_flows`), so an invariant over it need not scan the
  journal (RFC 0287 item 3).
- Money scale (minor units per currency unit) is a P5 decision; P0 uses one numeraire.

## Deterministic reduction (Section 19.4, parallelism)

`fixed_sum(xs)` splits `xs` into chunks of 1,024 (independent of thread count), sums
each chunk with a pairwise tree, then combines chunk sums with a pairwise tree. The
tree shape depends only on `xs.len()`, so 1 and 8 threads give identical bits.

## Timing spans (Section 19.4; ADR 0294)

`timing` records wall-time spans for the sim-loop harness: `enable()` turns it on for the
process, `span(name, good)` returns a guard that records (tick, name, good, rayon worker,
start, end) when it drops, and `take()` returns and clears the spans. While it is off a span is
one relaxed atomic load. `Sim::step` spans the tick, `begin_tick`, `end_tick` and the snapshot;
`Schedule::run_tick` spans each system by name; econ adds the T2b phases (`t2b.goods`,
`t2b.moves`, `t2b.post`, `t2b.transit`), each good's T2b task and its parts (`t2b.good`,
`t2b.kernel`, `t2b.record`, `t2b.legs`), `e2.local_chain` and `e3.price_refresh`. Spans read
only the clock, so state hashes and the journal are the same with timing on or off
(checked by `cargo run --release -p ceteris-econ --example identity`, which prints the
state hash and journal hash per tick). The QoS class of each worker is not recorded: reading it
needs an FFI call, which is `unsafe` outside ADR 0295's exception.

## Hazard draws (P3; ADR 0241)

`hazard::draw(seed, tick, hazard, cell)` returns one uniform per (hazard, cell, tick) from the
Philox stream keyed by `SystemId::HAZARD`, `HazardId` and the cell, whether or not the rate is
positive; an event fires when the uniform is below the rate. Branches therefore share the events
of untouched hazards, and a zero-rate hazard leaves every hash unchanged. Member parameter draws
use `SystemId::PARAM_DRAW` keyed by parameter id (ADR 0322). Specified in `specs/workbench/SPEC.md`.

## Chunk store (P3; Sections 14.3, 19.4)

Snapshots stay the canonical encoding below and the state hash stays over its bytes. P3 adds a
content-addressed store of zstd Arrow IPC chunks of at most 65,536 rows per table, a snapshot being
a manifest of (table, chunk hash); `restore` decodes chunks back to the canonical state and checks
the hash. Chunks from another engine version are a `DecodeError`. Layout, retention and bounds are
in `specs/workbench/SPEC.md`.

## Snapshots (Sections 4.1, 14.3)

Canonical encoding: little-endian, `f64` by bit pattern, slices length-prefixed,
prefixed with magic `CETS` and a format version (2 since ADR 0106: the ledger writes each
account's kind and its balance as `i64` or `i128`). A snapshot of another format version is a
`DecodeError` naming the version, never a misread. State hash = BLAKE3 of
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
- Journal retention beyond one tick (the on-disk run store is specified in `specs/workbench/SPEC.md`).
