#!/usr/bin/env bash
# Best-effort end-of-job cleanup for self-hosted runners. Called from an `if: always()` step.
# Prunes only disposable build output; the warm target dir, cargo registry and toolchains stay.
# Never touches anything outside this job's CARGO_TARGET_DIR, workspace and runner temp.
set -u

t="${CARGO_TARGET_DIR:-}"
if [ -n "$t" ] && t="$(cd "$t" 2>/dev/null && pwd -P)" && [ "$(basename "$(dirname "$t")")" = target ]; then
  # Incremental caches are the largest and least reusable part.
  find "$t" -type d -name incremental -prune -exec rm -rf {} + 2>/dev/null
  # Stale build products nobody has rebuilt in a week.
  find "$t" -type f -mtime +7 \( -path '*/deps/*' -o -path '*/build/*' \) -delete 2>/dev/null
  find "$t" -mindepth 1 -type d -empty -delete 2>/dev/null
  if command -v cargo-sweep >/dev/null 2>&1; then
    # cargo-sweep reads CARGO_TARGET_DIR; run it from the checkout so cargo metadata resolves.
    (cd "${GITHUB_WORKSPACE:-.}" && cargo sweep --time 14) || true
  fi
else
  echo "ci-cleanup: CARGO_TARGET_DIR unset or not under a target/ dir; skipping target prune"
fi

# Per-job leftovers.
[ -n "${GITHUB_WORKSPACE:-}" ] && rm -rf "${GITHUB_WORKSPACE:?}/pack-gate-logs"
[ -n "${RUNNER_TEMP:-}" ] && [ -d "$RUNNER_TEMP" ] && find "$RUNNER_TEMP" -mindepth 1 -maxdepth 1 -exec rm -rf {} + 2>/dev/null
exit 0
