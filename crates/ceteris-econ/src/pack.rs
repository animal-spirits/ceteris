//! A P2 world from the 2000 pack (pack format 3; `specs/econ/SPEC.md`, Pack additions).
//!
//! The economy's provinces are the pack provinces with value added and a labour force:
//! a province below the working-age floor of the pipeline has no value added, and an
//! uninhabited one has no pops, so neither can carry an enterprise or a wage. They stay in the
//! transport graph, which paths cross, and `ProvinceData::pack` maps each economy province
//! back to its pack province.

use std::path::Path;

use ceteris_geo::pack::{Pack, PackError, f64s, read_table, strings, u8s, u32s};
use ceteris_geo::paths::{Tree, shortest};
use ceteris_trade::{Leg, N_GOODS, Network, Wedges};
use rayon::prelude::*;

use crate::inputs::{Countries, EconInputs, EntData, N_EDU, PopData, ProvinceData};
use crate::params::EconParams;

/// Million USD per year, the pack's VA unit, to numeraire (USD) per month.
const VA_TO_MONTHLY: f64 = 1e6 / 12.0;

fn err(msg: impl Into<String>) -> PackError {
    PackError(msg.into())
}

/// Gateways of the pack (`gateways.arrow`): graph node and country of each, and its kind
/// (0 port, 1 airport, 2 land crossing).
#[derive(Clone, Debug, PartialEq)]
pub struct Gateways {
    pub node: Vec<u32>,
    pub country: Vec<u16>,
    pub kind: Vec<u8>,
}

impl Gateways {
    pub fn read(dir: &Path) -> Result<Self, PackError> {
        let t = read_table(&dir.join("gateways.arrow"))?;
        let country = u32s(&t, "country")?
            .into_iter()
            .map(|c| u16::try_from(c).map_err(|_| err(format!("gateway country {c}"))))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            node: u32s(&t, "node")?,
            country,
            kind: u8s(&t, "kind")?,
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

/// Columns `{prefix}0` to `{prefix}9`, as one row of goods per table row.
fn per_good(
    batch: &arrow_array::RecordBatch,
    prefix: &str,
) -> Result<Vec<[f64; N_GOODS]>, PackError> {
    let cols = (0..N_GOODS)
        .map(|g| f64s(batch, &format!("{prefix}{g}")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((0..batch.num_rows())
        .map(|r| std::array::from_fn(|g| cols[g][r]))
        .collect())
}

impl EconInputs {
    /// Loads the P2 world from the econ tables of a pack in `dir` and its transport graph.
    /// Leg costs are least-cost paths ([`EconInputs::network_from_graph`]); productivity A
    /// comes from `econ_calib.arrow` (columns province, the pack province, good and a) when
    /// present, else 1. The base point is filled with the default education efficiencies
    /// (`EconParams::default().edu_eff`), coefficients with one vintage.
    pub fn from_pack(pack: &Pack, dir: &Path) -> Result<EconInputs, PackError> {
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
        let io_table = table("econ_io.arrow")?;
        let a = f64s(&io_table, "a")?;
        expect_rows("econ_io", a.len(), nc * N_GOODS * N_GOODS)?;
        let countries = Countries {
            omega: per_good(&c, "fd_")?,
            io: a
                .chunks_exact(N_GOODS * N_GOODS)
                .map(|x| x.try_into().expect("one country of coefficients"))
                .collect(),
            s_l: f64s(&c, "labour_share")?,
            unemployment: f64s(&c, "unemployment")?,
            gini: f64s(&c, "gini")?,
        };

        let goods = table("econ_goods.arrow")?;
        let weight = f64s(&goods, "weight")?;
        expect_rows("econ_goods", weight.len(), N_GOODS)?;
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
        let va = per_good(&p, "va_")?;
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
        let mut provinces = ProvinceData::default();
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
        let calib = calibration(dir)?;
        let mut ent = EntData::default();
        for (i, &node) in provinces.pack.iter().enumerate() {
            for (g, &v) in va[node as usize].iter().enumerate() {
                if v > 0.0 {
                    let va0 = v * VA_TO_MONTHLY;
                    ent.province.push(i as u32);
                    ent.good.push(g as u8);
                    ent.a
                        .push(calib.as_ref().map_or(Ok(1.0), |c| c.a(node, g))?);
                    ent.va0.push(va0);
                    ent.va_target.push(va0);
                }
            }
        }

        let mut inputs = EconInputs {
            network: Network {
                province_country: provinces.country.clone(),
                ..Network::default()
            },
            weight: weight.try_into().expect("ten goods"),
            wedges,
            countries,
            provinces,
            pops: pop_data,
            enterprises: ent,
        };
        inputs.set_coefficients();
        inputs.set_base_point(&EconParams::default().edu_eff);
        inputs.network = inputs.network_from_graph(pack, &Gateways::read(dir)?);
        Ok(inputs)
    }

    /// Least-cost leg costs (S05) on the pack's transport graph as it stands, for this
    /// world's provinces and `gateways`: shortest paths from every province and every
    /// gateway, run in parallel, of which each keeps only the entries it needs (provinces
    /// of its country and its country's gateways from a province, gateways of other
    /// countries from a gateway), so no full tree outlives its source. Unreachable pairs get
    /// no leg. `route_taus` stays `None`: route choice is deferred past P2 (ADR 0017).
    pub fn network_from_graph(&self, pack: &Pack, gateways: &Gateways) -> Network {
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
                    let t = shortest(&pack.graph, nodes[s]);
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
                    let t = shortest(&pack.graph, gateways.node[a]);
                    let crossings = (0..ng)
                        .filter(|&b| gateways.country[b] != gateways.country[a])
                        .filter_map(|b| leg(&t, a, b, gateways.node[b]))
                        .collect();
                    (crossings, Vec::new())
                }
            })
            .collect();
        let mut net = Network {
            province_country: country.clone(),
            gateway_country: gateways.country.clone(),
            ..Network::default()
        };
        for (s, (first, second)) in legs.into_iter().enumerate() {
            if s < np {
                net.domestic.extend(first);
                net.access.extend(second);
            } else {
                net.crossings.extend(first);
            }
        }
        net
    }
}

/// Productivity A by (pack province, good) from `econ_calib.arrow`.
struct Calibration {
    key: Vec<(u32, u8)>,
    a: Vec<f64>,
}

impl Calibration {
    fn a(&self, province: u32, good: usize) -> Result<f64, PackError> {
        self.key
            .binary_search(&(province, good as u8))
            .map(|r| self.a[r])
            .map_err(|_| {
                err(format!(
                    "econ_calib has no A for province {province} good {good}"
                ))
            })
    }
}

fn calibration(dir: &Path) -> Result<Option<Calibration>, PackError> {
    let path = dir.join("econ_calib.arrow");
    if !path.exists() {
        return Ok(None);
    }
    let t = read_table(&path)?;
    let (province, good, a) = (u32s(&t, "province")?, u8s(&t, "good")?, f64s(&t, "a")?);
    let mut rows: Vec<((u32, u8), f64)> = province.into_iter().zip(good).zip(a).collect();
    rows.sort_by_key(|r| r.0);
    let (key, a) = rows.into_iter().unzip();
    Ok(Some(Calibration { key, a }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_geo::graph::{EdgeList, Mode, NodeKind, NodeList, TransportGraph, TransportParams};
    use ceteris_geo::pack::Provinces;

    /// Pack provinces 0, 1 and 3 in country 0, 2 in country 1; ports 4 (country 0) and 5
    /// (country 1) joined through sea node 6, the sea edge in chokepoint 1. The economy holds
    /// provinces 0, 1 and 2; province 3 lies on the road from 0 to 1.
    fn pack() -> Pack {
        let kind = [
            NodeKind::Province,
            NodeKind::Province,
            NodeKind::Province,
            NodeKind::Province,
            NodeKind::Port,
            NodeKind::Port,
            NodeKind::Sea,
        ];
        let nodes = NodeList {
            kind: kind.to_vec(),
            province: vec![0, 1, 2, 3, 0, 2, u32::MAX],
            lon: vec![0.0; 7],
            lat: vec![0.0; 7],
        };
        let e = [
            (0, 3, Mode::Road, 50.0, 0),
            (3, 1, Mode::Road, 50.0, 0),
            (0, 4, Mode::PortAccess, 10.0, 0),
            (4, 6, Mode::Sea, 1000.0, 1),
            (6, 5, Mode::Sea, 1000.0, 0),
            (5, 2, Mode::PortAccess, 10.0, 0),
        ];
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
        }
    }

    #[test]
    fn legs_keep_own_country_provinces_and_gateways_and_foreign_gateways() {
        let (mut pack, inputs) = (pack(), inputs());
        let net = inputs.network_from_graph(&pack, &gateways());
        let ends = |legs: &[Leg]| legs.iter().map(|l| (l.from, l.to)).collect::<Vec<_>>();
        assert_eq!(ends(&net.domestic), [(0, 1), (1, 0)]);
        assert_eq!(ends(&net.access), [(0, 0), (1, 0), (2, 1)]);
        assert_eq!(ends(&net.crossings), [(0, 1), (1, 0)]);
        assert!(net.route_taus.is_none());
        assert_eq!(
            (net.province_country, net.gateway_country),
            (vec![0, 0, 1], vec![0, 1])
        );
        let g = &pack.graph;
        let road = g.cost[0] + g.cost[1];
        assert_eq!(net.domestic[0].cost_per_t, road);
        assert_eq!(net.access[1].cost_per_t, road + g.cost[2]);
        assert_eq!(net.crossings[0].cost_per_t, g.cost[3] + g.cost[4]);
        assert_eq!(net.crossings[0].hours, g.hours[3] + g.hours[4]);

        // with the strait closed the ports cannot reach each other: no crossing legs
        pack.graph
            .set_chokepoint_closed(1, true, &TransportParams::default());
        let closed = inputs.network_from_graph(&pack, &gateways());
        assert!(closed.crossings.is_empty());
        assert_eq!(closed.domestic, net.domestic);
    }
}
