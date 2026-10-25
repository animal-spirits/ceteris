import collections
import json
import os
from pathlib import Path

import numpy as np
import pytest
import shapely
from hypothesis import given, settings
from hypothesis import strategies as st

from ceteris.packs import build, counties, fetch, fills, geo, provinces, transport

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


@settings(max_examples=50)
@given(st.integers(8, 300), st.integers(2, 8), st.randoms(use_true_random=False))
def test_weighted_bisection_balances_weight(n, k, rng):
    pts = np.array([[rng.uniform(0, 10), rng.uniform(0, 10)] for _ in range(n)])
    w = np.array([rng.uniform(1, 10) for _ in range(n)])
    groups = provinces._bisect(pts, np.arange(n), k, w)
    assert len(groups) == k and all(len(g) for g in groups)
    assert sorted(np.concatenate(groups).tolist()) == list(range(n))
    sums = [w[g].sum() for g in groups]
    # Each cut lands within one item of its target; errors compound over log2(k) levels.
    assert max(sums) - min(sums) <= 2 * (int(np.log2(k)) + 1) * w.max()


def test_county_level_prefers_the_coarsest_level_in_range():
    assert counties.choose_level({"ADM2": 96, "ADM3": 320}, 101) == "ADM3"
    assert counties.choose_level({"ADM2": 428}, 16) == "ADM2"
    assert counties.choose_level({"ADM2": 52, "ADM3": 8205}, 52) is None
    assert counties.choose_level({"ADM2": 25}, 15) == "ADM2"
    assert counties.choose_level({"ADM2": 76, "ADM3": 5162}, 13, override="ADM2") == "ADM2"


def test_county_split_follows_county_borders_and_covers_the_province():
    p = provinces.Province("X-1", "X", "XXX", "XXX", "XXX", shapely.box(0, 0, 4, 4))
    grid = [shapely.box(x, y, x + 1, y + 1) for x in range(4) for y in range(4)]
    parts = provinces.split_by_counties(p, 4, grid, max_area_km2=1e9, res=4)
    assert parts is not None and len(parts) == 4
    assert shapely.union_all([q.geom for q in parts]).symmetric_difference(p.geom).area < 1e-9
    for q in parts:
        assert q.geom.area == pytest.approx(4.0)
        # Every part is a union of whole counties.
        assert all(abs(q.geom.intersection(c).area - c.area) < 1e-9 or q.geom.intersection(c).area < 1e-9 for c in grid)


def test_county_split_declines_with_too_few_counties():
    p = provinces.Province("X-1", "X", "XXX", "XXX", "XXX", shapely.box(0, 0, 4, 4))
    assert provinces.split_by_counties(p, 4, [shapely.box(0, 0, 4, 2), shapely.box(0, 2, 4, 4)], 1e9, 4) is None


def _tri_area(xy, index):
    t = xy[index.reshape(-1, 3)]
    return 0.5 * np.abs(
        (t[:, 1, 0] - t[:, 0, 0]) * (t[:, 2, 1] - t[:, 0, 1]) - (t[:, 2, 0] - t[:, 0, 0]) * (t[:, 1, 1] - t[:, 0, 1])
    )


def test_fill_buffers_round_trip_and_cover_their_polygons(tmp_path):
    donut = shapely.box(0, 0, 20, 10).difference(shapely.box(5, 2, 8, 6))
    geoms = [donut, shapely.MultiPolygon([shapely.box(30, 30, 31, 31), shapely.box(40, 30, 42, 31)])]
    fills.write(tmp_path / "x.fill", geoms)
    xy, feature, index, n = fills.read(tmp_path / "x.fill")
    assert n == 2 and len(feature) == len(xy) and set(feature) == {0, 1}
    for f, g in enumerate(geoms):
        tris = index.reshape(-1, 3)
        mine = tris[(feature[tris] == f).all(axis=1)]
        expected = sum(
            shapely.Polygon(fills.mercator(np.asarray(p.exterior.coords))).area
            - sum(shapely.Polygon(fills.mercator(np.asarray(r.coords))).area for r in p.interiors)
            for p in shapely.get_parts(g)
        )
        assert _tri_area(xy.astype(float), mine.ravel()).sum() == pytest.approx(expected, rel=1e-4)


def test_subdivision_bounds_edges_and_leaves_no_cracks():
    # Two triangles sharing a long diagonal must split it at the same points.
    xy = np.array([[0.0, 0.0], [0.03, 0.0], [0.03, 0.02], [0.0, 0.02]])
    tri = np.array([0, 1, 2, 0, 2, 3], dtype=np.uint32)
    v, t = fills.subdivide(xy, tri)
    corners = v[t.reshape(-1, 3)]
    assert np.linalg.norm(corners - np.roll(corners, 1, axis=1), axis=2).max() <= fills.MAX_EDGE
    assert _tri_area(v, t).sum() == pytest.approx(0.03 * 0.02)
    edges = collections.Counter()
    for a, b, c in t.reshape(-1, 3):
        for e in ((a, b), (b, c), (c, a)):
            edges[tuple(sorted(e))] += 1
    boundary = [e for e, k in edges.items() if k == 1]
    # Each boundary edge lies on the square's outline: no T-junction gaps inside.
    on_outline = lambda p: np.isclose(p[0], 0) or np.isclose(p[0], 0.03) or np.isclose(p[1], 0) or np.isclose(p[1], 0.02)
    assert all(on_outline(v[a]) and on_outline(v[b]) for a, b in boundary)


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
    from ceteris.packs.econ import build as econ_build

    manifest = json.loads((PACK / "manifest.json").read_text())
    lines = [f"{name} {digest}" for name, digest in manifest["tables"].items() if name not in econ_build.TABLES]
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
    c = ipc.open_file(PACK / "counties.arrow").read_all()
    home = c["province"].to_numpy()
    assert (home < len(p)).all() and (np.diff(home.astype(np.int64)) >= 0).all()
    stats = json.loads((PACK / "manifest.json").read_text())["stats"]
    assert stats["county_area_outside_province"] < 0.05
    assert stats["split_by_counties"] > stats["split_by_voronoi"]


def test_fetch_does_not_download_manual_files(tmp_path, monkeypatch):
    monkeypatch.setattr(fetch, "CACHE", tmp_path)
    entry = {"name": "x.zip", "url": "https://example.invalid/x.zip", "manual": True}
    with pytest.raises(SystemExit, match="save https://example.invalid/x.zip"):
        fetch.fetch(entry)
    (tmp_path / "x.zip").write_bytes(b"data")
    assert fetch.fetch(entry) == tmp_path / "x.zip"


def test_icio_packages_are_manual_downloads():
    icio = [e for e in fetch.entries() if e["source"] == "oecd-icio"]
    assert len(icio) == 10 and all(e.get("manual") and e["sha256"] for e in icio)


def test_fetch_skips_optional_files_that_are_not_cached(tmp_path, monkeypatch, capsys):
    entries = [
        {"source": "x", "name": "need.zip", "url": "u", "manual": True, "sha256": "0" * 64, "bytes": 1},
        {"source": "x", "name": "later.zip", "url": "u", "manual": True, "optional": True, "sha256": "0" * 64, "bytes": 1},
    ]
    monkeypatch.setattr(fetch, "CACHE", tmp_path)
    monkeypatch.setattr(fetch, "entries", lambda: entries)
    monkeypatch.setattr(fetch, "validation_entries", list)
    monkeypatch.setattr("sys.argv", ["fetch"])
    with pytest.raises(SystemExit, match="need.zip"):
        fetch.main()
    (tmp_path / "need.zip").write_bytes(b"x")
    # The cached required file must match its pin (wrong here), the missing optional one is not asked for.
    assert fetch.main() == 1
    err = capsys.readouterr().err
    assert "need.zip: checksum mismatch" in err and "later.zip" not in err
    (tmp_path / "later.zip").write_bytes(b"x")
    assert fetch.main() == 1 and "later.zip: checksum mismatch" in capsys.readouterr().err


def test_validation_files_are_fetched_but_not_pack_sources():
    pack = {e["name"] for e in fetch.entries()}
    validation = {e["name"]: e for e in fetch.validation_entries()}
    assert "AdvancedGuideFiles.zip" in validation and not pack & set(validation)
    assert all(e["sha256"] for e in validation.values())


def test_no_pack_code_names_a_validation_file():
    names = {e["name"] for e in fetch.validation_entries()}
    code = Path(fetch.__file__).parent
    for path in code.rglob("*.py"):
        text = path.read_text()
        assert not [n for n in names if n in text], path


def test_only_the_2000_icio_package_is_required():
    icio = {e["name"]: e for e in fetch.entries() if e["source"] == "oecd-icio"}
    assert len(icio) == 10
    assert [n for n, e in icio.items() if not e.get("optional")] == ["1995-2000_SML.zip"]


def test_pack_build_does_not_demand_uncached_optional_sources(tmp_path, monkeypatch):
    optional = {"source": "x", "name": "later.zip", "url": "u", "manual": True, "optional": True}
    required = {"source": "x", "name": "need.zip", "url": "u", "manual": True}
    monkeypatch.setattr(fetch, "CACHE", tmp_path)
    monkeypatch.setattr(fetch, "entries", lambda: [optional])
    # The fetch loop passes; the build then stops for want of the real inputs, which is not a SystemExit about the optional file.
    with pytest.raises(Exception):  # noqa: B017
        build.build(tmp_path / "out")
    monkeypatch.setattr(fetch, "entries", lambda: [optional, required])
    with pytest.raises(SystemExit, match="need.zip"):
        build.build(tmp_path / "out")
