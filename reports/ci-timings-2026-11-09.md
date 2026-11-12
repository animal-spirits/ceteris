# CI timings and queue waits, atbraz/ceteris (2026-11-09)

Source: GitHub Actions job data via the GitHub MCP (read-only). Window: runs from 2026-11-09 02:36Z to 13:09Z.

Sample: 24 `rust` jobs from the 25 most recent non-cancelled `ci` runs (the 50 most recent completed runs were 456 in total for `ci`; cancelled runs and a schedule run without a `rust` job were skipped). 10 `rows` jobs from the most recent `rows` runs. Durations are job started_at to completed_at. Queue wait is job created_at to started_at. Percentiles are nearest-rank. Small sample, one day only.

## Where jobs run

- `ci` `rust` on push to main: `mbp-ceteris` (Mac, runner_id 21).
- `ci` `rust` on pull_request: `do-ceteris-1` (droplet, runner_id 25, label ceteris-linux).
- `rows`: always `do-ceteris-1`.
- Each runner is a single process. Every job on a machine runs one at a time, so jobs queue behind each other.

## Summary

| Set | n | Duration median | Duration p90 | Queue median | Queue p90 | Queue max |
|---|---|---|---|---|---|---|
| ci rust, push, Mac | 10 | 8m08s | 8m34s | 5.5s | 9s | 10s |
| ci rust, PR, droplet | 14 | 12m22s | 17m31s (max 32m08s) | 7m18s | 29m00s | 41m06s |
| rows, droplet | 10 | 6s | 14s | 19m12s | 38m17s | 44m20s |

## ci rust samples (seconds)

Mac, push to main (runner mbp-ceteris):

| Run | Queue | Duration | Result |
|---|---|---|---|
| 42574902383 | 9 | 457 | ok |
| 42569928477 | 10 | 541 | failed |
| 42561933664 | 3 | 500 | ok |
| 42560779303 | 1 | 514 | ok |
| 42559619735 | 1 | 504 | ok |
| 42558837659 | 8 | 456 | ok |
| 42555917442 | 1 | 337 | failed (nextest) |
| 42554490286 | 8 | 495 | ok |
| 42554078779 | 8 | 468 | ok |
| 42551966173 | 1 | 480 | ok |

Droplet, pull_request (runner do-ceteris-1):

| Run | Queue | Duration | Result |
|---|---|---|---|
| 42573438442 | 885 | 737 | ok |
| 42573348604 | 2466 | 718 | ok |
| 42573253846 | 1739 | 775 | ok |
| 42571710550 | 797 | 1051 | ok |
| 42569895086 | 10 | 1928 | ok |
| 42561495794 | 243 | 872 | ok |
| 42561209955 | 1298 | 819 | ok |
| 42560044831 | 434 | 706 | ok |
| 42559567882 | 7 | 728 | ok |
| 42557600646 | 1 | 826 | ok |
| 42554814805 | 500 | 689 | failed (nextest) |
| 42554441158 | 7 | 720 | ok |
| 42552298805 | 441 | 747 | ok |
| 42551909939 | 28 | 675 | ok |

## Where the droplet time goes

Step durations, median-ish, from the same jobs:

| Step | Mac (push) | Droplet (PR) |
|---|---|---|
| Stale-golden check | about 10s | 1m40s to 4m |
| Data-source licence manifest and research pack profile | about 1m15s | 4m to 7m30s |
| install-action | about 1s | 30s to 1m10s |
| cargo nextest | 4m to 5m | 4m to 6m40s (one 18m outlier, run 42569895086) |

PR jobs skip the determinism gate and bench build, so those steps are not compared. The cargo steps (fmt, clippy, deny) are similar on both. The gap is in the non-cargo steps, which run roughly 4 to 10 times slower on the droplet.

## Droplet size

From `runners-droplet-runbook-v2.md`: AlmaLinux 9, 2 vCPU, 4 GB RAM, shared with the yan-lang project. Not verified against the live droplet; the API did not expose droplet size.

## Interpretation

1. Most of the wait on the droplet is queueing, not job speed. The median PR rust job waits 7 minutes and the p90 waits 29 minutes. `rows` (6s of work) waits a median of 19 minutes behind rust jobs. One runner per machine means a bigger droplet alone does not shorten the queue.
2. The droplet is slow on the non-cargo steps. Resizing would cut the roughly 5 to 10 minutes of pack-check and golden-check time per PR job.
3. The Mac is not queue-bound. Its push jobs start within 10 seconds.

## Recommendation

Upgrade the droplet, and also register a second runner on it. The runbook's minimum for two ceteris runners is 4 vCPU and 8 GB each, with 8 vCPU and 32 GB recommended. A bigger droplet with one runner fixes speed but not the queue. Two runners on the current 2 vCPU box would only contend for the same CPU. Confirm the yan-lang load before sizing. I did not check current DigitalOcean pricing.
