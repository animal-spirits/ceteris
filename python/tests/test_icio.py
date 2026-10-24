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


def _table(economies=("AAA", "BBB"), invnt=None, deplete=None, seed=1) -> tuple[str, dict[str, np.ndarray]]:
    """An ICIO-shaped CSV (industry block, final demand block, OUT; rows then TLS, VA, OUT) and its parts."""
    rng = np.random.default_rng(seed)
    cols = [f"{e}_{i}" for e in economies for i in INDUSTRIES]
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
        assert got[code].va.sum() == pytest.approx(parts["va"][block].sum(), rel=1e-9)
        for g in range(goods.N_GOODS):
            member = [i for i, x in enumerate(GOOD) if x == g]
            assert got[code].output[g] == pytest.approx(parts["out"][block][member].sum() if member else 0.0, rel=1e-9)


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


def _country(output, va, final=None, io=None) -> CountryIO:
    return CountryIO(
        output=np.asarray(output, float),
        va=np.asarray(va, float),
        io=np.eye(10) * 0.1 if io is None else io,
        final_demand_shares=np.full(10, 0.1) if final is None else np.asarray(final, float),
    )


DONOR_VA = [10, 4, 1, 6, 2, 1, 1, 3, 2, 70]


def _donors():
    va = np.array(DONOR_VA, float)
    out = va * np.array([2, 2, 2, 2, 2, 2, 2, 2, 2, 3.0])
    a = _country(out, va, io=np.full((10, 10), 0.01))
    b = _country(out, va, final=[0.5, 0, 0, 0.5, 0, 0, 0, 0, 0, 0], io=np.full((10, 10), 0.02))
    c = _country(out, va, io=np.full((10, 10), 0.03))
    return {"AAA": a, "BBB": b, "CCC": c}


DONOR_GDP = {"AAA": 100e6, "BBB": 125e6, "CCC": 200e6}  # USD; donor VA is 100 million USD, so VA over GDP is 1, 0.8, 0.5
DONOR_POP = {"AAA": 1e6, "BBB": 5e6, "CCC": 1e6}


def _obs(**kw) -> Observed:
    return Observed(
        region={"AAA": "R1", "BBB": "R1", "CCC": "R2", "XXX": "R1", "YYY": "R2", **kw.get("region", {})},
        gdp_per_capita={"AAA": 1000.0, "BBB": 20000.0, "CCC": 10000.0, "XXX": 11000.0, "YYY": 11000.0},
        population={**DONOR_POP, "XXX": 1e6, "YYY": 2e6, "ZZZ": 3e6, **kw.get("population", {})},
    )


def _wdi(**kw) -> Wdi:
    full = {
        "gdp": {**DONOR_GDP, "XXX": 1e9},
        "agriculture": {"XXX": 0.1},
        "industry": {"XXX": 0.35},
        "manufacturing": {"XXX": 0.2},
        "services": {"XXX": 0.45},
    }
    return Wdi(**{**full, **kw})


def test_imputed_country_uses_nearest_donor():
    donors = _donors()
    got = icio.impute({"XXX": "XXX", "YYY": "YYY"}, donors, _wdi(gdp={**DONOR_GDP, "XXX": 1e9, "YYY": 1e9}), _obs(), {})
    # XXX: same region as AAA and BBB, closest log income among them is BBB, although CCC (other region) is closer overall.
    assert got["XXX"].donor == "BBB" and got["YYY"].donor == "CCC"
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
    # Output keeps the donor's output per unit of VA of each good.
    np.testing.assert_allclose(got.output, got.va * np.array([2, 2, 2, 2, 2, 2, 2, 2, 2, 3.0]), rtol=1e-12)
    assert got.va_basis == "wdi"


def test_missing_sector_share_takes_the_donor_structure_at_the_wdi_gdp_level():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(services={}), _obs(), {})["XXX"]
    # GDP 1000 million USD times the donor's VA over GDP (100 over 125).
    assert got.va_basis == "wdi_gdp" and got.donor == "BBB"
    assert got.va.sum() == pytest.approx(800.0)
    np.testing.assert_allclose(got.va / got.va.sum(), np.array(DONOR_VA) / 100, rtol=1e-12)


def test_industry_below_manufacturing_is_not_a_usable_split():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(industry={"XXX": 0.1}), _obs(), {})["XXX"]
    assert got.va_basis == "wdi_gdp"


def test_all_zero_sector_shares_count_as_missing():
    zero = {"agriculture": {"XXX": 0.0}, "industry": {"XXX": 0.0}, "manufacturing": {"XXX": 0.0}, "services": {"XXX": 0.0}}
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(**zero), _obs(), {})["XXX"]
    assert got.va_basis == "wdi_gdp" and got.va.sum() > 0


def test_missing_gdp_takes_population_times_donor_va_per_person():
    got = icio.impute({"XXX": "XXX"}, _donors(), _wdi(gdp=DONOR_GDP), _obs(), {})["XXX"]
    # Donor VA 100 million USD for 5 million people, so 20 USD per person; XXX has 1 million people.
    assert got.va_basis == "donor" and got.donor == "BBB"
    assert got.va.sum() == pytest.approx(20.0)
    np.testing.assert_allclose(got.va / got.va.sum(), np.array(DONOR_VA) / 100, rtol=1e-12)


def test_country_without_population_or_gdp_has_no_value_added():
    got = icio.impute({"ZZZ": "ZZZ"}, _donors(), _wdi(gdp=DONOR_GDP), _obs(region={"ZZZ": "R1"}, population={"ZZZ": 0.0}), {})[
        "ZZZ"
    ]
    assert got.va_basis == "none" and got.va.sum() == 0 and got.output.sum() == 0
    assert got.donor is not None


def test_split_economies_sum_gdp_and_weight_shares_by_gdp():
    wdi = Wdi(
        gdp={**DONOR_GDP, "PA": 1e9, "PB": 3e9},
        agriculture={"PA": 0.2, "PB": 0.0},
        industry={"PA": 0.3, "PB": 0.3},
        manufacturing={"PA": 0.1, "PB": 0.1},
        services={"PA": 0.4, "PB": 0.6},
    )
    obs = _obs(region={"PA": "R1"}, population={"PA": 1e6, "PB": 1e6})
    got = icio.impute({"XXX": "XXX"}, _donors(), wdi, obs, {}, parts={"XXX": ("PA", "PB")})["XXX"]
    assert got.va.sum() == pytest.approx(4000 * (0.2 * 1 / 4 + 0.3 + 0.4 * 1 / 4 + 0.6 * 3 / 4))
    assert got.va[0] == pytest.approx(4000 * 0.05)


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
        assert (c.va >= 0).all() and (c.output >= c.va - 1e-6).all(), code


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


def _pack_inputs():
    from pyarrow import ipc

    pack = fetch.CACHE.parent / "packs" / "2000" / "countries.arrow"
    if not pack.exists():
        pytest.skip(f"skipping: no countries.arrow in {pack.parent}; build the pack with `uv run python -m ceteris.packs.build`")
    for name in people_names():
        _cached(name)
    countries = {r["code"]: r["sovereign"] for r in ipc.open_file(str(pack)).read_all().to_pylist()}
    admin0 = fetch.path_of("ne_10m_admin_0_countries.zip")
    return countries, people.read_codes(admin0)[0], people.load_observed(fetch.CACHE, admin0)


def people_names():
    return [
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
    ]


@pytest.fixture(scope="module")
def imputed(real):
    countries, iso, obs = _pack_inputs()
    return icio.impute(countries, real, icio.load_wdi(fetch.CACHE), obs, iso), countries, iso, obs


def test_real_imputation_covers_every_pack_country(imputed, real):
    got, countries, iso, _ = imputed
    assert set(got) == set(countries) and len(got) == 242
    own = {c for c, v in got.items() if v.donor is None}
    assert own == {c for c in countries if iso.get(c, c) in set(real) - {"ROW"}}
    assert len(own) == 80
    for code, c in got.items():
        assert np.isfinite(c.io).all() and (c.io.sum(axis=0) < 1).all(), code
        assert c.final_demand_shares.sum() == pytest.approx(1.0, abs=1e-12), code
        assert np.isfinite(c.va).all() and (c.va >= 0).all(), code
        assert c.va_basis in icio.BASES, code


def test_real_imputed_donors_share_the_region_when_one_exists(imputed):
    got, _, iso, obs = imputed
    donors = {c: v.donor for c, v in got.items() if v.donor}
    assert len(donors) == 162
    same = sum(obs.region.get(iso.get(c, c)) == obs.region.get(d) for c, d in donors.items())
    assert same > 0.7 * len(donors)
    assert got["ZAF"].donor is None and got["PRI"].donor is not None


def test_real_imputed_value_added_follows_wdi_gdp(imputed):
    got = imputed[0]
    wdi = icio.load_wdi(fetch.CACHE)
    for code in ("PRI", "ISL", "KEN", "BGR", "PRK"):
        v = got[code]
        gdp = wdi.gdp.get(code)
        if v.va_basis == "wdi":
            assert 0.7 < v.va.sum() * 1e6 / gdp < 1.05
    assert sum(v.va_basis == "wdi" for v in got.values()) > 60
    assert all(v.va.sum() > 0 for v in got.values() if v.va_basis not in ("none",))
