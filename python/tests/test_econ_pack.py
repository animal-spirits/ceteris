import json
import math
import os
import re
import zipfile
from pathlib import Path

import numpy as np
import pyarrow as pa
import pytest
from pyarrow import ipc

from ceteris.calibrate import gravity
from ceteris.packs import fetch
from ceteris.packs.econ import build as econ_build
from ceteris.packs.econ import gateways, goods, people, tariffs, theta

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = Path(os.environ.get("CETERIS_FIXTURE_DIR", ROOT / "tests" / "golden" / "p2-fixture"))

# ICIO 2025 edition, 50 industries (ReadMe_ICIO_small.xlsx, "Areas and Economic Activities").
ICIO_2025 = [
    "A01",
    "A02",
    "A03",
    "B05",
    "B06",
    "B07",
    "B08",
    "B09",
    "C10T12",
    "C13T15",
    "C16",
    "C17_18",
    "C19",
    "C20",
    "C21",
    "C22",
    "C23",
    "C24A",
    "C24B",
    "C25",
    "C26",
    "C27",
    "C28",
    "C29",
    "C301",
    "C302T309",
    "C31T33",
    "D",
    "E",
    "F",
    "G",
    "H49",
    "H50",
    "H51",
    "H52",
    "H53",
    "I",
    "J58T60",
    "J61",
    "J62_63",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
]


def test_icio_2025_has_fifty_industries():
    assert len(ICIO_2025) == 50 and len(set(ICIO_2025)) == 50


def test_every_icio_industry_mapped_once():
    missing = [c for c in ICIO_2025 if c not in goods.ICIO_MAP]
    assert not missing
    assert all(0 <= goods.ICIO_MAP[c] < goods.N_GOODS for c in ICIO_2025)
    assert set(goods.ICIO_MAP.values()) - {goods.SPLIT} == set(range(goods.N_GOODS))


def test_icio_map_follows_the_spec_table():
    expected = {
        0: ["A01", "A02", "A03"],
        1: ["B06", "B09"],
        2: ["B05", "B07", "B08"],
        3: ["C10T12"],
        4: ["C13T15", "C16", "C17_18", "C31T33"],
        5: ["C19", "C20", "C21", "C22", "C23", "C24A", "C24B", "C25"],
        6: ["C26", "C27", "C28", "C29", "C301", "C302T309"],
        7: ["D", "E"],
        8: ["F"],
        9: [c for c in ICIO_2025 if c[0] in "GHIJKLMNOPQRST"],
    }
    for good, codes in expected.items():
        assert {c: goods.ICIO_MAP[c] for c in codes} == {c: good for c in codes}


def test_older_edition_names_map_to_the_same_goods():
    assert goods.ICIO_MAP["A01_02"] == 0
    assert goods.ICIO_MAP["B07_08"] == 2
    assert goods.ICIO_MAP["C24"] == 5
    assert goods.ICIO_MAP["C30"] == 6


def test_combined_coal_oil_gas_is_a_split_marker():
    assert goods.ICIO_MAP["B05_06"] == goods.SPLIT
    assert goods.SPLIT not in range(goods.N_GOODS)


def test_traded_and_local_goods():
    assert goods.TRADED == tuple(range(7))
    assert len(goods.NAMES) == goods.N_GOODS


def test_division_to_good_covers_isic_goods_divisions():
    assert goods.good_of_division("01") == 0
    assert goods.good_of_division("05") == 2
    assert goods.good_of_division("06") == 1
    assert goods.good_of_division("24") == 5
    assert goods.good_of_division("30") == 6
    assert goods.good_of_division("35") == 7
    assert goods.good_of_division("45") == 9
    assert goods.good_of_division("04") is None


def test_cpa_codes_map_to_goods_through_their_division():
    assert goods.good_of_cpa("A01") == 0
    assert goods.good_of_cpa("B05") == 2 and goods.good_of_cpa("B06") == 1 and goods.good_of_cpa("B072") == 2
    assert goods.good_of_cpa("C24A") == 5 and goods.good_of_cpa("C254") == 5
    assert goods.good_of_cpa("C302_309") == 6 and goods.good_of_cpa("C262") == 6
    assert goods.good_of_cpa("C32X325") == 4 and goods.good_of_cpa("C325") == 4
    assert goods.good_of_cpa("D35") == 7 and goods.good_of_cpa("E38") == 7
    assert goods.good_of_cpa("J58") == 9 and goods.good_of_cpa("R90") == 9
    assert goods.good_of_cpa("_X") is None


def test_hs_goods_shares_put_each_line_in_one_good():
    shares = goods.hs_goods_shares({"270111": "B05", "270900": "B06", "270120": "C19", "999999": "_X"})
    assert shares == {"270111": {2: 1.0}, "270900": {1: 1.0}, "270120": {5: 1.0}}


def _key(path: Path, lines: dict[str, str], version: int = 0) -> Path:
    """An xlsx shaped like the OECD BTiGE key: HS-version, HS-code, CPA, ENDUSE; a version-1 decoy row."""
    strings = ["HS-version", "HS-code", "CPA", "ENDUSE", "INT"]

    def s(text: str) -> int:
        if text not in strings:
            strings.append(text)
        return strings.index(text)

    def row(r: int, cells: list[tuple[str, object]]) -> str:
        out = "".join(
            f'<c r="{c}{r}"><v>{v}</v></c>' if isinstance(v, int) else f'<c r="{c}{r}" t="s"><v>{s(v)}</v></c>' for c, v in cells
        )
        return f'<row r="{r}">{out}</row>'

    rows = [row(1, [("A", "HS-version"), ("B", "HS-code"), ("C", "CPA"), ("D", "ENDUSE")])]
    body = [*((version, hs, cpa) for hs, cpa in lines.items()), (1, next(iter(lines)), "_X")]
    rows += [row(i, [("A", v), ("B", hs), ("C", cpa), ("D", "INT")]) for i, (v, hs, cpa) in enumerate(body, start=2)]
    main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    with zipfile.ZipFile(path, "w") as z:
        z.writestr(
            "xl/workbook.xml",
            f'<workbook xmlns="{main}" xmlns:r="{rel}"><sheets><sheet name="{goods.KEY_SHEET}" sheetId="1" r:id="rId1"/></sheets></workbook>',
        )
        z.writestr(
            "xl/_rels/workbook.xml.rels",
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>',
        )
        z.writestr("xl/sharedStrings.xml", f'<sst xmlns="{main}">' + "".join(f"<si><t>{t}</t></si>" for t in strings) + "</sst>")
        z.writestr("xl/worksheets/sheet1.xml", f'<worksheet xmlns="{main}"><sheetData>{"".join(rows)}</sheetData></worksheet>')
    return path


def test_read_hs_cpa_keeps_hs_1988_92_lines(tmp_path):
    key = _key(tmp_path / "key.xlsx", {"010111": "A01", "270900": "B06"})
    assert goods.read_hs_cpa(key) == {"010111": "A01", "270900": "B06"}


def test_read_hs_cpa_moves_gold_to_basic_metals_and_books_to_printing(tmp_path):
    lines = {"710812": "B072", "710813": "C24B", "490199": "J58", "852410": "J59_60", "271600": "D35", "970110": "R90"}
    got = goods.read_hs_cpa(_key(tmp_path / "key.xlsx", lines))
    assert got == {"710812": "C24B", "710813": "C24B", "490199": "C18", "852410": "C18", "271600": "D35", "970110": "R90"}
    shares = goods.hs_goods_shares(got)
    assert shares["710812"] == {5: 1.0} and shares["490199"] == {4: 1.0} and shares["852410"] == {4: 1.0}
    assert shares["271600"] == {7: 1.0} and shares["970110"] == {9: 1.0}


def test_weights_from_baci_pins_tonnes_per_usd(tmp_path):
    # One HS line per traded good: hs6 -> CPA 2.1 activity in divisions 01, 06, 05, 10, 13, 20, 26.
    lines = {
        "010121": "A01",
        "270900": "B06",
        "270112": "B05",
        "020110": "C10",
        "520100": "C13",
        "290110": "C20",
        "850110": "C261",
    }
    # BACI v is thousands of USD, q is tonnes; k carries no leading zero. t,i,j,k,v,q.
    rows = [
        "2000,1,2,10121,2,3",  # good 0: 3 t for 2000 USD
        "2000,1,3,10121,3,1",  # good 0: 1 t for 3000 USD
        "2000,2,3,10121,100,",  # good 0: no q, out of numerator and denominator
        "2000,1,2,10121,0.5,",  # good 0: no q, fractional value
        "2000,1,2,270900,8,2",
        "2000,1,2,270112,4,40",
        "2000,1,2,20110,5,10",
        "2000,1,2,520100,10,1",
        "2000,1,2,290110,20,4",
        "2000,1,2,850110,50,1",
        "2000,1,2,999999,7,7",  # absent from the key
    ]
    baci = tmp_path / "baci.zip"
    with zipfile.ZipFile(baci, "w") as z:
        z.writestr("BACI_HS92_Y2000_V202601.csv", "t,i,j,k,v,q\n" + "\n".join(rows) + "\n")
        z.writestr("BACI_HS92_Y2001_V202601.csv", "t,i,j,k,v,q\n2001,1,2,10121,1,999\n")

    w = goods.weights_from_baci(baci, _key(tmp_path / "key.xlsx", lines))

    # tonnes / (v x 1000): the 100 and 0.5 kUSD rows without q must not enter the denominator.
    expected = [4 / 5000, 2 / 8000, 40 / 4000, 10 / 5000, 1 / 10000, 4 / 20000, 1 / 50000]
    assert w[:7] == pytest.approx(expected, rel=1e-12)
    assert (w[7:] == 0).all()


def _isic_divisions(span: str) -> tuple[str, ...]:
    """ISIC Rev 3 codes of Table A.1 ("1-5", "36 -37", "351 - 359") as two-digit divisions present in Rev 3."""
    lo, _, hi = (x.strip() for x in span.partition("-"))
    lo, hi = lo[:2].zfill(2), (hi or lo)[:2].zfill(2)
    rev3 = {"01", "02", "05", *map(str, range(10, 38))}
    return tuple(f"{d:02d}" for d in range(int(lo), int(hi) + 1) if f"{d:02d}" in rev3)


def test_caliendo_parro_values_transcribed():
    """The table in `theta.py` is the one ADR 0023 transcribes from Caliendo and Parro (2015), with the page."""
    adr = (ROOT / "adr" / "0023-trade-elasticity-classes.md").read_text()
    assert "Table 1" in adr and "p. 18" in adr and "99% sample" in adr
    rows = re.findall(r"^\| (\d+) \| ([A-Za-z. ]+?) \| ([\d -]+?) \| ([\d.]+) \| \(([\d.]+)\) \|$", adr, re.MULTILINE)
    assert len(rows) == 20
    transcribed = {name: (_isic_divisions(span), float(t)) for _, name, span, t, _ in rows}
    assert transcribed == theta.CALIENDO_PARRO
    assert min(t for _, t in transcribed.values()) == 0.39 and max(t for _, t in transcribed.values()) == 64.85


def test_theta_by_good_weighted_by_trade(tmp_path):
    # Two light-manufacturing lines (Textile 8.10, Wood 11.50) and one food line (2.62); one line out of the key.
    lines = {"520100": "C13", "440310": "C16", "020110": "C10"}
    rows = [
        "2000,1,2,520100,1,",  # no q: still weighs by value
        "2000,1,3,520100,2,5",
        "2000,1,2,440310,1,2",
        "2000,1,2,20110,4,1",
        "2000,1,2,999999,100,1",
    ]
    baci = tmp_path / "baci.zip"
    with zipfile.ZipFile(baci, "w") as z:
        z.writestr("BACI_HS92_Y2000_V202601.csv", "t,i,j,k,v,q\n" + "\n".join(rows) + "\n")
        z.writestr("BACI_HS92_Y2001_V202601.csv", "t,i,j,k,v,q\n2001,1,2,440310,1000,1\n")

    t = theta.caliendo_parro_by_good(baci, _key(tmp_path / "key.xlsx", lines))

    assert t.shape == (goods.N_GOODS,)
    assert t[4] == pytest.approx((3 * 8.10 + 1 * 11.50) / 4, rel=1e-12)
    assert t[3] == pytest.approx(2.62, rel=1e-12)
    assert (t[[0, 1, 2, 5, 6]] == 0).all() and (t[7:] == 0).all()


def test_classes_follow_rule():
    # Sorted 2, 2.9 | 4, 5.9 | 8, 9, 11: the only split into 3 within a factor 1.5.
    t = np.array([2.0, 4.0, 5.9, 2.9, 8.0, 9.0, 11.0])
    v = np.array([1.0, 1.0, 3.0, 1.0, 1.0, 1.0, 2.0])
    of, ct = theta.classes(t, v)
    assert of.tolist() == [0, 1, 1, 0, 2, 2, 2]
    np.testing.assert_allclose(ct, [2.45, (4.0 + 3 * 5.9) / 4, (8.0 + 9.0 + 22.0) / 4], rtol=1e-12)
    for c in range(len(ct)):
        members = t[of == c]
        assert members.max() / members.min() <= theta.CLASS_SPREAD
    # Fewer distinct values still give 3 classes, the split with the smallest largest spread.
    of, _ = theta.classes(np.array([2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0]), np.ones(7))
    assert of.tolist() == [0, 1, 1, 1, 2, 2, 2]  # the first such split in order
    # A spread of exactly 1.5 is inside the bound.
    of, _ = theta.classes(np.array([1.0, 1.5, 3.0, 4.5, 9.0, 13.5, 13.5]), np.ones(7))
    assert of.tolist() == [0, 0, 1, 1, 2, 2, 2]
    with pytest.raises(ValueError, match="5 classes"):
        theta.classes(np.array([1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]), np.ones(7))


def test_fixture_theta_follows_the_classes():
    """`TradeParams::default()` as exported: every traded good at its class's theta, local goods 0."""
    if not (FIXTURE / "goods.arrow").exists():
        pytest.skip(f"skipping: no export in {FIXTURE}")
    g = ipc.open_file(str(FIXTURE / "goods.arrow")).read_all()
    if "theta_class" not in g.column_names:
        pytest.skip(f"skipping: {FIXTURE} predates export format 3")
    t, of, traded = (g[c].to_numpy() for c in ("theta", "theta_class", "traded"))
    assert (t[~traded] == 0).all() and (of[~traded] == 0).all()
    classes = sorted(set(of[traded].tolist()))
    assert classes == list(range(len(classes))) and 3 <= len(classes) <= 5
    by_class = [t[traded][of[traded] == c] for c in classes]
    assert all(len(set(x.tolist())) == 1 for x in by_class)
    assert [x[0] for x in by_class] == sorted(x[0] for x in by_class)


# Tests below need the downloaded BACI and concordances; CI is data-free.
def _cached(name: str):
    path = fetch.path_of(name)
    if not path.exists():
        pytest.skip(f"skipping: no {name} in {path.parent}; fetch it with `uv run python -m ceteris.packs.fetch`")
    return path


@pytest.fixture(scope="module")
def weights():
    return goods.weights_from_baci(_cached("BACI_HS92_V202601.zip"), _cached(econ_build.HS_KEY))


def test_weights_positive_for_traded(weights):
    assert weights.shape == (goods.N_GOODS,)
    assert (weights[:7] > 0).all() and np.isfinite(weights).all()
    # Tonnes per USD: a dropped or doubled x1000 moves every weight by 1000x, out of this band.
    assert ((weights[:7] > 1e-5) & (weights[:7] < 1e-1)).all()


def test_local_goods_weigh_nothing(weights):
    assert (weights[7:] == 0).all()


def test_bulk_heavier_than_machinery(weights):
    assert weights[2] > 10 * weights[6]


@pytest.fixture(scope="module")
def caliendo_parro():
    return theta.caliendo_parro_with_value(_cached("BACI_HS92_V202601.zip"), _cached(econ_build.HS_KEY))


def test_caliendo_parro_by_good_from_baci(caliendo_parro):
    """Theta per good to two decimals through the adapted OECD key, and its classes (ADR 0050 amendment). The
    WITS and UNSD route that ADR 0023 first used (goods 4 to 6 at 6.73, 12.64, 5.33; classes 2.62, 5.88, 12.55) was
    replaced by the key."""
    t, v = caliendo_parro
    np.testing.assert_allclose(t, [9.11, 13.53, 13.53, 2.62, 9.15, 10.57, 5.19, 0, 0, 0], atol=0.005)
    assert (v[:7] > 1e11).all() and (v[7:] == 0).all()
    of, ct = theta.classes(t[:7], v[:7])
    assert of.tolist() == [2, 2, 2, 0, 2, 2, 1]
    np.testing.assert_allclose(ct, [2.62, 5.19, 10.57], atol=0.005)
    assert math.log(ct[-1] / ct[0]) > math.log(theta.CLASS_SPREAD)


def test_fixture_defaults_match_the_calibration_constants():
    """The literals of ADR 0023 as exported from `TradeParams::default()` (pinned to the export by p2_acceptance),
    against `gravity.CLASSES` and `THETA_DEFAULT`. Needs no BACI; the estimator test below links the literals to
    `theta.classes`."""
    trade = json.loads((FIXTURE / "params.json").read_text())["trade"]
    assert trade["theta_class"] == [2, 2, 2, 0, 2, 2, 1, 0, 0, 0]
    assert trade["theta"] == [10.57, 10.57, 10.57, 2.62, 10.57, 10.57, 5.19, 0.0, 0.0, 0.0]
    of = trade["theta_class"][:7]
    assert gravity.CLASSES == tuple(tuple(g for g in range(7) if of[g] == c) for c in range(max(of) + 1))
    assert gravity.THETA_DEFAULT == tuple(trade["theta"][members[0]] for members in gravity.CLASSES)


def test_rust_theta_defaults_follow_the_estimator(caliendo_parro):
    """`TradeParams::default()` is pinned to the fixture's params.json by the p2_acceptance staleness check, so
    reading that file ties the Rust defaults, and the calibration constants, to the ADR 0023 rule on the pack."""
    t, v = caliendo_parro
    of, ct = theta.classes(t[:7], v[:7])
    trade = json.loads((FIXTURE / "params.json").read_text())["trade"]
    n = goods.N_GOODS
    assert trade["theta_class"] == of.tolist() + [0] * (n - 7)
    np.testing.assert_allclose(trade["theta"], [ct[c] for c in of] + [0.0] * (n - 7), atol=0.005)
    assert gravity.CLASSES == tuple(tuple(int(g) for g in np.flatnonzero(of == c)) for c in range(len(ct)))
    np.testing.assert_allclose(gravity.THETA_DEFAULT, ct, atol=0.005)


def test_every_traded_activity_of_the_key_has_a_sector_estimate():
    key = goods.read_hs_cpa(_cached(econ_build.HS_KEY))
    traded = {c for c in key.values() if goods.good_of_cpa(c) in goods.TRADED}
    assert traded <= set(theta.REV3_OF_CPA)
    assert {d for _, (divisions, _) in theta.CALIENDO_PARRO.items() for d in divisions} >= set(theta.REV3_OF_CPA.values())
    # Coal, crude oil and natural gas sit wholly in their goods (the principal-link rule of the old route).
    shares = goods.hs_goods_shares(key)
    assert shares["270112"] == {2: 1.0} and shares["270900"] == {1: 1.0} and shares["271121"] == {1: 1.0}


# Tariffs (Task 17)


def _observed(**region) -> people.Observed:
    return people.Observed(region=region, gdp_per_capita={k: 1000.0 * (i + 1) for i, k in enumerate(region)})


def test_tariffs_zero_inside_agreements_and_on_the_diagonal():
    codes = ["AAA", "BBB", "CCC"]
    rates = [{"AAA": 0.10, "BBB": 0.05, "CCC": 0.20}, {"AAA": 0.08, "BBB": 0.04, "CCC": 0.12}]
    t = tariffs.table(rates, {(0, 1), (1, 0)}, codes, {}, _observed(AAA="r", BBB="r", CCC="r"))
    assert t.rate.shape == (3, 3, 2)
    assert (t.rate[np.arange(3), np.arange(3)] == 0).all()
    assert (t.rate[0, 1] == 0).all() and (t.rate[1, 0] == 0).all()
    np.testing.assert_array_equal(t.rate[0, 2], [0.10, 0.08])
    np.testing.assert_array_equal(t.rate[2, 0], [0.20, 0.12])
    assert not t.imputed.any() and t.agreement[0, 1] and not t.agreement[0, 2]


def test_tariff_rates_skip_implausible_years():
    series = {"DJI": {1998: 100.13, 1999: 534.76, 2002: 20.97}, "EGY": {1999: 12.53, 2000: 12.88}, "XXX": {1990: 5.0}}
    assert tariffs.plausible_rates(series) == {"DJI": 0.2097, "EGY": 0.1288}


def test_tariffs_without_a_rate_take_the_nearest_donor_flagged_imputed():
    codes = ["AAA", "BBB", "CCC"]
    rates = [{"AAA": 0.10, "BBB": 0.30}, {"AAA": 0.08, "BBB": 0.25}]
    obs = people.Observed(
        region={"AAA": "r", "BBB": "r", "CCC": "r"}, gdp_per_capita={"AAA": 1000.0, "BBB": 9000.0, "CCC": 8000.0}
    )
    t = tariffs.table(rates, set(), codes, {}, obs)
    assert t.imputed.tolist() == [False, False, True] and t.donor[2] == "BBB"
    np.testing.assert_array_equal(t.rate[2, 0], [0.30, 0.25])


def test_dependencies_trade_free_with_their_sovereign_and_take_its_rate():
    codes = ["AAA", "BBB", "TER"]
    rates = [{"AAA": 0.10, "BBB": 0.30}, {"AAA": 0.08, "BBB": 0.25}]
    t = tariffs.table(rates, set(), codes, {}, _observed(AAA="r", BBB="r", TER="r"), {"TER": "AAA", "AAA": "AAA"})
    assert (t.rate[2, 0] == 0).all() and (t.rate[0, 2] == 0).all()
    np.testing.assert_array_equal(t.rate[2, 1], [0.10, 0.08])
    assert t.imputed.tolist() == [False, False, True] and t.donor[2] == "AAA"


def test_tariff_pairs_map_source_codes_to_pack_countries_both_ways():
    codes = ["DEU", "FRA", "YUG"]
    pairs = tariffs.pack_pairs({("DEU", "FRA"), ("SCG", "FRA"), ("SRB", "MNE"), ("XXX", "DEU")}, codes, {})
    assert pairs == {(0, 1), (1, 0), (2, 1), (1, 2)}


def test_tariff_table_columns_are_importer_major_with_groups_innermost():
    t = tariffs.table([{"A": 0.1, "B": 0.2}, {"A": 0.3, "B": 0.4}], set(), ["A", "B"], {}, _observed(A="r", B="r"))
    cols = tariffs.columns(t)
    assert len(cols["rate"]) == 8
    assert cols["importer"].to_pylist() == [0, 0, 0, 0, 1, 1, 1, 1]
    assert cols["exporter"].to_pylist() == [0, 0, 1, 1, 0, 0, 1, 1]
    assert cols["group"].to_pylist() == [0, 1] * 4
    assert cols["rate"].to_pylist() == [0, 0, 0.1, 0.3, 0.2, 0.4, 0, 0]


def test_read_dta_keeps_goods_fta_and_customs_unions_in_force_on_new_year(tmp_path):
    from test_downscale import _xlsx

    agreements = [
        {"WB ID": 1.0, "Date of Entry into Force (G)": 30000.0},  # 1982
        {"WB ID": 2.0, "Date of Entry into Force (G)": 36526.0},  # 1 Jan 2000
        {"WB ID": 3.0, "Date of Entry into Force (G)": 36708.0},  # 1 Jul 2000
    ]

    def row(other, wb, year, kind="CU"):
        return {"iso1": "AAA", "iso2": other, "WBID": wb, "year": year, "type": kind, "entry": year}

    pairs = [row("BBB", 1.0, 1999.0), row("BBB", 1.0, 2000.0)]
    pairs += [row("CCC", 1.0, 2000.0)]  # joined the 1982 agreement during 2000
    pairs += [row("DDD", 2.0, 2000.0, "FTA & EIA"), row("EEE", 3.0, 2000.0, "FTA")]
    pairs += [row("FFF", 1.0, 1999.0, "PSA"), row("FFF", 1.0, 2000.0, "PSA")]
    pairs += [row("GGG", 1.0, 1999.0, "FTA")]  # left before 2000; a 2000 row without a year is an agreement not in force
    pairs += [{"iso1": "AAA", "iso2": "HHH", "WBID": 1.0, "type": "CU"}]
    path = _xlsx(tmp_path / "dta.xlsx", {"Agreements": agreements, "Bilateral Information": pairs})
    assert tariffs.read_dta(path) == {("AAA", "BBB"), ("AAA", "DDD")}


def test_read_cepii_needs_a_goods_agreement_in_1999_and_2000(tmp_path):
    rows = ["year,iso3_o,iso3_d,fta_wto,rta_type,dist"]
    for y in (1999, 2000):
        rows += [f"{y},AAA,BBB,1,1,5", f"{y},AAA,CCC,1,7,5", f"{y},AAA,DDD,0,,5"]
    rows += ["2000,AAA,EEE,1,4,5", "2001,AAA,FFF,1,4,5"]
    path = tmp_path / "g.zip"
    with zipfile.ZipFile(path, "w") as z:
        z.writestr("Gravity_V202211.csv", "\n".join(rows) + "\n")
    assert tariffs.read_cepii(path) == {("AAA", "BBB")}


# Gateways (Task 17)


def _graph(points: list[tuple[str, float, float, int]], links: list[tuple[int, int, int]]):
    """Provinces first (one per country code given), then extra nodes; kind 0 province, 2 port, 3 airport."""
    codes = []
    for c, *_ in points:
        if c not in codes:
            codes.append(c)
    provinces = pa.table({"country": pa.array(codes)})
    first = {c: i for i, c in enumerate(codes)}
    nodes = pa.table(
        {
            "kind": pa.array([k for *_, k in points], pa.uint8()),
            "province": pa.array([first[c] for c, *_ in points], pa.uint32()),
            "lon": pa.array([x for _, x, _, _ in points], pa.float64()),
            "lat": pa.array([y for _, _, y, _ in points], pa.float64()),
        }
    )
    edges = pa.table(
        {
            "a": pa.array([a for a, _, _ in links], pa.uint32()),
            "b": pa.array([b for _, b, _ in links], pa.uint32()),
            "mode": pa.array([m for *_, m in links], pa.uint8()),
        }
    )
    return nodes, edges, provinces


def test_land_crossings_merge_within_50_km_per_country_pair():
    # nodes 0, 1 province centroids of AAA and BBB; 2-5 rail nodes near the border, 6 a CCC centroid
    pts = [("AAA", 0.0, 0.0, 0), ("BBB", 1.0, 0.0, 0), ("AAA", 0.4, 0.0, 1), ("BBB", 0.6, 0.0, 1)]
    pts += [("AAA", 0.4, 3.0, 1), ("BBB", 0.6, 3.0, 1), ("CCC", 0.5, -1.0, 0)]
    links = [(0, 1, 0), (2, 3, 1), (4, 5, 1), (0, 6, 0), (0, 2, 0), (1, 6, 2)]
    nodes, edges, provinces = _graph(pts, links)
    country = gateways.node_country(nodes, provinces)
    # the rail link 2-3 has the road link's midpoint (0.5, 0); 4-5 is 330 km north; the sea edge 1-6 is not land
    assert gateways.land_crossings(nodes, edges, country) == [(0, 1), (4, 5), (0, 6)]


def test_kmedoids_finds_separate_clusters_deterministically():
    rng = np.random.default_rng(1)
    centres = [(0.0, 0.0), (20.0, 10.0), (-30.0, 40.0)]
    lon = np.concatenate([c[0] + rng.normal(0, 0.5, 30) for c in centres])
    lat = np.concatenate([c[1] + rng.normal(0, 0.5, 30) for c in centres])
    m = gateways.kmedoids(lon, lat, 3)
    assert sorted(int(i) // 30 for i in m) == [0, 1, 2]
    np.testing.assert_array_equal(m, gateways.kmedoids(lon, lat, 3))
    assert gateways.kmedoids(lon[:2], lat[:2], 5).tolist() == [0, 1]


def test_gateway_counts_follow_the_square_root_of_trade():
    trade = {"A": 1e12, "B": 2.5e11, "C": 1e6}
    k = gateways.counts(trade, {"A": 100, "B": 100, "C": 100, "D": 100}, total=30)
    assert k["C"] == 1 and k["D"] == 1
    assert sum(k.values()) == 30 and k["A"] == pytest.approx(2 * k["B"], abs=1)
    assert gateways.counts(trade, {"A": 3, "B": 100, "C": 100, "D": 100}, total=30)["A"] == 3


def test_every_country_gets_a_gateway_from_its_ports_airports_or_land_crossings():
    pts = [("AAA", 0.0, 0.0, 0), ("BBB", 5.0, 0.0, 0), ("CCC", 9.0, 0.0, 0)]
    pts += [("AAA", -1.0, 0.0, 2), ("AAA", -1.0, 1.0, 3), ("AAA", 3.0, 0.0, 2), ("CCC", 9.5, 0.0, 3)]
    links = [(0, 1, 0), (1, 2, 1)]
    nodes, edges, provinces = _graph(pts, links)
    t = gateways.select(nodes, edges, provinces, {"AAA": 1e12, "BBB": 1e9, "CCC": 1e9}, total=5)
    rows = list(zip(*(t[c].to_pylist() for c in ("node", "country", "kind"))))
    assert sorted({c for _, c, _ in rows}) == [0, 1, 2]
    assert len(rows) == 5 and rows == sorted(rows, key=lambda r: (r[1], r[0]))
    # BBB has only its two land crossing points (its centroid, an endpoint of both crossings, counts once)
    assert [(n, k) for n, c, k in rows if c == 1] == [(1, gateways.LAND)]
    assert {k for _, c, k in rows if c == 0} <= {gateways.PORT, gateways.AIRPORT, gateways.LAND}


# The built pack (Task 17): skipped without it.
PACK = ROOT / "data" / "packs" / "2000"
GOLDEN_P2 = ROOT / "tests" / "golden" / "p2-pack.txt"


def _pack_table(name: str):
    path = PACK / name
    if not path.exists():
        pytest.skip(f"skipping: no {name} in {PACK}; build the pack with `uv run python -m ceteris.packs`")
    return ipc.open_file(str(path)).read_all()


def test_every_country_has_a_gateway():
    g, c = _pack_table("gateways.arrow"), _pack_table("countries.arrow")
    assert set(g["country"].to_pylist()) == set(range(c.num_rows))
    assert len(set(g["node"].to_pylist())) == g.num_rows


def test_gateway_count_near_600():
    assert 500 <= _pack_table("gateways.arrow").num_rows <= 700


def test_pack_econ_tables_are_consistent():
    countries, prov = _pack_table("econ_countries.arrow"), _pack_table("econ_provinces.arrow")
    io_ = _pack_table("econ_io.arrow")
    assert countries["code"].to_pylist() == _pack_table("countries.arrow")["code"].to_pylist()
    assert prov.num_rows == _pack_table("provinces.arrow").num_rows
    fd = np.column_stack([countries[f"fd_{g}"].to_numpy() for g in range(goods.N_GOODS)])
    np.testing.assert_allclose(fd.sum(axis=1), 1.0, atol=1e-12)
    a = io_["a"].to_numpy().reshape(countries.num_rows, goods.N_GOODS, goods.N_GOODS)
    assert (a >= 0).all() and (a.sum(axis=1) < 1).all()
    va = np.column_stack([prov[f"va_{g}"].to_numpy() for g in range(goods.N_GOODS)])
    national = np.column_stack([countries[f"va_{g}"].to_numpy() for g in range(goods.N_GOODS)])
    index = {c: i for i, c in enumerate(countries["code"].to_pylist())}
    by_country = np.zeros_like(national)
    np.add.at(by_country, [index[c] for c in _pack_table("provinces.arrow")["country"].to_pylist()], va)
    np.testing.assert_allclose(by_country, national, rtol=1e-9, atol=1e-6)
    t = _pack_table("econ_tariffs.arrow")
    assert t.num_rows == 2 * countries.num_rows**2
    r = t["rate"].to_numpy()
    assert (r >= 0).all() and (r < 1).all()
    pops = _pack_table("econ_pops.arrow")
    assert pops.num_rows == 3 * prov.num_rows
    lf, pop = pops["labour_force"].to_numpy(), pops["population"].to_numpy()
    assert (lf <= pop).all()


# Written by `python -m ceteris.reference calibrate` and installed into the pack (ADR 0089).
CALIBRATION_TABLES = (
    "econ_calib.arrow",
    "econ_calib_cells.arrow",
    "econ_calib_provinces.arrow",
    "econ_calib_pops.arrow",
    "econ_calib_pipeline.arrow",
    "econ_calib_countries.arrow",
)


def _p2_lines(tables: dict[str, str], calibration: dict[str, str]) -> str:
    lines = [f"{name} {tables[name]}" for name in econ_build.TABLES]
    # then the sha256 of the calibration tables installed in the pack
    lines += [f"{name} {calibration[name]}" for name in CALIBRATION_TABLES if name in calibration]
    return "# P2 pack 2000: econ table sha256 from manifest.json\n" + "\n".join(lines) + "\n"


def test_pack_econ_tables_match_golden():
    """Regenerate deliberately with CETERIS_BLESS=1 after rebuilding or recalibrating, and say why in the commit."""
    manifest = PACK / "manifest.json"
    if not manifest.exists():
        pytest.skip(f"skipping: no manifest.json in {PACK}; build the pack with `uv run python -m ceteris.packs`")
    import hashlib

    calibration = {
        name: hashlib.sha256((PACK / name).read_bytes()).hexdigest() for name in CALIBRATION_TABLES if (PACK / name).exists()
    }
    actual = _p2_lines(json.loads(manifest.read_text())["tables"], calibration)
    if os.environ.get("CETERIS_BLESS"):
        GOLDEN_P2.write_text(actual)
        return
    assert GOLDEN_P2.read_text() == actual


def test_pack_econ_tables_reproducible(tmp_path):
    """Two econ builds from the same P1 tables and cache give the same bytes."""
    _pack_table("provinces.arrow")
    for name in ("ppp_2000_1km_Aggregated.tif", "1995-2000_SML.zip", tariffs.GRAVITY, tariffs.DTA):
        _cached(name)
    first = econ_build.build(PACK, tmp_path / "a")["tables"]
    second = econ_build.build(PACK, tmp_path / "b")["tables"]
    assert first == second and set(first) == set(econ_build.TABLES)
