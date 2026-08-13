include!("bench_common.rs");

fn main() {
    println!("## Numbers\n");

    let mut rows = Vec::new();

    macro_rules! bench_numeric {
        ($label:expr, $gen:expr) => {{
            let (name, rt, pco_size) = bench_roundtrip($label, 100_000, $gen);
            let data = ($gen)(100_000);
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

    bench_numeric!("i8 (0-999)", generate_i8_data);
    bench_numeric!("i16 (0-999)", generate_i16_data);
    bench_numeric!("u8 (0-255)", generate_u8_data);
    bench_numeric!("u16 (0-999)", generate_u16_data);
    bench_numeric!("i32 (full)", generate_i32_data);
    bench_numeric!("i64 (full)", generate_i64_data);
    bench_numeric!("u32 (full)", generate_u32_data);
    bench_numeric!("f32 (normal)", generate_f32_data);
    bench_numeric!("f64 (normal)", generate_f64_data);
    bench_numeric!("i64 (80% zero, sparse)", generate_i64_large_sparse_data);

    print_comparison_table(&rows);
    println!();
}

fn noise_i32(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x01000193).wrapping_add(i as u32);
    (h ^ (h >> 16)) as i32 & 0xF
}

fn generate_i8_data(n: usize) -> Vec<i8> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            ((i % 1000) as i8).wrapping_add(n as i8)
        })
        .collect()
}

fn generate_u8_data(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 256) as u8).collect()
}

fn generate_i16_data(n: usize) -> Vec<i16> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            ((i % 1000) as i16) + (n as i16)
        })
        .collect()
}

fn generate_u16_data(n: usize) -> Vec<u16> {
    (0..n).map(|i| (i % 1000) as u16).collect()
}

fn generate_i32_data(n: usize) -> Vec<i32> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            let h = (i as u32).wrapping_mul(0x9e3779b9);
            ((h ^ (h >> 16)) as i32) + (n as i32)
        })
        .collect()
}

fn generate_u32_data(n: usize) -> Vec<u32> {
    (0..n)
        .map(|i| {
            let h = (i as u64).wrapping_mul(0x9e3779b97f4a7c15);
            (h ^ (h >> 32)) as u32
        })
        .collect()
}

fn generate_i64_data(n: usize) -> Vec<i64> {
    (0..n)
        .map(|i| {
            let h = (i as u64).wrapping_mul(0x9e3779b97f4a7c15);
            (h ^ (h >> 32)) as i64
        })
        .collect()
}

fn generate_f32_data(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let mut sum = 0.0f32;
            for k in 0..12 {
                sum += ((i.wrapping_add(k) as u64).wrapping_mul(0x5bd1e995) as f32) / (u32::MAX as f32);
            }
            sum - 6.0
        })
        .collect()
}

fn generate_f64_data(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let mut sum = 0.0f64;
            for k in 0..12 {
                let val = (i.wrapping_add(k) as u64).wrapping_mul(0x5bd1e9957c5a39f5);
                sum += val as f64 / (u64::MAX as f64);
            }
            sum - 6.0
        })
        .collect()
}

fn generate_i64_large_sparse_data(n: usize) -> Vec<i64> {
    (0..n)
        .map(|i| {
            if i % 5 != 0 {
                0i64
            } else {
                let h = (i as u64).wrapping_mul(0x9e3779b97f4a7c15);
                let bucket = (h ^ (h >> 32)) as u32;
                500_000i64 + (bucket as i64) % 9_500_000
            }
        })
        .collect()
}
