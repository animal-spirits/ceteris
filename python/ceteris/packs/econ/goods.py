"""The 10 P2 goods: ICIO industry mapping and weight per value from BACI (specs/econ/SPEC.md, Goods)."""

import csv
import io
import zipfile
from collections import defaultdict
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.csv as pacsv

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


def hs_goods_shares(hs_isic3: dict[str, str], bridge: dict[str, list[str]]) -> dict[str, dict[int, float]]:
    """Share of each HS 6-digit line in each good.

    An HS line maps to one ISIC Rev 3 class, which the bridge links to one or more Rev 4 classes. The
    line is split equally across the distinct Rev 4 divisions, then summed by good. A class absent from
    the bridge (Rev 3 splits 0500 into 0501 and 0502 in Rev 3.1) uses the classes sharing its first three
    digits; lines still unlinked are left out.
    """
    shares: dict[str, dict[int, float]] = {}
    for hs, isic3 in hs_isic3.items():
        links = bridge.get(isic3) or [c for k, v in bridge.items() if k[:3] == isic3[:3] for c in v]
        divisions = sorted({c[:2] for c in links})
        by_good: dict[int, float] = defaultdict(float)
        for d in divisions:
            good = good_of_division(d)
            if good is not None:
                by_good[good] += 1 / len(divisions)
        if by_good:
            shares[hs] = dict(by_good)
    return shares


def read_hs_isic3(path: Path) -> dict[str, str]:
    """WITS concordance HS 1988/92 to ISIC Rev 3 (zip holding one CSV): hs6 -> ISIC class."""
    with zipfile.ZipFile(path) as z:
        name = next(n for n in z.namelist() if n.lower().endswith(".csv"))
        rows = csv.reader(io.TextIOWrapper(z.open(name), encoding="utf-8-sig"))
        next(rows)
        return {r[0].zfill(6): r[2].zfill(4) for r in rows if r}


def read_bridge(path: Path) -> dict[str, list[str]]:
    """UNSD correspondence ISIC Rev 3.1 to Rev 4: Rev 3.1 class -> its principal Rev 4 classes.

    Principal links are the ones where the whole Rev 4 class lies inside the Rev 3.1 class
    (partialISIC4 = 0); the rest are exceptions (a few products of the class went elsewhere). When no
    link is complete, all are kept. Division 09 (mining support services) has no physical product, so it
    is dropped wherever another division remains.
    """
    links: dict[str, list[tuple[str, bool]]] = defaultdict(list)
    with path.open(newline="", encoding="utf-8-sig") as f:
        for row in csv.DictReader(f):
            links[row["ISIC31code"]].append((row["ISIC4code"], row["partialISIC4"] == "0"))
    bridge = {}
    for c3, rows in links.items():
        keep = [c for c, complete in rows if complete] or [c for c, _ in rows]
        bridge[c3] = [c for c in keep if c[:2] != "09"] or keep
    return bridge


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


def weights_from_baci(baci_2000: Path, hs_isic: Path, isic_bridge: Path) -> np.ndarray:
    """Tonnes per USD for each good (0 for the local goods 7 to 9), from the BACI 2000 flows."""
    shares = hs_goods_shares(read_hs_isic3(hs_isic), read_bridge(isic_bridge))
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
