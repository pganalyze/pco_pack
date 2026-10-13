use std::hint::black_box;
use std::io::Cursor;
use std::time::Instant;

pub const ITERATIONS: usize = 50;

/// pco_pack's rows per chunk; every codec (pco, PcoPack, msgpack+zstd) is
/// measured at this size for a fair comparison.
pub const CHUNK_ROWS: usize = 262_144;

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
    } else if bytes >= 2 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a duration; switches to seconds at or above 1000 ms, mirroring
/// `format_bytes` (big ms values are noisy).
pub fn format_ms(ms: f64) -> String {
    if ms >= 1000.0 {
        format!("{:.1} s", (ms / 1000.0 * 10.0).round() / 10.0)
    } else {
        format!("{:.1} ms", ((ms * 10.0).round() / 10.0))
    }
}

// Shared distribution primitives: each maps the row index through splitmix64.

/// splitmix64 hash: the seed for all generators.
pub fn splitmix64(i: usize) -> u64 {
    let mut z = (i as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform [0, 1) derived from a row index (53 bits of precision).
pub fn unit64(i: usize) -> f64 {
    (splitmix64(i) >> 11) as f64 / (1u64 << 53) as f64
}

/// Bucket a uniform draw falls into under a CDF (`cdf[i]` = cumulative
/// probability through `i`).
pub fn cdf_index(cdf: &[f64], r: f64) -> usize {
    match cdf.binary_search_by(|c| c.total_cmp(&r)) {
        Ok(i) => i,
        Err(i) => i.min(cdf.len() - 1),
    }
}

/// Right-skewed counter: ~64% in 1..22 (j^-2, p50 = 2), rest a Pareto tail.
pub fn skew_small(i: usize) -> i64 {
    const SPIKE_CDF: [f64; 22] = [
        0.6248, 0.7810, 0.8504, 0.8895, 0.9145, 0.9318, 0.9446, 0.9543, 0.9620, 0.9683, 0.9735, 0.9778, 0.9815, 0.9847,
        0.9875, 0.9899, 0.9921, 0.9940, 0.9957, 0.9973, 0.9987, 1.0,
    ];
    if unit64(i) < 0.64 {
        1 + cdf_index(&SPIKE_CDF, unit64(i.wrapping_add(0x5eed))) as i64
    } else {
        let u = unit64(i.wrapping_add(0xbeef));
        (10.0 * (1.0 - u).powf(-1.0 / 1.02)).min(3_362_520.0) as i64
    }
}

/// Positive log-normal values (exponentiated CLT normal): realistic
/// score/latency shape, p50 ~10, heavy right tail.
pub fn lognorm_small(i: usize) -> f64 {
    let mut s = 0.0f64;
    for k in 0..12 {
        s += unit64(i.wrapping_add(k * 7919));
    }
    s = (s - 6.0) * 1.1;
    s.exp() * 10.0
}

/// Repeating-pattern column: stable 12-slot snapshots of variable length
/// with occasional drift; repetitions feed zstd back-references.
pub fn gen_repeating<T: Copy>(n: usize, pattern: &[T], values: &[T], counts: &[f64]) -> Vec<T> {
    let total: f64 = counts.iter().sum();
    let mut cdf = Vec::with_capacity(counts.len());
    let mut acc = 0.0;
    for c in counts {
        acc += c / total;
        cdf.push(acc);
    }
    let mut slots: Vec<T> = pattern.to_vec();
    let mut out = Vec::with_capacity(n);
    let mut snap = 0usize;
    while out.len() < n {
        let len = 10 + (splitmix64(snap.wrapping_mul(2_654_435_761)) % 5) as usize;
        // Drift: 5% of snapshots redraw 1-2 slots from the measured marginal.
        if unit64(snap.wrapping_add(0xbeef)) < 0.05 {
            let k = 1 + (splitmix64(snap.wrapping_add(0x1234)) % 2) as usize;
            for j in 0..k {
                let s = (splitmix64(snap.wrapping_mul(7919).wrapping_add(j)) % 12) as usize;
                slots[s] = values[cdf_index(&cdf, unit64(snap.wrapping_mul(104_729).wrapping_add(s)))];
            }
        }
        // Random phase so consecutive snapshots don't start at the same slot;
        // rows share no aligned repetition, matching the real columns.
        let phase = (splitmix64(snap.wrapping_mul(31)) % 12) as usize;
        for k in 0..len {
            if out.len() == n {
                return out;
            }
            out.push(slots[(k + phase) % 12]);
        }
        snap += 1;
    }
    out
}

// Codecs

/// msgpack (rmp_serde) + zstd level 3, one zstd frame per `CHUNK_ROWS` chunk.
pub fn to_msgpack_zstd<T: serde::Serialize>(data: &[T]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(CHUNK_ROWS) {
        let mut buf = Vec::new();
        let mut ser = rmp_serde::Serializer::new(&mut buf);
        serde::Serialize::serialize(chunk, &mut ser).expect("msgpack failed");
        out.extend_from_slice(&zstd::encode_all(&*buf, 3).unwrap());
    }
    out
}

/// PcoPack write time (ms) and size (bytes) for a PcoSerde type; verifies a
/// full sequential roundtrip (skipping rows corrupts the forward-only reader).
pub fn bench_write<T: pco_pack::PcoSerde + Clone + PartialEq + std::fmt::Debug, F: Fn(usize) -> Vec<T>>(
    label: &str, n_rows: usize, build: F,
) -> (String, f64, usize) {
    let data = build(n_rows);
    let (ms, buf) = avg_ms_pair(|| T::write(data.clone(), 0, Default::default()).unwrap());
    let mut cursor = Cursor::new(&buf[..]);
    let mut reader = T::read(&mut cursor, 0, Default::default()).unwrap();
    for idx in 0..n_rows {
        assert_eq!(T::get(&mut reader, idx).unwrap(), Some(data[idx].clone()), "roundtrip mismatch at row {idx}");
    }
    (label.to_string(), ms, buf.len())
}

/// [`bench_write`] for lossy knobs (`float_round`/`time_round`); the roundtrip
/// compares against `expect(&original)`, the rounded value stored.
pub fn bench_write_round<
    T: pco_pack::PcoSerde + Clone + PartialEq + std::fmt::Debug,
    F: Fn(usize) -> Vec<T>,
    E: Fn(&T) -> T,
>(
    label: &str, n_rows: usize, build: F, float_round: u32, time_round: chrono::Duration, expect: E,
) -> (String, f64, usize) {
    let data = build(n_rows);
    let (ms, buf) = avg_ms_pair(|| T::write(data.clone(), float_round, time_round).unwrap());
    let mut cursor = Cursor::new(&buf[..]);
    let mut reader = T::read(&mut cursor, float_round, time_round).unwrap();
    for idx in 0..n_rows {
        assert_eq!(T::get(&mut reader, idx).unwrap(), Some(expect(&data[idx])), "roundtrip mismatch at row {idx}");
    }
    (label.to_string(), ms, buf.len())
}

/// msgpack+zstd write time in ms; verifies a single-frame decode roundtrip
/// once.
pub fn bench_msgpack<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(data: &[T]) -> f64 {
    let mut buf = Vec::new();
    let mut ser = rmp_serde::Serializer::new(&mut buf);
    serde::Serialize::serialize(data, &mut ser).expect("msgpack failed");
    let z = zstd::encode_all(&*buf, 3).unwrap();
    let decompressed = zstd::decode_all(&z[..]).unwrap();
    let back: Vec<T> = rmp_serde::from_read(Cursor::new(decompressed)).expect("msgpack decode failed");
    assert_eq!(back.len(), data.len());
    avg_ms(|| to_msgpack_zstd(data))
}

/// Standalone pco (`ChunkConfig::default()`, pco's own chunking) on a flat
/// numeric column; returns (write ms, size bytes).
pub fn bench_pco_col<T: pco::data_types::Number>(data: &[T]) -> (f64, usize) {
    let (ms, buf) = avg_ms_pair(|| pco::standalone::simple_compress(data, &pco::ChunkConfig::default()).unwrap());
    (ms, buf.len())
}

/// Standalone pco with 8-bit types enabled, the only config accepting
/// `u8`/`i8`; PcoPack enables them by default.
pub fn bench_pco_col_8bit<T: pco::data_types::Number>(data: &[T]) -> (f64, usize) {
    let cfg = pco::ChunkConfig::default().with_enable_8_bit(true);
    let (ms, buf) = avg_ms_pair(|| pco::standalone::simple_compress(data, &cfg).unwrap());
    (ms, buf.len())
}

/// Baseline for dynamic types (strings, byte arrays, JSON, maps): pco has
/// no serde API for them, so they are stored as msgpack+zstd.
#[allow(dead_code)]
pub fn bench_pco_fallback<T: serde::Serialize>(data: &[T]) -> (f64, usize) {
    let (ms, buf) = avg_ms_pair(|| to_msgpack_zstd(data));
    (ms, buf.len())
}

// Vortex, the columnar competitor, via serde_arrow: one compact file per
// column, values wrapped in `Item`.

#[allow(dead_code)] // only some benches measure Vortex
fn vortex_rt() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Builder::new_current_thread().build().unwrap())
}

/// Build the session inside `block_on` so vortex picks up the ambient tokio
/// runtime.
#[allow(dead_code)]
fn vortex_session() -> &'static vortex::session::VortexSession {
    static SESSION: std::sync::OnceLock<vortex::session::VortexSession> = std::sync::OnceLock::new();
    SESSION.get_or_init(|| {
        use vortex::VortexSessionDefault as _;
        vortex_rt().block_on(async { vortex::session::VortexSession::default() })
    })
}

/// Write one compact vortex file per column of `T`, verify a full
/// value-level roundtrip; `Err` for types the pipeline can't express. Maps
/// follow serde_arrow's default struct-promoting tracing; open-vocabulary
/// rows declare the native layout via [`bench_vortex_native_map`]. Declaring
/// it in the table, not error recovery, makes an upstream tracing change a
/// "-" cell or a deliberate edit, not silent drift.
#[allow(dead_code)]
pub fn bench_vortex<T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + PartialEq + std::fmt::Debug>(
    data: &[T],
) -> Result<(f64, usize), String> {
    bench_vortex_map_mode(data, true)
}

/// [`bench_vortex`] storing maps as native `vortex.map`
/// (`map_as_struct(false)`); struct promotion silently drops entries whose
/// key misses the sample-derived schema.
#[allow(dead_code)]
pub fn bench_vortex_native_map<
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + PartialEq + std::fmt::Debug,
>(
    data: &[T],
) -> Result<(f64, usize), String> {
    bench_vortex_map_mode(data, false)
}

/// [`bench_vortex`] / [`bench_vortex_native_map`] with the serde_arrow map
/// mode explicit: `map_as_struct=true` promotes stable-key maps to structs,
/// moving key names into the schema; `false` keeps the native arrow `Map`,
/// stored by vortex as `vortex.map`.
#[allow(dead_code)]
pub fn bench_vortex_map_mode<
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + PartialEq + std::fmt::Debug,
>(
    data: &[T], map_as_struct: bool,
) -> Result<(f64, usize), String> {
    use arrow_schema::FieldRef;
    use serde_arrow::schema::{SchemaLike, TracingOptions};
    use serde_arrow::utils::{Item, Items};
    use vortex::array::stream::ArrayStreamExt;
    use vortex::arrow::ArrowSessionExt as _;
    use vortex::buffer::ByteBuffer;
    use vortex::compressor::BtrBlocksCompressorBuilder;
    use vortex::file::{OpenOptionsSessionExt, WriteOptionsSessionExt, WriteStrategyBuilder};

    let rt = vortex_rt();
    let session = vortex_session();
    // Unsupported types panic inside the pipeline; silence the hook so a
    // caught panic prints nothing.
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Schema: `from_type` for self-describing types, else `from_samples`;
        // unit enums via `enums_without_data_as_strings`.
        let options = TracingOptions::default()
            .enums_without_data_as_strings(true)
            .allow_null_fields(true)
            .map_as_struct(map_as_struct);
        let fields: Vec<FieldRef> = match <Vec<FieldRef>>::from_type::<Item<T>>(options.clone()) {
            Ok(f) => f,
            Err(_) => {
                let samples: Vec<T> = data.iter().take(100).cloned().collect();
                match <Vec<FieldRef>>::from_samples(Items(samples), options) {
                    Ok(f) => f,
                    Err(e) => return Err(e.to_string()),
                }
            }
        };

        let items: Vec<Item<T>> = data.iter().cloned().map(Item).collect();
        let (ms, bytes) = avg_ms_pair(|| {
            rt.block_on(async {
                let batch = serde_arrow::to_record_batch(&fields, &items).unwrap();
                let schema_in = batch.schema();
                let array = session.arrow().from_arrow_record_batch(batch, &schema_in).unwrap();
                let mut out: Vec<u8> = vec![];
                let strategy = WriteStrategyBuilder::from_session(session)
                    .with_btrblocks_builder(BtrBlocksCompressorBuilder::from_session(session).with_compact())
                    .build();
                session.write_options().with_strategy(strategy).write(&mut out, array.to_array_stream()).await.unwrap();
                out
            })
        });

        let back: Vec<Item<T>> = rt.block_on(async {
            let file = session.open_options().open_buffer(ByteBuffer::from(bytes.clone())).unwrap();
            let scan = file.scan().unwrap();
            let array = scan.into_array_stream().unwrap().read_all().await.unwrap();
            let schema_out = session.arrow().to_arrow_schema(array.dtype()).unwrap();
            use vortex::array::VortexSessionExecute;
            #[allow(deprecated)]
            use vortex::arrow::ArrowArrayExecutor as _;
            let mut ctx = session.create_execution_ctx();
            #[allow(deprecated)]
            let batch = array.execute_record_batch(&schema_out, &mut ctx).unwrap();
            serde_arrow::from_record_batch(&batch).unwrap()
        });
        assert_eq!(back.iter().map(|i| i.0.clone()).collect::<Vec<_>>(), data.to_vec(), "vortex roundtrip mismatch");
        Ok((ms, bytes.len()))
    }));
    std::panic::set_hook(old_hook);
    match result {
        Ok(res) => res,
        Err(_) => Err("panic in the vortex pipeline (type unsupported?)".to_string()),
    }
}

/// [`bench_vortex`] as table cells, size first: "-" for types the pipeline
/// can't express (complex enums, heterogeneous-shape `serde_json::Value`).
#[allow(dead_code)]
pub fn vortex_cell<T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + PartialEq + std::fmt::Debug>(
    data: &[T],
) -> (String, String) {
    match bench_vortex(data) {
        Ok((t, s)) => (format_bytes(s), format_ms(t)),
        Err(_) => ("-".to_string(), "-".to_string()),
    }
}

/// [`bench_vortex_native_map`] as table cells, size first.
#[allow(dead_code)]
pub fn vortex_cell_native_map<
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + PartialEq + std::fmt::Debug,
>(
    data: &[T],
) -> (String, String) {
    match bench_vortex_native_map(data) {
        Ok((t, s)) => (format_bytes(s), format_ms(t)),
        Err(_) => ("-".to_string(), "-".to_string()),
    }
}

/// Row of the four-codec table (pco, PcoPack, msgpack+zstd, Vortex): size
/// first, then write time.
pub struct ComparisonRow {
    pub type_name: String,
    pub pco_size: String,
    pub pco_pack_size: String,
    pub mp_size: String,
    pub vortex_size: String,
    pub pco_time: String,
    pub pco_pack_time: String,
    pub mp_time: String,
    pub vortex_time: String,
}

impl markdown_tables::MarkdownTableRow for ComparisonRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Type", "pco", "PcoPack", "msgpack", "Vortex", "pco", "PcoPack", "msgpack", "Vortex"]
    }

    fn column_values(&self) -> Vec<String> {
        vec![
            self.type_name.clone(),
            self.pco_size.clone(),
            self.pco_pack_size.clone(),
            self.mp_size.clone(),
            self.vortex_size.clone(),
            self.pco_time.clone(),
            self.pco_pack_time.clone(),
            self.mp_time.clone(),
            self.vortex_time.clone(),
        ]
    }
}

pub fn print_comparison_table(rows: &[ComparisonRow]) {
    if !rows.is_empty() {
        println!("{}", markdown_tables::as_table(rows));
    }
}

/// Row of the three-codec table for types pco can't represent: PcoPack,
/// msgpack+zstd, Vortex (size first, then time).
pub struct SlimRow {
    pub type_name: String,
    pub pco_pack_size: String,
    pub mp_size: String,
    pub vortex_size: String,
    pub pco_pack_time: String,
    pub mp_time: String,
    pub vortex_time: String,
}

impl markdown_tables::MarkdownTableRow for SlimRow {
    fn column_names() -> Vec<&'static str> {
        vec!["Type", "PcoPack", "msgpack", "Vortex", "PcoPack", "msgpack", "Vortex"]
    }

    fn column_values(&self) -> Vec<String> {
        vec![
            self.type_name.clone(),
            self.pco_pack_size.clone(),
            self.mp_size.clone(),
            self.vortex_size.clone(),
            self.pco_pack_time.clone(),
            self.mp_time.clone(),
            self.vortex_time.clone(),
        ]
    }
}

pub fn print_slim_table(rows: &[SlimRow]) {
    if !rows.is_empty() {
        println!("{}", markdown_tables::as_table(rows));
    }
}
