# Nightly full-pack gate

`.github/workflows/pack-gate.yml` runs `scripts/pack-gate.sh` every night at 03:00 US Central,
and on demand from the Actions tab (Run workflow). It runs on the self-hosted runner already
registered for CI (labels `self-hosted, macOS, ARM64, ceteris`) and never on pull requests.

## One-time setup on the Mac

1. Point the runner at the pack. In the runner's directory (where `config.sh` lives), add one line to `.env`:
   `CETERIS_PACK=/Users/antonio/Code/Projects/ceteris/data/packs/2000`
   Then restart the service: `./svc.sh stop && ./svc.sh start`.
2. Commit the workflow, script and this file to main yourself. The `rows` guard fails any
   `claude/*` branch that touches `.github/`.
3. Run it once from Actions, then check the `pack-gate-logs` artifact.

## What it runs

The pack export into a scratch directory (`CETERIS_PACK_EXPORT`), followed by these tests in release:

- `ceteris-econ` golden and p2_acceptance (100 years, 1 against 8 threads)
- `ceteris-cli` p2_gate
- `ceteris-calibrate` steady_state and benchmark
- the whole workspace, which picks up the other pack-gated tests

Each step writes a log, and the logs are uploaded for 14 days.

While the pack goldens wait for a re-bless (as after #128, until the next pack build), the
job is expected to fail on those golden tests.
