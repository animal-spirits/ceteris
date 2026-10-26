"""Gateways of the two-level trade kernel (specs/econ/SPEC.md, Pack additions, Gateways; trade SPEC, Two-level kernel).

Each country gets k gateways, k proportional to the square root of its 2000 trade (BACI exports plus imports), at
least one, at most its candidate count, scaled so that the world total is near `TOTAL`. The candidates are the
country's listed ports and airports and its land crossing points: the endpoints of graph edges (road, rail) whose two
ends lie in different 2000 countries, merged within `MERGE_KM` per country pair. The k gateways are the k-medoids of
the candidates on great-circle distance (an agent default, ADR 0014; the spec's path cost needs the Rust paths).

Each gateway's mass m_a (ADR 0092) is its share of its country's 2000 trade throughput, so the masses of a country's
gateways sum to 1. The pack's sources give no throughput per gateway (Natural Earth ports and OurAirports airports
carry no tonnage, value or passenger counts, and land crossings no traffic), so every gateway of a country with k
gateways takes 1/k.

Every land crossing enters as a candidate, not as a gateway of its own: the P1 graph has 1,359 merged crossings, so
gateways for all of them would make the all-pairs crossing matrix about twenty times larger than the kernel's
budget of about 600 gateways (trade SPEC, cost per good).
"""

import zipfile
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.csv as pacsv

from .. import transport

TOTAL = 600
MERGE_KM = 50.0
EARTH_KM = 6371.0088
# Gateway kinds in gateways.arrow.
PORT, AIRPORT, LAND = 0, 1, 2
KINDS = ("port", "airport", "land crossing")
NO_PROVINCE = 0xFFFFFFFF
_LAND_MODES = (transport.Mode.ROAD, transport.Mode.RAIL)
# BACI names Taiwan "Other Asia, nes".
BACI_ALIASES = {"S19": "TWN"}


def great_circle_km(lon1, lat1, lon2, lat2) -> np.ndarray:
    lon1, lat1, lon2, lat2 = (np.radians(np.asarray(x, np.float64)) for x in (lon1, lat1, lon2, lat2))
    h = np.sin((lat2 - lat1) / 2) ** 2 + np.cos(lat1) * np.cos(lat2) * np.sin((lon2 - lon1) / 2) ** 2
    return 2 * EARTH_KM * np.arcsin(np.sqrt(np.minimum(h, 1.0)))


def node_country(nodes: pa.Table, provinces: pa.Table) -> np.ndarray:
    """Country code of each graph node through its province; "" for sea nodes."""
    country = np.array(provinces.column("country").to_pylist() + [""], dtype=object)
    prov = nodes.column("province").to_numpy().astype(np.int64)
    return country[np.where(prov == NO_PROVINCE, len(country) - 1, prov)]


def land_crossings(nodes: pa.Table, edges: pa.Table, country: np.ndarray) -> list[tuple[int, int]]:
    """Road and rail edges between two countries as (node, node), one per country pair and `MERGE_KM` of midpoint.

    Edges are taken in pack order; an edge is merged into the first kept crossing of its country pair whose midpoint
    lies within `MERGE_KM` of its own.
    """
    lon, lat = nodes.column("lon").to_numpy(), nodes.column("lat").to_numpy()
    a, b, mode = (edges.column(c).to_numpy() for c in ("a", "b", "mode"))
    land = np.isin(mode, [int(m) for m in _LAND_MODES]) & (country[a] != country[b]) & (country[a] != "") & (country[b] != "")
    kept: dict[tuple[str, str], list[tuple[float, float]]] = defaultdict(list)
    out = []
    for e in np.flatnonzero(land):
        x, y = int(a[e]), int(b[e])
        pair = tuple(sorted((country[x], country[y])))
        mid = (lon[x] + lon[y]) / 2, (lat[x] + lat[y]) / 2
        if abs(lon[x] - lon[y]) > 180:
            mid = (((mid[0] + 360) % 360) - 180, mid[1])
        near = kept[pair]
        if near and great_circle_km(mid[0], mid[1], *np.array(near).T).min() < MERGE_KM:
            continue
        near.append(mid)
        out.append((x, y))
    return out


def trade_by_country(baci_2000: Path, iso: Mapping[str, str], codes: Sequence[str]) -> dict[str, float]:
    """2000 exports plus imports (USD) of each pack country from BACI, summing the economies a country combines."""
    from . import people

    with zipfile.ZipFile(baci_2000) as z:
        names = z.namelist()
        with z.open(next(n for n in names if n.startswith("country_codes"))) as f:
            cc = pacsv.read_csv(f, convert_options=pacsv.ConvertOptions(column_types={"country_iso3": pa.string()}))
        numeric = dict(zip(cc["country_code"].to_pylist(), cc["country_iso3"].to_pylist()))
        convert = pacsv.ConvertOptions(include_columns=["i", "j", "v"], column_types={"v": pa.float64()})
        total: dict[int, float] = defaultdict(float)
        with z.open(next(n for n in names if n.endswith("Y2000_V202601.csv"))) as f:
            for batch in pacsv.open_csv(f, convert_options=convert):
                t = pa.Table.from_batches([batch])
                for side in ("i", "j"):
                    g = t.group_by(side).aggregate([("v", "sum")])
                    for k, v in zip(g[side].to_pylist(), pc.multiply(g["v_sum"], 1000.0).to_pylist()):
                        total[k] += v
    by_iso: dict[str, float] = defaultdict(float)
    for k, v in total.items():
        code = numeric.get(k)
        by_iso[BACI_ALIASES.get(code, code)] += v
    by_iso["SRB"] += by_iso.pop("SCG", 0.0)
    out = {}
    for c in codes:
        value = sum(by_iso.get(p, 0.0) for p in people.PARTS.get(c, (iso.get(c, c),)))
        if value > 0:
            out[c] = value
    return out


def counts(trade: Mapping[str, float], candidates: Mapping[str, int], total: int = TOTAL) -> dict[str, int]:
    """Gateways per country: round(s sqrt(trade)) within [1, candidates], with s the scale whose sum is nearest `total`."""
    codes = sorted(candidates)
    root = np.sqrt([trade.get(c, 0.0) for c in codes])
    cap = np.array([candidates[c] for c in codes])

    def k(s: float) -> np.ndarray:
        return np.clip(np.round(s * root), 1, cap).astype(int)

    lo, hi = 0.0, 1.0
    while k(hi).sum() < min(total, cap.sum()):
        hi *= 2
    for _ in range(200):
        mid = (lo + hi) / 2
        lo, hi = (mid, hi) if k(mid).sum() < total else (lo, mid)
    best = min((lo, hi), key=lambda s: (abs(k(s).sum() - total), s))
    return dict(zip(codes, k(best).tolist()))


def kmedoids(lon: np.ndarray, lat: np.ndarray, k: int, iterations: int = 100) -> np.ndarray:
    """Indices of k medoids on great-circle distance: greedy build, then alternate assignment and medoid updates.

    Deterministic: ties go to the lower index.
    """
    n = len(lon)
    if k >= n:
        return np.arange(n)
    d = great_circle_km(lon[:, None], lat[:, None], lon[None, :], lat[None, :])
    medoids = [int(np.argmin(d.sum(axis=1)))]
    nearest = d[medoids[0]].copy()
    for _ in range(1, k):
        gain = np.maximum(nearest[None, :] - d, 0.0).sum(axis=1)
        gain[medoids] = -1.0
        m = int(np.argmax(gain))
        medoids.append(m)
        nearest = np.minimum(nearest, d[m])
    medoids = np.array(medoids)
    for _ in range(iterations):
        label = np.argmin(d[medoids], axis=0)
        update = medoids.copy()
        for c in range(k):
            members = np.flatnonzero(label == c)
            if not len(members):
                continue
            update[c] = members[np.argmin(d[np.ix_(members, members)].sum(axis=1))]
        if (update == medoids).all():
            break
        medoids = update
    return np.sort(medoids)


def select(
    nodes: pa.Table, edges: pa.Table, provinces: pa.Table, trade_by_country: Mapping[str, float], total: int = TOTAL
) -> pa.Table:
    """`gateways.arrow`: node, country (index into the sorted country codes), kind, mass (1/k of the country's k
    gateways); sorted by country, then node."""
    country = node_country(nodes, provinces)
    kind = nodes.column("kind").to_numpy()
    lon, lat = nodes.column("lon").to_numpy(), nodes.column("lat").to_numpy()
    cand: dict[str, dict[int, int]] = defaultdict(dict)
    for i in np.flatnonzero(np.isin(kind, [int(transport.NodeKind.PORT), int(transport.NodeKind.AIRPORT)])):
        cand[country[i]][int(i)] = PORT if kind[i] == transport.NodeKind.PORT else AIRPORT
    for x, y in land_crossings(nodes, edges, country):
        for v in (x, y):
            cand[country[v]].setdefault(v, LAND)
    codes = sorted(set(provinces.column("country").to_pylist()))
    missing = [c for c in codes if not cand.get(c)]
    if missing:
        raise ValueError(f"countries without a port, airport or land crossing: {missing}")
    k = counts(trade_by_country, {c: len(cand[c]) for c in codes}, total)
    rows: list[tuple[int, int, int]] = []
    for ci, c in enumerate(codes):
        ids = np.array(sorted(cand[c]))
        for m in kmedoids(lon[ids], lat[ids], k[c]):
            rows.append((int(ids[m]), ci, cand[c][int(ids[m])]))
    node, ctry, knd = zip(*rows)
    per = np.bincount(ctry)
    return pa.table(
        {
            "node": pa.array(node, pa.uint32()),
            "country": pa.array(ctry, pa.uint32()),
            "kind": pa.array(knd, pa.uint8()),
            "mass": pa.array(1.0 / per[np.asarray(ctry)], pa.float64()),
        }
    )


def report(table: pa.Table, codes: Sequence[str]) -> str:
    kinds = np.bincount(table.column("kind").to_numpy(), minlength=len(KINDS))
    per = np.bincount(table.column("country").to_numpy(), minlength=len(codes))
    top = sorted(zip(per.tolist(), codes), reverse=True)[:8]
    return (
        f"gateways: {table.num_rows} ("
        + ", ".join(f"{n} {name}" for n, name in zip(kinds.tolist(), KINDS))
        + f"); per country min {per.min()}, max {per.max()}; most: "
        + ", ".join(f"{c} {n}" for n, c in top)
    )
