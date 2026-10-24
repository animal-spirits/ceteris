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
    hs_isic3: dict[str, str], shares: dict[str, dict[int, float]], value: dict[str, float]
) -> tuple[np.ndarray, np.ndarray]:
    """Theta per good, the trade-value-weighted arithmetic mean of the sector estimates over the HS
    lines of the good (the sector of a line is its ISIC Rev 3 division), and the trade value of the
    lines with an estimate. Lines without one, and goods without such lines, weigh nothing (theta 0)."""
    num, den = np.zeros(goods.N_GOODS), np.zeros(goods.N_GOODS)
    for hs, line_shares in shares.items():
        theta = _THETA_OF_DIVISION.get(hs_isic3[hs][:2])
        v = value.get(hs, 0.0)
        if theta is None or v == 0.0:
            continue
        for good, share in line_shares.items():
            num[good] += share * v * theta
            den[good] += share * v
    return np.divide(num, den, out=np.zeros(goods.N_GOODS), where=den > 0), den


def caliendo_parro_by_good(baci_2000: Path, hs_isic: Path, isic_bridge: Path) -> np.ndarray:
    """Theta per good (0 for the local goods 7 to 9) through the Task 13 concordance, BACI 2000 weights."""
    return caliendo_parro_with_value(baci_2000, hs_isic, isic_bridge)[0]


def caliendo_parro_with_value(baci_2000: Path, hs_isic: Path, isic_bridge: Path) -> tuple[np.ndarray, np.ndarray]:
    """`caliendo_parro_by_good` and the trade value behind each good's theta."""
    hs_isic3 = goods.read_hs_isic3(hs_isic)
    shares = goods.hs_goods_shares(hs_isic3, goods.read_bridge(isic_bridge))
    theta, value = by_good(hs_isic3, shares, baci_values(baci_2000))
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
