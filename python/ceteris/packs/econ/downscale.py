"""Province value added by good, downscaled from national VA (specs/econ/SPEC.md, Downscaling VA to provinces; ADR 0014).

Each country's VA of a good is split over its provinces by the first proxy of the good's chain that has a
positive total in the country's eligible provinces (at least `MIN_WORKING_AGE` working-age people):

- agriculture: MapSPAM 2000 physical area of all crops (cropland), then population;
- crude oil and gas: in the United States, EIA 2000 state production (`eia_oil_gas`); elsewhere GEM oil and
  gas fields; then population. GEM is flagged imputed where undated fields carry most of a country's weight;
- coal and other mining: GEM coal mines and P1 mineral sites (weighted by the country's coal share of the
  good), then population; without the GEM file, mineral sites alone;
- manufacturing (3 to 6): DOSE manufacturing GVA of the regions matched to provinces by exact name, then
  gridded GDP, then population;
- local goods (7 to 9): population, a base point only.

Provinces below the working-age floor get no VA; theirs follows the same proxy over the other provinces. A
country with no eligible province puts all its VA in its province with the most working-age people. A row is
imputed when a fallback placed it or the national VA is itself imputed (`CountryIO.va_basis` other than icio).

MapSPAM 2000 has no value of production (only area, production in tonnes of 20 crops, and yield), so cropland
stands in for it: tonnes of different crops do not add up, and SPAM2005's value of production ships with a
non-commercial licence. DOSE regions carry GADM GID codes and no ISO 3166-2 codes, so they are matched by name
only, never through GADM geometry (ADR 0005).
"""

import csv
import statistics
import unicodedata
import zipfile
from collections import defaultdict
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import NamedTuple

import numpy as np
import pyarrow as pa
import pyogrio
import rasterio
from rasterio.io import MemoryFile

from . import people
from .goods import N_GOODS, NAMES
from .icio import CountryIO

YEAR = people.YEAR
# Provinces with fewer working-age people get no VA (SPEC, Downscaling VA to provinces).
MIN_WORKING_AGE = 1000.0
# Cubic metres of natural gas per barrel of oil equivalent (5,800 cubic feet).
M3_PER_BOE = 5800 * 0.0283168466

SPAM = "spam2000v3.0.7_global_physical-area.geotiff.zip"
KUMMU = "kummu_gdp_ppp_2000_5arcmin.tif"
SEDS = "Complete_SEDS.csv"
# SEDS series: crude oil production and marketed natural gas production, billion Btu.
SEDS_SERIES = ("PAPRB", "NGMPB")
# SEDS federal offshore areas and the states whose coasts they lie off.
OFFSHORE = {"X3": ("TX", "LA", "MS", "AL", "FL"), "X5": ("CA",)}
DOSE = "DOSE_V2.14.csv"
ADMIN0 = "ne_10m_admin_0_countries.zip"
ADMIN1 = "ne_10m_admin_1_states_provinces.zip"
# Natural Earth admin-1 name fields a DOSE region name may equal, the primary name first; `|` separates alternates.
NAME_FIELDS = ("name", "name_en", "name_alt", "woe_name", "gn_name", "gns_name", "name_local")
RESOURCES = ("precious", "base", "ferrous", "energy", "industrial")

# Statuses of mines and fields that may have produced in 2000 (closed coal mines are judged by their closing year).
COAL_OPEN = {"Operating", "Mothballed"}
FIELD_PRODUCED = {"operating", "mothballed", "abandoned", "decommissioning"}

LEVELS = ("crops", "gem", "mining", "sites", "dose", "dose_rest", "gdp", "population", "largest", "eia")
# GEM oil and gas is flagged imputed in a country where fields with no start or discovery year carry more of its weight.
MAX_UNDATED = 0.5
MANUFACTURING = (3, 4, 5, 6)
LOCAL = (7, 8, 9)


@dataclass
class Provinces:
    country: Sequence[str]  # pack country code per province
    population: np.ndarray
    working_age: np.ndarray  # people aged 15 to 64


@dataclass
class Proxies:
    """Province proxies; None marks a source that is not available."""

    crops: np.ndarray | None = None  # MapSPAM 2000 physical area of all crops (ha)
    oil_gas: np.ndarray | None = None  # GEM oil and gas fields (million boe per year)
    oil_gas_undated: np.ndarray | None = None  # the part of `oil_gas` from fields with no start or discovery year
    eia: np.ndarray | None = None  # EIA 2000 crude oil and marketed gas (billion Btu), United States provinces only
    coal: np.ndarray | None = None  # GEM coal mines (Mt per year)
    sites: np.ndarray | None = None  # P1 producing and past-producing mineral sites (USGS MRDS)
    dose: np.ndarray | None = None  # DOSE manufacturing GVA of matched regions, NaN for provinces no region matches
    dose_rest: Mapping[str, float] = field(default_factory=dict)  # by country: DOSE share in regions without a province
    gdp: np.ndarray | None = None  # Kummu gridded GDP 2000 (PPP)


@dataclass
class Downscaled:
    country: np.ndarray
    va: np.ndarray  # [P, N_GOODS], million USD
    imputed: np.ndarray  # [P, N_GOODS]
    level: np.ndarray  # [P, N_GOODS], index into LEVELS of the proxy that placed the VA
    dropped: list[str] = field(default_factory=list)  # countries with VA but no province


@dataclass
class DoseMatch:
    matched: int
    regions: int
    gva_share: float  # share of the DOSE manufacturing GVA in matched regions
    by_country: dict[str, tuple[int, int]] = field(default_factory=dict)

    @property
    def rate(self) -> float:
        return self.matched / self.regions if self.regions else 0.0


Level = Callable[[np.ndarray, str, CountryIO], tuple[np.ndarray, np.ndarray, np.ndarray] | None]


def _plain(name: str, x: np.ndarray | None, imputed: bool) -> Level:
    def weights(idx, code, c):
        if x is None or not x[idx].sum() > 0:
            return None
        return x[idx], np.full(len(idx), imputed), np.full(len(idx), LEVELS.index(name))

    return weights


def _gem(p: Proxies) -> Level:
    def weights(idx, code, c):
        if p.oil_gas is None or not p.oil_gas[idx].sum() > 0:
            return None
        undated = p.oil_gas_undated is not None and p.oil_gas_undated[idx].sum() > MAX_UNDATED * p.oil_gas[idx].sum()
        return p.oil_gas[idx], np.full(len(idx), undated), np.full(len(idx), LEVELS.index("gem"))

    return weights


def _mining(p: Proxies) -> Level:
    if p.coal is None:
        return _plain("sites", p.sites, True)

    def weights(idx, code, c):
        coal = p.coal[idx].sum()
        sites = p.sites[idx].sum() if p.sites is not None else 0.0
        if coal > 0 and sites > 0:
            w = c.coal_share * p.coal[idx] / coal + (1 - c.coal_share) * p.sites[idx] / sites
        elif coal > 0:
            w = p.coal[idx]
        elif sites > 0:
            w = p.sites[idx]
        else:
            return None
        return w, np.zeros(len(idx), bool), np.full(len(idx), LEVELS.index("mining"))

    return weights


def _dose(p: Proxies, population: np.ndarray) -> Level:
    """DOSE shares for matched provinces; the share of unmatched regions goes to the unmatched provinces by gridded GDP."""

    def weights(idx, code, c):
        if p.dose is None:
            return None
        d = p.dose[idx]
        matched = np.isfinite(d)
        if not matched.any() or not d[matched].sum() > 0:
            return None
        rest = p.dose_rest.get(code, 0.0)
        w = np.zeros(len(idx))
        others = idx[~matched]
        if rest > 0 and len(others):
            sub = p.gdp[others] if p.gdp is not None and p.gdp[others].sum() > 0 else population[others]
            w[matched] = (1 - rest) * d[matched] / d[matched].sum()
            w[~matched] = rest * sub / sub.sum()
        else:
            w[matched] = d[matched]
        return w, ~matched, np.where(matched, LEVELS.index("dose"), LEVELS.index("dose_rest"))

    return weights


def _chains(p: Proxies, population: np.ndarray) -> list[list[Level]]:
    gdp, people_ = _plain("gdp", p.gdp, True), _plain("population", population, True)
    chains: list[list[Level]] = [[_plain("crops", p.crops, False), people_]]
    chains.append([_plain("eia", p.eia, False), _gem(p), people_])
    chains.append([_mining(p), people_])
    chains += [[_dose(p, population), gdp, people_] for _ in MANUFACTURING]
    chains += [[_plain("population", population, False)] for _ in LOCAL]
    return chains


def allocate(country_io: Mapping[str, CountryIO], provinces: Provinces, proxies: Proxies) -> Downscaled:
    """VA by province and good with its imputed flag and proxy level (see the module notes)."""
    country = np.asarray(provinces.country)
    n = len(country)
    population = np.asarray(provinces.population, np.float64)
    eligible = np.asarray(provinces.working_age) >= MIN_WORKING_AGE
    va = np.zeros((n, N_GOODS))
    imputed = np.zeros((n, N_GOODS), bool)
    level = np.full((n, N_GOODS), LEVELS.index("population"), np.uint8)
    chains = _chains(proxies, population)
    codes = set(country.tolist())
    for code in sorted(codes):
        c = country_io.get(code)
        if c is None:
            continue
        here = np.flatnonzero(country == code)
        imputed[here] = c.va_basis != "icio"
        idx = here[eligible[here]]
        if not len(idx):
            k = here[np.argmax(np.asarray(provinces.working_age)[here])]
            va[k], imputed[k], level[k] = c.va, True, LEVELS.index("largest")
            continue
        for g, chain in enumerate(chains):
            w, imp, lvl = next(r for r in (f(idx, code, c) for f in chain) if r is not None)
            va[idx, g] = c.va[g] * w / w.sum()
            imputed[idx, g] |= imp
            level[idx, g] = lvl
    dropped = sorted(k for k, c in country_io.items() if k not in codes and c.va.sum() > 0)
    return Downscaled(country, va, imputed, level, dropped)


def province_va(country_io: Mapping[str, CountryIO], provinces: Provinces, proxies: Proxies) -> tuple[np.ndarray, np.ndarray]:
    """VA[P, 10] (million USD) and imputed[P, 10]; national VA by country and good is kept exactly."""
    out = allocate(country_io, provinces, proxies)
    return out.va, out.imputed


def coverage(out: Downscaled) -> list[dict[str, float]]:
    """Per good: share of the world's VA of that good placed by each proxy level."""
    result = []
    for g in range(N_GOODS):
        total = out.va[:, g].sum()
        by = np.bincount(out.level[:, g], out.va[:, g], len(LEVELS))
        result.append({LEVELS[k]: float(v / total) for k, v in enumerate(by) if v > 0} if total > 0 else {})
    return result


def report(out: Downscaled, match: DoseMatch) -> str:
    lines = ["Proxy coverage (share of world VA by good):"]
    for g, shares in enumerate(coverage(out)):
        lines.append(f"  {NAMES[g]}: " + ", ".join(f"{k} {v:.1%}" for k, v in sorted(shares.items(), key=lambda x: -x[1])))
    lines.append(
        f"DOSE regions matched: {match.matched} of {match.regions} ({match.rate:.1%}); "
        f"{match.gva_share:.1%} of their manufacturing GVA"
    )
    if out.dropped:
        lines.append(f"Countries with VA but no province: {', '.join(out.dropped)}")
    return "\n".join(lines)


# DOSE


@dataclass
class Dose:
    values: dict[str, float]  # manufacturing GVA (local currency) by region name
    coverage: float  # population of these regions over the country's (WDI), at most 1


def read_dose(
    path: Path, population: Mapping[str, float] | None = None, year: int = YEAR, window: int = people.WINDOW
) -> dict[str, Dose]:
    """DOSE manufacturing GVA by region for each ISO country, of the year nearest `year` within `window` years.

    DOSE lists only some regions of some countries (Ethiopia 2000: Oromia alone); `coverage` is their people over
    the national `population` (2000) and is 1 where that is unknown.
    """
    values: dict[str, dict[int, dict[str, float]]] = defaultdict(lambda: defaultdict(lambda: defaultdict(float)))
    people_: dict[str, dict[int, float]] = defaultdict(lambda: defaultdict(float))
    with path.open(encoding="utf-8") as f:
        for r in csv.DictReader(f):
            if r["man_grp_pc_lcu"] and r["pop"]:
                values[r["GID_0"]][int(r["year"])][r["region"]] += float(r["man_grp_pc_lcu"]) * float(r["pop"])
                people_[r["GID_0"]][int(r["year"])] += float(r["pop"])
    out = {}
    for code, years in values.items():
        found = people.nearest_year({y: y for y in years}, year, window)
        if found is not None:
            total = (population or {}).get(code)
            coverage = min(people_[code][int(found)] / total, 1.0) if total else 1.0
            out[code] = Dose(dict(years[int(found)]), coverage)
    return out


def _key(name: str) -> str:
    return unicodedata.normalize("NFC", name).casefold().strip()


def _base(province_id: str) -> str:
    """Natural Earth adm1_code of a province, without the ".k" of a split region's parts."""
    head, _, tail = province_id.rpartition(".")
    return head if head and tail.isdigit() else province_id


def province_names(admin1: Path) -> dict[str, list[str]]:
    """Natural Earth names of each admin-1 region (adm1_code), primary name first."""
    _, t = pyogrio.read_arrow(admin1, columns=["adm1_code", *NAME_FIELDS], read_geometry=False)
    out = {}
    for r in t.to_pylist():
        names = [n.strip() for f in NAME_FIELDS for n in (r[f] or "").split("|") if n.strip()]
        out[r["adm1_code"]] = list(dict.fromkeys(names))
    return out


def match_dose(
    dose: Mapping[str, Dose],
    ids: Sequence[str],
    country: Sequence[str],
    names: Mapping[str, Sequence[str]],
    weight: np.ndarray,
    iso: Mapping[str, str],
) -> tuple[np.ndarray, dict[str, float], DoseMatch]:
    """Province DOSE proxy (NaN where no region matches), the unmatched share by country, and the match counts.

    A DOSE region matches the one Natural Earth region of its country (`iso`: pack code to ISO) whose primary
    name equals it, ignoring case; else the one with any name equal to it. A name shared by two regions matches
    neither, and a region takes one DOSE region at most. The parts of a split region share its value by `weight`.
    The unmatched share counts the regions DOSE lacks at their population share (`Dose.coverage`).
    """
    proxy = np.full(len(ids), np.nan)
    bases: dict[str, dict[str, list[int]]] = defaultdict(lambda: defaultdict(list))
    for i, (pid, c) in enumerate(zip(ids, country)):
        bases[iso.get(c, c)][_base(pid)].append(i)
    rest: dict[str, float] = {}
    by_country: dict[str, tuple[int, int]] = {}
    matched_gva = total_gva = 0.0
    matched = regions = 0
    pack_code = {iso.get(c, c): c for c in country}
    for code in sorted(dose):
        values = dose[code].values
        regions += len(values)
        total_gva += sum(values.values())
        if code not in bases:
            by_country[code] = (0, len(values))
            continue
        own = bases[code]
        found: dict[str, str] = {}
        for primary in (True, False):
            for region in sorted(values):
                if region in found:
                    continue
                key = _key(region)
                hits = [b for b in own if key in {_key(n) for n in (names.get(b, [])[:1] if primary else names.get(b, []))}]
                if len(hits) == 1 and hits[0] not in found.values():
                    found[region] = hits[0]
        for region, b in found.items():
            members = own[b]
            w = weight[members]
            proxy[members] = values[region] * (w / w.sum() if w.sum() > 0 else 1 / len(members))
        got = sum(values[r] for r in found)
        total = sum(values.values())
        rest[pack_code[code]] = 1 - dose[code].coverage * got / total if total > 0 else 0.0
        matched += len(found)
        matched_gva += got
        by_country[code] = (len(found), len(values))
    share = matched_gva / total_gva if total_gva > 0 else 0.0
    return proxy, rest, DoseMatch(matched, regions, share, by_country)


# GEM and mineral sites


def _number(v) -> float | None:
    if isinstance(v, float | int):
        return float(v)
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


class Asset(NamedTuple):
    lat: float
    lon: float
    weight: float
    dated: bool = True  # a start (or discovery, or opening) year is known
    offshore: bool = False
    country: str = ""  # as GEM writes it


def _located(rows: list[tuple[str, float | None, float | None, float | None, bool, bool]]) -> list[Asset]:
    """Assets with valid coordinates from (country, lat, lon, weight, dated, offshore) rows.

    A missing weight is 0 in a country where any asset has a figure; where none has, every asset takes the median
    positive figure (equal weights within the country).
    """
    weighted = {country for country, _, _, w, *_ in rows if w is not None}
    known = [w for _, _, _, w, *_ in rows if w is not None and w > 0]
    fill = statistics.median(known) if known else 1.0
    out = []
    for country, lat, lon, w, dated, offshore in rows:
        if lat is None or lon is None:
            continue
        if abs(lat) > 90 >= abs(lon):
            lat, lon = lon, lat  # some GEM rows have latitude and longitude swapped
        if abs(lat) > 90 or abs(lon) > 180:
            continue
        if w is None:
            w = 0.0 if country in weighted else fill
        out.append(Asset(lat, lon, w, dated, offshore, country))
    return out


def read_gem_coal(path: Path) -> list[Asset]:
    """GEM coal mines open in 2000 (Mt per year): non-closed mines operating or mothballed and opened by 2000 (or of
    unknown opening), and closed mines that closed in 2000 or later. Weight is reported production as given (0 stays
    0), capacity only where production is missing (`_located` fills the rest)."""
    rows = []
    for sheet in ("Non-closed mines", "Closed mines"):
        for r in people._xlsx_sheet(path, sheet):
            opening, closing = _number(r.get("Opening Year")), _number(r.get("Closing Year"))
            if opening is not None and opening > YEAR:
                continue
            if sheet == "Closed mines" and (closing is None or closing < YEAR):
                continue
            if sheet != "Closed mines" and r.get("Status") not in COAL_OPEN:
                continue
            production = _number(r.get("Production (Mtpa)"))
            weight = production if production is not None else _number(r.get("Capacity (Mtpa)"))
            lat, lon = _number(r.get("Latitude")), _number(r.get("Longitude"))
            rows.append((str(r.get("Country / Area")), lat, lon, weight, opening is not None, False))
    return _located(rows)


def read_gem_oil_gas(path: Path) -> list[Asset]:
    """GEM oil and gas fields that may have produced in 2000 (million boe per year).

    A field counts when its status shows it has produced and its production start (else discovery) year is 2000
    or earlier, or unknown (`dated` False). Weight is its latest reported production, 0 included: the largest
    liquids and the largest gas figure of that year, gas at `M3_PER_BOE`, or a boe figure (`_located` fills fields
    with none).
    """
    latest: dict[str, float] = {}
    by_unit: dict[str, dict[str, float]] = defaultdict(dict)
    for r in people._xlsx_sheet(path, "Field-level production data"):
        uid, year, q = r.get("Unit ID"), _number(r.get("Data Year")), _number(r.get("Quantity (converted)"))
        if uid is None or year is None or q is None:
            continue
        if year > latest.get(uid, -1):
            latest[uid], by_unit[uid] = year, {}
        if year == latest[uid]:
            units = str(r.get("Units (converted)"))
            by_unit[uid][units] = max(by_unit[uid].get(units, 0.0), q)
    rows = []
    for r in people._xlsx_sheet(path, "Field-level main data"):
        if r.get("Status") not in FIELD_PRODUCED:
            continue
        start = _number(r.get("Production start year"))
        start = start if start is not None else _number(r.get("Discovery year"))
        if start is not None and start > YEAR:
            continue
        q = by_unit.get(r.get("Unit ID"))
        if q is None:
            weight = None
        elif "million boe/y" in q:
            weight = q["million boe/y"]
        else:
            weight = q.get("million bbl/y", 0.0) + q.get("million m³/y", 0.0) / M3_PER_BOE
        lat, lon = _number(r.get("Latitude")), _number(r.get("Longitude"))
        offshore = r.get("Onshore/Offshore") == "offshore"
        rows.append((str(r.get("Country/Area")), lat, lon, weight, start is not None, offshore))
    return _located(rows)


def point_sums(points: Sequence[Sequence[float]], cells: pa.Table, n: int) -> np.ndarray:
    """Weights of points (lat, lon, weight, ...) summed by the province owning their cell; points with no owner are dropped."""
    owner = people.CellOwner(cells)
    out = np.zeros(n)
    for lat, lon, w, *_ in points:
        k = owner.of_point(lat, lon)
        if k >= 0:
            out[k] += w
    return out


# EIA


def read_seds(path: Path, year: int = YEAR) -> dict[str, float]:
    """EIA SEDS crude oil plus marketed natural gas production in `year` (billion Btu) by state code, with the
    federal offshore areas X3 (Gulf of Mexico) and X5 (Pacific); the US total is left out."""
    out: dict[str, float] = defaultdict(float)
    with path.open(encoding="utf-8-sig") as f:
        for r in csv.DictReader(f):
            if r["MSN"] in SEDS_SERIES and r["Year"] == str(year) and r["StateCode"] != "US" and r["Data"]:
                out[r["StateCode"]] += float(r["Data"])
    return dict(out)


def us_states(admin1: Path) -> dict[str, str]:
    """Natural Earth adm1_code of each United States region to its postal state code (from ISO 3166-2)."""
    _, t = pyogrio.read_arrow(admin1, columns=["adm1_code", "adm0_a3", "iso_3166_2"], read_geometry=False)
    return {
        r["adm1_code"]: r["iso_3166_2"].removeprefix("US-")
        for r in t.to_pylist()
        if r["adm0_a3"] == "USA" and (r["iso_3166_2"] or "").startswith("US-")
    }


def eia_oil_gas(
    seds: Mapping[str, float],
    provinces: pa.Table,
    states: Mapping[str, str],
    cells: pa.Table,
    population: np.ndarray,
    assets: Sequence[Asset] | None,
) -> np.ndarray:
    """US province weights for crude oil and gas from EIA 2000 state production, combined by heat content (Btu).

    A state's own production is spread over its provinces by the GEM weight of the onshore fields in them, else by
    population. Federal offshore production (`OFFSHORE`) goes to the coastal provinces of the adjacent states: each
    US offshore GEM field in the area adds its weight to the nearest of them; without such fields, by population.
    """
    n = provinces.num_rows
    country = provinces.column("country").to_pylist()
    state = np.array(
        [states.get(_base(i), "") if c == "USA" else "" for i, c in zip(provinces.column("id").to_pylist(), country)]
    )
    coastal = provinces.column("coastal").to_numpy(zero_copy_only=False)
    lon, lat = provinces.column("lon").to_numpy(), provinces.column("lat").to_numpy()
    us = [a for a in assets or () if a.country == "United States"]
    onshore = point_sums([a for a in us if not a.offshore], cells, n)
    out = np.zeros(n)
    for code, value in seds.items():
        idx = np.flatnonzero(state == code)
        if code in OFFSHORE or value <= 0 or not len(idx):
            continue
        w = onshore[idx] if onshore[idx].sum() > 0 else population[idx]
        out[idx] += value * w / w.sum()
    for area, coast in OFFSHORE.items():
        idx = np.flatnonzero(np.isin(state, coast) & coastal)
        if seds.get(area, 0.0) <= 0 or not len(idx):
            continue
        w = np.zeros(len(idx))
        for a in us:
            # Gulf of Mexico east of 100 W; the Pacific area off California south of 50 N (Alaska is a state's own).
            if a.offshore and (a.lon > -100 if area == "X3" else a.lon <= -100 and a.lat < 50):
                d = (lat[idx] - a.lat) ** 2 + ((lon[idx] - a.lon) * np.cos(np.radians(a.lat))) ** 2
                w[np.argmin(d)] += a.weight
        w = w if w.sum() > 0 else population[idx]
        out[idx] += seds[area] * w / w.sum()
    return out


def gridded_gdp(path: Path, cells: pa.Table) -> np.ndarray:
    """Kummu et al. gridded GDP (PPP, constant 2011 international dollars) of 2000 per province."""
    return people.province_population(path, cells)


def mineral_sites(provinces: pa.Table) -> np.ndarray:
    """P1 producing and past-producing mineral sites per province, all resource groups."""
    return sum(provinces.column(f"res_{g}").to_numpy().astype(np.float64) for g in RESOURCES)


def cropland(spam: Path, cells: pa.Table) -> np.ndarray:
    """MapSPAM physical area summed over its 21 crops (all technologies, `_A` rasters), per province."""
    with zipfile.ZipFile(spam) as z:
        members = sorted(n for n in z.namelist() if n.endswith("_A.tif"))
    total = None
    for m in members:
        with rasterio.open(f"zip://{spam}!{m}") as ds:
            a = ds.read(1).astype(np.float64)
            profile = ds.profile
        a[a < 0] = 0.0
        total = a if total is None else total + a
    profile.update(dtype="float64", nodata=None, count=1, compress=None)
    with MemoryFile() as mem:
        with mem.open(**profile) as ds:
            ds.write(total, 1)
        return people.province_population(mem.name, cells)


def load_proxies(
    cache: Path,
    provinces: pa.Table,
    cells: pa.Table,
    population: np.ndarray,
    national: Mapping[str, float],
    gem: tuple[Path, Path] | None,
) -> tuple[Proxies, DoseMatch]:
    """Every proxy from the fetch cache and the P1 tables; `national` is the 2000 population by ISO code (WDI),
    `gem` is (coal mines, oil and gas fields) or None.

    Gridded GDP is used when its file is in the cache (it is a manual, optional download).
    """
    n = provinces.num_rows
    crops = cropland(cache / SPAM, cells)
    gdp = gridded_gdp(cache / KUMMU, cells) if (cache / KUMMU).exists() else None
    oil = read_gem_oil_gas(gem[1]) if gem else None
    iso, _ = people.read_codes(cache / ADMIN0)
    ids, country = provinces.column("id").to_pylist(), provinces.column("country").to_pylist()
    split = gdp if gdp is not None else population
    dose, rest, match = match_dose(read_dose(cache / DOSE, national), ids, country, province_names(cache / ADMIN1), split, iso)
    proxies = Proxies(
        crops=crops,
        oil_gas=point_sums(oil, cells, n) if oil is not None else None,
        oil_gas_undated=point_sums([a for a in oil if not a.dated], cells, n) if oil is not None else None,
        eia=eia_oil_gas(read_seds(cache / SEDS), provinces, us_states(cache / ADMIN1), cells, population, oil),
        coal=point_sums(read_gem_coal(gem[0]), cells, n) if gem else None,
        sites=mineral_sites(provinces),
        dose=dose,
        dose_rest=rest,
        gdp=gdp,
    )
    return proxies, match
