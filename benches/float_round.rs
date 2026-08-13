include!("bench_common.rs");
use pco_pack::PcoPack;
use serde_json::{Value as JsonValue, json};
use std::collections::BTreeMap;

fn main() {
    let data = data(100_000);
    println!("## float_round\n");

    let (pco_no_round_serial_ms, pco_no_round_compressed) = avg_ms_pair(|| RecordNoRound::write(data.clone()).unwrap());
    let pco_no_round_buf: Vec<u8> = RecordNoRound::to_bytes(&pco_no_round_compressed).unwrap();
    let pco_no_round_deserial_ms = avg_ms(|| {
        let compressed = RecordNoRound::from_bytes(&pco_no_round_buf).unwrap();
        let results = RecordNoRound::filter(&compressed, json!({}), &vec![]).unwrap();
        assert_eq!(results.len(), data.len());
    });

    let data_round: Vec<RecordWithRound> = data.iter().map(RecordWithRound::from).collect();
    let (pco_round_serial_ms, pco_round_compressed) =
        avg_ms_pair(|| RecordWithRound::write(data_round.clone()).unwrap());
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
    struct FloatRoundRow {
        metric: String,
        no_round: String,
        round_2: String,
        msgpack: String,
    }

    impl markdown_tables::MarkdownTableRow for FloatRoundRow {
        fn column_names() -> Vec<&'static str> {
            vec!["Metric", "PcoPack (no round)", "PcoPack (round=2)", "msgpack"]
        }

        fn column_values(&self) -> Vec<String> {
            vec![self.metric.clone(), self.no_round.clone(), self.round_2.clone(), self.msgpack.clone()]
        }
    }

    let rows = vec![
        FloatRoundRow {
            metric: "Serialize".to_string(),
            no_round: format_ms(pco_no_round_serial_ms),
            round_2: format_ms(pco_round_serial_ms),
            msgpack: format_ms(map_serial_ms),
        },
        FloatRoundRow {
            metric: "Deserialize".to_string(),
            no_round: format_ms(pco_no_round_deserial_ms),
            round_2: format_ms(pco_round_deserial_ms),
            msgpack: format_ms(map_deserial_ms),
        },
        FloatRoundRow {
            metric: "Size".to_string(),
            no_round: format_bytes(pco_no_round_buf.len()),
            round_2: format_bytes(pco_round_buf.len()),
            msgpack: format_bytes(map_buf.len()),
        },
    ];
    println!("{}", markdown_tables::as_table(&rows));
}

fn serialize_flat_map(data: &[RecordNoRound]) -> Vec<u8> {
    let mut msgpack_buf = Vec::new();
    <[RecordNoRound] as serde::Serialize>::serialize(
        data,
        &mut rmp_serde::Serializer::new(&mut msgpack_buf).with_struct_map(),
    )
    .unwrap();
    zstd::encode_all(msgpack_buf.as_slice(), 3).unwrap()
}

fn deserialize_flat(buf: &[u8]) -> Vec<RecordNoRound> {
    let decompressed = zstd::decode_all(buf).unwrap();
    rmp_serde::from_slice(&decompressed).unwrap()
}

#[derive(Clone, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(index = [account_id])]
struct RecordNoRound {
    account_id: i64,
    name: String,
    active: bool,
    f64_value: f64,
    f32_value: f32,
    optional_f64: Option<f64>,
    float_vec: Vec<f64>,
    enum_with_float: Enum,
    tuple_with_float: (i64, f64),
    float_in_map: BTreeMap<String, f64>,
    nested: Struct,
    json_obj: JsonValue,
}

#[derive(Clone, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(index = [account_id], float_round = 2)]
struct RecordWithRound {
    account_id: i64,
    name: String,
    active: bool,
    f64_value: f64,
    f32_value: f32,
    optional_f64: Option<f64>,
    float_vec: Vec<f64>,
    enum_with_float: Enum,
    tuple_with_float: (i64, f64),
    float_in_map: BTreeMap<String, f64>,
    nested: Struct,
    json_obj: JsonValue,
}

impl From<&RecordNoRound> for RecordWithRound {
    fn from(r: &RecordNoRound) -> Self {
        RecordWithRound {
            account_id: r.account_id,
            name: r.name.clone(),
            active: r.active,
            f64_value: r.f64_value,
            f32_value: r.f32_value,
            optional_f64: r.optional_f64,
            float_vec: r.float_vec.clone(),
            enum_with_float: r.enum_with_float.clone(),
            tuple_with_float: r.tuple_with_float,
            float_in_map: r.float_in_map.clone(),
            nested: r.nested.clone(),
            json_obj: r.json_obj.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
#[pco_pack(float_round = 2)]
enum Enum {
    Int(i32),
    Float(f64),
}

impl Default for Enum {
    fn default() -> Self {
        Enum::Int(0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, PcoPack, serde::Serialize, serde::Deserialize)]
struct Struct {
    value: f64,
}

fn noise_i32(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x01000193).wrapping_add(i as u32);
    (h ^ (h >> 16)) as i32 & 0xF
}

fn data(n: usize) -> Vec<RecordNoRound> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            let base = (i as f64) * 0.123456789012345;
            let float_in_map = if i == 500 {
                let mut m = BTreeMap::new();
                m.insert("key".to_string(), 500.0);
                m
            } else if i % 10 == 0 {
                let mut m = BTreeMap::new();
                m.insert("key".to_string(), base * 10.0 + 1.0);
                m
            } else {
                BTreeMap::new()
            };
            let f64_value = if i == 4050 { 500.0 } else { base + 0.000000007 };
            RecordNoRound {
                account_id: (i as i64 / 1000) * 1000,
                name: format!("name_{}_{}", i % 5000, n & 0xFF),
                active: (i + n as usize) % 3 != 0,
                f64_value,
                f32_value: (base as f32) + 0.0000001,
                optional_f64: if i % 2 == 0 { Some(base * 1.000000001) } else { None },
                float_vec: (0..((i % 5) + 1)).map(|j| base + j as f64 * 0.000000001).collect(),
                enum_with_float: if i % 3 == 0 { Enum::Float(base * 2.0 + 0.000000003) } else { Enum::Int(n as i32) },
                tuple_with_float: (i as i64, base * 1.5 + 0.000000005),
                float_in_map,
                nested: Struct { value: base * 0.7 + 0.000000009 },
                json_obj: json!({
                    "alpha": base + 0.000000003,
                    "beta": base * 1.5 + 0.000000007,
                    "gamma": base * 0.5 + 0.000000011,
                }),
            }
        })
        .collect()
}
