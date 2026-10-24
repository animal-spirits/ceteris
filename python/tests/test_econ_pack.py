import math
import os
import re
import zipfile
from pathlib import Path

import numpy as np
import pytest
from pyarrow import ipc

from ceteris.packs import fetch
from ceteris.packs.econ import goods, theta

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


def test_hs_goods_shares_split_ambiguous_codes_equally():
    hs_isic3 = {"010111": "0121", "270900": "1110", "999999": "9999"}
    bridge = {"0121": ["0141", "0145"], "1110": ["0610", "0910"]}
    shares = goods.hs_goods_shares(hs_isic3, bridge)
    assert shares["010111"] == {0: 1.0}
    assert shares["270900"] == {1: 1.0}
    assert "999999" not in shares

    bridge = {"1110": ["0610", "0510"]}
    assert goods.hs_goods_shares({"270900": "1110"}, bridge)["270900"] == {1: 0.5, 2: 0.5}


def test_hs_goods_shares_fall_back_to_the_three_digit_prefix():
    bridge = {"0501": ["0311"], "0502": ["0321"]}
    assert goods.hs_goods_shares({"030212": "0500"}, bridge)["030212"] == {0: 1.0}


def test_bridge_keeps_principal_links_and_drops_mining_support(tmp_path):
    path = tmp_path / "bridge.txt"
    path.write_text(
        '"ISIC31code","partialISIC31","ISIC4code","partialISIC4","Detail"\n'
        '"1010",1,"0510",0,""\n"1010",1,"0990",1,"Support"\n"1010",1,"1920",1,"Briquettes"\n'
        '"1410",1,"0810",1,"Quarrying"\n"1410",1,"0990",1,"Support"\n'
        '"2411",1,"1910",1,"Pitch"\n"2411",1,"2011",1,"Basic chemicals"\n'
    )
    assert goods.read_bridge(path) == {"1010": ["0510"], "1410": ["0810"], "2411": ["1910", "2011"]}


def test_weights_from_baci_pins_tonnes_per_usd(tmp_path):
    # One HS line per traded good: hs6 -> (ISIC Rev 3 class, ISIC Rev 4 class) in divisions 01, 06, 05, 10, 13, 20, 26.
    lines = {
        "010121": ("0111", "0111"),
        "270900": ("1110", "0610"),
        "270112": ("1010", "0510"),
        "020110": ("1511", "1010"),
        "520100": ("1711", "1310"),
        "290110": ("2411", "2011"),
        "850110": ("3110", "2610"),
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
        "2000,1,2,999999,7,7",  # absent from the concordance
    ]
    baci = tmp_path / "baci.zip"
    with zipfile.ZipFile(baci, "w") as z:
        z.writestr("BACI_HS92_Y2000_V202601.csv", "t,i,j,k,v,q\n" + "\n".join(rows) + "\n")
        z.writestr("BACI_HS92_Y2001_V202601.csv", "t,i,j,k,v,q\n2001,1,2,10121,1,999\n")
    wits = tmp_path / "wits.zip"
    with zipfile.ZipFile(wits, "w") as z:
        text = "HS1988-92 Product Code,Product Description,ISIC Revision 3 Product Code\n"
        text += "".join(f"{hs},x,{i3}\n" for hs, (i3, _) in lines.items())
        z.writestr("JobID-0_Concordance_H0_to_I3.CSV", text)
    bridge = tmp_path / "bridge.txt"
    bridge.write_text(
        '"ISIC31code","partialISIC31","ISIC4code","partialISIC4","Detail"\n'
        + "".join(f'"{i3}",0,"{i4}",0,""\n' for i3, i4 in lines.values())
    )

    w = goods.weights_from_baci(baci, wits, bridge)

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
    # Two light-manufacturing lines (Textile 8.10, Wood 11.50) and one food line (2.62); one line out of the concordance.
    lines = {"520100": ("1711", "1310"), "440310": ("2010", "1610"), "020110": ("1511", "1010")}
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
    wits = tmp_path / "wits.zip"
    with zipfile.ZipFile(wits, "w") as z:
        text = "HS1988-92 Product Code,Product Description,ISIC Revision 3 Product Code\n"
        text += "".join(f"{hs},x,{i3}\n" for hs, (i3, _) in lines.items())
        z.writestr("JobID-0_Concordance_H0_to_I3.CSV", text)
    bridge = tmp_path / "bridge.txt"
    bridge.write_text(
        '"ISIC31code","partialISIC31","ISIC4code","partialISIC4","Detail"\n'
        + "".join(f'"{i3}",0,"{i4}",0,""\n' for i3, i4 in lines.values())
    )

    t = theta.caliendo_parro_by_good(baci, wits, bridge)

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
    baci = _cached("BACI_HS92_V202601.zip")
    hs_isic = _cached("Concordance_H0_to_I3.zip")
    bridge = _cached("ISIC31_ISIC4.txt")
    return goods.weights_from_baci(baci, hs_isic, bridge)


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
    baci = _cached("BACI_HS92_V202601.zip")
    hs_isic = _cached("Concordance_H0_to_I3.zip")
    bridge = _cached("ISIC31_ISIC4.txt")
    return theta.caliendo_parro_with_value(baci, hs_isic, bridge)


def test_caliendo_parro_by_good_from_baci(caliendo_parro):
    """ADR 0023 values: theta per good to two decimals, and the classes of `TradeParams::default()`."""
    t, v = caliendo_parro
    np.testing.assert_allclose(t, [9.11, 13.53, 13.53, 2.62, 6.73, 12.64, 5.33, 0, 0, 0], atol=0.005)
    assert (v[:7] > 1e11).all() and (v[7:] == 0).all()
    of, ct = theta.classes(t[:7], v[:7])
    assert of.tolist() == [2, 2, 2, 0, 1, 2, 1]
    np.testing.assert_allclose(ct, [2.62, 5.88, 12.55], atol=0.005)
    assert math.log(ct[-1] / ct[0]) > math.log(theta.CLASS_SPREAD)
