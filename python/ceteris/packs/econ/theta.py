"""Trade elasticity theta of the P2 goods from Caliendo and Parro (2015) sector estimates, and its
classes (ADRs 0018 and 0023)."""

import itertools
import zipfile
from collections import defaultdict
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.csv as pacsv

from . import goods

# Caliendo and Parro (2015), Table 1, 99% sample (their benchmark, column 5): sector -> (ISIC Rev 3
# divisions of Table A.1, theta). Review of Economic Studies 82(1), Advance Access version of
# 14 November 2014, p. 18 (Table 1) and p. 42 (Table A.1). Transcribed in ADR 0023.
CALIENDO_PARRO: dict[str, tuple[tuple[str, ...], float]] = {
    "Agriculture": (("01", "02", "05"), 9.11),
    "Mining": (("10", "11", "12", "13", "14"), 13.53),
    "Food": (("15", "16"), 2.62),
    "Textile": (("17", "18", "19"), 8.10),
    "Wood": (("20",), 11.50),
    "Paper": (("21", "22"), 16.52),
    "Petroleum": (("23",), 64.85),
    "Chemicals": (("24",), 3.13),
    "Plastic": (("25",), 1.67),
    "Minerals": (("26",), 2.41),
    "Basic metals": (("27",), 3.28),
    "Metal products": (("28",), 6.99),
    "Machinery n.e.c.": (("29",), 1.45),
    "Office": (("30",), 12.95),
    "Electrical": (("31",), 12.91),
    "Communication": (("32",), 3.95),
    "Medical": (("33",), 8.71),
    "Auto": (("34",), 1.84),
    "Other Transport": (("35",), 0.39),
    "Other": (("36", "37"), 3.98),
}

_THETA_OF_DIVISION = {d: theta for divisions, theta in CALIENDO_PARRO.values() for d in divisions}

# ISIC Rev 3 division of each CPA 2.1 activity of the OECD key (`goods.read_hs_cpa`), from the Rev 3 to Rev 4
# breaks: weapons (C254) and military vehicles (C304) were Rev 3 2927, computers (C262) Rev 3 30, magnetic and
# optical media (C268) Rev 3 2429, medical instruments (C325) Rev 3 3311, recycling (E38) Rev 3 37; books and
# recorded media arrive as C18 (`goods.ADAPTED_CPA`), Rev 3 22. Domestic appliances (Rev 3 2930) sit inside C27 and
# count as Electrical.
# Electricity and the service codes (D35, H53, M71, M74, R90, R91) and unallocated lines have no estimate.
REV3_OF_CPA = {
    "A01": "01", "A02": "02", "A03": "05",
    "B05": "10", "B06": "11", "B071": "13", "B072": "13", "B08": "14",
    "C10": "15", "C11": "15", "C12": "16", "C13": "17", "C14": "18", "C15": "19", "C16": "20", "C17": "21",
    "C18": "22", "C19": "23", "C20": "24", "C21": "24", "C268": "24", "C22": "25",
    "C23": "26", "C24A": "27", "C24B": "27", "C25X254": "28", "C254": "29", "C28": "29", "C304": "29",
    "C262": "30", "C27": "31", "C261": "32", "C263": "32", "C264": "32",
    "C265": "33", "C266": "33", "C267": "33", "C325": "33", "C29": "34", "C301": "35", "C302_309": "35",
    "C303": "35", "C31": "36", "C32X325": "36", "E38": "37",
}  # fmt: skip

# Largest ratio of two thetas in one class (ADR 0023), and the allowed class counts.
CLASS_SPREAD = 1.5
CLASS_COUNTS = (3, 4, 5)


def baci_values(baci_2000: Path) -> dict[str, float]:
    """Trade value (USD) by HS 6-digit line over every 2000 flow; BACI gives v in thousands of USD."""
    convert = pacsv.ConvertOptions(column_types={"k": pa.string(), "v": pa.float64()}, include_columns=["k", "v"])
    value: dict[str, float] = defaultdict(float)
    with zipfile.ZipFile(baci_2000) as z:
        member = next(n for n in z.namelist() if n.endswith("Y2000_V202601.csv"))
        with z.open(member) as f:
            for batch in pacsv.open_csv(f, convert_options=convert):
                grouped = pa.Table.from_batches([batch]).group_by("k").aggregate([("v", "sum")])
                for k, v in zip(grouped["k"].to_pylist(), grouped["v_sum"].to_pylist()):
                    value[k.zfill(6)] += v * 1000
    return value


def by_good(
    hs_cpa: dict[str, str], shares: dict[str, dict[int, float]], value: dict[str, float]
) -> tuple[np.ndarray, np.ndarray]:
    """Theta per good, the trade-value-weighted arithmetic mean of the sector estimates over the HS
    lines of the good (the sector of a line is the ISIC Rev 3 division of its CPA activity), and the trade
    value of the lines with an estimate. Lines without one, and goods without such lines, weigh nothing
    (theta 0)."""
    num, den = np.zeros(goods.N_GOODS), np.zeros(goods.N_GOODS)
    for hs, line_shares in shares.items():
        theta = _THETA_OF_DIVISION.get(REV3_OF_CPA.get(hs_cpa[hs], ""))
        v = value.get(hs, 0.0)
        if theta is None or v == 0.0:
            continue
        for good, share in line_shares.items():
            num[good] += share * v * theta
            den[good] += share * v
    return np.divide(num, den, out=np.zeros(goods.N_GOODS), where=den > 0), den


def caliendo_parro_by_good(baci_2000: Path, hs_key: Path) -> np.ndarray:
    """Theta per good (0 for the local goods 7 to 9) through the OECD HS to CPA key, BACI 2000 weights."""
    return caliendo_parro_with_value(baci_2000, hs_key)[0]


def caliendo_parro_with_value(baci_2000: Path, hs_key: Path) -> tuple[np.ndarray, np.ndarray]:
    """`caliendo_parro_by_good` and the trade value behind each good's theta."""
    hs_cpa = goods.read_hs_cpa(hs_key)
    shares = goods.hs_goods_shares(hs_cpa)
    theta, value = by_good(hs_cpa, shares, baci_values(baci_2000))
    local = [g for g in range(goods.N_GOODS) if g not in goods.TRADED]
    theta[local], value[local] = 0.0, 0.0
    return theta, value


def classes(theta: np.ndarray, value: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """ADR 0023: sort the thetas by log value and split them into the fewest classes (3 to 5) of
    consecutive goods whose largest theta is at most `CLASS_SPREAD` times their smallest; among
    such splits the one with the smallest largest spread, then the first in order. Returns the class
    of each good (0 for the lowest thetas) and each class's theta, the trade-value-weighted mean of
    its goods'."""
    theta, value = np.asarray(theta, dtype=np.float64), np.asarray(value, dtype=np.float64)
    order = np.argsort(theta, kind="stable")  # the order of log theta
    ts = theta[order]
    n = len(theta)
    for k in CLASS_COUNTS:
        best = None
        for cuts in itertools.combinations(range(1, n), k - 1):
            bounds = (0, *cuts, n)
            # as a ratio, exact at the bound: a log spread of ln 1.5 can round either way
            spread = max(ts[b - 1] / ts[a] for a, b in itertools.pairwise(bounds))
            if spread <= CLASS_SPREAD and (best is None or spread < best[0]):
                best = (spread, bounds)
        if best is not None:
            of = np.empty(n, dtype=np.uint8)
            class_theta = np.empty(k)
            for c, (a, b) in enumerate(itertools.pairwise(best[1])):
                members = order[a:b]
                of[members] = c
                class_theta[c] = np.average(theta[members], weights=value[members])
            return of, class_theta
    raise ValueError(f"no split into {CLASS_COUNTS[-1]} classes keeps theta within a factor {CLASS_SPREAD}: {theta}")
