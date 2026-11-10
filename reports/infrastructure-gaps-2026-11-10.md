# Infrastructure gaps for 1.0 and sustained development, 10 November 2026

Scope: what ceteris still needs beyond the droplet runners (do-ceteris-1/2), the Mac runners (mbp-ceteris, mbp-ceteris-pack), the nightly pack gate, `scripts/package_app.sh` and the Steam depot scripts in `release/steam/`.
Sources read: `docs/release-steam.md`, `docs/privacy.md`, `docs/pack-runner.md`, `specs/release/PLAN.md`, RFCs 0214, 0215, 0216, `reports/pre-1.0-polish-plan-2026-11-09.md`, `reports/ci-timings-2026-11-09.md`, `.github/workflows/`, queue #179, and the droplet runbook.
All costs and efforts are my estimates, not measurements. Prices and platform rules are from memory; verify before committing money. Effort is owner-weeks on RFC 0313's basis unless stated. Items marked [.github] cannot be done by agents (CLAUDE.md); Antonio applies them.

## Ranked gaps

| # | Gap | 1.0 impact |
|---|---|---|
| 1 | Release workflow: signing, notarisation, upload | Blocks ship |
| 2 | Single physical Mac holds CI, pack, signing and the only pack copy | Blocks ship, bus factor |
| 3 | Redistributable pack hosting and Zenodo archive | Blocks first run |
| 4 | Cross-platform hash job (macOS, Linux x86_64, Linux aarch64) | Blocks R gate |
| 5 | Crash and problem-report path | Blocks P7 |
| 6 | Usage-stats collector (RFC 0214) | Optional, can drop |
| 7 | Source URL drift job | Medium |
| 8 | Secrets handling | Medium, grows with 1 |
| 9 | Backups of owner-only data | Medium |
| 10 | Update delivery | Low (Steam does it) |
| 11 | Docs and community site | Low to medium |
| 12 | Cost and runner monitoring | Low |
| 13 | Clean-machine test fleet | Medium, acceptance of #411 |

### 1. Release workflow, signing, notarisation, upload

- What: `package_app.sh` stages an unsigned bundle per host. Nothing signs, notarises, builds Linux artifacts on a clean runner, checks `SHA256SUMS`, or runs `steamcmd`. The Steam vdfs hold placeholder ids. RFC 0216 plans a release workflow (build, sign, notarise, checksums, cask, formula) not yet written.
- Cheapest: a manual `scripts/release.sh` run on the Mac that signs with `codesign`, notarises with `notarytool` using a keychain profile, and fails closed without credentials (spec Branch D test). Upload with `steamcmd` by hand. A GitHub release workflow [.github] comes only if releases become frequent. Skip the Homebrew cask and formula for 1.0; Steam is the channel.
- Cost: Apple Developer Program 99 USD per year; Steamworks app fee 100 USD per app (verify). Effort 1 to 1.5 owner-weeks for the script and a notarisation dry run. Needs Antonio's Apple and Steam accounts.

### 2. One Mac is the single point of failure

- What: the Mac is the `ceteris` push runner, the `pack-gate` runner, the only holder of `data/packs/2000` (`CETERIS_PACK` points at it), the quiet-host benchmark machine, and the only place a notarised build can come from. Antonio has no physical access at times (memory). A dead disk or a macOS upgrade stops merges to main, nightly gates and releases. Droplet PR CI already queues 7 to 41 minutes (`reports/ci-timings-2026-11-09.md`), so the droplet cannot absorb Mac work.
- Cheapest: (a) keep a second copy of the built pack and the fetch cache off the machine (see 3 and 9); (b) write a one-page rebuild runbook for the Mac runners, like the droplet runbook; (c) add a second Mac-class build host only if the Mac proves unreliable. A rented Apple silicon cloud Mac (about 25 to 100 USD a month on demand, 24 hour minimum on some hosts; verify) is the fallback for signing and Mac-only golden runs.
- Cost: runbook 0.2 weeks; cloud Mac on demand 0 until needed.

### 3. Redistributable pack hosting and source archive

- What: ADR 0215 and the P14 decision need a prebuilt redistributable pack in the download plus a Zenodo archive of redistributable sources per release. Nothing exists to build, version, upload or fetch it. The Steam depot size limit and pack size (about 6 GB of sources; the built pack size is not in the docs I read) are unmeasured. Licence audit (#231, #174) gates what may be included.
- Cheapest: ship the pack inside a Steam depot if it fits; otherwise a GitHub release asset (2 GB per file limit; verify) or Zenodo (50 GB per record; verify, one DOI per release as in ADR 0215). `pack build` stays a Mac owner action; add `scripts/publish_pack.sh` to hash, upload and record the manifest.
- Cost: free tiers likely cover it; effort 1 to 2 owner-weeks (P14, already planned). Unknown: pack size, so measure first.

### 4. Cross-platform hash job (Branch H, #203)

- What: the golden hash is checked on the Mac and, manually, on quant-dev (Linux x86_64, 9 Nov). There is no job for Linux aarch64 (now a shipped platform), and none for Windows. The job touches `.github/`.
- Cheapest: Linux x86_64 on the droplet in the PR workflow, Linux aarch64 on a free or cheap Arm runner (GitHub-hosted Arm runners are free for public repos only, and the repo is private until 1.0; verify) or a 4 vCPU Arm droplet at roughly 20 to 40 USD a month. Windows waits for demand (RFC 0334).
- Cost: effort 0.5 to 1 week plus Antonio applying the workflow; about 30 USD a month if an Arm host is rented.

### 5. Crash and problem-report path

- What: RFC 0214 writes bundles locally and sends them only if the user chooses "report a problem". The destination is not defined. Options below. Without one, Steam users have no path to the author.
- Cheapest: "report a problem" opens a prefilled GitHub issue URL and tells the user to attach the bundle (zero backend). Steam community hub discussions can serve as the fallback. No automatic upload, so no privacy exposure.
- Cost: 0.3 week. A hosted collector (Sentry, a Cloudflare Worker with R2) is only needed for automatic upload; skip for 1.0.

### 6. Usage-stats collector

- What: Antonio chose opt-in statistics (2026-10-28 card) in RFC 0214, which needs a hosted collector, an operator, a privacy notice and a retention policy. The privacy statement already promises counters only and no identifier.
- Cheapest: a Cloudflare Worker plus a free D1 or KV store, about 0 to 5 USD a month, about 1 week including the network test ("with statistics off, no request"). Alternative: drop statistics for 1.0 and amend RFC 0214 (the app then makes no network request except the optional update check and fetch). That removes a hosted service, a GDPR surface and an operator.
- Recommendation: drop for 1.0, revisit after launch.

### 7. Source URL drift job

- What: RFC 0215 specifies a monthly CI job that checks every pinned URL. It touches `.github/` and is not built. Upstream publishers move files; without it the fetch silently breaks for users.
- Cheapest: a script `scripts/check_source_urls.sh` (HEAD requests plus hash on small files) that Antonio wires to a monthly workflow [.github]. Effort 0.3 week.

### 8. Secrets handling

- What: needed secrets at 1.0: Apple notarisation profile, Steamworks builder account (steamcmd with Steam Guard), runner registration tokens, droplet SSH keys, any Zenodo token. No document says where each lives. The Mac runner has repo access via a long-lived service and builds PR code on the same machine that will hold signing keys.
- Cheapest: signing and Steam uploads only from Antonio's own session or a login keychain, never from the runner account used for PR jobs; a dedicated Steam builder account with only the upload permission; Zenodo token in the keychain. Record the list (not the values) in `docs/release-steam.md`. While the repo is private, PR CI runs only trusted code; reassess when it opens, since self-hosted runners on a public repo are a standard attack path (restrict fork PR workflows).
- Cost: 0.2 week; 0 money.

### 9. Backups of owner-only data

- What: owner-only items not in git: built pack, fetch cache with 13 manual downloads (three providers, registration), `~/ceteris-logs/`, Mac runner state, pending licence correspondence, Steamworks and Apple credentials. Losing the manual downloads means redoing registrations.
- Cheapest: Time Machine or `restic` to an external disk and an encrypted cloud bucket (Backblaze B2 about 6 USD per TB per month; verify). Also record manual-file hashes in `fetch.toml` (partly done by hash pinning).
- Cost: under 10 USD a month; 0.2 week.

### 10. Update delivery

- What: RFC 0216 has the app ask GitHub Releases for a new version. With Steam the app is updated by the Steam client, and a private repo has no public releases API.
- Cheapest: no in-app update check for the Steam build; Steam is the update channel. Keep the notice only for a future direct-download channel. Amend RFC 0216 accordingly.
- Cost: negative effort.

### 11. Docs and community site

- What: mdBook user guide (P15), Discussions, issue templates and CONTRIBUTING (P10, [.github]), a store page, and a public site. No hosting set.
- Cheapest: mdBook built to GitHub Pages (needs a public repo or a paid plan for private; verify) or Cloudflare Pages free tier. No custom domain at 1.0, or one at about 10 to 15 USD a year.
- Cost: about 0 money; effort in P10 and P15.

### 12. Cost and runner monitoring

- What: droplet specs (2 vCPU, 4 GB) already queue PR jobs for tens of minutes. Nobody is alerted when a runner goes offline, a droplet fills its disk, or the nightly gate fails silently. Actions minutes ran out once (27 October).
- Cheapest: a daily check job on the droplet that posts offline runners and disk use to a GitHub issue or an email; a budget cap and alerts on the DigitalOcean account; upgrade the droplet to 4 vCPU and 8 GB (about 48 USD a month, verify) if queueing continues.
- Cost: 0.2 week; 12 to 48 USD a month for the droplet.

### 13. Clean-machine test fleet

- What: the acceptance of #411 and gate H need an install and launch on a clean Mac and a clean Linux machine, including the 16 GB minimum-hardware run (P7) and Vulkan floor (RFC 0334). Antonio's Mac is not clean.
- Cheapest: a macOS VM (Tart on the Mac, 16 GB guest if host allows) and a Linux VM or a cloud Arm and x86 instance for a few hours per release. Cost under 20 USD per release; 0.5 week to script.

## Not needed

Windows build and signing (waits for demand, RFC 0334); an in-game download menu (#265); a paid-build pipeline (2.0, #262); telemetry beyond 6; trademark filing (#261).

## Suggested order

1. Items 1 and 2 first: they gate shipping and are single points of failure. Start with the Mac rebuild runbook and the signing script dry run.
2. Items 3 and 9 together: one decision on where the pack lives fixes hosting and backup.
3. Item 4 with the J2 gate; 5, 7, 8 are small and can go in one batch.
4. Defer 6, 10, 11, 12, 13 until the R gate approaches.

Total new effort if all recommended choices are taken: about 4 to 6 owner-weeks, of which P8 (Steam path) already covers about 1 to 1.5 and P14 covers 1 to 2. Recurring cost: about 10 USD a month (Apple fee spread) plus backups plus an optional droplet upgrade, so roughly 25 to 80 USD a month.

## Decisions for Antonio

1. Statistics collector (item 6): drop for 1.0 and amend RFC 0214, or build the minimal Worker. Recommended: drop.
2. Where the redistributable pack lives (item 3): Steam depot, GitHub release assets, or Zenodo only. Recommended: Steam depot if size allows, with the Zenodo record as the archive of record.
3. Mac single point of failure (item 2): runbook plus backups only, or also commit to a second build host now. Recommended: runbook plus backups, rent a cloud Mac only on failure.
4. Problem reports (item 5): prefilled GitHub issue, or a hosted collector. Recommended: prefilled GitHub issue.
5. Update check (item 10): Steam only, or keep the GitHub check. Recommended: Steam only.
6. Linux aarch64 hash coverage (item 4): rent a small Arm host, or defer to a manual check per release. Recommended: manual check per release until the repo opens.

## Decisions taken (Antonio, 2026-11-10)

1. Statistics collector: dropped for 1.0; RFC 0214 to be amended.
2. Pack location: Steam depot, with the Zenodo record as archive of record.
3. Mac single point of failure: runbook plus backups; rent a cloud Mac only on failure.
4. Problem reports: prefilled GitHub issue.
5. Update check: Steam only.
6. Linux aarch64 hash coverage: rent a small Arm host (not the recommended manual check).
