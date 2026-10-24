import numpy as np
import pytest

from ceteris.packs import fetch
from ceteris.packs.econ import goods

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


def test_local_goods_weigh_nothing(weights):
    assert (weights[7:] == 0).all()


def test_bulk_heavier_than_machinery(weights):
    assert weights[2] > 10 * weights[6]
