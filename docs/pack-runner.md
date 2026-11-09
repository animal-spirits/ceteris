# Nightly full-pack gate

`.github/workflows/pack-gate.yml` runs `scripts/pack-gate.sh` every night at 03:00 US Central,
and on demand from the Actions tab (Run workflow). It runs on a second self-hosted runner with the
`pack-gate` label, separate from the CI runner (`ceteris`), so a long gate never queues PR checks. It never runs on pull requests.

## One-time setup on the Mac

1. Register a second runner in its own directory (token: repo Settings, Actions, Runners, New self-hosted runner):
   ```
   mkdir -p ~/actions-runner/pack-gate && cd ~/actions-runner/pack-gate
   # unpack the same runner tarball used for ~/actions-runner/ceteris, then:
   ./config.sh --url https://github.com/atbraz/ceteris --token <TOKEN> \
     --name mbp-ceteris-pack --labels pack-gate --unattended
   echo 'CETERIS_PACK=/Users/antonio/Code/Projects/ceteris/data/packs/2000' >> .env
   ./svc.sh install && ./svc.sh start
   ```
   The CI runner keeps its `ceteris` label and no longer takes the gate. The first run compiles
   from scratch in the new work directory. The two runners share the Mac's cores, so avoid
   running the tick-budget benchmark while the gate runs.
2. Commit the workflow, script and this file to main yourself. The `rows` guard fails any
   `claude/*` branch that touches `.github/`.
3. Run it once from Actions, then check the `pack-gate-logs` artifact.

## What it runs

The pack export into a scratch directory (`CETERIS_PACK_EXPORT`), followed by these tests in release:

- `cargo test --workspace --release --no-fail-fast`, which covers the econ golden and p2_acceptance,
  cli p2_gate, calibrate steady_state and benchmark, and the other pack-gated tests

Each step writes a log, `timings.log` has per-step seconds, and the logs are uploaded for 14 days.

While the pack goldens wait for a re-bless (as after #128, until the next pack build), the
job is expected to fail on those golden tests.
