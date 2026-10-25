import zipfile

import numpy as np
import pytest

from ceteris.packs import fetch
from ceteris.packs.econ import goods, icio, people
from ceteris.packs.econ.icio import CountryIO, Wdi
from ceteris.packs.econ.people import Observed

# Industries of the hand-made table and their goods: A01 and A02 are good 0, B06 good 1, C10T12 good 3, G and K good 9.
INDUSTRIES = ["A01", "A02", "B06", "C10T12", "G", "K"]
GOOD = [0, 0, 1, 3, 9, 9]
FINAL = ["HFCE", "NPISH", "GGFC", "GFCF", "INVNT", "DPABR"]


def _f(x) -> str:
    return repr(float(x))


def _table(
    economies=("AAA", "BBB"), invnt=None, deplete=None, seed=1, industries=INDUSTRIES
) -> tuple[str, dict[str, np.ndarray]]:
    """An ICIO-shaped CSV (industry block, final demand block, OUT; rows then TLS, VA, OUT) and its parts."""
    rng = np.random.default_rng(seed)
    cols = [f"{e}_{i}" for e in economies for i in industries]
    fd_cols = [f"{e}_{f}" for e in economies for f in FINAL]
    n = len(cols)
    z = rng.uniform(1, 10, (n, n))
    fd = rng.uniform(1, 10, (n, len(fd_cols)))
    if invnt is not None:
        for j, c in enumerate(fd_cols):
            if c.endswith("INVNT"):
                fd[:, j] = invnt
                if deplete is not None:
                    fd[0, j] = deplete
    tls = rng.uniform(1, 3, n)
    va = rng.uniform(10, 30, n)
    out = z.sum(0) + tls + va
    header = ["V1", *cols, *fd_cols, "OUT"]
    lines = [",".join(header)]
    for i, c in enumerate(cols):
        lines.append(",".join([c, *map(_f, z[i]), *map(_f, fd[i]), repr(float(z[i].sum() + fd[i].sum()))]))
    lines.append(",".join(["TLS", *map(_f, tls), *["0"] * len(fd_cols), repr(float(tls.sum()))]))
    lines.append(",".join(["VA", *map(_f, va), *["0"] * len(fd_cols), repr(float(va.sum()))]))
    lines.append(",".join(["OUT", *map(_f, out), *["0"] * len(fd_cols), repr(float(out.sum()))]))
    return "\n".join(lines) + "\n", {"z": z, "fd": fd, "va": va, "out": out}


@pytest.fixture
def table(tmp_path):
    text, parts = _table()
    path = tmp_path / "2000_SML.csv"
    path.write_text(text)
    return path, parts


def test_aggregation_preserves_output_totals(table):
    path, parts = table
    got = icio.aggregate(path)
    assert set(got) == {"AAA", "BBB"}
    for k, code in enumerate(("AAA", "BBB")):
        block = slice(k * 6, k * 6 + 6)
        assert got[code].output.sum() == pytest.approx(parts["out"][block].sum(), rel=1e-9)
        # VA is output less intermediate inputs (ICIO VA plus taxes less subsidies on products).
        expected_va = parts["out"][block] - parts["z"][:, block].sum(axis=0)
        assert got[code].va.sum() == pytest.approx(expected_va.sum(), rel=1e-9)
        for g in range(goods.N_GOODS):
            member = [i for i, x in enumerate(GOOD) if x == g]
            assert got[code].output[g] == pytest.approx(parts["out"][block][member].sum() if member else 0.0, rel=1e-9)


def test_exports_sum_sales_to_foreign_columns(table):
    """Intermediate and final sales of an economy's industries to the other economy's columns, by good."""
    path, parts = table
    got = icio.exports(path)
    n_fd = len(FINAL)
    for k, code in enumerate(("AAA", "BBB")):
        rows, other = slice(k * 6, k * 6 + 6), slice((1 - k) * 6, (1 - k) * 6 + 6)
        abroad = parts["z"][rows, other].sum(axis=1) + parts["fd"][rows, (1 - k) * n_fd : (2 - k) * n_fd].sum(axis=1)
        want = np.bincount(GOOD, abroad, goods.N_GOODS)
        assert got[code] == pytest.approx(want, rel=1e-12)


def test_value_added_closes_the_identity(table):
    path, _ = table
    for c in icio.aggregate(path).values():
        np.testing.assert_allclose(c.va + c.io.sum(axis=0) * c.output, c.output, rtol=1e-9)


def test_intermediates_count_inputs_from_every_source_economy(table):
    path, parts = table
    a = icio.aggregate(path)["AAA"]
    # Input of good 9 (rows G, K of both economies) per unit of AAA's good 0 (columns A01, A02).
    rows = [i for i in range(12) if GOOD[i % 6] == 9]
    expected = parts["z"][np.ix_(rows, [0, 1])].sum() / parts["out"][[0, 1]].sum()
    assert a.io[9, 0] == pytest.approx(expected, rel=1e-12)
    assert a.io.shape == (goods.N_GOODS, goods.N_GOODS)


def test_io_coefficients_below_one(table):
    path, _ = table
    for c in icio.aggregate(path).values():
        assert (c.io >= 0).all() and (c.io.sum(axis=0) < 1).all()


def test_goods_without_output_have_an_empty_column(table):
    path, _ = table
    a = icio.aggregate(path)["AAA"]
    # Goods 2, 4 to 8 have no industry in the hand table.
    assert a.output[2] == 0 and (a.io[:, 2] == 0).all()
    assert np.isfinite(a.io).all()


def test_final_demand_shares_sum_to_one(table):
    path, parts = table
    got = icio.aggregate(path)["BBB"]
    assert got.final_demand_shares.sum() == pytest.approx(1.0, abs=1e-12)
    assert (got.final_demand_shares[[2, 4, 5, 6, 7, 8]] == 0).all()
    # BBB columns 6 to 11 of the final block, all five demand kinds but purchases abroad, every source economy.
    fd = parts["fd"][:, 6:12][:, :5]
    expected = np.array([fd[[0, 1, 6, 7]].sum(), 0, 0, fd[[3, 9]].sum()])
    assert got.final_demand_shares[0] == pytest.approx(expected[0] / fd.sum(), rel=1e-12)
    assert got.final_demand_shares[3] == pytest.approx(expected[3] / fd.sum(), rel=1e-12)


def test_inventory_changes_enter_final_demand(tmp_path):
    text, _ = _table(invnt=0.0)
    (tmp_path / "a.csv").write_text(text)
    text, _ = _table(invnt=5.0)
    (tmp_path / "b.csv").write_text(text)
    a, b = (icio.aggregate(tmp_path / f"{x}.csv")["AAA"].final_demand_shares for x in "ab")
    assert not np.allclose(a, b)


def test_negative_final_demand_share_stops_the_build(tmp_path):
    text, _ = _table(invnt=1.0, deplete=-1e6)
    path = tmp_path / "neg.csv"
    path.write_text(text)
    with pytest.raises(ValueError, match="negative final demand"):
        icio.aggregate(path)


def test_aggregate_reads_a_zip_of_yearly_files(tmp_path, table):
    path, _ = table
    z = tmp_path / "1995-2000_SML.zip"
    with zipfile.ZipFile(z, "w") as f:
        f.writestr("1999_SML.csv", "wrong\n")
        f.writestr("2000_SML.csv", path.read_text())
    got = icio.aggregate(z)
    assert got["AAA"].output.sum() == pytest.approx(icio.aggregate(path)["AAA"].output.sum())


def test_combined_coal_industry_is_not_supported(tmp_path):
    text = "V1,AAA_B05_06,AAA_HFCE,OUT\nAAA_B05_06,1,1,2\nTLS,1,0,1\nVA,1,0,1\nOUT,3,0,3\n"
    path = tmp_path / "old.csv"
    path.write_text(text)
    with pytest.raises(NotImplementedError, match="B05_06"):
        icio.aggregate(path)


def _country(colsum: float, final=None) -> CountryIO:
    """A donor whose VA is DONOR_VA and whose inputs take `colsum` of every column's output."""
    va = np.array(DONOR_VA, float)
    return CountryIO(
        output=va / (1 - colsum),
        va=va,
        io=np.full((10, 10), colsum / 10),
        final_demand_shares=np.full(10, 0.1) if final is None else np.asarray(final, float),
    )


DONOR_VA = [10, 4, 1, 6, 2, 1, 1, 3, 2, 70]


def _donors():
    return {
        "AAA": _country(0.1),
        "BBB": _country(0.2, final=[0.5, 0, 0, 0.5, 0, 0, 0, 0, 0, 0]),
        "CCC": _country(0.3),
    }


DONOR_GDP = {"AAA": 100e6, "BBB": 200e6, "CCC": 125e6}  # USD; donor VA is 100 million USD, so VA over GDP is 1, 0.5, 0.8
DONOR_POP = {"AAA": 1e6, "BBB": 5e6, "CCC": 1e6}
PWT = {"AAA": 1000.0, "BBB": 20000.0, "CCC": 10000.0}
WDI_INCOME = {"AAA": 10000.0, "BBB": 50000.0, "CCC": 3000.0}


def _obs(pwt=None, **kw) -> Observed:
    return Observed(
        region={"AAA": "R1", "BBB": "R1", "CCC": "R2", "XXX": "R1", "YYY": "R2", **kw.get("region", {})},
        gdp_per_capita={**PWT, "XXX": 11000.0, "YYY": 11000.0} if pwt is None else pwt,
        population={**DONOR_POP, "XXX": 1e6, "YYY": 2e6, "ZZZ": 3e6, **kw.get("population", {})},
    )


def _no_pwt_for_xxx() -> Observed:
    return _obs(pwt=dict(PWT))


def _wdi(**kw) -> Wdi:
    full = {
        "gdp": {**DONOR_GDP, "XXX": 1e9},
        "income": WDI_INCOME,
        "income_source": dict.fromkeys(WDI_INCOME, "wdi"),
        "agriculture": {"XXX": 0.1},
        "industry": {"XXX": 0.35},
        "manufacturing": {"XXX": 0.2},
        "services": {"XXX": 0.45},
    }
    return Wdi(**{**full, **kw})


def _closes(c: CountryIO) -> None:
    np.testing.assert_allclose(c.va + c.io.sum(axis=0) * c.output, c.output, rtol=1e-9, atol=1e-12)


def test_imputed_country_uses_nearest_donor():
    donors = _donors()
    got = icio.impute({"XXX": "XXX", "YYY": "YYY"}, donors, _wdi(gdp={**DONOR_GDP, "XXX": 1e9, "YYY": 1e9}), _obs(), {})
    # XXX: same region as AAA and BBB, closest log income among them is BBB, although CCC (other region) is closer overall.
    assert got["XXX"].donor == "BBB" and got["YYY"].donor == "CCC"
    assert got["XXX"].region == "R1" and got["XXX"].income_source == "pwt"
    np.testing.assert_array_equal(got["XXX"].io, donors["BBB"].io)
    np.testing.assert_array_equal(got["XXX"].final_demand_shares, donors["BBB"].final_demand_shares)
    assert got["XXX"].final_demand_shares is not donors["BBB"].final_demand_shares


def test_icio_economies_keep_their_own_tables():
    donors = _donors()
    got = icio.impute({"AAA": "AAA", "XXX": "XXX"}, donors, _wdi(), _obs(), {})
    assert got["AAA"].donor is None and got["AAA"].va_basis == "icio" and got["AAA"] is donors["AAA"]
    assert got["XXX"].donor == "BBB"


def test_imputed_va_follows_wdi_shares_split_by_donor_proportions():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(), _obs(), {})["XXX"]
    # GDP 1000 million USD; agriculture 100, services 450, manufacturing 200 over goods 3 to 6 as the donor
    # (6, 2, 1, 1), industry other than manufacturing 150 over goods 1, 2, 7, 8 as the donor (4, 1, 3, 2).
    expected = [100, 60, 15, 120, 40, 20, 20, 45, 30, 450]
    np.testing.assert_allclose(got.va, expected, rtol=1e-12)
    # Output keeps the donor's output per unit of VA of each good (BBB's inputs take 0.2 of output).
    np.testing.assert_allclose(got.output, got.va / 0.8, rtol=1e-12)
    assert got.va_basis == "wdi"
    _closes(got)


def test_missing_sector_share_uses_gdp_times_the_shares_there_are():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(services={}), _obs(), {})["XXX"]
    # Agriculture 0.1 and industry 0.35 of GDP 1000 million USD, split as the donor's VA (the donor's own VA/GDP is not used).
    assert got.va_basis == "wdi_gdp" and got.donor == "BBB"
    assert got.va.sum() == pytest.approx(450.0)
    np.testing.assert_allclose(got.va / got.va.sum(), np.array(DONOR_VA) / 100, rtol=1e-12)
    _closes(got)


def test_missing_manufacturing_splits_industry_over_goods_one_to_eight():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(manufacturing={}), _obs(), {})["XXX"]
    assert got.va_basis == "wdi_gdp"
    assert got.va[0] == pytest.approx(100) and got.va[9] == pytest.approx(450)
    # Industry 350 over goods 1 to 8 as the donor (4, 1, 6, 2, 1, 1, 3, 2) / 20.
    np.testing.assert_allclose(got.va[1:9], 350 * np.array([4, 1, 6, 2, 1, 1, 3, 2]) / 20, rtol=1e-12)
    _closes(got)


def test_industry_below_manufacturing_counts_as_missing_manufacturing():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(industry={"XXX": 0.1}), _obs(), {})["XXX"]
    assert got.va_basis == "wdi_gdp" and got.va.sum() == pytest.approx(650.0)
    assert got.va[1:9].sum() == pytest.approx(100.0)


def test_all_zero_sector_shares_count_as_missing_and_use_the_icio_median_va_over_gdp():
    zero = {"agriculture": {"XXX": 0.0}, "industry": {"XXX": 0.0}, "manufacturing": {"XXX": 0.0}, "services": {"XXX": 0.0}}
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(**zero), _obs(), {})["XXX"]
    # No usable share: GDP 1000 million USD times the median of the donors' VA over GDP (1, 0.5, 0.8), not donor BBB's 0.5.
    assert got.donor == "BBB" and got.va_basis == "wdi_gdp" and got.va.sum() == pytest.approx(800.0)
    np.testing.assert_allclose(got.va / got.va.sum(), np.array(DONOR_VA) / 100, rtol=1e-12)
    assert icio.median_va_over_gdp(_donors(), _wdi()) == pytest.approx(0.8)


def test_donor_basis_scales_the_donors_level_by_income():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(gdp=DONOR_GDP), _obs(), {})["XXX"]
    # Donor VA 100 million USD for 5 million people, so 20 USD per person; XXX has 1 million people at 11000/20000 of its income.
    assert got.va_basis == "donor" and got.donor == "BBB" and got.income_source == "pwt"
    assert got.va.sum() == pytest.approx(20.0 * 11000 / 20000)
    np.testing.assert_allclose(got.va / got.va.sum(), np.array(DONOR_VA) / 100, rtol=1e-12)
    _closes(got)


def _wdi_income(source: str) -> Wdi:
    income = {**WDI_INCOME, "XXX": 12000.0}
    return _wdi(gdp=DONOR_GDP, income=income, income_source={**dict.fromkeys(WDI_INCOME, "wdi"), "XXX": source})


def test_without_pwt_income_donors_are_ranked_and_scaled_by_wdi_gdp_per_person():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi_income("wdi"), _no_pwt_for_xxx(), {})["XXX"]
    # PWT would pick BBB (20000 is the closest in its region to 12000); WDI incomes pick AAA (10000, BBB is 50000).
    assert got.donor == "AAA" and got.income_source == "wdi"
    # AAA: 100 million USD for 1 million people; XXX has 1 million people at 12000/10000 of its income.
    assert got.va.sum() == pytest.approx(100.0 * 12000 / 10000)


def test_adb_income_is_flagged():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi_income("adb"), _no_pwt_for_xxx(), {})["XXX"]
    assert got.income_source == "adb" and got.donor == "AAA"


def test_without_pwt_or_wdi_income_donors_are_ranked_and_scaled_by_maddison_gdp_per_person():
    maddison = {"AAA": 1500.0, "BBB": 4000.0, "CCC": 2000.0, "XXX": 3000.0}
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(gdp=DONOR_GDP, maddison=maddison), _no_pwt_for_xxx(), {})["XXX"]
    # In region R1, Maddison values pick BBB (4000 is closer in logs to 3000 than 1500), where WDI incomes would pick AAA.
    assert got.donor == "BBB" and got.income_source == "maddison" and got.va_basis == "donor"
    # BBB: 100 million USD for 5 million people; XXX has 1 million people at 3000/4000 of BBB's Maddison income.
    assert got.va.sum() == pytest.approx(20.0 * 3000 / 4000)


def test_wdi_income_takes_precedence_over_maddison():
    wdi = _wdi_income("wdi")
    wdi.maddison = {"AAA": 1500.0, "BBB": 4000.0, "XXX": 3000.0}
    got = icio.impute({"XXX": "XXX"}, _donors(), wdi, _no_pwt_for_xxx(), {})["XXX"]
    assert got.donor == "AAA" and got.income_source == "wdi"


def test_without_any_income_the_donor_is_the_region_median_and_the_level_is_unscaled():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(gdp=DONOR_GDP), _no_pwt_for_xxx(), {})["XXX"]
    # Region R1 holds AAA (1000) and BBB (20000): the median of their logs is equidistant, so the name decides.
    assert got.donor == "AAA" and got.income_source == "none" and got.va_basis == "donor"
    assert got.va.sum() == pytest.approx(100.0)


def test_region_override_places_an_unclassified_country():
    obs = _obs(region={"XXX": None})
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(), obs, {}, regions={"XXX": "R2"})["XXX"]
    assert got.region == "R2" and got.donor == "CCC"


def test_people_come_from_worldpop_when_wdi_has_none():
    obs = _obs(region={"WWW": "R1"})
    got = icio.impute({"WWW": "WWW"}, _donors(), _wdi(gdp=DONOR_GDP), obs, {}, worldpop={"WWW": 2e6})["WWW"]
    # No income of its own, so donor AAA's level: 2 million people at 100 USD per person.
    assert got.donor == "AAA" and got.va_basis == "donor" and got.va.sum() == pytest.approx(200.0)


def test_uninhabited_territory_keeps_zero_value_added():
    obs = _obs(region={"ZZZ": "R1"}, population={"ZZZ": 0.0})
    got = icio.impute({"ZZZ": "ZZZ"}, _donors(), _wdi(gdp=DONOR_GDP), obs, {})["ZZZ"]
    assert got.va_basis == "none" and got.va.sum() == 0 and got.output.sum() == 0
    assert got.donor is not None
    _closes(got)


def test_territory_inside_a_parent_takes_its_population_share_from_the_parent():
    obs = _obs(region={"TER": "R1"})
    plain = icio.impute({"XXX": "XXX"}, _donors(), _wdi(), obs, {})["XXX"]
    got = icio.impute({"XXX": "XXX", "TER": "XXX"}, _donors(), _wdi(), obs, {}, worldpop={"TER": 1e5}, inside={"TER": "XXX"})
    ter, par = got["TER"], got["XXX"]
    # 100,000 of the parent's 1,000,000 people: 10 percent of its VA moves out, nothing is added.
    assert ter.va_basis == "parent" and ter.donor == "BBB" and par.shed == pytest.approx(0.1)
    np.testing.assert_allclose(ter.va, 0.1 * plain.va, rtol=1e-12)
    np.testing.assert_allclose(par.va, 0.9 * plain.va, rtol=1e-12)
    np.testing.assert_allclose(par.output + ter.output, plain.output, rtol=1e-12)
    np.testing.assert_array_equal(ter.io, plain.io)
    _closes(ter)
    _closes(par)


def test_territory_inside_an_icio_economy_shrinks_that_economy():
    donors = _donors()
    got = icio.impute({"AAA": "AAA", "TER": "AAA"}, donors, _wdi(), _obs(), {}, worldpop={"TER": 1e5}, inside={"TER": "AAA"})
    # AAA has 1 million people.
    np.testing.assert_allclose(got["AAA"].va, 0.9 * donors["AAA"].va, rtol=1e-12)
    np.testing.assert_allclose(got["TER"].va, 0.1 * donors["AAA"].va, rtol=1e-12)
    assert got["TER"].donor == "AAA" and got["AAA"].va_basis == "icio" and got["AAA"].shed == pytest.approx(0.1)
    _closes(got["AAA"])


def test_split_economies_sum_gdp_and_weight_shares_by_gdp():
    wdi = Wdi(
        gdp={**DONOR_GDP, "PA": 1e9, "PB": 3e9},
        income=WDI_INCOME,
        agriculture={"PA": 0.2, "PB": 0.0},
        industry={"PA": 0.3, "PB": 0.3},
        manufacturing={"PA": 0.1, "PB": 0.1},
        services={"PA": 0.4, "PB": 0.6},
    )
    obs = _obs(region={"PA": "R1"}, population={"PA": 1e6, "PB": 1e6})
    got = icio.impute({"XXX": "XXX"}, _donors(), wdi, obs, {}, parts={"XXX": ("PA", "PB")})["XXX"]
    assert got.va.sum() == pytest.approx(4000 * (0.2 * 1 / 4 + 0.3 + 0.4 * 1 / 4 + 0.6 * 3 / 4))
    assert got.va[0] == pytest.approx(4000 * 0.05)


def test_income_is_gdp_per_person_of_the_year_nearest_2000_within_1995_to_2010():
    gdp = {"AAA": {2000: 100.0, 2001: 200.0}, "BBB": {1994: 50.0, 2008: 80.0, 2012: 90.0}, "CCC": {2012: 5.0}}
    pop = {"AAA": {2000: 10.0, 2001: 10.0}, "BBB": {1994: 5.0, 2008: 8.0, 2012: 9.0}, "CCC": {2012: 5.0}}
    # AAA: 2000 (10 per person); BBB: 1994 and 2012 are outside the window, 2008 is inside (10); CCC: only 2012, none.
    assert icio.income_by_year(gdp, pop) == {"AAA": 10.0, "BBB": 10.0}


# Tests below need the downloaded ICIO package and the 2000 pack; CI is data-free.
def _cached(name: str):
    path = fetch.path_of(name)
    if not path.exists():
        pytest.skip(f"skipping: no {name} in {path.parent}; fetch it with `uv run python -m ceteris.packs.fetch`")
    return path


@pytest.fixture(scope="module")
def real():
    return icio.aggregate(_cached("1995-2000_SML.zip"))


def test_real_2000_has_80_economies_and_row(real):
    assert len(real) == 81 and "ROW" in real and {"USA", "CHN", "DEU", "TWN", "ZAF"} <= set(real)


def test_real_2000_coefficients_and_shares(real):
    for code, c in real.items():
        assert np.isfinite(c.io).all() and (c.io >= 0).all(), code
        assert (c.io.sum(axis=0) < 1).all(), code
        assert c.final_demand_shares.sum() == pytest.approx(1.0, abs=1e-12), code
        assert (c.final_demand_shares > 0).all(), code
        assert (c.va >= 0).all() and (c.output >= c.va).all(), code


def test_real_2000_value_added_closes_the_identity(real):
    for code, c in real.items():
        np.testing.assert_allclose(c.va + c.io.sum(axis=0) * c.output, c.output, rtol=1e-9, atol=1e-9, err_msg=code)


def test_real_2000_totals_match_the_file(real):
    total = sum(c.output.sum() for c in real.values())
    # The OUT row of the file, summed over its industry columns (the final demand columns hold zeros there).
    with zipfile.ZipFile(_cached("1995-2000_SML.zip")) as z, z.open("2000_SML.csv") as f:
        row = next(line for line in f if line.startswith(b"OUT,")).decode().split(",")[1:]
    assert total == pytest.approx(sum(float(x) for x in row), rel=1e-9)
    assert 5e7 < total < 1e8
    assert 8e6 < real["USA"].output.sum() < 2.5e7


def test_real_2000_goods_look_right(real):
    usa, deu = real["USA"], real["DEU"]
    assert usa.output[9] > usa.output[:7].sum()  # services dominate
    assert usa.io[5, 5] > 0.05 and usa.io[1, 5] > 0.001  # chemicals use chemicals and oil
    assert real["SAU"].va[1] > real["SAU"].va[6]  # oil and gas against machinery
    assert deu.final_demand_shares[9] > 0.3


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
    "wdi_NV.AGR.TOTL.ZS.zip",
    "wdi_NV.IND.TOTL.ZS.zip",
    "wdi_NV.IND.MANF.ZS.zip",
    "wdi_NV.SRV.TOTL.ZS.zip",
    "wdi_NY.GDP.MKTP.CD.zip",
    "adb_kidb_NY_GDP_PCAP_CD_COO.csv",
    "mpd2023_web.xlsx",
    "ppp_2000_1km_Aggregated.tif",
]


def _pack_table(name: str):
    from pyarrow import ipc

    path = fetch.CACHE.parent / "packs" / "2000" / name
    if not path.exists():
        pytest.skip(f"skipping: no {name} in {path.parent}; build the pack with `uv run python -m ceteris.packs.build`")
    return ipc.open_file(str(path)).read_all()


@pytest.fixture(scope="module")
def pack():
    for name in NAMES:
        _cached(name)
    countries = _pack_table("countries.arrow").to_pylist()
    provinces = _pack_table("provinces.arrow")["country"].to_pylist()
    admin0 = fetch.path_of("ne_10m_admin_0_countries.zip")
    population = people.province_population(fetch.path_of("ppp_2000_1km_Aggregated.tif"), _pack_table("cells.arrow"))
    worldpop: dict[str, float] = {}
    for c, p in zip(provinces, population):
        worldpop[c] = worldpop.get(c, 0.0) + float(p)
    codes, _ = people.read_codes(admin0)
    return {r["code"]: r["sovereign"] for r in countries}, codes, people.load_observed(fetch.CACHE, admin0), worldpop


@pytest.fixture(scope="module")
def wdi():
    for name in NAMES:
        _cached(name)
    return icio.load_wdi(fetch.CACHE)


@pytest.fixture(scope="module")
def imputed(real, pack, wdi):
    countries, iso, obs, worldpop = pack
    return icio.impute(countries, real, wdi, obs, iso, worldpop)


UNINHABITED = {"ATC", "ATF", "CLP", "CSI", "HMD", "PGA", "SGS", "UMI", "PCN", "VAT", "IOT"}


def test_real_imputation_covers_every_pack_country(imputed, real, pack):
    countries, iso, _, _ = pack
    assert set(imputed) == set(countries) and len(imputed) == 242
    own = {c for c, v in imputed.items() if v.donor is None}
    assert own == {c for c in countries if iso.get(c, c) in set(real) - {"ROW"}}
    assert len(own) == 80
    for code, c in imputed.items():
        assert np.isfinite(c.io).all() and (c.io.sum(axis=0) < 1).all(), code
        assert c.final_demand_shares.sum() == pytest.approx(1.0, abs=1e-12), code
        assert np.isfinite(c.va).all() and (c.va >= 0).all(), code
        assert c.va_basis in icio.BASES, code
        np.testing.assert_allclose(c.va + c.io.sum(axis=0) * c.output, c.output, rtol=1e-9, atol=1e-9, err_msg=code)


def test_real_donors_stay_inside_the_resolved_region(imputed, pack):
    _, _, obs, _ = pack
    placed = 0
    for code, c in imputed.items():
        if c.donor and c.va_basis != "parent" and c.region:
            assert obs.region[c.donor] == c.region, code
            placed += 1
    assert placed > 140
    assert all(v.region for c, v in imputed.items() if c in ("SAH", "CYN", "SOL"))


def test_real_populated_territories_have_value_added(imputed):
    populated = ["SOL", "SAH", "CYN", "JEY", "GGY", "ALD", "AIA", "BLM", "COK", "WLF", "SPM", "MSR", "SHN"]
    for code in populated:
        assert imputed[code].va.sum() > 0, code
    assert {c for c, v in imputed.items() if v.va_basis == "none"} <= UNINHABITED | {"IOA", "NFK", "NIU"}
    assert imputed["SAH"].region == "Middle East & North Africa"
    assert imputed["CYN"].region == "Europe & Central Asia" and imputed["SOL"].region == "Sub-Saharan Africa"


def test_real_territories_inside_a_parent_take_their_share_of_it(imputed, real):
    for territory, parent in icio.INSIDE.items():
        t, p = imputed[territory], imputed[parent]
        assert t.va_basis == "parent" and 0 < p.shed < 0.5 and t.va.sum() > 0, territory
        assert t.va.sum() / (t.va.sum() + p.va.sum()) == pytest.approx(p.shed, rel=1e-9)
    assert imputed["FIN"].va.sum() < real["FIN"].va.sum() and imputed["CYP"].va.sum() < real["CYP"].va.sum()


def test_real_wdi_basis_value_added_is_gdp_times_the_shares(imputed, wdi, pack):
    _, iso, _, _ = pack
    checked = 0
    for code, c in imputed.items():
        own = iso.get(code, code)
        if c.va_basis == "wdi" and own in wdi.gdp:  # split economies sum several
            share = wdi.agriculture[own] + wdi.industry[own] + wdi.services[own]
            assert c.va.sum() * 1e6 == pytest.approx(wdi.gdp[own] * share, rel=1e-9), code
            checked += 1
    assert checked > 80


def test_real_donor_basis_levels_are_scaled_by_income(imputed):
    donor_basis = {c: v for c, v in imputed.items() if v.va_basis == "donor"}
    assert "PRK" in donor_basis and donor_basis["PRK"].income_source == "maddison"
    # North Korea at Cambodia's Maddison income (1,657 and 1,659 dollars of 2011): about 7 billion USD, not the 43 billion of an
    # unscaled Thai level.
    assert 5e3 < donor_basis["PRK"].va.sum() < 3e4
    assert donor_basis["COK"].income_source == "adb"
    for code, v in donor_basis.items():
        assert v.income_source in ("pwt", "wdi", "adb", "maddison", "none"), code


GDP_WITHOUT_SHARES = {
    "ARM", "ASM", "BHR", "CAF", "DJI", "GNQ", "GUM", "IMN", "LCA", "LIE", "MCO", "MNP", "NRU", "SMR", "SOM", "VEN", "VIR",
}  # fmt: skip


def test_real_gdp_without_shares_uses_the_icio_median_va_over_gdp(imputed, real, wdi, pack):
    _, iso, _, _ = pack
    scope = set()
    ratio = icio.median_va_over_gdp(real, wdi)
    assert 0.90 < ratio < 0.96
    for code, c in imputed.items():
        gdp, shares = icio._sector_shares(wdi, [iso.get(code, code)])
        if c.va_basis == "wdi_gdp" and gdp > 0 and all(shares[k] is None for k in ("agriculture", "industry", "services")):
            scope.add(code)
            # SOM gave 17.9 percent of its VA to Somaliland.
            assert c.va.sum() * 1e6 == pytest.approx(gdp * ratio * (1 - c.shed), rel=1e-9), code
    assert scope == GDP_WITHOUT_SHARES
    # Somalia is no longer scaled by Congo's 0.43 (it was 0.356 of GDP).
    som = imputed["SOM"]
    assert som.va.sum() / (1 - som.shed) * 1e6 / wdi.gdp["SOM"] > 0.9


def test_real_venezuela_is_not_left_empty(imputed):
    # WDI reports zero sector shares for 2000, so GDP (117 billion USD) times the ICIO median VA over GDP.
    assert imputed["VEN"].va_basis == "wdi_gdp" and 80e3 < imputed["VEN"].va.sum() < 130e3
