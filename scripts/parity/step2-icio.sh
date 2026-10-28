#!/usr/bin/env bash
# Step 2 icio parity: the input-output columns of econ_countries and the econ_io table.
set -euo pipefail
exec "$(dirname "$0")/step2-econ.sh" icio
