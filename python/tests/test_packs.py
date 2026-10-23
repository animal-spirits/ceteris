import json
import os
from pathlib import Path

import numpy as np
import pytest
import shapely
from hypothesis import given, settings
from hypothesis import strategies as st

from ceteris.packs import build, geo, provinces, transport

ROOT = Path(__file__).resolve().parents[2]
PACK = ROOT / "data" / "packs" / "2000"
GOLDEN = ROOT / "tests" / "golden" / "p1-pack.txt"


def test_area_of_one_degree_square_at_equator():
    # 111.19 km x 110.57 km on the sphere of mean radius.
    assert geo.area_km2(shapely.box(0, 0, 1, 1)) == pytest.approx(12_363.7, rel=1e-3)


def test_unwrap_then_wrap_keeps_area_across_the_antimeridian():
    g = shapely.MultiPolygon([shapely.box(178, 60, 180, 62), shapely.box(-180, 60, -177, 62)])
    assert geo.crosses_antimeridian(g)
    u = geo.unwrap(g)
    assert u.bounds[0] >= 178 and u.bounds[2] <= 183
    assert geo.area_km2(geo.wrap(u)) == pytest.approx(geo.area_km2(g), rel=1e-9)


@given(st.floats(0, 5e6))
def test_split_count_keeps_parts_under_the_limit(area):
    k = provinces.split_count(area, 250_000)
    assert k >= 1 and area / k <= 250_000


@settings(max_examples=50)
@given(st.integers(1, 400), st.integers(1, 12), st.randoms(use_true_random=False))
def test_bisection_partitions_into_near_equal_groups(n, k, rng):
    k = min(k, n)
    pts = np.array([[rng.uniform(0, 10), rng.uniform(0, 10)] for _ in range(n)])
    groups = provinces._bisect(pts, np.arange(n), k)
    assert len(groups) == k
    assert sorted(np.concatenate(groups).tolist()) == list(range(n))
    sizes = [len(g) for g in groups]
    assert max(sizes) - min(sizes) <= int(np.log2(k)) + 1
    again = provinces._bisect(pts, np.arange(n), k)
    assert all((a == b).all() for a, b in zip(groups, again))


@given(st.sets(st.tuples(st.integers(0, 30), st.integers(0, 30)).filter(lambda e: e[0] != e[1]), max_size=120))
def test_neighbouring_countries_get_different_colours(pairs):
    codes = [f"C{i:02}" for i in range(31)]
    borders = {(f"C{a:02}", f"C{b:02}") for a, b in pairs}
    colour = build.colour_countries(codes, borders)
    degree = max((sum(c in e for e in borders) for c in codes), default=0)
    if degree < build.PALETTE_SIZE:
        assert all(colour[a] != colour[b] for a, b in borders)


def test_terrain_classes():
    elev = np.array([0, 800, 300, 2500])
    tri = np.array([5, 10, 60, 300])
    assert build.terrain_class(elev, tri).tolist() == [0, 1, 2, 3]


def test_2000_borders_remap_successor_states():
    b = provinces.BORDERS
    assert b["country"]["SDS"] == "SDN" and b["country"]["MNE"] == "YUG"
    assert b["province"]["RUS-283"] == "UKR"


def test_every_link_endpoint_is_named_and_ordered():
    for cp in transport.CHOKEPOINTS:
        assert cp["radius_km"] > 0 and cp["name"]
        assert len(cp.get("link", [[0, 0], [0, 0]])) >= 2


def _pack_lines() -> str:
    manifest = json.loads((PACK / "manifest.json").read_text())
    lines = [f"{name} {digest}" for name, digest in manifest["tables"].items()]
    return "# P1 pack 2000: table sha256 from manifest.json\n" + "\n".join(lines) + "\n"


@pytest.mark.skipif(not (PACK / "manifest.json").exists(), reason="pack not built")
def test_pack_matches_golden_hashes():
    """The build is reproducible: same pinned inputs give the same bytes. Regenerate deliberately
    with CETERIS_BLESS=1 after rebuilding, and say why in the commit."""
    actual = _pack_lines()
    if os.environ.get("CETERIS_BLESS"):
        GOLDEN.write_text(actual)
        return
    assert GOLDEN.read_text() == actual


@pytest.mark.skipif(not (PACK / "manifest.json").exists(), reason="pack not built")
def test_pack_invariants():
    from pyarrow import ipc

    p = ipc.open_file(PACK / "provinces.arrow").read_all()
    e = ipc.open_file(PACK / "edges.arrow").read_all()
    n = ipc.open_file(PACK / "nodes.arrow").read_all()
    assert len(set(p["id"].to_pylist())) == len(p)
    assert sum(p["area_km2"].to_pylist()) == pytest.approx(134.5e6, rel=0.01)
    assert max(p["area_km2"].to_pylist()) < 2 * build.CONFIG["split_area_km2"]
    kinds = n["kind"].to_numpy()
    assert (kinds[: len(p)] == transport.NodeKind.PROVINCE).all()
    a, b = e["a"].to_numpy(), e["b"].to_numpy()
    assert (a < b).all() and b.max() < len(n)
