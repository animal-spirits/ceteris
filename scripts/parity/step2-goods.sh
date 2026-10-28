#!/usr/bin/env bash
# Step 2 goods parity: the econ_goods table, then the theta functions (no table) as readers.
set -euo pipefail
here="$(dirname "$0")"
"$here/step2-econ.sh" goods
"$here/step2-readers.sh" theta theta-bands theta-fixture
