# RFC 0285. Interventions replay on seek, and parameters are snapshot state

Status: proposed
Date: 2026-11-01
Design sections: 14.3
Research: P3 readiness review, 2026-11-01 (project files, `research/p3-readiness-2026-11-01.md`); pre-P3 plan, Branch D (`specs/pre-p3/PLAN.md`)

## Summary

Section 14 makes every experiment a branch: a parent run plus an ordered list of timed edits,
with any tick reachable by loading the nearest earlier snapshot and replaying. Before this RFC
the P2 world could not do that across an edit. `EconWorld::set_network` and any parameter change
moved the static digest that opened every snapshot, so a snapshot taken before the edit could
not be restored after it, and `Sim::seek` replayed from an earlier snapshot without the edit,
silently losing it. Parameters lived outside snapshots altogether, so an ex-post parameter edit
(14.2) could not survive a restore. P3 B1, B2 and C1 need both. This RFC records how pre-P3
Branch D (tasks D1 to D3) closes the gap. It replaces the ADR the pre-P3 plan planned as 0113 and
then as "the next free number".

As built: `Sim` keeps an intervention log; snapshots open with the inputs digest without the
network, the network's digest and both parameter sets (snapshot format 3), followed by the same
dynamic state as before; `EconWorld::set_params` and `set_trade_params` join `set_network` as
between-tick edits; a restore rebuilds the kernel and layout where the snapshot's network or
parameters differ from the live world's. The calibrated-load guard (`structural_digest`, ADR
0089) is unchanged: calibrated tables start only under the structural parameters they were built
under, and an edit after load is how a structural sweep starts from the baseline steady state.
`static_digest` keeps its meaning (inputs and both parameter sets), so `run.json` and the
calibration guard do not move. The dynamic state of the fixture run is bit for bit what it was
on main f3bd6a3; only the encoding changes, which re-blesses `p0-toy.txt` and `p2-econ.txt`.

## Decisions

### D1. How should seek treat an intervention made between ticks?

Options:

- A. Log and replay, a fork drops the later timeline: `Sim::intervene` applies the edit and
  records it at its tick; seek reapplies recorded edits as it passes their ticks (a snapshot at a
  tick holds the world before that tick's edits). A new edit after a seek back discards the
  snapshots and recorded edits later than its tick.
- B. Log and replay, edits only at the tip: as A, but an edit is refused unless the run is at
  its latest tick; a fork needs a new `Sim`.
- C. No log: `Sim` is unchanged and a seek across an edit loses it unless the caller reapplies it.

Recommendation: A, because 14.1 already defines a branch as timed edits replayed from
snapshots, and dropping the stale future is what an edit after a seek back means. `world_mut`
stays for edits that seek should not replay.

Decision: pending.
Row: pending

### D2. How should a snapshot name the network it was taken on?

Options:

- A. By the network's content digest (32 bytes): the same network gives the same state hash
  whatever order it was set in.
- B. By a version number (`u32` index into the run's network list), as the pre-P3 plan wrote it.

Recommendation: A, because state hashes then depend only on content, which keeps forks that
set the same networks comparable and makes a null intervention reproduce the parent by
construction (14.3).

Decision: pending.
Row: pending

### D3. Are a run's parameters snapshot state?

Options:

- A. Yes: every snapshot carries `EconParams` and `TradeParams` (about 300 bytes on the fixture),
  and a restore takes the snapshot's set, rebuilding the kernel and layout where it differs.
- B. No: parameters stay static, and a parameter edit cannot be sought across.

Recommendation: A, as the pre-P3 plan ruled (Task D2): 14.2 lists parameter edits as ex post,
and an ex-post edit must survive a restore.

Decision: pending.
Row: pending

## As built

- Network history: the run keeps every network it has had, by digest, shared between a world
  and its restored copies and not state. A snapshot naming a network the restoring run has never
  had is a `DecodeError`, as is a snapshot of other inputs or another format version.
- Kernel rebuilds: a restore or edit that changes the network or the trade parameters builds a
  fresh kernel, without crossing lags set by `set_crossing_lags` (as `set_network` already did).
- Structural edits: `set_params` and `set_trade_params` leave the dynamic state as it is. An edit
  of a structural parameter, the PIGL parameters included, moves a calibrated world off its
  steady state, since the calibration inverted its weights at the defaults.
- Warm starts (ADR 0097, D3): the only iterative solve that enters state and starts from state is
  the frictionless clearing wage of E1, which starts from the last wage, itself state. The
  same-tick local chain starts from fills of 1 every tick; the PIGL inversion, the lognormal
  dispersion and the Gauss-Hermite nodes run at load into the layout, which a restore rebuilds
  from inputs and parameters; storage (ADR 0280) is closed form; `settle`'s income iteration
  runs only at a start. The seek tests run in both modes.
- Tests: `seek_across_network_change_replays`, `null_intervention_reproduces_parent`,
  `seek_across_param_edit_replays` (default and frictionless; a seek back and a seek forward from
  a live world on the other side of the edit), `theta_sweep_starts_from_baseline_steady_state`,
  `load_guard_still_panics_with_other_structural_params`, `restore_takes_the_snapshots_parameters`,
  `restore_rejects_a_network_the_run_has_not_had`, and in P0 `seek_across_an_intervention_replays_it`
  and `intervening_after_a_seek_back_drops_the_later_timeline`.

## Spec changes

- 14.3: snapshots carry the run's parameters and name its network; seek replays the logged
  interventions; an intervention after a seek back drops the later timeline (rows pending).
- `specs/econ/SPEC.md`, Initial state: the load guard and the intervention path.
