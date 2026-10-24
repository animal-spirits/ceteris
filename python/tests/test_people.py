import zipfile
from pathlib import Path

import h3.api.basic_int as h3
import numpy as np
import pyarrow as pa
import pytest
import rasterio
from pyarrow import ipc
from rasterio.transform import from_origin

from ceteris.packs import fetch
from ceteris.packs.econ import people
from ceteris.packs.econ.people import Labour, Observed

ROOT = Path(__file__).resolve().parents[2]
PACK = ROOT / "data" / "packs" / "2000"
# WorldPop marks nodata with the largest float32, which is positive.
NODATA = float(np.finfo(np.float32).max)


def _csv_zip(path: Path, member: str, text: str) -> Path:
    with zipfile.ZipFile(path, "w") as z:
        z.writestr(member, text)
    return path


def test_nearest_year_prefers_the_closest_and_the_earlier_on_a_tie():
    assert people.nearest_year({1996: 1.0, 1999: 2.0, 2003: 3.0}) == 2.0
    assert people.nearest_year({1999: 1.0, 2001: 2.0}) == 1.0
    assert people.nearest_year({1995: 7.0}) == 7.0 and people.nearest_year({2005: 8.0}) == 8.0
    assert people.nearest_year({1994: 1.0, 2006: 2.0}) is None
    assert people.nearest_year({}) is None


def test_read_ilostat_keeps_both_sexes_and_the_asked_age_class_as_fractions(tmp_path):
    path = tmp_path / "ilo.csv"
    path.write_text(
        '"ref_area","source","indicator","sex","classif1","time","obs_value","obs_status"\n'
        '"AFG","XA:1","EAP","SEX_T","AGE_YTHADULT_YGE15","2000",46.5,\n'
        '"AFG","XA:1","EAP","SEX_T","AGE_YTHADULT_Y15-64","2000",47.5,\n'
        '"AFG","XA:1","EAP","SEX_M","AGE_YTHADULT_Y15-64","2000",80.0,\n'
        '"ALB","XA:1","EAP","SEX_T","AGE_YTHADULT_Y15-64","2000",,\n',
        encoding="utf-8-sig",
    )
    assert people.read_ilostat(path, "AGE_YTHADULT_Y15-64") == {"AFG": pytest.approx(0.475)}


def test_read_wic_groups_working_ages_into_three_levels(tmp_path):
    rows = ['"Time","sex","edu","agest","pop","region","source"']
    for sex in "fm":
        for edu, level_pop in zip(("e1", "e2", "e3", "e4", "e5", "e6"), (1, 2, 3, 4, 5, 6)):
            for age in (10, 15, 60, 65):
                rows.append(f'2000,"{sex}","{edu}",{age},{level_pop},"reg4","Back Projection"')
            rows.append(f'1995,"{sex}","{edu}",15,100,"reg4","Back Projection"')
    path = tmp_path / "wic.csv"
    path.write_text("\n".join(rows) + "\n")
    shares = people.read_wic(path, {4: "AFG"})["AFG"]
    # Ages 15 and 60 only, both sexes: 2 sexes x 2 ages x (1+2+3, 4+5, 6).
    assert shares == pytest.approx(np.array([6, 9, 6]) / 21)
    assert people.read_wic(path, {5: "XXX"}) == {}


def test_read_barro_lee_uses_the_total_population_of_2000(tmp_path):
    path = tmp_path / "bl.csv"
    path.write_text(
        "BLcode,country,year,sex,agefrom,ageto,lu,lp,lpc,ls,lsc,lh,lhc,yr_sch,yr_sch_pri,yr_sch_sec,yr_sch_ter,WBcode,region_code,pop\n"
        '1,"Romania",2000,"MF",15,64,10,30,5,50,9,10,4,9,5,3,1,"ROM","Europe",1\n'
        '1,"Romania",2005,"MF",15,64,0,0,0,100,0,0,0,9,5,3,1,"ROM","Europe",1\n'
        '1,"Romania",2000,"F",15,64,0,0,0,100,0,0,0,9,5,3,1,"ROM","Europe",1\n'
    )
    assert people.read_barro_lee(path)["ROU"] == pytest.approx([0.4, 0.5, 0.1])


def test_read_wdi_and_regions(tmp_path):
    api = (
        '"Data Source","World Development Indicators",\n\n"Last Updated Date","2026-10-08",\n\n'
        '"Country Name","Country Code","Indicator Name","Indicator Code","1999","2000","2001",\n'
        '"Aruba","ABW","Gini","X","","41.5","",\n"Albania","ALB","Gini","X","30","","33",\n'
    )
    meta = '"Country Code","Region","IncomeGroup","SpecialNotes","TableName",\n"ALB","Europe & Central Asia","Upper","","Albania",\n"WLD","","","","World",\n'
    path = tmp_path / "wdi.zip"
    with zipfile.ZipFile(path, "w") as z:
        z.writestr("API_X_DS2_en_csv_v2_1.csv", api)
        z.writestr("Metadata_Country_API_X_DS2_en_csv_v2_1.csv", meta)
        z.writestr("Metadata_Indicator_API_X_DS2_en_csv_v2_1.csv", "junk")
    assert people.read_wdi(path) == {"ABW": {2000: 41.5}, "ALB": {1999: 30.0, 2001: 33.0}}
    assert people.read_wdi_regions(path) == ({"ALB": "Europe & Central Asia"}, {"Albania": "ALB"})


def test_read_swiid_places_countries_by_name(tmp_path):
    text = (
        "country,year,gini_disp,gini_disp_se,gini_mkt,gini_mkt_se,abs_red,abs_red_se,rel_red,rel_red_se\n"
        "Albania,2000,31.4,1,,,,,,\n"
        "Albania,2001,,1,,,,,,\n"
        "Congo-Kinshasa,2000,40,1,,,,,,\n"
        "Soviet Union,1990,24,1,,,,,,\n"
    )
    path = _csv_zip(tmp_path / "swiid.zip", "swiid/swiid_summary.csv", text)
    assert people.read_swiid(path, {"Albania": "ALB"}) == {"ALB": {2000: pytest.approx(0.314)}, "COD": {2000: 0.4}}


def test_xlsx_sheet_reads_numbers_and_shared_strings(tmp_path):
    main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    path = tmp_path / "t.xlsx"
    with zipfile.ZipFile(path, "w") as z:
        z.writestr(
            "xl/workbook.xml",
            f'<workbook xmlns="{main}" xmlns:r="{rel}"><sheets><sheet name="Info" sheetId="1" r:id="rId1"/>'
            f'<sheet name="Data" sheetId="2" r:id="rId2"/></sheets></workbook>',
        )
        z.writestr(
            "xl/_rels/workbook.xml.rels",
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="worksheets/sheet2.xml"/></Relationships>',
        )
        z.writestr(
            "xl/sharedStrings.xml", f'<sst xmlns="{main}"><si><t>code</t></si><si><t>labsh</t></si><si><t>AAA</t></si></sst>'
        )
        z.writestr("xl/worksheets/sheet1.xml", f'<worksheet xmlns="{main}"><sheetData/></worksheet>')
        z.writestr(
            "xl/worksheets/sheet2.xml",
            f'<worksheet xmlns="{main}"><sheetData>'
            '<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row>'
            '<row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2" t="n"><v>0.55</v></c></row>'
            '<row r="3"><c r="A3" t="s"><v>2</v></c><c r="B3"/></row>'
            "</sheetData></worksheet>",
        )
    assert list(people._xlsx_sheet(path, "Data")) == [{"code": "AAA", "labsh": 0.55}, {"code": "AAA"}]


def _observed() -> Observed:
    """Eight economies in two regions: income, then labour variables for some of them."""
    income = {"AAA": 1000.0, "BBB": 2000.0, "CCC": 9000.0, "DDD": 10000.0, "EEE": 1500.0, "FFF": 1100.0}
    region = {"AAA": "R1", "BBB": "R1", "CCC": "R2", "DDD": "R2", "EEE": "R1", "FFF": "R1"}
    low, mid = np.array([0.7, 0.25, 0.05]), np.array([0.2, 0.5, 0.3])
    return Observed(
        participation={"AAA": 0.6, "BBB": 0.7, "CCC": 0.8, "DDD": 0.75},
        unemployment={"AAA": 0.1, "BBB": 0.2, "CCC": 0.05, "DDD": 0.06},
        working_age={k: 0.6 for k in income},
        edu_wic={"AAA": low, "CCC": mid},
        edu_barro_lee={"BBB": np.array([0.5, 0.4, 0.1])},
        labour_share={"AAA": 0.5, "BBB": 0.6, "CCC": 0.65, "DDD": 0.7},
        gini_wdi={"AAA": 0.3, "CCC": 0.4},
        gini_swiid={"AAA": 0.9, "BBB": 0.35},
        gdp_per_capita=income,
        population={"AAA": 1.0, "BBB": 3.0},
        region=region,
    )


def test_country_labour_prefers_data_then_fallback_source_then_donor():
    obs = _observed()
    countries = {c: c for c in ("AAA", "BBB", "CCC", "DDD", "EEE", "FFF")}
    out = people.country_labour(countries, obs, {})

    a, b = out["AAA"], out["BBB"]
    assert (a.gini, a.gini_source, a.gini_imputed) == (0.3, "wdi", False)  # WDI wins over SWIID
    assert (b.gini, b.gini_source, b.gini_imputed) == (0.35, "swiid", True)
    assert not b.edu_imputed and b.edu_shares == pytest.approx([0.5, 0.4, 0.1])  # Barro-Lee fills a WIC gap
    # DDD has no Gini or education from any source: the donor is the same-region economy nearest in income.
    d = out["DDD"]
    assert (d.gini, d.gini_source, d.gini_imputed) == (0.4, "donor", True)
    assert d.edu_imputed and d.edu_shares == pytest.approx([0.2, 0.5, 0.3])
    # EEE (1500) is nearer BBB (2000) than AAA (1000) in logs: 0.288 against 0.405.
    e = out["EEE"]
    assert e.participation_imputed and e.participation == 0.7
    assert e.labour_share == 0.6 and e.labour_share_imputed
    # Observed values are never flagged.
    assert not (a.participation_imputed or a.unemployment_imputed or a.labour_share_imputed or a.working_age_imputed)


def test_country_labour_donor_uses_the_region_median_without_income():
    obs = _observed()
    out = people.country_labour({"ZZZ": "ZZZ"}, obs, {})
    z = out["ZZZ"]
    # No region and no income: the donor is the economy nearest the median log income of all donors.
    assert z.participation_imputed and z.participation in (0.6, 0.7, 0.8, 0.75)
    obs.region["ZZZ"] = "R2"
    z = people.country_labour({"ZZZ": "ZZZ"}, obs, {})["ZZZ"]
    assert z.participation in (0.8, 0.75)  # R2 donors only


def test_country_labour_combines_split_economies_by_population_and_follows_the_sovereign_region():
    obs = _observed()
    out = people.country_labour({"YUG": "YUG", "TER": "CCC"}, obs, {})
    # YUG = SRB + MNE (PARTS): neither is in the series, both missing, so a donor; give them data and combine.
    obs.participation.update({"SRB": 0.5, "MNE": 0.9})
    obs.population.update({"SRB": 3.0, "MNE": 1.0})
    combined = people.country_labour({"YUG": "YUG"}, obs, {})["YUG"]
    assert combined.participation == pytest.approx(0.6) and not combined.participation_imputed
    # A territory with no region of its own takes its sovereign's region for the donor search.
    assert out["TER"].participation in (0.8, 0.75)


def test_labour_columns_and_flags():
    obs = _observed()
    labour = people.country_labour({"AAA": "AAA", "DDD": "DDD"}, obs, {})
    cols = people.country_columns(["DDD", "AAA"], labour)
    assert cols["gini"].to_pylist() == [0.4, 0.3]
    assert cols["gini_imputed"].to_pylist() == [True, False]
    assert cols["edu_imputed"].to_pylist() == [True, False]
    total = sum(np.array(cols[k].to_pylist()) for k in ("edu_low", "edu_secondary", "edu_tertiary"))
    assert total == pytest.approx([1.0, 1.0])


def _labour(shares, participation=0.5, working_age=0.6, **flags) -> Labour:
    return Labour(participation, 0.1, np.array(shares), 0.5, working_age, 0.3, "wdi", **flags)


def test_pops_split_each_province_by_its_countrys_education_shares():
    labour = {"AAA": _labour([0.5, 0.3, 0.2]), "BBB": _labour([0.1, 0.1, 0.8], participation=0.8, edu_imputed=True)}
    population = np.array([1000.0, 200.0, 50.0])
    t = people.pops(["AAA", "AAA", "BBB"], population, labour)
    assert t.num_rows == 9
    assert t["province"].to_pylist() == [0, 0, 0, 1, 1, 1, 2, 2, 2]
    assert t["edu"].to_pylist() == [0, 1, 2] * 3
    pop = np.array(t["population"].to_pylist()).reshape(3, 3)
    assert pop.sum(axis=1) == pytest.approx(population)
    assert pop[0] == pytest.approx([500, 300, 200])
    wa = np.array(t["working_age"].to_pylist()).reshape(3, 3)
    assert wa.sum(axis=1) == pytest.approx(population * 0.6)
    lf = np.array(t["labour_force"].to_pylist()).reshape(3, 3)
    assert lf[2] == pytest.approx(wa[2] * 0.8) and lf[0] == pytest.approx(wa[0] * 0.5)
    assert t["imputed"].to_pylist() == [False] * 6 + [True] * 3


def test_province_columns_scale_by_working_age_share():
    labour = {"AAA": _labour([1, 0, 0], working_age=0.5), "BBB": _labour([1, 0, 0], working_age=0.7, working_age_imputed=True)}
    cols = people.province_columns(["AAA", "BBB"], np.array([10.0, 10.0]), labour)
    assert cols["working_age"].to_pylist() == [5.0, 7.0] and cols["working_age_imputed"].to_pylist() == [False, True]


def _raster(path: Path, values: np.ndarray, lon0: float, lat0: float, step: float) -> Path:
    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        height=values.shape[0],
        width=values.shape[1],
        count=1,
        dtype="float32",
        crs="EPSG:4326",
        transform=from_origin(lon0, lat0, step, step),
        nodata=NODATA,
    ) as ds:
        ds.write(values.astype("float32"), 1)
    return path


def _centres(values: np.ndarray, lon0: float, lat0: float, step: float):
    rows, cols = np.nonzero((values > 0) & (values != NODATA))
    return (lat0 - (rows + 0.5) * step), (lon0 + (cols + 0.5) * step)


def test_province_population_sums_pixels_by_cell_owner(tmp_path):
    values = np.ones((40, 40))
    values[0, 0] = NODATA
    values[5, 5] = 0.0
    lon0, lat0, step = 10.0, 50.0, 0.05
    path = _raster(tmp_path / "wp.tif", values, lon0, lat0, step)
    lat, lon = _centres(values, lon0, lat0, step)
    cells = sorted({h3.latlng_to_cell(a, o, 4) for a, o in zip(lat, lon)})
    assert len(cells) >= 4
    owner = [i % 2 for i in range(len(cells))]
    table = pa.table({"cell": pa.array(cells, pa.uint64()), "province": pa.array(owner, pa.uint32())})
    got = people.province_population(path, table)
    expected = np.zeros(2)
    for a, o in zip(lat, lon):
        expected[owner[cells.index(h3.latlng_to_cell(a, o, 4))]] += 1
    assert got == pytest.approx(expected)
    assert got.sum() == len(lat) == 1600 - 2


def test_province_population_sends_an_unowned_cell_to_the_nearest_owned_one(tmp_path):
    values = np.ones((40, 40))
    lon0, lat0, step = 10.0, 50.0, 0.05
    path = _raster(tmp_path / "wp.tif", values, lon0, lat0, step)
    lat, lon = _centres(values, lon0, lat0, step)
    cells = sorted({h3.latlng_to_cell(a, o, 4) for a, o in zip(lat, lon)})
    drop = cells[0]
    near = min(
        (c for c in cells if c != drop), key=lambda c: h3.great_circle_distance(h3.cell_to_latlng(c), h3.cell_to_latlng(drop))
    )
    kept = [c for c in cells if c != drop]
    owner = [0 if c != near else 1 for c in kept]
    table = pa.table({"cell": pa.array(kept, pa.uint64()), "province": pa.array(owner, pa.uint32())})
    got = people.province_population(path, table)
    assert got.sum() == values.sum()
    dropped = sum(1 for a, o in zip(lat, lon) if h3.latlng_to_cell(a, o, 4) == drop)
    own = sum(1 for a, o in zip(lat, lon) if h3.latlng_to_cell(a, o, 4) == near)
    assert got[1] == own + dropped


def test_province_population_prefers_the_finest_owned_cell(tmp_path):
    values = np.ones((40, 40))
    lon0, lat0, step = 10.0, 50.0, 0.05
    path = _raster(tmp_path / "wp.tif", values, lon0, lat0, step)
    lat, lon = _centres(values, lon0, lat0, step)
    coarse = sorted({h3.latlng_to_cell(a, o, 4) for a, o in zip(lat, lon)})
    fine = h3.latlng_to_cell(lat[100], lon[100], 6)
    table = pa.table({"cell": pa.array([*coarse, fine], pa.uint64()), "province": pa.array([0] * len(coarse) + [1], pa.uint32())})
    got = people.province_population(path, table)
    in_fine = sum(1 for a, o in zip(lat, lon) if h3.latlng_to_cell(a, o, 6) == fine)
    assert in_fine > 0
    assert got[1] == in_fine and got.sum() == values.sum()


def test_scale_to_national_keeps_shares_and_leaves_unknown_countries_alone():
    population = np.array([10.0, 30.0, 5.0, 0.0, 0.0, 7.0])
    country = ["AAA", "AAA", "BBB", "CCC", "CCC", "DDD"]
    out = people.scale_to_national(population, country, {"AAA": 80.0, "CCC": 50.0})
    assert out.tolist() == [20.0, 60.0, 5.0, 0.0, 0.0, 7.0]  # BBB has no total, CCC no people, DDD no total
    assert population.tolist() == [10.0, 30.0, 5.0, 0.0, 0.0, 7.0]


def test_national_population_sums_split_economies_and_drops_unknown_ones():
    obs = Observed(population={"SRB": 7.0, "MNE": 0.6, "FRA": 59.0})
    out = people.national_population(["YUG", "FRA", "CLP"], obs, {"FRA": "FRA"})
    assert out == {"YUG": pytest.approx(7.6), "FRA": 59.0}


# Tests below need the downloaded sources and the 2000 pack; CI is data-free.
def _cached(name: str):
    path = fetch.path_of(name)
    if not path.exists():
        pytest.skip(f"skipping: no {name} in {path.parent}; fetch it with `uv run python -m ceteris.packs.fetch`")
    return path


def _pack(name: str) -> pa.Table:
    path = PACK / name
    if not path.exists():
        pytest.skip(f"skipping: no {name} in {PACK}; build the pack with `uv run python -m ceteris.packs.build`")
    return ipc.open_file(str(path)).read_all()


NAMES = [
    "BL_v3_MF1564.csv",
    "BPJresult_AGE_V01.csv",
    "ilostat_EAP_2WAP_SEX_AGE_RT_A_2000.csv",
    "ilostat_UNE_2EAP_SEX_AGE_RT_A_2000.csv",
    "pwt110.xlsx",
    "wdi_SI.POV.GINI.zip",
    "wdi_SP.POP.TOTL.zip",
    "wdi_SP.POP.1564.TO.ZS.zip",
    "swiid9_92.zip",
    "ne_10m_admin_0_countries.zip",
]


@pytest.fixture(scope="module")
def observed():
    for name in NAMES:
        _cached(name)
    return people.load_observed(fetch.CACHE, fetch.path_of("ne_10m_admin_0_countries.zip"))


@pytest.fixture(scope="module")
def labour(observed):
    countries = _pack("countries.arrow").to_pylist()
    iso, _ = people.read_codes(fetch.path_of("ne_10m_admin_0_countries.zip"))
    return people.country_labour({r["code"]: r["sovereign"] for r in countries}, observed, iso)


@pytest.fixture(scope="module")
def province_population():
    path = _cached("ppp_2000_1km_Aggregated.tif")
    return people.province_population(path, _pack("cells.arrow"))


def test_every_pack_country_has_labour_inputs(labour):
    assert len(labour) == _pack("countries.arrow").num_rows == 242


def test_edu_shares_sum_to_one(labour):
    shares = np.array([l.edu_shares for l in labour.values()])
    assert shares.shape == (242, 3)
    assert (shares >= 0).all() and shares.sum(axis=1) == pytest.approx(np.ones(242), abs=1e-12)


def test_labour_share_in_unit_interval(labour):
    for field in ("labour_share", "participation", "unemployment", "working_age_share"):
        v = np.array([getattr(l, field) for l in labour.values()])
        assert ((v > 0) & (v < 1)).all(), field
    assert 0.3 < np.median([l.labour_share for l in labour.values()]) < 0.8


def test_gini_in_unit_interval_with_source_flag(labour):
    g = np.array([l.gini for l in labour.values()])
    assert ((g > 0.15) & (g < 0.8)).all()
    assert all(l.gini_imputed == (l.gini_source != "wdi") for l in labour.values())
    assert {l.gini_source for l in labour.values()} <= {"wdi", "swiid", "donor"}
    # Most countries have a WDI Gini within 1995 to 2005 or a SWIID value.
    assert sum(l.gini_source != "donor" for l in labour.values()) > 150


def test_data_shares_follow_the_gini_rule(observed, labour):
    assert labour["USA"].gini_source == "wdi" and labour["USA"].gini == pytest.approx(observed.gini_wdi["USA"])
    assert 0.35 < labour["USA"].gini < 0.45 and 0.5 < labour["BRA"].gini < 0.65


def test_wic_and_barro_lee_agree_on_education(observed):
    both = sorted(set(observed.edu_wic) & set(observed.edu_barro_lee))
    assert len(both) > 100
    wic = np.array([observed.edu_wic[k] for k in both])
    barro_lee = np.array([observed.edu_barro_lee[k] for k in both])
    # Barro-Lee counts any secondary or tertiary attendance, WIC only completed levels: the means differ by a few points.
    assert np.abs(wic - barro_lee).mean() < 0.10
    assert np.corrcoef(wic[:, 2], barro_lee[:, 2])[0, 1] > 0.7
    assert np.corrcoef(wic[:, 0], barro_lee[:, 0])[0, 1] > 0.9


def _by_country(population) -> dict[str, float]:
    got: dict[str, float] = {}
    for c, p in zip(_pack("provinces.arrow")["country"].to_pylist(), population):
        got[c] = got.get(c, 0.0) + p
    return got


def test_worldpop_totals_follow_the_united_nations_within_the_known_source_gap(province_population, observed):
    """Raw WorldPop 2000 is not UN-adjusted: large countries differ from WDI by -12 to +38%, so only gross errors fail here."""
    iso, _ = people.read_codes(fetch.path_of("ne_10m_admin_0_countries.zip"))
    got = _by_country(province_population)
    expected = people.national_population(list(got), observed, iso)
    big = [c for c, e in expected.items() if e >= 10e6]
    assert len(big) >= 50
    for code in big:
        assert 1 / 1.5 < got[code] / expected[code] < 1.5, code
    assert province_population.sum() == pytest.approx(sum(expected.values()), rel=0.05)


def test_population_sums_match_country_totals_within_2pct(province_population, observed):
    iso, _ = people.read_codes(fetch.path_of("ne_10m_admin_0_countries.zip"))
    country = _pack("provinces.arrow")["country"].to_pylist()
    expected = people.national_population(sorted(set(country)), observed, iso)
    got = _by_country(people.scale_to_national(province_population, country, expected))
    big = [c for c, e in expected.items() if e >= 10e6]
    assert len(big) >= 50
    for code in big:
        assert got[code] == pytest.approx(expected[code], rel=0.02), code
    # Provinces keep their shares of the country.
    raw = _by_country(province_population)
    scaled = people.scale_to_national(province_population, country, expected)
    first = next(i for i, c in enumerate(country) if c == "FRA")
    assert scaled[first] / got["FRA"] == pytest.approx(province_population[first] / raw["FRA"])


def test_pops_from_the_pack_cover_every_province(province_population, labour):
    provinces = _pack("provinces.arrow")
    t = people.pops(provinces["country"].to_pylist(), province_population, labour)
    assert t.num_rows == provinces.num_rows * 3
    assert sum(t["population"].to_pylist()) == pytest.approx(province_population.sum())
    assert sum(t["labour_force"].to_pylist()) < sum(t["working_age"].to_pylist()) < province_population.sum()


def test_codes_follow_the_sources_spelling():
    _cached("ne_10m_admin_0_countries.zip")
    iso, m49 = people.read_codes(fetch.path_of("ne_10m_admin_0_countries.zip"))
    assert iso["KOS"] == "XKX" and iso["PSX"] == "PSE" and iso["FRA"] == "FRA"
    assert m49["NOR"] == 578 and m49["TWN"] == 158 and m49["SSD"] == 728 and "XKX" not in m49
