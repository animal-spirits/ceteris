# Repo move runbook: machines side

Moves `atbraz/ceteris` to the org `animal-spirits` (repo name `ceteris`), then makes it public.
Covers the Mac, the droplets and quant-dev. Everything is run over Remote Control or ssh; nobody
needs to touch the Mac. Steps marked **[Antonio]** are GitHub web UI only and cannot be scripted.
Secrets: none in this file or the scripts; tokens come from the environment. The repo-side audit
and history cleanup are a separate thread; this runbook assumes it is done before step 9.

Variables used throughout:

```bash
export NEW_OWNER=animal-spirits NEW_REPO=ceteris    # OLD_OWNER=atbraz OLD_REPO=ceteris are the defaults
```

The GitHub login rename (atbraz to something else) does not change `NEW_OWNER`; the org name is
independent. It only affects `gh` logins, CODEOWNERS and the author link on commits (step 11).

## Machines

| Machine | Runners | Service manager | Clone | Notes |
|---|---|---|---|---|
| Mac | `mbp-ceteris` (`ceteris`), `mbp-ceteris-pack` (`pack-gate`) | launchd, `~/actions-runner/*` | `~/Code/Projects/ceteris` | holds pack, signing keychain, Steam, restic. Must stay awake (`caffeinate -imsu`, never `-d`). |
| do-ceteris-1/2 | `do-ceteris-1/2` (`ceteris-linux`) | systemd, `/home/ghrunner-ceteris/runner-N` | none needed | limits drop-ins in `/etc/systemd/system/<unit>.d/` are preserved by the script. yan-lang runners on the same droplet belong to another repo and are not touched. |
| quant-dev | none | n/a | wherever cloned | remotes only. |

Scripts (all dry run by default, `--yes` applies, idempotent, bash 3.2 safe for macOS):

| Script | Does |
|---|---|
| `scripts/repo-move-remotes.sh` | `git remote set-url` in every clone under `--root` (default `$HOME`, `/home/*/`), keeps https or ssh form, lists ssh config and gitconfig lines naming the old owner. |
| `scripts/repo-move-runners.sh status` / `reconfigure` | lists runners; re-registers each at the new URL with the same name and labels, reinstalls the service (the name embeds the owner), copies the systemd drop-in, keeps `.env` (PATH, `CARGO_BUILD_JOBS`, `CETERIS_PACK`). `--scope org --group NAME` registers at org level instead. |
| `scripts/repo-move-verify.sh` | read-only: runners online with right labels, fork PR approval policy, token permissions, runner groups, leftover old slug in clones and tracked files. Exit status = failed checks. |

Getting the scripts onto a machine: the Mac has a clone (`git pull` before the move, scripts are
in the checkout). Droplets: `scp scripts/repo-move-runners.sh scripts/repo-move-remotes.sh root@DROPLET:/root/`.
Registration token, per machine (valid 1 hour), on any host with `gh` logged in as an admin:

```bash
gh api -X POST repos/$NEW_OWNER/$NEW_REPO/actions/runners/registration-token --jq .token
# pass it as RUNNER_TOKEN in the environment of the command, never as an argument or in a file
```

## Cutover sequence

**0. Preflight (machines, day before).**
- Mac: `caffeinate -imsu` detached; `scripts/repo-move-runners.sh status > ~/ceteris-logs/runners-before.txt` on every machine (rollback record).
- restic: `restic snapshots` younger than 36 h (`docs/mac-recovery.md`); take a manual snapshot. The include list is path based and has no repo name, so it needs no change.
- Unpushed work: `git -C <clone> status -sb` and `git branch -vv` in every clone; push or note it.
- Queue: no running jobs (`gh run list --status in_progress`), no pack-gate run within 3 h of the move.

**1. [Antonio] Pre-public GitHub hardening, still private.** Settings > Actions > General:
fork PR workflows "Require approval for all outside collaborators" (strictest option), workflow
token default "Read repository contents", "Allow GitHub Actions to create PRs" off.

**2. [Antonio] Create the org `animal-spirits` (free plan), add yourself as owner.** Optional,
only if using org-level runners: Org Settings > Actions > Runner groups > new group `ceteris`,
repository access "Selected repositories", and enable "Allow public repositories" on that group
only. Recommendation: stay with repo-level runners (4 runners, one repo); skip the group.

**3. [Antonio] Transfer.** Repo Settings > Danger zone > Transfer ownership to `animal-spirits`,
keep the name `ceteris`. Do not create a new `atbraz/ceteris` afterwards; that would break the
redirect. Issues, PRs, secrets, variables (`CI_RUNNER_PR`), branch protection and deploy keys
move with the repo. Self-hosted runners registered at repo level may be dropped by GitHub on
transfer; the script handles both cases. Reconnect GitHub in the project settings and re-install
the Claude GitHub App on the org for the repo.

**4. Machine: remotes.** On every machine (dry run first):

```bash
scripts/repo-move-remotes.sh                 # review
scripts/repo-move-remotes.sh --yes
git -C ~/Code/Projects/ceteris fetch origin  # proves the new URL works
```

Old URLs redirect, so a missed clone keeps working until the old name is reused; fix it anyway.

**5. Machine: runners.** Order matters: droplets first (they carry PR checks), then Mac `ceteris`,
then Mac `pack-gate` last. Between machines check `status`.

```bash
# droplet, as root; token fetched elsewhere, passed through the environment
RUNNER_TOKEN=... scripts/repo-move-runners.sh reconfigure           # dry run: read it
RUNNER_TOKEN=... scripts/repo-move-runners.sh reconfigure --yes
# Mac, as the user that owns ~/actions-runner
RUNNER_TOKEN=... scripts/repo-move-runners.sh reconfigure --yes
# org level instead: add  --scope org --group ceteris  and fetch the token from orgs/$NEW_OWNER/...
```

Labels set by runner name (`*-pack` to `pack-gate`, `do-ceteris-*` to `ceteris-linux`,
`mbp-ceteris` to `ceteris`). New service names: `actions.runner.animal-spirits-ceteris.<runner>`
(Linux unit and macOS launchd label). Old units are uninstalled by the script; confirm with
`systemctl list-units 'actions.runner.*'` and `launchctl list | grep actions.runner`.
Workflow `runs-on` labels are unchanged, so no `.github/` edit is needed for the move.

**6. Verify.** `scripts/repo-move-verify.sh --root ~/Code` (any machine with an admin `gh` login).
Then dispatch `pack-gate` once from the Actions tab and open a throwaway PR from a `claude/*`
branch to see `rows` and `rust` run on the droplets. Every check must pass before step 9.

**7. Machine: references on disk (read-only review, hand edits).**
- `~/.ssh/config`, `core.sshCommand`, deploy keys: deploy keys are per repo and moved with it; an ssh remote with a repo-specific `Host` alias needs the key re-checked (`ssh -T git@github.com`).
- Mac launchd plists in `~/Library/LaunchAgents/actions.runner.atbraz-*` must be gone (the script removes them through `svc.sh uninstall`; delete strays by hand).
- Steam: `release/steam/*.vdf` and the Steamworks account carry no repo name; no change. Zenodo records created by `scripts/publish_pack_zenodo.sh` may carry the GitHub URL in their metadata; set the new URL at the next publish.
- Signing, notarytool profile, keychain, restic repository and passwords: independent of the repo name; no change. The runner user still must not read the signing keychain (`docs/mac-recovery.md`, item 8).
- Cached credentials: https tokens are keyed by host and survive the move; `gh auth status` per machine.

**8. Repo-side follow-up (separate thread, not this runbook).** Replace `atbraz/ceteris` in
`app/src/halt.rs` (URL and its test), `docs/mac-recovery.md`, `docs/pack-runner.md`,
`docs/development-laws.md`, `CLAUDE.md`, and the Claude memory; CODEOWNERS is
**[Antonio]** only (`.github/`). `git grep -n 'atbraz'` lists the rest.

**9. [Antonio] Go public** only after steps 1 to 8 and the audit thread. Settings > Danger zone >
Change visibility. Irreversible in effect (forks and clones persist). Immediately re-run
`scripts/repo-move-verify.sh`.

**10. Post-public hygiene.** Re-run verify weekly for a month. Check the runner groups and the
runner list after any collaborator change.

**11. [Antonio] Rename the GitHub login** (any time after step 3). Then on each machine
`gh auth refresh` (or `gh auth login`), re-check `git config user.name/email` (the noreply email
is keyed by id, not name), and update CODEOWNERS `@atbraz` entries in a hand-made PR.

## Public-repo runner safety

Anyone with write access runs code on the runners; fork PRs must never reach them.

1. Fork PR approval: all outside contributors (step 1). The workflows also guard with `head.repo.full_name == github.repository`; keep it. `pull_request_target` is not used and must not be added.
2. `pack-gate` runs only on `schedule` and `workflow_dispatch`, never on PRs. Before going public, check that the `pack-gate-logs` artifact holds no licensed data: artifacts of a public repo are downloadable by anyone signed in. Set a short retention.
3. Move PR checks to the droplets (`CI_RUNNER_PR` variable, already supported by `ci.yml` and `rows.yml`), so the Mac only sees pushes to `main`, `pack-gate` and `slow`. The Mac runner user is non-admin, has no keychain access and no sudo.
4. Droplet runners: no sudo, no docker, no secrets, memory and task limits via the preserved drop-in. Ephemeral runners (`config.sh --ephemeral` with a JIT config per job) are stronger but lose the cargo cache; adopt only if a fork-PR path is ever opened.
5. Org runner group (if used): selected repositories only, "allow public repositories" on for that group alone, never on `Default`.
6. Keep collaborators few; a collaborator PR to a branch runs on the runners.

## Rollback

- Before step 3: nothing has changed.
- After step 3, before step 9: transfer back (org Settings, or the repo's Danger zone), then swap the variables and rerun the same scripts:
  `OLD_OWNER=animal-spirits NEW_OWNER=atbraz scripts/repo-move-remotes.sh --yes` and
  `OLD_OWNER=animal-spirits NEW_OWNER=atbraz RUNNER_TOKEN=... scripts/repo-move-runners.sh reconfigure --yes`.
  Records: `~/ceteris-logs/runners-before.txt`.
- A runner that fails to register: the machine keeps working as a clone; PR checks fall back by unsetting `CI_RUNNER_PR` (`gh variable delete CI_RUNNER_PR --repo $NEW_OWNER/$NEW_REPO`) so the Mac serves them, and the other way round while the Mac is being fixed. Droplets: `journalctl -u <unit> -n 50`; a 203/EXEC after reinstall is SELinux: `restorecon -Rv /home/ghrunner-*/runner-*` and rerun `svc.sh install`.
- After step 9 the repo can be made private again but copies persist; treat as one-way.

## Verification checklist

- [ ] `repo-move-verify.sh` exits 0 on a machine with admin `gh`
- [ ] 4 runners online, labels `ceteris`, `pack-gate`, `ceteris-linux` x2; no stale runner under the old registration
- [ ] `git fetch` works on the Mac and quant-dev; `git remote -v` shows the new owner everywhere
- [ ] `pack-gate` dispatch green; a `claude/*` PR runs rows and rust on droplets
- [ ] `systemctl list-units 'actions.runner.*'` and `launchctl list | grep actions` show only new-owner units
- [ ] fork PR approval = all outside contributors; default token read-only; no runner group open to public repos
- [ ] restic snapshot after the move succeeds; recovery doc updated (step 8)
- [ ] `gh variable list` shows `CI_RUNNER_PR`; project GitHub link and Claude App re-granted
