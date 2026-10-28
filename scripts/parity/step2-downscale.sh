#!/usr/bin/env bash
# Step 2 downscale parity: the VA columns of econ_provinces, then the H3 cell and province of
# every GEM asset and every downscaling proxy, as readers.
set -euo pipefail
here="$(dirname "$0")"
"$here/step2-econ.sh" downscale
"$here/step2-readers.sh" point-owners:coal point-owners:oil-gas proxies
