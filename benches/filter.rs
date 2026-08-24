include!("bench_common.rs");

use markdown_tables::{MarkdownTableRow, as_table};
use pco_pack::PcoPack;
use serde_bytes::ByteBuf;
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Debug)]
struct FilterRow {
    filter: String,
    time_ms: String,
    rows: String,
}

impl MarkdownTableRow for FilterRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Filter", "Time (ms)", "Rows"]
    }

    fn column_values(&self) -> Vec<String> {
        vec![self.filter.clone(), self.time_ms.clone(), self.rows.clone()]
    }
}

fn main() {
    let n_rows = 100_000;
    let data = generate_data(n_rows);
    let mut rows = Vec::new();
    let bytes = AllTypes::serialize(data).unwrap();

    println!("## Filters\n");

    bench_filter(&mut rows, &bytes, "no filter", n_rows, |b| AllTypes::deserialize(b).unwrap().len());
    bench_filter(&mut rows, &bytes, "empty filter", n_rows, |b| {
        AllTypes::filter_bytes(b, json!({}), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "i64 exact (id == 50_000)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"id": 50_000}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "i64 range (50_000..=50_999)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"id": {"start": 50_000, "end": 50_999}}), &[]).unwrap().len()
    });
    let i64_inclusion_values: Vec<_> =
        (0..100).map(|i| json!((100_000 + i) as i64)).chain(std::iter::once(json!(50_000i64))).collect();
    let i64_inclusion_query = json!({"id": i64_inclusion_values});
    bench_filter(&mut rows, &bytes, "i64 inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, i64_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "i32 exact (int32_val == 50)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"int32_val": 50}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "i32 range (50..=50.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"int32_val": {"start": 50.0, "end": 50.99}}), &[]).unwrap().len()
    });
    let i32_inclusion_values: Vec<_> = (0..100).map(|i| json!(100 + i)).chain(std::iter::once(json!(50))).collect();
    let i32_inclusion_query = json!({"int32_val": i32_inclusion_values});
    bench_filter(&mut rows, &bytes, "i32 inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, i32_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "i8 exact (int8_val == 50)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"int8_val": 50}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "i8 range (50..=50.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"int8_val": {"start": 50.0, "end": 50.99}}), &[]).unwrap().len()
    });
    let i8_inclusion_values: Vec<_> = (0..100).map(|i| json!(100 + i)).chain(std::iter::once(json!(50i64))).collect();
    let i8_inclusion_query = json!({"int8_val": i8_inclusion_values});
    bench_filter(&mut rows, &bytes, "i8 inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, i8_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "u8 exact (u8_val == 50)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"u8_val": 50}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "u8 range (50..=50.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"u8_val": {"start": 50.0, "end": 50.99}}), &[]).unwrap().len()
    });
    let u8_inclusion_values: Vec<_> = (0..100).map(|i| json!(100 + i)).chain(std::iter::once(json!(50i64))).collect();
    let u8_inclusion_query = json!({"u8_val": u8_inclusion_values});
    bench_filter(&mut rows, &bytes, "u8 inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, u8_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "f64 exact (float64_val == 50.0)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"float64_val": 50.0}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "f64 range (50.0..=50.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"float64_val": {"start": 50.0, "end": 50.99}}), &[]).unwrap().len()
    });
    let f64_inclusion_values: Vec<_> =
        (0..100).map(|i| json!(10_000.0 + i as f64)).chain(std::iter::once(json!(50.0))).collect();
    let f64_inclusion_query = json!({"float64_val": f64_inclusion_values});
    bench_filter(&mut rows, &bytes, "f64 inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, f64_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "f32 exact (float32_val == 50.0)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"float32_val": 50.0}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "f32 range (50.0..=50.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"float32_val": {"start": 50.0, "end": 50.99}}), &[]).unwrap().len()
    });
    let f32_inclusion_values: Vec<_> =
        (0..100).map(|i| json!(100.0 + i as f64)).chain(std::iter::once(json!(50.0))).collect();
    let f32_inclusion_query = json!({"float32_val": f32_inclusion_values});
    bench_filter(&mut rows, &bytes, "f32 inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, f32_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "f16 exact (f16_val == 50.0)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"f16_val": 50.0}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "f16 range (50.0..=50.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"f16_val": {"start": 50.0, "end": 50.99}}), &[]).unwrap().len()
    });
    let f16_inclusion_values: Vec<_> =
        (0..100).map(|i| json!(100.0 + i as f64)).chain(std::iter::once(json!(50.0))).collect();
    let f16_inclusion_query = json!({"f16_val": f16_inclusion_values});
    bench_filter(&mut rows, &bytes, "f16 inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, f16_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "string exact", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"string_val": "value_50"}), &[]).unwrap().len()
    });
    let string_inclusion: Vec<_> = (0..10).map(|i| json!(format!("value_{}", i))).collect();
    let string_inclusion_query = json!({"string_val": string_inclusion});
    bench_filter(&mut rows, &bytes, "string inclusion (10 values)", 10_000, |b| {
        AllTypes::filter_bytes(b, string_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "bool exact (bool_val == true)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"bool_val": true}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "bool exact (bool_val == false)", n_rows - 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"bool_val": false}), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "enum exact (status == V50)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"status": 50}), &[]).unwrap().len()
    });
    let status_inclusion_values: Vec<_> = (0..10).map(|i| json!(i)).collect();
    let status_inclusion_query = json!({"status": status_inclusion_values});
    bench_filter(&mut rows, &bytes, "enum inclusion (status in 0..9)", 10_000, |b| {
        AllTypes::filter_bytes(b, status_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "option exact (option_val == 50)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"option_val": 50}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "option range (50..=50.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"option_val": {"start": 50.0, "end": 50.99}}), &[]).unwrap().len()
    });
    let option_inclusion_values: Vec<_> = (0..100).map(|i| json!(100 + i)).chain(std::iter::once(json!(50))).collect();
    let option_inclusion_query = json!({"option_val": option_inclusion_values});
    bench_filter(&mut rows, &bytes, "option inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, option_inclusion_query.clone(), &[]).unwrap().len()
    });

    // Vec filters
    bench_filter(&mut rows, &bytes, "vec contains (vec_val has 5000)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"vec_val": 5000}), &[]).unwrap().len()
    });
    let vec_inclusion_values: Vec<_> = (0..10).map(|i| json!(i as i32 * 100)).collect();
    let vec_inclusion_query = json!({"vec_val": vec_inclusion_values});
    bench_filter(&mut rows, &bytes, "vec contains inclusion (any of 10 values)", 10_000, |b| {
        AllTypes::filter_bytes(b, vec_inclusion_query.clone(), &[]).unwrap().len()
    });

    // Hex-encoded bytes filters.
    let bytes_50_hex = "32".repeat(32);
    bench_filter(&mut rows, &bytes, "bytes exact (hex match)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"bytes_val": bytes_50_hex}), &[]).unwrap().len()
    });

    // HashMap filters. Matches key presence only. Dot notation and value matching are not supported.
    bench_filter(&mut rows, &bytes, "map exact (has key 'key_50')", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"map_val": "key_50"}), &[]).unwrap().len()
    });
    let map_key_inclusion: Vec<_> = (0..=9).map(|i| json!(format!("key_{i}"))).collect();
    let map_key_inclusion_query = json!({"map_val": map_key_inclusion});
    bench_filter(&mut rows, &bytes, "map inclusion (any of key_0..key_9)", 10_000, |b| {
        AllTypes::filter_bytes(b, map_key_inclusion_query.clone(), &[]).unwrap().len()
    });

    // JSON filters. Equality matching only.
    bench_filter(&mut rows, &bytes, "json exact (tag_50 string)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"json_val": "tag_50"}), &[]).unwrap().len()
    });

    let uuid_50 = uuid::Uuid::from_fields(50u32 << 24, 0, 0, &[50, 0, 0, 0, 0, 0, 0, 0]);
    bench_filter(&mut rows, &bytes, "uuid exact (bucket 50)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"uuid_val": uuid_50.to_string()}), &[]).unwrap().len()
    });
    let uuid_inclusion_values: Vec<_> = (100..200)
        .map(|i| {
            let u = uuid::Uuid::from_fields((i as u32) << 24, 0, 0, &[i as u8, 0, 0, 0, 0, 0, 0, 0]);
            json!(u.to_string())
        })
        .chain(std::iter::once(json!(uuid_50.to_string())))
        .collect();
    let uuid_inclusion_query = json!({"uuid_val": uuid_inclusion_values});
    bench_filter(&mut rows, &bytes, "uuid inclusion (100 values, 1 match)", 1_000, |b| {
        AllTypes::filter_bytes(b, uuid_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "nested (nested.inner_id == 50_000)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"nested.inner_id": 50_000}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "nested (nested.inner_id range 50_000..=50_000.99)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"nested.inner_id": {"start": 50_000.0, "end": 50_000.99}}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "nested (nested.inner_name exact)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"nested.inner_name": "inner_50"}), &[]).unwrap().len()
    });
    let nested_name_inclusion: Vec<_> = (0..10).map(|i| json!(format!("inner_{}", i))).collect();
    let nested_name_inclusion_query = json!({"nested.inner_name": nested_name_inclusion});
    bench_filter(&mut rows, &bytes, "nested (nested.inner_name inclusion, 10 values)", 10_000, |b| {
        AllTypes::filter_bytes(b, nested_name_inclusion_query.clone(), &[]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "partial fields (id + string_val)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"id": 50_000}), &["id", "string_val"]).unwrap().len()
    });

    bench_filter(&mut rows, &bytes, "multi-field (id + int32_val)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"id": 50_000, "int32_val": 50}), &[]).unwrap().len()
    });

    // These pair a lazy-deserialized column with u8_val, which compresses smallest and so is
    // filtered first. Filters on dynamically sized types are able to skip deserializing entire
    // chunks of 128 values if previous filters didn't find any matches.
    bench_filter(&mut rows, &bytes, "multi-field (bytes_val + u8_val)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"bytes_val": bytes_50_hex, "u8_val": 50}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "multi-field (string_val + u8_val)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"string_val": "value_50", "u8_val": 50}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "multi-field (json_val + u8_val)", 1_000, |b| {
        AllTypes::filter_bytes(b, json!({"json_val": "tag_50", "u8_val": 50}), &[]).unwrap().len()
    });
    bench_filter(&mut rows, &bytes, "multi-field zero-match (id + int32_val + string_val)", 0, |b| {
        AllTypes::filter_bytes(b, json!({"id": 50_000, "int32_val": 51, "string_val": "value_0"}), &[]).unwrap().len()
    });

    println!("{}", as_table(&rows));
    println!();
}

fn bench_filter<F>(rows: &mut Vec<FilterRow>, bytes: &[u8], label: &str, expected_rows: usize, filter_fn: F) -> f64
where
    F: Fn(&[u8]) -> usize,
{
    let actual = filter_fn(bytes);
    let tolerance = (expected_rows as f64 * 0.1).max(1.0) as usize;
    assert!(
        (actual as isize - expected_rows as isize).abs() <= tolerance as isize,
        "{}: expected ~{} rows (+/-{}), got {}",
        label,
        expected_rows,
        tolerance,
        actual
    );
    let ms = avg_ms(|| {
        let result = filter_fn(bytes);
        black_box(result);
    });
    rows.push(FilterRow { filter: label.to_string(), time_ms: format_ms(ms), rows: format!("{actual}") });
    ms
}

// Generate data where each filterable field has ~100 unique values, each appearing ~1,000 times.
// This normalizes deserialization cost across filter benchmarks.
fn generate_data(n: usize) -> Vec<AllTypes> {
    (0..n)
        .map(|i| {
            let bucket = (i / 1000) % 100;
            AllTypes {
                id: (bucket as i64) * 1000,
                int8_val: bucket as i8,
                int32_val: bucket as i32,
                u8_val: bucket as u8,
                float32_val: bucket as f32,
                float64_val: bucket as f64,
                f16_val: half::f16::from_f64(bucket as f64),
                bool_val: i % 100 == 0,
                string_val: format!("value_{}", bucket),
                status: Status::from_discriminant(bucket as u16),
                option_val: Some(bucket as i32),
                vec_val: vec![bucket as i32 * 100],
                uuid_val: uuid::Uuid::from_fields((bucket as u32) << 24, 0, 0, &[bucket as u8, 0, 0, 0, 0, 0, 0, 0]),
                bytes_val: ByteBuf::from(vec![bucket as u8; 32]),
                nested: NestedStruct { inner_id: (bucket as i64) * 1000, inner_name: format!("inner_{}", bucket) },
                map_val: {
                    let mut m = HashMap::new();
                    m.insert(format!("key_{}", bucket), bucket as i32);
                    // extra_key exists in buckets 10..=99, value is (bucket-10)*2
                    if bucket >= 10 {
                        m.insert("extra_key".to_string(), ((bucket - 10) * 2) as i32);
                    }
                    m
                },
                json_val: json!(format!("tag_{}", bucket)),
            }
        })
        .collect()
}

#[derive(Clone, PartialEq, Default, PcoPack)]
struct NestedStruct {
    inner_id: i64,
    inner_name: String,
}

#[derive(Clone, PartialEq, Default, PcoPack)]
// Note: chunk_size is changed so all data lives in a single chunk,
// forcing all data to be deserialized/skipped in all filter scenarios.
#[pco_pack(chunk_size = 100_000)]
struct AllTypes {
    id: i64,
    int8_val: i8,
    int32_val: i32,
    u8_val: u8,
    float32_val: f32,
    float64_val: f64,
    f16_val: half::f16,
    bool_val: bool,
    string_val: String,
    status: Status,
    option_val: Option<i32>,
    vec_val: Vec<i32>,
    uuid_val: uuid::Uuid,
    bytes_val: ByteBuf,
    nested: NestedStruct,
    map_val: HashMap<String, i32>,
    json_val: Value,
}

/// 100 variants so each discriminant appears ~1,000 times in 100k rows.
#[derive(Clone, Copy, PartialEq, Default, PcoPack)]
#[repr(u16)]
enum Status {
    #[default]
    V00 = 0,
    V01 = 1,
    V02 = 2,
    V03 = 3,
    V04 = 4,
    V05 = 5,
    V06 = 6,
    V07 = 7,
    V08 = 8,
    V09 = 9,
    V10 = 10,
    V11 = 11,
    V12 = 12,
    V13 = 13,
    V14 = 14,
    V15 = 15,
    V16 = 16,
    V17 = 17,
    V18 = 18,
    V19 = 19,
    V20 = 20,
    V21 = 21,
    V22 = 22,
    V23 = 23,
    V24 = 24,
    V25 = 25,
    V26 = 26,
    V27 = 27,
    V28 = 28,
    V29 = 29,
    V30 = 30,
    V31 = 31,
    V32 = 32,
    V33 = 33,
    V34 = 34,
    V35 = 35,
    V36 = 36,
    V37 = 37,
    V38 = 38,
    V39 = 39,
    V40 = 40,
    V41 = 41,
    V42 = 42,
    V43 = 43,
    V44 = 44,
    V45 = 45,
    V46 = 46,
    V47 = 47,
    V48 = 48,
    V49 = 49,
    V50 = 50,
    V51 = 51,
    V52 = 52,
    V53 = 53,
    V54 = 54,
    V55 = 55,
    V56 = 56,
    V57 = 57,
    V58 = 58,
    V59 = 59,
    V60 = 60,
    V61 = 61,
    V62 = 62,
    V63 = 63,
    V64 = 64,
    V65 = 65,
    V66 = 66,
    V67 = 67,
    V68 = 68,
    V69 = 69,
    V70 = 70,
    V71 = 71,
    V72 = 72,
    V73 = 73,
    V74 = 74,
    V75 = 75,
    V76 = 76,
    V77 = 77,
    V78 = 78,
    V79 = 79,
    V80 = 80,
    V81 = 81,
    V82 = 82,
    V83 = 83,
    V84 = 84,
    V85 = 85,
    V86 = 86,
    V87 = 87,
    V88 = 88,
    V89 = 89,
    V90 = 90,
    V91 = 91,
    V92 = 92,
    V93 = 93,
    V94 = 94,
    V95 = 95,
    V96 = 96,
    V97 = 97,
    V98 = 98,
    V99 = 99,
}

impl Status {
    fn from_discriminant(d: u16) -> Self {
        match d {
            0 => Status::V00,
            1 => Status::V01,
            2 => Status::V02,
            3 => Status::V03,
            4 => Status::V04,
            5 => Status::V05,
            6 => Status::V06,
            7 => Status::V07,
            8 => Status::V08,
            9 => Status::V09,
            10 => Status::V10,
            11 => Status::V11,
            12 => Status::V12,
            13 => Status::V13,
            14 => Status::V14,
            15 => Status::V15,
            16 => Status::V16,
            17 => Status::V17,
            18 => Status::V18,
            19 => Status::V19,
            20 => Status::V20,
            21 => Status::V21,
            22 => Status::V22,
            23 => Status::V23,
            24 => Status::V24,
            25 => Status::V25,
            26 => Status::V26,
            27 => Status::V27,
            28 => Status::V28,
            29 => Status::V29,
            30 => Status::V30,
            31 => Status::V31,
            32 => Status::V32,
            33 => Status::V33,
            34 => Status::V34,
            35 => Status::V35,
            36 => Status::V36,
            37 => Status::V37,
            38 => Status::V38,
            39 => Status::V39,
            40 => Status::V40,
            41 => Status::V41,
            42 => Status::V42,
            43 => Status::V43,
            44 => Status::V44,
            45 => Status::V45,
            46 => Status::V46,
            47 => Status::V47,
            48 => Status::V48,
            49 => Status::V49,
            50 => Status::V50,
            51 => Status::V51,
            52 => Status::V52,
            53 => Status::V53,
            54 => Status::V54,
            55 => Status::V55,
            56 => Status::V56,
            57 => Status::V57,
            58 => Status::V58,
            59 => Status::V59,
            60 => Status::V60,
            61 => Status::V61,
            62 => Status::V62,
            63 => Status::V63,
            64 => Status::V64,
            65 => Status::V65,
            66 => Status::V66,
            67 => Status::V67,
            68 => Status::V68,
            69 => Status::V69,
            70 => Status::V70,
            71 => Status::V71,
            72 => Status::V72,
            73 => Status::V73,
            74 => Status::V74,
            75 => Status::V75,
            76 => Status::V76,
            77 => Status::V77,
            78 => Status::V78,
            79 => Status::V79,
            80 => Status::V80,
            81 => Status::V81,
            82 => Status::V82,
            83 => Status::V83,
            84 => Status::V84,
            85 => Status::V85,
            86 => Status::V86,
            87 => Status::V87,
            88 => Status::V88,
            89 => Status::V89,
            90 => Status::V90,
            91 => Status::V91,
            92 => Status::V92,
            93 => Status::V93,
            94 => Status::V94,
            95 => Status::V95,
            96 => Status::V96,
            97 => Status::V97,
            98 => Status::V98,
            99 => Status::V99,
            _ => panic!("invalid Status discriminant: {}", d),
        }
    }
}
