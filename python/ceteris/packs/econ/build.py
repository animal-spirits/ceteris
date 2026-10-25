"""The 2000 economic tables of pack format 3 (specs/econ/SPEC.md, Pack additions), built after the P1 tables.

    uv run python -m ceteris.packs.econ.build [--pack data/packs/2000] [--out DIR]

Reads the P1 tables of `--pack` and the fetch cache, and writes the tables of `TABLES` into `--out` (the pack by
default). The full pack build (`python -m ceteris.packs`) calls `build` itself.
"""

import argparse
import json
from pathlib import Path

import numpy as np
import pyarrow as pa
from pyarrow import ipc

from .. import build as pack_build
from .. import fetch
from . import downscale, gateways, goods, icio, people, tariffs

TABLES = (
    "econ_goods.arrow",
    "econ_countries.arrow",
    "econ_io.arrow",
    "econ_provinces.arrow",
    "econ_pops.arrow",
    "econ_tariffs.arrow",
    "gateways.arrow",
)
ICIO = "1995-2000_SML.zip"
BACI = "BACI_HS92_V202601.zip"
HS_KEY = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx"
WORLDPOP = "ppp_2000_1km_Aggregated.tif"
GEM = ("Global Coal Mine Tracker, August 2026.xlsx", "Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx")
# P02 inventory target coverage s_g in months, the prior's start (trade SPEC, Parameters; TradeParams::coverage).
COVERAGE = (3.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0)


def _read(path: Path) -> pa.Table:
    return ipc.open_file(str(path)).read_all()


def _f64(x) -> pa.Array:
    return pa.array(np.asarray(x, np.float64), pa.float64())


def build(pack: Path, out: Path, cache: Path = fetch.CACHE) -> dict:
    """Write the econ tables of `pack` (its P1 tables) into `out`; returns their sha256 by name and a stats dict."""
    provinces = _read(pack / "provinces.arrow")
    cells = _read(pack / "cells.arrow")
    nodes, edges = _read(pack / "nodes.arrow"), _read(pack / "edges.arrow")
    countries = _read(pack / "countries.arrow")
    codes = countries.column("code").to_pylist()
    sovereign = dict(zip(codes, countries.column("sovereign").to_pylist()))
    country = provinces.column("country").to_pylist()
    admin0 = cache / downscale.ADMIN0

    iso, _ = people.read_codes(admin0)
    obs = people.load_observed(cache, admin0)
    labour = people.country_labour(sovereign, obs, iso)
    raw = people.province_population(cache / WORLDPOP, cells)
    worldpop: dict[str, float] = {}
    for c, p in zip(country, raw):
        worldpop[c] = worldpop.get(c, 0.0) + float(p)
    population = people.scale_to_national(raw, country, people.national_population(codes, obs, iso))
    working_age = population * np.array([labour[c].working_age_share for c in country])

    wdi = icio.load_wdi(cache, admin0)
    world = icio.impute(sovereign, icio.aggregate(cache / ICIO), wdi, obs, iso, worldpop)
    gem = tuple(cache / n for n in GEM)
    proxies, match = downscale.load_proxies(
        cache, provinces, cells, population, dict(obs.population), gem if all(p.exists() for p in gem) else None
    )
    placed = downscale.allocate(world, downscale.Provinces(country, population, working_age), proxies)

    weight = goods.weights_from_baci(cache / BACI, cache / HS_KEY)
    trade = gateways.trade_by_country(cache / BACI, iso, codes)
    agreements = tariffs.pack_pairs(
        tariffs.read_cepii(cache / tariffs.GRAVITY) | tariffs.read_dta(cache / tariffs.DTA), codes, iso
    )
    rates = tariffs.table(tariffs.read_rates(cache), agreements, codes, iso, obs, sovereign)
    gw = gateways.select(nodes, edges, provinces, trade)

    out.mkdir(parents=True, exist_ok=True)
    tables = {
        "econ_goods.arrow": {
            "name": pa.array(goods.NAMES),
            "traded": pa.array([g in goods.TRADED for g in range(goods.N_GOODS)], pa.bool_()),
            "weight": _f64(weight),
            "coverage": _f64(COVERAGE),
        },
        "econ_countries.arrow": country_columns(codes, world, labour, wdi, iso),
        "econ_io.arrow": io_columns(codes, world),
        "econ_provinces.arrow": province_columns(country, population, labour, placed),
        "econ_pops.arrow": dict(zip(*_columns(people.pops(country, population, labour)))),
        "econ_tariffs.arrow": tariffs.columns(rates),
        "gateways.arrow": dict(zip(*_columns(gw))),
    }
    hashes = {name: pack_build.write_table(out / name, cols) for name, cols in tables.items()}
    assert tuple(hashes) == TABLES
    stats = {
        "gateways": gw.num_rows,
        "gateway_kinds": dict(zip(gateways.KINDS, np.bincount(gw.column("kind").to_numpy(), minlength=3).tolist())),
        "tariff_importers_own": int((~rates.imputed).sum()),
        "tariff_agreement_pairs": int(rates.agreement.sum()) // 2,
        "dose_match_rate": round(match.rate, 4),
        "provinces_with_va": int((placed.va.sum(axis=1) > 0).sum()),
        "va_imputed_share": round(float(placed.va[placed.imputed].sum() / placed.va.sum()), 4),
    }
    print(downscale.report(placed, match))
    print(tariffs.report(rates, codes, trade))
    print(gateways.report(gw, codes))
    return {"tables": hashes, "stats": stats}


def _columns(t: pa.Table) -> tuple[list[str], list[pa.Array]]:
    return t.column_names, [c.combine_chunks() for c in t.columns]


def country_columns(
    codes: list[str], world: dict[str, icio.CountryIO], labour: dict[str, people.Labour], wdi: icio.Wdi, iso: dict[str, str]
) -> dict[str, pa.Array]:
    """`econ_countries.arrow`: one row per pack country in code order. VA is million USD per year."""
    rows = [world[c] for c in codes]
    gdp, gdp_imputed = [], []
    for c, r in zip(codes, rows):
        parts = [wdi.gdp[p] for p in people.PARTS.get(c, (iso.get(c, c),)) if p in wdi.gdp]
        own = bool(parts) and r.va_basis != "parent"
        gdp.append(sum(parts) if own else r.va.sum() * 1e6)
        gdp_imputed.append(not own)
    cols = {
        "code": pa.array(codes),
        "donor": pa.array([r.donor or "" for r in rows]),
        "va_basis": pa.array([r.va_basis for r in rows]),
        "income_source": pa.array([r.income_source for r in rows]),
        "region": pa.array([r.region or "" for r in rows]),
        "va_imputed": pa.array([r.va_basis != "icio" for r in rows], pa.bool_()),
        "gdp": _f64(gdp),
        "gdp_imputed": pa.array(gdp_imputed, pa.bool_()),
        "coal_share": _f64([r.coal_share for r in rows]),
        **{f"fd_{g}": _f64([r.final_demand_shares[g] for r in rows]) for g in range(goods.N_GOODS)},
        **{f"va_{g}": _f64([r.va[g] for r in rows]) for g in range(goods.N_GOODS)},
    }
    return cols | people.country_columns(codes, labour)


def io_columns(codes: list[str], world: dict[str, icio.CountryIO]) -> dict[str, pa.Array]:
    """`econ_io.arrow`: a_kg for every country, input k and good g, country-major, g innermost."""
    n, ng = len(codes), goods.N_GOODS
    a = np.stack([world[c].io for c in codes])
    c, k, g = np.meshgrid(np.arange(n), np.arange(ng), np.arange(ng), indexing="ij")
    return {
        "country": pa.array(c.ravel().astype(np.uint32)),
        "input": pa.array(k.ravel().astype(np.uint8)),
        "good": pa.array(g.ravel().astype(np.uint8)),
        "a": _f64(a.ravel()),
    }


def province_columns(
    country: list[str], population: np.ndarray, labour: dict[str, people.Labour], placed: downscale.Downscaled
) -> dict[str, pa.Array]:
    """`econ_provinces.arrow`: population, working age, and VA (million USD per year), its flag and proxy level by good."""
    cols = people.province_columns(country, population, labour)
    for g in range(goods.N_GOODS):
        cols[f"va_{g}"] = _f64(placed.va[:, g])
    for g in range(goods.N_GOODS):
        cols[f"va_imputed_{g}"] = pa.array(placed.imputed[:, g], pa.bool_())
    for g in range(goods.N_GOODS):
        cols[f"va_level_{g}"] = pa.array(placed.level[:, g], pa.uint8())
    return cols


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pack", type=Path, default=pack_build.ROOT / "data" / "packs" / "2000")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    result = build(args.pack, args.out or args.pack)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
