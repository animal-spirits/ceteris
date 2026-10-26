"""Inputs of the GE reference model, read from a `ceteris export` directory (export format 9)."""

import json
from dataclasses import dataclass, field
from functools import cached_property
from pathlib import Path

import numpy as np
from pyarrow import ipc

from . import demand

EXPORT_FORMAT = "9"


@dataclass(frozen=True)
class Entries:
    """Kernel legs of all traded goods. Domestic: row origin, col destination province. Access:
    row province, col gateway (origin access read forward, destination access backward).
    Crossing: row and col gateways, `k` with the gateway masses (ADR 0092). `inv_tau` is the share
    of shipped units that arrives over a domestic or access leg; crossings carry `inv_loss` (the
    same, border loss included), `inv_duty` (1 / (1 + tariff)) and `lag` (transit months)
    instead."""

    good: np.ndarray
    row: np.ndarray
    col: np.ndarray
    k: np.ndarray
    inv_tau: np.ndarray | None = None
    inv_loss: np.ndarray | None = None
    inv_duty: np.ndarray | None = None
    lag: np.ndarray | None = None


@dataclass(frozen=True)
class Inputs:
    theta: np.ndarray  # (G,) P01 per good, 0 for local goods (ADR 0018)
    sigma_demand: float  # P06, within a PIGL category
    pigl: demand.Pigl  # P46, P47
    category: np.ndarray  # (G,) PIGL category of each good
    sigma_fl: float
    traded: np.ndarray  # (G,) bool
    province_country: np.ndarray  # (P,)
    gateway_country: np.ndarray  # (N,)
    labour_supply: np.ndarray  # (P,) effective units, sum_k e_k LF_k
    population: np.ndarray  # (P,) sum of the province's pops, rebate weights
    w0: np.ndarray  # (P,)
    base_price: np.ndarray  # (P, G) base delivered unit price q_d (1 for local goods, inf where unreachable)
    s_l: np.ndarray  # (C,)
    unemployment: np.ndarray  # (C,) 2000 rate (data)
    u_star: np.ndarray  # (C,) natural rate (ADR 0089): labour clears at (1 - u*) L^s
    gini: np.ndarray  # (C,)
    omega: np.ndarray  # (C, G)
    io: np.ndarray  # (C, G, G), io[c, k, g] = a_kg
    active: np.ndarray  # (P, G) bool: origin markets of traded goods, province markets of local goods
    ent_province: np.ndarray  # (E,)
    ent_good: np.ndarray  # (E,)
    ent_a: np.ndarray  # (E,)
    ent_va0: np.ndarray  # (E,)
    ent_l0: np.ndarray  # (E,)
    pop_province: np.ndarray  # (K,)
    pop_effective: np.ndarray  # (K,) e_k LF_k: the pop's share of its province's wages and dividends
    pop_population: np.ndarray  # (K,)
    domestic: Entries
    access: Entries
    crossing: Entries
    ent_va_target: np.ndarray | None = None  # (E,) value added the calibration matches
    coverage: np.ndarray | None = None  # (G,) P02 merchant stock target in months of orders
    pop_edu: np.ndarray | None = None  # (K,) education level
    static_digest: str = field(default="")
    # A calibrated steady state (`calibrate.reference_inputs`) replaces the base column, its a_VA and
    # the PIGL weights inverted at prices 1.
    calibrated_coef: np.ndarray | None = None  # (E, G)
    calibrated_a_va: np.ndarray | None = None  # (E,)
    calibrated_weights: demand.Weights | None = None
    gateway_mass: np.ndarray | None = None  # (N,) share of the country's trade throughput (ADR 0092)

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
        """(E, G): physical input coefficients, as `EconInputs::base_column`: the sector's ICIO
        cost share a_kg over the base delivered price of k at the enterprise's province, 0 where
        the sector does not use k."""
        if self.calibrated_coef is not None:
            return self.calibrated_coef
        a = self.io[self.ent_country, :, self.ent_good]
        with np.errstate(divide="ignore", invalid="ignore"):
            return np.where(a > 0, a / self.base_price[self.ent_province], 0.0)

    @cached_property
    def ent_a_va(self) -> np.ndarray:
        """1 - sum_k coef_k q_k at base prices, as `EconInputs::base_a_va` (1 - sum_k a_kg)."""
        if self.calibrated_a_va is not None:
            return self.calibrated_a_va
        coef = self.ent_inputs
        cost = np.zeros(len(coef))
        for k in range(coef.shape[1]):  # summed in Rust's order
            cost += np.where(coef[:, k] > 0, coef[:, k] * self.base_price[self.ent_province, k], 0.0)
        return 1.0 - cost

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
    def employed_supply(self) -> np.ndarray:
        """(P,) effective labour employed at the natural rate, (1 - u*) L^s: the frictionless
        wage clears labour there (ADR 0089, as Rust's E1)."""
        return (1.0 - self.u_star[self.province_country]) * self.labour_supply

    @cached_property
    def base_income(self) -> np.ndarray:
        """Household income per province at the base point, rebates aside: w0 (1 - u*) L^s wages
        plus the province's population share of its country's profit pool sum (1 - s_L) VA0
        (national ownership, owner 2026-10-25), as `world::base_income`."""
        c = self.province_country
        u = self.u_star[c]
        pool = np.bincount(c[self.ent_province], (1.0 - self.ent_s_l) * self.ent_va0, self.n_countries)
        people = np.bincount(c, self.population, self.n_countries)
        share = np.divide(self.population, people[c], out=np.zeros(self.n_provinces), where=people[c] > 0)
        return self.w0 * (1.0 - u) * self.labour_supply + share * pool[c]

    @cached_property
    def base_spend(self) -> np.ndarray:
        """(C,) base spending per person: base income over the pops' population, 0 for a country
        without people, as `world::base_spend`."""
        c = self.province_country
        income = np.bincount(c, self.base_income, self.n_countries)
        people = np.bincount(c[self.pop_province], self.pop_population, self.n_countries)
        return np.divide(income, people, out=np.zeros(self.n_countries), where=people > 0)

    @cached_property
    def sigma_income(self) -> np.ndarray:
        """(C,) log income dispersion of every pop of a country, from its Gini."""
        return demand.lognormal_sigma(self.gini)

    @cached_property
    def pigl_weights(self) -> demand.Weights:
        """PIGL weights per country, as `world::household_preferences`."""
        if self.calibrated_weights is not None:
            return self.calibrated_weights
        return demand.invert(self.omega, self.base_spend, self.sigma_income, self.category, self.pigl)

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
    extra = (
        {"inv_loss": t["inv_loss"], "inv_duty": t["inv_duty"], "lag": t["lag"].astype(np.int64)}
        if crossing
        else {"inv_tau": t["inv_tau"]}
    )
    return Entries(
        good=t["good"].astype(np.int64),
        row=t[row].astype(np.int64),
        col=t[col].astype(np.int64),
        k=t["k"].astype(np.float64),
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
    gateways = _table(d / "gateways.arrow")

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
        pigl=demand.Pigl(
            sigma_b=float(params["econ"]["pigl_sigma_b"]),
            phi=float(params["econ"]["pigl_phi"]),
            gamma=float(params["econ"]["pigl_gamma"]),
            theta=np.asarray(params["econ"]["pigl_theta"], dtype=np.float64),
            eps=float(params["econ"]["pigl_eps"]),
        ),
        category=goods["category"].astype(np.int64),
        sigma_fl=float(params["econ"]["sigma_fl"]),
        traded=goods["traded"].astype(bool),
        province_country=provinces["country"].astype(np.int64),
        gateway_country=gateways["country"].astype(np.int64),
        labour_supply=np.bincount(pop_province, edu_eff[pops["edu"]] * pops["labour_force"], n_p),
        population=np.bincount(pop_province, pops["population"], n_p),
        w0=provinces["w0"].astype(np.float64),
        base_price=np.stack([provinces[f"base_price_{g}"] for g in range(n_goods)], axis=1).astype(np.float64),
        s_l=countries["s_l"],
        unemployment=countries["unemployment"],
        u_star=countries["u_star"],
        gini=countries["gini"],
        omega=np.stack([countries[f"omega_{g}"] for g in range(n_goods)], axis=1),
        io=a,
        active=act,
        ent_province=ent["province"].astype(np.int64),
        ent_good=ent["good"].astype(np.int64),
        ent_a=ent["a"].astype(np.float64),
        ent_va0=ent["va0"].astype(np.float64),
        ent_l0=ent["l0"].astype(np.float64),
        pop_province=pop_province,
        pop_effective=edu_eff[pops["edu"]] * pops["labour_force"],
        pop_population=pops["population"].astype(np.float64),
        domestic=_entries(_table(d / "kernel_domestic.arrow"), "o", "d"),
        access=_entries(_table(d / "kernel_access.arrow"), "province", "gateway"),
        crossing=_entries(_table(d / "kernel_crossing.arrow"), "a", "b", crossing=True),
        ent_va_target=ent["va_target"].astype(np.float64),
        coverage=goods["coverage"].astype(np.float64),
        pop_edu=pops["edu"].astype(np.int64),
        static_digest=params["static_digest"],
        gateway_mass=gateways["mass"],
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
    _ = inp.pigl_weights  # raises where a country's final demand shares are outside the PIGL form
