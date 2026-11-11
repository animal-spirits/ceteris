#!/usr/bin/env bash
# Repoint git remotes in every clone of the old repo on this machine (Mac, droplets, quant-dev).
# Dry run by default; pass --yes to apply. Idempotent: clones already on the new URL are skipped.
#
#   NEW_OWNER=animal-spirits scripts/repo-move-remotes.sh [--yes] [--root DIR]...
#
# Env: NEW_OWNER (required), NEW_REPO (default ceteris), OLD_OWNER (default atbraz),
#      OLD_REPO (default ceteris).
# --root  directory to search, depth 4 (repeatable). Default: $HOME and /home/*/ (skips caches).
# Keeps the URL scheme (https or ssh). Also reports stored credentials and ssh config that name
# the old owner; it never edits those.
set -uo pipefail

old_owner=${OLD_OWNER:-atbraz} old_repo=${OLD_REPO:-ceteris}
new_owner=${NEW_OWNER:?set NEW_OWNER} new_repo=${NEW_REPO:-ceteris}
yes=0 roots=()
while [ $# -gt 0 ]; do
  case $1 in
    --yes) yes=1 ;;
    --root) shift; roots+=("${1:?--root needs a value}") ;;
    *) sed -n '2,11p' "$0"; exit 2 ;;
  esac
  shift
done
[ ${#roots[@]} -gt 0 ] || roots=("$HOME" /home/*/)

run() { if [ "$yes" -eq 1 ]; then echo "+ $*"; "$@"; else echo "[dry run] $*"; fi; }
lower() { printf '%s' "$1" | tr 'A-Z' 'a-z'; }
old_slug=$(lower "$old_owner/$old_repo")

found=0 changed=0
while IFS= read -r gitdir; do
  clone=$(dirname "$gitdir")
  for remote in $(git -C "$clone" remote 2>/dev/null); do
    url=$(git -C "$clone" remote get-url "$remote" 2>/dev/null) || continue
    # strip scheme/host prefix, optional .git suffix
    path=${url#*github.com[:/]}
    [ "$path" != "$url" ] || continue
    slug=$(lower "${path%.git}")
    [ "$slug" = "$old_slug" ] || continue
    found=$((found + 1))
    new_url=${url%%github.com[:/]*}
    case $url in
      *github.com:*) new_url="${new_url}github.com:" ;;
      *) new_url="${new_url}github.com/" ;;
    esac
    new_url="$new_url$new_owner/$new_repo"
    case $url in *.git) new_url="$new_url.git" ;; esac
    echo "== $clone [$remote] $url -> $new_url"
    run git -C "$clone" remote set-url "$remote" "$new_url"
    changed=$((changed + 1))
  done
done < <(find "${roots[@]}" -maxdepth 4 -name .git \( -type d -o -type f \) \
           -not -path '*/node_modules/*' -not -path '*/Library/*' -not -path '*/.cargo/*' \
           -not -path '*/_work/*' 2>/dev/null)

echo "clones on the old URL: $found"
[ "$found" -gt 0 ] || echo "nothing to change (already moved, or no clone under: ${roots[*]})"

echo "-- places that may still name the old owner (review by hand):"
grep -n -i "$old_owner" "$HOME/.ssh/config" 2>/dev/null | sed 's/^/   ~\/.ssh\/config:/'
git config --global --get-regexp "url\..*$old_owner|credential\..*$old_owner" 2>/dev/null | sed 's/^/   gitconfig: /'
echo "   https credentials are keyed by host (github.com) and survive; a token of the renamed user must be re-issued"
[ "$yes" -eq 1 ] || echo "dry run only; rerun with --yes to apply"
