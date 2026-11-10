# Mac build host: backups and recovery

One Mac is the `ceteris` and `pack-gate` runner host, the only holder of the built pack and the
fetch cache, the quiet-host benchmark machine, and the only place a signed build can come from
(decision: runbook plus backups, rent a cloud Mac only on failure; infra gaps report item 2).
Everything here runs over SSH or Screen Sharing; nothing needs physical access except where marked.
No secret values are written in this file or in the repo. Values marked (est.) are estimates.

## 1. What lives only on the Mac

| Item | Location | Replaceable by | Back up |
|---|---|---|---|
| Built pack | `~/Code/Projects/ceteris/data/packs/2000` (`CETERIS_PACK`) | `pack build` from the fetch cache (hours, needs GEOS 3.15, tippecanoe) | yes |
| Fetch cache incl. manual downloads | cache dir used by `ceteris pack fetch` | re-download; the manual files need provider registration again | yes, critical |
| Logs | `~/ceteris-logs/` | no | yes |
| Runner state | `~/actions-runner/ceteris`, `~/actions-runner/pack-gate` (`.env`, `.runner`, `.credentials*`) | re-register with a new token | `.env` only; credentials are not restorable, re-register |
| Signing and upload credentials | login keychain (Apple notarytool profile, Developer ID cert), steamcmd builder login, Zenodo token | reissue from the provider accounts | keychain export to an encrypted file kept by Antonio |
| Pending licence correspondence | mail, drafts | n/a | covered by mail provider |
| Repo working copy | `~/Code/Projects/ceteris` | `git clone` | no (push everything; unpushed branches are the risk) |

Source code, specs, workflows and scripts are in git and need no backup.

## 2. Backups

Two independent copies, both automatic, both restorable remotely.

1. **Local, Time Machine** to an external disk (hourly). Exclude `target/` directories and
   `~/actions-runner/*/_work`. Protects against disk-level failure only if the disk is not
   in the same failed machine; treat it as the fast restore path, not the only one.
2. **Offsite, restic** to an encrypted cloud bucket (Backblaze B2 or any S3 API; price about
   6 USD per TB per month, verify). Nightly via launchd; scope below. Expect well under 50 GB (est.).

Restic scope, as an include list in `~/.config/ceteris-backup/include.txt`:

```
~/Code/Projects/ceteris/data/packs
<fetch cache dir>
~/ceteris-logs
~/actions-runner/ceteris/.env
~/actions-runner/pack-gate/.env
~/.config/ceteris-backup
~/Documents/ceteris-recovery        # exported keychain items, encrypted, see 2.2
```

One-time setup (SSH as the admin user; B2 account and bucket created by Antonio):

```
brew install restic
mkdir -p ~/.config/ceteris-backup && chmod 700 ~/.config/ceteris-backup
# credentials go in this file, mode 600, never in git; contents: RESTIC_REPOSITORY, RESTIC_PASSWORD_FILE,
# B2_ACCOUNT_ID, B2_ACCOUNT_KEY (or AWS_* for S3)
$EDITOR ~/.config/ceteris-backup/env
set -a; . ~/.config/ceteris-backup/env; set +a
restic init
```

Nightly job `~/Library/LaunchAgents/dev.ceteris.backup.plist` runs this script at 02:00 local
(before the 03:00 US Central pack gate; if the zones overlap, move it, since a running gate does
not write the pack):

```
#!/usr/bin/env bash
set -euo pipefail
set -a; . ~/.config/ceteris-backup/env; set +a
restic backup --files-from ~/.config/ceteris-backup/include.txt --tag nightly
restic forget --keep-daily 7 --keep-weekly 8 --keep-monthly 6 --prune
restic check --read-data-subset=5%
```

The restic password and the B2 key are also stored in Antonio's password manager, so the
repository is recoverable with no access to the Mac. Without them the backup is unreadable.

### 2.1 Verify, monthly

From any machine with the env file contents: `restic snapshots` shows a snapshot younger than
36 hours; `restic restore latest --target /tmp/restore-test --include <pack dir>` then
`ceteris pack check-sources --profile research` or the pack hash check from the pack build
log passes. A backup never restored is not a backup.

### 2.2 Credentials

Do not put keychain contents in restic unencrypted. Export the Developer ID identity as a
password-protected `.p12` (`security export -k login.keychain-db -t identities -f pkcs12`),
keep the passphrase in the password manager, and note the notarytool profile name (reissue
with `xcrun notarytool store-credentials` from a new app-specific password if lost). Steam
builder and Zenodo tokens are reissued, not restored. Record the list of credentials, never the
values, in `docs/release-steam.md`.

## 3. Failure triage (remote)

| Symptom | Check | Action |
|---|---|---|
| Runners show offline in GitHub Settings, Actions, Runners | `ssh mac 'cd ~/actions-runner/ceteris && ./svc.sh status'` | `./svc.sh stop && ./svc.sh start`; same for `pack-gate` |
| Host unreachable | Screen Sharing; router; power | wait 15 min (power cut or update reboot); then go to 4 |
| Disk full | `df -h /`; `du -sh ~/actions-runner/*/_work ~/Code/Projects/ceteris/target` | `scripts/ci-cleanup.sh` style prune of `target/`; never touch `data/packs` |
| Runner job hangs after macOS update | `launchctl list \| grep actions` | reboot, re-run `svc.sh start`; pause automatic macOS updates |
| Pack gate fails with "no pack" | `ls $CETERIS_PACK`; `.env` in the pack-gate runner dir | restore from restic (section 5) |

Keep macOS automatic updates to "download only" and apply them on a day merges are not urgent.

## 4. Stand up a replacement host

Target: Apple silicon, 14 cores and 32 GB or more preferred (the current host is 14 cores, 48 GB).
Quiet-host benchmark timings from a different machine are not comparable; a golden or G1 baseline
taken on a new host must be re-recorded as a reviewed event, not compared (law: a golden move is a
reviewed event).

Options in order of preference: (a) repaired or replacement owned Mac, (b) rented Apple silicon
cloud Mac (about 25 to 100 USD a month, minimum rental may be 24 hours; verify). Rent only on failure.

Steps, all over SSH or VNC once the host exists:

1. Enable Remote Login and Screen Sharing; add Antonio's SSH key; turn on FileVault only if
   remote unlock is available (otherwise leave off for a headless host); set
   `sudo pmset -a sleep 0 disksleep 0` and "start up automatically after power failure".
2. Toolchain:
   ```
   xcode-select --install
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   brew install restic geos tippecanoe cargo-deny cargo-nextest
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # rust-toolchain.toml pins 1.99.1
   ```
   Check `geos-config --version` is 3.15 (build from source if brew is behind).
3. Code: `git clone https://github.com/atbraz/ceteris ~/Code/Projects/ceteris`.
4. Restore data (section 5).
5. Register runners (token: repo Settings, Actions, Runners, New self-hosted runner, a new one
   per runner; tokens expire in an hour). Follow `docs/pack-runner.md` for `pack-gate`; the CI
   runner is the same with `--name mbp-ceteris --labels ceteris` in `~/actions-runner/ceteris`. After
   `./svc.sh install && ./svc.sh start`, remove the dead runners in GitHub so jobs do not queue for them.
   Runner service must run as a non-admin user that has no access to the keychain used for signing
   (infra report item 8).
6. Install the backup job (section 2) and run it once.
7. Verify: `cargo fmt --all --check`, `cargo test --workspace`, `cargo run -p ceteris-rows -- check`,
   then dispatch `pack-gate` from the Actions tab. First gate run is a cold build (est. tens of minutes).
8. Signing: re-import the `.p12`, `xcrun notarytool store-credentials`, run `scripts/package_app.sh`
   and a notarisation dry run from the admin account (#228).
9. Update this file if any path or version changed.

While the host is down, droplet runners (do-ceteris-1/2) still run PR checks, so merges
continue; nightly pack gate, flatness gate, signing and releases wait. Do not move pack-gated
jobs to the droplet or any host without the pack.

## 5. Restore data

```
set -a; . ~/.config/ceteris-backup/env; set +a   # recreate the env file from the password manager first
restic snapshots
restic restore latest --target / --include ~/Code/Projects/ceteris/data/packs \
  --include ~/ceteris-logs --include <fetch cache dir>
cargo run --release --bin ceteris -- pack fetch   # verifies sha256 and size against fetch.toml
```

If the pack is lost and the fetch cache survived: `pack build` (owner-side; needs GEOS 3.15 and
tippecanoe), then re-bless any pack goldens as a reviewed event. If both are lost, the automatic
downloads re-fetch; the manual ones need provider registration again (see `fetch.toml`).

## 6. Open points for Antonio

- Create the B2 (or S3) bucket and the external disk; supply the repository URL and keys to the
  Mac and the password manager. An agent cannot do this.
- Whether the Mac must have its own UPS or a spare host on standby (not recommended before 1.0).
