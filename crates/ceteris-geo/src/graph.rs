//! Multimodal transport graph (Section 5.3) in compressed-row form, priced per tonne
//! from mode parameters (register P04). See `specs/geo/SPEC.md`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum NodeKind {
    Province = 0,
    Rail = 1,
    Port = 2,
    Airport = 3,
    Sea = 4,
}

impl NodeKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        [
            Self::Province,
            Self::Rail,
            Self::Port,
            Self::Airport,
            Self::Sea,
        ]
        .get(v as usize)
        .copied()
    }
}

/// Edge modes. Access modes join a province to its rail network, ports and airports;
/// their length is travelled by road.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Mode {
    Road = 0,
    Rail = 1,
    Sea = 2,
    Air = 3,
    RailAccess = 4,
    PortAccess = 5,
    MinorPortAccess = 6,
    AirAccess = 7,
    Canal = 8,
}

impl Mode {
    pub const ALL: [Mode; 9] = [
        Mode::Road,
        Mode::Rail,
        Mode::Sea,
        Mode::Air,
        Mode::RailAccess,
        Mode::PortAccess,
        Mode::MinorPortAccess,
        Mode::AirAccess,
        Mode::Canal,
    ];

    pub fn from_u8(v: u8) -> Option<Self> {
        Self::ALL.get(v as usize).copied()
    }
}

/// How the trade kernel's legs choose their paths (ADR 0279).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PathChoice {
    /// The least freight cost per tonne (S05 as built).
    #[default]
    Freight,
    /// Per traded good, the least iceberg cost `w_g c + lambda_g h / 24`.
    Iceberg,
}

/// Per-tonne mode costs in 2000 US dollars and effective speeds. Priors for P04: the
/// ordering sea < rail < road < air is imposed (Section 5.3); magnitudes are calibrated in P9.
#[derive(Clone, Debug, PartialEq)]
pub struct TransportParams {
    /// P04a-d: cost per tonne-km by road, rail, sea and air.
    pub per_tkm: [f64; 4],
    /// P04e-h: path length over great-circle length (circuity) by road, rail, sea and air.
    pub circuity: [f64; 4],
    /// P04i-l: effective speed in km/h by road, rail, sea and air, including waits.
    pub speed_kmh: [f64; 4],
    /// P04m: handling fee per tonne at a rail terminal, listed port, minor port, airport.
    pub fee: [f64; 4],
    /// P04n: handling time in hours, same order as `fee`.
    pub fee_hours: [f64; 4],
    /// P04o: canal transit fee per tonne.
    pub canal_fee: f64,
    /// P04p: road and rail cost increase per 100 m of terrain ruggedness index.
    pub terrain_slope: f64,
    /// P04q: cap on the terrain multiplier.
    pub terrain_cap: f64,
    /// How kernel legs choose their paths (ADR 0279); edge prices do not depend on it.
    pub path_choice: PathChoice,
}

impl Default for TransportParams {
    fn default() -> Self {
        Self {
            per_tkm: [0.10, 0.04, 0.008, 0.80],
            circuity: [1.3, 1.3, 1.0, 1.05],
            speed_kmh: [40.0, 25.0, 25.0, 500.0],
            fee: [5.0, 8.0, 20.0, 60.0],
            fee_hours: [24.0, 48.0, 72.0, 12.0],
            canal_fee: 5.0,
            terrain_slope: 0.5,
            terrain_cap: 3.0,
            path_choice: PathChoice::Freight,
        }
    }
}

impl TransportParams {
    /// These parameters at a bulk freight rate (P154, P155, ADR 0247): the sea cost per
    /// tonne-km (sea and canal edges) times `sea`, the listed and minor port fees times `port`.
    pub fn bulk(&self, sea: f64, port: f64) -> Self {
        let mut p = self.clone();
        p.per_tkm[SEA] *= sea;
        p.fee[1] *= port;
        p.fee[2] *= port;
        p
    }
}

const ROAD: usize = 0;
const RAIL: usize = 1;
const SEA: usize = 2;
const AIR: usize = 3;

/// Multiplier on road and rail costs from mean terrain ruggedness (TRI, metres) along an edge.
pub fn terrain_factor(tri_m: f64, p: &TransportParams) -> f64 {
    // TODO(owner): the default is linear in TRI and capped; see specs/geo/SPEC.md, open questions.
    (1.0 + p.terrain_slope * tri_m / 100.0).min(p.terrain_cap)
}

/// Cost per tonne and hours to traverse one edge.
pub fn price(mode: Mode, length_km: f64, tri_m: f64, p: &TransportParams) -> (f64, f64) {
    let line = |m: usize, factor: f64| {
        let km = length_km * p.circuity[m];
        (p.per_tkm[m] * km * factor, km / p.speed_kmh[m])
    };
    let access = |fee: usize| {
        let (c, h) = line(ROAD, 1.0);
        (c + p.fee[fee], h + p.fee_hours[fee])
    };
    match mode {
        Mode::Road => line(ROAD, terrain_factor(tri_m, p)),
        Mode::Rail => line(RAIL, terrain_factor(tri_m, p)),
        Mode::Sea => line(SEA, 1.0),
        Mode::Canal => {
            let (c, h) = line(SEA, 1.0);
            (c + p.canal_fee, h)
        }
        Mode::Air => line(AIR, 1.0),
        Mode::RailAccess => access(0),
        Mode::PortAccess => access(1),
        Mode::MinorPortAccess => access(2),
        Mode::AirAccess => access(3),
    }
}

/// Undirected edges stored once; adjacency lists both directions in compressed-row form.
/// Node `i < provinces` is province `i`.
#[derive(Clone, Debug)]
pub struct TransportGraph {
    pub provinces: usize,
    pub kind: Vec<NodeKind>,
    pub node_province: Vec<u32>,
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
    pub edge_a: Vec<u32>,
    pub edge_b: Vec<u32>,
    pub mode: Vec<Mode>,
    pub length_km: Vec<f64>,
    pub tri_m: Vec<f64>,
    pub chokepoint: Vec<u8>,
    pub cost: Vec<f64>,
    pub hours: Vec<f64>,
    offsets: Vec<u32>,
    arc_to: Vec<u32>,
    arc_edge: Vec<u32>,
    closed: Vec<u8>,
    version: u64,
    params: TransportParams,
}

pub struct EdgeList {
    pub a: Vec<u32>,
    pub b: Vec<u32>,
    pub mode: Vec<Mode>,
    pub length_km: Vec<f64>,
    pub tri_m: Vec<f64>,
    pub chokepoint: Vec<u8>,
}

pub struct NodeList {
    pub kind: Vec<NodeKind>,
    pub province: Vec<u32>,
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
}

impl TransportGraph {
    pub fn new(
        provinces: usize,
        nodes: NodeList,
        edges: EdgeList,
        params: &TransportParams,
    ) -> Self {
        let n = nodes.kind.len();
        assert!(
            provinces <= n
                && nodes.kind[..provinces]
                    .iter()
                    .all(|k| *k == NodeKind::Province)
        );
        let mut degree = vec![0u32; n + 1];
        for (&a, &b) in edges.a.iter().zip(&edges.b) {
            assert!(
                (a as usize) < n && (b as usize) < n,
                "edge endpoint out of range"
            );
            degree[a as usize + 1] += 1;
            degree[b as usize + 1] += 1;
        }
        let mut offsets = degree;
        for i in 1..=n {
            offsets[i] += offsets[i - 1];
        }
        let mut fill = offsets.clone();
        let m = edges.a.len();
        let mut arc_to = vec![0u32; 2 * m];
        let mut arc_edge = vec![0u32; 2 * m];
        for e in 0..m {
            for (from, to) in [(edges.a[e], edges.b[e]), (edges.b[e], edges.a[e])] {
                let slot = fill[from as usize] as usize;
                arc_to[slot] = to;
                arc_edge[slot] = e as u32;
                fill[from as usize] += 1;
            }
        }
        let mut g = Self {
            provinces,
            kind: nodes.kind,
            node_province: nodes.province,
            lon: nodes.lon,
            lat: nodes.lat,
            edge_a: edges.a,
            edge_b: edges.b,
            mode: edges.mode,
            length_km: edges.length_km,
            tri_m: edges.tri_m,
            chokepoint: edges.chokepoint,
            cost: vec![0.0; m],
            hours: vec![0.0; m],
            offsets,
            arc_to,
            arc_edge,
            closed: Vec::new(),
            version: 0,
            params: params.clone(),
        };
        g.reprice(params);
        g
    }

    pub fn nodes(&self) -> usize {
        self.kind.len()
    }

    pub fn edges(&self) -> usize {
        self.mode.len()
    }

    /// Arcs leaving `node` as (target, edge).
    pub fn arcs(&self, node: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        let (s, e) = (
            self.offsets[node as usize] as usize,
            self.offsets[node as usize + 1] as usize,
        );
        self.arc_to[s..e]
            .iter()
            .copied()
            .zip(self.arc_edge[s..e].iter().copied())
    }

    /// Changes whenever any edge cost changes; path caches compare against it.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The parameters the edges were last priced at.
    pub fn params(&self) -> &TransportParams {
        &self.params
    }

    pub fn reprice(&mut self, params: &TransportParams) {
        for e in 0..self.edges() {
            let (c, h) = if self.closed.contains(&self.chokepoint[e]) {
                (f64::INFINITY, f64::INFINITY)
            } else {
                price(self.mode[e], self.length_km[e], self.tri_m[e], params)
            };
            self.cost[e] = c;
            self.hours[e] = h;
        }
        self.params = params.clone();
        self.version += 1;
    }

    /// Close (a shock) or reopen a chokepoint; ids start at 1 in pack order.
    pub fn set_chokepoint_closed(&mut self, id: u8, closed: bool, params: &TransportParams) {
        assert!(id > 0, "chokepoint 0 means none");
        self.closed.retain(|&c| c != id);
        if closed {
            self.closed.push(id);
        }
        self.reprice(params);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_costs_keep_the_imposed_ordering() {
        let p = TransportParams::default();
        let [road, rail, sea, air] = p.per_tkm;
        assert!(sea < rail && rail < road && road < air);
    }

    #[test]
    fn terrain_raises_land_costs_up_to_the_cap() {
        let p = TransportParams::default();
        assert_eq!(terrain_factor(0.0, &p), 1.0);
        assert!(terrain_factor(100.0, &p) > 1.0);
        assert_eq!(terrain_factor(1e6, &p), p.terrain_cap);
        let flat = price(Mode::Road, 100.0, 0.0, &p).0;
        assert!(price(Mode::Road, 100.0, 300.0, &p).0 > flat);
        assert_eq!(
            price(Mode::Sea, 100.0, 300.0, &p),
            price(Mode::Sea, 100.0, 0.0, &p)
        );
    }

    #[test]
    fn bulk_rate_reprices_sea_and_port_fees() {
        let (p, b) = (
            TransportParams::default(),
            TransportParams::default().bulk(0.25, 0.5),
        );
        assert_eq!(
            price(Mode::Sea, 100.0, 0.0, &b).0,
            0.25 * price(Mode::Sea, 100.0, 0.0, &p).0
        );
        // the canal edge is the sea line plus the unchanged canal fee
        let canal_line = |q: &TransportParams| price(Mode::Canal, 100.0, 0.0, q).0 - q.canal_fee;
        assert!((canal_line(&b) - 0.25 * canal_line(&p)).abs() < 1e-12);
        assert_eq!(b.canal_fee, p.canal_fee);
        for (mode, fee) in [(Mode::PortAccess, 1), (Mode::MinorPortAccess, 2)] {
            assert_eq!(price(mode, 0.0, 0.0, &b).0, 0.5 * p.fee[fee]);
            assert_eq!(price(mode, 0.0, 0.0, &b).1, price(mode, 0.0, 0.0, &p).1);
            assert_eq!(
                price(mode, 10.0, 0.0, &b).0,
                price(Mode::Road, 10.0, 0.0, &p).0 + 0.5 * p.fee[fee]
            );
        }
        for mode in [
            Mode::Road,
            Mode::Rail,
            Mode::Air,
            Mode::RailAccess,
            Mode::AirAccess,
        ] {
            assert_eq!(price(mode, 100.0, 50.0, &b), price(mode, 100.0, 50.0, &p));
        }
        assert_eq!(
            price(Mode::Sea, 100.0, 0.0, &b).1,
            price(Mode::Sea, 100.0, 0.0, &p).1
        );
        assert_eq!(TransportParams::default().bulk(1.0, 1.0), p);
    }

    #[test]
    fn access_edges_charge_the_handling_fee_once() {
        let p = TransportParams::default();
        assert_eq!(
            price(Mode::PortAccess, 0.0, 0.0, &p),
            (p.fee[1], p.fee_hours[1])
        );
        assert_eq!(price(Mode::Canal, 0.0, 0.0, &p).0, p.canal_fee);
    }
}
