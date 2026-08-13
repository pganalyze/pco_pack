include!("bench_common.rs");
use serde_bytes::ByteBuf;
use std::collections::{BTreeMap, HashMap};

fn main() {
    println!("## Collections\n");

    let mut rows = Vec::new();

    macro_rules! bench_collection {
        ($label:expr, $n:expr, $gen:expr) => {{
            let (name, rt, pco_size) = bench_roundtrip($label, $n, $gen);
            let data = ($gen)($n);
            let mp_size = to_msgpack_zstd(&data).len();
            let mp_rt = bench_msgpack(&data);
            drop(data);
            let time_ratio = mp_rt / rt;
            let size_ratio = mp_size as f64 / pco_size as f64;
            rows.push(ComparisonRow {
                type_name: format!("`{}`", name),
                pco_time: format_ms(rt),
                mp_time: format_ms(mp_rt),
                time_ratio: format!("{:.2}x", time_ratio),
                pco_size: format_bytes(pco_size),
                mp_size: format_bytes(mp_size),
                size_ratio: format!("{:.1}x", size_ratio),
            });
        }};
    }

    bench_collection!("Vec<i32> (avg 4)", 50_000, generate_vec_i32_data);
    bench_collection!("Vec<String> (avg 4)", 50_000, generate_vec_string_data);

    bench_collection!("Option<i32> (50% null)", 100_000, generate_option_i32_data);
    bench_collection!("i32 (50% zero as sentinel)", 100_000, generate_i32_zero_none_data);

    bench_collection!("ByteBuf (avg 128B)", 100_000, generate_bytebuf_data);

    bench_collection!("serde_json::Value", 100_000, generate_json_value_data);

    bench_collection!("BTreeMap<(String, i32), i32>", 10_000, generate_btremap_string_data);

    {
        let (name, rt, pco_size) =
            bench_roundtrip("BTreeMap<(SmolStr, i32), i32>", 10_000, generate_btremap_smolstr_data);
        let smolstr_data = generate_btremap_smolstr_data(10_000);
        let string_data: Vec<BTreeMap<(String, i32), i32>> = smolstr_data
            .iter()
            .map(|m| m.iter().map(|((k, v), val)| ((k.as_ref().to_string(), *v), *val)).collect())
            .collect();
        let mp_size = to_msgpack_zstd(&string_data).len();
        let mp_rt = avg_ms(|| {
            let data = generate_btremap_smolstr_data(10_000);
            let data: Vec<BTreeMap<(String, i32), i32>> = data
                .iter()
                .map(|m| m.iter().map(|((k, v), val)| ((k.as_ref().to_string(), *v), *val)).collect())
                .collect();
            let buf = to_msgpack_zstd(&data);
            let decompressed = zstd::decode_all(&buf[..]).unwrap();
            let cursor = std::io::Cursor::new(decompressed);
            let _: Vec<BTreeMap<(String, i32), i32>> = rmp_serde::from_read(cursor).unwrap();
            std::hint::black_box(())
        });
        drop(smolstr_data);
        let time_ratio = mp_rt / rt;
        let size_ratio = mp_size as f64 / pco_size as f64;
        rows.push(ComparisonRow {
            type_name: format!("`{}`", name),
            pco_time: format_ms(rt),
            mp_time: format_ms(mp_rt),
            time_ratio: format!("{:.2}x", time_ratio),
            pco_size: format_bytes(pco_size),
            mp_size: format_bytes(mp_size),
            size_ratio: format!("{:.1}x", size_ratio),
        });
    }

    bench_collection!("HashMap<(String, i32), i32>", 10_000, generate_hashmap_string_data);

    {
        let (name, rt, pco_size) =
            bench_roundtrip("HashMap<(SmolStr, i32), i32>", 10_000, generate_hashmap_smolstr_data);
        let smolstr_data = generate_hashmap_smolstr_data(10_000);
        let string_data: Vec<HashMap<(String, i32), i32>> = smolstr_data
            .iter()
            .map(|m| m.iter().map(|((k, v), val)| ((k.as_ref().to_string(), *v), *val)).collect())
            .collect();
        let mp_size = to_msgpack_zstd(&string_data).len();
        let mp_rt = avg_ms(|| {
            let data = generate_hashmap_smolstr_data(10_000);
            let data: Vec<HashMap<(String, i32), i32>> = data
                .iter()
                .map(|m| m.iter().map(|((k, v), val)| ((k.as_ref().to_string(), *v), *val)).collect())
                .collect();
            let buf = to_msgpack_zstd(&data);
            let decompressed = zstd::decode_all(&buf[..]).unwrap();
            let cursor = std::io::Cursor::new(decompressed);
            let _: Vec<HashMap<(String, i32), i32>> = rmp_serde::from_read(cursor).unwrap();
            std::hint::black_box(())
        });
        drop(smolstr_data);
        let time_ratio = mp_rt / rt;
        let size_ratio = mp_size as f64 / pco_size as f64;
        rows.push(ComparisonRow {
            type_name: format!("`{}`", name),
            pco_time: format_ms(rt),
            mp_time: format_ms(mp_rt),
            time_ratio: format!("{:.2}x", time_ratio),
            pco_size: format_bytes(pco_size),
            mp_size: format_bytes(mp_size),
            size_ratio: format!("{:.1}x", size_ratio),
        });
    }

    print_comparison_table(&rows);
    println!();
}

fn noise_i32(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x01000193).wrapping_add(i as u32);
    (h ^ (h >> 16)) as i32 & 0xF
}

fn generate_vec_i32_data(n: usize) -> Vec<Vec<i32>> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            let len = (i % 8) + 1;
            (0..len).map(|j| (i * 100 + j) as i32 + n).collect()
        })
        .collect()
}

fn generate_vec_string_data(n: usize) -> Vec<Vec<String>> {
    (0..n)
        .map(|i| {
            let len = (i % 8) + 1;
            (0..len).map(|j| format!("item_{}_{}", i, j)).collect()
        })
        .collect()
}

fn generate_option_i32_data(n: usize) -> Vec<Option<i32>> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            if i % 2 == 0 { Some((i as i32) + n) } else { None }
        })
        .collect()
}

fn generate_i32_zero_none_data(n: usize) -> Vec<i32> {
    // Same distribution as Option<i32>: even indices get values, odd indices are 0
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            if i % 2 == 0 { (i as i32) + n } else { 0 }
        })
        .collect()
}

fn generate_btremap_string_data(n: usize) -> Vec<BTreeMap<(String, i32), i32>> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            let mut map = BTreeMap::new();
            for j in 0..((i % 5) + 1) {
                map.insert((format!("key_{}_{}", j, noise_i32(j) & 0xF), (i as i32) + n), (i * 10 + j) as i32 + n);
            }
            map
        })
        .collect()
}

fn generate_hashmap_string_data(n: usize) -> Vec<HashMap<(String, i32), i32>> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            let mut map = HashMap::new();
            for j in 0..((i % 5) + 1) {
                map.insert((format!("key_{}_{}", j, noise_i32(j) & 0xF), (i as i32) + n), (i * 10 + j) as i32 + n);
            }
            map
        })
        .collect()
}

fn generate_btremap_smolstr_data(n: usize) -> Vec<BTreeMap<(smol_str::SmolStr, i32), i32>> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            let mut map = BTreeMap::new();
            for j in 0..((i % 5) + 1) {
                map.insert(
                    (smol_str::SmolStr::from(format!("key_{}_{}", j, noise_i32(j) & 0xF).leak()), (i as i32) + n),
                    (i * 10 + j) as i32 + n,
                );
            }
            map
        })
        .collect()
}

fn generate_hashmap_smolstr_data(n: usize) -> Vec<HashMap<(smol_str::SmolStr, i32), i32>> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            let mut map = HashMap::new();
            for j in 0..((i % 5) + 1) {
                map.insert(
                    (smol_str::SmolStr::from(format!("key_{}_{}", j, noise_i32(j) & 0xF).leak()), (i as i32) + n),
                    (i * 10 + j) as i32 + n,
                );
            }
            map
        })
        .collect()
}

fn generate_bytebuf_data(n: usize) -> Vec<ByteBuf> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            ByteBuf::from(vec![n as u8; (i % 256) + 1])
        })
        .collect()
}

fn generate_json_value_data(n: usize) -> Vec<serde_json::Value> {
    use serde_json::json;
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            match i % 5 {
                0 => json!({"id": i, "name": format!("name_{}", i), "score": (i as f64) / 100.0}),
                1 => json!({"tags": vec!["a", "b", "c"], "count": (i % 10) as i64 + n as i64}),
                2 => json!(format!("value_{}_{}", i, n & 0xFF)),
                3 => json!(null),
                _ => json!((i as f64) + (n as f64) / 10.0),
            }
        })
        .collect()
}
