"""Country input-output tables for the 10 P2 goods, from ICIO 2000 and imputed for the other economies.

`aggregate` reads one year of the OECD ICIO 2025 edition (small version) and sums its 50 industries into
the goods of `goods.ICIO_MAP`: output, value added, input coefficients and final demand shares per
economy. `impute` fills the pack countries outside ICIO from the nearest ICIO economy
(specs/econ/SPEC.md, Economies outside ICIO; ADR 0014).

Units are million current USD, as in ICIO. The 2025 edition splits coal (B05) from oil and gas (B06), so
no production split is needed; an earlier edition's combined B05_06 (`goods.SPLIT`) would need EIA 2000
production values at Pink Sheet prices and is not implemented.
"""

import csv
import zipfile
from collections import defaultdict
from collections.abc import Mapping
from dataclasses import dataclass, field, replace
from pathlib import Path

import numpy as np
import pyarrow.csv as pacsv

from . import goods, people
from .goods import ICIO_MAP, N_GOODS

YEAR = 2000
# Final demand columns that enter the shares. Direct purchases abroad by residents (DPABR) are bought
# from other economies' services and are left out with the rest of the cross-border residual.
FINAL_DEMAND = ("HFCE", "NPISH", "GGFC", "GFCF", "INVNT")
_SPECIAL_ROWS = ("TLS", "VA", "OUT")
# WDI sector groups as goods: agriculture; industry other than manufacturing (mining, utilities, construction);
# manufacturing; services.
GROUPS = ((0,), (1, 2, 7, 8), (3, 4, 5, 6), (9,))
BASES = ("icio", "wdi", "wdi_gdp", "donor", "parent", "none")
# Territories whose people and product WDI counts in the parent's totals, by pack code (territory -> parent).
INSIDE = {"SOL": "SOM", "ALD": "FIN", "CYN": "CYP"}
# World Bank regions for pack countries the World Bank does not classify (and whose sovereign it does not either).
REGIONS = {"SAH": "Middle East & North Africa", "CYN": "Europe & Central Asia", "SOL": "Sub-Saharan Africa"}
# Income files where WDI has none: ADB Key Indicators (Cook Islands) and the Maddison Project (North Korea).
ADB_INCOME = "adb_kidb_NY_GDP_PCAP_CD_COO.csv"
MADDISON = "mpd2023_web.xlsx"
ADB_ISO = {"COO": "COK"}  # ADB economy codes that differ from ISO 3166-1 alpha-3
# GDP per person is taken from the year nearest 2000 within these years.
INCOME_YEARS = range(1995, 2011)


@dataclass
class CountryIO:
    """One economy's 2000 structure by good. `io[k, g]` is units of k per unit of g (all source economies)."""

    output: np.ndarray  # million USD
    va: np.ndarray  # million USD; output less intermediate inputs, so it includes taxes less subsidies on products
    io: np.ndarray  # a_kg, columns sum to less than one
    final_demand_shares: np.ndarray  # sum to one
    donor: str | None = None  # ICIO economy an imputed country copies; None for ICIO economies
    # Where the VA level and split come from: icio; wdi (GDP and all four sector shares); wdi_gdp (WDI GDP times the
    # shares it has, the donor's split for the rest); donor (population times the donor's VA per person, scaled by
    # income when a source exists); parent (a territory's population share of its parent); none (no GDP, no people).
    va_basis: str = "icio"
    region: str | None = None  # World Bank region the donor was looked up in
    # Income the donor ranking and the donor-basis level scaling used: pwt, wdi (GDP per person, nearest year in 1995 to
    # 2010), adb (ADB Key Indicators GDP per person), maddison (Maddison Project GDP per person, 2011 international
    # dollars) or none (region median donor, donor's level unscaled).
    income_source: str = "none"
    shed: float = 0.0  # share of this economy's own VA given to territories counted inside it
    coal_share: float = 0.0  # coal (ICIO B05) in the VA of good 2, coal and other mining; an imputed country has its donor's


@dataclass
class Wdi:
    """WDI series by ISO code, year nearest 2000: GDP in current USD, sector value added as fractions of GDP."""

    gdp: dict[str, float] = field(default_factory=dict)
    income: dict[str, float] = field(default_factory=dict)  # GDP per person in current USD, nearest year in 1995 to 2010
    income_source: dict[str, str] = field(default_factory=dict)  # wdi, or adb where WDI has no value
    maddison: dict[str, float] = field(default_factory=dict)  # Maddison Project GDP per person, same years
    agriculture: dict[str, float] = field(default_factory=dict)
    industry: dict[str, float] = field(default_factory=dict)
    manufacturing: dict[str, float] = field(default_factory=dict)
    services: dict[str, float] = field(default_factory=dict)


def _open_table(path: Path, year: int):
    options = pacsv.ConvertOptions(null_values=["NA"], strings_can_be_null=False)
    if path.suffix == ".zip":
        with zipfile.ZipFile(path) as z:
            member = next(n for n in z.namelist() if n.endswith(f"{year}_SML.csv"))
            with z.open(member) as f:
                return pacsv.read_csv(f, convert_options=options)
    return pacsv.read_csv(path, convert_options=options)


def aggregate(path: Path, year: int = YEAR) -> dict[str, CountryIO]:
    """Economy code -> `CountryIO`, from an ICIO CSV (rows: industries, TLS, VA, OUT) or a package zip of yearly CSVs.

    Output is the OUT row (column totals); input coefficients divide the intermediate use of a good by the
    output of the buying good, summing the sellers of every source economy (imports included). Final demand
    shares sum the `FINAL_DEMAND` columns over all sellers. A good an economy does not produce has an empty
    coefficient column. A negative share (inventory changes can cause one) stops the build.

    Value added is output less intermediate inputs: ICIO's VA plus its taxes less subsidies on products (TLS),
    so that VA/output = 1 - sum of the column of `io` holds exactly. The coal share is B05's part of good 2's VA,
    which the downscaling uses to weigh coal mines against other mineral sites.
    """
    table = _open_table(path, year)
    labels = table.column(0).to_pylist()
    names = table.column_names[1:-1]
    industry_rows, row_good = [], []
    for i, label in enumerate(labels):
        if label not in _SPECIAL_ROWS:
            industry = label.split("_", 1)[1]
            if ICIO_MAP[industry] == goods.SPLIT:
                raise NotImplementedError(f"{industry} combines coal with oil and gas; use the 2025 edition")
            industry_rows.append(i)
            row_good.append(ICIO_MAP[industry])
    rows, row_good = np.array(industry_rows), np.array(row_good)
    out_row = labels.index("OUT")

    inputs: dict[str, np.ndarray] = {}
    output: dict[str, np.ndarray] = {}
    final: dict[str, np.ndarray] = {}
    coal: dict[str, float] = defaultdict(float)
    mining: dict[str, float] = defaultdict(float)
    for j, name in enumerate(names, start=1):
        column = table.column(j)
        if column.null_count:
            raise ValueError(f"{name} has NA cells")
        x = column.to_numpy().astype(np.float64)
        code, tail = name.split("_", 1)
        if tail in FINAL_DEMAND:
            final.setdefault(code, np.zeros(N_GOODS))
            final[code] += np.bincount(row_good, x[rows], N_GOODS)
        elif tail != "DPABR":
            g = ICIO_MAP[tail]
            inputs.setdefault(code, np.zeros((N_GOODS, N_GOODS)))
            output.setdefault(code, np.zeros(N_GOODS))
            inputs[code][:, g] += np.bincount(row_good, x[rows], N_GOODS)
            output[code][g] += x[out_row]
            if g == 2:
                va = x[out_row] - x[rows].sum()
                mining[code] += va
                coal[code] += va if tail == "B05" else 0.0

    result = {}
    for code, x in output.items():
        shares = final[code] / final[code].sum()
        if (shares < 0).any():
            raise ValueError(f"negative final demand share for {code}: {shares.round(4).tolist()}")
        io = np.divide(inputs[code], x, out=np.zeros((N_GOODS, N_GOODS)), where=x > 0)
        share = min(max(coal[code] / mining[code], 0.0), 1.0) if mining[code] > 0 else 0.0
        result[code] = CountryIO(x, x - inputs[code].sum(axis=0), io, shares, coal_share=share)
    return result


def exports(path: Path, year: int = YEAR) -> dict[str, np.ndarray]:
    """Economy code -> sales of its industries to every other economy by good (million USD): intermediate use and
    final demand of the foreign columns. Output less exports is what an economy absorbs of its own output."""
    table = _open_table(path, year)
    labels = table.column(0).to_pylist()
    rows = np.array([i for i, label in enumerate(labels) if label not in _SPECIAL_ROWS])
    seller = np.array([labels[i].split("_", 1)[0] for i in rows])
    good = np.array([ICIO_MAP[labels[i].split("_", 1)[1]] for i in rows])
    abroad = np.zeros(len(rows))
    for name in table.column_names[1:-1]:
        x = table.column(name).to_numpy().astype(np.float64)[rows]
        abroad += np.where(seller != name.split("_", 1)[0], x, 0.0)
    out = {}
    for code in sorted(set(seller.tolist())):
        mine = seller == code
        out[code] = np.bincount(good[mine], abroad[mine], N_GOODS)
    return out


def _nearest_income(series: Mapping[int, float]) -> float | None:
    """Value of the year nearest 2000 within 1995 to 2010 (the earlier year on a tie)."""
    near = [y for y in series if INCOME_YEARS.start <= y < INCOME_YEARS.stop]
    return series[min(near, key=lambda y: (abs(y - people.YEAR), y))] if near else None


def read_adb_income(path: Path) -> dict[str, dict[int, float]]:
    """ADB Key Indicators (SDMX CSV) GDP per capita in current USD: value by year for each ISO code."""
    out: dict[str, dict[int, float]] = defaultdict(dict)
    with path.open(newline="", encoding="utf-8-sig") as f:
        for r in csv.DictReader(f):
            if r["INDICATOR"] == "NY_GDP_PCAP_CD" and r["OBS_VALUE"].strip():
                out[ADB_ISO.get(r["ECONOMY_CODE"], r["ECONOMY_CODE"])][int(r["TIME_PERIOD"])] = float(r["OBS_VALUE"])
    return dict(out)


def read_maddison(path: Path) -> dict[str, dict[int, float]]:
    """Maddison Project Database GDP per capita (2011 international dollars): value by year for each ISO code."""
    out: dict[str, dict[int, float]] = defaultdict(dict)
    for r in people._xlsx_sheet(path, "Full data"):
        if isinstance(r.get("gdppc"), float):
            out[str(r["countrycode"])][int(r["year"])] = r["gdppc"]
    return dict(out)


def income_by_year(gdp: Mapping[str, Mapping[int, float]], population: Mapping[str, Mapping[int, float]]) -> dict[str, float]:
    """GDP per person (current USD) of the year nearest 2000 within 1995 to 2010, for each economy that has one."""
    out = {}
    for code, g in gdp.items():
        per_person = {y: v / population[code][y] for y, v in g.items() if y in population.get(code, {})}
        if (found := _nearest_income(per_person)) is not None:
            out[code] = found
    return out


def load_wdi(cache: Path) -> Wdi:
    """The WDI series imputation needs, from the fetch cache, with ADB and Maddison Project income where WDI has none."""

    def series(indicator: str, scale: float = 1.0) -> dict[str, float]:
        raw = people.read_wdi(cache / f"wdi_{indicator}.zip")
        found = {k: people.nearest_year(v) for k, v in raw.items()}
        return {k: v / scale for k, v in found.items() if v is not None}

    income = income_by_year(people.read_wdi(cache / "wdi_NY.GDP.MKTP.CD.zip"), people.read_wdi(cache / "wdi_SP.POP.TOTL.zip"))
    source = dict.fromkeys(income, "wdi")
    for code, v in read_adb_income(cache / ADB_INCOME).items():
        if code not in income and (found := _nearest_income(v)) is not None:
            income[code], source[code] = found, "adb"
    maddison = {k: found for k, v in read_maddison(cache / MADDISON).items() if (found := _nearest_income(v)) is not None}
    return Wdi(
        gdp=series("NY.GDP.MKTP.CD"),
        income=income,
        income_source=source,
        maddison=maddison,
        agriculture=series("NV.AGR.TOTL.ZS", 100),
        industry=series("NV.IND.TOTL.ZS", 100),
        manufacturing=series("NV.IND.MANF.ZS", 100),
        services=series("NV.SRV.TOTL.ZS", 100),
    )


def median_va_over_gdp(icio_by_code: Mapping[str, CountryIO], wdi: Wdi) -> float:
    """Median over the ICIO economies with WDI GDP of their VA (including product taxes) over GDP."""
    return float(np.median([c.va.sum() * 1e6 / wdi.gdp[k] for k, c in icio_by_code.items() if k in wdi.gdp]))


def _sector_shares(wdi: Wdi, ps: list[str]) -> tuple[float, dict[str, float | None]]:
    """GDP (USD) of the parts and their GDP-weighted VA shares of GDP: agriculture, industry, manufacturing, services.

    A share is None when any part with GDP lacks it. All-zero shares (WDI carries zeros for Venezuela in 2000)
    count as missing, and so does manufacturing above industry.
    """
    have = [p for p in ps if p in wdi.gdp]
    gdp = sum(wdi.gdp[p] for p in have)
    shares: dict[str, float | None] = {}
    for name in ("agriculture", "industry", "manufacturing", "services"):
        values = [getattr(wdi, name).get(p) for p in have]
        ok = bool(have) and None not in values
        shares[name] = float(np.average(values, weights=[wdi.gdp[p] for p in have])) if ok else None
    if sum(shares[k] or 0.0 for k in ("agriculture", "industry", "services")) <= 0:
        shares = dict.fromkeys(shares)
    if shares["manufacturing"] is not None and shares["industry"] is not None and shares["manufacturing"] > shares["industry"]:
        shares["manufacturing"] = None
    return gdp, shares


def _split(total: float, goods_: list[int], donor: CountryIO) -> np.ndarray:
    """`total` over the goods by the donor's VA proportions (a zero vector elsewhere)."""
    va = np.zeros(N_GOODS)
    va[goods_] = total * donor.va[goods_] / donor.va[goods_].sum()
    return va


def _value_added(
    donor: CountryIO,
    gdp: float,
    shares: dict[str, float | None],
    people_: float,
    donor_people: float,
    ratio: float,
    scale: float | None,
) -> tuple[np.ndarray, str]:
    """VA by good (million USD) and its basis, from what WDI and the population give for this country."""
    agr, ind, man, srv = (shares[k] for k in ("agriculture", "industry", "manufacturing", "services"))
    available = [x for x in (agr, ind, srv) if x is not None]
    if gdp > 0 and available:
        gdp = gdp / 1e6
        if len(available) == 3 and man is not None:
            va = sum(_split(gdp * t, list(g), donor) for t, g in zip((agr, ind - man, man, srv), GROUPS))
            return va, "wdi"
        if len(available) == 3:
            return _split(gdp * agr, [0], donor) + _split(gdp * ind, list(range(1, 9)), donor) + _split(
                gdp * srv, [9], donor
            ), "wdi_gdp"
        return _split(gdp * sum(available), list(range(N_GOODS)), donor), "wdi_gdp"
    if gdp > 0:
        # WDI has GDP but no usable share: the median VA over GDP of the ICIO economies, split as the donor's VA.
        return _split(gdp / 1e6 * ratio, list(range(N_GOODS)), donor), "wdi_gdp"
    if people_ > 0:
        per_person = donor.va.sum() / donor_people * (1.0 if scale is None else scale)
        return _split(people_ * per_person, list(range(N_GOODS)), donor), "donor"
    return np.zeros(N_GOODS), "none"


def impute(
    countries: Mapping[str, str],
    icio_by_code: Mapping[str, CountryIO],
    wdi: Wdi,
    obs: people.Observed,
    iso: Mapping[str, str],
    worldpop: Mapping[str, float] | None = None,
    parts: Mapping[str, tuple[str, ...]] = people.PARTS,
    inside: Mapping[str, str] = INSIDE,
    regions: Mapping[str, str] = REGIONS,
) -> dict[str, CountryIO]:
    """`CountryIO` for each pack country (code -> sovereign code); ICIO economies keep their own.

    Another country copies the input coefficients and final demand shares of the ICIO economy in its World
    Bank region (`regions` overrides it) with the closest log income, in PWT GDP per person when it has one,
    else WDI GDP per person (ADB where WDI has none), else Maddison Project GDP per person, compared with the
    donors' in the same source; with no income at all, the region's median. Only ICIO economies with WDI GDP and population can donate.

    VA by good: WDI GDP times the sector shares (agriculture, industry, manufacturing, services), each group
    split over its goods by the donor's VA (`va_basis` wdi); with manufacturing missing, industry is split over
    goods 1 to 8; with another share missing, GDP times the shares there are, split as the donor's VA (wdi_gdp);
    with no share at all, GDP times the ICIO median VA over GDP (wdi_gdp); with no GDP, population (WDI, else the `worldpop` national total) times the donor's VA per
    person, scaled by own over donor income where both are known (donor). Output follows at the donor's
    output per unit of VA, which keeps VA + inputs = output. A territory in `inside` (counted in its parent's
    WDI totals) takes its population share of the parent's VA and output out of the parent's (parent).
    """
    worldpop = worldpop or {}
    eligible = {c: v for c, v in icio_by_code.items() if c in wdi.gdp and c in obs.population}
    by_wdi = people.Observed(region=obs.region, gdp_per_capita=wdi.income)
    by_maddison = people.Observed(region=obs.region, gdp_per_capita=wdi.maddison)
    ratio = median_va_over_gdp(icio_by_code, wdi)
    out = {}
    for code, sovereign in countries.items():
        own = iso.get(code, code)
        if own in icio_by_code:
            out[code] = icio_by_code[own]
        elif code not in inside:
            ps = list(parts.get(code, (own,)))
            parents = list(parts.get(sovereign, (iso.get(sovereign, sovereign),)))
            region = regions.get(code) or next((obs.region[p] for p in ps + parents if p in obs.region), None)
            pwt = [obs.gdp_per_capita[p] for p in ps if p in obs.gdp_per_capita]
            income = [wdi.income[p] for p in ps if p in wdi.income]
            ppp = [wdi.maddison[p] for p in ps if p in wdi.maddison]
            if pwt:
                ranking, level, source = obs, float(np.mean(pwt)), "pwt"
            elif income:
                adb = all(wdi.income_source[p] == "adb" for p in ps if p in wdi.income)
                ranking, level, source = by_wdi, float(np.mean(income)), "adb" if adb else "wdi"
            elif ppp:
                ranking, level, source = by_maddison, float(np.mean(ppp)), "maddison"
            else:
                ranking, level, source = obs, None, "none"
            name = people.donor(region, None if level is None else float(np.log(level)), eligible, ranking, ps)
            gdp, shares = _sector_shares(wdi, ps)
            population = sum(obs.population.get(p, 0.0) for p in ps) or worldpop.get(code, 0.0)
            scale = None if level is None else level / ranking.gdp_per_capita[name]
            va, basis = _value_added(eligible[name], gdp, shares, population, obs.population[name], ratio, scale)
            d = eligible[name]
            per_va = np.divide(d.output, d.va, out=np.zeros(N_GOODS), where=d.va > 0)
            out[code] = CountryIO(
                va * per_va,
                va,
                d.io.copy(),
                d.final_demand_shares.copy(),
                donor=name,
                va_basis=basis,
                region=region,
                income_source=source,
                coal_share=d.coal_share,
            )
    share_of: dict[str, float] = defaultdict(float)
    original = dict(out)
    for code, parent in inside.items():
        if code in countries and parent in original:
            share = worldpop.get(code, 0.0) / sum(
                obs.population.get(p, 0.0) for p in parts.get(parent, (iso.get(parent, parent),))
            )
            base = original[parent]
            share_of[parent] += share
            out[code] = replace(
                _scaled(base, share),
                donor=base.donor or iso.get(parent, parent),
                va_basis="parent",
                region=regions.get(code, base.region),
            )
    for parent, share in share_of.items():
        out[parent] = replace(_scaled(original[parent], 1 - share), shed=share)
    return out


def _scaled(c: CountryIO, f: float) -> CountryIO:
    return replace(c, output=c.output * f, va=c.va * f, io=c.io.copy(), final_demand_shares=c.final_demand_shares.copy())
