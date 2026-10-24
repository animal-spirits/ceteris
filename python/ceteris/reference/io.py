"""Inputs of the GE reference model, read from a `ceteris export` directory (export format 3)."""

import json
from dataclasses import dataclass, field
from functools import cached_property
from pathlib import Path

import numpy as np
from pyarrow import ipc

EXPORT_FORMAT = "3"


@dataclass(frozen=True)
class Entries:
    """Kernel legs of all traded goods. Domestic: row origin, col destination province. Access:
    row province, col gateway (origin access read forward, destination access backward).
    Crossing: row and col gateways; `border_tau` and `tariff` are set for crossings only."""

    good: np.ndarray
    row: np.ndarray
    col: np.ndarray
    k: np.ndarray
    tau: np.ndarray
    border_tau: np.ndarray | None = None
    tariff: np.ndarray | None = None


@dataclass(frozen=True)
class Inputs:
    theta: np.ndarray  # (G,) P01 per good, 0 for local goods (ADR 0018)
    sigma_demand: float
    sigma_fl: float
    weight_floor: float
    traded: np.ndarray  # (G,) bool
    province_country: np.ndarray  # (P,)
    gateway_country: np.ndarray  # (N,)
    labour_supply: np.ndarray  # (P,) effective units, sum_k e_k LF_k
    population: np.ndarray  # (P,) sum of the province's pops, rebate weights
    w0: np.ndarray  # (P,)
    s_l: np.ndarray  # (C,)
    unemployment: np.ndarray  # (C,)
    omega: np.ndarray  # (C, G)
    io: np.ndarray  # (C, G, G), io[c, k, g] = a_kg
    active: np.ndarray  # (P, G) bool: origin markets of traded goods, province markets of local goods
    ent_province: np.ndarray  # (E,)
    ent_good: np.ndarray  # (E,)
    ent_a: np.ndarray  # (E,)
    ent_va0: np.ndarray  # (E,)
    ent_l0: np.ndarray  # (E,)
    domestic: Entries
    access: Entries
    crossing: Entries
    static_digest: str = field(default="")

    @property
    def n_provinces(self) -> int:
        return len(self.province_country)

    @property
    def n_goods(self) -> int:
        return len(self.traded)

    @property
    def n_countries(self) -> int:
        return len(self.s_l)

    @property
    def n_gateways(self) -> int:
        return len(self.gateway_country)

    @cached_property
    def ent_country(self) -> np.ndarray:
        return self.province_country[self.ent_province]

    @cached_property
    def ent_inputs(self) -> np.ndarray:
        """(E, G): a_kg of each enterprise's sector, by input k."""
        return self.io[self.ent_country, :, self.ent_good]

    @cached_property
    def ent_a_va(self) -> np.ndarray:
        return 1.0 - self.ent_inputs.sum(axis=1)

    @cached_property
    def ent_s_l(self) -> np.ndarray:
        return self.s_l[self.ent_country]

    @cached_property
    def omega_p(self) -> np.ndarray:
        """(P, G) final demand shares of each province's country."""
        return self.omega[self.province_country]

    @cached_property
    def active_cells(self) -> np.ndarray:
        """Flat indices `province * G + good` of active cells, row-major: the price unknowns."""
        return np.flatnonzero(self.active.ravel())

    @cached_property
    def base_income(self) -> np.ndarray:
        """Household income per province in the data state, as the Rust world mints and pays it:
        w0 (1 - u) L^s wages plus the profit pool sum (1 - s_L) VA0."""
        u = self.unemployment[self.province_country]
        pool = np.bincount(self.ent_province, (1.0 - self.ent_s_l) * self.ent_va0, self.n_provinces)
        return self.w0 * (1.0 - u) * self.labour_supply + pool

    @cached_property
    def index_weights(self) -> np.ndarray:
        """(P, G) base household consumption quantities at prices 1: weights of the world price index."""
        om = self.omega_p
        return om / om.sum(axis=1, keepdims=True) * self.base_income[:, None]

    @cached_property
    def base_output(self) -> np.ndarray:
        """(P, G) base output Y0 = A VA0 / a_VA of each cell's enterprises."""
        out = np.zeros(self.active.shape)
        np.add.at(out, (self.ent_province, self.ent_good), self.ent_a * self.ent_va0 / self.ent_a_va)
        return out

    @cached_property
    def base_demand(self) -> np.ndarray:
        """(P, G) households' base demand plus one month of base input use (Y0 = A VA0 / a_VA)."""
        y0 = self.ent_a * self.ent_va0 / self.ent_a_va
        out = self.index_weights.copy()
        np.add.at(out, self.ent_province, self.ent_inputs * y0[:, None])
        return out


def _table(path: Path) -> dict[str, np.ndarray]:
    t = ipc.open_file(str(path)).read_all()
    return {name: t.column(name).to_numpy() for name in t.column_names}


def _entries(t: dict[str, np.ndarray], row: str, col: str, crossing: bool = False) -> Entries:
    extra = {"border_tau": t["border_tau"].astype(np.float64), "tariff": t["tariff"].astype(np.float64)} if crossing else {}
    return Entries(
        good=t["good"].astype(np.int64),
        row=t[row].astype(np.int64),
        col=t[col].astype(np.int64),
        k=t["k"].astype(np.float64),
        tau=t["tau"].astype(np.float64),
        **extra,
    )


def load(directory: Path | str) -> Inputs:
    d = Path(directory)
    params = json.loads((d / "params.json").read_text())
    if params["format"] != EXPORT_FORMAT:
        raise ValueError(f"export format {params['format']}, expected {EXPORT_FORMAT}")
    goods = _table(d / "goods.arrow")
    countries = _table(d / "countries.arrow")
    provinces = _table(d / "provinces.arrow")
    pops = _table(d / "pops.arrow")
    ent = _table(d / "enterprises.arrow")
    active = _table(d / "active.arrow")
    io = _table(d / "country_io.arrow")

    n_goods = params["n_goods"]
    n_c, n_p = len(countries["country"]), len(provinces["province"])
    a = np.zeros((n_c, n_goods, n_goods))
    a[io["country"], io["input"], io["sector"]] = io["a"]
    act = np.zeros((n_p, n_goods), dtype=bool)
    act[active["province"], active["good"]] = active["active"]
    edu_eff = np.asarray(params["econ"]["edu_eff"])
    pop_province = pops["province"].astype(np.int64)
    inp = Inputs(
        theta=goods["theta"].astype(np.float64),
        sigma_demand=float(params["econ"]["sigma_demand"]),
        sigma_fl=float(params["econ"]["sigma_fl"]),
        weight_floor=float(params["weight_floor"]),
        traded=goods["traded"].astype(bool),
        province_country=provinces["country"].astype(np.int64),
        gateway_country=_table(d / "gateways.arrow")["country"].astype(np.int64),
        labour_supply=np.bincount(pop_province, edu_eff[pops["edu"]] * pops["labour_force"], n_p),
        population=np.bincount(pop_province, pops["population"], n_p),
        w0=provinces["w0"].astype(np.float64),
        s_l=countries["s_l"],
        unemployment=countries["unemployment"],
        omega=np.stack([countries[f"omega_{g}"] for g in range(n_goods)], axis=1),
        io=a,
        active=act,
        ent_province=ent["province"].astype(np.int64),
        ent_good=ent["good"].astype(np.int64),
        ent_a=ent["a"].astype(np.float64),
        ent_va0=ent["va0"].astype(np.float64),
        ent_l0=ent["l0"].astype(np.float64),
        domestic=_entries(_table(d / "kernel_domestic.arrow"), "o", "d"),
        access=_entries(_table(d / "kernel_access.arrow"), "province", "gateway"),
        crossing=_entries(_table(d / "kernel_crossing.arrow"), "a", "b", crossing=True),
        static_digest=params["static_digest"],
    )
    validate(inp)
    return inp


def validate(inp: Inputs) -> None:
    """Rejects inputs whose Rust steady state the model does not encode."""
    local = ~inp.traded
    cells = np.zeros(inp.active.shape, dtype=bool)
    cells[inp.ent_province, inp.ent_good] = True
    stuck = cells & ~inp.active & local[None, :]
    if stuck.any():
        # Rust keeps such a cell's price at its initial 1, which is not homogeneous in prices.
        raise ValueError(f"inactive local cells with an enterprise: {np.argwhere(stuck).tolist()}")
    if np.any(inp.ent_a_va <= 0):
        raise ValueError("an enterprise has a nonpositive value-added coefficient")
