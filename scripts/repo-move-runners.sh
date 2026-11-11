#!/usr/bin/env bash
# Re-register this machine's GitHub Actions runners after the repo moved owner (and optionally
# to an org-level runner group). Works on the Mac (launchd) and the droplets (systemd).
# Dry run by default; pass --yes to apply. Idempotent: runners already on the target URL are skipped.
#
#   scripts/repo-move-runners.sh status
#   NEW_OWNER=animal-spirits scripts/repo-move-runners.sh reconfigure [--yes]
#       [--scope repo|org] [--group NAME] [--root DIR]...
#
# Env: NEW_OWNER (required for reconfigure), NEW_REPO (default ceteris), OLD_OWNER (default atbraz),
#      OLD_REPO (default ceteris), RUNNER_TOKEN (registration token for the target scope, valid 1 h;
#      if unset, fetched with `gh api`, which needs repo admin, or org admin for --scope org).
#
# Per runner: stop and uninstall the service, deregister the old registration (best effort, GitHub
# may already have dropped it on transfer), delete local credentials (.env is kept: it holds PATH,
# CARGO_BUILD_JOBS, CETERIS_PACK), register at the new URL with the same name and labels, reinstall
# the service, restore systemd limits drop-ins, start. Service names change with the owner
# (actions.runner.<owner>-<repo>.<name>); the drop-in is copied to the new unit.
# Labels come from the runner name: *-pack -> pack-gate, do-ceteris-* -> ceteris-linux,
# mbp-ceteris -> ceteris. Other names are skipped.
# Run as root on the droplets (each runner is driven as the directory's owner), as the runner user on the Mac.
# --scope org registers at https://github.com/NEW_OWNER in --group (default "Default"; restrict the
# group to the repo and enable "allow public repositories" in the GitHub UI first).
set -uo pipefail

cmd=${1:-}; shift || true
old_owner=${OLD_OWNER:-atbraz} old_repo=${OLD_REPO:-ceteris}
new_repo=${NEW_REPO:-ceteris}
yes=0 scope=repo group= roots=()
while [ $# -gt 0 ]; do
  case $1 in
    --yes) yes=1 ;;
    --scope) shift; scope=${1:?}; [[ $scope == repo || $scope == org ]] || { echo "scope: repo|org" >&2; exit 2; } ;;
    --group) shift; group=${1:?} ;;
    --root) shift; roots+=("${1:?--root needs a value}") ;;
    *) sed -n '2,6p' "$0"; exit 2 ;;
  esac
  shift
done
case $cmd in status|reconfigure) ;; *) sed -n '2,6p' "$0"; exit 2 ;; esac

os=$(uname -s)
[ ${#roots[@]} -gt 0 ] || roots=("$HOME"/actions-runner* /home/ghrunner-*/runner-*)

mask() { local s="$*"; [ -n "${RUNNER_TOKEN:-}" ] && s=${s//"$RUNNER_TOKEN"/<token>}; printf '%s' "$s"; }
run() { if [ "$yes" -eq 1 ]; then echo "+ $(mask "$@")"; "$@"; else echo "[dry run] $(mask "$@")"; fi; }
field() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8-sig")).get(sys.argv[2], ""))' "$1" "$2"; }

# every directory holding a .runner file, one level or two below each root
dirs=()
while IFS= read -r f; do dirs+=("$(dirname "$f")"); done < <(
  for r in "${roots[@]}"; do [ -d "$r" ] && find "$r" -maxdepth 3 -name .runner -type f 2>/dev/null; done | sort -u)
[ ${#dirs[@]} -gt 0 ] || { echo "no runner directories found under: ${roots[*]}"; exit 0; }

if [ "$cmd" = status ]; then
  for d in "${dirs[@]}"; do
    unit= ; [ -f "$d/.service" ] && unit=$(cat "$d/.service")
    state=n/a; [ "$os" = Linux ] && [ -n "$unit" ] && state=$(systemctl is-active "$unit" 2>/dev/null)
    echo "$(field "$d/.runner" agentName)  url=$(field "$d/.runner" gitHubUrl)  id=$(field "$d/.runner" agentId)  dir=$d  service=${unit:-none}  state=$state"
  done
  exit 0
fi

new_owner=${NEW_OWNER:?set NEW_OWNER}
if [ "$scope" = org ]; then
  url="https://github.com/$new_owner"; tok_path="orgs/$new_owner/actions/runners/registration-token"
else
  url="https://github.com/$new_owner/$new_repo"; tok_path="repos/$new_owner/$new_repo/actions/runners/registration-token"
fi
if [ "$yes" -eq 1 ] && [ -z "${RUNNER_TOKEN:-}" ]; then
  RUNNER_TOKEN=$(gh api -X POST "$tok_path" --jq .token) || { echo "no RUNNER_TOKEN and gh failed" >&2; exit 1; }
fi
: "${RUNNER_TOKEN:=<token>}"

labels_for() {
  case $1 in
    *-pack) echo pack-gate ;;
    do-ceteris-*) echo ceteris-linux ;;
    mbp-ceteris) echo ceteris ;;
    *) return 1 ;;
  esac
}

for d in "${dirs[@]}"; do
  name=$(field "$d/.runner" agentName); id=$(field "$d/.runner" agentId); cur=$(field "$d/.runner" gitHubUrl)
  echo "== $name  $cur  dir=$d"
  if [ "$cur" = "$url" ]; then echo "   already on $url, skipped"; continue; fi
  labels=$(labels_for "$name") || { echo "   unknown runner name, skipped"; continue; }
  case $cur in "https://github.com/$old_owner/$old_repo"|"https://github.com/$old_owner/$old_repo/") ;; *)
    echo "   registered elsewhere ($cur), skipped"; continue ;; esac

  owner=$(stat -c %U "$d" 2>/dev/null || stat -f %Su "$d")
  as_user() {
    if [ "$(id -u)" -eq 0 ] && [ "$owner" != root ]; then
      sudo -u "$owner" bash -c 'cd "$1" && shift && "$@"' _ "$d" "$@"
    else (cd "$d" && "$@"); fi
  }

  unit=; [ -f "$d/.service" ] && unit=$(cat "$d/.service")
  dropin=
  if [ "$os" = Linux ] && [ -n "$unit" ] && [ -d "/etc/systemd/system/$unit.d" ]; then
    dropin=$(mktemp -d); cp -a "/etc/systemd/system/$unit.d/." "$dropin/"
  fi

  # 1. stop and remove the old service
  if [ -x "$d/svc.sh" ]; then
    if [ "$os" = Linux ]; then run bash -c "cd '$d' && ./svc.sh stop; ./svc.sh uninstall; true"
    else run as_user bash -c './svc.sh stop; ./svc.sh uninstall; true'; fi
  fi
  [ "$os" = Linux ] && [ -n "$unit" ] && run rm -rf "/etc/systemd/system/$unit.d"
  # 2. deregister the old runner (best effort) and drop local credentials
  [ -n "$id" ] && { run gh api -X DELETE "repos/$new_owner/$new_repo/actions/runners/$id" || echo "   not deregistered by id (already gone, or other scope)"; }
  run as_user rm -f .runner .credentials .credentials_rsaparams
  # 3. register at the target
  cfg=(./config.sh --unattended --replace --url "$url" --token "$RUNNER_TOKEN" --name "$name" --labels "$labels" --work _work)
  [ "$scope" = org ] && [ -n "$group" ] && cfg+=(--runnergroup "$group")
  run as_user "${cfg[@]}"
  # 4. reinstall and start the service
  if [ "$os" = Linux ]; then
    run bash -c "cd '$d' && ./svc.sh install '$owner' && { ! command -v getenforce >/dev/null || [ \"\$(getenforce)\" = Disabled ] || chcon -t bin_t runsvc.sh; }"
    if [ "$yes" -eq 1 ]; then
      newunit=$(cat "$d/.service")
      if [ -n "$dropin" ]; then mkdir -p "/etc/systemd/system/$newunit.d"; cp -a "$dropin/." "/etc/systemd/system/$newunit.d/"; rm -rf "$dropin"; fi
      systemctl daemon-reload
    else echo "[dry run] copy systemd drop-in $unit.d to the new unit, daemon-reload"; fi
    run bash -c "cd '$d' && ./svc.sh start"
  else
    run as_user bash -c './svc.sh install && ./svc.sh start'
  fi
  [ "$yes" -eq 1 ] && sleep 3
done
[ "$yes" -eq 1 ] || echo "dry run only; rerun with --yes to apply"
echo "next: scripts/repo-move-verify.sh"
