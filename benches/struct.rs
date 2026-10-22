include!("bench_common.rs");

use arrow_schema::FieldRef;
use columnar::{Borrow, Columnar, FromBytes, Index, Len};
use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::PcoPack;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_arrow::schema::{SchemaLike, TracingOptions};
use serde_columnar::{columnar, from_bytes, to_vec};
use serde_json::json;
use vortex::array::expr::{and, col, eq, lit, or};
use vortex::array::stream::ArrayStreamExt;
use vortex::arrow::ArrowSessionExt as _;
use vortex::buffer::ByteBuffer;
use vortex::compressor::BtrBlocksCompressorBuilder;
use vortex::file::{OpenOptionsSessionExt, WriteOptionsSessionExt, WriteStrategyBuilder};
use vortex::session::VortexSession;

/// The row the multi-filter (`color == Red && score == 500.0`) matches once:
/// its fields are forced and no generated `lognorm_small` value hits 500.0.
const SENTINEL: usize = 12_345;

fn main() {
    let data = data(500_000);
    println!("## Structs\n");

    // ---- PcoPack benchmarks ----
    let pco_serial_ms = avg_ms(|| Record::write(data.clone()).unwrap());
    let pco_compressed = Record::write(data.clone()).unwrap();
    let pco_buf = Record::to_bytes(&pco_compressed).unwrap();
    let pco_deserial_ms = avg_ms(|| {
        let compressed = Record::from_bytes(&pco_buf).unwrap();
        let results = Record::filter(&compressed, json!({}), &vec![]).unwrap();
        assert_eq!(results.len(), data.len());
    });

    // ---- Columnar benchmarks ----
    let columnar_serial_ms = avg_ms(|| build_columnar(&data));
    let columnar_buf = build_columnar(&data);
    let columnar_size_bytes = columnar_buf.len();
    let columnar_deserial_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*columnar_buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        type B<'a> = columnar::BorrowedOf<'a, ColumnarRecord>;
        let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
        black_box(decoded);
        assert_eq!(Len::len(&decoded), data.len());
    });

    // ---- serde_columnar benchmarks ----
    let sc_serial_ms = avg_ms(|| build_serde_columnar(&data));
    let sc_buf = build_serde_columnar(&data);
    let sc_deserial_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*sc_buf).unwrap();
        let decoded: serde_columnar_record::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
        black_box(decoded.data.len());
    });

    // ---- msgpack benchmarks (single-frame) ----
    let map_serial_ms = avg_ms(|| serialize_flat_map(&data));
    let map_buf = serialize_flat_map(&data);
    let map_deserial_ms = avg_ms(|| deserialize_flat(&map_buf));

    // ---- Vortex benchmarks ----
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let (vortex_session, vortex_buf) = setup_vortex_file(&rt, &data);
    let vortex_serial_ms = avg_ms(|| {
        let bytes = rt.block_on(async { write_vortex_file(&vortex_session, &data).await });
        black_box(bytes.len())
    });
    let vortex_deserial_ms = avg_ms(|| {
        let rows = rt.block_on(async { vortex_read_all(&vortex_session, vortex_buf.clone()).await });
        assert_eq!(rows.len(), data.len());
        black_box(rows.len())
    });
    let vfilter_single = eq(col("account_id"), lit(2_i64));
    let vortex_filter_single_ms = avg_ms(|| {
        rt.block_on(async {
            let batch =
                vortex_scan(&vortex_session, vortex_buf.clone(), Some(vfilter_single.clone()), Some(&["account_id"]))
                    .await;
            assert_eq!(batch.num_rows(), 100_000);
            black_box(batch)
        })
    });
    let vfilter_multi = and(eq(col("color"), lit(0_u8)), eq(col("score"), lit(500_f64)));
    let vortex_filter_multi_ms = avg_ms(|| {
        rt.block_on(async {
            let batch = vortex_scan(
                &vortex_session,
                vortex_buf.clone(),
                Some(vfilter_multi.clone()),
                Some(&["color", "score"]),
            )
            .await;
            assert_eq!(batch.num_rows(), 1);
            black_box(batch)
        })
    });

    #[derive(Debug)]
    struct StructRow {
        metric: String,
        pco_pack: String,
        columnar: String,
        serde_columnar: String,
        msgpack: String,
        vortex: String,
    }

    impl MarkdownTableRow for StructRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Metric", "PcoPack", "Vortex", "columnar", "serde_columnar", "msgpack"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![
                self.metric.clone(),
                self.pco_pack.clone(),
                self.vortex.clone(),
                self.columnar.clone(),
                self.serde_columnar.clone(),
                self.msgpack.clone(),
            ]
        }
    }

    // --- Single filter: account_id == 2 ---
    let pco_compressed2 = Record::write(data.clone()).unwrap();
    let pco_buf2 = Record::to_bytes(&pco_compressed2).unwrap();
    let pco_filter_single_ms = avg_ms(|| {
        let compressed = Record::from_bytes(&pco_buf2).unwrap();
        let results = Record::filter(&compressed, json!({"account_id": 2}), &vec![]).unwrap();
        assert_eq!(results.len(), 100_000);
    });

    let col_filter_single_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*columnar_buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        type B<'a> = columnar::BorrowedOf<'a, ColumnarRecord>;
        let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
        let count = Len::len(&decoded);
        let mut matches: usize = 0;
        for i in 0..count {
            let record = Index::get(&decoded, i);
            if *record.account_id == 2 {
                matches += 1;
            }
        }
        assert_eq!(matches, 100_000);
    });

    let map_filter_single_ms = avg_ms(|| {
        let deserialized = deserialize_flat(&map_buf);
        let results: Vec<_> = deserialized.into_iter().filter(|r| r.account_id == 2).collect();
        assert_eq!(results.len(), 100_000);
    });

    // --- Multi filter: color == Red && score == 500.0 ---
    let pco_filter_multi_ms = avg_ms(|| {
        let compressed = Record::from_bytes(&pco_buf2).unwrap();
        let results = Record::filter(&compressed, json!({"color": 0, "score": 500.0}), &vec![]).unwrap();
        assert_eq!(results.len(), 1);
    });

    let col_filter_multi_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*columnar_buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        type B<'a> = columnar::BorrowedOf<'a, ColumnarRecord>;
        let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
        let count = Len::len(&decoded);
        let mut matches: usize = 0;
        for i in 0..count {
            let record = Index::get(&decoded, i);
            if *record.color == 0 && *record.score == 500.0 {
                matches += 1;
            }
        }
        assert_eq!(matches, 1);
    });

    let map_filter_multi_ms = avg_ms(|| {
        let deserialized = deserialize_flat(&map_buf);
        let results: Vec<_> =
            deserialized.into_iter().filter(|r| matches!(r.color, Color::Red) && r.score == 500.0).collect();
        assert_eq!(results.len(), 1);
    });

    // --- serde_columnar filter: full deserialize then in-Rust filter ---
    let sc_filter_single_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*sc_buf).unwrap();
        let decoded: serde_columnar_record::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
        let count = decoded.data.len();
        let mut matches: usize = 0;
        for i in 0..count {
            if decoded.data[i].account_id == 2 {
                matches += 1;
            }
        }
        assert_eq!(matches, 100_000);
    });

    let sc_filter_multi_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*sc_buf).unwrap();
        let decoded: serde_columnar_record::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
        let count = decoded.data.len();
        let mut matches: usize = 0;
        for i in 0..count {
            if decoded.data[i].color == 0 && decoded.data[i].score == 500.0 {
                matches += 1;
            }
        }
        assert_eq!(matches, 1);
    });

    // --- Text filters over the high-cardinality `name` column ---
    let name_hit = data[17].name.clone();
    let text_eq_exp = data.iter().filter(|r| r.name == name_hit).count();
    assert!(text_eq_exp > 0, "text eq target must match at least one row");
    let name_in = vec![data[0].name.clone(), name_hit.clone()];
    let text_in_exp = data.iter().filter(|r| name_in.contains(&r.name)).count();

    let pco_text_eq_ms = avg_ms(|| {
        let compressed = Record::from_bytes(&pco_buf2).unwrap();
        let results = Record::filter(&compressed, json!({"name": name_hit.clone()}), &vec![]).unwrap();
        assert_eq!(results.len(), text_eq_exp);
    });
    let pco_text_miss_ms = avg_ms(|| {
        let compressed = Record::from_bytes(&pco_buf2).unwrap();
        let results = Record::filter(&compressed, json!({"name": "name_zzz_no_such_name"}), &vec![]).unwrap();
        assert!(results.is_empty());
    });
    let pco_text_in_ms = avg_ms(|| {
        let compressed = Record::from_bytes(&pco_buf2).unwrap();
        let results = Record::filter(&compressed, json!({"name": name_in.clone()}), &vec![]).unwrap();
        assert_eq!(results.len(), text_in_exp);
    });

    let col_text_eq_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*columnar_buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        type B<'a> = columnar::BorrowedOf<'a, ColumnarRecord>;
        let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
        let count = Len::len(&decoded);
        let mut matches: usize = 0;
        for i in 0..count {
            let record = Index::get(&decoded, i);
            if record.name == name_hit.as_bytes() {
                matches += 1;
            }
        }
        assert_eq!(matches, text_eq_exp);
    });

    let sc_text_eq_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*sc_buf).unwrap();
        let decoded: serde_columnar_record::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
        let matches = decoded.data.iter().filter(|r| r.name == name_hit).count();
        assert_eq!(matches, text_eq_exp);
    });

    let col_text_miss_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*columnar_buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        type B<'a> = columnar::BorrowedOf<'a, ColumnarRecord>;
        let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
        let count = Len::len(&decoded);
        let mut matches: usize = 0;
        for i in 0..count {
            let record = Index::get(&decoded, i);
            if record.name == "name_zzz_no_such_name".as_bytes() {
                matches += 1;
            }
        }
        assert_eq!(matches, 0);
    });

    let sc_text_miss_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*sc_buf).unwrap();
        let decoded: serde_columnar_record::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
        let matches = decoded.data.iter().filter(|r| r.name == "name_zzz_no_such_name").count();
        assert_eq!(matches, 0);
    });

    let map_text_miss_ms = avg_ms(|| {
        let deserialized = deserialize_flat(&map_buf);
        let results: Vec<_> = deserialized.into_iter().filter(|r| r.name == "name_zzz_no_such_name").collect();
        assert!(results.is_empty());
    });

    let vfilter_text_miss = eq(col("name"), lit("name_zzz_no_such_name"));
    let vortex_text_miss_ms = avg_ms(|| {
        rt.block_on(async {
            let batch =
                vortex_scan(&vortex_session, vortex_buf.clone(), Some(vfilter_text_miss.clone()), Some(&["name"]))
                    .await;
            assert_eq!(batch.num_rows(), 0);
            black_box(batch)
        })
    });

    let map_text_eq_ms = avg_ms(|| {
        let deserialized = deserialize_flat(&map_buf);
        let results: Vec<_> = deserialized.into_iter().filter(|r| r.name == name_hit).collect();
        assert_eq!(results.len(), text_eq_exp);
    });

    let vfilter_text = eq(col("name"), lit(name_hit.clone()));
    let vortex_text_eq_ms = avg_ms(|| {
        rt.block_on(async {
            let batch =
                vortex_scan(&vortex_session, vortex_buf.clone(), Some(vfilter_text.clone()), Some(&["name"])).await;
            assert_eq!(batch.num_rows(), text_eq_exp);
            black_box(batch)
        })
    });

    // Text IN over both names: the comparison codecs run the membership
    // test in Rust after decode; Vortex gets an or() filter pushdown.
    let col_text_in_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*columnar_buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        type B<'a> = columnar::BorrowedOf<'a, ColumnarRecord>;
        let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
        let count = Len::len(&decoded);
        let mut matches: usize = 0;
        for i in 0..count {
            let record = Index::get(&decoded, i);
            if name_in.iter().any(|n| record.name == n.as_bytes()) {
                matches += 1;
            }
        }
        assert_eq!(matches, text_in_exp);
    });

    let sc_text_in_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*sc_buf).unwrap();
        let decoded: serde_columnar_record::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
        let matches = decoded.data.iter().filter(|r| name_in.contains(&r.name)).count();
        assert_eq!(matches, text_in_exp);
    });

    let map_text_in_ms = avg_ms(|| {
        let deserialized = deserialize_flat(&map_buf);
        let results: Vec<_> = deserialized.into_iter().filter(|r| name_in.contains(&r.name)).collect();
        assert_eq!(results.len(), text_in_exp);
    });

    let vfilter_text_in = or(eq(col("name"), lit(name_in[0].clone())), eq(col("name"), lit(name_in[1].clone())));
    let vortex_text_in_ms = avg_ms(|| {
        rt.block_on(async {
            let batch =
                vortex_scan(&vortex_session, vortex_buf.clone(), Some(vfilter_text_in.clone()), Some(&["name"])).await;
            assert_eq!(batch.num_rows(), text_in_exp);
            black_box(batch)
        })
    });

    // --- Filter + projection + reduce: active == true, load [account_id,
    // score] only, reduce account_id => avg(score). The name/tags/color
    // payloads stay compressed; the other codecs have no partial read path. ---
    let active_exp = data.iter().filter(|r| r.active).count();
    let mut proj_truth: std::collections::HashMap<i64, (f64, u64)> = Default::default();
    for r in data.iter().filter(|r| r.active) {
        let e = proj_truth.entry(r.account_id).or_default();
        e.0 += r.score;
        e.1 += 1;
    }
    let (proj_ms, proj_map) = avg_ms_pair(|| {
        let compressed = Record::from_bytes(&pco_buf2).unwrap();
        let results = Record::filter(&compressed, json!({"active": true}), &["account_id", "score"]).unwrap();
        assert_eq!(results.len(), active_exp);
        assert!(results.iter().all(|r| r.name.is_empty()), "unprojected text stays default");
        let mut acc: std::collections::HashMap<i64, (f64, u64)> = Default::default();
        for r in &results {
            let e = acc.entry(r.account_id).or_default();
            e.0 += r.score;
            e.1 += 1;
        }
        acc
    });
    assert_eq!(proj_map, proj_truth, "projected reduce");

    // Comparison-codec baselines: none can project, so each decodes every
    // row it filters and folds active survivors into the same map. The
    // numbers name the full-decode cost of not having metadata.
    let col_reduce_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*columnar_buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        type B<'a> = columnar::BorrowedOf<'a, ColumnarRecord>;
        let decoded = B::from_bytes(&mut columnar::bytes::indexed::decode(words));
        let count = Len::len(&decoded);
        let mut acc: std::collections::HashMap<i64, (f64, u64)> = Default::default();
        for i in 0..count {
            let record = Index::get(&decoded, i);
            if record.active {
                let e = acc.entry(*record.account_id).or_default();
                e.0 += *record.score;
                e.1 += 1;
            }
        }
        assert_eq!(acc, proj_truth, "columnar reduce");
    });

    let sc_reduce_ms = avg_ms(|| {
        let decompressed = zstd::decode_all(&*sc_buf).unwrap();
        let decoded: serde_columnar_record::SerdeColumnarStore = from_bytes(&decompressed).unwrap();
        let mut acc: std::collections::HashMap<i64, (f64, u64)> = Default::default();
        for r in decoded.data.iter().filter(|r| r.active) {
            let e = acc.entry(r.account_id).or_default();
            e.0 += r.score;
            e.1 += 1;
        }
        assert_eq!(acc, proj_truth, "serde_columnar reduce");
    });

    let map_reduce_ms = avg_ms(|| {
        let deserialized = deserialize_flat(&map_buf);
        let mut acc: std::collections::HashMap<i64, (f64, u64)> = Default::default();
        for r in deserialized.iter().filter(|r| r.active) {
            let e = acc.entry(r.account_id).or_default();
            e.0 += r.score;
            e.1 += 1;
        }
        assert_eq!(acc, proj_truth, "msgpack reduce");
    });

    let vfilter_active = eq(col("active"), lit(true));
    let vortex_reduce_ms = avg_ms(|| {
        rt.block_on(async {
            // Same partial read as PcoPack's projected reduce: the pushed
            // active filter is exact, so the fold consumes every projected
            // (account_id, score) row without re-testing.
            let batch = vortex_scan(
                &vortex_session,
                vortex_buf.clone(),
                Some(vfilter_active.clone()),
                Some(&["account_id", "score"]),
            )
            .await;
            assert_eq!(batch.num_rows(), active_exp);
            let rows: Vec<(i64, f64)> = serde_arrow::from_record_batch(&batch).unwrap();
            let mut acc: std::collections::HashMap<i64, (f64, u64)> = Default::default();
            for (account_id, score) in &rows {
                let e = acc.entry(*account_id).or_default();
                e.0 += *score;
                e.1 += 1;
            }
            assert_eq!(acc, proj_truth, "vortex reduce");
            black_box(rows)
        })
    });

    let text_eq_label = format!("Text eq ({} rows)", text_eq_exp);
    let text_in_label = format!("Text IN ({} rows)", text_in_exp);
    let pct = active_exp as f64 * 100.0 / data.len() as f64;
    let num = format!("{:.2}", pct).trim_end_matches('0').trim_end_matches('.').to_string();
    let reduce_label = format!("Reduce {num}%");

    let rows = vec![
        StructRow {
            metric: "Size".to_string(),
            pco_pack: format_bytes(pco_buf.len()),
            columnar: format_bytes(columnar_size_bytes),
            serde_columnar: format_bytes(sc_buf.len()),
            msgpack: format_bytes(map_buf.len()),
            vortex: format_bytes(vortex_buf.len()),
        },
        StructRow {
            metric: "Serialize".to_string(),
            pco_pack: format_ms(pco_serial_ms),
            columnar: format_ms(columnar_serial_ms),
            serde_columnar: format_ms(sc_serial_ms),
            msgpack: format_ms(map_serial_ms),
            vortex: format_ms(vortex_serial_ms),
        },
        StructRow {
            metric: "Deserialize".to_string(),
            pco_pack: format_ms(pco_deserial_ms),
            columnar: format_ms(columnar_deserial_ms),
            serde_columnar: format_ms(sc_deserial_ms),
            msgpack: format_ms(map_deserial_ms),
            vortex: format_ms(vortex_deserial_ms),
        },
        StructRow {
            metric: "Filter 20%".to_string(),
            pco_pack: format_ms(pco_filter_single_ms),
            columnar: format_ms(col_filter_single_ms),
            serde_columnar: format_ms(sc_filter_single_ms),
            msgpack: format_ms(map_filter_single_ms),
            vortex: format_ms(vortex_filter_single_ms),
        },
        StructRow {
            metric: "Filter <0.01%".to_string(),
            pco_pack: format_ms(pco_filter_multi_ms),
            columnar: format_ms(col_filter_multi_ms),
            serde_columnar: format_ms(sc_filter_multi_ms),
            msgpack: format_ms(map_filter_multi_ms),
            vortex: format_ms(vortex_filter_multi_ms),
        },
        StructRow {
            metric: text_eq_label,
            pco_pack: format_ms(pco_text_eq_ms),
            columnar: format_ms(col_text_eq_ms),
            serde_columnar: format_ms(sc_text_eq_ms),
            msgpack: format_ms(map_text_eq_ms),
            vortex: format_ms(vortex_text_eq_ms),
        },
        StructRow {
            metric: "Text miss".to_string(),
            pco_pack: format_ms(pco_text_miss_ms),
            columnar: format_ms(col_text_miss_ms),
            serde_columnar: format_ms(sc_text_miss_ms),
            msgpack: format_ms(map_text_miss_ms),
            vortex: format_ms(vortex_text_miss_ms),
        },
        StructRow {
            metric: text_in_label,
            pco_pack: format_ms(pco_text_in_ms),
            columnar: format_ms(col_text_in_ms),
            serde_columnar: format_ms(sc_text_in_ms),
            msgpack: format_ms(map_text_in_ms),
            vortex: format_ms(vortex_text_in_ms),
        },
        StructRow {
            metric: reduce_label,
            pco_pack: format_ms(proj_ms),
            columnar: format_ms(col_reduce_ms),
            serde_columnar: format_ms(sc_reduce_ms),
            msgpack: format_ms(map_reduce_ms),
            vortex: format_ms(vortex_reduce_ms),
        },
    ];
    println!("{}", as_table(&rows));
}

fn build_columnar(data: &[Record]) -> Vec<u8> {
    let mut container = <ColumnarRecord as columnar::Columnar>::Container::default();
    for record in data.iter() {
        columnar::Push::push(&mut container, &record.into());
    }
    let mut words = vec![];
    columnar::bytes::indexed::encode(&mut words, &container.borrow());
    let bytes = bytemuck::cast_slice(&words);
    zstd::encode_all(bytes, 3).unwrap()
}

fn build_serde_columnar(data: &[Record]) -> Vec<u8> {
    let store = serde_columnar_record::SerdeColumnarStore { data: data.iter().map(|r| r.into()).collect() };
    let postcard_buf = to_vec(&store).unwrap();
    zstd::encode_all(postcard_buf.as_slice(), 3).unwrap()
}

#[derive(Clone, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(index = [account_id])]
struct Record {
    account_id: i64,
    name: String,
    score: f64,
    active: bool,
    tag: Option<i32>,
    tags: Vec<i32>,
    color: Color,
}

/// Integer-backed serde serialization for optimal msgpack compression.
#[derive(Clone, Copy, PartialEq, Default, PcoPack)]
#[repr(u8)]
enum Color {
    #[default]
    Red = 0,
    Green = 1,
    Blue = 2,
    Yellow = 3,
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            0 => Ok(Color::Red),
            1 => Ok(Color::Green),
            2 => Ok(Color::Blue),
            3 => Ok(Color::Yellow),
            _ => Err(serde::de::Error::custom("Invalid variant integer")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Columnar, Serialize, Deserialize)]
struct ColumnarRecord {
    account_id: i64,
    name: String,
    score: f64,
    active: bool,
    tag: Option<i32>,
    tags: Vec<i32>,
    color: u8,
}

impl From<&Record> for ColumnarRecord {
    fn from(r: &Record) -> Self {
        ColumnarRecord {
            account_id: r.account_id,
            name: r.name.clone(),
            score: r.score,
            active: r.active,
            tag: r.tag,
            tags: r.tags.clone(),
            color: r.color as u8,
        }
    }
}

mod serde_columnar_record {
    use super::*;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct Tags {
        #[columnar(strategy = "Rle")]
        pub values: Vec<i32>,
    }

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct SerdeColumnarRecord {
        #[columnar(strategy = "DeltaRle")]
        pub account_id: i64,
        #[columnar(strategy = "Rle")]
        pub name: String,
        #[columnar(strategy = "Rle")]
        pub score: f64,
        #[columnar(strategy = "BoolRle")]
        pub active: bool,
        pub tag: Option<i32>,
        pub color: u8,
        pub tags: Tags,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct SerdeColumnarStore {
        #[columnar(class = "vec")]
        pub data: Vec<SerdeColumnarRecord>,
    }

    impl From<&Record> for SerdeColumnarRecord {
        fn from(r: &Record) -> Self {
            SerdeColumnarRecord {
                account_id: r.account_id,
                name: r.name.clone(),
                score: r.score,
                active: r.active,
                tag: r.tag,
                color: r.color as u8,
                tags: Tags { values: r.tags.clone() },
            }
        }
    }
}

/// Realistic field distributions (skewed counters, log-normal scores, skewed
/// enum, random names); `account_id` is the filter's index key.
fn data(n: usize) -> Vec<Record> {
    (0..n)
        .map(|i| {
            let sentinel = i == SENTINEL;
            Record {
                account_id: (i % 5) as i64, // 5 unique account IDs
                name: format!("name_{}", splitmix64(i.wrapping_add(0xA5A5)) % 1_000_000),
                score: if sentinel { 500.0 } else { lognorm_small(i) },
                active: if sentinel { true } else { unit64(i.wrapping_add(0x0707)) < 0.7 },
                tag: if i % 50 < 25 { None } else { Some(skew_small(i.wrapping_add(999)) as i32) },
                tags: (0..(i % 5 + 1)).map(|j| skew_small(i.wrapping_add(j * 131)) as i32).collect(),
                color: if sentinel {
                    Color::Red
                } else {
                    match cdf_index(&[0.6751, 0.9528, 0.9969, 1.0], unit64(i.wrapping_add(0xC0DE))) {
                        0 => Color::Red,
                        1 => Color::Green,
                        2 => Color::Blue,
                        _ => Color::Yellow,
                    }
                },
            }
        })
        .collect()
}

fn serialize_flat_map(data: &[Record]) -> Vec<u8> {
    let mut msgpack_buf = Vec::new();
    data.serialize(&mut rmp_serde::Serializer::new(&mut msgpack_buf).with_struct_map()).unwrap();
    let compressed = zstd::encode_all(msgpack_buf.as_slice(), 3).unwrap();
    compressed
}

fn deserialize_flat(buf: &[u8]) -> Vec<Record> {
    let decompressed = zstd::decode_all(buf).unwrap();
    let data: Vec<Record> = rmp_serde::from_slice(&decompressed).unwrap();
    data
}

// ---- Vortex helpers ----

/// Construct the session inside the Tokio runtime and write one compressed vortex file.
fn setup_vortex_file(rt: &tokio::runtime::Runtime, data: &[Record]) -> (VortexSession, ByteBuffer) {
    rt.block_on(async {
        use vortex::VortexSessionDefault as _;
        let session = VortexSession::default();
        let bytes = write_vortex_file(&session, data).await;
        (session, ByteBuffer::from(bytes))
    })
}

/// Serialize records to Arrow via serde_arrow, then write vortex bytes with
/// the compact strategy (serde_arrow is also needed to read back Utf8View).
async fn write_vortex_file(session: &VortexSession, data: &[Record]) -> Vec<u8> {
    let fields = <Vec<FieldRef>>::from_type::<Record>(TracingOptions::default()).unwrap();
    let batch = serde_arrow::to_record_batch(&fields, &data).unwrap();
    let schema_in = batch.schema();
    let array = session.arrow().from_arrow_record_batch(batch, &schema_in).unwrap();
    let mut bytes: Vec<u8> = vec![];
    let strategy = WriteStrategyBuilder::from_session(session)
        .with_btrblocks_builder(BtrBlocksCompressorBuilder::from_session(session).with_compact())
        .build();
    session.write_options().with_strategy(strategy).write(&mut bytes, array.to_array_stream()).await.unwrap();
    bytes
}

/// Open the file and scan all rows, materializing through Arrow back into Rust records.
async fn vortex_read_all(session: &VortexSession, buffer: ByteBuffer) -> Vec<Record> {
    let batch = vortex_scan(session, buffer, None, None).await;
    serde_arrow::from_record_batch(&batch).unwrap()
}

/// Open the file and run an (optionally filtered, optionally projected) scan,
/// materializing an Arrow RecordBatch of matches. Projection is the SELECT
/// list: one more scan knob next to the filter, same shape of API.
async fn vortex_scan(
    session: &VortexSession, buffer: ByteBuffer, filter: Option<vortex::array::expr::Expression>,
    projection: Option<&[&'static str]>,
) -> arrow_array::RecordBatch {
    let file = session.open_options().open_buffer(buffer).unwrap();
    let mut scan = file.scan().unwrap();
    if let Some(expr) = filter {
        let bound = expr.bind(file.dtype()).unwrap().optimize_recursive().unwrap();
        scan = scan.with_filter(bound);
    }
    if let Some(fields) = projection {
        let bound = vortex::array::expr::select(fields, vortex::array::expr::root())
            .bind(file.dtype())
            .unwrap()
            .optimize_recursive()
            .unwrap();
        scan = scan.with_projection(bound);
    }
    // Materialize the compressed scan result through Arrow (the session's stable export path).
    let array = scan.into_array_stream().unwrap().read_all().await.unwrap();
    let schema_out = session.arrow().to_arrow_schema(array.dtype()).unwrap();
    materialize_vortex_batch(session, array, &schema_out)
}

/// Materialize a vortex array into an Arrow RecordBatch via the session's executor.
#[allow(deprecated)] // execute_record_batch is deprecated in favor of the new Arrow exporter but still stable.
fn materialize_vortex_batch(
    session: &VortexSession, array: vortex::array::ArrayRef, schema: &arrow_schema::Schema,
) -> arrow_array::RecordBatch {
    use vortex::array::VortexSessionExecute;
    use vortex::arrow::ArrowArrayExecutor as _;
    let mut ctx = session.create_execution_ctx();
    array.execute_record_batch(schema, &mut ctx).unwrap()
}
