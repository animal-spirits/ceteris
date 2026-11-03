//! A P2 world from the 2000 pack (pack format 3; `specs/econ/SPEC.md`, Pack additions).
//!
//! The economy's provinces are the pack provinces with value added and a labour force:
//! a province below the working-age floor of the pipeline has no value added, and an
//! uninhabited one has no pops, so neither can carry an enterprise or a wage. They stay in the
//! transport graph, which paths cross, and `ProvinceData::pack` maps each economy province
//! back to its pack province.

use std::path::Path;

use ceteris_core::rows::Rows;
use ceteris_geo::graph::{PathChoice, TransportGraph};
use ceteris_geo::pack::{Pack, PackError, bools, f64s, read_table, strings, u8s, u32s};
use ceteris_geo::paths::{Tree, shortest_by};
use ceteris_trade::goods::p2;
use ceteris_trade::{BulkLegs, GoodLegs, Goods, Leg, LegSet, Network, TradeParams, Wedges, kappa};
use rayon::prelude::*;

use crate::inputs::{Countries, EconInputs, EntData, N_EDU, PopData, ProvinceData};
use crate::params::EconParams;

/// Million USD per year, the pack's VA unit, to numeraire (USD) per month.
const VA_TO_MONTHLY: f64 = 1e6 / 12.0;

fn err(msg: impl Into<String>) -> PackError {
    PackError(msg.into())
}

/// Every country with enterprises has people to own them: national ownership divides its
/// profit pool by the population of its pops (`EconWorld::new`).
fn check_owners(
    codes: &[String],
    provinces: &ProvinceData,
    pops: &PopData,
    ent: &EntData,
) -> Result<(), PackError> {
    let mut people = vec![0.0; codes.len()];
    for (&i, &n) in pops.province.iter().zip(&pops.population) {
        people[usize::from(provinces.country[i as usize])] += n;
    }
    match ent
        .province
        .iter()
        .map(|&i| usize::from(provinces.country[i as usize]))
        .find(|&c| people[c] <= 0.0)
    {
        Some(c) => Err(err(format!(
            "country {} has enterprises but no population to own them",
            codes[c]
        ))),
        None => Ok(()),
    }
}

/// Gateways of the pack (`gateways.arrow`): graph node and country of each, its kind
/// (0 port, 1 airport, 2 land crossing) and its mass, the share of its country's trade
/// throughput (ADR 0092; the pack builder's source is in the econ SPEC, Gateways).
#[derive(Clone, Debug, PartialEq)]
pub struct Gateways {
    pub node: Vec<u32>,
    pub country: Vec<u16>,
    pub kind: Vec<u8>,
    pub mass: Vec<f64>,
}

/// Largest deviation from 1 of a country's summed gateway masses (as `Kernel::build` checks).
const MASS_TOLERANCE: f64 = 1e-12;

/// Each gateway's mass in (0, 1], and each country's masses summing to 1 (ADR 0092).
fn check_masses(country: &[u16], mass: &[f64]) -> Result<(), PackError> {
    let mut total: std::collections::BTreeMap<u16, f64> = std::collections::BTreeMap::new();
    for (a, (&c, &m)) in country.iter().zip(mass).enumerate() {
        // also rejects NaN
        if !(m > 0.0 && m <= 1.0) {
            return Err(err(format!("gateways.arrow: gateway {a} has mass {m}")));
        }
        *total.entry(c).or_insert(0.0) += m;
    }
    match total
        .iter()
        .find(|(_, t)| (*t - 1.0).abs() > MASS_TOLERANCE)
    {
        Some((c, t)) => Err(err(format!(
            "gateways.arrow: the gateway masses of country {c} sum to {t}, not 1 (ADR 0092)"
        ))),
        None => Ok(()),
    }
}

impl Gateways {
    pub fn read(dir: &Path) -> Result<Self, PackError> {
        let t = read_table(&dir.join("gateways.arrow"))?;
        let country: Vec<u16> = u32s(&t, "country")?
            .into_iter()
            .map(|c| u16::try_from(c).map_err(|_| err(format!("gateway country {c}"))))
            .collect::<Result<_, _>>()?;
        if t.column_by_name("mass").is_none() {
            return Err(err(format!(
                "{} has no mass column (gateway masses, ADR 0092): the pack's econ tables \
                 predate it; rebuild them with `ceteris pack build --econ-only`",
                dir.join("gateways.arrow").display()
            )));
        }
        let mass = f64s(&t, "mass")?;
        check_masses(&country, &mass)?;
        Ok(Self {
            node: u32s(&t, "node")?,
            country,
            kind: u8s(&t, "kind")?,
            mass,
        })
    }

    pub fn len(&self) -> usize {
        self.node.len()
    }

    pub fn is_empty(&self) -> bool {
        self.node.is_empty()
    }
}

fn expect_rows(what: &str, got: usize, want: usize) -> Result<(), PackError> {
    if got == want {
        Ok(())
    } else {
        Err(err(format!("{what}: {got} rows, expected {want}")))
    }
}

/// Columns `{prefix}0` to `{prefix}{n - 1}`, as one row of `n` goods per table row.
fn per_good(
    batch: &arrow_array::RecordBatch,
    prefix: &str,
    n: usize,
) -> Result<Rows<f64>, PackError> {
    let cols = (0..n)
        .map(|g| f64s(batch, &format!("{prefix}{g}")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Rows::from_flat(
        (0..batch.num_rows())
            .flat_map(|r| cols.iter().map(move |c| c[r]))
            .collect(),
        n,
    ))
}

/// The goods table of `econ_goods`: one good per row, traded goods first. A pack without
/// `storable`, `tariff_group` and `category` columns (the 2000 pack, format 3) must hold the P2
/// goods, whose values those are.
fn read_goods(goods: &arrow_array::RecordBatch) -> Result<Goods, PackError> {
    let name = strings(goods, "name")?;
    let traded = bools(goods, "traded")?;
    let n_traded = traded.iter().take_while(|&&t| t).count();
    if traded[n_traded..].contains(&true) {
        return Err(err("econ_goods: traded goods must come first"));
    }
    let has = |c: &str| goods.column_by_name(c).is_some();
    if has("storable") && has("tariff_group") && has("category") {
        let storable = bools(goods, "storable")?;
        let group = u8s(goods, "tariff_group")?;
        let category = u8s(goods, "category")?;
        if storable[n_traded..].contains(&true)
            || group.iter().any(|&g| g > 1)
            || category.iter().any(|&c| c > 2)
        {
            return Err(err(
                "econ_goods: a storable local good, a tariff group above 1 or a category above 2",
            ));
        }
        return Ok(Goods::new(name, n_traded, storable, group, category));
    }
    let p2 = Goods::p2();
    if name != p2.name || n_traded != p2::TRADED {
        return Err(err(
            "econ_goods: goods other than P2's need storable, tariff_group and category columns",
        ));
    }
    Ok(p2)
}

impl EconInputs {
    /// Loads the P2 world from the econ tables of a pack in `dir` and its transport graph,
    /// [`EconInputs::from_pack_data`], calibrated by the tables of ADR 0089 when the pack
    /// holds them ([`EconInputs::calibrate`]): a world from these inputs starts from the
    /// calibrated steady state.
    pub fn from_pack(pack: &Pack, dir: &Path) -> Result<EconInputs, PackError> {
        let mut inputs = EconInputs::from_pack_data(pack, dir)?;
        if EconInputs::has_calibration(dir) {
            inputs.calibrate(dir)?;
        }
        Ok(inputs)
    }

    /// The uncalibrated P2 world of a pack, the data state, which the calibration starts from
    /// and `ceteris export` writes. Leg costs are least-cost paths
    /// ([`EconInputs::network_from_graph`]); A is 1. The base point is filled with the default
    /// education efficiencies (`EconParams::default().edu_eff`) at the natural rate u*, base
    /// prices under the default trade parameters, coefficients with one vintage.
    pub fn from_pack_data(pack: &Pack, dir: &Path) -> Result<EconInputs, PackError> {
        let table = |name: &str| read_table(&dir.join(name));
        let c = table("econ_countries.arrow")?;
        let codes = strings(&c, "code")?;
        let nc = codes.len();
        let country_of = |code: &str| {
            codes
                .iter()
                .position(|x| x == code)
                .and_then(|i| u16::try_from(i).ok())
                .ok_or_else(|| err(format!("province country {code} not in econ_countries")))
        };
        let goods = table("econ_goods.arrow")?;
        let weight = f64s(&goods, "weight")?;
        // runs read P02 from the trade parameters, so a pack column that disagrees is stale
        let coverage = f64s(&goods, "coverage")?;
        let goods = read_goods(&goods)?;
        let default = TradeParams::default();
        if goods == default.goods && coverage != default.coverage {
            return Err(err(format!(
                "econ_goods: coverage {coverage:?} is not TradeParams::coverage (P02) {:?}",
                default.coverage
            )));
        }
        let ng = goods.n();
        let io_table = table("econ_io.arrow")?;
        let a = f64s(&io_table, "a")?;
        expect_rows("econ_io", a.len(), nc * ng * ng)?;
        let countries = Countries {
            omega: per_good(&c, "fd_", ng)?,
            io: Rows::from_flat(a, ng * ng),
            s_l: f64s(&c, "labour_share")?,
            unemployment: f64s(&c, "unemployment")?,
            u_star: f64s(&c, "u_star")?,
            gini: f64s(&c, "gini")?,
            pigl: None,
        };

        let t = table("econ_tariffs.arrow")?;
        let rate = f64s(&t, "rate")?;
        expect_rows("econ_tariffs", rate.len(), 2 * nc * nc)?;
        let (imp, exp, group) = (
            u32s(&t, "importer")?,
            u32s(&t, "exporter")?,
            u8s(&t, "group")?,
        );
        let in_order = (0..rate.len()).all(|r| {
            imp[r] as usize == r / (2 * nc)
                && exp[r] as usize == (r / 2) % nc
                && usize::from(group[r]) == r % 2
        });
        if !in_order {
            return Err(err(
                "econ_tariffs rows are not importer, exporter, group order",
            ));
        }
        let wedges = Wedges {
            tariff: rate.iter().map(|&r| r as f32).collect(),
        };

        let p = table("econ_provinces.arrow")?;
        let n = pack.provinces.len();
        let va = per_good(&p, "va_", ng)?;
        expect_rows("econ_provinces", va.len(), n)?;
        let population = f64s(&p, "population")?;
        let pops = table("econ_pops.arrow")?;
        let (pop_province, edu) = (u32s(&pops, "province")?, u8s(&pops, "edu")?);
        let (pop_people, labour_force) = (f64s(&pops, "population")?, f64s(&pops, "labour_force")?);
        expect_rows("econ_pops", pop_province.len(), N_EDU * n)?;
        let mut lf = vec![0.0; n];
        for (k, &i) in pop_province.iter().enumerate() {
            lf[i as usize] += labour_force[k];
        }

        // pack province -> economy province
        let mut index = vec![u32::MAX; n];
        let mut provinces = ProvinceData {
            base_price: Rows::new(ng),
            gamma: Rows::new(ng),
            ..ProvinceData::default()
        };
        for i in 0..n {
            let has_va = va[i].iter().sum::<f64>() > 0.0;
            if has_va && lf[i] <= 0.0 {
                // the pipeline gives VA only to provinces with workers; leaving one out loses VA
                return Err(err(format!(
                    "pack province {i} has value added but no labour force"
                )));
            }
            if has_va {
                index[i] = provinces.pack.len() as u32;
                provinces.pack.push(i as u32);
                provinces
                    .country
                    .push(country_of(&pack.provinces.country[i])?);
                provinces.population.push(population[i]);
            }
        }
        let mut pop_data = PopData::default();
        for k in 0..pop_province.len() {
            let i = index[pop_province[k] as usize];
            if i != u32::MAX {
                pop_data.province.push(i);
                pop_data.edu.push(edu[k]);
                pop_data.population.push(pop_people[k]);
                pop_data.labour_force.push(labour_force[k]);
            }
        }
        let mut ent = EntData::default();
        for (i, &node) in provinces.pack.iter().enumerate() {
            for (g, &v) in va[node as usize].iter().enumerate() {
                if v > 0.0 {
                    let va0 = v * VA_TO_MONTHLY;
                    ent.province.push(i as u32);
                    ent.good.push(g as u8);
                    ent.a.push(1.0);
                    ent.va0.push(va0);
                    ent.va_target.push(va0);
                }
            }
        }

        check_owners(&codes, &provinces, &pop_data, &ent)?;
        let mut inputs = EconInputs {
            goods,
            network: Network {
                province_country: provinces.country.clone(),
                ..Network::default()
            },
            weight,
            wedges,
            countries,
            provinces,
            pops: pop_data,
            enterprises: ent,
            steady: None,
            border_nu: None,
            market_elasticity: None,
        };
        inputs.network =
            inputs.network_from_graph(pack, &Gateways::read(dir)?, &TradeParams::default());
        inputs.set_base_prices(&TradeParams::default());
        inputs.set_coefficients();
        inputs.set_base_point(&EconParams::default().edu_eff);
        Ok(inputs)
    }

    /// Least-cost leg costs (S05) on the pack's transport graph as it stands, for this
    /// world's provinces and `gateways`: shortest paths from every province and every
    /// gateway, run in parallel, of which each keeps only the entries it needs (provinces
    /// of its country and its country's gateways from a province, gateways of other
    /// countries from a gateway), so no full tree outlives its source. Unreachable pairs get
    /// no leg. `route_taus` stays `None`: route choice is deferred past P2 (ADR 0017).
    ///
    /// Where `trade` gives theta classes a bulk freight rate (P154, P155, ADR 0247), the same
    /// runs on a copy of the graph repriced at the graph's parameters with that rate
    /// (`TransportParams::bulk`) give `Network::bulk`. The rate changes no edge's reachability,
    /// so the bulk legs have the base legs' endpoints in the same order.
    ///
    /// Where the graph's parameters choose paths by the iceberg cost (`PathChoice::Iceberg`,
    /// ADR 0279), each traded good also gets the same runs by `shortest_by` at its kappa, on the
    /// bulk graph for a bulk class's goods: `Network::good_legs`, one set per distinct (kappa,
    /// bulk rate) in the order of the goods.
    pub fn network_from_graph(
        &self,
        pack: &Pack,
        gateways: &Gateways,
        trade: &TradeParams,
    ) -> Network {
        let transport = pack.graph.params();
        let [domestic, access, crossings] = self.legs_on(&pack.graph, gateways, 0.0);
        let bulk_graph = trade.bulk_rate().map(|(classes, sea, port)| {
            let mut graph = pack.graph.clone();
            graph.reprice(&transport.bulk(sea, port));
            (classes, sea, port, graph)
        });
        let good_legs = (transport.path_choice == PathChoice::Iceberg).then(|| {
            let on_bulk = |g: usize| {
                bulk_graph
                    .as_ref()
                    .is_some_and(|b| b.0[usize::from(trade.theta_class[g])])
            };
            let traded = trade.goods.traded_goods();
            let kappa: Vec<f64> = traded
                .clone()
                .map(|g| kappa(self.weight[g], trade.time_cost[g]))
                .collect();
            let mut keys: Vec<(bool, u64)> = Vec::new();
            let set: Vec<u8> = traded
                .map(|g| {
                    assert!(
                        self.weight[g] > 0.0 && trade.time_cost[g] >= 0.0,
                        "good {g}: weight {} and time cost {} give no iceberg paths",
                        self.weight[g],
                        trade.time_cost[g]
                    );
                    let key = (on_bulk(g), kappa[g].to_bits());
                    let s = keys.iter().position(|&k| k == key).unwrap_or_else(|| {
                        keys.push(key);
                        keys.len() - 1
                    });
                    u8::try_from(s).expect("fewer than 255 leg sets")
                })
                .collect();
            let sets = keys
                .iter()
                .map(|&(bulk, k)| {
                    let graph = match &bulk_graph {
                        Some((.., g)) if bulk => g,
                        _ => &pack.graph,
                    };
                    let [domestic, access, crossings] =
                        self.legs_on(graph, gateways, f64::from_bits(k));
                    LegSet {
                        domestic,
                        access,
                        crossings,
                    }
                })
                .collect();
            GoodLegs { set, kappa, sets }
        });
        let bulk = bulk_graph.map(|(classes, sea, port, graph)| {
            let [domestic, access, crossings] = self.legs_on(&graph, gateways, 0.0);
            BulkLegs {
                classes,
                sea,
                port,
                domestic,
                access,
                crossings,
            }
        });
        Network {
            province_country: self.provinces.country.clone(),
            gateway_country: gateways.country.clone(),
            gateway_mass: gateways.mass.clone(),
            domestic,
            access,
            crossings,
            route_taus: None,
            bulk,
            good_legs,
        }
    }

    /// Domestic, access and crossing legs on `graph` ([`Self::network_from_graph`]), on the
    /// least `c + kappa h` paths (`shortest_by`; kappa 0 is the least-cost path).
    fn legs_on(&self, graph: &TransportGraph, gateways: &Gateways, kappa: f64) -> [Vec<Leg>; 3] {
        let nodes = &self.provinces.pack;
        assert!(
            !nodes.is_empty(),
            "a network from the graph needs a world loaded from the pack"
        );
        let country = &self.provinces.country;
        let nc = self.n_countries();
        let (np, ng) = (nodes.len(), gateways.len());
        let mut provinces_of = vec![Vec::new(); nc];
        for (i, &c) in country.iter().enumerate() {
            provinces_of[usize::from(c)].push(i);
        }
        let mut gateways_of = vec![Vec::new(); nc];
        for (a, &c) in gateways.country.iter().enumerate() {
            assert!(usize::from(c) < nc, "gateway {a}: country {c} of {nc}");
            gateways_of[usize::from(c)].push(a);
        }
        let leg = |t: &Tree, from: usize, to: usize, node: u32| {
            let cost = t.cost[node as usize];
            cost.is_finite().then(|| Leg {
                from: from as u32,
                to: to as u32,
                cost_per_t: cost,
                hours: t.hours[node as usize],
            })
        };
        let legs: Vec<(Vec<Leg>, Vec<Leg>)> = (0..np + ng)
            .into_par_iter()
            .map(|s| {
                if s < np {
                    let t = shortest_by(graph, nodes[s], kappa);
                    let c = usize::from(country[s]);
                    let domestic = provinces_of[c]
                        .iter()
                        .filter(|&&d| d != s)
                        .filter_map(|&d| leg(&t, s, d, nodes[d]))
                        .collect();
                    let access = gateways_of[c]
                        .iter()
                        .filter_map(|&a| leg(&t, s, a, gateways.node[a]))
                        .collect();
                    (domestic, access)
                } else {
                    let a = s - np;
                    let t = shortest_by(graph, gateways.node[a], kappa);
                    let crossings = (0..ng)
                        .filter(|&b| gateways.country[b] != gateways.country[a])
                        .filter_map(|b| leg(&t, a, b, gateways.node[b]))
                        .collect();
                    (crossings, Vec::new())
                }
            })
            .collect();
        let [mut domestic, mut access, mut crossings] = [Vec::new(), Vec::new(), Vec::new()];
        for (s, (first, second)) in legs.into_iter().enumerate() {
            if s < np {
                domestic.extend(first);
                access.extend(second);
            } else {
                crossings.extend(first);
            }
        }
        [domestic, access, crossings]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_geo::graph::{
        EdgeList, Mode, NodeKind, NodeList, PathChoice, TransportGraph, TransportParams,
    };
    use ceteris_geo::pack::Provinces;
    use ceteris_trade::Kernel;
    #[allow(unused_imports)]
    use ceteris_trade::goods::p2::{N as N_GOODS, TRADED as N_TRADED};

    /// Pack provinces 0, 1 and 3 in country 0, 2 in country 1; ports 4 (country 0) and 5
    /// (country 1) joined through sea node 6, the sea edge in chokepoint 1. The economy holds
    /// provinces 0, 1 and 2; province 3 lies on the road from 0 to 1.
    fn pack() -> Pack {
        pack_with_coastal_route(false)
    }

    /// [`pack`], and with `coastal` a port 7 in province 1, 1000 km by sea from port 4: a
    /// second path from province 0 to province 1, dearer than the road at P04's defaults.
    fn pack_with_coastal_route(coastal: bool) -> Pack {
        let mut kind = vec![
            NodeKind::Province,
            NodeKind::Province,
            NodeKind::Province,
            NodeKind::Province,
            NodeKind::Port,
            NodeKind::Port,
            NodeKind::Sea,
        ];
        let mut province = vec![0, 1, 2, 3, 0, 2, u32::MAX];
        let mut e = vec![
            (0, 3, Mode::Road, 50.0, 0),
            (3, 1, Mode::Road, 50.0, 0),
            (0, 4, Mode::PortAccess, 10.0, 0),
            (4, 6, Mode::Sea, 1000.0, 1),
            (6, 5, Mode::Sea, 1000.0, 0),
            (5, 2, Mode::PortAccess, 10.0, 0),
        ];
        if coastal {
            kind.push(NodeKind::Port);
            province.push(1);
            e.push((4, 7, Mode::Sea, 1000.0, 0));
            e.push((7, 1, Mode::PortAccess, 10.0, 0));
        }
        let nodes = NodeList {
            lon: vec![0.0; kind.len()],
            lat: vec![0.0; kind.len()],
            kind,
            province,
        };
        let edges = EdgeList {
            a: e.iter().map(|x| x.0).collect(),
            b: e.iter().map(|x| x.1).collect(),
            mode: e.iter().map(|x| x.2).collect(),
            length_km: e.iter().map(|x| x.3).collect(),
            tri_m: vec![0.0; e.len()],
            chokepoint: e.iter().map(|x| x.4).collect(),
        };
        let p = TransportParams::default();
        Pack {
            dir: std::path::PathBuf::new(),
            provinces: Provinces {
                id: (0..4).map(|i| i.to_string()).collect(),
                name: vec![String::new(); 4],
                country: ["A", "A", "B", "A"].map(String::from).to_vec(),
                lon: vec![0.0; 4],
                lat: vec![0.0; 4],
            },
            chokepoints: vec!["Strait".into()],
            graph: TransportGraph::new(4, nodes, edges, &p),
        }
    }

    fn inputs() -> EconInputs {
        EconInputs {
            countries: Countries {
                s_l: vec![0.5; 2],
                ..Countries::default()
            },
            provinces: ProvinceData {
                country: vec![0, 0, 1],
                pack: vec![0, 1, 2],
                ..ProvinceData::default()
            },
            ..EconInputs::default()
        }
    }

    fn gateways() -> Gateways {
        Gateways {
            node: vec![4, 5],
            country: vec![0, 1],
            kind: vec![0, 0],
            mass: vec![1.0, 1.0],
        }
    }

    #[test]
    fn country_with_enterprises_needs_population() {
        let codes = ["A".to_owned(), "B".to_owned()];
        let provinces = ProvinceData {
            country: vec![0, 1],
            ..ProvinceData::default()
        };
        let ent = EntData {
            province: vec![0, 1],
            ..EntData::default()
        };
        let pops = |population: [f64; 2]| PopData {
            province: vec![0, 1],
            population: population.to_vec(),
            ..PopData::default()
        };
        assert!(check_owners(&codes, &provinces, &pops([1.0, 2.0]), &ent).is_ok());
        for none in [[1.0, 0.0], [0.0, 2.0]] {
            let e = check_owners(&codes, &provinces, &pops(none), &ent).unwrap_err();
            let owner = if none[0] == 0.0 { "A" } else { "B" };
            assert!(e.to_string().contains(&format!("country {owner} ")), "{e}");
        }
    }

    #[test]
    fn legs_keep_own_country_provinces_and_gateways_and_foreign_gateways() {
        let (mut pack, inputs) = (pack(), inputs());
        let net = inputs.network_from_graph(&pack, &gateways(), &TradeParams::default());
        let ends = |legs: &[Leg]| legs.iter().map(|l| (l.from, l.to)).collect::<Vec<_>>();
        assert_eq!(ends(&net.domestic), [(0, 1), (1, 0)]);
        assert_eq!(ends(&net.access), [(0, 0), (1, 0), (2, 1)]);
        assert_eq!(ends(&net.crossings), [(0, 1), (1, 0)]);
        assert!(net.route_taus.is_none());
        assert_eq!(
            (net.province_country, net.gateway_country),
            (vec![0, 0, 1], vec![0, 1])
        );
        assert_eq!(net.gateway_mass, gateways().mass);
        let g = &pack.graph;
        let road = g.cost[0] + g.cost[1];
        assert_eq!(net.domestic[0].cost_per_t, road);
        assert_eq!(net.access[1].cost_per_t, road + g.cost[2]);
        assert_eq!(net.crossings[0].cost_per_t, g.cost[3] + g.cost[4]);
        assert_eq!(net.crossings[0].hours, g.hours[3] + g.hours[4]);

        // with the strait closed the ports cannot reach each other: no crossing legs
        pack.graph
            .set_chokepoint_closed(1, true, &TransportParams::default());
        let closed = inputs.network_from_graph(&pack, &gateways(), &TradeParams::default());
        assert!(closed.crossings.is_empty());
        assert_eq!(closed.domestic, net.domestic);
    }

    /// ADR 0247: a class with a bulk freight rate gets legs with the same endpoints, priced on
    /// the graph repriced at that rate; at multipliers 1 there are no bulk legs.
    #[test]
    fn bulk_legs_follow_the_bulk_paths() {
        let (pack, inputs) = (pack(), inputs());
        let off = TradeParams {
            bulk_sea: [1.0; 4],
            bulk_port: [1.0; 4],
            ..TradeParams::default()
        };
        assert!(
            inputs
                .network_from_graph(&pack, &gateways(), &off)
                .bulk
                .is_none()
        );
        let on = TradeParams {
            bulk_sea: [1.0, 1.0, 1.0, 0.5],
            bulk_port: [1.0, 1.0, 1.0, 0.25],
            ..TradeParams::default()
        };
        let base = inputs.network_from_graph(&pack, &gateways(), &off);
        let net = inputs.network_from_graph(&pack, &gateways(), &on);
        let b = net.bulk.as_ref().unwrap();
        assert_eq!(b.classes, [false, false, false, true]);
        let ends = |legs: &[Leg]| legs.iter().map(|l| (l.from, l.to)).collect::<Vec<_>>();
        assert_eq!(ends(&b.domestic), ends(&net.domestic));
        assert_eq!(ends(&b.access), ends(&net.access));
        assert_eq!(ends(&b.crossings), ends(&net.crossings));
        let (bp, g) = (TransportParams::default().bulk(0.5, 0.25), &pack.graph);
        let price =
            |e: usize| ceteris_geo::graph::price(g.mode[e], g.length_km[e], g.tri_m[e], &bp);
        // roads only: unchanged
        assert_eq!(b.domestic, net.domestic);
        // province 0 to its port: the road line plus a quarter of the listed-port fee
        assert_eq!(b.access[0].cost_per_t, price(2).0);
        assert_eq!(b.access[0].hours, net.access[0].hours);
        // port to port: half the sea cost
        assert_eq!(b.crossings[0].cost_per_t, price(3).0 + price(4).0);
        assert!((b.crossings[0].cost_per_t - 0.5 * net.crossings[0].cost_per_t).abs() < 1e-12);
        // the base legs do not move
        assert_eq!(
            (&net.domestic, &net.access, &net.crossings),
            (&base.domestic, &base.access, &base.crossings)
        );
    }

    /// ADR 0247: the bulk legs are least-cost paths on the repriced graph, so the rate can
    /// change the path. From province 0 to province 1 the road wins at P04's defaults and the
    /// coastal sea route wins at a tenth of the sea cost and port fees.
    #[test]
    fn bulk_rate_can_switch_the_path() {
        let (pack, inputs) = (pack_with_coastal_route(true), inputs());
        let on = TradeParams {
            bulk_sea: [1.0, 1.0, 1.0, 0.1],
            bulk_port: [1.0, 1.0, 1.0, 0.1],
            ..TradeParams::default()
        };
        let net = inputs.network_from_graph(&pack, &gateways(), &on);
        let b = net.bulk.as_ref().unwrap();
        let g = &pack.graph;
        let at = |p: &TransportParams, edges: &[usize]| {
            edges
                .iter()
                .map(|&e| ceteris_geo::graph::price(g.mode[e], g.length_km[e], g.tri_m[e], p))
                .fold((0.0, 0.0), |(c, h), (ec, eh)| (c + ec, h + eh))
        };
        let (road, coastal) = ([0, 1], [2, 6, 7]);
        let (base, bulk) = (
            TransportParams::default(),
            TransportParams::default().bulk(0.1, 0.1),
        );
        assert!(at(&base, &road).0 < at(&base, &coastal).0);
        assert!(at(&bulk, &coastal).0 < at(&bulk, &road).0);
        let leg = |l: &Leg| (l.from, l.to, l.cost_per_t, l.hours);
        assert_eq!(
            leg(&net.domestic[0]),
            (0, 1, at(&base, &road).0, at(&base, &road).1)
        );
        assert_eq!(
            leg(&b.domestic[0]),
            (0, 1, at(&bulk, &coastal).0, at(&bulk, &coastal).1)
        );
    }

    /// [`pack`] with airports 7 (province 0) and 8 (province 2), 2,000 km apart by air, as
    /// gateways beside the ports, the graph priced for iceberg paths (ADR 0279), and inputs whose
    /// good 6 weighs 2e-5 t per USD and goods 4 and 5 share a weight.
    fn air_world() -> (Pack, EconInputs, Gateways) {
        let mut pack = pack();
        let g = &pack.graph;
        let mut kind = g.kind.clone();
        kind.extend([NodeKind::Airport, NodeKind::Airport]);
        let mut province = g.node_province.clone();
        province.extend([0, 2]);
        let mut e: Vec<(u32, u32, Mode, f64, u8)> = (0..g.edges())
            .map(|i| {
                (
                    g.edge_a[i],
                    g.edge_b[i],
                    g.mode[i],
                    g.length_km[i],
                    g.chokepoint[i],
                )
            })
            .collect();
        e.extend([
            (0, 7, Mode::AirAccess, 10.0, 0),
            (7, 8, Mode::Air, 2000.0, 0),
            (8, 2, Mode::AirAccess, 10.0, 0),
        ]);
        let nodes = NodeList {
            lon: vec![0.0; kind.len()],
            lat: vec![0.0; kind.len()],
            kind,
            province,
        };
        let edges = EdgeList {
            a: e.iter().map(|x| x.0).collect(),
            b: e.iter().map(|x| x.1).collect(),
            mode: e.iter().map(|x| x.2).collect(),
            length_km: e.iter().map(|x| x.3).collect(),
            tri_m: vec![0.0; e.len()],
            chokepoint: e.iter().map(|x| x.4).collect(),
        };
        let iceberg = TransportParams {
            path_choice: PathChoice::Iceberg,
            ..TransportParams::default()
        };
        pack.graph = TransportGraph::new(4, nodes, edges, &iceberg);
        let mut inputs = inputs();
        inputs.weight = vec![0.5, 0.8, 0.9, 0.3, 0.002, 0.002, 2e-5, 0.0, 0.0, 0.0];
        let gateways = Gateways {
            node: vec![4, 5, 7, 8],
            country: vec![0, 1, 0, 1],
            kind: vec![0, 0, 1, 1],
            mass: vec![0.5; 4],
        };
        (pack, inputs, gateways)
    }

    /// The (cost, hours) of the path over `edges` of `graph`.
    fn path(graph: &TransportGraph, edges: &[usize]) -> (f64, f64) {
        edges.iter().fold((0.0, 0.0), |(c, h), &e| {
            (c + graph.cost[e], h + graph.hours[e])
        })
    }

    /// ADR 0279: under `PathChoice::Iceberg` each traded good's legs are its least-iceberg paths:
    /// the light good flies from airport to airport, the heavy goods sail,
    /// goods of equal kappa share a set, the bulk class takes its paths at the bulk rate, and the
    /// base and bulk lists are the freight ones.
    #[test]
    fn iceberg_legs_price_each_good_on_its_paths() {
        let (pack, inputs, gateways) = air_world();
        let trade = TradeParams::default();
        let net = inputs.network_from_graph(&pack, &gateways, &trade);
        let gl = net
            .good_legs
            .as_ref()
            .expect("iceberg paths give good legs");
        let mut freight_pack = air_world().0;
        freight_pack.graph.reprice(&TransportParams::default());
        let freight = inputs.network_from_graph(&freight_pack, &gateways, &trade);
        assert!(freight.good_legs.is_none());
        assert_eq!(
            (&net.domestic, &net.access, &net.crossings, &net.bulk),
            (
                &freight.domestic,
                &freight.access,
                &freight.crossings,
                &freight.bulk
            )
        );
        assert_eq!(gl.set[4], gl.set[5]);
        assert_eq!((gl.set.len(), gl.kappa.len()), (N_TRADED, N_TRADED));
        let distinct: std::collections::BTreeSet<u8> = gl.set.iter().copied().collect();
        assert_eq!(distinct.len(), gl.sets.len());
        assert_eq!(gl.sets.len(), 6);
        let g = &pack.graph;
        let (sea, air) = (path(g, &[3, 4]), path(g, &[7]));
        // airport 7 to airport 8, and port 4 to port 5
        let crossing = |s: &LegSet, a: u32, b: u32| {
            let l = s
                .crossings
                .iter()
                .find(|l| (l.from, l.to) == (a, b))
                .unwrap();
            (l.cost_per_t, l.hours)
        };
        let (light, heavy) = (
            &gl.sets[usize::from(gl.set[6])],
            &gl.sets[usize::from(gl.set[0])],
        );
        assert_eq!(crossing(light, 2, 3), air);
        // the heavy good leaves the airport by road for the port and sails
        let (c, h) = crossing(heavy, 2, 3);
        assert_eq!((c, h), path(g, &[6, 2, 3, 4, 5, 8]));
        assert!(c < air.0 && h > air.1);
        // port to port both sail: the sea leg is cheaper and quicker than the airport detour
        assert_eq!((crossing(light, 0, 1), crossing(heavy, 0, 1)), (sea, sea));
        assert_eq!(heavy.crossings, freight.crossings);
        let bulk = &gl.sets[usize::from(gl.set[1])];
        let bl = freight.bulk.as_ref().unwrap();
        assert!(bl.classes[usize::from(trade.theta_class[1])]);
        assert_eq!(bulk.crossings, bl.crossings);
        for x in [&net, &freight] {
            Kernel::build(x, &inputs.weight, &Wedges::none(2), &trade);
        }
    }

    /// ADR 0279: at zero time costs the iceberg legs are the freight legs bit for bit, the bulk
    /// class's on the bulk ones.
    #[test]
    fn iceberg_at_zero_time_cost_is_freight() {
        let (pack, inputs, gateways) = air_world();
        let trade = TradeParams {
            time_cost: vec![0.0; N_GOODS],
            ..TradeParams::default()
        };
        let net = inputs.network_from_graph(&pack, &gateways, &trade);
        let gl = net.good_legs.as_ref().unwrap();
        let bulk = net.bulk.as_ref().unwrap();
        for g in 0..gl.set.len() {
            let s = &gl.sets[usize::from(gl.set[g])];
            let (d, a, c) = if bulk.classes[usize::from(trade.theta_class[g])] {
                (&bulk.domestic, &bulk.access, &bulk.crossings)
            } else {
                (&net.domestic, &net.access, &net.crossings)
            };
            let bits = |l: &[Leg]| -> Vec<(u32, u32, u64, u64)> {
                l.iter()
                    .map(|l| (l.from, l.to, l.cost_per_t.to_bits(), l.hours.to_bits()))
                    .collect()
            };
            assert_eq!(bits(&s.domestic), bits(d), "good {g}");
            assert_eq!(bits(&s.access), bits(a), "good {g}");
            assert_eq!(bits(&s.crossings), bits(c), "good {g}");
        }
        assert_eq!(gl.sets.len(), 2);
    }

    #[test]
    fn iceberg_legs_are_thread_count_independent() {
        let (pack, inputs, gateways) = air_world();
        let trade = TradeParams::default();
        let build = |threads: usize| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| inputs.network_from_graph(&pack, &gateways, &trade))
        };
        let (one, many) = (build(1), build(8));
        assert!(one.good_legs.is_some());
        assert_eq!(one.good_legs, many.good_legs);
        assert_eq!(one.crossings, many.crossings);
    }

    /// Masses that do not sum to 1 per country, or are not in (0, 1], are a load error.
    #[test]
    fn gateway_masses_are_checked_at_load() {
        assert!(check_masses(&[0, 0, 1], &[0.5, 0.5, 1.0]).is_ok());
        let e = check_masses(&[0, 0, 1], &[0.5, 0.6, 1.0]).unwrap_err();
        assert!(e.0.contains("country 0 sum to"), "{e}");
        for bad in [0.0, f64::NAN, 1.5] {
            let e = check_masses(&[0, 1], &[bad, 1.0]).unwrap_err();
            assert!(e.0.contains("gateway 0 has mass"), "{e}");
        }
    }

    /// A pack built before the gateway masses is refused with an error naming the column.
    #[test]
    fn gateways_without_mass_name_the_column() {
        use arrow_array::{RecordBatch, UInt8Array, UInt32Array};
        use arrow_ipc::writer::FileWriter;
        use std::sync::Arc;

        let dir = std::env::temp_dir().join(format!("ceteris-gateways-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = RecordBatch::try_from_iter([
            ("node", Arc::new(UInt32Array::from(vec![4, 5])) as _),
            ("country", Arc::new(UInt32Array::from(vec![0, 1])) as _),
            ("kind", Arc::new(UInt8Array::from(vec![0, 0])) as _),
        ])
        .unwrap();
        let file = std::fs::File::create(dir.join("gateways.arrow")).unwrap();
        let mut w = FileWriter::try_new(file, &t.schema()).unwrap();
        w.write(&t).unwrap();
        w.finish().unwrap();
        let e = Gateways::read(&dir).unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(e.0.contains("has no mass column"), "{e}");
    }
}
