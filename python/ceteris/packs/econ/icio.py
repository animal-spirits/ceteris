"""Country input-output tables for the 10 P2 goods, from ICIO 2000 and imputed for the other economies.

`aggregate` reads one year of the OECD ICIO 2025 edition (small version) and sums its 50 industries into
the goods of `goods.ICIO_MAP`: output, value added, input coefficients and final demand shares per
economy. `impute` fills the pack countries outside ICIO from the nearest ICIO economy
(specs/econ/SPEC.md, Economies outside ICIO; ADR 0014).

Units are million current USD, as in ICIO. The 2025 edition splits coal (B05) from oil and gas (B06), so
no production split is needed; an earlier edition's combined B05_06 (`goods.SPLIT`) would need EIA 2000
production values at Pink Sheet prices and is not implemented.
"""

import zipfile
from collections.abc import Mapping
from dataclasses import dataclass, field
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
BASES = ("icio", "wdi", "wdi_gdp", "donor", "none")


@dataclass
class CountryIO:
    """One economy's 2000 structure by good. `io[k, g]` is units of k per unit of g (all source economies)."""

    output: np.ndarray  # million USD
    va: np.ndarray  # value added at basic prices, million USD
    io: np.ndarray  # a_kg, columns sum to less than one
    final_demand_shares: np.ndarray  # sum to one
    donor: str | None = None  # ICIO economy an imputed country copies; None for ICIO economies
    # Where the VA level and split come from: icio; wdi (GDP and all four sector shares); wdi_gdp (WDI GDP, the
    # donor's split); donor (population times the donor's VA per person); none (no GDP and no people).
    va_basis: str = "icio"


@dataclass
class Wdi:
    """WDI series by ISO code, year nearest 2000: GDP in current USD, sector value added as fractions of GDP."""

    gdp: dict[str, float] = field(default_factory=dict)
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
    va_row, out_row = labels.index("VA"), labels.index("OUT")

    inputs: dict[str, np.ndarray] = {}
    output: dict[str, np.ndarray] = {}
    value_added: dict[str, np.ndarray] = {}
    final: dict[str, np.ndarray] = {}
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
            for store in (inputs, output, value_added):
                store.setdefault(code, np.zeros((N_GOODS, N_GOODS) if store is inputs else N_GOODS))
            inputs[code][:, g] += np.bincount(row_good, x[rows], N_GOODS)
            output[code][g] += x[out_row]
            value_added[code][g] += x[va_row]

    result = {}
    for code, x in output.items():
        shares = final[code] / final[code].sum()
        if (shares < 0).any():
            raise ValueError(f"negative final demand share for {code}: {shares.round(4).tolist()}")
        io = np.divide(inputs[code], x, out=np.zeros((N_GOODS, N_GOODS)), where=x > 0)
        result[code] = CountryIO(x, value_added[code], io, shares)
    return result


def load_wdi(cache: Path) -> Wdi:
    """The WDI series imputation needs, from the fetch cache."""

    def series(indicator: str, scale: float = 1.0) -> dict[str, float]:
        raw = people.read_wdi(cache / f"wdi_{indicator}.zip")
        found = {k: people.nearest_year(v) for k, v in raw.items()}
        return {k: v / scale for k, v in found.items() if v is not None}

    return Wdi(
        gdp=series("NY.GDP.MKTP.CD"),
        agriculture=series("NV.AGR.TOTL.ZS", 100),
        industry=series("NV.IND.TOTL.ZS", 100),
        manufacturing=series("NV.IND.MANF.ZS", 100),
        services=series("NV.SRV.TOTL.ZS", 100),
    )


def _sector_shares(wdi: Wdi, ps: list[str]) -> tuple[float, np.ndarray | None]:
    """GDP (USD) of the parts and the GDP-weighted VA shares of agriculture, other industry, manufacturing, services.

    The shares are None unless every part reports all four, industry covers manufacturing and they are not all
    zero (WDI carries zeros for Venezuela in 2000).
    """
    have = [p for p in ps if p in wdi.gdp]
    gdp = sum(wdi.gdp[p] for p in have)
    rows = []
    for p in have:
        s = [wdi.agriculture.get(p), wdi.industry.get(p), wdi.manufacturing.get(p), wdi.services.get(p)]
        if None in s or s[1] < s[2] or s[0] + s[1] + s[3] <= 0:
            return gdp, None
        rows.append([s[0], s[1] - s[2], s[2], s[3]])
    if not rows:
        return gdp, None
    return gdp, np.average(rows, axis=0, weights=[wdi.gdp[p] for p in have])


def _value_added(donor: CountryIO, groups: np.ndarray) -> np.ndarray:
    """VA by good: the group totals split within each group by the donor's proportions."""
    va = np.zeros(N_GOODS)
    for total, group in zip(groups, GROUPS):
        idx = list(group)
        va[idx] = total * donor.va[idx] / donor.va[idx].sum()
    return va


def impute(
    countries: Mapping[str, str],
    icio_by_code: Mapping[str, CountryIO],
    wdi: Wdi,
    obs: people.Observed,
    iso: Mapping[str, str],
    parts: Mapping[str, tuple[str, ...]] = people.PARTS,
) -> dict[str, CountryIO]:
    """`CountryIO` for each pack country (code -> sovereign code); ICIO economies keep their own.

    Another country copies the input coefficients and final demand shares of the ICIO economy in its World
    Bank region with the closest log GDP per person (any region when it has none). Only economies with
    WDI GDP and population can donate. Value added by good comes from WDI GDP and sector shares (agriculture,
    industry, manufacturing, services) with each group split over its goods by the donor's VA, and output
    follows at the donor's output per unit of VA. The `va_basis` field records what was missing:
    sector shares (the donor's split at the donor's VA over GDP), then GDP (population times the donor's VA
    per person), then people (no value added).
    """
    eligible = {c: v for c, v in icio_by_code.items() if c in wdi.gdp and c in obs.population}
    out = {}
    for code, sovereign in countries.items():
        own = iso.get(code, code)
        if own in icio_by_code:
            out[code] = icio_by_code[own]
            continue
        ps = list(parts.get(code, (own,)))
        parents = list(parts.get(sovereign, (iso.get(sovereign, sovereign),)))
        region = next((obs.region[p] for p in ps + parents if p in obs.region), None)
        incomes = [obs.gdp_per_capita[p] for p in ps if p in obs.gdp_per_capita]
        log_income = float(np.log(np.mean(incomes))) if incomes else None
        name = people.donor(region, log_income, eligible, obs, ps)
        d = eligible[name]
        gdp, shares = _sector_shares(wdi, ps)
        population = sum(obs.population.get(p, 0.0) for p in ps)
        if shares is not None:
            va, basis = _value_added(d, shares * gdp / 1e6), "wdi"
        elif gdp > 0:
            total = gdp / 1e6 * d.va.sum() / (wdi.gdp[name] / 1e6)
            va, basis = d.va * total / d.va.sum(), "wdi_gdp"
        elif population > 0:
            total = population * d.va.sum() / obs.population[name]
            va, basis = d.va * total / d.va.sum(), "donor"
        else:
            va, basis = np.zeros(N_GOODS), "none"
        per_va = np.divide(d.output, d.va, out=np.zeros(N_GOODS), where=d.va > 0)
        out[code] = CountryIO(va * per_va, va, d.io.copy(), d.final_demand_shares.copy(), donor=name, va_basis=basis)
    return out
