#!/usr/bin/env bash
# Archive the redistributable source files of a release on Zenodo (ADR 0215), one record per release.
# Usage: scripts/publish_pack_zenodo.sh [--execute] [--publish] [--sandbox] --version V DIR
#   DIR       files to archive, already filtered to commercial_use = true / redistributable sources
#   (default) dry run: no network, no token; prints the plan, sizes and metadata
#   --execute create a draft deposition and upload DIR; needs ZENODO_TOKEN
#   --publish also publish the draft (irreversible: Zenodo records cannot be deleted); implies --execute
#   --sandbox use sandbox.zenodo.org
# Token comes only from the environment. Limits (verify): 50 GB and 100 files per record.
set -euo pipefail

execute=0 publish=0 host=zenodo.org version="" dir=""
while [ $# -gt 0 ]; do
  case $1 in
    --execute) execute=1 ;;
    --publish) publish=1 execute=1 ;;
    --sandbox) host=sandbox.zenodo.org ;;
    --version) version=${2:?--version needs a value}; shift ;;
    -*) echo "unknown option $1" >&2; exit 2 ;;
    *) dir=$1 ;;
  esac
  shift
done
[ -n "$version" ] && [ -d "$dir" ] || { sed -n 2,9p "$0" >&2; exit 2; }

api=https://$host/api
manifest=$(mktemp)
trap 'rm -f "$manifest"' EXIT
(cd "$dir" && find . -type f ! -name SHA256SUMS | sort | xargs -d '\n' sha256sum) > "$manifest"
count=$(wc -l < "$manifest" | tr -d ' ')
bytes=$(cd "$dir" && find . -type f ! -name SHA256SUMS -exec wc -c {} + | awk '$2 != "total" { s += $1 } END { print s + 0 }')

metadata=$(jq -n --arg v "$version" '{metadata: {
  title: ("Ceteris Paribus source archive " + $v),
  upload_type: "dataset",
  description: ("Redistributable pinned source files for Ceteris Paribus release " + $v + ". SHA256SUMS lists every file; fetch.toml mirror URLs point here."),
  version: $v,
  creators: [{name: "Ceteris Paribus maintainers"}],
  access_right: "open"
}}')

echo "target: $api"
echo "files:  $count ($bytes bytes)"
[ "$count" -le 100 ] || echo "warning: more than 100 files; archive into tarballs first" >&2
[ "$bytes" -le 53687091200 ] || echo "warning: over 50 GB; split into several records" >&2
echo "metadata: $metadata"

if [ "$execute" -eq 0 ]; then
  echo "dry run: nothing sent (pass --execute to create a draft)"
  exit 0
fi

: "${ZENODO_TOKEN:?set ZENODO_TOKEN in the environment}"
auth=(-H "Authorization: Bearer $ZENODO_TOKEN")
cp "$manifest" "$dir/SHA256SUMS"

dep=$(curl -fsS "${auth[@]}" -H 'Content-Type: application/json' -d '{}' "$api/deposit/depositions")
id=$(jq -r .id <<< "$dep")
bucket=$(jq -r .links.bucket <<< "$dep")
echo "draft deposition $id"

(cd "$dir" && find . -type f | sort) | while IFS= read -r f; do
  curl -fsS "${auth[@]}" --upload-file "$dir/$f" "$bucket/$(basename "$f")" > /dev/null
  echo "uploaded $f"
done
curl -fsS "${auth[@]}" -X PUT -H 'Content-Type: application/json' -d "$metadata" "$api/deposit/depositions/$id" > /dev/null

if [ "$publish" -eq 1 ]; then
  doi=$(curl -fsS "${auth[@]}" -X POST "$api/deposit/depositions/$id/actions/publish" | jq -r .doi)
  echo "published: https://doi.org/$doi"
else
  echo "draft left unpublished: https://$host/deposit/$id"
fi
