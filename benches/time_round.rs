include!("bench_common.rs");

use chrono::{DateTime, Duration, Utc};
use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::PcoPack;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

fn main() {
    let data = data(100_000);
    println!("## time_round\n");

    let pco_no_round_serial_ms = avg_ms(|| RecordNoRound::write(data.clone()).unwrap());
    let pco_no_round_compressed = RecordNoRound::write(data.clone()).unwrap();
    let pco_no_round_buf: Vec<u8> = RecordNoRound::to_bytes(&pco_no_round_compressed).unwrap();
    let pco_no_round_deserial_ms = avg_ms(|| {
        let compressed = RecordNoRound::from_bytes(&pco_no_round_buf).unwrap();
        let results = RecordNoRound::filter(&compressed, json!({}), &vec![]).unwrap();
        assert_eq!(results.len(), data.len());
    });

    let data_round: Vec<RecordWithRound> = data.iter().map(RecordWithRound::from).collect();
    let pco_round_serial_ms = avg_ms(|| RecordWithRound::write(data_round.clone()).unwrap());
    let pco_round_compressed = RecordWithRound::write(data_round.clone()).unwrap();
    let pco_round_buf: Vec<u8> = RecordWithRound::to_bytes(&pco_round_compressed).unwrap();
    let pco_round_deserial_ms = avg_ms(|| {
        let compressed = RecordWithRound::from_bytes(&pco_round_buf).unwrap();
        let results = RecordWithRound::filter(&compressed, json!({}), &vec![]).unwrap();
        assert_eq!(results.len(), data.len());
    });

    let map_serial_ms = avg_ms(|| serialize_flat_map(&data));
    let map_buf = serialize_flat_map(&data);
    let map_deserial_ms = avg_ms(|| deserialize_flat(&map_buf));

    #[derive(Debug)]
    struct TimeRoundRow {
        metric: String,
        no_round: String,
        round_60s: String,
        msgpack: String,
    }

    impl MarkdownTableRow for TimeRoundRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Metric", "PcoPack (no round)", "PcoPack (round=60s)", "msgpack"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![self.metric.clone(), self.no_round.clone(), self.round_60s.clone(), self.msgpack.clone()]
        }
    }

    let rows = vec![
        TimeRoundRow {
            metric: "Serialize".to_string(),
            no_round: format_ms(pco_no_round_serial_ms),
            round_60s: format_ms(pco_round_serial_ms),
            msgpack: format_ms(map_serial_ms),
        },
        TimeRoundRow {
            metric: "Deserialize".to_string(),
            no_round: format_ms(pco_no_round_deserial_ms),
            round_60s: format_ms(pco_round_deserial_ms),
            msgpack: format_ms(map_deserial_ms),
        },
        TimeRoundRow {
            metric: "Size".to_string(),
            no_round: format_bytes(pco_no_round_buf.len()),
            round_60s: format_bytes(pco_round_buf.len()),
            msgpack: format_bytes(map_buf.len()),
        },
    ];
    println!("{}", as_table(&rows));
}

#[derive(Clone, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(index = [account_id])]
struct RecordNoRound {
    account_id: i64,
    name: String,
    active: bool,
    ts: DateTime<Utc>,
    optional_ts: Option<DateTime<Utc>>,
    ts_vec: Vec<DateTime<Utc>>,
    enum_with_ts: Enum,
    tuple_with_ts: (i64, DateTime<Utc>),
    ts_in_map: BTreeMap<String, DateTime<Utc>>,
    nested: Struct,
}

#[derive(Clone, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(index = [account_id], time_round = chrono::Duration::seconds(60))]
struct RecordWithRound {
    account_id: i64,
    name: String,
    active: bool,
    ts: DateTime<Utc>,
    optional_ts: Option<DateTime<Utc>>,
    ts_vec: Vec<DateTime<Utc>>,
    enum_with_ts: Enum,
    tuple_with_ts: (i64, DateTime<Utc>),
    ts_in_map: BTreeMap<String, DateTime<Utc>>,
    nested: Struct,
}

impl From<&RecordNoRound> for RecordWithRound {
    fn from(r: &RecordNoRound) -> Self {
        RecordWithRound {
            account_id: r.account_id,
            name: r.name.clone(),
            active: r.active,
            ts: r.ts,
            optional_ts: r.optional_ts,
            ts_vec: r.ts_vec.clone(),
            enum_with_ts: r.enum_with_ts.clone(),
            tuple_with_ts: r.tuple_with_ts,
            ts_in_map: r.ts_in_map.clone(),
            nested: r.nested.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, PcoPack, Serialize, Deserialize)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
enum Enum {
    Int(i32),
    Ts(DateTime<Utc>),
}

impl Default for Enum {
    fn default() -> Self {
        Enum::Int(0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, PcoPack, Serialize, Deserialize)]
#[pco_pack(time_round = chrono::Duration::seconds(60))]
struct Struct {
    ts: DateTime<Utc>,
}

fn noise_i32(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x01000193).wrapping_add(i as u32);
    (h ^ (h >> 16)) as i32 & 0xF
}

fn data(n: usize) -> Vec<RecordNoRound> {
    let base = DateTime::<Utc>::from_timestamp(1700000000, 0).unwrap();
    // Spread data across ~24 hours with realistic microsecond-precision timestamps
    let total_seconds: i64 = 24 * 3600;
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            // Each row gets a unique timestamp spread across 24 hours, with microsecond jitter
            let micro_offset = if n == 0 {
                0
            } else {
                ((i as i64).wrapping_mul(86_400_000_000 / n as i64) + n as i64 * 12345) % total_seconds * 1_000_000
            };
            let ts = base + Duration::microseconds(micro_offset);
            // The row that will match the filter: ts == base (timestamp 1700000000)
            let ts = if i == 42 { base } else { ts };
            // ts_in_map: the key "match" has ts == base
            let ts_in_map = if i == 42 {
                let mut m = BTreeMap::new();
                m.insert("match".to_string(), base);
                m
            } else if i % 10 == 0 {
                let mut m = BTreeMap::new();
                m.insert("start".to_string(), base + Duration::microseconds(micro_offset));
                m
            } else {
                BTreeMap::new()
            };
            RecordNoRound {
                account_id: (i as i64 / 1000) * 1000,
                name: format!("name_{}_{}", i % 5000, n & 0xFF),
                active: (i + n as usize) % 3 != 0,
                ts,
                optional_ts: if i % 2 == 0 { Some(ts) } else { None },
                ts_vec: (0..((i % 5) + 1)).map(|j| ts + Duration::microseconds(j as i64 * 1_234_567)).collect(),
                enum_with_ts: if i % 3 == 0 { Enum::Ts(ts) } else { Enum::Int(n as i32) },
                tuple_with_ts: (i as i64, ts),
                ts_in_map,
                nested: Struct { ts: ts + Duration::microseconds(30_000_000) },
            }
        })
        .collect()
}

fn serialize_flat_map(data: &[RecordNoRound]) -> Vec<u8> {
    let mut msgpack_buf = Vec::new();
    data.serialize(&mut rmp_serde::Serializer::new(&mut msgpack_buf).with_struct_map()).unwrap();
    let compressed = zstd::encode_all(msgpack_buf.as_slice(), 3).unwrap();
    compressed
}

fn deserialize_flat(buf: &[u8]) -> Vec<RecordNoRound> {
    let decompressed = zstd::decode_all(buf).unwrap();
    let data: Vec<RecordNoRound> = rmp_serde::from_slice(&decompressed).unwrap();
    data
}
