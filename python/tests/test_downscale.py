import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

import h3.api.basic_int as h3
import numpy as np
import pyarrow as pa
import pytest

from ceteris.packs import fetch
from ceteris.packs.econ import downscale, icio, people
from ceteris.packs.econ.downscale import Provinces, Proxies
from ceteris.packs.econ.goods import N_GOODS
from ceteris.packs.econ.icio import CountryIO

# Hand-made world: AAA has four provinces (the second below 1,000 working-age people), BBB three, CCC one tiny one.
COUNTRY = ["AAA"] * 4 + ["BBB"] * 3 + ["CCC"]
WORKING_AGE = np.array([5000.0, 500.0, 20000.0, 8000.0, 3000.0, 4000.0, 1500.0, 300.0])
POPULATION = WORKING_AGE * 1.5
AAA, BBB, CCC = np.arange(4), np.arange(4, 7), np.array([7])


def _io(va: np.ndarray, basis: str = "icio", coal_share: float = 0.4) -> CountryIO:
    n = len(va)
    return CountryIO(va * 2, va, np.zeros((n, n)), np.full(n, 1 / n), va_basis=basis, coal_share=coal_share)


def _world(seed: int = 3) -> dict[str, CountryIO]:
    rng = np.random.default_rng(seed)
    return {
        "AAA": _io(rng.uniform(10, 1e6, N_GOODS)),
        "BBB": _io(rng.uniform(10, 1e4, N_GOODS), basis="wdi"),
        "CCC": _io(rng.uniform(1, 5, N_GOODS)),
    }


def _provinces() -> Provinces:
    return Provinces(COUNTRY, POPULATION, WORKING_AGE)


def _proxies(**over) -> Proxies:
    rng = np.random.default_rng(7)
    base = {
        "crops": rng.uniform(0, 10, 8),
        "oil_gas": np.array([0, 3, 0, 1, 0, 0, 0, 0.0]),
        "coal": np.array([2, 0, 0, 6, 0, 1, 1, 0.0]),
        "sites": np.array([1, 5, 3, 0, 0, 0, 0, 0.0]),
        "dose": np.array([4, np.nan, 2, 2, np.nan, np.nan, np.nan, np.nan]),
        "dose_rest": {"AAA": 0.0},
        "gdp": rng.uniform(1, 10, 8),
    }
    return Proxies(**{**base, **over})


def _shares(x: np.ndarray, idx: np.ndarray) -> np.ndarray:
    return x[idx] / x[idx].sum()


def test_downscaling_preserves_national_va():
    world = _world()
    for proxies in (_proxies(), Proxies(), _proxies(coal=None, dose=None, gdp=None, oil_gas=None)):
        va, imputed = downscale.province_va(world, _provinces(), proxies)
        assert va.shape == imputed.shape == (8, N_GOODS)
        assert (va >= 0).all()
        for code, idx in (("AAA", AAA), ("BBB", BBB), ("CCC", CCC)):
            np.testing.assert_allclose(va[idx].sum(axis=0), world[code].va, rtol=1e-9, atol=1e-9, err_msg=code)


def test_empty_provinces_get_no_va():
    va, _ = downscale.province_va(_world(), _provinces(), _proxies())
    assert (va[1] == 0).all()
    # Its share goes to the other provinces by the same proxy: crops for agriculture, population for services.
    eligible = np.array([0, 2, 3])
    np.testing.assert_allclose(va[eligible, 0] / va[eligible, 0].sum(), _shares(_proxies().crops, eligible), rtol=1e-12)
    np.testing.assert_allclose(va[eligible, 9] / va[eligible, 9].sum(), _shares(POPULATION, eligible), rtol=1e-12)


def test_a_country_without_any_eligible_province_puts_its_va_in_its_largest():
    world = _world()
    out = downscale.allocate(world, _provinces(), _proxies())
    np.testing.assert_allclose(out.va[7], world["CCC"].va, rtol=1e-12)
    assert out.imputed[7].all()
    assert {downscale.LEVELS[k] for k in out.level[7]} == {"largest"}


def test_proxy_levels_follow_the_spec_order():
    out = downscale.allocate(_world(), _provinces(), _proxies())
    name = np.vectorize(lambda k: downscale.LEVELS[k])(out.level)
    assert set(name[AAA[[0, 2, 3]], 0]) == {"crops"}
    assert set(name[AAA[[0, 2, 3]], 1]) == {"gem"}  # province 1 holds oil but is too small
    assert set(name[AAA[[0, 2, 3]], 3]) == {"dose"}
    assert set(name[AAA[[0, 2, 3]], 7]) == set(name[AAA[[0, 2, 3]], 9]) == {"population"}
    # BBB has no oil fields and no DOSE regions.
    # Only manufacturing falls back to gridded GDP; oil and gas go straight to population.
    assert set(name[BBB, 4]) == {"gdp"} and set(name[BBB, 1]) == {"population"}
    assert set(name[BBB, 2]) == {"mining"}


def test_fallbacks_are_flagged_imputed_and_data_levels_are_not():
    out = downscale.allocate(_world(), _provinces(), _proxies())
    eligible = AAA[[0, 2, 3]]
    assert not out.imputed[eligible].any()  # ICIO country, every good from its first proxy
    assert out.imputed[BBB].all()  # national VA is imputed (wdi basis)
    world = _world()
    world["BBB"] = _io(world["BBB"].va)
    out = downscale.allocate(world, _provinces(), _proxies())
    assert out.imputed[BBB][:, [1, 3, 4, 5, 6]].all()  # population stands in for GEM, gridded GDP for DOSE
    assert not out.imputed[BBB][:, [0, 2, 7, 8, 9]].any()


def test_without_crops_agriculture_follows_population():
    out = downscale.allocate(_world(), _provinces(), _proxies(crops=np.zeros(8)))
    eligible = AAA[[0, 2, 3]]
    np.testing.assert_allclose(_shares(out.va[:, 0], eligible), _shares(POPULATION, eligible), rtol=1e-12)
    assert out.imputed[eligible, 0].all()


def test_oil_gas_and_mining_fall_back_to_population_not_gridded_gdp():
    eligible = AAA[[0, 2, 3]]
    out = downscale.allocate(_world(), _provinces(), _proxies(oil_gas=None, coal=None, sites=None))
    for g in (1, 2):
        np.testing.assert_allclose(_shares(out.va[:, g], eligible), _shares(POPULATION, eligible), rtol=1e-12)
        assert out.imputed[eligible, g].all()


def test_eia_state_production_comes_before_gem():
    eia = np.array([0, 0, 1.0, 3.0, 0, 0, 0, 0])
    out = downscale.allocate(_world(), _provinces(), _proxies(eia=eia))
    eligible = AAA[[0, 2, 3]]
    np.testing.assert_allclose(out.va[eligible, 1] / out.va[AAA, 1].sum(), [0, 0.25, 0.75], rtol=1e-12)
    assert {downscale.LEVELS[k] for k in out.level[eligible, 1]} == {"eia"} and not out.imputed[eligible, 1].any()
    # BBB has no EIA data: its own chain (no fields there, so population).
    assert {downscale.LEVELS[k] for k in out.level[BBB, 1]} == {"population"}


def test_gem_oil_and_gas_is_imputed_where_undated_fields_carry_most_weight():
    oil = _proxies().oil_gas  # AAA eligible weight: 1 (province 3)
    out = downscale.allocate(_world(), _provinces(), _proxies(oil_gas_undated=np.array([0, 3, 0, 0.5, 0, 0, 0, 0])))
    assert not out.imputed[AAA[[0, 2, 3]], 1].any()
    out = downscale.allocate(_world(), _provinces(), _proxies(oil_gas_undated=oil * 0.6))
    assert out.imputed[AAA[[0, 2, 3]], 1].all()
    assert {downscale.LEVELS[k] for k in out.level[AAA[[0, 2, 3]], 1]} == {"gem"}


def test_mining_weights_coal_mines_and_mineral_sites_by_the_coal_share():
    p = _proxies()
    out = downscale.allocate(_world(), _provinces(), p)
    eligible = AAA[[0, 2, 3]]
    expected = 0.4 * _shares(p.coal, eligible) + 0.6 * _shares(p.sites, eligible)
    np.testing.assert_allclose(_shares(out.va[:, 2], eligible), expected, rtol=1e-12)
    assert not out.imputed[eligible, 2].any()


def test_without_gem_mining_follows_mineral_sites_flagged_imputed():
    p = _proxies(coal=None)
    out = downscale.allocate(_world(), _provinces(), p)
    eligible = AAA[[0, 2, 3]]
    np.testing.assert_allclose(_shares(out.va[:, 2], eligible), _shares(p.sites, eligible), rtol=1e-12)
    assert out.imputed[eligible, 2].all()
    assert {downscale.LEVELS[k] for k in out.level[eligible, 2]} == {"sites"}


def test_dose_regions_without_a_province_leave_their_share_to_the_unmatched_by_gridded_gdp():
    p = _proxies(dose=np.array([6, np.nan, 2, np.nan] + [np.nan] * 4), dose_rest={"AAA": 0.25})
    out = downscale.allocate(_world(), _provinces(), p)
    total = out.va[AAA, 3].sum()
    # Matched provinces 0 and 2 split 75% by DOSE (6:2); unmatched province 3 takes the other 25% alone.
    np.testing.assert_allclose(out.va[[0, 2, 3], 3] / total, [0.5625, 0.1875, 0.25], rtol=1e-12)
    assert not out.imputed[[0, 2], 3].any() and out.imputed[3, 3]
    assert downscale.LEVELS[out.level[3, 3]] == "dose_rest"


def test_local_goods_split_by_population():
    out = downscale.allocate(_world(), _provinces(), _proxies())
    for g in (7, 8, 9):
        np.testing.assert_allclose(_shares(out.va[:, g], BBB), _shares(POPULATION, BBB), rtol=1e-12)


def test_coverage_shares_sum_to_one_per_good():
    out = downscale.allocate(_world(), _provinces(), _proxies())
    cover = downscale.coverage(out)
    for g in range(N_GOODS):
        assert sum(cover[g].values()) == pytest.approx(1.0, abs=1e-12)
    assert cover[9] == {
        "population": pytest.approx(1 - out.va[7, 9] / out.va[:, 9].sum()),
        "largest": pytest.approx(out.va[7, 9] / out.va[:, 9].sum()),
    }


# DOSE


DOSE_HEADER = "country,region,GID_0,GID_1,year,grp_lcu,pop,grp_pc_lcu,ag_grp_pc_lcu,man_grp_pc_lcu,serv_grp_pc_lcu\n"


def test_read_dose_takes_manufacturing_gva_of_the_nearest_year_and_its_coverage(tmp_path):
    path = tmp_path / "dose.csv"
    rows = [
        "A,R1,AAA,AAA.1_1,2000,1,10,1,,2,",
        "A,R2,AAA,AAA.2_1,2000,1,10,1,,3,",
        "A,R3,AAA,AAA.3_1,2000,1,10,1,,,",  # no manufacturing value: not a DOSE region of 2000
        "A,R1,AAA,AAA.1_1,2002,1,10,1,,4,",
        "A,R2,AAA,AAA.2_1,2002,1,20,1,,5,",
        "A,R3,AAA,AAA.3_1,2002,1,5,1,,6,",
        "B,Q1,BBB,BBB.1_1,2010,1,10,1,,2,",  # outside the five-year window
        "C,S1,CCC,CCC.1_1,1999,1,10,1,,2,",
    ]
    path.write_text(DOSE_HEADER + "\n".join(rows) + "\n")
    got = downscale.read_dose(path, {"AAA": 40.0})
    assert got == {"AAA": downscale.Dose({"R1": 20.0, "R2": 30.0}, 0.5), "CCC": downscale.Dose({"S1": 20.0}, 1.0)}


def test_dose_match_rate_reported(capsys):
    dose = {
        "AAA": downscale.Dose({"Rio de Janeiro": 30.0, "Nowhere": 10.0, "Ostland": 60.0}, 1.0),
        "BBB": downscale.Dose({"X": 1.0}, 1.0),
    }
    # Provinces: two parts of one Natural Earth region (".1", ".2"), one matched by an alternate name.
    ids = ["AAA-1.1", "AAA-1.2", "AAA-2", "AAA-3", "CCC-1"]
    names = {"AAA-1": ["Rio De Janeiro"], "AAA-2": ["Westland", "Ostland"], "AAA-3": ["Southland"], "CCC-1": ["X"]}
    country = ["AAA", "AAA", "AAA", "AAA", "CCC"]
    weight = np.array([1.0, 3.0, 1.0, 1.0, 1.0])
    proxy, rest, match = downscale.match_dose(dose, ids, country, names, weight, iso={})
    np.testing.assert_allclose(proxy[:4], [7.5, 22.5, 60.0, np.nan])
    assert np.isnan(proxy[4])
    assert rest == {"AAA": pytest.approx(0.1)}
    assert (match.matched, match.regions) == (2, 4) and match.rate == pytest.approx(0.5)
    # BBB has no provinces; AAA's matched regions hold 90% of its DOSE manufacturing.
    assert match.gva_share == pytest.approx(90 / 101)
    print(downscale.report(downscale.allocate(_world(), _provinces(), _proxies()), match))
    out = capsys.readouterr().out
    assert "DOSE regions matched: 2 of 4 (50.0%)" in out
    # Regions DOSE does not list count at their population share: 80% coverage leaves 1 - 0.8 x 0.9 unplaced.
    dose["AAA"].coverage = 0.8
    _, rest, _ = downscale.match_dose(dose, ids, country, names, weight, iso={})
    assert rest == {"AAA": pytest.approx(0.28)}


def test_dose_names_must_be_unique_within_the_country():
    dose = {"AAA": downscale.Dose({"Twin": 1.0}, 1.0)}
    names = {"AAA-1": ["Twin"], "AAA-2": ["Twin"]}
    proxy, rest, match = downscale.match_dose(dose, ["AAA-1", "AAA-2"], ["AAA", "AAA"], names, np.ones(2), iso={})
    assert np.isnan(proxy).all() and match.matched == 0 and rest == {"AAA": 1.0}


# GEM


def _xlsx(path: Path, sheets: dict[str, list[dict]]) -> Path:
    main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"

    def column(i: int) -> str:
        return chr(ord("A") + i) if i < 26 else "A" + chr(ord("A") + i - 26)

    def cell(ref: str, v) -> str:
        if isinstance(v, str):
            return f'<c r="{ref}" t="str"><v>{escape(v)}</v></c>'
        return f'<c r="{ref}"><v>{v}</v></c>'

    with zipfile.ZipFile(path, "w") as z:
        entries = "".join(f'<sheet name="{n}" sheetId="{i}" r:id="rId{i}"/>' for i, n in enumerate(sheets, 1))
        z.writestr("xl/workbook.xml", f'<workbook xmlns="{main}" xmlns:r="{rel}"><sheets>{entries}</sheets></workbook>')
        rels = "".join(f'<Relationship Id="rId{i}" Target="worksheets/sheet{i}.xml"/>' for i in range(1, len(sheets) + 1))
        z.writestr(
            "xl/_rels/workbook.xml.rels",
            f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>',
        )
        z.writestr("xl/sharedStrings.xml", f'<sst xmlns="{main}"></sst>')
        for i, rows in enumerate(sheets.values(), 1):
            header = list(dict.fromkeys(k for r in rows for k in r))
            lines = ["".join(cell(f"{column(j)}1", h) for j, h in enumerate(header))]
            for n, r in enumerate(rows, 2):
                lines.append("".join(cell(f"{column(j)}{n}", r[h]) for j, h in enumerate(header) if h in r))
            data = "".join(f'<row r="{n}">{x}</row>' for n, x in enumerate(lines, 1))
            z.writestr(f"xl/worksheets/sheet{i}.xml", f'<worksheet xmlns="{main}"><sheetData>{data}</sheetData></worksheet>')
    return path


def test_gem_coal_keeps_mines_open_in_2000_weighted_by_output(tmp_path):
    def mine(name, status, opening=None, production=None, capacity=None, lat=10.0, lon=20.0, country="Aland"):
        r = {"Mine Name": name, "Country / Area": country, "Status": status, "Latitude": lat, "Longitude": lon}
        for k, v in (("Opening Year", opening), ("Production (Mtpa)", production), ("Capacity (Mtpa)", capacity)):
            if v is not None:
                r[k] = v
        return r

    open_ = [
        mine("old", "Operating", 1980, production=4.0, capacity=9.0),
        mine("cap", "Mothballed", 1995, capacity=2.0),
        mine("undated", "Operating", production=1.0),
        mine("guess", "Operating", 1990),  # no figure while others in its country have one: 0
        mine("idle", "Operating", 1990, production=0.0, capacity=5.0),  # reported 0 stays 0
        mine("b1", "Operating", 1990, country="Bland"),  # Bland has no figures: the median positive figure
        mine("b2", "Operating", 1990, country="Bland"),
        mine("new", "Operating", 2005, production=50.0),
        mine("plan", "Proposed", 1990, production=50.0),
    ]
    closed = [
        {"Mine Name": "late", "Country / Area": "Aland", "Opening Year": 1950.0, "Closing Year": 2004.0,
         "Production (Mtpa)": 3.0, "Latitude": 144.0, "Longitude": -38.0},  # GEM swapped its coordinates
        {"Mine Name": "early", "Country / Area": "Aland", "Closing Year": 1999.0, "Production (Mtpa)": 3.0,
         "Latitude": 1.0, "Longitude": 1.0},
        {"Mine Name": "unknown", "Country / Area": "Aland", "Production (Mtpa)": 3.0, "Latitude": 1.0, "Longitude": 1.0},
    ]  # fmt: skip
    path = _xlsx(tmp_path / "coal.xlsx", {"README": [{"x": "y"}], "Non-closed mines": open_, "Closed mines": closed})
    got = downscale.read_gem_coal(path)
    assert (got[-1].lat, got[-1].lon) == (-38.0, 144.0)
    assert sorted(a.weight for a in got if a.country == "Aland") == [0.0, 0.0, 1.0, 2.0, 3.0, 4.0]
    assert [a.weight for a in got if a.country == "Bland"] == [2.5, 2.5]
    assert [a.dated for a in got if a.country == "Aland"] == [True, True, False, True, True, True]


def test_gem_oil_and_gas_keeps_producing_fields_weighted_by_boe(tmp_path):
    def field(uid, status, start="", discovery="", lat="1.0", lon="2.0", country="Aland"):
        return {"Unit ID": uid, "Country/Area": country, "Status": status, "Production start year": start,
                "Discovery year": discovery, "Latitude": lat, "Longitude": lon}  # fmt: skip

    fields = [
        field("A", "operating", 1970.0),
        field("B", "operating", "", 1985.0),
        field("C", "abandoned", 1960.0),
        field("D", "operating", 2010.0),
        field("E", "discovered", 1990.0),
        field("F", "operating", 1990.0, lat=""),
        field("G", "mothballed"),
        field("H", "operating", 1990.0),
        field("S", "operating", 1990.0, country="Bland") | {"Onshore/Offshore": "offshore"},
        field("T", "operating", country="Bland"),
    ]

    def prod(uid, quantity, units, year=2023.0):
        return {"Unit ID": uid, "Quantity (converted)": quantity, "Units (converted)": units, "Data Year": year}

    production = [
        prod("A", 10.0, "million bbl/y"),
        prod("A", 8.0, "million bbl/y"),  # same year, another liquid: the larger counts
        prod("A", 1642.0, "million m³/y"),
        prod("A", 99.0, "million bbl/y", 2015.0),  # an older year is ignored
        prod("B", 3.0, "million boe/y"),
        prod("C", 5.0, "million bbl/y"),
        prod("H", 0.0, "million bbl/y"),  # latest production 0 stays 0
    ]
    path = _xlsx(tmp_path / "goget.xlsx", {"Field-level main data": fields, "Field-level production data": production})
    got = downscale.read_gem_oil_gas(path)
    gas = 1642.0 / downscale.M3_PER_BOE
    # A: 10 bbl + gas; B: 3; C: 5; G: no data while Aland has figures, 0; H: 0.
    weights = sorted(round(a.weight, 6) for a in got if a.country == "Aland")
    assert weights == sorted(round(x, 6) for x in (10 + gas, 3.0, 5.0, 0.0, 0.0))
    # Bland has no figures at all: each field takes the median positive figure (A, B, C: 3, 5, 10 + gas), 5.
    bland = [a for a in got if a.country == "Bland"]
    assert [a.weight for a in bland] == [5.0, 5.0] and [a.offshore for a in bland] == [True, False]
    assert [a.dated for a in bland] == [True, False]
    assert {a.dated for a in got if a.country == "Aland"} == {True, False}  # G has no year


def _cells(points: list[tuple[float, float]]) -> pa.Table:
    cells = [h3.latlng_to_cell(lat, lon, 4) for lat, lon in points]
    return pa.table({"cell": pa.array(cells, pa.uint64()), "province": pa.array(range(len(cells)), pa.uint32())})


def test_points_go_to_the_province_owning_their_cell():
    cells = _cells([(10.0, 20.0), (-30.0, 140.0)])
    got = downscale.point_sums([(10.0, 20.0, 2.0), (-30.0, 140.0, 1.0), (10.0, 20.0, 0.5), (60.0, -40.0, 9.0)], cells, 2)
    np.testing.assert_allclose(got, [2.5, 1.0])


SEDS_HEADER = "Data_Status,MSN,StateCode,Year,Data\n"


def test_read_seds_sums_crude_and_marketed_gas_by_state(tmp_path):
    path = tmp_path / "seds.csv"
    rows = [
        '"2024F","PAPRB","TX","2000",10',
        '"2024F","NGMPB","TX","2000",30',
        '"2024F","PAPRB","TX","2001",99',
        '"2024F","PAPRP","TX","2000",99',  # thousand barrels: not the Btu series
        '"2024F","PAPRB","X3","2000",50',
        '"2024F","PAPRB","US","2000",90',
        '"2024F","NGMPB","VT","2000",',
    ]
    path.write_text(SEDS_HEADER + "\n".join(rows) + "\n", encoding="utf-8-sig")
    assert downscale.read_seds(path) == {"TX": 40.0, "X3": 50.0}


def test_eia_spreads_states_by_onshore_fields_and_offshore_over_the_nearest_coastal_province():
    # Texas in two parts (the second coastal), Louisiana coastal, Kansas inland, and one Canadian province.
    points = [(31.0, -101.0), (28.5, -96.5), (30.5, -92.0), (38.5, -98.0), (53.0, -113.0)]
    cells = _cells(points)
    table = pa.table(
        {
            "id": ["USA-1.1", "USA-1.2", "USA-2", "USA-3", "CAN-1"],
            "country": ["USA", "USA", "USA", "USA", "CAN"],
            "coastal": [False, True, True, False, False],
            "lat": [p[0] for p in points],
            "lon": [p[1] for p in points],
        }
    )
    states = {"USA-1": "TX", "USA-2": "LA", "USA-3": "KS"}
    seds = {"TX": 40.0, "LA": 10.0, "KS": 6.0, "X3": 100.0}
    assets = [
        downscale.Asset(31.0, -101.0, 3.0, country="United States"),
        downscale.Asset(28.5, -96.5, 1.0, country="United States"),
        downscale.Asset(28.0, -91.0, 9.0, offshore=True, country="United States"),  # off Louisiana
        downscale.Asset(27.5, -96.0, 1.0, offshore=True, country="United States"),  # off Texas
        downscale.Asset(53.0, -113.0, 50.0, country="Canada"),
    ]
    population = np.array([1.0, 1.0, 1.0, 4.0, 1.0])
    got = downscale.eia_oil_gas(seds, table, states, cells, population, assets)
    # TX 40 by onshore fields 3:1, plus 10 of offshore; LA 10 plus 90 of offshore; KS has no fields: by population.
    np.testing.assert_allclose(got, [30.0, 10.0 + 10.0, 10.0 + 90.0, 6.0, 0.0])
    without = downscale.eia_oil_gas(seds, table, states, cells, population, None)
    np.testing.assert_allclose(without, [20.0, 20.0 + 50.0, 10.0 + 50.0, 6.0, 0.0])


def test_gridded_gdp_sums_the_derived_geotiff_and_skips_nodata(tmp_path):
    import rasterio
    from rasterio.transform import from_origin

    path = tmp_path / "gdp.tif"
    with rasterio.open(path, "w", driver="GTiff", width=2, height=1, count=1, dtype="float32", crs="EPSG:4326",
                       nodata=-9.0, transform=from_origin(19.0, 11.0, 2.0, 2.0)) as ds:  # fmt: skip
        ds.write(np.array([[20.0, -9.0]], np.float32), 1)
    got = downscale.gridded_gdp(path, _cells([(10.0, 20.0)]))
    np.testing.assert_allclose(got, [20.0])


def test_mineral_sites_sum_the_resource_groups():
    t = pa.table({f"res_{g}": pa.array([1, 0, 2], pa.uint32()) for g in ("precious", "base", "ferrous", "energy", "industrial")})
    np.testing.assert_allclose(downscale.mineral_sites(t), [5, 0, 10])


def test_coal_share_is_coal_value_added_over_mining(tmp_path):
    from test_icio import _table

    text, parts = _table(industries=["A01", "B05", "B07", "B08", "C10T12", "G"])
    path = tmp_path / "2000_SML.csv"
    path.write_text(text)
    got = icio.aggregate(path)
    for k, code in enumerate(("AAA", "BBB")):
        va = parts["out"] - parts["z"].sum(axis=0)
        block = va[k * 6 : k * 6 + 6]
        assert got[code].coal_share == pytest.approx(block[1] / block[1:4].sum(), rel=1e-12)


# Tests below need the 2000 pack and the downloaded sources; CI is data-free.
ROOT = Path(__file__).resolve().parents[2]
PACK = ROOT / "data" / "packs" / "2000"
GEM = ("Global Coal Mine Tracker, August 2026.xlsx", "Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx")


def _need(path: Path, how: str) -> Path:
    if not path.exists():
        pytest.skip(f"skipping: no {path.name} in {path.parent}; {how}")
    return path


def _cached(name: str) -> Path:
    return _need(fetch.path_of(name), "fetch it with `uv run python -m ceteris.packs.fetch`")


def _gem() -> tuple[Path, Path]:
    for name in GEM:
        _need(fetch.path_of(name), "GEM trackers are saved by hand from the GEM download form")
    return tuple(fetch.path_of(n) for n in GEM)


def _pack(name: str):
    from pyarrow import ipc

    return ipc.open_file(str(_need(PACK / name, "build the pack with `uv run python -m ceteris.packs.build`"))).read_all()


@pytest.fixture(scope="module")
def real_gem():
    coal, oil = _gem()
    return downscale.read_gem_coal(coal), downscale.read_gem_oil_gas(oil)


def test_real_gem_trackers_have_the_main_producers(real_gem):
    coal, oil = real_gem
    assert len(coal) > 2000 and len(oil) > 4000
    assert all(-90 <= a.lat <= 90 and -180 <= a.lon <= 180 and a.weight >= 0 for a in coal + oil)


@pytest.fixture(scope="module")
def real_dose():
    population = {k: v[2000] for k, v in people.read_wdi(_cached("wdi_SP.POP.TOTL.zip")).items() if 2000 in v}
    return downscale.read_dose(_cached("DOSE_V2.14.csv"), population)


def test_real_dose_covers_large_economies_near_2000(real_dose):
    assert {"USA", "CHN", "IND", "BRA", "DEU", "JPN"} <= set(real_dose)
    assert len(real_dose["USA"].values) == 51 and real_dose["USA"].coverage > 0.99
    assert real_dose["ETH"].coverage < 0.5  # Oromia alone


@pytest.fixture(scope="module")
def real_world():
    from test_icio import NAMES

    for name in NAMES + ["1995-2000_SML.zip", "ne_10m_admin_1_states_provinces.zip", downscale.SPAM]:
        _cached(name)
    countries = _pack("countries.arrow").to_pylist()
    table = _pack("provinces.arrow")
    cells = _pack("cells.arrow")
    country = table["country"].to_pylist()
    admin0 = fetch.path_of("ne_10m_admin_0_countries.zip")
    iso, _ = people.read_codes(admin0)
    obs = people.load_observed(fetch.CACHE, admin0)
    codes = {r["code"]: r["sovereign"] for r in countries}
    raw = people.province_population(fetch.path_of("ppp_2000_1km_Aggregated.tif"), cells)
    worldpop: dict[str, float] = {}
    for c, p in zip(country, raw):
        worldpop[c] = worldpop.get(c, 0.0) + float(p)
    population = people.scale_to_national(raw, country, people.national_population(list(codes), obs, iso))
    labour = people.country_labour(codes, obs, iso)
    working_age = population * np.array([labour[c].working_age_share for c in country])
    world = icio.impute(codes, icio.aggregate(fetch.path_of("1995-2000_SML.zip")), icio.load_wdi(fetch.CACHE), obs, iso, worldpop)
    gem = tuple(fetch.path_of(n) for n in GEM)
    national = {k: v for k, v in obs.population.items()}
    proxies, match = downscale.load_proxies(
        fetch.CACHE, table, cells, population, national, gem if all(p.exists() for p in gem) else None
    )
    provinces = Provinces(country, population, working_age)
    return world, provinces, proxies, match, downscale.allocate(world, provinces, proxies)


def test_real_downscaling_preserves_national_va(real_world):
    world, provinces, _, _, out = real_world
    country = np.asarray(provinces.country)
    for code, c in world.items():
        here = country == code
        if here.any():
            np.testing.assert_allclose(out.va[here].sum(axis=0), c.va, rtol=1e-9, atol=1e-9, err_msg=code)


def test_real_provinces_under_1000_working_age_have_no_va_unless_alone(real_world):
    _, provinces, _, _, out = real_world
    small = provinces.working_age < downscale.MIN_WORKING_AGE
    alone = (out.level == downscale.LEVELS.index("largest")).all(axis=1)
    assert small.sum() > 100
    assert (out.va[small & ~alone] == 0).all()


def test_real_dose_match_rate_reported(real_world, capsys):
    *_, match, out = real_world
    assert 0.6 < match.rate <= 1.0
    print(downscale.report(out, match))
    assert "DOSE regions matched" in capsys.readouterr().out


def test_real_gem_places_oil_and_coal_where_they_are(real_world):
    _gem()
    _, provinces, proxies, _, out = real_world
    country = np.asarray(provinces.country)
    gem = downscale.LEVELS.index("gem")
    # Oil: Saudi oil VA is placed by GEM fields, and the Eastern Province leads.
    sau = np.flatnonzero(country == "SAU")
    assert (out.level[sau, 1] == gem).all()
    names = _pack("provinces.arrow")["name"].to_pylist()
    assert {names[i].split(" (")[0] for i in sau[out.va[sau, 1] > 0.05 * out.va[sau, 1].sum()]} == {"Ash Sharqiyah"}
    # Coal: Shanxi leads China and Wyoming the United States (production as reported, no median fill).
    for code, top in (("CHN", "Shanxi"), ("USA", "Wyoming")):
        here = np.flatnonzero(country == code)
        assert names[here[np.argmax(out.va[here, 2])]].split(" (")[0] == top
    # US oil and gas follows EIA 2000 state production: Texas and Louisiana (with the Gulf offshore) hold most.
    usa = np.flatnonzero(country == "USA")
    assert (out.level[usa, 1] == downscale.LEVELS.index("eia")).all()
    share = {}
    for i in usa:
        share[names[i].split(" (")[0]] = share.get(names[i].split(" (")[0], 0.0) + out.va[i, 1] / out.va[usa, 1].sum()
    assert share["Texas"] + share["Louisiana"] > 0.5 and share.get("Pennsylvania", 0.0) < 0.02
    assert proxies.coal is not None and proxies.oil_gas is not None
    cover = downscale.coverage(out)
    assert cover[1].get("gem", 0) + cover[1].get("eia", 0) > 0.8 and cover[2].get("mining", 0) > 0.8


def test_real_gridded_gdp_places_manufacturing_dose_leaves(real_world):
    _cached(downscale.KUMMU)
    *_, proxies, _, out = real_world
    assert proxies.gdp is not None
    # Kummu 2000, constant 2011 international dollars: about 62 trillion over the pack's provinces.
    assert 5e13 < proxies.gdp.sum() < 7.5e13
    cover = downscale.coverage(out)
    for g in downscale.MANUFACTURING:
        assert cover[g].get("gdp", 0) > 0.1 and cover[g].get("population", 0) < 0.01
    for g in (1, 2):
        assert "gdp" not in cover[g]
