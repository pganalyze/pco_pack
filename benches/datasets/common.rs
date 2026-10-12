//! Shared machinery for the datasets bench: pinned-file downloads, one-pass
//! codec runners, value-roundtrip mirrors, and the table printer.
//!
//! Each op (serialize, deserialize, filter) runs once per dataset; the
//! 50-iteration convention of smaller benches is infeasible at this scale.

use super::*;
use columnar::{Borrow, FromBytes, Index, Len};
use std::path::{Path, PathBuf};
use std::time::Instant;

// ---- One-pass timing + codec runners ----

/// Run an op once and return elapsed ms with its result.
pub fn one_pass<T, F: FnOnce() -> T>(f: F) -> (f64, T) {
    let start = Instant::now();
    let result = f();
    (start.elapsed().as_secs_f64() * 1_000.0, result)
}

/// One codec's one-pass results. All-`None` means the codec could not express
/// the data (the Vortex degrade path).
#[derive(Debug, Default, Clone)]
pub struct CodecMetrics {
    pub serial_ms: Option<f64>,
    pub deserial_ms: Option<f64>,
    pub size: Option<usize>,
    /// One entry per measured filter, in order; `None` if not expressible.
    pub filter_ms: Vec<Option<f64>>,
}

pub fn bench_pco_pack<T, V>(data: Vec<T>, filters: &[(&serde_json::Value, usize)], verify: V) -> CodecMetrics
where
    T: pco_pack::PcoPack + std::fmt::Debug,
    V: FnOnce(&[T]),
{
    let (serial_ms, bytes) = one_pass(|| {
        let chunks = pco_pack::PcoPack::write(data).unwrap();
        T::to_bytes(&chunks).unwrap()
    });
    let (deserial_ms, rows) = one_pass(|| {
        let chunks = T::from_bytes(&bytes).unwrap();
        T::filter(&chunks, serde_json::json!({}), &[]).unwrap()
    });
    verify(&rows);
    let mut filter_ms = Vec::new();
    for (query, expected) in filters {
        let (ms, rows) = one_pass(|| {
            let chunks = T::from_bytes(&bytes).unwrap();
            T::filter(&chunks, (*query).clone(), &[]).unwrap()
        });
        if rows.len() != *expected {
            // Rows return in stored order, so a boundary bug shows at the
            // head/tail; unique ids identify the rows.
            let n = rows.len();
            eprintln!("[debug] pco_pack filter count: got {n}, expected {expected}; query {query}");
            for i in 0..n {
                if i < 3 || i + 6 >= n {
                    let row = &rows[i];
                    eprintln!("[debug]   row[{i}]: {row:?}");
                } else if i == 3 {
                    eprintln!("[debug]   ... {} omitted", n - 9);
                }
            }
            panic!("pco_pack filter count");
        }
        filter_ms.push(Some(ms));
    }
    CodecMetrics { serial_ms: Some(serial_ms), deserial_ms: Some(deserial_ms), size: Some(bytes.len()), filter_ms }
}

/// columnar runner. Serialize: mirror to `Container` + indexed encode +
/// zstd L3. Deserialize/filter: decode + per-row `Index::get` + count assert.
pub fn bench_columnar<R, B, K, F>(
    n_rows: usize, build_rows: B, filters: &[(K, usize)], count_matches: F,
) -> CodecMetrics
where
    R: columnar::Columnar + std::fmt::Debug,
    B: FnOnce() -> Vec<R>,
    F: for<'a> Fn(&columnar::BorrowedOf<'a, R>, &K) -> usize,
{
    let (serial_ms, buf) = one_pass(|| {
        let mut container = <R as columnar::Columnar>::Container::default();
        for row in build_rows().iter() {
            columnar::Push::push(&mut container, row);
        }
        let mut words = vec![];
        columnar::bytes::indexed::encode(&mut words, &container.borrow());
        zstd::encode_all(bytemuck::cast_slice(&words), 3).unwrap()
    });
    // The decompressed buffer must outlive the borrowed view, so each
    // measurement decompresses inline and returns only a count.
    let (deserial_ms, count) = one_pass(|| {
        let decompressed = zstd::decode_all(&*buf).unwrap();
        let words: &[u64] = bytemuck::cast_slice(&decompressed);
        let decoded = columnar::BorrowedOf::<R>::from_bytes(&mut columnar::bytes::indexed::decode(words));
        let count = Len::len(&decoded);
        for i in 0..count {
            std::hint::black_box(Index::get(&decoded, i));
        }
        count
    });
    assert_eq!(count, n_rows, "columnar row count");
    let mut filter_ms = Vec::new();
    for (k, expected) in filters {
        let (ms, matched) = one_pass(|| {
            let decompressed = zstd::decode_all(&*buf).unwrap();
            let words: &[u64] = bytemuck::cast_slice(&decompressed);
            let decoded = columnar::BorrowedOf::<R>::from_bytes(&mut columnar::bytes::indexed::decode(words));
            count_matches(&decoded, k)
        });
        assert_eq!(matched, *expected, "columnar filter count");
        filter_ms.push(Some(ms));
    }
    CodecMetrics { serial_ms: Some(serial_ms), deserial_ms: Some(deserial_ms), size: Some(buf.len()), filter_ms }
}

/// A serde_columnar store: the per-dataset `#[columnar(ser, de)]` wrapper over
/// the row struct, with its row count exposed for the roundtrip asserts.
pub trait ScStore: serde::Serialize + for<'de> serde::Deserialize<'de> {
    fn row_count(&self) -> usize;
}

/// serde_columnar runner. Serialize: mirror store `to_vec` + zstd L3.
/// Deserialize/filter: full decode + in-Rust store predicate + count assert.
pub fn bench_serde_columnar<S, B, K, F>(
    n_rows: usize, build_store: B, filters: &[(K, usize)], count_matches: F,
) -> CodecMetrics
where
    S: ScStore,
    B: FnOnce() -> S,
    F: Fn(&S, &K) -> usize,
{
    let (serial_ms, buf) = one_pass(|| {
        let store = build_store();
        let postcard_buf = serde_columnar::to_vec(&store).unwrap();
        zstd::encode_all(postcard_buf.as_slice(), 3).unwrap()
    });
    let (deserial_ms, count) = one_pass(|| {
        let decompressed = zstd::decode_all(&*buf).unwrap();
        let store: S = serde_columnar::from_bytes(&decompressed).unwrap();
        store.row_count()
    });
    assert_eq!(count, n_rows, "serde_columnar row count");
    let mut filter_ms = Vec::new();
    for (k, expected) in filters {
        let (ms, matched) = one_pass(|| {
            let decompressed = zstd::decode_all(&*buf).unwrap();
            let store: S = serde_columnar::from_bytes(&decompressed).unwrap();
            count_matches(&store, k)
        });
        assert_eq!(matched, *expected, "serde_columnar filter count");
        filter_ms.push(Some(ms));
    }
    CodecMetrics { serial_ms: Some(serial_ms), deserial_ms: Some(deserial_ms), size: Some(buf.len()), filter_ms }
}

/// msgpack+zstd runner. Serialize uses `to_msgpack_zstd` (zstd L3);
/// deserialize is a full multi-frame decode + full value roundtrip.
pub fn bench_msgpack<T, K, F>(data: &[T], filters: &[(K, usize)], matches: F) -> CodecMetrics
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + PartialEq + std::fmt::Debug,
    F: Fn(&T, &K) -> bool,
{
    let (serial_ms, buf) = one_pass(|| to_msgpack_zstd(data));
    // One zstd frame per 262k-row chunk, each a single msgpack array; the
    // decoder is multi-frame, so one `decode_all` covers the buffer.
    let decode = || {
        let decompressed = zstd::decode_all(&*buf).unwrap();
        let n_frames = (data.len() + CHUNK_ROWS - 1) / CHUNK_ROWS;
        let mut cursor = std::io::Cursor::new(decompressed);
        let mut rows: Vec<T> = Vec::new();
        for _ in 0..n_frames {
            let frame: Vec<T> = rmp_serde::from_read(&mut cursor).unwrap();
            rows.extend(frame);
        }
        rows
    };
    let (deserial_ms, rows) = one_pass(&decode);
    assert_eq!(rows, data.to_vec(), "msgpack roundtrip mismatch");
    let mut filter_ms = Vec::new();
    for (k, expected) in filters {
        let (ms, rows) = one_pass(&decode);
        let matched = rows.iter().filter(|r| matches(r, k)).count();
        assert_eq!(matched, *expected, "msgpack filter count");
        filter_ms.push(Some(ms));
    }
    CodecMetrics { serial_ms: Some(serial_ms), deserial_ms: Some(deserial_ms), size: Some(buf.len()), filter_ms }
}

/// Current-thread tokio runtime, shared across the dataset benches.
pub fn ds_vortex_rt() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Builder::new_current_thread().build().unwrap())
}

/// Create the session inside `block_on` so vortex picks up the ambient
/// tokio runtime (struct bench convention).
fn ds_vortex_session() -> &'static vortex::session::VortexSession {
    static SESSION: std::sync::OnceLock<vortex::session::VortexSession> = std::sync::OnceLock::new();
    SESSION.get_or_init(|| {
        use vortex::VortexSessionDefault as _;
        ds_vortex_rt().block_on(async { vortex::session::VortexSession::default() })
    })
}

/// Vortex runner. Compact btrblocks file (serde_arrow->arrow->vortex),
/// roundtrip-verified unmeasured. A caught panic degrades all metrics to "-".
pub fn bench_vortex_ds<T, K, E>(data: &[T], filters: &[(K, usize)], build_expr: E) -> CodecMetrics
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + PartialEq + std::fmt::Debug,
    E: Fn(&K) -> vortex::array::expr::Expression,
{
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        use arrow_schema::FieldRef;
        use serde_arrow::schema::{SchemaLike, TracingOptions};
        use vortex::array::stream::ArrayStreamExt;
        use vortex::arrow::ArrowSessionExt as _;
        use vortex::buffer::ByteBuffer;
        use vortex::compressor::BtrBlocksCompressorBuilder;
        use vortex::file::{OpenOptionsSessionExt, WriteOptionsSessionExt, WriteStrategyBuilder};

        let rt = ds_vortex_rt();
        let session = ds_vortex_session();

        // Schema: `from_type` with a `from_samples` fallback (chrono/uuid).
        // Samples are plain rows, not `Items` (which wraps each row). Maps
        // stay native arrow Map -> vortex.map: promoting an open vocabulary
        // to a sample-derived schema silently drops entries whose keys
        // missed the sample; rows without maps are unaffected.
        let options =
            TracingOptions::default().enums_without_data_as_strings(true).allow_null_fields(true).map_as_struct(false);
        let fields: Vec<FieldRef> = match <Vec<FieldRef>>::from_type::<T>(options.clone()) {
            Ok(f) => f,
            Err(_) => {
                // Spread samples across the dataset: a union variant slot
                // exists only for values seen; a missing slot fails derive.
                let step = data.len().div_ceil(100).max(1);
                let samples: Vec<T> = data.iter().step_by(step).take(100).cloned().collect();
                <Vec<FieldRef>>::from_samples(samples, options).unwrap()
            }
        };

        let (serial_ms, bytes) = one_pass(|| {
            rt.block_on(async {
                let batch = serde_arrow::to_record_batch(&fields, &data).unwrap();
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
        let buffer = ByteBuffer::from(bytes.clone());

        // Read path: scan (optionally filtered), materialized back to rows
        // through Arrow.
        let read = |filter: Option<vortex::array::expr::Expression>| {
            rt.block_on(async {
                let file = session.open_options().open_buffer(buffer.clone()).unwrap();
                let mut scan = file.scan().unwrap();
                if let Some(expr) = filter {
                    let bound = expr.bind(file.dtype()).unwrap().optimize_recursive().unwrap();
                    scan = scan.with_filter(bound);
                }
                let array = scan.into_array_stream().unwrap().read_all().await.unwrap();
                let schema_out = session.arrow().to_arrow_schema(array.dtype()).unwrap();
                #[allow(deprecated)] // deprecated, but the stable export path
                {
                    use vortex::array::VortexSessionExecute;
                    use vortex::arrow::ArrowArrayExecutor as _;
                    let mut ctx = session.create_execution_ctx();
                    let batch = array.execute_record_batch(&schema_out, &mut ctx).unwrap();
                    serde_arrow::from_record_batch(&batch).unwrap()
                }
            })
        };

        // Write-time roundtrip check (harness convention; unmeasured).
        let back: Vec<T> = read(None);
        assert_eq!(back, data.to_vec(), "vortex roundtrip mismatch");

        let (deserial_ms, rows) = one_pass(|| read(None));
        std::hint::black_box(rows.len());
        let mut filter_ms = Vec::new();
        for (k, expected) in filters {
            let (ms, rows) = one_pass(|| read(Some(build_expr(k))));
            assert_eq!(rows.len(), *expected, "vortex filter count");
            filter_ms.push(Some(ms));
        }
        CodecMetrics { serial_ms: Some(serial_ms), deserial_ms: Some(deserial_ms), size: Some(bytes.len()), filter_ms }
    }));
    std::panic::set_hook(old_hook);
    match result {
        Ok(metrics) => metrics,
        Err(panic) => {
            // `PCO_PACK_BENCH_DEBUG=1` prints the caught panic.
            if std::env::var_os("PCO_PACK_BENCH_DEBUG").is_some() {
                let msg = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "non-string panic".to_string());
                eprintln!("vortex pipeline panic: {msg}");
            }
            CodecMetrics::default() // all None -> "-" cells
        }
    }
}

// ---- Downloads: cached in a gitignored tmp dir ----

/// A pinned source file. Downloaded to the data dir on first run, verified
/// against the pinned size (+ md5 where pinned).
#[derive(Debug, Clone, Copy)]
pub struct Ds {
    pub name: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub md5: Option<&'static str>,
}

/// The bench data dir: `$PCO_PACK_BENCH_DATA` if set, else gitignored `tmp/`
/// at the project root.
pub fn data_dir() -> PathBuf {
    match std::env::var("PCO_PACK_BENCH_DATA") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp"),
    }
}

impl Ds {
    /// Local path of the file; downloads it if the cached copy is missing or
    /// fails the pinned check.
    pub fn ensure_downloaded(&self) -> PathBuf {
        let dir = data_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(self.name);
        if path.exists() && file_ok(&path, self) {
            return path; // pinned files are immutable; no network on re-runs
        }
        if path.exists() {
            eprintln!("[{}] cached copy is stale; re-downloading", self.name);
            std::fs::remove_file(&path).unwrap();
        }
        eprintln!("[{}] downloading {} ({} B)", self.name, self.url, self.size);
        let part = dir.join(format!(".{name}.part", name = self.name));
        let status = std::process::Command::new("curl")
            .args(["-fL", "--retry", "3", "-C", "-", "-o"])
            .arg(&part)
            .arg(self.url)
            .status()
            .expect("failed to run curl (is it installed?)");
        assert!(status.success(), "curl failed for {}", self.url);
        assert!(
            file_ok(&part, self),
            "downloaded {} fails the pinned check (expected {} B{})",
            self.url,
            self.size,
            self.md5.map(|m| format!(" + md5 {m}")).unwrap_or_default()
        );
        std::fs::rename(&part, &path).unwrap();
        path
    }
}

fn file_ok(path: &Path, ds: &Ds) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() != ds.size {
        return false;
    }
    match ds.md5 {
        None => true,
        Some(pinned) => file_md5(path) == pinned,
    }
}

fn file_md5(path: &Path) -> String {
    use md5::Digest;
    let mut file = std::fs::File::open(path).unwrap();
    let mut hasher = md5::Md5::new();
    std::io::copy(&mut file, &mut hasher).unwrap();
    hex::encode(hasher.finalize())
}

// ---- Value-roundtrip mirrors: the exact values a lossy PcoPack write stores ----

/// PcoPack stores `DateTime<Utc>` as i64 microseconds: the exact instant a
/// default write stores (sub-microsecond truncated).
pub fn us_floor(dt: &chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_micros(dt.timestamp_micros()).unwrap()
}

/// The exact instant a `time_round` write stores: round to the nearest
/// multiple of `g` microseconds (0 = none), like chrono DurationRound.
pub fn ts_rounded(dt: &chrono::DateTime<chrono::Utc>, g: i64) -> chrono::DateTime<chrono::Utc> {
    if g == 0 {
        return *dt;
    }
    let micros = dt.timestamp_micros();
    let q = micros / g;
    let r = micros % g;
    let half = g / 2;
    let q = if r >= half || (r < 0 && (-r) > half) { q + 1 } else { q };
    chrono::DateTime::from_timestamp_micros(q * g).unwrap()
}

/// The exact f32 a `float_round = N` write stores (src/float.rs: round to N
/// decimal places via i64).
pub fn round_f32(v: f32, n: u32) -> f32 {
    ((v as f64 * 10f64.powi(n as i32)).round() / 10f64.powi(n as i32)) as f32
}

/// The exact f64 a `float_round = N` write stores.
pub fn round_f64(v: f64, n: u32) -> f64 {
    (v * 10f64.powi(n as i32)).round() / 10f64.powi(n as i32)
}

// ---- Table printing ----

/// One table cell; a missing metric renders as "-".
fn cell(m: &CodecMetrics, kind: &str, filter_idx: Option<usize>) -> String {
    let value = match kind {
        "Serialize" => m.serial_ms.as_ref().map(|v| format_ms(*v)),
        "Deserialize" => m.deserial_ms.as_ref().map(|v| format_ms(*v)),
        "Size" => m.size.as_ref().map(|v| format_bytes(*v)),
        _ => match filter_idx {
            Some(i) if i < m.filter_ms.len() => {
                let v: Option<f64> = m.filter_ms[i];
                v.map(format_ms)
            }
            _ => None,
        },
    };
    value.unwrap_or_else(|| "-".to_string())
}

/// All six codecs' one-pass results for one (dataset, entity) table.
pub struct CodecResults {
    pub pco_pack: CodecMetrics,
    /// The lossy PcoPack variant (`float_round`/`time_round`); `None` when
    /// the dataset has no lossy variant (column omitted).
    pub pco_pack_lossy: Option<CodecMetrics>,
    pub columnar: CodecMetrics,
    pub serde_columnar: CodecMetrics,
    pub msgpack: CodecMetrics,
    pub vortex: CodecMetrics,
}

impl CodecResults {
    /// The dataset codec table: `Metric | PcoPack | [PcoPack (lossy)] | Vortex
    /// | columnar | serde_columnar | msgpack`, Size first.
    pub fn print(self, filter_labels: &[&str]) {
        let metrics = ["Size", "Serialize", "Deserialize"];
        if let Some(l) = &self.pco_pack_lossy {
            let mut rows: Vec<CodecRowLossy> = metrics
                .iter()
                .map(|m| CodecRowLossy {
                    metric: m.to_string(),
                    pco_pack: cell(&self.pco_pack, m, None),
                    pco_pack_lossy: cell(l, m, None),
                    vortex: cell(&self.vortex, m, None),
                    columnar: cell(&self.columnar, m, None),
                    serde_columnar: cell(&self.serde_columnar, m, None),
                    msgpack: cell(&self.msgpack, m, None),
                })
                .collect();
            for (i, label) in filter_labels.iter().enumerate() {
                rows.push(CodecRowLossy {
                    metric: label.to_string(),
                    pco_pack: cell(&self.pco_pack, "Filter", Some(i)),
                    pco_pack_lossy: cell(l, "Filter", Some(i)),
                    vortex: cell(&self.vortex, "Filter", Some(i)),
                    columnar: cell(&self.columnar, "Filter", Some(i)),
                    serde_columnar: cell(&self.serde_columnar, "Filter", Some(i)),
                    msgpack: cell(&self.msgpack, "Filter", Some(i)),
                });
            }
            println!("{}", markdown_tables::as_table(&rows));
        } else {
            let mut rows: Vec<CodecRow> = metrics
                .iter()
                .map(|m| CodecRow {
                    metric: m.to_string(),
                    pco_pack: cell(&self.pco_pack, m, None),
                    vortex: cell(&self.vortex, m, None),
                    columnar: cell(&self.columnar, m, None),
                    serde_columnar: cell(&self.serde_columnar, m, None),
                    msgpack: cell(&self.msgpack, m, None),
                })
                .collect();
            for (i, label) in filter_labels.iter().enumerate() {
                rows.push(CodecRow {
                    metric: label.to_string(),
                    pco_pack: cell(&self.pco_pack, "Filter", Some(i)),
                    vortex: cell(&self.vortex, "Filter", Some(i)),
                    columnar: cell(&self.columnar, "Filter", Some(i)),
                    serde_columnar: cell(&self.serde_columnar, "Filter", Some(i)),
                    msgpack: cell(&self.msgpack, "Filter", Some(i)),
                });
            }
            println!("{}", markdown_tables::as_table(&rows));
        }
    }
}

/// Codec-table row without the lossy column.
struct CodecRow {
    metric: String,
    pco_pack: String,
    vortex: String,
    columnar: String,
    serde_columnar: String,
    msgpack: String,
}

impl markdown_tables::MarkdownTableRow for CodecRow {
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

/// Codec-table row with the `PcoPack (lossy)` column.
struct CodecRowLossy {
    metric: String,
    pco_pack: String,
    pco_pack_lossy: String,
    vortex: String,
    columnar: String,
    serde_columnar: String,
    msgpack: String,
}

impl markdown_tables::MarkdownTableRow for CodecRowLossy {
    fn column_names() -> Vec<&'static str> {
        vec!["Metric", "PcoPack", "PcoPack (lossy)", "Vortex", "columnar", "serde_columnar", "msgpack"]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.metric.clone(),
            self.pco_pack.clone(),
            self.pco_pack_lossy.clone(),
            self.vortex.clone(),
            self.columnar.clone(),
            self.serde_columnar.clone(),
            self.msgpack.clone(),
        ]
    }
}

/// Short filter row label: `Filter 4.05%` (trailing zeros trimmed; `<0.01%`
/// floor for near-empty matches).
pub fn filter_label(matched: usize, total: usize) -> String {
    let pct = matched as f64 * 100.0 / total as f64;
    if pct < 0.01 {
        return "Filter <0.01%".to_string();
    }
    let num = format!("{:.2}", pct).trim_end_matches('0').trim_end_matches('.').to_string();
    format!("Filter {num}%")
}
