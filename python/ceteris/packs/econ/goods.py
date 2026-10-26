"""The 10 P2 goods: ICIO industry mapping and weight per value from BACI (specs/econ/SPEC.md, Goods)."""

import zipfile
from collections import defaultdict
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.csv as pacsv

from . import people

N_GOODS = 10
TRADED = tuple(range(7))
NAMES = (
    "Agriculture",
    "Crude oil and gas",
    "Coal and other mining",
    "Food",
    "Light manufacturing",
    "Chemicals, refined fuels and metals",
    "Machinery, electronics and vehicles",
    "Electricity and utilities",
    "Construction",
    "Services",
)

# Sheet of the OECD BTiGE HS to CPA 2.1 conversion key.
KEY_SHEET = "BTiGE FromHS-ToCPA-ToENDUSE"
# Local adaptations of the key (ADR 0050 amendment). Non-monetary gold (HS 7108) is basic metals, not metal ore
# (the key puts unwrought gold, 710812, in B072), so coal and other mining stays physical bulk. Printed books and
# recorded media (J58, J59_60) are goods made by printing and reproduction (C18), not publishing services.
GOLD = "7108"
ADAPTED_CPA = {"J58": "C18", "J59_60": "C18"}

# Marks an industry shared between goods 1 and 2 (the combined B05_06 of earlier editions); the ICIO
# aggregation splits it by EIA 2000 production values at Pink Sheet prices.
SPLIT = -1

# ICIO industry -> good. The 2025 edition has 50 industries (A01, A02 and A03 separate, B05 and B06
# split, C24A/C24B, C301/C302T309); the older names (A01_02, B05_06, B07_08, C24, C30) stay mapped.
_SERVICES = [
    "G",
    "H49",
    "H50",
    "H51",
    "H52",
    "H53",
    "I",
    "J58T60",
    "J61",
    "J62_63",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
]
ICIO_MAP: dict[str, int] = {
    **dict.fromkeys(["A01", "A02", "A03", "A01_02"], 0),
    **dict.fromkeys(["B06", "B09"], 1),
    **dict.fromkeys(["B05", "B07", "B08", "B07_08"], 2),
    "B05_06": SPLIT,
    "C10T12": 3,
    **dict.fromkeys(["C13T15", "C16", "C17_18", "C31T33"], 4),
    **dict.fromkeys(["C19", "C20", "C21", "C22", "C23", "C24", "C24A", "C24B", "C25"], 5),
    **dict.fromkeys(["C26", "C27", "C28", "C29", "C30", "C301", "C302T309"], 6),
    **dict.fromkeys(["D", "E"], 7),
    "F": 8,
    **dict.fromkeys(_SERVICES, 9),
}

# ISIC Rev 4 division -> ICIO industry; divisions 24 and 30 hold two industries that share a good.
_DIVISION_INDUSTRY = {
    "01": "A01",
    "02": "A02",
    "03": "A03",
    "05": "B05",
    "06": "B06",
    "07": "B07",
    "08": "B08",
    "09": "B09",
    "10": "C10T12",
    "11": "C10T12",
    "12": "C10T12",
    "13": "C13T15",
    "14": "C13T15",
    "15": "C13T15",
    "16": "C16",
    "17": "C17_18",
    "18": "C17_18",
    "19": "C19",
    "20": "C20",
    "21": "C21",
    "22": "C22",
    "23": "C23",
    "24": "C24A",
    "25": "C25",
    "26": "C26",
    "27": "C27",
    "28": "C28",
    "29": "C29",
    "30": "C301",
    "31": "C31T33",
    "32": "C31T33",
    "33": "C31T33",
    "35": "D",
    **dict.fromkeys(["36", "37", "38", "39"], "E"),
    **dict.fromkeys(["41", "42", "43"], "F"),
}


def good_of_division(division: str) -> int | None:
    """Good of a two-digit ISIC Rev 4 division; services (45 and up) are good 9, None for unknown."""
    industry = _DIVISION_INDUSTRY.get(division)
    if industry is not None:
        return ICIO_MAP[industry]
    return 9 if division.isdigit() and 45 <= int(division) <= 99 else None


def good_of_cpa(code: str) -> int | None:
    """Good of a CPA 2.1 code of the OECD key (section letter, division, then any finer split such as C24A or
    C302_309); None for the unallocated `_X`."""
    division = code[1:3]
    return good_of_division(division) if division.isdigit() else None


def hs_goods_shares(hs_cpa: dict[str, str]) -> dict[str, dict[int, float]]:
    """Share of each HS 6-digit line in each good. The key gives every line one activity, so a line lies wholly in
    one good; unallocated lines are left out."""
    return {hs: {good: 1.0} for hs, code in hs_cpa.items() if (good := good_of_cpa(code)) is not None}


def read_hs_cpa(path: Path) -> dict[str, str]:
    """OECD BTiGE conversion key (2025): HS 1988/92 (key version 0) 6-digit line -> CPA 2.1 activity, with the
    local adaptations above (gold to C24B, books and media to C18). CPA 2.1 equals ISIC Rev 4 (NACE Rev 2) at
    division level, and the key splits divisions as ICIO 2025 does (B05 and B06, C24A and C24B, C301 and C302_309)."""
    key = {r["HS-code"]: r["CPA"] for r in people._xlsx_sheet(path, KEY_SHEET) if r["HS-version"] == 0.0}
    return {hs: "C24B" if hs.startswith(GOLD) else ADAPTED_CPA.get(code, code) for hs, code in key.items()}


def baci_by_product(baci_2000: Path) -> tuple[dict[str, float], dict[str, float]]:
    """Tonnes and value (USD) by HS 6-digit line over the 2000 flows with a reported quantity.

    BACI gives value v in thousands of USD and quantity q in metric tons; rows with missing q are
    excluded from both sums so that the ratio is a true weight per value.
    """
    convert = pacsv.ConvertOptions(
        column_types={"k": pa.string(), "v": pa.float64(), "q": pa.float64()},
        include_columns=["k", "v", "q"],
        null_values=["", "NA"],
    )
    tonnes: dict[str, float] = defaultdict(float)
    value: dict[str, float] = defaultdict(float)
    with zipfile.ZipFile(baci_2000) as z:
        member = next(n for n in z.namelist() if n.endswith("Y2000_V202601.csv"))
        with z.open(member) as f:
            reader = pacsv.open_csv(f, convert_options=convert)
            for batch in reader:
                batch = batch.filter(pc.is_valid(batch["q"]))
                table = pa.Table.from_batches([batch]).append_column("k6", pc.utf8_lpad(batch["k"], 6, "0"))
                grouped = table.group_by("k6").aggregate([("q", "sum"), ("v", "sum")])
                for k, q, v in zip(grouped["k6"].to_pylist(), grouped["q_sum"].to_pylist(), grouped["v_sum"].to_pylist()):
                    tonnes[k] += q
                    value[k] += v * 1000
    return tonnes, value


def weights_from_baci(baci_2000: Path, hs_key: Path) -> np.ndarray:
    """Tonnes per USD for each good (0 for the local goods 7 to 9), from the BACI 2000 flows."""
    shares = hs_goods_shares(read_hs_cpa(hs_key))
    tonnes, value = baci_by_product(baci_2000)
    t, v = np.zeros(N_GOODS), np.zeros(N_GOODS)
    for hs, by_good in shares.items():
        for good, share in by_good.items():
            t[good] += share * tonnes.get(hs, 0.0)
            v[good] += share * value.get(hs, 0.0)
    w = np.zeros(N_GOODS)
    traded = list(TRADED)
    w[traded] = t[traded] / v[traded]
    return w
