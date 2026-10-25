"""2000 applied tariffs by importer, exporter and tariff group (specs/econ/SPEC.md, Pack additions; trade SPEC, Tariff wedge).

The importer's WDI weighted mean applied rate of each group (primary goods 0 to 3, manufactures 4 to 6) applies to
every exporter outside its trade agreements; inside a free trade agreement or customs union in force on 1 January 2000
the rate is zero. Agreements come from the CEPII gravity RTA flags (in force in 1999 and 2000) and the World Bank Deep
Trade Agreements pair list (in force on 1 January 2000), whichever lists the pair. Partial scope agreements and
services-only agreements leave the rate unchanged. An importer with no WDI rate within five years of 2000 takes its
donor's (people.donor: same region, nearest log income), flagged imputed.
"""

import zipfile
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.csv as pacsv

from . import icio, people

YEAR = people.YEAR
# WDI weighted mean applied rates, percent: primary products, manufactured products (the tariff groups).
SERIES = ("TM.TAX.TCOM.WM.AR.ZS", "TM.TAX.MANF.WM.AR.ZS")
GROUPS = ("primary", "manufactured")
GRAVITY = "Gravity_csv_V202211.zip"
DTA = "DTA 1.0 - Horizontal Content (v2).xlsx"
# CEPII rta_type codes that remove tariffs on goods: customs unions and free trade agreements, with or without an EIA.
CEPII_FREE = {1, 2, 4, 5}
DTA_FREE = {"CU", "CU & EIA", "FTA", "FTA & EIA"}
# 1 January 2000 as an Excel serial date (the DTA workbook's dates).
NEW_YEAR_SERIAL = 36526
# A weighted mean applied rate of 100% or more is a data error (WDI gives Djibouti 534.76% for primary goods in 1999,
# 100.13% in 1998 and 20.97% in 2002); such years count as missing.
MAX_RATE = 1.0
# Source codes of economies that a pack country combines or that the sources name otherwise, besides people.PARTS.
ALIASES = {"SCG": "YUG", "SRB": "YUG", "MNE": "YUG", "ANT": "ANT"}


@dataclass
class Tariffs:
    rate: np.ndarray  # [importer, exporter, group], fraction
    own: np.ndarray  # [importer, group]: the importer's rate outside its agreements
    agreement: np.ndarray  # [importer, exporter], bool
    imputed: np.ndarray  # [importer], bool: the importer's rates are its donor's
    donor: list[str]  # per importer, "" where the rates are its own


def read_rates(cache: Path) -> list[dict[str, float]]:
    """WDI rate per group as a fraction, the year nearest 2000 within five years, by source code."""
    return [plausible_rates(people.read_wdi(cache / f"wdi_{s}.zip")) for s in SERIES]


def plausible_rates(series: Mapping[str, Mapping[int, float]]) -> dict[str, float]:
    """Rate as a fraction of the year nearest 2000 within five years, leaving out years at or above `MAX_RATE`."""
    found = {k: people.nearest_year({y: v for y, v in by_year.items() if v < 100 * MAX_RATE}) for k, by_year in series.items()}
    return {k: v / 100 for k, v in found.items() if v is not None}


def read_cepii(path: Path) -> set[tuple[str, str]]:
    """Source code pairs with a goods FTA or customs union flagged in force in both 1999 and 2000 (CEPII gravity)."""
    convert = pacsv.ConvertOptions(
        include_columns=["year", "iso3_o", "iso3_d", "fta_wto", "rta_type"],
        column_types={
            "year": pa.int32(),
            "iso3_o": pa.string(),
            "iso3_d": pa.string(),
            "fta_wto": pa.float64(),
            "rta_type": pa.float64(),
        },
    )
    by_year: dict[int, set[tuple[str, str]]] = {YEAR - 1: set(), YEAR: set()}
    with zipfile.ZipFile(path) as z:
        member = next(n for n in z.namelist() if n.startswith("Gravity_V"))
        with z.open(member) as f:
            for batch in pacsv.open_csv(f, convert_options=convert):
                keep = pc.and_(pc.is_in(batch["year"], pa.array([YEAR - 1, YEAR], pa.int32())), pc.equal(batch["fta_wto"], 1.0))
                b = batch.filter(pc.fill_null(keep, False))
                for y, o, d, t in zip(*(b[c].to_pylist() for c in ("year", "iso3_o", "iso3_d", "rta_type"))):
                    if t is not None and int(t) in CEPII_FREE:
                        by_year[y].add((o, d))
    return by_year[YEAR - 1] & by_year[YEAR]


def read_dta(path: Path) -> set[tuple[str, str]]:
    """Source code pairs in a goods FTA or customs union in force on 1 January 2000 (World Bank DTA pair list).

    The pair list has one row per pair, agreement and year in force. A pair listed for 2000 is in force on 1 January
    when it is also listed for an earlier year, or when its agreement entered into force on 1 January 2000; a pair
    first listed in 2000 under an older agreement joined during the year, on a date the list does not give.
    """
    entry = {}
    for r in people._xlsx_sheet(path, "Agreements"):
        if isinstance(r.get("WB ID"), float) and isinstance(r.get("Date of Entry into Force (G)"), float):
            entry[int(r["WB ID"])] = r["Date of Entry into Force (G)"]
    first: dict[tuple[str, str, int], float] = {}
    listed = set()
    for r in people._xlsx_sheet(path, "Bilateral Information"):
        if isinstance(r.get("year"), float) and r.get("type") in DTA_FREE and r["year"] <= YEAR:
            key = (r["iso1"], r["iso2"], int(r["WBID"]))
            first[key] = min(first.get(key, np.inf), r["year"])
            if r["year"] == YEAR:
                listed.add(key)
    out = set()
    for key in sorted(listed):
        a, b, wb = key
        if first[key] < YEAR or entry.get(wb) == NEW_YEAR_SERIAL:
            out.add((a, b))
    return out


def _sources(code: str, iso: Mapping[str, str]) -> tuple[str, ...]:
    return people.PARTS.get(code, (iso.get(code, code),))


def pack_pairs(pairs: Iterable[tuple[str, str]], codes: Sequence[str], iso: Mapping[str, str]) -> set[tuple[int, int]]:
    """Source code pairs as pack country index pairs, both orders; pairs within one pack country are dropped."""
    index: dict[str, int] = {}
    for i, c in enumerate(codes):
        for s in (*_sources(c, iso), c):
            index.setdefault(s, i)
    for s, c in ALIASES.items():
        if c in codes:
            index.setdefault(s, list(codes).index(c))
    out = set()
    for a, b in pairs:
        i, j = index.get(a), index.get(b)
        if i is not None and j is not None and i != j:
            out |= {(i, j), (j, i)}
    return out


def table(
    rates: Sequence[Mapping[str, float]],
    agreements: set[tuple[int, int]],
    codes: Sequence[str],
    iso: Mapping[str, str],
    obs: people.Observed,
    sovereign: Mapping[str, str] | None = None,
) -> Tariffs:
    """Rates for every (importer, exporter, group) of the pack countries `codes`; zero on the diagonal and in agreements.

    A dependency (`sovereign` names another pack country) trades free of tariffs with its sovereign, and without a WDI
    rate of its own it takes the sovereign's; any other country without one takes its donor's.
    """
    sovereign = sovereign or {}
    index = {c: i for i, c in enumerate(codes)}
    n = len(codes)
    own = np.full((n, len(GROUPS)), np.nan)
    for i, c in enumerate(codes):
        for g, series in enumerate(rates):
            found = [series[s] for s in _sources(c, iso) if s in series]
            if found:
                own[i, g] = float(np.mean(found))
    have = {c: own[i] for i, c in enumerate(codes) if np.isfinite(own[i]).all()}
    by_iso = {iso.get(c, c): v for c, v in have.items()}
    donors = [""] * n
    for i, c in enumerate(codes):
        if c not in have and sovereign.get(c, c) in have and sovereign[c] != c:
            donors[i] = sovereign[c]
            own[i] = have[sovereign[c]]
        elif c not in have:
            ps = list(_sources(c, iso))
            region = icio.REGIONS.get(c) or next((obs.region[p] for p in ps if p in obs.region), None)
            income = [obs.gdp_per_capita[p] for p in ps if p in obs.gdp_per_capita]
            d = people.donor(region, float(np.log(np.mean(income))) if income else None, by_iso, obs, ps)
            donors[i] = d
            own[i] = by_iso[d]
    rate = np.repeat(own[:, None, :], n, axis=1)
    agreement = np.zeros((n, n), bool)
    for i, j in agreements:
        agreement[i, j] = True
    for c, s in sovereign.items():
        if c != s and c in index and s in index:
            agreement[index[c], index[s]] = agreement[index[s], index[c]] = True
    rate[agreement] = 0.0
    rate[np.arange(n), np.arange(n)] = 0.0
    return Tariffs(rate, own, agreement, np.array([d != "" for d in donors]), donors)


def columns(t: Tariffs) -> dict[str, pa.Array]:
    """`econ_tariffs.arrow`: one row per (importer, exporter, group), importer-major, groups innermost."""
    n, _, k = t.rate.shape
    imp, exp, grp = np.meshgrid(np.arange(n), np.arange(n), np.arange(k), indexing="ij")
    return {
        "importer": pa.array(imp.ravel().astype(np.uint32)),
        "exporter": pa.array(exp.ravel().astype(np.uint32)),
        "group": pa.array(grp.ravel().astype(np.uint8)),
        "rate": pa.array(t.rate.ravel(), pa.float64()),
        "agreement": pa.array(np.repeat(t.agreement.ravel(), k), pa.bool_()),
        "imputed": pa.array(np.repeat(np.repeat(t.imputed, n), k), pa.bool_()),
    }


def report(t: Tariffs, codes: Sequence[str], trade: Mapping[str, float] | None = None) -> str:
    n = len(codes)
    pairs = int(t.agreement.sum()) // 2
    lines = [
        (
            f"tariffs: {n - int(t.imputed.sum())} of {n} importers with WDI rates, {int(t.imputed.sum())} from donors; "
            f"{pairs} country pairs in agreements"
        ),
        "mean rate (primary, manufactured), own importers: " + ", ".join(f"{x:.4f}" for x in t.own[~t.imputed].mean(axis=0)),
    ]
    if trade:
        covered = sum(v for c, v in trade.items() if c in codes and not t.imputed[list(codes).index(c)])
        lines.append(f"share of 2000 trade with own rates: {covered / sum(trade.values()):.4f}")
    return "\n".join(lines)
