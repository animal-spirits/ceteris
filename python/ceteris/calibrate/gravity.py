"""Direct estimation of trade costs by PPML structural gravity (ADR 0022).

    uv run python -m ceteris.calibrate.gravity [--pack data/packs/2000] [--write] [--out FILE]

The estimator (`reference.ppml.hdfe`) first reproduces the published PPML estimates of Yotov,
Piermartini, Monteiro and Larch (2016) on their sample (`yotov`). On 2000 BACI it then estimates, with
least-cost (S05) costs between countries from the pack's transport graph (route choice is deferred past
P2, ADR 0017):

- theta per class (P01, ADR 0023): minus the coefficient on ln(1 + tariff), international flows;
- the border friction nu (P05): a border dummy on a sample with domestic flows (2000 ICIO output less
  exports), as an ad valorem equivalent at the class thetas of the same fit;
- mode cost ratios (P04a-d): coefficients on tonne-km by mode along the S05 paths, relative to road.

Outputs carry provenance "data". `--write` puts them in `packs/econ/gravity_2000.toml` only when no stop
rule of the task fires (`stop_rules`); the econ pack build does not read that file yet, since whether the
model switches to the estimates is the owner's decision (PLAN Task 18).
"""

import struct
import zipfile
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from ..reference import ppml

# ---------------------------------------------------------------- Stata files

_DTA_NUMERIC = {251: ("b", 1), 252: ("h", 2), 253: ("i", 4), 254: ("f", 4), 255: ("d", 8)}
# Smallest missing value per type (Stata 113 to 115): codes at or above it are missing.
_DTA_MISSING = {251: 101, 252: 32741, 253: 2147483621, 254: 1.7014118e38, 255: 8.98846567431158e307}


def read_dta(data: bytes) -> dict[str, np.ndarray]:
    """Columns of a Stata dataset in format 113 to 115 (Stata 8 to 13): numbers as float64 with
    missing values as NaN, strings as str arrays."""
    version, order = data[0], data[1]
    if version not in (113, 114, 115):
        raise ValueError(f"Stata format {version} is not supported")
    e = "<" if order == 2 else ">"
    nvar, nobs = struct.unpack_from(e + "Hi", data, 4)
    pos = 4 + 2 + 4 + 81 + 18
    types = list(data[pos : pos + nvar])
    pos += nvar
    names = [data[pos + 33 * i : pos + 33 * (i + 1)].split(b"\0")[0].decode() for i in range(nvar)]
    pos += 33 * nvar + 2 * (nvar + 1) + (49 if version >= 114 else 12) * nvar + 33 * nvar
    pos += 81 * nvar  # variable labels
    while True:  # expansion fields
        kind, length = struct.unpack_from(e + "bi", data, pos)
        pos += 5 + length
        if kind == 0:
            break
    widths = [_DTA_NUMERIC[t][1] if t in _DTA_NUMERIC else t for t in types]
    dtype = np.dtype([(f"c{i}", e + _DTA_NUMERIC[t][0] if t in _DTA_NUMERIC else f"S{t}") for i, t in enumerate(types)])
    assert dtype.itemsize == sum(widths)
    rows = np.frombuffer(data, dtype=dtype, count=nobs, offset=pos)
    out = {}
    for i, (name, t) in enumerate(zip(names, types, strict=True)):
        col = rows[f"c{i}"]
        if t in _DTA_NUMERIC:
            v = col.astype(np.float64)
            v[col >= _DTA_MISSING[t]] = np.nan
            out[name] = v
        else:
            out[name] = np.char.decode(np.char.rstrip(col, b"\0"), "latin-1")
    return out


# ---------------------------------------------------------------- Yotov et al. (2016)

YOTOV = "AdvancedGuideFiles.zip"
YOTOV_YEARS = (1986, 1990, 1994, 1998, 2002, 2006)
YEAR_TERMS = tuple(f"ln_DIST_{y}" for y in YOTOV_YEARS)
GRAVITY_TERMS = ("CNTG", "LANG", "CLNY")

# Published PPML estimates and pair-clustered standard errors, transcribed from Yotov, Piermartini,
# Monteiro and Larch (2016), An Advanced Guide to Trade Policy Analysis: The Structural Gravity Model,
# WTO and UNCTAD: Table 1 column (4), p. 42; Table 2 columns (2) to (5), p. 47; Table 3 columns (2) to
# (4), p. 51. Each column: {term: (estimate, s.e.)}, observations, and R2 where printed.
YOTOV_PUBLISHED: dict[str, tuple[dict[str, tuple[float, float]], int, float | None]] = {
    "table1_ppml": (
        {"ln_DIST": (-0.841, 0.032), "CNTG": (0.437, 0.083), "LANG": (0.247, 0.077), "CLNY": (-0.222, 0.116)},
        28152,
        0.614,
    ),
    "table2_ppml": (
        {
            "ln_DIST_1986": (-0.859, 0.037),
            "ln_DIST_1990": (-0.834, 0.038),
            "ln_DIST_1994": (-0.835, 0.035),
            "ln_DIST_1998": (-0.847, 0.035),
            "ln_DIST_2002": (-0.848, 0.032),
            "ln_DIST_2006": (-0.836, 0.031),
            "CNTG": (0.437, 0.083),
            "LANG": (0.248, 0.077),
            "CLNY": (-0.222, 0.116),
        },
        28152,
        None,
    ),
    "table2_intra": (
        {
            "ln_DIST_1986": (-0.980, 0.072),
            "ln_DIST_1990": (-0.940, 0.073),
            "ln_DIST_1994": (-0.915, 0.072),
            "ln_DIST_1998": (-0.887, 0.071),
            "ln_DIST_2002": (-0.884, 0.071),
            "ln_DIST_2006": (-0.872, 0.071),
            "CNTG": (0.371, 0.140),
            "LANG": (0.337, 0.168),
            "CLNY": (0.019, 0.156),
            "ln_DIST_INTRA": (-0.488, 0.101),
        },
        28566,
        None,
    ),
    "table2_brdr": (
        {
            "ln_DIST_1986": (-0.857, 0.063),
            "ln_DIST_1990": (-0.819, 0.063),
            "ln_DIST_1994": (-0.796, 0.063),
            "ln_DIST_1998": (-0.770, 0.063),
            "ln_DIST_2002": (-0.767, 0.063),
            "ln_DIST_2006": (-0.754, 0.062),
            "CNTG": (0.574, 0.155),
            "LANG": (0.352, 0.137),
            "CLNY": (0.027, 0.125),
            "ln_DIST_INTRA": (-0.602, 0.109),
            "SMCTRY": (1.689, 0.574),
        },
        28566,
        None,
    ),
    "table2_fes": (
        {
            "ln_DIST_1986": (-0.910, 0.032),
            "ln_DIST_1990": (-0.879, 0.032),
            "ln_DIST_1994": (-0.860, 0.032),
            "ln_DIST_1998": (-0.833, 0.032),
            "ln_DIST_2002": (-0.829, 0.032),
            "ln_DIST_2006": (-0.811, 0.032),
            "CNTG": (0.442, 0.082),
            "LANG": (0.241, 0.076),
            "CLNY": (-0.220, 0.117),
        },
        28566,
        None,
    ),
    "table3_ppml": (
        {
            "ln_DIST": (-0.822, 0.031),
            "CNTG": (0.416, 0.083),
            "LANG": (0.250, 0.077),
            "CLNY": (-0.205, 0.114),
            "RTA": (0.191, 0.066),
        },
        28152,
        None,
    ),
    "table3_intra": (
        {
            "ln_DIST": (-0.800, 0.030),
            "CNTG": (0.393, 0.079),
            "LANG": (0.244, 0.077),
            "CLNY": (-0.182, 0.113),
            "RTA": (0.409, 0.069),
        },
        28566,
        None,
    ),
    "table3_endg": ({"RTA": (0.557, 0.102)}, 28482, None),
}


def _yotov_data(path, member: str) -> dict[str, np.ndarray]:
    with zipfile.ZipFile(path) as z:
        name = next(n for n in z.namelist() if n.endswith(f"Chapter1/Datasets/{member}"))
        d = read_dta(z.read(name))
    keep = np.isin(d["year"], YOTOV_YEARS)
    return {k: v[keep] for k, v in d.items()}


def _codes(*cols: np.ndarray) -> np.ndarray:
    keys = np.rec.fromarrays(cols)
    return np.unique(keys, return_inverse=True)[1]


def yotov(path) -> dict[str, ppml.Fit]:
    """The PPML columns of `YOTOV_PUBLISHED`, estimated as in the authors' Stata files
    (`TraditionalGravity.do`, `DistancePuzzle.do`, `RTAsEffects.do`): exporter-time and importer-time
    fixed effects, standard errors clustered by country pair."""
    fits = {}
    d = _yotov_data(path, "Chapter1Application1.dta")
    exp_t, imp_t, pair = _codes(d["exporter"], d["year"]), _codes(d["importer"], d["year"]), _codes(d["pair_id"])
    foreign = d["exporter"] != d["importer"]
    ln_dist = np.log(d["DIST"])
    sub = {k: v[foreign] for k, v in d.items()}
    fits["table1_ppml"] = ppml.hdfe(
        sub["trade"],
        [exp_t[foreign], imp_t[foreign]],
        {"ln_DIST": ln_dist[foreign], **{k: sub[k] for k in GRAVITY_TERMS}},
        pair[foreign],
    )

    d = _yotov_data(path, "Chapter1Application2.dta")
    exp_t, imp_t, pair = _codes(d["exporter"], d["year"]), _codes(d["importer"], d["year"]), _codes(d["pair_id"])
    foreign = d["exporter"] != d["importer"]
    smctry = (~foreign).astype(np.float64)
    ln_dist = np.log(d["DIST"])
    by_year = {f"ln_DIST_{y}": np.where(d["year"] == y, ln_dist, 0.0) for y in YOTOV_YEARS}
    gravity = {k: d[k] for k in GRAVITY_TERMS}
    fits["table2_ppml"] = ppml.hdfe(
        d["trade"][foreign],
        [exp_t[foreign], imp_t[foreign]],
        {k: v[foreign] for k, v in (by_year | gravity).items()},
        pair[foreign],
    )
    intra_years = {k: v * (1.0 - smctry) for k, v in by_year.items()}
    intra = intra_years | gravity | {"ln_DIST_INTRA": ln_dist * smctry}
    fits["table2_intra"] = ppml.hdfe(d["trade"], [exp_t, imp_t], intra, pair)
    fits["table2_brdr"] = ppml.hdfe(d["trade"], [exp_t, imp_t], intra | {"SMCTRY": smctry}, pair)
    intra_fe = np.where(foreign, "", d["exporter"])  # one group per country's internal trade, one for all international
    fits["table2_fes"] = ppml.hdfe(d["trade"], [exp_t, imp_t, _codes(intra_fe)], intra_years | gravity, pair)

    d = _yotov_data(path, "Chapter1Application3.dta")
    foreign = d["exporter"] != d["importer"]
    exp_t, imp_t = _codes(d["exporter"], d["year"]), _codes(d["importer"], d["year"])
    pair = _codes(d["pair_id"] + np.where(foreign, 0, 100000))  # as RTAsEffects.do: internal pairs apart
    ln_dist = np.log(d["DIST"])
    terms = {"ln_DIST": ln_dist, **{k: d[k] for k in GRAVITY_TERMS}, "RTA": d["RTA"]}
    fits["table3_ppml"] = ppml.hdfe(
        d["trade"][foreign], [exp_t[foreign], imp_t[foreign]], {k: v[foreign] for k, v in terms.items()}, pair[foreign]
    )
    intra_fe = np.where(foreign, "", d["exporter"])
    fits["table3_intra"] = ppml.hdfe(
        d["trade"],
        [exp_t, imp_t, _codes(intra_fe)],
        terms,
        pair,
    )
    fits["table3_endg"] = ppml.hdfe(d["trade"], [exp_t, imp_t, pair], {"RTA": d["RTA"]}, pair)
    return fits


# ---------------------------------------------------------------- S05 costs on the pack graph

ROAD, RAIL, SEA, AIR = 0, 1, 2, 3
RAIL_ACCESS, PORT_ACCESS, MINOR_PORT_ACCESS, AIR_ACCESS, CANAL = 4, 5, 6, 7, 8
# Path quantities per tonne: cost-relevant km by road, rail, sea and air (length times circuity times the
# terrain factor, the km that P04a-d price), handling and canal fees (USD), and hours.
N_Q = 6
Q_FEE, Q_HOURS = 4, 5
MODE_NAMES = ("road", "rail", "sea", "air")


@dataclass(frozen=True)
class Transport:
    """Mirror of `ceteris_geo::graph::TransportParams::default()` (geo SPEC, P04)."""

    per_tkm: tuple[float, ...] = (0.10, 0.04, 0.008, 0.80)  # P04a-d
    circuity: tuple[float, ...] = (1.3, 1.3, 1.0, 1.05)  # P04e-h
    speed_kmh: tuple[float, ...] = (40.0, 25.0, 25.0, 500.0)  # P04i-l
    fee: tuple[float, ...] = (5.0, 8.0, 20.0, 60.0)  # P04m: rail terminal, listed port, minor port, airport
    fee_hours: tuple[float, ...] = (24.0, 48.0, 72.0, 12.0)  # P04n
    canal_fee: float = 5.0  # P04o
    terrain_slope: float = 0.5  # P04p
    terrain_cap: float = 3.0  # P04q


P04 = Transport()

# P43 time cost of transit lambda_g per day, the starts of `TradeParams::default()` (trade SPEC), traded goods.
TIME_COST = (0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010)
# P01 classes of `TradeParams::default()` (ADR 0023) and their current values.
CLASSES = ((3,), (6,), (0, 1, 2, 4, 5))
THETA_DEFAULT = (2.62, 5.19, 10.57)
BORDER_DEFAULT = 0.3  # P05 start
TARIFF_GROUP = (0, 0, 0, 0, 1, 1, 1)  # ceteris_trade::goods::tariff_group: primary, manufactured
N_TRADED = 7


def edge_quantities(mode: np.ndarray, length_km: np.ndarray, tri_m: np.ndarray, p: Transport = P04) -> np.ndarray:
    """Per-edge path quantities (`N_Q` columns) as `ceteris_geo::graph::price` prices them."""
    mode, length, tri = np.asarray(mode), np.asarray(length_km, np.float64), np.asarray(tri_m, np.float64)
    q = np.zeros((len(mode), N_Q))
    phi = np.minimum(1.0 + p.terrain_slope * tri / 100.0, p.terrain_cap)
    for m in (ROAD, RAIL, SEA, AIR):
        on = mode == m
        km = length[on] * p.circuity[m]
        q[on, m] = km * (phi[on] if m in (ROAD, RAIL) else 1.0)
        q[on, Q_HOURS] = km / p.speed_kmh[m]
    for fee, m in enumerate((RAIL_ACCESS, PORT_ACCESS, MINOR_PORT_ACCESS, AIR_ACCESS)):
        on = mode == m
        km = length[on] * p.circuity[ROAD]
        q[on, ROAD] = km
        q[on, Q_FEE] = p.fee[fee]
        q[on, Q_HOURS] = km / p.speed_kmh[ROAD] + p.fee_hours[fee]
    on = mode == CANAL
    km = length[on] * p.circuity[SEA]
    q[on, SEA] = km
    q[on, Q_FEE] = p.canal_fee
    q[on, Q_HOURS] = km / p.speed_kmh[SEA]
    return q


def edge_cost(q: np.ndarray, p: Transport = P04) -> np.ndarray:
    """Cost per tonne of path quantities: P04a-d times the km by mode, plus fees."""
    return q[..., :4] @ np.array(p.per_tkm) + q[..., Q_FEE]


class Graph:
    """The pack's transport graph priced at `p`, for least-cost trees with path quantities."""

    def __init__(self, nodes: int, a: np.ndarray, b: np.ndarray, q: np.ndarray, p: Transport = P04):
        from scipy import sparse

        cost = edge_cost(q, p)
        u, v = np.concatenate([a, b]).astype(np.int64), np.concatenate([b, a]).astype(np.int64)
        edge = np.concatenate([np.arange(len(a)), np.arange(len(a))])
        order = np.lexsort((cost[edge], v, u))  # the cheapest of parallel edges first
        u, v, edge = u[order], v[order], edge[order]
        first = np.ones(len(u), dtype=bool)
        first[1:] = (u[1:] != u[:-1]) | (v[1:] != v[:-1])
        u, v, edge = u[first], v[first], edge[first]
        self.n = nodes
        self.key = u * nodes + v  # sorted
        self.edge = edge
        self.q = q
        # csgraph would drop explicit zeros; a zero-cost edge (one air edge of length 0) keeps a negligible cost
        self.matrix = sparse.csr_matrix((np.maximum(cost[edge], 1e-300), (u, v)), shape=(nodes, nodes))

    def trees(self, sources: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
        """Least cost from each source to every node, and the path quantities along each least-cost path
        (shape sources x nodes x `N_Q`); unreachable nodes have infinite cost and zero quantities."""
        from scipy.sparse import csgraph

        dist, pred = csgraph.dijkstra(self.matrix, indices=sources, return_predecessors=True)
        k, n = dist.shape
        node = np.broadcast_to(np.arange(n), (k, n))
        has = pred >= 0
        parent = np.where(has, pred, node)
        e = np.searchsorted(self.key, parent[has].astype(np.int64) * n + node[has])
        acc = np.zeros((k, n, N_Q))
        acc[has] = self.q[self.edge[e]]
        rows = np.arange(k)[:, None]
        while True:  # pointer jumping: each pass doubles the stretch of path summed into acc
            up = parent[rows, parent]
            acc += acc[rows, parent]
            if np.array_equal(up, parent):
                return dist, acc
            parent = up


@dataclass(frozen=True)
class CountryCosts:
    """Mean S05 path quantities between countries per traded good, `q[exporter, importer, good, N_Q]`,
    weighted by exporter province VA of the good times importer province VA; `reach` is False where no
    weighted province pair is connected."""

    q: np.ndarray
    reach: np.ndarray


def country_costs(pack, p: Transport = P04) -> CountryCosts:
    """S05 costs between and within the pack's countries as the trade kernel builds them
    (`EconInputs::network_from_graph`): domestic pairs by the least-cost path between provinces, and pairs
    of different countries by the cheapest chain of an access leg, a crossing leg between gateways and an
    access leg, over every gateway pair. Inside a province trade is costless (A2)."""
    from pyarrow import ipc

    def read(name):
        return ipc.open_file(str(pack / name)).read_all()

    nodes, edges, gw, prov = read("nodes.arrow"), read("edges.arrow"), read("gateways.arrow"), read("econ_provinces.arrow")
    countries = read("countries.arrow").column("code").to_pylist()
    province_country_code = read("provinces.arrow").column("country").to_pylist()
    index = {c: i for i, c in enumerate(countries)}
    country = np.array([index[c] for c in province_country_code])
    np_, nc = len(country), len(countries)
    va = np.column_stack([prov.column(f"va_{g}").to_numpy() for g in range(10)])
    q_edge = edge_quantities(edges["mode"].to_numpy(), edges["length_km"].to_numpy(), edges["tri_m"].to_numpy(), p)
    graph = Graph(nodes.num_rows, edges["a"].to_numpy(), edges["b"].to_numpy(), q_edge, p)
    gnode = gw["node"].to_numpy().astype(np.int64)
    gcountry = gw["country"].to_numpy().astype(np.int64)
    provinces_of = [np.flatnonzero(country == c) for c in range(nc)]
    gateways_of = [np.flatnonzero(gcountry == c) for c in range(nc)]

    dom_cost, dom_q, acc_cost, acc_q = {}, {}, {}, {}
    chunk = 64
    for start in range(0, np_, chunk):
        src = np.arange(start, min(start + chunk, np_))
        dist, q = graph.trees(src)
        for i, s in enumerate(src):
            c = country[s]
            dom_cost[s], dom_q[s] = dist[i, provinces_of[c]], q[i, provinces_of[c]]
            acc_cost[s], acc_q[s] = dist[i, gnode[gateways_of[c]]], q[i, gnode[gateways_of[c]]]
    x_cost = np.empty((len(gnode), len(gnode)))
    x_q = np.empty((len(gnode), len(gnode), N_Q))
    for start in range(0, len(gnode), chunk):
        src = np.arange(start, min(start + chunk, len(gnode)))
        dist, q = graph.trees(gnode[src])
        x_cost[src], x_q[src] = dist[:, gnode], q[:, gnode]

    weight_out = va[:, list(range(N_TRADED))]  # exporter: the good's VA
    weight_in = va.sum(axis=1)  # importer: all VA
    out = np.zeros((nc, nc, N_TRADED, N_Q))
    reach = np.zeros((nc, nc), dtype=bool)
    for a in range(nc):
        pa_ = provinces_of[a]
        if len(pa_) == 0 or weight_in[pa_].sum() == 0:
            continue
        wo = weight_out[pa_]
        wo = np.where(wo.sum(axis=0) > 0, wo, weight_in[pa_, None])  # a good the country does not make: all VA
        # domestic
        cost = np.stack([dom_cost[s] for s in pa_])
        q = np.stack([dom_q[s] for s in pa_])
        ok = np.isfinite(cost)
        w = wo[:, None, :] * (weight_in[pa_][None, :, None] * ok[..., None])
        if w.sum() > 0:
            out[a, a] = np.einsum("odg,odk->gk", w, q) / w.sum(axis=(0, 1))[:, None]
            reach[a, a] = True
        ga = gateways_of[a]
        if len(ga) == 0:
            continue
        ca = np.stack([acc_cost[s] for s in pa_])  # (provinces of a, gateways of a)
        qa = np.stack([acc_q[s] for s in pa_])
        # cheapest first two legs to every gateway b: min over a's gateways of access + crossing
        total = ca[:, :, None] + x_cost[ga][None]  # (o, a-gateway, b)
        first = np.argmin(total, axis=1)  # (o, b)
        u = np.take_along_axis(total, first[:, None, :], axis=1)[:, 0]
        for b in range(nc):
            pb, gb = provinces_of[b], gateways_of[b]
            if b == a or len(pb) == 0 or len(gb) == 0 or weight_in[pb].sum() == 0:
                continue
            cb = np.stack([acc_cost[s] for s in pb])  # (d, b-gateway)
            chain = u[:, gb][:, None, :] + cb[None]  # (o, d, b-gateway)
            last = np.argmin(chain, axis=2)  # (o, d)
            best = np.take_along_axis(chain, last[..., None], axis=2)[..., 0]
            ok = np.isfinite(best)
            if not ok.any():
                continue
            o_idx, d_idx = np.arange(len(pa_))[:, None], np.arange(len(pb))[None]
            ai = first[:, gb][o_idx, last]  # position of the gateway of a used among a's gateways, (o, d)
            qb = np.stack([acc_q[s] for s in pb])  # (d, b-gateway, N_Q)
            q = qa[o_idx, ai] + x_q[ga[ai], gb[last]] + qb[d_idx, last]
            q[~ok] = 0.0
            w = wo[:, None, :] * (weight_in[pb][None, :, None] * ok[..., None])
            if w.sum() > 0:
                out[a, b] = np.einsum("odg,odk->gk", w, q) / w.sum(axis=(0, 1))[:, None]
                reach[a, b] = True
    return CountryCosts(out, reach)


def tau(q: np.ndarray, weight: np.ndarray, time_cost: np.ndarray, p: Transport = P04) -> np.ndarray:
    """S05 iceberg factor 1 + c w_g + lambda_g T (trade SPEC, Leg factors) of path quantities."""
    return 1.0 + edge_cost(q, p) * weight + time_cost * q[..., Q_HOURS] / 24.0


# ---------------------------------------------------------------- flows


def _pack_index(codes, iso) -> dict[str, int]:
    from ..packs.econ import people

    index: dict[str, int] = {}
    for i, c in enumerate(codes):
        for s in (*people.PARTS.get(c, (iso.get(c, c),)), c):
            index.setdefault(s, i)
    for s, c in {"S19": "TWN", "SCG": "YUG"}.items():
        if c in index:
            index.setdefault(s, index[c])
    return index


def baci_flows(baci_2000, hs_key, codes, iso) -> np.ndarray:
    """2000 BACI trade by (exporter, importer, traded good) of the pack countries, USD; HS lines assigned to
    goods by the OECD HS to CPA key (`goods.hs_goods_shares`)."""
    import pyarrow as pa
    import pyarrow.csv as pacsv

    from ..packs.econ import goods

    shares = goods.hs_goods_shares(goods.read_hs_cpa(hs_key))
    hs = np.array(sorted(int(k) for k in shares))
    share = np.zeros((len(hs) + 1, N_TRADED))  # last row: lines outside the concordance
    for i, k in enumerate(hs):
        for g, s in shares[f"{k:06d}"].items():
            if g < N_TRADED:
                share[i, g] = s
    index = _pack_index(codes, iso)
    nc = len(codes)
    flows = np.zeros((nc * nc, N_TRADED))
    with zipfile.ZipFile(baci_2000) as z:
        names = z.namelist()
        with z.open(next(n for n in names if n.startswith("country_codes"))) as f:
            cc = pacsv.read_csv(f, convert_options=pacsv.ConvertOptions(column_types={"country_iso3": pa.string()}))
        to_pack = {k: index.get(v, -1) for k, v in zip(cc["country_code"].to_pylist(), cc["country_iso3"].to_pylist())}
        lookup = np.full(max(to_pack) + 1, -1)
        for k, v in to_pack.items():
            lookup[k] = v
        convert = pacsv.ConvertOptions(include_columns=["i", "j", "k", "v"], column_types={"k": pa.int64(), "v": pa.float64()})
        with z.open(next(n for n in names if n.endswith("Y2000_V202601.csv"))) as f:
            for batch in pacsv.open_csv(f, convert_options=convert):
                i = lookup[batch["i"].to_numpy()]
                j = lookup[batch["j"].to_numpy()]
                k = batch["k"].to_numpy()
                pos = np.searchsorted(hs, k)
                pos = np.where((pos < len(hs)) & (hs[np.minimum(pos, len(hs) - 1)] == k), pos, len(hs))
                ok = (i >= 0) & (j >= 0) & (i != j)
                v = batch["v"].to_numpy()[ok] * 1000.0
                cell = i[ok] * nc + j[ok]
                for g in range(N_TRADED):
                    flows[:, g] += np.bincount(cell, v * share[pos[ok], g], minlength=nc * nc)
    return flows.reshape(nc, nc, N_TRADED)


def icio_domestic(icio_zip, codes, iso, va_basis) -> np.ndarray:
    """Domestic flows by (pack country, traded good), USD: 2000 ICIO output less exports, for the countries
    that are ICIO economies (`va_basis` icio); NaN elsewhere and where exports exceed output."""
    from ..packs.econ import icio

    output = icio.aggregate(icio_zip)
    sold = icio.exports(icio_zip)
    out = np.full((len(codes), N_TRADED), np.nan)
    for i, (c, basis) in enumerate(zip(codes, va_basis, strict=True)):
        code = iso.get(c, c)
        if basis == "icio" and code in output and code != "ROW":
            d = (output[code].output - sold[code])[:N_TRADED] * 1e6
            out[i] = np.where(d >= 0, d, np.nan)
    return out


# ---------------------------------------------------------------- estimators


@dataclass(frozen=True)
class Sample:
    """One observation per (exporter, importer, traded good); `q` the S05 path quantities (`N_Q`
    columns) and `tau` their iceberg factor at the good's weight and time cost."""

    exporter: np.ndarray
    importer: np.ndarray
    good: np.ndarray
    flow: np.ndarray  # USD
    tariff: np.ndarray  # ad valorem
    tau: np.ndarray
    q: np.ndarray
    weight: np.ndarray  # tonnes per USD of the good
    time_cost: np.ndarray  # P43 lambda of the good

    def rows(self, mask: np.ndarray) -> "Sample":
        return Sample(*(getattr(self, f)[mask] for f in self.__dataclass_fields__))

    def fixed_effects(self) -> list[np.ndarray]:
        """Exporter-good and importer-good effects (outward and inward multilateral resistance)."""
        return [self.exporter * N_TRADED + self.good, self.importer * N_TRADED + self.good]

    def pairs(self) -> np.ndarray:
        """Unordered country pair: the cluster of Yotov et al. (2016)."""
        lo, hi = np.minimum(self.exporter, self.importer), np.maximum(self.exporter, self.importer)
        return lo * (hi.max() + 1) + hi


@dataclass(frozen=True)
class Theta:
    goods: tuple[int, ...]
    theta: float  # minus the coefficient on ln(1 + tariff)
    se: float
    tau: float  # coefficient on ln tau (S05)
    se_tau: float
    n: int
    pseudo_r2: float


def theta_by_class(s: Sample, classes=CLASSES) -> list[Theta]:
    """P01 per class: PPML of international flows of the class's goods on ln(1 + tariff) and ln tau (S05)
    with exporter-good and importer-good fixed effects, clustered by country pair."""
    out = []
    for goods_ in classes:
        sub = s.rows(np.isin(s.good, goods_) & (s.exporter != s.importer))
        fit = ppml.hdfe(
            sub.flow, sub.fixed_effects(), {"ln_tariff": np.log1p(sub.tariff), "ln_tau": np.log(sub.tau)}, sub.pairs()
        )
        out.append(
            Theta(tuple(goods_), -fit["ln_tariff"], fit.se("ln_tariff"), fit["ln_tau"], fit.se("ln_tau"), fit.n, fit.pseudo_r2)
        )
    return out


@dataclass(frozen=True)
class Border:
    nu: float  # P05, ad valorem: exp(sum_c share_c ln(1 + nu_c)) - 1
    se: float
    nu_by_class: tuple[float, ...]
    se_by_class: tuple[float, ...]
    theta_by_class: tuple[float, ...]  # from the same fit
    border_by_class: tuple[float, ...]  # coefficients on the border dummy
    n: int
    pseudo_r2: float


def border_friction(s: Sample, classes=CLASSES, shares=None) -> Border:
    """P05: PPML on a sample with domestic flows, per class a border dummy, ln(1 + tariff) and ln tau, with
    exporter-good and importer-good fixed effects, clustered by country pair. The border coefficient is
    -theta_c ln(1 + nu) (trade SPEC, two-level kernel), so ln(1 + nu_c) = b_c / beta_c with beta_c the
    tariff coefficient; nu pools the classes with weights `shares` (default equal). Delta-method errors."""
    member = [np.isin(s.good, g) for g in classes]
    border = (s.exporter != s.importer).astype(np.float64)
    reg = {}
    for c, m in enumerate(member):
        reg[f"ln_tariff_{c}"] = np.log1p(s.tariff) * m
        reg[f"ln_tau_{c}"] = np.log(s.tau) * m
        reg[f"border_{c}"] = border * m
    fit = ppml.hdfe(s.flow, s.fixed_effects(), reg, s.pairs())
    k = len(classes)
    w = np.full(k, 1.0 / k) if shares is None else np.asarray(shares, np.float64) / np.sum(shares)
    it = [fit.names.index(f"ln_tariff_{c}") for c in range(k)]
    ib = [fit.names.index(f"border_{c}") for c in range(k)]
    beta, b = fit.coef[it], fit.coef[ib]
    ln_nu = b / beta
    nu_c, se_c = [], []
    grad = np.zeros(len(fit.names))
    for c in range(k):
        g = np.zeros(len(fit.names))
        g[ib[c]], g[it[c]] = 1.0 / beta[c], -b[c] / beta[c] ** 2
        nu_c.append(float(np.expm1(ln_nu[c])))
        se_c.append(float(np.exp(ln_nu[c]) * np.sqrt(g @ fit.cov @ g)))
        grad += w[c] * g
    pooled = float(w @ ln_nu)
    se = float(np.exp(pooled) * np.sqrt(grad @ fit.cov @ grad))
    return Border(
        float(np.expm1(pooled)), se, tuple(nu_c), tuple(se_c), tuple((-beta).tolist()), tuple(b.tolist()), fit.n, fit.pseudo_r2
    )


@dataclass(frozen=True)
class Modes:
    ratio: np.ndarray  # P04b-d over P04a: rail, sea and air cost per tonne-km relative to road
    se: np.ndarray
    identified: np.ndarray  # False for a mode no path uses
    theta_by_class: tuple[float, ...]
    tau_by_class: tuple[float, ...]  # coefficient on ln tau(ratio)
    n: int
    pseudo_r2: float
    iterations: int


MODE_TOL = 1e-9
MODE_MAX_ITER = 50


def mode_costs(s: Sample, classes=CLASSES, p: Transport = P04) -> Modes:
    """P04 ratios: nonlinear PPML of international flows on, per class, ln(1 + tariff) and ln tau(r), with
    tau(r) = 1 + w (f_road (km_road + sum_m r_m km_m) + fees) + lambda hours / 24 the S05 factor along the
    least-cost paths and road at its P04a value as numeraire. Gauss-Newton: each step is a PPML on the
    same regressors plus kappa_c w f_road km_m / tau, whose coefficients update r; at convergence the
    clustered covariance of those coefficients is that of r."""
    s = s.rows(s.exporter != s.importer)
    member = [np.isin(s.good, g) for g in classes]
    km = s.q[:, 1:4]
    identified = km.max(axis=0) > 0
    r = np.array(p.per_tkm[1:]) / p.per_tkm[0]

    def tau_of(r):
        return 1.0 + s.weight * (p.per_tkm[0] * (s.q[:, 0] + km @ r) + s.q[:, Q_FEE]) + s.time_cost * s.q[:, Q_HOURS] / 24.0

    def base(t):
        reg = {}
        for c, m in enumerate(member):
            reg[f"ln_tariff_{c}"] = np.log1p(s.tariff) * m
            reg[f"ln_tau_{c}"] = np.log(t) * m
        return reg

    fit = ppml.hdfe(s.flow, s.fixed_effects(), base(tau_of(r)), s.pairs())
    for iteration in range(1, MODE_MAX_ITER + 1):
        t = tau_of(r)
        kappa = sum(fit[f"ln_tau_{c}"] * m for c, m in enumerate(member))
        reg = base(t) | {
            f"d_{MODE_NAMES[m + 1]}": kappa * s.weight * p.per_tkm[0] * km[:, m] / t for m in range(3) if identified[m]
        }
        fit = ppml.hdfe(s.flow, s.fixed_effects(), reg, s.pairs())
        step = np.array([fit[f"d_{MODE_NAMES[m + 1]}"] if identified[m] else 0.0 for m in range(3)])
        r = r + step
        if np.all(np.abs(step) <= MODE_TOL * np.maximum(1.0, np.abs(r))):
            break
    else:
        raise RuntimeError(f"mode costs did not converge in {MODE_MAX_ITER} steps: {r}")
    se = np.array([fit.se(f"d_{MODE_NAMES[m + 1]}") if identified[m] else np.nan for m in range(3)])
    return Modes(
        np.where(identified, r, np.nan),
        se,
        identified,
        tuple(-fit[f"ln_tariff_{c}"] for c in range(len(classes))),
        tuple(fit[f"ln_tau_{c}"] for c in range(len(classes))),
        fit.n,
        fit.pseudo_r2,
        iteration,
    )


# ---------------------------------------------------------------- 2000 BACI on the pack


@dataclass(frozen=True)
class Estimates:
    theta: list[Theta]
    border: Border
    modes: Modes
    class_shares: tuple[float, ...]  # 2000 BACI international trade by class, the weights of P05
    n_countries: int
    n_domestic_countries: int
    notes: dict


def samples(pack, cache) -> tuple[Sample, Sample, dict]:
    """The international sample (every pair of different pack countries connected by S05, zeros kept) and
    the sample with domestic flows (ICIO economies only, both ends), and counts for the report."""
    from pyarrow import ipc

    from ..packs.econ import build as econ_build
    from ..packs.econ import downscale, people

    def read(name):
        return ipc.open_file(str(pack / name)).read_all()

    codes = read("countries.arrow").column("code").to_pylist()
    iso, _ = people.read_codes(cache / downscale.ADMIN0)
    nc = len(codes)
    flows = baci_flows(cache / econ_build.BACI, cache / econ_build.HS_KEY, codes, iso)
    domestic = icio_domestic(cache / econ_build.ICIO, codes, iso, read("econ_countries.arrow").column("va_basis").to_pylist())
    t = read("econ_tariffs.arrow")
    rate = np.zeros((nc, nc, 2))
    rate[t["importer"].to_numpy(), t["exporter"].to_numpy(), t["group"].to_numpy()] = t["rate"].to_numpy()
    costs = country_costs(pack)
    weight = read("econ_goods.arrow").column("weight").to_numpy()[:N_TRADED]

    def build(mask):
        o, d, g = np.nonzero(mask)
        q = costs.q[o, d, g]
        w, lam = weight[g], np.array(TIME_COST)[g]
        flow = np.where(o == d, np.nan_to_num(domestic[o, g]), flows[o, d, g])
        tariff = np.where(o == d, 0.0, rate[d, o, np.array(TARIFF_GROUP)[g]])
        return Sample(o, d, g, flow, tariff, tau(q, w, lam), q, w, lam)

    pair = costs.reach & ~np.eye(nc, dtype=bool)
    international = build(np.broadcast_to(pair[..., None], (nc, nc, N_TRADED)))
    icio_ok = np.isfinite(domestic)  # (country, good)
    both = np.isfinite(domestic).all(axis=1)
    mask = (pair & both[:, None] & both[None, :])[..., None] & np.ones(N_TRADED, dtype=bool)
    eye = np.eye(nc, dtype=bool)[..., None] & icio_ok[:, None, :] & costs.reach[..., None]
    with_domestic = build(mask | eye)
    info = {
        "countries": nc,
        "countries_with_baci_trade": int((flows.sum(axis=(1, 2)) + flows.sum(axis=(0, 2)) > 0).sum()),
        "unreached_pairs": int((~costs.reach & ~np.eye(nc, dtype=bool)).sum()),
        "baci_international_usd": float(flows.sum()),
        "baci_international_usd_in_sample": float(international.flow.sum()),
        "domestic_countries": int(both.sum()),
        "domestic_cells_dropped": int((~icio_ok[both]).sum()),
        "class_trade_usd": [float(flows[..., list(c)].sum()) for c in CLASSES],
    }
    return international, with_domestic, info


def estimate(pack, cache) -> Estimates:
    international, with_domestic, info = samples(pack, cache)
    trade = np.array(info["class_trade_usd"])
    shares = tuple((trade / trade.sum()).tolist())
    return Estimates(
        theta_by_class(international),
        border_friction(with_domestic, CLASSES, shares),
        mode_costs(international),
        shares,
        info["countries"],
        info["domestic_countries"],
        info,
    )


# ---------------------------------------------------------------- outputs

OUT = Path(__file__).resolve().parents[1] / "packs" / "econ" / "gravity_2000.toml"
# The P04 rows are ratios to road (P04a), the numeraire of `mode_costs`.
P04_IDS = ("P04b/P04a", "P04c/P04a", "P04d/P04a")


def rows(est: Estimates, p: Transport = P04) -> list[dict]:
    """One row per estimated value with its standard error, observations, pseudo-R2 and current default."""
    out = []
    for c, t in enumerate(est.theta):
        out.append(
            {"id": "P01", "index": c, "goods": list(t.goods), "value": t.theta, "se": t.se, "n": t.n,
             "pseudo_r2": t.pseudo_r2, "default": THETA_DEFAULT[c]}
        )  # fmt: skip
    b = est.border
    out.append({"id": "P05", "index": 0, "goods": [], "value": b.nu, "se": b.se, "n": b.n, "pseudo_r2": b.pseudo_r2,
                "default": BORDER_DEFAULT})  # fmt: skip
    m = est.modes
    for k, name in enumerate(P04_IDS):
        out.append({"id": name, "index": 0, "goods": [], "value": float(m.ratio[k]), "se": float(m.se[k]), "n": m.n,
                    "pseudo_r2": m.pseudo_r2, "default": p.per_tkm[k + 1] / p.per_tkm[0]})  # fmt: skip
    return out


def _num(x: float) -> str:
    return "nan" if not np.isfinite(x) else repr(float(x))


def to_toml(est: Estimates) -> str:
    lines = [
        "# P01 theta by class, P05 border friction and P04 mode cost ratios, estimated by PPML structural gravity on",
        "# 2000 BACI with least-cost (S05) costs between the countries of the pack (ADR 0022; written by",
        "# `python -m ceteris.calibrate.gravity --write`).",
        "# TradeParams::default() keeps its values until the owner decides on the switch (PLAN Task 18).",
        'provenance = "data"',
        "",
    ]
    for r in rows(est):
        lines += [
            "[[estimate]]",
            f'id = "{r["id"]}"',
            f"index = {r['index']}",
            f"goods = {r['goods']}",
            f"value = {_num(r['value'])}",
            f"se = {_num(r['se'])}",
            f"n = {r['n']}",
            f"pseudo_r2 = {_num(r['pseudo_r2'])}",
            f"default = {_num(r['default'])}",
            "",
        ]
    return "\n".join(lines)


def stop_rules(est: Estimates) -> list[str]:
    """The task's stop rules (PLAN Task 18a): estimates that go to the owner instead of the pack."""
    fired = []
    for c, t in enumerate(est.theta):
        if not 1.0 <= t.theta <= 25.0 or t.se > abs(t.theta):
            fired.append(f"P01 class {c}: theta {t.theta:.3f} (s.e. {t.se:.3f})")
    if not 0.05 <= est.border.nu <= 3.0:
        fired.append(f"P05: nu {est.border.nu:.3f} (s.e. {est.border.se:.3f})")
    rail, sea, air = est.modes.ratio
    if not (sea < rail < 1.0 < air):  # water < rail < road < air, road = 1; NaN (unidentified) fails
        fired.append(f"P04: ratios to road rail {rail:.4g}, sea {sea:.4g}, air {air:.4g} break water < rail < road < air")
    return fired


def report(est: Estimates) -> str:
    lines = ["| ID | Class / mode | Estimate | s.e. | N | pseudo-R2 | Default |", "|---|---|---|---|---|---|---|"]
    for r in rows(est):
        label = f"class {r['index']}, goods {r['goods']}" if r["id"] == "P01" else ""
        lines.append(
            f"| {r['id']} | {label} | {r['value']:.4g} | {r['se']:.3g} | {r['n']} | {r['pseudo_r2']:.3f} | {r['default']:.4g} |"
        )
    b = est.border
    lines.append("")
    lines.append(f"P05 by class: nu {', '.join(f'{x:.3f}' for x in b.nu_by_class)} (s.e. {', '.join(f'{x:.3f}' for x in b.se_by_class)}); "
                 f"theta in that fit {', '.join(f'{x:.3f}' for x in b.theta_by_class)}; border coefficients "
                 f"{', '.join(f'{x:.3f}' for x in b.border_by_class)}; class weights {', '.join(f'{x:.3f}' for x in est.class_shares)}")  # fmt: skip
    m = est.modes
    lines.append(f"P04 fit: theta by class {', '.join(f'{x:.3f}' for x in m.theta_by_class)}, ln tau coefficients "
                 f"{', '.join(f'{x:.3f}' for x in m.tau_by_class)}, {m.iterations} Gauss-Newton steps")  # fmt: skip
    t = est.theta
    lines.append("ln tau coefficients in the theta fits: " + ", ".join(f"{x.tau:.3f} ({x.se_tau:.3f})" for x in t))
    return "\n".join(lines)


def main() -> int:
    import argparse
    import json

    from ..packs import fetch

    parser = argparse.ArgumentParser()
    parser.add_argument("--pack", type=Path, default=Path(__file__).resolve().parents[3] / "data" / "packs" / "2000")
    parser.add_argument("--write", action="store_true", help=f"write {OUT.name} unless a stop rule fires")
    parser.add_argument("--out", type=Path, default=OUT)
    args = parser.parse_args()
    est = estimate(args.pack, fetch.CACHE)
    print(report(est))
    print(json.dumps(est.notes, indent=2))
    fired = stop_rules(est)
    for f in fired:
        print(f"stop rule: {f}")
    if args.write and not fired:
        args.out.write_text(to_toml(est))
        print(f"wrote {args.out}")
    return 1 if fired else 0


if __name__ == "__main__":
    raise SystemExit(main())
