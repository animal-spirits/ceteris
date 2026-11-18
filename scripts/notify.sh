#!/usr/bin/env bash
# Push a notification when a long run ends, through ntfy.
# Usage: notify.sh STATUS [LOG] [LABEL]      (STATUS: exit code or a word; LOG: log path)
# Env:   CETERIS_NTFY_TOPIC   required; unset or empty makes this a silent no-op
#        CETERIS_NTFY_SERVER  default https://ntfy.sh
# Never fails the caller. Example: trap 'notify.sh $? "$log" free-moment' EXIT
topic=${CETERIS_NTFY_TOPIC:-}
[[ -n $topic ]] || exit 0
status=${1:-?} log=${2:-} label=${3:-ceteris run}
case $status in 0) word=ok ;; *[!0-9]*) word=$status ;; *) word="failed ($status)" ;; esac
msg=$label$'\n'$word
[[ -n $log ]] && msg+=$'\n'$log
curl -fsS --max-time 10 -H "Title: $label $word" -d "$msg" \
  "${CETERIS_NTFY_SERVER:-https://ntfy.sh}/$topic" >/dev/null 2>&1 || true
