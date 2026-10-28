"""Canonical JSON of one econ source reader's output, for comparison with `ceteris pack dump-reader`.

    python scripts/parity/readers.py NAME [ARGS]

Keys are sorted, floats are printed by `repr`, sets become sorted lists and tuples lists.
"""

import dataclasses
import json
import re
import sys
from pathlib import Path

import numpy as np
import pyarrow.ipc as ipc

from ceteris.packs import fetch
from ceteris.packs.econ import downscale, gateways, goods, icio, people, tariffs, theta

CACHE = fetch.CACHE
ADMIN0 = CACHE / downscale.ADMIN0
ADMIN1 = CACHE / downscale.ADMIN1
BACI = CACHE / "BACI_HS92_V202601.zip"
PACK = Path(__file__).resolve().parents[2] / "data" / "packs" / "2000"


def plain(x):
    if dataclasses.is_dataclass(x):
        return plain(dataclasses.asdict(x))
    if isinstance(x, dict):
        return {str(k): plain(v) for k, v in x.items()}
    if isinstance(x, set):
        return sorted(plain(v) for v in x)
    if isinstance(x, list | tuple):
        return [plain(v) for v in x]
    if isinstance(x, np.ndarray):
        return plain(x.tolist())
    if isinstance(x, np.bool_ | bool):
        return bool(x)
    if isinstance(x, np.integer):
        return int(x)
    if isinstance(x, np.floating):
        return float(x)
    return x


# Ruled: the Rust xlsx reader decodes OOXML `_xHHHH_` escapes, as the spec says; the
# Python reader keeps them raw. The cell dump decodes them on this side so the two compare equal; no
# reader consumes an escaped cell (only a GEM coal "Mine Name" has one).
OOXML_ESCAPE = re.compile(r"_x([0-9A-Fa-f]{4})_")


def xlsx_rows(name, sheet):
    def decode(v):
        return OOXML_ESCAPE.sub(lambda m: chr(int(m.group(1), 16)), v) if isinstance(v, str) else v

    return [{decode(k): decode(v) for k, v in r.items()} for r in people._xlsx_sheet(CACHE / name, sheet)]


def population_2000():
    return {k: v[2000] for k, v in people.read_wdi(CACHE / "wdi_SP.POP.TOTL.zip").items() if 2000 in v}


def pack_codes():
    return ipc.open_file(PACK / "countries.arrow").read_all().column("code").to_pylist()


def io_fields(by_code):
    keep = ("output", "va", "io", "final_demand_shares", "coal_share")
    return {k: {f: getattr(c, f) for f in keep} for k, c in by_code.items()}


READERS = {
    "xlsx": xlsx_rows,
    "codes": lambda: people.read_codes(ADMIN0),
    "ilostat-participation": lambda: people.read_ilostat(
        CACHE / "ilostat_EAP_2WAP_SEX_AGE_RT_A_2000.csv", "AGE_YTHADULT_Y15-64"
    ),
    "ilostat-unemployment": lambda: people.read_ilostat(
        CACHE / "ilostat_UNE_2EAP_SEX_AGE_RT_A_2000.csv", "AGE_YTHADULT_YGE15"
    ),
    "ilostat-natural": lambda: people.read_ilostat_mean(
        CACHE / "ilostat_UNE_2EAP_SEX_AGE_RT_A_1995-2005.csv", "AGE_YTHADULT_YGE15", *people.U_STAR_YEARS
    ),
    "wic": lambda: people.read_wic(
        CACHE / "BPJresult_AGE_V01.csv", {m: i for i, m in people.read_codes(ADMIN0)[1].items()}
    ),
    "barro-lee": lambda: people.read_barro_lee(CACHE / "BL_v3_MF1564.csv"),
    "wdi": lambda indicator: people.read_wdi(CACHE / f"wdi_{indicator}.zip"),
    "wdi-regions": lambda: people.read_wdi_regions(CACHE / "wdi_SI.POV.GINI.zip"),
    "pwt": lambda: people.read_pwt(CACHE / "pwt110.xlsx"),
    "observed": lambda: people.load_observed(CACHE, ADMIN0),
    "adb-income": lambda: icio.read_adb_income(CACHE / icio.ADB_INCOME),
    "maddison": lambda: icio.read_maddison(CACHE / icio.MADDISON),
    "load-wdi": lambda: icio.load_wdi(CACHE),
    "icio-aggregate": lambda: io_fields(icio.aggregate(CACHE / "1995-2000_SML.zip")),
    "icio-exports": lambda: icio.exports(CACHE / "1995-2000_SML.zip"),
    "hs-cpa": lambda: goods.read_hs_cpa(CACHE / "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx"),
    "baci-by-product": lambda: goods.baci_by_product(BACI),
    "baci-values": lambda: theta.baci_values(BACI),
    "trade-by-country": lambda: gateways.trade_by_country(BACI, people.read_codes(ADMIN0)[0], pack_codes()),
    "rates": lambda: tariffs.read_rates(CACHE),
    "cepii": lambda: tariffs.read_cepii(CACHE / tariffs.GRAVITY),
    "dta": lambda: tariffs.read_dta(CACHE / tariffs.DTA),
    "dose": lambda: downscale.read_dose(CACHE / downscale.DOSE, population_2000()),
    "province-names": lambda: downscale.province_names(ADMIN1),
    "us-states": lambda: downscale.us_states(ADMIN1),
    "gem-coal": lambda: downscale.read_gem_coal(CACHE / "Global Coal Mine Tracker, August 2026.xlsx"),
    "gem-oil-gas": lambda: downscale.read_gem_oil_gas(CACHE / "Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx"),
    "seds": lambda: downscale.read_seds(CACHE / downscale.SEDS),
}


def main() -> None:
    name, *args = sys.argv[1:]
    out = plain(READERS[name](*args))
    sys.stdout.write(json.dumps(out, sort_keys=True, ensure_ascii=False, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
