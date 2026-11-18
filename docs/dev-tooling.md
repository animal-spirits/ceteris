# Developer tooling

## Tests: cargo-nextest

`cargo install cargo-nextest --locked`, then `cargo nextest run --workspace` (doctests stay
with `cargo test --workspace --doc`). Config in `.config/nextest.toml`:

- Default profile: no retries, so a flaky test fails locally.
- `--profile ci`: two retries; a pass on retry is reported FLAKY and is a bug to file. Golden
  and acceptance binaries never retry, since a retry would hide drift.

## Builds: sccache

`cargo install sccache --locked`, then `export RUSTC_WRAPPER=sccache` in your shell profile
(not committed: the repo works without it). `sccache --show-stats` shows hits. It does not cache
incremental builds, so set `CARGO_INCREMENTAL=0` where you want hits. CI use is in the workflow
and is changed only by an approved PR (agents do not edit `.github/`).

## Profiling the tick

`scripts/profile_tick.sh hyperfine|samply|flamegraph [criterion filter]` builds the `goods`
bench with line tables and runs it; default filter `tick/45` (45 goods, target 160 ms; the
17 November profile in `reports/tick-profile-2026-11-17.md` has the current spans). Install
`samply`, `flamegraph` with `cargo install`, and `hyperfine` from the package manager. Use
`CETERIS_PACK` for the real pack. Numbers worth quoting come from `scripts/bench_quiet.sh`;
this script is for finding where the time goes and for before/after checks.

## Run notifications

`scripts/notify.sh STATUS [LOG] [LABEL]` posts to ntfy. Set `CETERIS_NTFY_TOPIC` in your own
environment (an unguessable topic name, never committed); without it the script does nothing.
`CETERIS_NTFY_SERVER` overrides `https://ntfy.sh`. In a detached run:

```
log=~/ceteris-logs/run.log
trap 'scripts/notify.sh $? "$log" run-name' EXIT
```
