use std::hint::black_box;
use std::io::Cursor;
use std::time::Instant;

pub const ITERATIONS: usize = 50;

/// Benchmark a closure and return average time in milliseconds (rounded to 1 decimal).
pub fn avg_ms<T, F: Fn() -> T>(f: F) -> f64 {
    let mut total_ns: u128 = 0;
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let _ = black_box(f());
        total_ns += start.elapsed().as_nanos();
    }
    ((total_ns / ITERATIONS as u128) as f64 / 1_000_000.0 * 10.0).round() / 10.0
}

/// Benchmark a closure, returning the average time in ms and its last result.
pub fn avg_ms_pair<T, F: Fn() -> T>(f: F) -> (f64, T) {
    let mut total_ns: u128 = 0;
    let result = black_box(f());
    for _ in 1..ITERATIONS {
        let start = Instant::now();
        black_box(f());
        total_ns += start.elapsed().as_nanos();
    }
    (((total_ns / (ITERATIONS as u128 - 1)) as f64 / 1_000_000.0 * 10.0).round() / 10.0, result)
}

/// Format bytes into a human-readable string (B, KB, MB, GB).
pub fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

/// Format milliseconds with 1 decimal precision (rounded). Always outputs ms.
pub fn format_ms(ms: f64) -> String {
    format!("{:.1} ms", ((ms * 10.0).round() / 10.0))
}

/// Serialize data to msgpack and compress with zstd (level 3).
pub fn to_msgpack_zstd<T: serde::Serialize>(data: &[T]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut ser = rmp_serde::Serializer::new(&mut buf);
    serde::Serialize::serialize(&data, &mut ser).expect("msgpack failed");
    zstd::encode_all(&*buf, 3).unwrap()
}

/// Benchmark PcoPack roundtrip (write + read all rows) for a PcoSerde type.
/// Returns (label, deserialization time in ms, serialized size in bytes).
pub fn bench_roundtrip<T: pco_pack::PcoSerde + Clone + std::fmt::Debug, F: Fn(usize) -> Vec<T>>(
    label: &str, n_rows: usize, build: F,
) -> (String, f64, usize) {
    let data = build(n_rows);
    let buf = T::write(data.clone(), 0, Default::default()).unwrap();
    let size = buf.len();
    let ms = avg_ms(|| {
        let mut cursor = Cursor::new(&buf[..]);
        let mut reader = T::read(&mut cursor, 0, Default::default()).unwrap();
        let mut result = Vec::with_capacity(data.len());
        for row_idx in 0..data.len() {
            result.push(T::get(&mut reader, row_idx));
        }
        black_box(result)
    });
    (label.to_string(), ms, size)
}

/// Benchmark msgpack roundtrip on a Serialize+Deserialize type. Returns time in ms.
pub fn bench_msgpack<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(data: &[T]) -> f64 {
    let buf = to_msgpack_zstd(data);
    let decompressed = zstd::decode_all(&buf[..]).unwrap();
    match rmp_serde::from_read::<_, Vec<T>>(Cursor::new(decompressed.clone())) {
        Ok(_result) => {
            // eprintln!("bench_msgpack: deserialization OK ({})", result.len());
        }
        Err(e) => {
            eprintln!(
                "bench_msgpack: FAILED to deserialize {} items of type `{}`: {:?}",
                data.len(),
                std::any::type_name::<T>(),
                e
            );
        }
    }
    avg_ms(|| {
        let decompressed = zstd::decode_all(&buf[..]).unwrap();
        let cursor = Cursor::new(decompressed);
        black_box(rmp_serde::from_read::<_, Vec<T>>(cursor).unwrap());
    })
}

/// Row for comparing PcoPack vs msgpack (Type, time, size columns).
pub struct ComparisonRow {
    pub type_name: String,
    pub pco_time: String,
    pub mp_time: String,
    pub time_ratio: String,
    pub pco_size: String,
    pub mp_size: String,
    pub size_ratio: String,
}

impl markdown_tables::MarkdownTableRow for ComparisonRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Type", "PcoPack", "msgpack", "Time ratio", "PcoPack", "msgpack", "Size ratio"]
    }

    fn column_values(&self) -> Vec<String> {
        vec![
            self.type_name.clone(),
            self.pco_time.clone(),
            self.mp_time.clone(),
            self.time_ratio.clone(),
            self.pco_size.clone(),
            self.mp_size.clone(),
            self.size_ratio.clone(),
        ]
    }
}

pub fn print_comparison_table(rows: &[ComparisonRow]) {
    if !rows.is_empty() {
        println!("{}", markdown_tables::as_table(rows));
    }
}

/// Row for struct benchmarks (Metric, PcoPack, msgpack, Ratio).
pub struct StructRow {
    pub metric: String,
    pub pco_val: String,
    pub mp_val: String,
    pub ratio: String,
}

impl markdown_tables::MarkdownTableRow for StructRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Metric", "PcoPack", "msgpack", "Ratio"]
    }

    fn column_values(&self) -> Vec<String> {
        vec![self.metric.clone(), self.pco_val.clone(), self.mp_val.clone(), self.ratio.clone()]
    }
}

pub fn print_struct_table(rows: &[StructRow]) {
    if !rows.is_empty() {
        println!("{}", markdown_tables::as_table(rows));
    }
}
