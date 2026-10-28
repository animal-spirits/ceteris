#!/usr/bin/env bash
# Step 2 people parity: the people columns of econ_countries, econ_provinces and econ_pops.
set -euo pipefail
exec "$(dirname "$0")/step2-econ.sh" people
