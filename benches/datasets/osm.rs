//! osm: OpenStreetMap Austria (Geofabrik 2026-09-23, ODbL): 87.8M nodes /
//! 9.4M ways / 181k relations from the 811 MB PBF into three entity tables.
//! Sparse tag maps (`HashMap<SmolStr, String>`), spatially correlated
//! `Vec<i64>` refs, unit enum, f64 coords with `float_round = 7` (~1 cm;
//! the nanodegree tail is noise), bbox filter on the bulk centroid
//! (cross-border).

use osm_pbf::FileBlock;
use osm_pbf::decode::decode_elements;
use osm_pbf::parse::parse_osm_pbf;
use osm_types::Element;
use pco_pack::PcoPack;
use smol_str::SmolStr;
use std::collections::HashMap;

use super::common::{self, CodecResults, Ds};

pub const DS: Ds = Ds {
    name: "austria-260923.osm.pbf",
    url: "https://download.geofabrik.de/europe/austria-260923.osm.pbf",
    size: 810_746_083,
    md5: Some("481bb65e61d5d5d22bb8a065b22506e2"),
};

// ---------------------------------------------------------------------------
// PcoPack rows
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: i64,
    pub lat: f64, // degrees, from PBF nanodegrees
    pub lon: f64,
    pub tags: HashMap<SmolStr, String>, // keys: small closed vocabulary -> SmolStr;
                                        // values: free text (name, desc) -> String
}

#[derive(Clone, Debug, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Way {
    pub id: i64,
    pub refs: Vec<i64>, // node ids, spatially correlated in file order
    pub tags: HashMap<SmolStr, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
pub enum MemberType {
    #[default]
    Node,
    Way,
    Relation,
}

#[derive(Clone, Debug, PartialEq, Default, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Member {
    pub rtype: MemberType,
    pub ref_id: i64,           // the PBF field is `ref`: a Rust keyword
    pub role: Option<SmolStr>, // short closed vocabulary ("outer", "left", ...)
}

#[derive(Clone, Debug, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
pub struct Relation {
    pub id: i64,
    pub members: Vec<Member>,
    pub tags: HashMap<SmolStr, String>,
}

/// Lossy variant: `float_round = 7` drops sub-centimeter noise (OSM coords
/// are meaningful at ~1 cm, ~1e-7 deg; PBF nanodegrees are finer).
#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(float_round = 7)]
pub struct NodeF {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
    pub tags: HashMap<SmolStr, String>,
}

impl From<&Node> for NodeF {
    fn from(r: &Node) -> Self {
        NodeF { id: r.id, lat: r.lat, lon: r.lon, tags: r.tags.clone() }
    }
}

// ---------------------------------------------------------------------------
// Bench (three entity tables)
// ---------------------------------------------------------------------------

pub fn bench() {
    let (nodes, ways, rels, stats) = load();
    let (n_nodes, n_ways, n_rels) = (nodes.len(), ways.len(), rels.len());

    println!("### OpenStreetMap\n");

    // ---- Node: the lossy + two-filter table ----
    {
        // Two bbox windows centered on the in-country bulk's centroid; the
        // bulk's min-lat min-lon corner is not a corner of the country.
        let (lat_c, lon_c) =
            ((stats.lat_bulk_min + stats.lat_bulk_max) / 2.0, (stats.lon_bulk_min + stats.lon_bulk_max) / 2.0);
        // (half-lat, half-lon, shape label)
        let windows: [(f64, f64, &str); 2] = [(0.05, 0.1, "0.1deg x 0.2deg"), (0.25, 0.5, "0.5deg x 1.0deg")];
        let boxes: Vec<(f64, f64, f64, f64)> =
            windows.iter().map(|(dlat, dlon, _)| (lat_c - dlat, lat_c + dlat, lon_c - dlon, lon_c + dlon)).collect();
        let in_box = |r: &Node, k: &(f64, f64, f64, f64)| r.lat >= k.0 && r.lat <= k.1 && r.lon >= k.2 && r.lon <= k.3;
        let exp: Vec<usize> = boxes.iter().map(|k| nodes.iter().filter(|r| in_box(r, k)).count()).collect();
        // The lossy variant stores lat/lon rounded to 7 digits, so the rows it
        // reports for the same query can differ at the boundary.
        let exp_f: Vec<usize> = boxes
            .iter()
            .map(|k| {
                nodes
                    .iter()
                    .filter(|r| {
                        let lat = common::round_f64(r.lat, 7);
                        let lon = common::round_f64(r.lon, 7);
                        lat >= k.0 && lat <= k.1 && lon >= k.2 && lon <= k.3
                    })
                    .count()
            })
            .collect();
        let labels: Vec<String> = exp.iter().map(|&e| common::filter_label(e, n_nodes)).collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

        println!("#### Node ({n_nodes} rows)\n");

        let queries: Vec<serde_json::Value> = boxes
            .iter()
            .map(|(a, b, c, d)| {
                serde_json::json!({
                    "lat": { "start": a, "end": b },
                    "lon": { "start": c, "end": d },
                })
            })
            .collect();
        let pco_filters: Vec<(&serde_json::Value, usize)> =
            queries.iter().zip(exp.iter()).map(|(q, e)| (q, *e)).collect();
        let pcof_filters: Vec<(&serde_json::Value, usize)> =
            queries.iter().zip(exp_f.iter()).map(|(q, e)| (q, *e)).collect();

        let pco = common::bench_pco_pack(nodes.to_vec(), &pco_filters, |back| {
            // No `timestamp` field -> PcoPack keeps file order -> positional roundtrip check.
            assert_eq!(back, &nodes[..], "osm node roundtrip mismatch");
        });

        let nodesf: Vec<NodeF> = nodes.iter().map(NodeF::from).collect();
        let pcof = common::bench_pco_pack(nodesf, &pcof_filters, |back| {
            for (b, r) in back.iter().zip(nodes.iter()) {
                assert_eq!(b.id, r.id);
                assert_eq!(b.tags, r.tags);
                assert_eq!(b.lat, common::round_f64(r.lat, 7), "node lat (float_round = 7)");
                assert_eq!(b.lon, common::round_f64(r.lon, 7), "node lon (float_round = 7)");
            }
        });

        let box_filters: Vec<((f64, f64, f64, f64), usize)> =
            boxes.iter().zip(exp.iter()).map(|(b, e)| (*b, *e)).collect();

        let col = common::bench_columnar(
            n_nodes,
            || nodes.iter().map(NodeCol::from).collect(),
            &box_filters,
            |decoded: &columnar::BorrowedOf<NodeCol>, k: &(f64, f64, f64, f64)| {
                let count = columnar::Len::len(decoded);
                (0..count)
                    .filter(|&i| {
                        let r = columnar::Index::get(decoded, i);
                        k.0 <= *r.lat && *r.lat <= k.1 && k.2 <= *r.lon && *r.lon <= k.3
                    })
                    .count()
            },
        );

        let sc = common::bench_serde_columnar(
            n_nodes,
            || osm_sc::NodeScStore { data: nodes.iter().map(osm_sc::NodeScRow::from).collect() },
            &box_filters,
            |s: &osm_sc::NodeScStore, k: &(f64, f64, f64, f64)| {
                s.data.iter().filter(|r| k.0 <= r.lat && r.lat <= k.1 && k.2 <= r.lon && r.lon <= k.3).count()
            },
        );

        let mp = common::bench_msgpack(&nodes, &box_filters, in_box);

        let nodes_vx: Vec<NodeVx> = nodes.iter().map(NodeVx::from).collect();
        let vx = common::bench_vortex_ds(&nodes_vx, &box_filters, |k: &(f64, f64, f64, f64)| {
            vortex::array::expr::and(
                vortex::array::expr::and(
                    vortex::array::expr::gt_eq(vortex::array::expr::col("lat"), vortex::array::expr::lit(k.0)),
                    vortex::array::expr::lt_eq(vortex::array::expr::col("lat"), vortex::array::expr::lit(k.1)),
                ),
                vortex::array::expr::and(
                    vortex::array::expr::gt_eq(vortex::array::expr::col("lon"), vortex::array::expr::lit(k.2)),
                    vortex::array::expr::lt_eq(vortex::array::expr::col("lon"), vortex::array::expr::lit(k.3)),
                ),
            )
        });

        CodecResults {
            pco_pack: pco,
            pco_pack_lossy: Some(pcof),
            columnar: col,
            serde_columnar: sc,
            msgpack: mp,
            vortex: vx,
        }
        .print(&label_refs);
    }
    drop(nodes);

    // ---- Way: no lossy row, no filter row (no natural scalar key) ----
    {
        println!("#### Way ({n_ways} rows)\n");

        let pco = common::bench_pco_pack(ways.to_vec(), &[], |back| {
            assert_eq!(back, &ways[..], "osm way roundtrip mismatch");
        });
        let col = common::bench_columnar(
            n_ways,
            || ways.iter().map(WayCol::from).collect(),
            &[] as &[((), usize)],
            |_: &columnar::BorrowedOf<WayCol>, _: &()| 0,
        );
        let sc = common::bench_serde_columnar(
            n_ways,
            || osm_sc::WayScStore { data: ways.iter().map(osm_sc::WayScRow::from).collect() },
            &[] as &[((), usize)],
            |_: &osm_sc::WayScStore, _: &()| 0,
        );
        let mp = common::bench_msgpack(&ways, &[] as &[((), usize)], |_: &Way, _: &()| false);
        let ways_vx: Vec<WayVx> = ways.iter().map(WayVx::from).collect();
        let vx = common::bench_vortex_ds(&ways_vx, &[] as &[((), usize)], |_: &()| unreachable!());

        CodecResults {
            pco_pack: pco,
            pco_pack_lossy: None,
            columnar: col,
            serde_columnar: sc,
            msgpack: mp,
            vortex: vx,
        }
        .print(&[]);
    }
    drop(ways);

    // ---- Relation ----
    {
        println!("#### Relation ({n_rels} rows)\n");

        let pco = common::bench_pco_pack(rels.to_vec(), &[], |back| {
            assert_eq!(back, &rels[..], "osm relation roundtrip mismatch");
        });
        let col = common::bench_columnar(
            n_rels,
            || rels.iter().map(RelationCol::from).collect(),
            &[] as &[((), usize)],
            |_: &columnar::BorrowedOf<RelationCol>, _: &()| 0,
        );
        let sc = common::bench_serde_columnar(
            n_rels,
            || osm_sc::RelationScStore { data: rels.iter().map(osm_sc::RelationScRow::from).collect() },
            &[] as &[((), usize)],
            |_: &osm_sc::RelationScStore, _: &()| 0,
        );
        let mp = common::bench_msgpack(&rels, &[] as &[((), usize)], |_: &Relation, _: &()| false);
        let rels_vx: Vec<RelationVx> = rels.iter().map(RelationVx::from).collect();
        let vx = common::bench_vortex_ds(&rels_vx, &[] as &[((), usize)], |_: &()| unreachable!());

        CodecResults {
            pco_pack: pco,
            pco_pack_lossy: None,
            columnar: col,
            serde_columnar: sc,
            msgpack: mp,
            vortex: vx,
        }
        .print(&[]);
    }
}

// ---------------------------------------------------------------------------
// columnar mirrors: map -> Vec<(K, V)> (no map impl in the crate), enum -> u8
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct NodeCol {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
    pub tags: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct WayCol {
    pub id: i64,
    pub refs: Vec<i64>,
    pub tags: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct MemberCol {
    pub rtype: u8,
    pub ref_id: i64,
    pub role: Option<String>,
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, serde::Serialize, serde::Deserialize)]
pub struct RelationCol {
    pub id: i64,
    pub members: Vec<MemberCol>,
    pub tags: Vec<(String, String)>,
}

fn tags_kv(tags: &HashMap<SmolStr, String>) -> Vec<(String, String)> {
    tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn rtype_u8(t: &MemberType) -> u8 {
    match t {
        MemberType::Node => 0,
        MemberType::Way => 1,
        MemberType::Relation => 2,
    }
}

impl From<&Node> for NodeCol {
    fn from(r: &Node) -> Self {
        NodeCol { id: r.id, lat: r.lat, lon: r.lon, tags: tags_kv(&r.tags) }
    }
}

impl From<&Way> for WayCol {
    fn from(r: &Way) -> Self {
        WayCol { id: r.id, refs: r.refs.clone(), tags: tags_kv(&r.tags) }
    }
}

impl From<&Member> for MemberCol {
    fn from(r: &Member) -> Self {
        MemberCol { rtype: rtype_u8(&r.rtype), ref_id: r.ref_id, role: r.role.as_deref().map(str::to_string) }
    }
}

impl From<&Relation> for RelationCol {
    fn from(r: &Relation) -> Self {
        RelationCol { id: r.id, members: r.members.iter().map(MemberCol::from).collect(), tags: tags_kv(&r.tags) }
    }
}

// ---------------------------------------------------------------------------
// serde_columnar mirrors: ints -> DeltaRle, f64 -> Rle (the struct.rs
// convention); map/Option/Vec<primitive> fields -> per-row blobs (no strategy)
// ---------------------------------------------------------------------------

mod osm_sc {
    use super::*;
    use serde_columnar::columnar;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct NodeScRow {
        #[columnar(strategy = "DeltaRle")]
        pub id: i64,
        #[columnar(strategy = "Rle")]
        pub lat: f64,
        #[columnar(strategy = "Rle")]
        pub lon: f64,
        pub tags: Vec<(String, String)>,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct WayScRow {
        #[columnar(strategy = "DeltaRle")]
        pub id: i64,
        pub refs: Vec<i64>,
        pub tags: Vec<(String, String)>,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct MemberScRow {
        #[columnar(strategy = "DeltaRle")]
        pub rtype: u8,
        #[columnar(strategy = "DeltaRle")]
        pub ref_id: i64,
        pub role: Option<String>,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct RelationScRow {
        #[columnar(strategy = "DeltaRle")]
        pub id: i64,
        #[columnar(class = "vec")]
        pub members: Vec<MemberScRow>,
        pub tags: Vec<(String, String)>,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct NodeScStore {
        #[columnar(class = "vec")]
        pub data: Vec<NodeScRow>,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct WayScStore {
        #[columnar(class = "vec")]
        pub data: Vec<WayScRow>,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct RelationScStore {
        #[columnar(class = "vec")]
        pub data: Vec<RelationScRow>,
    }

    impl From<&Node> for NodeScRow {
        fn from(r: &Node) -> Self {
            NodeScRow { id: r.id, lat: r.lat, lon: r.lon, tags: super::tags_kv(&r.tags) }
        }
    }

    impl From<&Way> for WayScRow {
        fn from(r: &Way) -> Self {
            WayScRow { id: r.id, refs: r.refs.clone(), tags: super::tags_kv(&r.tags) }
        }
    }

    impl From<&Member> for MemberScRow {
        fn from(r: &Member) -> Self {
            MemberScRow {
                rtype: super::rtype_u8(&r.rtype),
                ref_id: r.ref_id,
                role: r.role.as_deref().map(str::to_string),
            }
        }
    }

    impl From<&Relation> for RelationScRow {
        fn from(r: &Relation) -> Self {
            RelationScRow {
                id: r.id,
                members: r.members.iter().map(MemberScRow::from).collect(),
                tags: super::tags_kv(&r.tags),
            }
        }
    }
}

impl common::ScStore for osm_sc::NodeScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

impl common::ScStore for osm_sc::WayScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

impl common::ScStore for osm_sc::RelationScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

// ---------------------------------------------------------------------------
// Vortex mirrors: tags stay native HashMap, stored as `vortex.map` via
// `map_as_struct(false)` (an open tag vocabulary promoted to a
// sample-derived schema would silently drop unseen keys),
// SmolStr -> String, MemberType -> utf8 (rare `relation` variant ->
// Union, which Vortex can't encode)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeVx {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
    pub tags: HashMap<SmolStr, String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WayVx {
    pub id: i64,
    pub refs: Vec<i64>,
    pub tags: HashMap<SmolStr, String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemberVx {
    pub rtype: String,
    pub ref_id: i64,
    pub role: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RelationVx {
    pub id: i64,
    pub members: Vec<MemberVx>,
    pub tags: HashMap<SmolStr, String>,
}

fn rtype_str(t: &MemberType) -> &'static str {
    match t {
        MemberType::Node => "node",
        MemberType::Way => "way",
        MemberType::Relation => "relation",
    }
}

impl From<&Node> for NodeVx {
    fn from(r: &Node) -> Self {
        NodeVx { id: r.id, lat: r.lat, lon: r.lon, tags: r.tags.clone() }
    }
}

impl From<&Way> for WayVx {
    fn from(r: &Way) -> Self {
        WayVx { id: r.id, refs: r.refs.clone(), tags: r.tags.clone() }
    }
}

impl From<&Member> for MemberVx {
    fn from(r: &Member) -> Self {
        MemberVx {
            rtype: rtype_str(&r.rtype).to_string(),
            ref_id: r.ref_id,
            role: r.role.as_deref().map(str::to_string),
        }
    }
}

impl From<&Relation> for RelationVx {
    fn from(r: &Relation) -> Self {
        RelationVx { id: r.id, members: r.members.iter().map(MemberVx::from).collect(), tags: r.tags.clone() }
    }
}

// ---------------------------------------------------------------------------
// Load (osm-pbf: parse blocks -> decode elements -> convert, one pass)
// ---------------------------------------------------------------------------

/// Austria bbox (~ 46.37-49.01 N, 9.53-17.10 E), padded by 0.1 deg. The
/// pad admits nodes of ways and relations that cross the border.
const AUSTRIA_LAT0: f64 = 46.27;
const AUSTRIA_LAT1: f64 = 49.11;
const AUSTRIA_LON0: f64 = 9.43;
const AUSTRIA_LON1: f64 = 17.20;

#[derive(Debug, Clone, Copy)]
struct Stats {
    /// Full range over all nodes (a few cross-border strays widen it).
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    /// Bbox of the in-country bulk (inside the padded bbox above); its
    /// centroid is where the bbox filter window is centered.
    lat_bulk_min: f64,
    lat_bulk_max: f64,
    lon_bulk_min: f64,
    lon_bulk_max: f64,
    n_outside: usize,
    members_node: usize,
    members_way: usize,
    members_relation: usize,
}

fn load() -> (Vec<Node>, Vec<Way>, Vec<Relation>, Stats) {
    let path = DS.ensure_downloaded();
    let buf = std::fs::read(&path).unwrap();

    // Owned blocks let the 811 MB buf drop before decode.
    let blocks: Vec<FileBlock> = common::ds_vortex_rt().block_on(async {
        use futures::TryStreamExt;
        parse_osm_pbf(std::io::Cursor::new(&buf)).try_collect().await.expect("PBF parse")
    });
    drop(buf);

    // decode_elements is an iterator; this pass is synchronous.
    let mut nodes: Vec<Node> = Vec::new();
    let mut ways: Vec<Way> = Vec::new();
    let mut relations: Vec<Relation> = Vec::new();
    let mut stats = Stats {
        lat_min: f64::INFINITY,
        lat_max: f64::NEG_INFINITY,
        lon_min: f64::INFINITY,
        lon_max: f64::NEG_INFINITY,
        lat_bulk_min: f64::INFINITY,
        lat_bulk_max: f64::NEG_INFINITY,
        lon_bulk_min: f64::INFINITY,
        lon_bulk_max: f64::NEG_INFINITY,
        n_outside: 0,
        members_node: 0,
        members_way: 0,
        members_relation: 0,
    };
    for fb in blocks {
        if let FileBlock::Primitive(pb) = fb {
            for el in decode_elements(pb) {
                let el = el.expect("PBF decode");
                match el {
                    Element::Node(n) => {
                        let lat = n.lat.as_f64();
                        let lon = n.lon.as_f64();
                        stats.lat_min = stats.lat_min.min(lat);
                        stats.lat_max = stats.lat_max.max(lat);
                        stats.lon_min = stats.lon_min.min(lon);
                        stats.lon_max = stats.lon_max.max(lon);
                        if lat >= AUSTRIA_LAT0 && lat <= AUSTRIA_LAT1 && lon >= AUSTRIA_LON0 && lon <= AUSTRIA_LON1 {
                            stats.lat_bulk_min = stats.lat_bulk_min.min(lat);
                            stats.lat_bulk_max = stats.lat_bulk_max.max(lat);
                            stats.lon_bulk_min = stats.lon_bulk_min.min(lon);
                            stats.lon_bulk_max = stats.lon_bulk_max.max(lon);
                        } else {
                            stats.n_outside += 1;
                        }
                        nodes.push(Node {
                            id: n.id.0,
                            lat,
                            lon,
                            tags: n.tags.iter().map(|(k, v)| (SmolStr::new(&**k), v.to_string())).collect(),
                        });
                    }
                    Element::Way(w) => {
                        ways.push(Way {
                            id: w.id.0,
                            refs: w.refs.iter().map(|id| id.0).collect(),
                            tags: w.tags.iter().map(|(k, v)| (SmolStr::new(&**k), v.to_string())).collect(),
                        });
                    }
                    Element::Relation(r) => {
                        let members = r
                            .members
                            .iter()
                            .map(|m| {
                                let rtype = match m.ty {
                                    osm_types::MemberType::Node => MemberType::Node,
                                    osm_types::MemberType::Way => MemberType::Way,
                                    osm_types::MemberType::Relation => MemberType::Relation,
                                };
                                match rtype {
                                    MemberType::Node => stats.members_node += 1,
                                    MemberType::Way => stats.members_way += 1,
                                    MemberType::Relation => stats.members_relation += 1,
                                }
                                Member { rtype, ref_id: m.id.0, role: m.role.as_deref().map(SmolStr::new) }
                            })
                            .collect();
                        relations.push(Relation {
                            id: r.id.0,
                            members,
                            tags: r.tags.iter().map(|(k, v)| (SmolStr::new(&**k), v.to_string())).collect(),
                        });
                    }
                }
            }
        }
        // Header / unknown blocks carry no elements.
    }

    // Loose bounds catch a unit error (raw nanodegrees) or a wrong file;
    // nearly all nodes must be in the padded bbox.
    assert!(
        stats.lat_min >= 45.5 && stats.lat_max <= 50.0,
        "node lat range implausible: [{}, {}]",
        stats.lat_min,
        stats.lat_max,
    );
    assert!(
        stats.lon_min >= 7.5 && stats.lon_max <= 20.0,
        "node lon range implausible: [{}, {}]",
        stats.lon_min,
        stats.lon_max,
    );
    assert!(
        stats.n_outside * 10 <= nodes.len(),
        "{} of {} nodes ({:.3}%) outside the padded Austria bbox",
        stats.n_outside,
        nodes.len(),
        stats.n_outside as f64 / nodes.len() as f64 * 100.0,
    );
    // The PBF lists each element once; duplicate ids would mean a decode
    // bug the bbox checks can't catch.
    let node_ids: std::collections::HashSet<i64> = nodes.iter().map(|n| n.id).collect();
    assert_eq!(node_ids.len(), nodes.len(), "duplicate node ids in the PBF");
    let way_ids: std::collections::HashSet<i64> = ways.iter().map(|w| w.id).collect();
    assert_eq!(way_ids.len(), ways.len(), "duplicate way ids in the PBF");
    let rel_ids: std::collections::HashSet<i64> = relations.iter().map(|r| r.id).collect();
    assert_eq!(rel_ids.len(), relations.len(), "duplicate relation ids in the PBF");
    (nodes, ways, relations, stats)
}
