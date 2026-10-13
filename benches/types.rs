//! One bench per field type and data distribution: pco / PcoPack /
//! msgpack+zstd / Vortex.
//!
//! All rows share one chunk size so sizes are comparable. pco gets a column
//! only where it compresses without a major transformation.

include!("bench_common.rs");
use chrono::{DateTime, Duration, Utc};
use pco_pack::PcoPack;
use serde_bytes::ByteBuf;
use std::collections::{BTreeMap, HashMap};

const N: usize = 100_000;

fn main() {
    println!("## Types\n");
    numbers_table();
    booleans_table();
    option_table();
    enums_table();
    strings_bytes_table();
    maps_table();
    floats_table();
    timestamps_table();
}

// Data model for the enums table: Status is a skewed tiny-alphabet enum;
// Event nests struct payloads and unit variants.

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, PcoPack)]
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

// Row shapes: Vortex fills every row it can express.

macro_rules! row3 {
    ($rows:expr, $label:expr, $n:expr, $gen:expr, $pco:expr) => {{
        let data = ($gen)($n);
        let (name, pp_t, pp_s) = bench_write($label, $n, $gen);
        let (p_t, p_s) = $pco(&data);
        let mp_s = to_msgpack_zstd(&data).len();
        let mp_t = bench_msgpack(&data);
        let (vx_s, vx_t) = vortex_cell(&data);
        $rows.push(ComparisonRow {
            type_name: format!("`{name}`"),
            pco_size: format_bytes(p_s),
            pco_pack_size: format_bytes(pp_s),
            mp_size: format_bytes(mp_s),
            vortex_size: vx_s,
            pco_time: format_ms(p_t),
            pco_pack_time: format_ms(pp_t),
            mp_time: format_ms(mp_t),
            vortex_time: vx_t,
        });
    }};
}

macro_rules! row3_nopco {
    ($rows:expr, $label:expr, $n:expr, $gen:expr) => {{
        let data = ($gen)($n);
        let (name, pp_t, pp_s) = bench_write($label, $n, $gen);
        let mp_s = to_msgpack_zstd(&data).len();
        let mp_t = bench_msgpack(&data);
        let (vx_s, vx_t) = vortex_cell(&data);
        $rows.push(ComparisonRow {
            type_name: format!("`{name}`"),
            pco_size: "-".to_string(),
            pco_pack_size: format_bytes(pp_s),
            mp_size: format_bytes(mp_s),
            vortex_size: vx_s,
            pco_time: "-".to_string(),
            pco_pack_time: format_ms(pp_t),
            mp_time: format_ms(mp_t),
            vortex_time: vx_t,
        });
    }};
}

/// The default Vortex cell struct-promotes maps (serde_arrow default
/// tracing); the `native` form stores them as `vortex.map`. Rows declare
/// their layout; it is never inferred from whether promotion succeeds.
macro_rules! row2 {
    ($rows:expr, $label:expr, $n:expr, $gen:expr) => {
        row2_vx!($rows, $label, $n, $gen, vortex_cell)
    };
    ($rows:expr, $label:expr, $n:expr, $gen:expr, native) => {
        row2_vx!($rows, $label, $n, $gen, vortex_cell_native_map)
    };
}

macro_rules! row2_vx {
    ($rows:expr, $label:expr, $n:expr, $gen:expr, $vx:path) => {{
        let data = ($gen)($n);
        let (name, pp_t, pp_s) = bench_write($label, $n, $gen);
        let mp_s = to_msgpack_zstd(&data).len();
        let mp_t = bench_msgpack(&data);
        let (vx_s, vx_t) = $vx(&data);
        $rows.push(SlimRow {
            type_name: format!("`{name}`"),
            pco_pack_size: format_bytes(pp_s),
            mp_size: format_bytes(mp_s),
            vortex_size: vx_s,
            pco_pack_time: format_ms(pp_t),
            mp_time: format_ms(mp_t),
            vortex_time: vx_t,
        });
    }};
}

/// msgpack runs on the String-converted data; Vortex on the original
/// SmolStr data.
macro_rules! row2_smolstr {
    ($rows:expr, $label:expr, $n:expr, $gen:expr, $map:ident) => {
        row2_smolstr_vx!($rows, $label, $n, $gen, $map, vortex_cell)
    };
    ($rows:expr, $label:expr, $n:expr, $gen:expr, $map:ident, native) => {
        row2_smolstr_vx!($rows, $label, $n, $gen, $map, vortex_cell_native_map)
    };
}

macro_rules! row2_smolstr_vx {
    ($rows:expr, $label:expr, $n:expr, $gen:expr, $map:ident, $vx:path) => {{
        let data = ($gen)($n);
        let (name, pp_t, pp_s) = bench_write($label, $n, $gen);
        let string_data: Vec<$map<(String, i32), i32>> =
            data.iter().map(|m| m.iter().map(|((k, v), val)| ((k.as_ref().to_string(), *v), *val)).collect()).collect();
        let mp_size = to_msgpack_zstd(&string_data).len();
        let mp_rt = avg_ms(|| {
            let data = ($gen)($n);
            let string_data: Vec<$map<(String, i32), i32>> = data
                .iter()
                .map(|m| m.iter().map(|((k, v), val)| ((k.as_ref().to_string(), *v), *val)).collect())
                .collect();
            to_msgpack_zstd(&string_data)
        });
        let (vx_s, vx_t) = $vx(&data);
        $rows.push(SlimRow {
            type_name: format!("`{name}`"),
            pco_pack_size: format_bytes(pp_s),
            mp_size: format_bytes(mp_size),
            vortex_size: vx_s,
            pco_pack_time: format_ms(pp_t),
            mp_time: format_ms(mp_rt),
            vortex_time: vx_t,
        });
    }};
}

// Numbers: synthetic codec-mechanics controls. Each row isolates one axis:
// distinct count, dwell length, recurrence distance, placement, sparsity,
// skew, chunk fixed costs. Predicted winners come from the same section 2
// findings, so a row that disagrees with its prediction is drift signal.

fn numbers_table() {
    println!("### Numbers\n");
    let mut rows: Vec<ComparisonRow> = Vec::new();

    // --- entropy floor: full-width uniform, nothing to exploit ---
    row3!(&mut rows, "u8 (uniform, full width)", CHUNK_ROWS, uniform_u8, |d: &Vec<u8>| bench_pco_col_8bit(d));
    row3!(&mut rows, "i16 (uniform, full width)", CHUNK_ROWS, uniform_i16, |d: &Vec<i16>| bench_pco_col(d));
    row3!(&mut rows, "i32 (uniform, full width)", CHUNK_ROWS, uniform_i32, |d: &Vec<i32>| bench_pco_col(d));
    row3!(&mut rows, "i64 (uniform, full width)", CHUNK_ROWS, uniform_i64, |d: &Vec<i64>| bench_pco_col(d));

    // --- type-agnosticism: identical values in a too-large type ---
    row3!(&mut rows, "i16 (uniform 0..1000)", CHUNK_ROWS, uniform1000_i16, |d: &Vec<i16>| bench_pco_col(d));
    row3!(&mut rows, "i32 (uniform 0..1000)", CHUNK_ROWS, uniform1000_i32, |d: &Vec<i32>| bench_pco_col(d));

    // --- delta coding: monotone/cyclic ramp + noise (pco's wheelhouse) ---
    row3!(&mut rows, "i64 (counter, monotonic)", CHUNK_ROWS, counter_i64, |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i16 (counter, cyclic ~20% range)", CHUNK_ROWS, counter_i16, |d: &Vec<i16>| bench_pco_col(d));
    row3!(&mut rows, "u8 (counter, cyclic 8-bit)", CHUNK_ROWS, counter_u8, |d: &Vec<u8>| bench_pco_col_8bit(d));

    // --- heavy tail: right-skew magnitudes ---
    row3!(&mut rows, "i64 (right-skew, sparse variant)", CHUNK_ROWS, skew_sparse_i64, |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (right-skew)", CHUNK_ROWS, skew_i64, |d: &Vec<i64>| bench_pco_col(d));

    // --- sparsity placement: same marginal, placement decides ---
    row3!(&mut rows, "i64 (93% zeros, scattered)", CHUNK_ROWS, zero_spike_i64, |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (93% zeros, clustered windows)", CHUNK_ROWS, zero_window_i64, |d: &Vec<i64>| bench_pco_col(
        d
    ));

    // --- tiny alphabet: i.i.d. floor + dwell sweep (run length) ---
    row3!(&mut rows, "u8 (tiny alphabet i.i.d., d=4)", CHUNK_ROWS, states_u8, |d: &Vec<u8>| bench_pco_col_8bit(d));
    row3!(&mut rows, "u8 (dwell sweep, d=3, run~16)", CHUNK_ROWS, |n| dwell_u8(n, 16), |d: &Vec<u8>| {
        bench_pco_col_8bit(d)
    });
    row3!(&mut rows, "u8 (dwell sweep, d=3, run~256)", CHUNK_ROWS, |n| dwell_u8(n, 256), |d: &Vec<u8>| {
        bench_pco_col_8bit(d)
    });
    row3!(&mut rows, "u8 (dwell sweep, d=3, run~4096)", CHUNK_ROWS, |n| dwell_u8(n, 4096), |d: &Vec<u8>| {
        bench_pco_col_8bit(d)
    });

    // --- distinct-count sweep @ writer chunk: dictionary-territory crossover;
    //     8191/8193 bracket the CAP=8192 gate, 32768 is the churn ceiling ---
    row3!(&mut rows, "i64 (d-sweep, d=4)", CHUNK_ROWS, |n| pool_iid_i64(n, 4), |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (d-sweep, d=35)", CHUNK_ROWS, |n| pool_iid_i64(n, 35), |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (d-sweep, d=86)", CHUNK_ROWS, |n| pool_iid_i64(n, 86), |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (d-sweep, d=1024)", CHUNK_ROWS, |n| pool_iid_i64(n, 1024), |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (d-sweep, d=8191)", CHUNK_ROWS, |n| pool_iid_i64(n, 8191), |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (d-sweep, d=8193)", CHUNK_ROWS, |n| pool_iid_i64(n, 8193), |d: &Vec<i64>| bench_pco_col(d));
    row3!(&mut rows, "i64 (d-sweep, d=32768)", CHUNK_ROWS, |n| pool_iid_i64(n, 32768), |d: &Vec<i64>| bench_pco_col(d));

    // --- placement pair: same d=574 multiset, order decides the winner ---
    row3!(&mut rows, "i64 (placement pair, d=574 grouped)", CHUNK_ROWS, |n| placement_pair_d574(n, true), |d: &Vec<
        i64,
    >| {
        bench_pco_col(d)
    });
    row3!(
        &mut rows,
        "i64 (placement pair, d=574 shuffled)",
        CHUNK_ROWS,
        |n| placement_pair_d574(n, false),
        |d: &Vec<i64>| bench_pco_col(d)
    );

    // --- recurrence-distance pair: same alphabet and d=4096, repeat
    //     spacing ~512 vs ~4096 (match-offset cost, LZ vs bitpack) ---
    row3!(
        &mut rows,
        "i64 (recurrence pair, d=4096 local-block)",
        CHUNK_ROWS,
        |n| recurrence_i64(n, true),
        |d: &Vec<i64>| bench_pco_col(d)
    );
    row3!(
        &mut rows,
        "i64 (recurrence pair, d=4096 full-cycle)",
        CHUNK_ROWS,
        |n| recurrence_i64(n, false),
        |d: &Vec<i64>| bench_pco_col(d)
    );

    // --- chunk fixed costs: same shape (d=64 pool), tiny tail chunk vs
    //     writer chunk; shows the per-chunk frame overhead ---
    row3!(&mut rows, "i64 (d=64 pool, N=2k tail chunk)", 2_048, |n| pool_iid_i64(n, 64), |d: &Vec<i64>| bench_pco_col(
        d
    ));
    row3!(&mut rows, "i64 (d=64 pool, full writer chunk)", CHUNK_ROWS, |n| pool_iid_i64(n, 64), |d: &Vec<i64>| {
        bench_pco_col(d)
    });

    print_comparison_table(&rows);
    println!();
}

// Booleans: the 1:1 bool -> u8 mapping keeps pco comparable. Rows vary where
// the entropy sits: i.i.d. fractions, clustered runs, alternation.

fn booleans_table() {
    println!("### Booleans\n");
    let mut rows: Vec<ComparisonRow> = Vec::new();

    row3!(&mut rows, "bool (i.i.d. 50% true)", N, bool_iid_50, pco_bool);
    row3!(&mut rows, "bool (i.i.d. 90% true)", N, bool_iid_90, pco_bool);
    row3!(&mut rows, "bool (clustered runs)", N, bool_runs, pco_bool);
    row3!(&mut rows, "bool (alternating, period 2)", N, bool_alternating, pco_bool);
    row3!(&mut rows, "bool (i.i.d. 5% true)", N, bool_iid_5, pco_bool);
    row3!(&mut rows, "bool (clustered, ~20% true runs)", N, bool_runs_low, pco_bool);

    print_comparison_table(&rows);
    println!();
}

/// bool -> u8 is a 1:1 value mapping (a pco u8 column), not a decomposition.
/// 8-bit enabled: the fair raw-pco config.
fn pco_bool(d: &Vec<bool>) -> (f64, usize) {
    let v: Vec<u8> = d.iter().map(|b| *b as u8).collect();
    bench_pco_col_8bit(&v)
}

// Option<i32> vs sentinel: the present+values decomposition is PcoPack-only.
// Each Option row pairs with a sentinel row (0 = "none") of the same data.

fn option_table() {
    println!("### Option<i32> vs sentinel\n");
    let mut rows: Vec<SlimRow> = Vec::new();
    row2!(&mut rows, "Option<i32> (80% null, clustered)", N, |n| option_i32_data(n, 0.8, false));
    row2!(&mut rows, "i32 (80% zero sentinel, clustered)", N, |n| sentinel_i32_data(n, 0.8, false));
    row2!(&mut rows, "Option<i32> (80% null, scattered)", N, |n| option_i32_data(n, 0.8, true));
    row2!(&mut rows, "i32 (80% zero sentinel, scattered)", N, |n| sentinel_i32_data(n, 0.8, true));
    row2!(&mut rows, "Option<i32> (90% null, clustered)", N, |n| option_i32_data(n, 0.9, false));
    row2!(&mut rows, "i32 (90% zero sentinel, clustered)", N, |n| sentinel_i32_data(n, 0.9, false));
    row2!(&mut rows, "Option<i32> (90% null, scattered)", N, |n| option_i32_data(n, 0.9, true));
    row2!(&mut rows, "i32 (90% zero sentinel, scattered)", N, |n| sentinel_i32_data(n, 0.9, true));
    print_slim_table(&rows);
    println!();
}

// Enums vs enum-as-integer: the type-safety tax. Enum rows (discriminant +
// payload columns) are PcoPack-only; integer-companion rows hold all codecs.

fn enums_table() {
    println!("### Enums vs enum-as-integer\n");
    let mut rows: Vec<ComparisonRow> = Vec::new();

    row3_nopco!(&mut rows, "Enum (simple, skewed 68/27/5)", N, generate_status_data);
    row3!(&mut rows, "u8 (skewed 68/27/5, plain integer)", N, status_disc_u8, |d: &Vec<u8>| bench_pco_col_8bit(d));
    row3_nopco!(&mut rows, "Enum (complex, repeating pattern + payloads)", N, generate_event_data);
    row3!(&mut rows, "u8 (repeating-pattern discriminant, plain integer)", N, event_disc_u8, |d: &Vec<u8>| {
        bench_pco_col_8bit(d)
    });

    print_comparison_table(&rows);
    println!();
}

// Strings & bytes: dynamic-size types have no pco serde API, so the
// comparison is PcoPack vs msgpack+zstd only.

fn strings_bytes_table() {
    println!("### Strings & bytes\n");
    let mut rows: Vec<SlimRow> = Vec::new();

    row2!(&mut rows, "String", N, generate_string_data);
    row2!(&mut rows, "SmolStr", N, generate_smol_str_data);
    row2!(&mut rows, "Uuid (v4 random)", N, generate_uuid_data);
    row2!(&mut rows, "ByteBuf (avg 128B)", N, generate_bytebuf_data);
    row2!(&mut rows, "Vec<String> (avg 4)", 50_000, generate_vec_string_data);
    row2!(&mut rows, "serde_json::Value", N, generate_json_value_data);
    row2!(&mut rows, "serde_json::Value (5 stable keys, 50k rows)", 50_000, generate_json_stable_data);

    print_slim_table(&rows);
    println!();
}

// Maps: PcoPack vs msgpack+zstd, dynamic-size keys and values. The four
// open-vocabulary tuple-key rows use native `vortex.map`: struct promotion
// cannot express them (tuple keys fail tracing; string keys would drop
// everything outside the 100-row sample schema, measured 25909 of 39872
// entries). The stable-vocabulary rows declare struct promotion: measured
// ~1.5-1.7x smaller than native (keys move into the schema; native keeps
// them in the entries column, varbin->zstd).

fn maps_table() {
    println!("### Maps\n");
    let mut rows: Vec<SlimRow> = Vec::new();

    row2!(&mut rows, "BTreeMap<String, i32>", 10_000, generate_btremap_string_data, native);
    row2!(&mut rows, "HashMap<String, i32>", 10_000, generate_hashmap_string_data, native);
    row2_smolstr!(&mut rows, "BTreeMap<SmolStr, i32>", 10_000, generate_btremap_smolstr_data, BTreeMap, native);
    row2_smolstr!(&mut rows, "HashMap<SmolStr, i32>", 10_000, generate_hashmap_smolstr_data, HashMap, native);

    // Stable tag vocabulary (osm-tags shape): every row carries every key, so
    // struct promotion traces to one columnar payload per key. PcoPack
    // flattens to counts+keys+values regardless; skipping the keys is the win
    // this measures.
    let n = 10_000;
    row2!(&mut rows, "HashMap<SmolStr, i32> (8 stable keys)", n, generate_tagmap_dense_data);
    row2!(&mut rows, "HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null)", n, generate_tagmap_sparse_data);

    // Chunk-sized walk rows, same declared struct layout: each key's value
    // autocorrelates along row order (the time-sorted indexed-capture
    // shape). Per-key columns separate signal that one interleaved values
    // stream dilutes.
    row2!(&mut rows, "HashMap<SmolStr, i32> (8 stable keys, walk)", CHUNK_ROWS, generate_walkmap_data);
    row2!(
        &mut rows,
        "HashMap<SmolStr, Option<i32>> (8 stable keys, ~60% null, walk)",
        CHUNK_ROWS,
        generate_walkmap_sparse_data
    );

    print_slim_table(&rows);
    println!();
}

// Floats (float_round): a per-column lossy knob. Values round to N decimals
// into a quantized i64 column; other codecs compress the exact values.

fn floats_table() {
    println!("### Floats (float_round)\n");
    let mut rows: Vec<ComparisonRow> = Vec::new();

    // Other codecs have no quantization knob: compute once per type.
    let f64_data = normal_f64(N);
    let (f64_p_t, f64_p_s) = bench_pco_col(&f64_data);
    let f64_mp_s = to_msgpack_zstd(&f64_data).len();
    let f64_mp_t = bench_msgpack(&f64_data);
    let (f64_vx_s, f64_vx_t) = vortex_cell(&f64_data);
    for round in [0u32, 1, 2] {
        let label = if round == 0 {
            "f64 (normal, no round)".to_string()
        } else {
            format!("f64 (normal, float_round={round})")
        };
        let expect = |v: &f64| if round == 0 { *v } else { f64_rounded(*v, round) };
        let (_name, pp_t, pp_s) = bench_write_round(&label, N, normal_f64, round, Duration::default(), expect);
        rows.push(ComparisonRow {
            type_name: format!("`{label}`"),
            pco_size: format_bytes(f64_p_s),
            pco_pack_size: format_bytes(pp_s),
            mp_size: format_bytes(f64_mp_s),
            vortex_size: f64_vx_s.clone(),
            pco_time: format_ms(f64_p_t),
            pco_pack_time: format_ms(pp_t),
            mp_time: format_ms(f64_mp_t),
            vortex_time: f64_vx_t.clone(),
        });
    }

    let f32_data = normal_f32(N);
    let (f32_p_t, f32_p_s) = bench_pco_col(&f32_data);
    let f32_mp_s = to_msgpack_zstd(&f32_data).len();
    let f32_mp_t = bench_msgpack(&f32_data);
    let (f32_vx_s, f32_vx_t) = vortex_cell(&f32_data);
    for round in [0u32, 1, 2] {
        let label = if round == 0 {
            "f32 (normal, no round)".to_string()
        } else {
            format!("f32 (normal, float_round={round})")
        };
        let expect = |v: &f32| if round == 0 { *v } else { f32_rounded(*v, round) };
        let (_name, pp_t, pp_s) = bench_write_round(&label, N, normal_f32, round, Duration::default(), expect);
        rows.push(ComparisonRow {
            type_name: format!("`{label}`"),
            pco_size: format_bytes(f32_p_s),
            pco_pack_size: format_bytes(pp_s),
            mp_size: format_bytes(f32_mp_s),
            vortex_size: f32_vx_s.clone(),
            pco_time: format_ms(f32_p_t),
            pco_pack_time: format_ms(pp_t),
            mp_time: format_ms(f32_mp_t),
            vortex_time: f32_vx_t.clone(),
        });
    }

    print_comparison_table(&rows);
    println!();
}

/// The exact value a float_round=N write stores and a read returns (the
/// same arithmetic as src/float.rs: scale, round, i64, unround).
fn f64_rounded(v: f64, n: u32) -> f64 {
    let scale = 10i64.pow(n) as f64;
    ((v * scale).round().clamp(i64::MIN as f64, i64::MAX as f64) as i64) as f64 / scale
}

/// Same as [`f64_rounded`] for f32 (scaled through f64, as src/float.rs
/// does).
fn f32_rounded(v: f32, n: u32) -> f32 {
    let scale = 10i64.pow(n) as f64;
    (((v as f64 * scale).round().clamp(i64::MIN as f64, i64::MAX as f64) as i64) as f64 / scale) as f32
}

// Timestamps (time_round): 60s-step per-minute series, +/-15s jitter, us
// times; the lossy round drops sub-granularity. pco (i64 us) is 1:1 here.

fn timestamps_table() {
    println!("### Timestamps (time_round)\n");
    let mut rows: Vec<ComparisonRow> = Vec::new();

    let data = generate_datetime_data(N);
    let micros: Vec<i64> = data.iter().map(|dt| dt.timestamp_micros()).collect();
    let (p_t, p_s) = bench_pco_col(&micros);
    let mp_s = to_msgpack_zstd(&data).len();
    let mp_t = bench_msgpack(&data);
    let (vx_s, vx_t) = vortex_cell(&data);

    for (label, g) in [
        ("DateTime (60s step, no round)", 0i64),
        ("DateTime (60s step, time_round=1s)", 1_000_000i64),
        ("DateTime (60s step, time_round=60s)", 60_000_000i64),
    ] {
        let dur = if g == 0 { Duration::default() } else { Duration::microseconds(g) };
        let expect = |dt: &DateTime<Utc>| ts_rounded(dt, g);
        let (_name, pp_t, pp_s) = bench_write_round(label, N, generate_datetime_data, 0, dur, expect);
        rows.push(ComparisonRow {
            type_name: format!("`{label}`"),
            pco_size: format_bytes(p_s),
            pco_pack_size: format_bytes(pp_s),
            mp_size: format_bytes(mp_s),
            vortex_size: vx_s.clone(),
            pco_time: format_ms(p_t),
            pco_pack_time: format_ms(pp_t),
            mp_time: format_ms(mp_t),
            vortex_time: vx_t.clone(),
        });
    }

    print_comparison_table(&rows);
    println!();
}

/// The exact instant a time_round write stores: rounded to the nearest
/// multiple of the granularity (chrono DurationRound behavior).
fn ts_rounded(dt: &DateTime<Utc>, g: i64) -> DateTime<Utc> {
    if g == 0 {
        return *dt;
    }
    let micros = dt.timestamp_micros();
    let q = micros / g;
    let r = micros % g;
    let half = g / 2;
    let q = if r >= half || (r < 0 && (-r) > half) { q + 1 } else { q };
    DateTime::from_timestamp_micros(q * g).unwrap()
}

// Generators: one shared transformation per distribution (splitmix64-seeded).

// --- Booleans -------------------------------------------------------------

/// i.i.d. 50/50 draws; a cycling pattern would be near-zero-entropy and let
/// zstd back-references win unrealistically.
fn bool_iid_50(n: usize) -> Vec<bool> {
    (0..n).map(|i| unit64(i) < 0.5).collect()
}

/// i.i.d. 90/10: a skewed bool (an "is_active" column shape).
fn bool_iid_90(n: usize) -> Vec<bool> {
    (0..n).map(|i| unit64(i) < 0.9).collect()
}

/// Alternating true/false (period 2): pure sequential redundancy.
fn bool_alternating(n: usize) -> Vec<bool> {
    (0..n).map(|i| i % 2 == 0).collect()
}

/// i.i.d. 95/5: scattered low-fraction minority; entropy floor 0.29 bits/val.
fn bool_iid_5(n: usize) -> Vec<bool> {
    (0..n).map(|i| unit64(i) < 0.05).collect()
}

/// Clustered low-fraction: ~20% true in short runs of 4-11 inside false
/// stretches (bursty activity); run material for zstd at a low fraction.
fn bool_runs_low(n: usize) -> Vec<bool> {
    let mut out = Vec::with_capacity(n);
    let mut b = 0usize;
    while out.len() < n {
        for _ in 0..(20 + (splitmix64(b.wrapping_mul(104729)) % 40) as usize) {
            // false 20..59
            if out.len() == n {
                return out;
            }
            out.push(false);
        }
        for _ in 0..(4 + (splitmix64(b.wrapping_mul(1299709)) % 8) as usize) {
            // true 4..11
            if out.len() == n {
                return out;
            }
            out.push(true);
        }
        b += 1;
    }
    out
}

/// Clustered runs: ~30-row blocks of one value (a service's active window),
/// the placement zstd back-references exploit.
fn bool_runs(n: usize) -> Vec<bool> {
    let mut out = Vec::with_capacity(n);
    let mut b = 0usize;
    while out.len() < n {
        let v = unit64(b) < 0.5;
        let len = 10 + (splitmix64(b.wrapping_mul(7919)) % 40) as usize; // 10..49, avg ~29
        for _ in 0..len {
            if out.len() == n {
                return out;
            }
            out.push(v);
        }
        b += 1;
    }
    out
}

// --- Counters (increment + noise) ------------------------------------------

/// Cyclic ramp over ~20% of the i16 range (a minute-of-day-like field) plus
/// symmetric +/-noise: delta annihilates the ramp, the residual is the noise.
fn counter_i16(n: usize) -> Vec<i16> {
    (0..n).map(|i| ((i % 13_107) as i64 + noise15(i)) as i16).collect()
}

/// Monotonic counter (a sequence number) + +/-noise.
fn counter_i64(n: usize) -> Vec<i64> {
    (0..n).map(|i| i as i64 + noise15(i)).collect()
}

/// Full-width 8-bit counter (wrapping): the 8-bit path on a compressible
/// shape.
fn counter_u8(n: usize) -> Vec<u8> {
    (0..n)
        .map(|i| {
            let v = (i % 256) as u8;
            let nz = (noise15(i) as i32 & 0xFF) as u8;
            v.wrapping_add(nz)
        })
        .collect()
}

/// Symmetric +/-noise: the counter perturbation.
fn noise15(i: usize) -> i64 {
    (splitmix64(i) % 31) as i64 - 15
}

// --- Uniform random, full width --------------------------------------------

fn uniform_u8(n: usize) -> Vec<u8> {
    (0..n).map(|i| splitmix64(i) as u8).collect()
}

fn uniform_i16(n: usize) -> Vec<i16> {
    (0..n).map(|i| (splitmix64(i) % 65_536) as i16).collect()
}

fn uniform_i32(n: usize) -> Vec<i32> {
    (0..n).map(|i| (splitmix64(i) as u32) as i32).collect()
}

fn uniform_i64(n: usize) -> Vec<i64> {
    (0..n).map(|i| splitmix64(i) as i64).collect()
}

// --- Uniform 0..1000, random order -----------------------------------------

fn uniform1000_i16(n: usize) -> Vec<i16> {
    (0..n).map(|i| (splitmix64(i) % 1000) as i16).collect()
}

fn uniform1000_i32(n: usize) -> Vec<i32> {
    (0..n).map(|i| (splitmix64(i) % 1000) as i32).collect()
}

// --- Tiny alphabet: i.i.d. floor + dwell sweep -------------------------------

/// i.i.d. draws from a skewed 4-value alphabet (dwell = 1): frequency-model
/// material, no runs to exploit.
fn states_u8(n: usize) -> Vec<u8> {
    const VALUES: [u8; 4] = [0, 1, 3, 4];
    const CDF: [f64; 4] = [0.6751, 0.9528, 0.9999999, 1.0];
    (0..n).map(|i| VALUES[cdf_index(&CDF, unit64(i))]).collect()
}

/// Dwell sweep: alphabet {0,1,2}, i.i.d. value per run, run lengths uniform
/// in dwell/2 .. 3*dwell/2, no adjacent repeats. Isolates run length, the
/// axis fixed-cycle generators could not show: LZ exploits run length, not
/// just the small alphabet.
fn dwell_u8(n: usize, dwell: usize) -> Vec<u8> {
    const VALUES: [u8; 3] = [0, 1, 2];
    let mut out = Vec::with_capacity(n);
    let mut b = 0usize;
    let mut prev = 255u8;
    while out.len() < n {
        let mut v = VALUES[(splitmix64(b.wrapping_mul(7919) ^ 0xD9E1) % 3) as usize];
        if v == prev {
            v = VALUES[(v as usize + 1) % 3];
        }
        prev = v;
        let len = ((dwell / 2) + (splitmix64(b.wrapping_mul(104729) ^ 0x7A) % dwell as u64) as usize).max(1);
        for _ in 0..len {
            if out.len() == n {
                return out;
            }
            out.push(v);
        }
        b += 1;
    }
    out
}

// --- Distinct-count sweep ------------------------------------------------------

/// i.i.d. draws from a pool of `d` distinct 64-bit ids: the distinct-count
/// axis at the writer chunk length, where dictionary/LZ and bitpack territory
/// cross over as `d` grows.
fn pool_iid_i64(n: usize, d: usize) -> Vec<i64> {
    let pool: Vec<i64> = (0..d).map(|j| splitmix64(j ^ 0xC0FFEE) as i64).collect();
    (0..n).map(|i| pool[(splitmix64(i ^ 0xBADF00D) % d as u64) as usize]).collect()
}

/// Placement pair: one multiset of 574 distinct ids with Zipf(0.7) skewed
/// counts, emitted grouped into runs or deterministically shuffled (seeded
/// Fisher-Yates). Any winner change between the two rows is placement, not
/// marginal.
fn placement_pair_d574(n: usize, grouped: bool) -> Vec<i64> {
    const D: usize = 574;
    let pool: Vec<i64> = (0..D).map(|j| splitmix64(j ^ 0xF16E5) as i64).collect();
    let w: Vec<f64> = (1..=D).map(|j| (j as f64).powf(-0.7)).collect();
    let t: f64 = w.iter().sum();
    let mut counts: Vec<usize> = w.iter().map(|x| ((x / t * n as f64) as usize).max(1)).collect();
    let sum: usize = counts.iter().sum();
    if sum < n {
        // truncation loses fractional counts; the top (dominant) value absorbs
        let d = n - sum;
        counts[0] += d;
    } else if sum > n {
        // .max(1) inflation: trim from the tail, floor 1
        let mut extra = sum - n;
        for c in counts.iter_mut().skip(1).rev() {
            if extra == 0 {
                break;
            }
            let take = (*c - 1).min(extra);
            *c -= take;
            extra -= take;
        }
        counts[0] -= extra.min(counts[0]);
    }
    let mut out: Vec<i64> = Vec::with_capacity(n);
    for (j, &c) in counts.iter().enumerate() {
        for _ in 0..c {
            out.push(pool[j]);
        }
    }
    out.truncate(n);
    if !grouped {
        for i in (1..out.len()).rev() {
            let j = (splitmix64(i ^ 0x51E7) % (i as u64 + 1)) as usize;
            out.swap(i, j);
        }
    }
    out
}

/// Recurrence-distance pair: same 4096-value alphabet and distinct count.
/// One side repeats values at ~512-row spacing (i.i.d. inside fresh local
/// windows of 8); the other at exactly 4096-row spacing. Repeat spacing
/// decides whether LZ match tokens stay cheap.
fn recurrence_i64(n: usize, local: bool) -> Vec<i64> {
    const D: usize = 4096;
    const LOCAL_POOL: usize = 8;
    let pool: Vec<i64> = (0..D).map(|j| splitmix64(j ^ 0x2B1E5) as i64).collect();
    if local {
        (0..n)
            .map(|i| {
                let block = i / 512;
                pool[(block * LOCAL_POOL + (splitmix64(i ^ 0x1F) % LOCAL_POOL as u64) as usize) % D]
            })
            .collect()
    } else {
        (0..n).map(|i| pool[i % D]).collect()
    }
}

// --- Right-skew counters -------------------------------------------------------

fn skew_i64(n: usize) -> Vec<i64> {
    (0..n).map(|i| skew_small(i)).collect()
}

fn skew_sparse_i64(n: usize) -> Vec<i64> {
    (0..n).map(|i| if unit64(i) < 0.17 { 0 } else { skew_small(i.wrapping_add(0x77)) }).collect()
}

// --- Sparsity: scattered vs clustered windows -----------------------------

/// Same marginal: 93% zeros scattered i.i.d., heavy tail where the non-zeros
/// fall; the placement-decides control.
fn zero_spike_i64(n: usize) -> Vec<i64> {
    (0..n)
        .map(|i| {
            if unit64(i) < 0.931 {
                0
            } else {
                let u = unit64(i.wrapping_add(0x77));
                ((1.0 - u).powf(-2.0)).min(20_655_873.0) as i64
            }
        })
        .collect()
}

/// Same zero fraction, clustered placement: long zero windows with short tail
/// runs between. The runs are LZ material the scattered control cannot offer
/// (the synthetic form of the section 2 clustered-io finding).
fn zero_window_i64(n: usize) -> Vec<i64> {
    let mut out = Vec::with_capacity(n);
    let mut b = 0usize;
    while out.len() < n {
        let zeros = 512 + (splitmix64(b.wrapping_mul(7919) ^ 0x0E) % 3072) as usize; // avg ~2k
        for _ in 0..zeros {
            if out.len() == n {
                return out;
            }
            out.push(0);
        }
        let tail = 1 + (splitmix64(b.wrapping_mul(104729) ^ 0x0F) % 300) as usize; // avg ~150
        for _j in 0..tail {
            if out.len() == n {
                return out;
            }
            let u = unit64(out.len() * 7 + b);
            out.push(((1.0 - u).powf(-2.0)).min(20_655_873.0) as i64);
        }
        b += 1;
    }
    out
}

// --- Enums (same distributions as the Enums table's integer rows) -------------------

/// Skewed small enum: a states-like measured marginal (68% top value, near
/// random order); the shape pco's frequency model wins on.
fn generate_status_data(n: usize) -> Vec<Status> {
    const CDF: [f64; 3] = [0.68, 0.95, 1.0];
    (0..n)
        .map(|i| match cdf_index(&CDF, unit64(i)) {
            0 => Status::Active,
            1 => Status::Inactive,
            _ => Status::Pending,
        })
        .collect()
}

/// The Status distribution as a plain u8 column (same draws).
fn status_disc_u8(n: usize) -> Vec<u8> {
    const CDF: [f64; 3] = [0.68, 0.95, 1.0];
    (0..n).map(|i| cdf_index(&CDF, unit64(i)) as u8).collect()
}

/// Repeating-pattern enum (wait_event_types-like marginal) in multi-value
/// "snapshot" patterns; payloads are realistic counters and low-card keys.
fn generate_event_data(n: usize) -> Vec<Event> {
    let disc = gen_repeating(n, &[0u8, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3], &[0u8, 1, 2, 3], &[0.34, 0.30, 0.20, 0.16]);
    (0..n)
        .map(|i| match disc[i] {
            0 => Event::Unknown,
            1 => Event::Click(Click { x: skew_small(i) as i32, y: skew_small(i.wrapping_add(7)) as i32 }),
            2 => Event::KeyPress(KeyPress { key: format!("key_{}", splitmix64(i.wrapping_add(13)) % 1000) }),
            _ => Event::Resize,
        })
        .collect()
}

/// The Event discriminant distribution as a plain u8 column (same draws).
fn event_disc_u8(n: usize) -> Vec<u8> {
    gen_repeating(n, &[0u8, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3], &[0u8, 1, 2, 3], &[0.34, 0.30, 0.20, 0.16])
}

// --- Option / sentinel: scattered vs clustered zeros. Row kinds hold
// identical draws; only zero fraction and placement vary.

/// Zero-position mask + per-row value draw (`skew_small`) shared by the
/// Option and sentinel rows. `frac` = zero fraction; `scattered` = placement.
fn zero_option_parts(n: usize, frac: f64, scattered: bool) -> (Vec<bool>, Vec<i32>) {
    let num_zero = (n as f64 * frac).round() as usize;
    let mut is_zero = vec![false; n];
    if scattered {
        // Rank every position by a splitmix64 hash; the `num_zero` lowest
        // ranks are the zero positions (a deterministic random permutation).
        let mut ranked: Vec<(u64, usize)> = (0..n).map(|i| (splitmix64(i), i)).collect();
        ranked.sort_unstable_by_key(|&(h, _)| h);
        for &(_, i) in ranked.iter().take(num_zero) {
            is_zero[i] = true;
        }
    } else {
        for i in 0..num_zero {
            is_zero[i] = true;
        }
    }
    let values: Vec<i32> = (0..n).map(|i| skew_small(i) as i32).collect();
    (is_zero, values)
}

fn option_i32_data(n: usize, frac: f64, scattered: bool) -> Vec<Option<i32>> {
    let (is_zero, values) = zero_option_parts(n, frac, scattered);
    is_zero.iter().zip(values.iter()).map(|(&z, v)| if z { None } else { Some(*v) }).collect()
}

fn sentinel_i32_data(n: usize, frac: f64, scattered: bool) -> Vec<i32> {
    let (is_zero, values) = zero_option_parts(n, frac, scattered);
    is_zero.iter().zip(values.iter()).map(|(&z, v)| if z { 0 } else { *v }).collect()
}

// --- Strings / bytes / JSON -------------------------------------------------------------

fn generate_string_data(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("string_{}_{}", i % 5000, splitmix64(i) % 16)).collect()
}

fn generate_smol_str_data(n: usize) -> Vec<smol_str::SmolStr> {
    (0..n).map(|i| smol_str::SmolStr::from(format!("string_{}_{}", i % 5000, splitmix64(i) % 16).leak())).collect()
}

/// Truly random (v4) UUIDs: incompressible 128 bits (`uuid`'s `v4` feature,
/// dev-dependency only); sizes stay stable across runs.
fn generate_uuid_data(n: usize) -> Vec<uuid::Uuid> {
    (0..n).map(|_| uuid::Uuid::new_v4()).collect()
}

/// Random byte payloads (real payloads don't repeat one byte): 1-256 bytes.
fn generate_bytebuf_data(n: usize) -> Vec<ByteBuf> {
    (0..n)
        .map(|i| {
            let len = (i % 256) + 1;
            ByteBuf::from((0..len).map(|j| splitmix64(i * 257 + j) as u8).collect::<Vec<u8>>())
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

/// The same five keys in every row, homogeneous types, `note` null on every
/// fourth row. `from_samples` traces this into a struct, so Vortex
/// columnarizes it exactly like the stable-key map rows.
fn generate_json_stable_data(n: usize) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value, json};
    let notes = ["", "ok", "warn", "error"];
    (0..n)
        .map(|i| {
            let mut m = Map::new();
            m.insert("level".into(), json!((i % 5) as i64));
            m.insert("count".into(), json!(skew_small(i)));
            m.insert("ok".into(), json!(unit64(i.wrapping_mul(7).wrapping_add(3)) < 0.7));
            m.insert("score".into(), json!((lognorm_small(i) * 100.0).round() / 100.0));
            match notes[i % notes.len()] {
                "" => m.insert("note".into(), Value::Null),
                s => m.insert("note".into(), json!(s)),
            };
            Value::Object(m)
        })
        .collect()
}

fn generate_json_value_data(n: usize) -> Vec<serde_json::Value> {
    use serde_json::json;
    (0..n)
        .map(|i| {
            match i % 5 {
                0 => json!({"id": i, "name": format!("name_{}", splitmix64(i.wrapping_add(0x11)) % 1_000_000), "score": lognorm_small(i)}),
                1 => json!({"tags": vec!["a", "b", "c"], "count": skew_small(i)}),
                2 => json!(format!("value_{}_{}", i, splitmix64(i.wrapping_add(0x22)) % 256)),
                3 => json!(null),
                _ => json!(lognorm_small(i) * 10.0),
            }
        })
        .collect()
}

// --- Maps -----------------------------------------------------------------------------

fn generate_btremap_string_data(n: usize) -> Vec<BTreeMap<(String, i32), i32>> {
    (0..n)
        .map(|i| {
            let mut map = BTreeMap::new();
            for j in 0..((i % 5) + 1) {
                let k = (splitmix64(i.wrapping_add(j * 131)) % 1000) as i32;
                map.insert((format!("key_{}_{}", j, k), k), skew_small(i.wrapping_add(j * 7)) as i32);
            }
            map
        })
        .collect()
}

fn generate_hashmap_string_data(n: usize) -> Vec<HashMap<(String, i32), i32>> {
    (0..n)
        .map(|i| {
            let mut map = HashMap::new();
            for j in 0..((i % 5) + 1) {
                let k = (splitmix64(i.wrapping_add(j * 131)) % 1000) as i32;
                map.insert((format!("key_{}_{}", j, k), k), skew_small(i.wrapping_add(j * 7)) as i32);
            }
            map
        })
        .collect()
}

fn generate_btremap_smolstr_data(n: usize) -> Vec<BTreeMap<(smol_str::SmolStr, i32), i32>> {
    (0..n)
        .map(|i| {
            let mut map = BTreeMap::new();
            for j in 0..((i % 5) + 1) {
                let k = (splitmix64(i.wrapping_add(j * 131)) % 1000) as i32;
                map.insert(
                    (smol_str::SmolStr::from(format!("key_{}_{}", j, k).leak()), k),
                    skew_small(i.wrapping_add(j * 7)) as i32,
                );
            }
            map
        })
        .collect()
}

fn generate_hashmap_smolstr_data(n: usize) -> Vec<HashMap<(smol_str::SmolStr, i32), i32>> {
    (0..n)
        .map(|i| {
            let mut map = HashMap::new();
            for j in 0..((i % 5) + 1) {
                let k = (splitmix64(i.wrapping_add(j * 131)) % 1000) as i32;
                map.insert(
                    (smol_str::SmolStr::from(format!("key_{}_{}", j, k).leak()), k),
                    skew_small(i.wrapping_add(j * 7)) as i32,
                );
            }
            map
        })
        .collect()
}

/// A short closed tag vocabulary: keys duplicate on every row (the osm-tags
/// shape), which is what map_as_struct columnarization rewards.
fn tag_keys() -> [smol_str::SmolStr; 8] {
    [
        "name".into(),
        "amenity".into(),
        "highway".into(),
        "building".into(),
        "shop".into(),
        "surface".into(),
        "lit".into(),
        "width".into(),
    ]
}

/// Stable-vocabulary dense map: every row carries all 8 keys.
fn generate_tagmap_dense_data(n: usize) -> Vec<HashMap<smol_str::SmolStr, i32>> {
    let keys = tag_keys();
    (0..n)
        .map(|i| {
            keys.iter()
                .enumerate()
                .map(|(j, k)| (k.clone(), skew_small(i.wrapping_mul(31).wrapping_add(j * 7)) as i32))
                .collect()
        })
        .collect()
}

/// Sparse map: all 8 keys stay present (a struct mirror cannot express
/// absence, only null), ~60% of values are None.
fn generate_tagmap_sparse_data(n: usize) -> Vec<HashMap<smol_str::SmolStr, Option<i32>>> {
    let keys = tag_keys();
    (0..n)
        .map(|i| {
            keys.iter()
                .enumerate()
                .map(|(j, k)| {
                    let v = if unit64(i.wrapping_mul(7919).wrapping_add(j * 131)) < 0.4 {
                        Some(skew_small(i.wrapping_add(j * 7)) as i32)
                    } else {
                        None
                    };
                    (k.clone(), v)
                })
                .collect()
        })
        .collect()
}

/// Stable keys, autocorrelated values ("walk"): each key's value drifts on
/// ~5% of rows, sticky state between changes.
fn generate_walkmap_data(n: usize) -> Vec<HashMap<smol_str::SmolStr, i32>> {
    let keys = tag_keys();
    let mut cur = [0i32; 8];
    for (j, c) in cur.iter_mut().enumerate() {
        *c = (splitmix64(1_000 + j) % 50) as i32;
    }
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        for j in 0..8 {
            if unit64(i.wrapping_mul(7919).wrapping_add(j * 131)) < 0.05 {
                cur[j] = (cur[j] + (splitmix64(i.wrapping_mul(31).wrapping_add(j)) % 9) as i32 - 4).max(0);
            }
        }
        let mut row = HashMap::with_capacity(8);
        for (j, k) in keys.iter().enumerate() {
            row.insert(k.clone(), cur[j]);
        }
        rows.push(row);
    }
    rows
}

/// Same autocorrelated walk, ~60% of cells null (the struct mirror encodes
/// absence as null; absent-vs-null collapses).
fn generate_walkmap_sparse_data(n: usize) -> Vec<HashMap<smol_str::SmolStr, Option<i32>>> {
    let keys = tag_keys();
    let mut cur = [0i32; 8];
    for (j, c) in cur.iter_mut().enumerate() {
        *c = (splitmix64(0xC0FFEE + j) % 50) as i32;
    }
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        for j in 0..8 {
            if unit64(i.wrapping_mul(7919).wrapping_add(j * 131)) < 0.05 {
                cur[j] = (cur[j] + (splitmix64(i.wrapping_mul(31).wrapping_add(j)) % 9) as i32 - 4).max(0);
            }
        }
        let mut row = HashMap::with_capacity(8);
        for (j, k) in keys.iter().enumerate() {
            let v = if unit64(i.wrapping_mul(1_047_291).wrapping_add(j * 97)) < 0.4 { Some(cur[j]) } else { None };
            row.insert(k.clone(), v);
        }
        rows.push(row);
    }
    rows
}

// --- Floats (normal: a CLT sum of 12 uniforms, centered) -------------------------------

fn normal_f64(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let mut sum = 0.0f64;
            for k in 0..12 {
                sum += unit64(i.wrapping_add(k * 7919));
            }
            sum - 6.0
        })
        .collect()
}

fn normal_f32(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let mut sum = 0.0f32;
            for k in 0..12 {
                sum += unit64(i.wrapping_add(k * 7919)) as f32;
            }
            sum - 6.0
        })
        .collect()
}

// --- Timestamps ---------------------------------------------------------------------------

/// 60s-step per-minute series with +/-15s jitter and microsecond collection
/// times (the sub-second part is what time_round removes).
fn generate_datetime_data(n: usize) -> Vec<DateTime<Utc>> {
    let base = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    (0..n)
        .map(|i| {
            let jitter = (splitmix64(i) % 31) as i64 - 15;
            let micros = (splitmix64(i.wrapping_add(7)) % 1_000_000) as i64;
            base + Duration::seconds(i as i64 * 60 + jitter) + Duration::microseconds(micros)
        })
        .collect()
}
