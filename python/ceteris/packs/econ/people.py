"""2000 population, education, labour and Gini (specs/econ/SPEC.md, Pack additions; ADR 0014).

Provinces get WorldPop 2000 summed through the H3 cells. Countries get education shares (WIC
reconstruction, Barro-Lee as the second source), participation and unemployment (ILOSTAT modelled
estimates), the working-age share (WDI), the labour share (PWT 11) and the Gini (WDI, else SWIID).
A country with no value from any source takes the value of its nearest donor (same World Bank region,
closest log GDP per capita) and is flagged imputed.
"""

import csv
import io
import statistics
import xml.etree.ElementTree as ET
import zipfile
from collections import defaultdict
from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path

import h3.api.basic_int as h3
import numpy as np
import pyarrow as pa
import pyogrio
import rasterio
from rasterio.windows import Window

YEAR = 2000
# A value from a year other than 2000 is used when it lies within this many years (SPEC, Gini; same rule for all series).
WINDOW = 5
N_EDU = 3
# WIC levels: no education, some primary, primary | lower and upper secondary | post secondary.
WIC_LEVEL = {"e1": 0, "e2": 0, "e3": 0, "e4": 1, "e5": 1, "e6": 2}
# The working ages, the starting ages of the WIC five-year groups 15-19 to 60-64.
WORKING_AGES = range(15, 65, 5)
# Rings of base cells searched for the owner of a populated pixel whose own cell belongs to no province.
MAX_RING = 4

# Natural Earth codes that differ from the ISO 3166-1 alpha-3 code the data sources use.
EXTRA_ISO = {"KOS": "XKX"}
# Countries of 1 January 2000 that the sources split or rename: the source economies they combine.
PARTS = {"YUG": ("SRB", "MNE"), "SDN": ("SDN", "SSD"), "ANT": ("CUW", "SXM")}
# Names of the SWIID that neither the World Bank nor Natural Earth spells the same way.
SWIID_NAMES = {
    "Cape Verde": "CPV",
    "Congo-Brazzaville": "COG",
    "Congo-Kinshasa": "COD",
    "Korea": "KOR",
    "Palestinian Territories": "PSE",
    "St. Vincent and Grenadines": "VCT",
    "São Tomé and Príncipe": "STP",
}
# Barro-Lee uses older World Bank codes.
BARRO_LEE_CODES = {"ROM": "ROU", "ZAR": "COD", "TMP": "TLS"}


@dataclass
class Labour:
    """One country's 2000 labour inputs; each `*_imputed` flag is set when the value is a donor's or a fallback source's."""

    participation: float  # labour force over the population aged 15 to 64
    unemployment: float  # unemployed over labour force
    edu_shares: np.ndarray  # shares of the working-age population: below secondary, secondary, tertiary
    labour_share: float  # compensation of employees over GDP
    working_age_share: float  # population aged 15 to 64 over the whole population
    gini: float  # fraction in [0, 1]
    gini_source: str  # wdi, swiid or donor
    participation_imputed: bool = False
    unemployment_imputed: bool = False
    edu_imputed: bool = False
    labour_share_imputed: bool = False
    working_age_imputed: bool = False
    gini_imputed: bool = False


@dataclass
class Observed:
    """Series from the sources, keyed by ISO 3166-1 alpha-3; scalars are the year nearest 2000."""

    participation: dict[str, float] = field(default_factory=dict)
    unemployment: dict[str, float] = field(default_factory=dict)
    working_age: dict[str, float] = field(default_factory=dict)
    edu_wic: dict[str, np.ndarray] = field(default_factory=dict)
    edu_barro_lee: dict[str, np.ndarray] = field(default_factory=dict)
    labour_share: dict[str, float] = field(default_factory=dict)
    gini_wdi: dict[str, float] = field(default_factory=dict)
    gini_swiid: dict[str, float] = field(default_factory=dict)
    gdp_per_capita: dict[str, float] = field(default_factory=dict)
    population: dict[str, float] = field(default_factory=dict)
    region: dict[str, str] = field(default_factory=dict)


def nearest_year(series: Mapping[int, float], year: int = YEAR, window: int = WINDOW) -> float | None:
    """Value of the year nearest `year` within `window` years (the earlier year on a tie), else None."""
    near = [y for y in series if abs(y - year) <= window]
    return series[min(near, key=lambda y: (abs(y - year), y))] if near else None


def _nearest(series: Mapping[str, Mapping[int, float]]) -> dict[str, float]:
    found = {k: nearest_year(v) for k, v in series.items()}
    return {k: v for k, v in found.items() if v is not None}


def _csv(f) -> csv.DictReader:
    return csv.DictReader(io.TextIOWrapper(f, encoding="utf-8-sig"))


def read_codes(admin0: Path) -> tuple[dict[str, str], dict[str, int]]:
    """Natural Earth ADM0_A3 to ISO alpha-3 (as WDI and ILOSTAT write it), and ISO alpha-3 to UN M49 numeric code."""
    _, t = pyogrio.read_arrow(admin0, columns=["ADM0_A3", "ISO_A3", "ISO_A3_EH", "ISO_N3_EH"], read_geometry=False)
    iso, m49 = {}, {}
    for a3, own, i3, n3 in zip(*(t.column(c).to_pylist() for c in ("ADM0_A3", "ISO_A3", "ISO_A3_EH", "ISO_N3_EH"))):
        # The _EH code fills gaps (France, Norway) but also gives dependencies without a code of their own the parent's.
        if i3 != "-99" and (own != "-99" or a3 == i3):
            iso[a3] = i3
            if n3 != "-99":
                m49[i3] = int(n3)
    return {**iso, **EXTRA_ISO}, m49


def read_ilostat(path: Path, age: str) -> dict[str, float]:
    """ILOSTAT indicator CSV (modelled estimates, 2000): rate as a fraction for both sexes and one age class."""
    with path.open("rb") as f:
        return {
            r["ref_area"]: float(r["obs_value"]) / 100
            for r in _csv(f)
            if r["sex"] == "SEX_T" and r["classif1"] == age and r["obs_value"]
        }


def read_wic(path: Path, m49_to_iso: Mapping[int, str]) -> dict[str, np.ndarray]:
    """WIC reconstruction CSV: shares of the 2000 population aged 15 to 64 in the three education levels, by ISO code."""
    counts: dict[int, np.ndarray] = defaultdict(lambda: np.zeros(N_EDU))
    with path.open("rb") as f:
        for r in _csv(f):
            if r["Time"] == str(YEAR) and int(r["agest"]) in WORKING_AGES:
                counts[int(r["region"].removeprefix("reg"))][WIC_LEVEL[r["edu"]]] += float(r["pop"])
    return {m49_to_iso[m]: c / c.sum() for m, c in counts.items() if m in m49_to_iso and c.sum() > 0}


def read_barro_lee(path: Path) -> dict[str, np.ndarray]:
    """Barro-Lee v3 (both sexes, aged 15 to 64, year 2000): shares with at most primary, secondary and tertiary schooling."""
    out = {}
    with path.open("rb") as f:
        for r in _csv(f):
            if r["year"] == str(YEAR) and r["sex"] == "MF":
                lu, lp, ls, lh = (float(r[k]) for k in ("lu", "lp", "ls", "lh"))
                out[BARRO_LEE_CODES.get(r["WBcode"], r["WBcode"])] = np.array([lu + lp, ls, lh]) / (lu + lp + ls + lh)
    return out


def read_wdi(path: Path) -> dict[str, dict[int, float]]:
    """World Bank indicator zip: value by year for each economy, in the indicator's own units."""
    with zipfile.ZipFile(path) as z:
        member = next(n for n in z.namelist() if n.startswith("API_"))
        text = io.TextIOWrapper(z.open(member), encoding="utf-8-sig").read()
    lines = text.splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith('"Country Name"'))
    out: dict[str, dict[int, float]] = {}
    for r in csv.DictReader(lines[start:]):
        out[r["Country Code"]] = {int(y): float(v) for y, v in r.items() if y and y.isdigit() and v}
    return out


def read_wdi_regions(path: Path) -> tuple[dict[str, str], dict[str, str]]:
    """World Bank region by economy code (aggregates have none), and economy code by table name."""
    with zipfile.ZipFile(path) as z:
        member = next(n for n in z.namelist() if n.startswith("Metadata_Country"))
        rows = [r for r in _csv(z.open(member)) if r["Region"]]
    return {r["Country Code"]: r["Region"] for r in rows}, {r["TableName"]: r["Country Code"] for r in rows}


def read_swiid(path: Path, names: Mapping[str, str]) -> dict[str, dict[int, float]]:
    """SWIID summary (zip): disposable income Gini as a fraction by year, for the countries `names` can place."""
    out: dict[str, dict[int, float]] = defaultdict(dict)
    with zipfile.ZipFile(path) as z:
        member = next(n for n in z.namelist() if n.endswith("_summary.csv"))
        for r in _csv(z.open(member)):
            iso = SWIID_NAMES.get(r["country"], names.get(r["country"]))
            if iso and r["gini_disp"]:
                out[iso][int(r["year"])] = float(r["gini_disp"]) / 100
    return dict(out)


def _xlsx_sheet(path: Path, sheet: str) -> Iterator[dict[str, object]]:
    """Rows of one worksheet (first row is the header) as dicts by header; numbers as float, text as str."""
    main = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
    rel = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"
    with zipfile.ZipFile(path) as z:
        book = ET.parse(z.open("xl/workbook.xml")).getroot()
        rid = next(s.get(rel) for s in book.iter(f"{main}sheet") if s.get("name") == sheet)
        rels = ET.parse(z.open("xl/_rels/workbook.xml.rels")).getroot()
        target = next(r.get("Target") for r in rels if r.get("Id") == rid)
        strings = [
            "".join(t.text or "" for t in si.iter(f"{main}t")) for si in ET.parse(z.open("xl/sharedStrings.xml")).getroot()
        ]
        header: dict[str, str] = {}
        for _, row in ET.iterparse(z.open("xl/" + target.removeprefix("/xl/")), events=("end",)):
            if row.tag != f"{main}row":
                continue
            cells: dict[str, object] = {}
            for c in row:
                v = c.find(f"{main}v")
                if v is None or v.text is None:
                    continue
                col = "".join(ch for ch in c.get("r") if ch.isalpha())
                cells[col] = strings[int(v.text)] if c.get("t") == "s" else v.text if c.get("t") == "str" else float(v.text)
            row.clear()
            if not header:
                header = {col: str(v) for col, v in cells.items()}
            elif cells:
                yield {header[col]: v for col, v in cells.items() if col in header}


def read_pwt(path: Path) -> tuple[dict[str, dict[int, float]], dict[str, dict[int, float]], dict[str, dict[int, float]]]:
    """PWT 11 Data sheet: labour share (labsh), GDP per person (rgdpna / pop) and population (millions) by year."""
    share: dict[str, dict[int, float]] = defaultdict(dict)
    income: dict[str, dict[int, float]] = defaultdict(dict)
    people: dict[str, dict[int, float]] = defaultdict(dict)
    for r in _xlsx_sheet(path, "Data"):
        code, year = str(r["countrycode"]), int(r["year"])
        if "labsh" in r:
            share[code][year] = float(r["labsh"])
        if "pop" in r:
            people[code][year] = float(r["pop"])
            if "rgdpna" in r and r["pop"]:
                income[code][year] = float(r["rgdpna"]) / float(r["pop"])
    return dict(share), dict(income), dict(people)


def load_observed(cache: Path, admin0: Path) -> Observed:
    """Read every source from the fetch cache (`fetch.toml` names)."""
    _, m49 = read_codes(admin0)
    m49_to_iso = {m: i for i, m in m49.items()}
    gini_zip = cache / "wdi_SI.POV.GINI.zip"
    region, table_names = read_wdi_regions(gini_zip)
    ne_names = {}
    _, t = pyogrio.read_arrow(admin0, columns=["ADMIN", "NAME", "NAME_LONG", "ISO_A3_EH"], read_geometry=False)
    for admin, name, long, i3 in zip(*(t.column(c).to_pylist() for c in ("ADMIN", "NAME", "NAME_LONG", "ISO_A3_EH"))):
        for n in (admin, name, long):
            ne_names.setdefault(n, i3)
    labsh, income, _ = read_pwt(cache / "pwt110.xlsx")
    pop = read_wdi(cache / "wdi_SP.POP.TOTL.zip")
    return Observed(
        participation=read_ilostat(cache / "ilostat_EAP_2WAP_SEX_AGE_RT_A_2000.csv", "AGE_YTHADULT_Y15-64"),
        unemployment=read_ilostat(cache / "ilostat_UNE_2EAP_SEX_AGE_RT_A_2000.csv", "AGE_YTHADULT_YGE15"),
        working_age={k: v / 100 for k, v in _nearest(read_wdi(cache / "wdi_SP.POP.1564.TO.ZS.zip")).items()},
        edu_wic=read_wic(cache / "BPJresult_AGE_V01.csv", m49_to_iso),
        edu_barro_lee=read_barro_lee(cache / "BL_v3_MF1564.csv"),
        labour_share=_nearest(labsh),
        gini_wdi={k: v / 100 for k, v in _nearest(read_wdi(gini_zip)).items()},
        gini_swiid=_nearest(read_swiid(cache / "swiid9_92.zip", {**ne_names, **table_names})),
        gdp_per_capita=_nearest(income),
        population={k: v[YEAR] for k, v in pop.items() if YEAR in v},
        region=region,
    )


def _weighted(values: Sequence, weights: Sequence[float]):
    if len(values) == 1:
        return values[0]
    w = np.asarray(weights, float)
    mean = sum(v * x for v, x in zip(values, w)) / w.sum()
    return mean if isinstance(mean, np.ndarray) else float(mean)


def donor(
    region: str | None, log_income: float | None, observed: Mapping[str, object], obs: Observed, exclude: Sequence[str]
) -> str:
    """Observed economy in the same region with the closest log GDP per person (the region's median if unknown)."""
    pool = [k for k in sorted(observed) if k not in exclude and k in obs.gdp_per_capita]
    near = [k for k in pool if obs.region.get(k) == region] if region else []
    pool = near or pool
    logs = {k: np.log(obs.gdp_per_capita[k]) for k in pool}
    target = log_income if log_income is not None else statistics.median(logs.values())
    return min(pool, key=lambda k: (abs(logs[k] - target), k))


def country_labour(countries: Mapping[str, str], obs: Observed, iso: Mapping[str, str]) -> dict[str, Labour]:
    """Labour inputs for each pack country (code -> sovereign code), from data where any source has the country.

    A country the sources split (`PARTS`) takes the population-weighted mean of its parts. Education comes
    from WIC, else Barro-Lee. The Gini comes from WDI, else SWIID (flagged imputed), else a donor (flagged imputed).
    """

    def parts(code: str) -> list[str]:
        return list(PARTS.get(code, (iso.get(code, code),)))

    def weights(ps: Sequence[str]) -> list[float]:
        return [obs.population.get(p, 1.0) for p in ps]

    def lookup(series: Mapping[str, object], code: str):
        found = [p for p in parts(code) if p in series]
        return _weighted([series[p] for p in found], weights(found)) if found else None

    def features(code: str) -> tuple[str | None, float | None]:
        ps = parts(code)
        sovereign = parts(countries.get(code, code))
        region = next((obs.region[p] for p in ps + sovereign if p in obs.region), None)
        income = lookup(obs.gdp_per_capita, code)
        return region, None if income is None else float(np.log(income))

    def value(code: str, *series: Mapping[str, object]):
        """(value, index of the series that gave it) with the donor's value as index len(series)."""
        for i, s in enumerate(series):
            v = lookup(s, code)
            if v is not None:
                return v, i
        merged = {k: v for s in reversed(series) for k, v in s.items()}
        region, income = features(code)
        return merged[donor(region, income, merged, obs, parts(code))], len(series)

    out = {}
    for code in countries:
        participation, p_i = value(code, obs.participation)
        unemployment, u_i = value(code, obs.unemployment)
        working_age, w_i = value(code, obs.working_age)
        labour_share, l_i = value(code, obs.labour_share)
        edu, e_i = value(code, obs.edu_wic, obs.edu_barro_lee)
        gini, g_i = value(code, obs.gini_wdi, obs.gini_swiid)
        out[code] = Labour(
            participation=participation,
            unemployment=unemployment,
            edu_shares=edu / edu.sum(),
            labour_share=labour_share,
            working_age_share=working_age,
            gini=gini,
            gini_source=("wdi", "swiid", "donor")[g_i],
            participation_imputed=p_i == 1,
            unemployment_imputed=u_i == 1,
            edu_imputed=e_i == 2,
            labour_share_imputed=l_i == 1,
            working_age_imputed=w_i == 1,
            gini_imputed=g_i > 0,
        )
    return out


def country_columns(codes: Sequence[str], labour: Mapping[str, Labour]) -> dict[str, pa.Array]:
    """`econ_countries.arrow` columns for this task, in `codes` order."""
    rows = [labour[c] for c in codes]
    column = {
        "participation": [r.participation for r in rows],
        "unemployment": [r.unemployment for r in rows],
        "working_age_share": [r.working_age_share for r in rows],
        "labour_share": [r.labour_share for r in rows],
        "gini": [r.gini for r in rows],
    }
    out = {k: pa.array(v, pa.float64()) for k, v in column.items()}
    for level, name in enumerate(("edu_low", "edu_secondary", "edu_tertiary")):
        out[name] = pa.array([r.edu_shares[level] for r in rows], pa.float64())
    for flag in ("participation", "unemployment", "working_age", "labour_share", "edu", "gini"):
        out[f"{flag}_imputed"] = pa.array([getattr(r, f"{flag}_imputed") for r in rows], pa.bool_())
    return out


def province_population(worldpop: Path, cells: pa.Table, block_rows: int = 512) -> np.ndarray:
    """WorldPop 2000 people per province: each populated pixel goes to the province owning its H3 cell.

    `cells` is the pack's cells table (`cell` uint64, `province`); cells of several resolutions may
    coexist, and the finest cell containing a pixel decides. A pixel in a cell no province owns (a coastal
    cell whose centre is at sea) goes to the nearest owned cell within `MAX_RING` rings; pixels further
    out are dropped.
    """
    owner_of = dict(zip(cells.column("cell").to_pylist(), cells.column("province").to_pylist()))
    n = max(owner_of.values()) + 1
    resolutions = sorted({h3.get_resolution(c) for c in owner_of})
    base, finer = resolutions[0], resolutions[1:]
    split = {h3.cell_to_parent(c, base) for c in owner_of if h3.get_resolution(c) > base}
    nearest: dict[int, int] = {}

    def of_base(cell: int) -> int:
        if cell in owner_of:
            return owner_of[cell]
        if cell not in nearest:
            lat, lng = h3.cell_to_latlng(cell)
            nearest[cell] = -1
            for k in range(1, MAX_RING + 1):
                owned = [c for c in h3.grid_disk(cell, k) if c in owner_of]
                if owned:
                    best = min(
                        owned, key=lambda c: ((h3.cell_to_latlng(c)[0] - lat) ** 2 + (h3.cell_to_latlng(c)[1] - lng) ** 2, c)
                    )
                    nearest[cell] = owner_of[best]
                    break
        return nearest[cell]

    total = np.zeros(n)
    with rasterio.open(worldpop) as ds:
        a, _, x0, _, e, y0 = ds.transform[:6]
        for r0 in range(0, ds.height, block_rows):
            block = ds.read(1, window=Window(0, r0, ds.width, min(block_rows, ds.height - r0)))
            valid = block > 0
            if ds.nodata is not None:
                valid &= block != np.float32(ds.nodata)
            rows, cols = np.nonzero(valid)
            if not len(rows):
                continue
            lat = (y0 + (rows + r0 + 0.5) * e).tolist()
            lng = (x0 + (cols + 0.5) * a).tolist()
            cell = np.fromiter((h3.latlng_to_cell(la, lo, base) for la, lo in zip(lat, lng)), np.int64, len(lat))
            unique, inverse = np.unique(cell, return_inverse=True)
            owner = np.array([of_base(c) for c in unique.tolist()])[inverse]
            fine = np.flatnonzero(np.isin(cell, list(split))) if split else ()
            for i in fine:
                for r in reversed(finer):
                    child = owner_of.get(h3.latlng_to_cell(lat[i], lng[i], r))
                    if child is not None:
                        owner[i] = child
                        break
            kept = owner >= 0
            total += np.bincount(owner[kept], block[rows, cols][kept].astype(np.float64), n)
    return total


def national_population(codes: Sequence[str], obs: Observed, iso: Mapping[str, str]) -> dict[str, float]:
    """WDI population in 2000 (UN World Population Prospects) of each pack country, summing split economies."""
    totals = {c: sum(obs.population.get(p, 0.0) for p in PARTS.get(c, (iso.get(c, c),))) for c in codes}
    return {c: v for c, v in totals.items() if v > 0}


def scale_to_national(population: np.ndarray, country: Sequence[str], totals: Mapping[str, float]) -> np.ndarray:
    """Scale each country's provinces to its national total, keeping their shares (WorldPop's UN adjustment).

    WorldPop 2000 differs from the UN estimates by -12 to +38% for large countries, and H3 cell ownership
    moves a few percent across borders. A country without a total, or without people in WorldPop, is unchanged.
    """
    country = np.asarray(country)
    out = population.astype(np.float64).copy()
    for c in np.unique(country):
        here = country == c
        have = population[here].sum()
        if c in totals and have > 0:
            out[here] *= totals[c] / have
    return out


def province_columns(country: Sequence[str], population: np.ndarray, labour: Mapping[str, Labour]) -> dict[str, pa.Array]:
    """`econ_provinces.arrow` columns for this task: population and working-age population (15 to 64)."""
    share = np.array([labour[c].working_age_share for c in country])
    imputed = [labour[c].working_age_imputed for c in country]
    return {
        "population": pa.array(population, pa.float64()),
        "working_age": pa.array(population * share, pa.float64()),
        "working_age_imputed": pa.array(imputed, pa.bool_()),
    }


def pops(country: Sequence[str], population: np.ndarray, labour: Mapping[str, Labour]) -> pa.Table:
    """Pops: province-major, three education rows each.

    A province splits its people (children included) by its country's education shares of the working-age
    population; its working-age people and labour force follow. The row is imputed when any share it uses is.
    """
    n = len(country)
    rows = [labour[c] for c in country]
    shares = np.array([r.edu_shares for r in rows])
    working_age = population * np.array([r.working_age_share for r in rows])
    participation = np.array([r.participation for r in rows])
    imputed = np.array([r.edu_imputed or r.working_age_imputed or r.participation_imputed for r in rows])
    people = (population[:, None] * shares).ravel()
    adults = (working_age[:, None] * shares).ravel()
    return pa.table(
        {
            "province": pa.array(np.repeat(np.arange(n), N_EDU), pa.uint32()),
            "edu": pa.array(np.tile(np.arange(N_EDU), n), pa.uint8()),
            "population": pa.array(people, pa.float64()),
            "working_age": pa.array(adults, pa.float64()),
            "labour_force": pa.array((adults.reshape(n, N_EDU) * participation[:, None]).ravel(), pa.float64()),
            "imputed": pa.array(np.repeat(imputed, N_EDU), pa.bool_()),
        }
    )
