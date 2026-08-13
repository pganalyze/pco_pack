include!("bench_common.rs");
use pco_pack::PcoPack;

fn main() {
    println!("## Others\n");

    let mut rows = Vec::new();

    macro_rules! bench_type {
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

    bench_type!("bool (50% true)", generate_bool_data);
    bench_type!("String", generate_string_data);
    bench_type!("SmolStr", generate_smol_str_data);
    bench_type!("Enum (simple)", generate_status_data);
    bench_type!("Enum (complex)", generate_event_data);
    bench_type!("chrono::DateTime", generate_datetime_data);
    bench_type!("uuid::Uuid", generate_uuid_data);

    print_comparison_table(&rows);
    println!();
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize, PcoPack)]
enum Status {
    #[default]
    Active,
    Inactive,
    Pending,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, PcoPack)]
struct Click {
    x: i32,
    y: i32,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, PcoPack)]
struct KeyPress {
    key: String,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, PcoPack)]
enum Event {
    #[default]
    Unknown,
    Click(Click),
    KeyPress(KeyPress),
    Resize,
}

fn noise_i32(i: usize) -> i32 {
    let h = (i as u32).wrapping_mul(0x01000193).wrapping_add(i as u32);
    (h ^ (h >> 16)) as i32 & 0xF
}

fn generate_bool_data(n: usize) -> Vec<bool> {
    (0..n).map(|i| i % 2 == 0).collect()
}

fn generate_string_data(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            format!("string_{}_{}", i % 5000, n & 0xF)
        })
        .collect()
}

fn generate_smol_str_data(n: usize) -> Vec<smol_str::SmolStr> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            smol_str::SmolStr::from(format!("string_{}_{}", i % 5000, n & 0xF).leak())
        })
        .collect()
}

fn generate_status_data(n: usize) -> Vec<Status> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            match (i + n as usize) % 3 {
                0 => Status::Active,
                1 => Status::Inactive,
                _ => Status::Pending,
            }
        })
        .collect()
}

fn generate_event_data(n: usize) -> Vec<Event> {
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            match (i + n as usize) % 5 {
                0 => Event::Click(Click { x: (i as i32) + n, y: (i * 2) as i32 + n }),
                1 => Event::KeyPress(KeyPress { key: format!("key_{}_{}", i % 100, n & 0xFF) }),
                _ => Event::Resize,
            }
        })
        .collect()
}

fn generate_datetime_data(n: usize) -> Vec<chrono::DateTime<chrono::Utc>> {
    let base = chrono::DateTime::<chrono::Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    (0..n)
        .map(|i| {
            let n = noise_i32(i);
            base + chrono::Duration::seconds((i as i64) * 60 + n as i64)
        })
        .collect()
}

fn generate_uuid_data(n: usize) -> Vec<::uuid::Uuid> {
    (0..n)
        .map(|i| {
            let mut bytes = [0u8; 16];
            bytes[0..8].copy_from_slice(&(i as u64).to_be_bytes());
            bytes[8..12].copy_from_slice(&((i * 7) as u32).to_be_bytes());
            ::uuid::Uuid::from_bytes(bytes)
        })
        .collect()
}
