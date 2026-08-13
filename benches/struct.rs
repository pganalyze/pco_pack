include!("bench_common.rs");

use columnar::{Borrow, Columnar, FromBytes, Index, Len};
use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::PcoPack;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_bytes::ByteBuf;
use serde_columnar::{columnar, from_bytes, to_vec};
use serde_json::json;

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

    // ---- msgpack benchmarks ----
    let map_serial_ms = avg_ms(|| serialize_flat_map(&data));
    let map_buf = serialize_flat_map(&data);
    let map_deserial_ms = avg_ms(|| deserialize_flat(&map_buf));

    #[derive(Debug)]
    struct StructRow {
        metric: String,
        pco_pack: String,
        columnar: String,
        serde_columnar: String,
        msgpack: String,
    }

    impl MarkdownTableRow for StructRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Metric", "PcoPack", "columnar", "serde_columnar", "msgpack"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![
                self.metric.clone(),
                self.pco_pack.clone(),
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

    let rows = vec![
        StructRow {
            metric: "Serialize".to_string(),
            pco_pack: format_ms(pco_serial_ms),
            columnar: format_ms(columnar_serial_ms),
            serde_columnar: format_ms(sc_serial_ms),
            msgpack: format_ms(map_serial_ms),
        },
        StructRow {
            metric: "Deserialize".to_string(),
            pco_pack: format_ms(pco_deserial_ms),
            columnar: format_ms(columnar_deserial_ms),
            serde_columnar: format_ms(sc_deserial_ms),
            msgpack: format_ms(map_deserial_ms),
        },
        StructRow {
            metric: "Size".to_string(),
            pco_pack: format_bytes(pco_buf.len()),
            columnar: format_bytes(columnar_size_bytes),
            serde_columnar: format_bytes(sc_buf.len()),
            msgpack: format_bytes(map_buf.len()),
        },
        StructRow {
            metric: "Filter account_id (20% of rows)".to_string(),
            pco_pack: format_ms(pco_filter_single_ms),
            columnar: format_ms(col_filter_single_ms),
            serde_columnar: format_ms(sc_filter_single_ms),
            msgpack: format_ms(map_filter_single_ms),
        },
        StructRow {
            metric: "Filter color + score (1 row)".to_string(),
            pco_pack: format_ms(pco_filter_multi_ms),
            columnar: format_ms(col_filter_multi_ms),
            serde_columnar: format_ms(sc_filter_multi_ms),
            msgpack: format_ms(map_filter_multi_ms),
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
    payload: ByteBuf,
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
    payload: Vec<u8>,
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
            payload: r.payload.as_slice().to_vec(),
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
    pub struct Payload {
        #[columnar(strategy = "Rle")]
        pub values: Vec<u8>,
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
        pub payload: Payload,
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
                payload: Payload { values: r.payload.as_slice().to_vec() },
            }
        }
    }
}

fn noise_i32(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x01000193).wrapping_add(i as u32);
    (h ^ (h >> 16)) as i32 & 0xF
}

fn data(n: usize) -> Vec<Record> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            Record {
                account_id: i as i64 % 5, // 5 unique account IDs
                name: format!("name_{}_{}", i % 5000, n & 0xFF),
                score: (i as f64) / 100.0 + (n as f64) / 100.0,
                active: (i + n as usize) % 3 != 0,
                tag: if i % 2 == 0 { Some(((i % 100) as i32) + n) } else { None },
                tags: (0..((i % 5) + 1)).map(|j| (i * 100 + j) as i32 + n).collect(),
                color: match (i + n as usize) % 4 {
                    0 => Color::Red,
                    1 => Color::Green,
                    2 => Color::Blue,
                    _ => Color::Yellow,
                },
                payload: ByteBuf::from(vec![n as u8; 64]),
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
