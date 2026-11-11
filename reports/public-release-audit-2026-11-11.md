# Public-release audit, 2026-11-11

Scope: read-only audit of `atbraz/ceteris` at the head of `main` and all 87 commits (all refs) before the repo goes public (later moving to `animal-spirits/ceteris`). No history was rewritten. No `.github/` file was edited; changes there are proposals (section 3).

Method: no gitleaks/trufflehog in the sandbox, so regex scans over `git log --all -p` and `git grep`: AWS, GitHub, Slack, OpenAI, Google keys, PEM blocks, `key|secret|token|password = "..."`, `.env`/credential/key filenames, e-mail addresses, `/Users/` and `/home/` paths, IPv4 addresses, blobs over 500 kB, data-like extensions. Recommended: run `gitleaks detect --log-opts="--all"` once on the Mac as a second opinion before flipping visibility.

## 1. Secrets and personal data

| # | Finding | Severity | Fix |
|---|---|---|---|
| 1.1 | No API keys, tokens, private keys, passwords, `.env` files or credentials in any commit or filename. Runner registration tokens are not in the tree (`docs/pack-runner.md`, `docs/mac-recovery.md` say to fetch them from Settings). | none | Run gitleaks once for confirmation. |
| 1.2 | Rust build output is tracked: 97 files under `scripts/clippy-bans/target/` (rustc fingerprint, `.d` files, 3 MB `librayon` rmeta), added in one commit (`e5d9c29`, #400). They contain `/Users/antonio/Code/Projects/ceteris/.claude/worktrees/reverent-merkle-18dcd3/...`, the local username, toolchain path and host triple. `.gitignore` covers `/target` and `app/target/` only. | low (username and a worktree name; no credentials) | Follow-up PR: `git rm -r --cached scripts/clippy-bans/target`, add `scripts/clippy-bans/target/` to `.gitignore`. History keeps the files; acceptable, they hold nothing secret. Rewriting history is not recommended for this. |
| 1.3 | `docs/pack-runner.md:15` has `/Users/antonio/Code/Projects/ceteris/data/packs/2000`. | trivial | Replace with `$HOME/...` or `<repo>/data/packs/2000`. |
| 1.4 (owner chose rewrite) | Commit author e-mails become public: `antonio@redacted.invalid` (46 commits), `redacted@gmail.com` (17), plus `noreply@anthropic.com`. Real name "Antonio Braz" on all. Cannot be removed without rewriting history. | decision | Antonio: accept (recommended; both are already attached to public GitHub activity if any), or rewrite history with a mailmap (not recommended: breaks PR links, hashes, golden provenance). For new commits, enable "Keep my email addresses private" in GitHub settings. |
| 1.5 | `docs/mac-recovery.md` names the B2/S3 backup approach, `ssh mac`, runner paths and service names; no host names, IPs, keys or bucket names. `reports/infrastructure-gaps-2026-11-10.md` lists the secrets needed at 1.0 by name only (Apple notarisation profile, Steamworks builder, restic key). | low | Both read as an infrastructure map of the owner's Mac. Optional: move `mac-recovery.md` and `reports/infrastructure-gaps-*` out of the public tree (project files) before flipping. Recommended for `mac-recovery.md`; fine to keep the other. |

## 2. Licensed or restricted data

| # | Finding | Severity | Fix |
|---|---|---|---|
| 2.1 | No raw licensed data is committed. No EXIOBASE, OECD ICIO, EPR/GeoEPR, IEA, PWT, LIS, WTO or TRAINS tables anywhere in history. `exiobase` appears only in `sources.toml` (`commercial_use = false`, user-fetch only) and tests under `crates/` do not read it. | none | Add the planned test that EXIOBASE never feeds calibration (decided 2026-11-10) before or soon after public. |
| 2.2 | `tests/golden/p2-fixture/**` (Arrow) is generated with `ceteris export --fixture` (10 synthetic goods; `params.json` references only register ids), not from a source dataset. | none | State "synthetic" in a README beside it so no one reads it as ICIO-derived. |
| 2.3 | `crates/ceteris-pack/tests/data/pixel_cells.txt` (725 kB) records WorldPop raster window values and cell ids ("recorded from the Python pipeline"). WorldPop is CC BY 4.0. It is a small derived sample, so permitted, but it needs attribution. | low | Add a `NOTICE` entry (WorldPop, CC BY 4.0) and a header line in the file's test or README. |
| 2.4 | `crates/ceteris-calibrate/tests/data/benchmark/*.shocks` are tariff shock lists (US-China 2018, Brexit, border). Derived from public tariff data; the source of each file is not stated. | low | Add a provenance line per file (source and licence). If any came from UNCTAD TRAINS or CEPII MAcMap (both non-commercial per `sources.toml`), say so: fine for a non-commercial open repo, but it would then be excluded from the paid build. |
| 2.5 | `sources.toml` has 8 `LicenseRef-NoLicence`, 39 `commercial_use = false`, 1 `WTO-all-rights-reserved`, 2 CC BY-NC-SA 4.0 (EPR). The manifest is metadata only. | none | `pack check-sources --profile release` already gates the paid build. |
| 2.6 | No `NOTICE`, `THIRD_PARTY` or attribution file at the root. README links the licence manifest and says data is never bundled. Fonts carry their licences (`app/ui/fonts/LICENSE-*`: IBM Plex and Source Serif 4, both OFL). Mock art is self-drawn. | medium | Add a root `NOTICE`/`THIRD_PARTY.md`: fonts (OFL, with reserved-name note), WorldPop sample, any shock-file sources, and EXIOBASE licence citation (required by their 2026-11-10 reply: "individual eligible if public EXIOBASE licence is cited in docs"). Slint terms are decided in RFC 0088 (GPL-3.0 or royalty-free licence); `app/` is its own workspace excluded from `cargo deny check licenses`, so app dependency licences are unchecked: run `cargo deny` in `app/` once and record which Slint licence the public repo claims. |
| 2.7 | Dual licence `MIT OR Apache-2.0` stated in README and `Cargo.toml`; both licence files present. | none | None. |

## 3. CI hardening (proposals only; Antonio applies, `.github/` is owner-only)

Current state:
- `ci.yml` `rust` and `rows.yml` `rows` already skip fork PRs (`head.repo.full_name == github.repository`). `slow`, `gates`, `report` run only on schedule/dispatch; `app` and the bench/golden steps only on push. `pack-gate.yml` is schedule/dispatch only, with the comment that fork code must not reach the Mac. No `pull_request_target`, no secrets referenced. Good.
- Same-repo PRs run on `vars.CI_RUNNER_PR` (droplets), push to `main` on `mbp-ceteris`.

Gaps and changes:
1. **Workflow `permissions:`** Neither file sets a top-level `permissions:`; the token takes the repo default. Add `permissions: contents: read` at the top of `ci.yml`, `rows.yml`, `pack-gate.yml`; keep `issues: write` only on the `report` job (already set). Set the repo default to read-only (Settings, Actions, General, Workflow permissions).
2. **Fork approval:** Settings, Actions, General: "Fork pull request workflows from outside collaborators" = *Require approval for all outside collaborators*. The `if:` guard already keeps fork PRs off self-hosted runners; approval also stops hosted-minute abuse once PRs use hosted runners.
3. **Hosted runners for PRs:** after public, hosted runners are free for public repos. Set `CI_RUNNER_PR` to `["ubuntu-latest"]` (or leave the droplets) so untrusted code is never on owned machines. Keep `push`/`schedule` on the Mac.
4. **Self-hosted runner exposure:** a same-repo PR can edit a workflow and run it on the droplets, and a push to `main` runs on the Mac, where the pack and cache live. Mitigate: branch protection or a ruleset on `main` (PRs required, no direct push, CODEOWNERS review on `/.github/`), "Restrict who can create branches/tags" to collaborators you trust, and never add write collaborators you would not give shell on the droplet. Prefer ephemeral/JIT droplet runners; the Mac runners are persistent, so keep them to `main` pushes and schedules only (already the case).
5. **Runner groups (org move):** `animal-spirits/ceteris` needs runners re-registered at org or repo level; org runner groups should be restricted to this repo and, in the group settings, to selected workflows. Repo variables (`CI_RUNNER_PR`) and CODEOWNERS (`@atbraz`) must be re-created or checked after transfer. Do the transfer after the audit-fix PRs merge and re-check Actions settings after it.
6. **Third-party actions:** `actions/checkout@v4`, `actions/github-script@v7`, `actions/upload-artifact@v4`, `taiki-e/install-action@v2`. Pin by commit SHA (at least `taiki-e/install-action`), enable Dependabot for `github-actions`.
7. **`claude/*` guard in `rows.yml`** already blocks agent PRs from `.github/` and CODEOWNERS; keep it.
8. Enable secret scanning and push protection (free on public repos), private vulnerability reporting, and add `SECURITY.md`.

## 4. Docs naming and private notes

| # | Finding | Fix |
|---|---|---|
| 4.1 | The project name "Ceteris Paribus"/`ceteris` appears throughout the repo (it is the product); nothing in the repo names the Claude project or channel ("hearth", project ids). External-email constraint (do not name the project in outreach) is a policy for messages, not the repo; the public repo name itself will now reveal it. | Decision: outreach drafts that promise "public at first release" (EXIOBASE reply) should be updated before flipping to say it is public now. The six unsent licence drafts must not link the repo unless Antonio wants the name disclosed. |
| 4.2 | `docs/research/README.md` has 65 rows linking private `claude.ai/artifact/...` pages; 100 more links sit in 93 files under `rfcs/` (e.g. 0001, 0115-0122). 165 links total. Outsiders get a login wall; the links also advertise the AI-assisted workflow and artifact ids. | Recommended: keep the index text, strip the URLs (or replace with the file in `docs/research/` where one exists). A scripted rewrite PR touching `rfcs/` and `docs/research/` is an Antonio-merge PR (spec/RFC). |
| 4.3 | Owner-directed process text: `CLAUDE.md`, `docs/development-laws.md`, `.claude/` agents and rules, "Antonio alone decides", "coordinator", "question cards". Not secret; reads as internal workflow. | Keep. Optionally add a README line explaining that agents assist the work. |
| 4.4 | `reports/` (2 files) and `docs/mac-recovery.md`, `docs/privacy.md` (draft), `docs/release-steam.md`: internal plans, costs, revenue/price intent (Steam price in RFCs/ADRs, 14.99 USD paid 2.0). `docs/art-inventory.md` mentions third-party makers. | Decide per file whether business plans (price, Steam strategy, infra gaps) should be public. Recommended: remove `mac-recovery.md` and `reports/infrastructure-gaps-*` from the public tree; the rest is fine. |
| 4.5 | Named third parties appear only in `sources.toml` notes (UN, OECD, FAO contact addresses that are public provider addresses, e.g. `copyright@fao.org`, `stan.contact@oecd.org`, `exiobase-support@googlegroups.com`). No correspondence text or private contacts. The EXIOBASE verdict quote is from the public Zenodo page. | None. Do not commit the provider correspondence (Girardin, EXIOBASE replies) to the repo. |
| 4.6 | `spec/` and `specs/` contain `.superpowers/sdd/...` path references (an unversioned local tool dir) in `specs/econ/MIGRATION.md`. | Trivial; leave. |
| 4.7 | No `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, issue or PR templates. | Add before accepting outside PRs; state that spec/RFC/ADR changes need owner approval. |

## Recommended order

1. Antonio decisions (cards): 1.4 e-mail exposure, 4.2 strip claude.ai links, 4.4 which internal docs stay public.
2. One PR (non-`.github`, non-spec): untrack `scripts/clippy-bans/target`, fix `.gitignore`, fix `docs/pack-runner.md` path, add `NOTICE`, fixture and data provenance READMEs, `SECURITY.md`, `CONTRIBUTING.md`.
3. Antonio, in Settings: items 3.1, 3.2, 3.4, 3.8; `.github/` edits for 3.1, 3.3, 3.6 on a non-`claude/*` branch.
4. gitleaks on the Mac, then flip visibility, then transfer to the org and redo section 3.5.

## Not done / limits

- Regex scan, not a verified secret scanner. Binary blobs (fonts, PNG, Arrow) were checked only by name and size.
- Repo settings (branch protection, Actions permissions, collaborators, existing runner registrations) are not visible from this session; section 3 items on settings are unverified against the live repo.
- Arrow fixtures treated as synthetic on the basis of the `--fixture` export flag and `params.json`; not inspected row by row.

## Decisions taken 2026-11-11

- `docs/mac-recovery.md` and `reports/infrastructure-gaps-2026-11-10.md` removed from the tree in this PR (copies in project files); RFC 0340 reference reworded.
- Author e-mails: owner chose a history rewrite. Not run from the cloud: it force-pushes `main`. Script `rewrite-authors.sh` (mailmap to the GitHub noreply address, also drops `scripts/clippy-bans/target`) runs on a fresh mirror and pushes nothing. Run it once, just before the visibility flip, with no branches in flight; PR commit SHAs change.
- claude.ai link stripping: blocked, see PR thread. `tools/rows` treats claude.ai URLs as the research-index keys, so removing them needs a checker change.
