#!/usr/bin/env bash
# Step 2 goods parity: the econ_goods table (the theta functions have no table; their tests run
# with the module's).
set -euo pipefail
exec "$(dirname "$0")/step2-econ.sh" goods
