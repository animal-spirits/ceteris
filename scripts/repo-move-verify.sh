#!/usr/bin/env bash
# Read-only checks after the repo move. Exit status is the number of failed checks.
#
#   NEW_OWNER=animal-spirits scripts/repo-move-verify.sh [--root DIR]...
#
# Env: NEW_OWNER (required), NEW_REPO (default ceteris), OLD_OWNER (default atbraz),
#      OLD_REPO (default ceteris), EXPECT_RUNNERS (default "mbp-ceteris mbp-ceteris-pack do-ceteris-1 do-ceteris-2").
# Needs `gh` logged in with repo admin (org admin for the runner-group checks, skipped if denied).
# Checks: repo visibility, expected runners online with the right labels, fork PR approval policy,
# read-only default workflow token, org runner group scoping, no clone or tracked file under --root
# still naming the old slug. A WARN needs a human look; a FAIL counts toward the exit status.
set -uo pipefail

old_owner=${OLD_OWNER:-atbraz} old_repo=${OLD_REPO:-ceteris}
new_owner=${NEW_OWNER:?set NEW_OWNER} new_repo=${NEW_REPO:-ceteris}
expect=${EXPECT_RUNNERS:-mbp-ceteris mbp-ceteris-pack do-ceteris-1 do-ceteris-2}
roots=()
while [ $# -gt 0 ]; do
  case $1 in --root) shift; roots+=("${1:?}") ;; *) sed -n '2,10p' "$0"; exit 2 ;; esac
  shift
done
[ ${#roots[@]} -gt 0 ] || roots=("$HOME")
slug=$new_owner/$new_repo fails=0
ok() { echo "ok    $*"; }
warn() { echo "WARN  $*"; }
fail() { echo "FAIL  $*"; fails=$((fails + 1)); }

vis=$(gh api "repos/$slug" --jq '.visibility + " " + .default_branch' 2>/dev/null) \
  && ok "repo $slug reachable ($vis)" || { fail "repo $slug not reachable with this gh login"; exit 1; }

# runners: repo-level plus org-level (inherited runners appear in the repo listing too)
rows=$(gh api "repos/$slug/actions/runners" --paginate --jq '.runners[]|[.name,.status,(.labels|map(.name)|join(","))]|@tsv' 2>/dev/null)
for n in $expect; do
  line=$(printf '%s\n' "$rows" | awk -F'\t' -v n="$n" '$1 == n')
  [ -n "$line" ] || { fail "runner $n not registered for $slug"; continue; }
  status=$(printf '%s' "$line" | cut -f2); lab=$(printf '%s' "$line" | cut -f3)
  case $n in *-pack) want=pack-gate ;; do-ceteris-*) want=ceteris-linux ;; *) want=ceteris ;; esac
  [ "$status" = online ] && ok "runner $n online" || fail "runner $n is $status"
  [[ ",$lab," == *",$want,"* ]] || fail "runner $n lacks label $want (has $lab)"
done
extra=$(printf '%s\n' "$rows" | awk -F'\t' 'NF' | cut -f1 | grep -vxF -f <(printf '%s\n' $expect) || true)
[ -z "$extra" ] || warn "unexpected runners: $(echo $extra)"

# public-repo safety
pol=$(gh api "repos/$slug/actions/permissions/fork-pr-contributor-approval" --jq .approval_policy 2>/dev/null)
case $pol in
  all_external_contributors) ok "fork PRs need approval for all outside contributors" ;;
  "") warn "could not read fork PR approval policy; check Settings > Actions > General by hand" ;;
  *) fail "fork PR approval policy is '$pol', want all_external_contributors" ;;
esac
perm=$(gh api "repos/$slug/actions/permissions/workflow" --jq .default_workflow_permissions 2>/dev/null)
[ "$perm" = read ] && ok "default workflow token is read-only" || warn "default workflow permissions: ${perm:-unreadable}"
groups=$(gh api "orgs/$new_owner/actions/runner-groups" --jq '.runner_groups[]|[.name,.visibility,.allows_public_repositories]|@tsv' 2>/dev/null) \
  && { echo "$groups" | while IFS=$'\t' read -r g v p; do echo "info  runner group '$g' visibility=$v allows_public=$p"; done
       echo "$groups" | awk -F'\t' '$2 == "all" && $3 == "true" { bad = 1 } END { exit bad }' \
         || warn "a runner group is open to all repos including public ones; restrict it to $new_repo"; } \
  || echo "info  org runner groups not readable (repo-level runners, or not an org admin)"

# local state under --root
for r in "${roots[@]}"; do
  while IFS= read -r g; do
    c=$(dirname "$g")
    git -C "$c" remote -v 2>/dev/null | grep -qi "$old_owner/$old_repo" && fail "clone $c still points at $old_owner/$old_repo"
  done < <(find "$r" -maxdepth 4 -name .git -not -path '*/Library/*' -not -path '*/node_modules/*' -not -path '*/_work/*' 2>/dev/null)
  if [ -d "$r/.git" ]; then
    hits=$(git -C "$r" grep -n -i "$old_owner/$old_repo" -- . ':!rfcs' ':!reports' ':!docs/research' ':!docs/repo-move-runbook.md' ':!scripts/repo-move-*.sh' 2>/dev/null | head -5)
    [ -z "$hits" ] || warn "tracked files still name the old slug in $r (first 5):"$'\n'"$hits"
  fi
done

[ "$fails" -eq 0 ] && echo "all checks passed" || echo "$fails check(s) failed"
exit "$fails"
