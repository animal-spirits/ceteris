#!/bin/sh
# PostToolUse: format the Rust file just edited.
f=$(jq -r '.tool_input.file_path // empty' 2>/dev/null)
case "$f" in
  *.rs) rustfmt --edition 2024 "$f" 2>/dev/null ;;
esac
exit 0
