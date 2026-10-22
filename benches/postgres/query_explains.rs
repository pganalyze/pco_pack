//! query_explains: the full query_explains_7d window (237,651 analyzed
//! queries). Mirrors the source table exactly (field order/names/types) so
//! the chunk layout matches how the table is stored: index [server_id],
//! timestamp seen_at. UUID remapping and fingerprint folding are done by
//! src/bin's extractor.
//!
//! Every row carries the full EXPLAIN (FORMAT JSON) document, averaging ~9
//! KB of nested JSON: the payload is one zstd contest over `output_json`.
//! The float columns carry millisecond values as decimals; the lossy variant
//! rounds seen_at to whole seconds and the EXPLAIN floats to 2 decimals.
//!
//! Five PcoPack scenarios run against the same rows, printed as their own
//! table, then the non-PcoPack crates in a second table: string keeps the
//! JSON payload as a plain string (the way Vortex stores it); JSON keeps the
//! `serde_json::Value` tree; arena replaces `output_json` with the flattened
//! typed arena of `arena.rs` (one row-local `Vec<PlanNode>` per plan plus
//! typed `Workers` rows, numerically exact); arena lossy rides the lossy
//! mirror of `arena_lossy.rs`. Every scenario owns the rows it writes, so
//! no serialize pass pays a deep input clone. The full-capture exactness
//! guard for the arena lives in its verify closure; the schema's own corpus
//! guards sit in `tests/query_explains_arena.rs`.

use chrono::{DateTime, Utc};
use pco_pack::PcoPack;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

use super::arena;
use super::arena_lossy;
use super::common::{self, CodecRowOther, CodecRowPcoPack, cell};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, PcoPack)]
#[pco_pack(index = [server_id], timestamp = seen_at)]
pub struct QueryExplain {
    pub server_id: i64,
    pub database_id: i64,
    pub postgres_role_id: i64,
    pub query_fingerprint: i64,
    pub format: i32,
    pub source: i32,
    pub error: String,
    pub seen_at: DateTime<Utc>,
    pub runtime_ms: f32,
    pub total_cost: Option<f32>,
    pub total_shared_blks_hit: Option<i64>,
    pub total_shared_blks_read: Option<i64>,
    pub total_shared_blks_written: Option<i64>,
    pub total_shared_blks_dirtied: Option<i64>,
    pub total_blk_read_time: Option<f32>,
    pub total_blk_write_time: Option<f32>,
    pub plan_node_types: Vec<SmolStr>,
    pub plan_fingerprint: Option<i64>,
    pub output_text: Option<String>,
    pub output_json: Option<serde_json::Value>,
    pub node_insights: Option<serde_json::Value>,
}

/// The string mirror (`text` column): `output_json` and `node_insights`
/// ride as plain strings, the way Vortex's arrow schema carries the payload.
#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(index = [server_id], timestamp = seen_at)]
pub struct QueryExplainString {
    pub server_id: i64,
    pub database_id: i64,
    pub postgres_role_id: i64,
    pub query_fingerprint: i64,
    pub format: i32,
    pub source: i32,
    pub error: String,
    pub seen_at: DateTime<Utc>,
    pub runtime_ms: f32,
    pub total_cost: Option<f32>,
    pub total_shared_blks_hit: Option<i64>,
    pub total_shared_blks_read: Option<i64>,
    pub total_shared_blks_written: Option<i64>,
    pub total_shared_blks_dirtied: Option<i64>,
    pub total_blk_read_time: Option<f32>,
    pub total_blk_write_time: Option<f32>,
    pub plan_node_types: Vec<SmolStr>,
    pub plan_fingerprint: Option<i64>,
    pub output_text: Option<String>,
    pub output_json: Option<String>,
    pub node_insights: Option<String>,
}

impl From<&QueryExplain> for QueryExplainString {
    fn from(r: &QueryExplain) -> Self {
        QueryExplainString {
            server_id: r.server_id,
            database_id: r.database_id,
            postgres_role_id: r.postgres_role_id,
            query_fingerprint: r.query_fingerprint,
            format: r.format,
            source: r.source,
            error: r.error.clone(),
            seen_at: r.seen_at,
            runtime_ms: r.runtime_ms,
            total_cost: r.total_cost,
            total_shared_blks_hit: r.total_shared_blks_hit,
            total_shared_blks_read: r.total_shared_blks_read,
            total_shared_blks_written: r.total_shared_blks_written,
            total_shared_blks_dirtied: r.total_shared_blks_dirtied,
            total_blk_read_time: r.total_blk_read_time,
            total_blk_write_time: r.total_blk_write_time,
            plan_node_types: r.plan_node_types.clone(),
            plan_fingerprint: r.plan_fingerprint,
            output_text: r.output_text.clone(),
            output_json: r.output_json.as_ref().map(serde_json::Value::to_string),
            node_insights: r.node_insights.as_ref().map(serde_json::Value::to_string),
        }
    }
}

/// The lossy model (`JSON lossy` column): `time_round = 1 s` on
/// seen_at, `float_round = 2` on the EXPLAIN millisecond floats (rounding
/// at the hundredths-of-ms level).
#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(
    index = [server_id],
    timestamp = seen_at,
    time_round = chrono::Duration::seconds(1),
    float_round = 2
)]
pub struct QueryExplainLossy {
    pub server_id: i64,
    pub database_id: i64,
    pub postgres_role_id: i64,
    pub query_fingerprint: i64,
    pub format: i32,
    pub source: i32,
    pub error: String,
    pub seen_at: DateTime<Utc>,
    pub runtime_ms: f32,
    pub total_cost: Option<f32>,
    pub total_shared_blks_hit: Option<i64>,
    pub total_shared_blks_read: Option<i64>,
    pub total_shared_blks_written: Option<i64>,
    pub total_shared_blks_dirtied: Option<i64>,
    pub total_blk_read_time: Option<f32>,
    pub total_blk_write_time: Option<f32>,
    pub plan_node_types: Vec<SmolStr>,
    pub plan_fingerprint: Option<i64>,
    pub output_text: Option<String>,
    pub output_json: Option<serde_json::Value>,
    pub node_insights: Option<serde_json::Value>,
}

impl From<&QueryExplain> for QueryExplainLossy {
    fn from(r: &QueryExplain) -> Self {
        QueryExplainLossy {
            server_id: r.server_id,
            database_id: r.database_id,
            postgres_role_id: r.postgres_role_id,
            query_fingerprint: r.query_fingerprint,
            format: r.format,
            source: r.source,
            error: r.error.clone(),
            seen_at: r.seen_at,
            runtime_ms: r.runtime_ms,
            total_cost: r.total_cost,
            total_shared_blks_hit: r.total_shared_blks_hit,
            total_shared_blks_read: r.total_shared_blks_read,
            total_shared_blks_written: r.total_shared_blks_written,
            total_shared_blks_dirtied: r.total_shared_blks_dirtied,
            total_blk_read_time: r.total_blk_read_time,
            total_blk_write_time: r.total_blk_write_time,
            plan_node_types: r.plan_node_types.clone(),
            plan_fingerprint: r.plan_fingerprint,
            output_text: r.output_text.clone(),
            output_json: r.output_json.clone(),
            node_insights: r.node_insights.clone(),
        }
    }
}

/// The arena model: the same table with `output_json` replaced by the
/// flattened `PlanArena`. Non-JSON columns ride unchanged, so the same
/// filter queries apply. Building the mirror is the codec input transform
/// and, like the lossy mirror, happens outside the timed passes.
#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(index = [server_id], timestamp = seen_at)]
pub struct QueryExplainArena {
    pub server_id: i64,
    pub database_id: i64,
    pub postgres_role_id: i64,
    pub query_fingerprint: i64,
    pub format: i32,
    pub source: i32,
    pub error: String,
    pub seen_at: DateTime<Utc>,
    pub runtime_ms: f32,
    pub total_cost: Option<f32>,
    pub total_shared_blks_hit: Option<i64>,
    pub total_shared_blks_read: Option<i64>,
    pub total_shared_blks_written: Option<i64>,
    pub total_shared_blks_dirtied: Option<i64>,
    pub total_blk_read_time: Option<f32>,
    pub total_blk_write_time: Option<f32>,
    pub plan_node_types: Vec<SmolStr>,
    pub plan_fingerprint: Option<i64>,
    pub output_text: Option<String>,
    pub arena: Option<arena::PlanArena>,
    pub node_insights: Option<serde_json::Value>,
}

impl From<&QueryExplain> for QueryExplainArena {
    fn from(r: &QueryExplain) -> Self {
        QueryExplainArena {
            server_id: r.server_id,
            database_id: r.database_id,
            postgres_role_id: r.postgres_role_id,
            query_fingerprint: r.query_fingerprint,
            format: r.format,
            source: r.source,
            error: r.error.clone(),
            seen_at: r.seen_at,
            runtime_ms: r.runtime_ms,
            total_cost: r.total_cost,
            total_shared_blks_hit: r.total_shared_blks_hit,
            total_shared_blks_read: r.total_shared_blks_read,
            total_shared_blks_written: r.total_shared_blks_written,
            total_shared_blks_dirtied: r.total_shared_blks_dirtied,
            total_blk_read_time: r.total_blk_read_time,
            total_blk_write_time: r.total_blk_write_time,
            plan_node_types: r.plan_node_types.clone(),
            plan_fingerprint: r.plan_fingerprint,
            output_text: r.output_text.clone(),
            arena: r.output_json.as_ref().map(arena::PlanArena::flatten),
            node_insights: r.node_insights.clone(),
        }
    }
}

/// The arena lossy mirror: the same flattened arena on a row struct with
/// `time_round = 1 s` and `float_round = 2`, backed by `arena_lossy` where
/// every struct owning floats declares `float_round = 2` itself. Costs and
/// times quantize to two decimals; key presence, number kinds, and the
/// overflow structure stay intact. The verify closure is counts-only (the
/// rebuilt JSON legitimately differs from the stored floats); the lossy
/// filter counts fold through the same rounding as the JSON lossy column.
#[derive(Clone, Debug, PartialEq, PcoPack)]
#[pco_pack(
    index = [server_id],
    timestamp = seen_at,
    time_round = chrono::Duration::seconds(1),
    float_round = 2
)]
pub struct QueryExplainArenaLossy {
    pub server_id: i64,
    pub database_id: i64,
    pub postgres_role_id: i64,
    pub query_fingerprint: i64,
    pub format: i32,
    pub source: i32,
    pub error: String,
    pub seen_at: DateTime<Utc>,
    pub runtime_ms: f32,
    pub total_cost: Option<f32>,
    pub total_shared_blks_hit: Option<i64>,
    pub total_shared_blks_read: Option<i64>,
    pub total_shared_blks_written: Option<i64>,
    pub total_shared_blks_dirtied: Option<i64>,
    pub total_blk_read_time: Option<f32>,
    pub total_blk_write_time: Option<f32>,
    pub plan_node_types: Vec<SmolStr>,
    pub plan_fingerprint: Option<i64>,
    pub output_text: Option<String>,
    pub arena: Option<arena_lossy::PlanArena>,
    pub node_insights: Option<serde_json::Value>,
}

impl From<&QueryExplain> for QueryExplainArenaLossy {
    fn from(r: &QueryExplain) -> Self {
        QueryExplainArenaLossy {
            server_id: r.server_id,
            database_id: r.database_id,
            postgres_role_id: r.postgres_role_id,
            query_fingerprint: r.query_fingerprint,
            format: r.format,
            source: r.source,
            error: r.error.clone(),
            seen_at: r.seen_at,
            runtime_ms: r.runtime_ms,
            total_cost: r.total_cost,
            total_shared_blks_hit: r.total_shared_blks_hit,
            total_shared_blks_read: r.total_shared_blks_read,
            total_shared_blks_written: r.total_shared_blks_written,
            total_shared_blks_dirtied: r.total_shared_blks_dirtied,
            total_blk_read_time: r.total_blk_read_time,
            total_blk_write_time: r.total_blk_write_time,
            plan_node_types: r.plan_node_types.clone(),
            plan_fingerprint: r.plan_fingerprint,
            output_text: r.output_text.clone(),
            arena: r.output_json.as_ref().map(arena_lossy::PlanArena::flatten),
            node_insights: r.node_insights.clone(),
        }
    }
}

/// FNV-1a over a `Value` rendered with `PartialEq` semantics: numbers fold
/// to their f64 bits (so `Int(127)` and `Float(127.0)` fingerprint alike,
/// as the codec roundtrip allows), map keys iterate sorted. The verify
/// closure fingerprints every row's rebuild and every stored `output_json`,
/// then compares the sorted vectors: full-capture exactness, permutation-
/// invariant because indexed writes return rows index-grouped.
fn fingerprint(o: &Option<serde_json::Value>) -> u64 {
    fn step(h: u64, bytes: &[u8]) -> u64 {
        let mut h = h;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        h
    }
    fn walk(v: &serde_json::Value, h: u64) -> u64 {
        match v {
            serde_json::Value::Null => step(h, b"n"),
            serde_json::Value::Bool(b) => step(h, if *b { b"t" } else { b"f" }),
            serde_json::Value::Number(n) => step(h, &n.as_f64().unwrap_or(0.0).to_ne_bytes()),
            serde_json::Value::String(s) => step(step(h, b"s"), s.as_bytes()),
            serde_json::Value::Array(a) => {
                let mut h = step(h, b"[");
                for e in a {
                    h = walk(e, h);
                }
                step(h, b"]")
            }
            serde_json::Value::Object(o) => {
                let mut h = step(h, b"{");
                for (k, e) in o {
                    h = step(step(h, k.as_bytes()), b":");
                    h = walk(e, h);
                }
                step(h, b"}")
            }
        }
    }
    walk(o.as_ref().unwrap_or(&serde_json::Value::Null), 0xcbf2_9ce4_8422_2325)
}

/// Like `fingerprint` but with every number folded to one opaque token:
/// the lossy arena rebuilds the stored JSON modulo float rounding, so the
/// full-capture guard compares structure (keys, presence, strings, tree
/// shape, integer-vs-float identity untouched) instead of exact numbers.
fn fingerprint_structure(o: &Option<serde_json::Value>) -> u64 {
    fn step(h: u64, bytes: &[u8]) -> u64 {
        let mut h = h;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        h
    }
    fn walk(v: &serde_json::Value, h: u64) -> u64 {
        match v {
            serde_json::Value::Null => step(h, b"n"),
            serde_json::Value::Bool(b) => step(h, if *b { b"t" } else { b"f" }),
            serde_json::Value::Number(_) => step(h, b"N"),
            serde_json::Value::String(s) => step(step(h, b"s"), s.as_bytes()),
            serde_json::Value::Array(a) => {
                let mut h = step(h, b"[");
                for e in a {
                    h = walk(e, h);
                }
                step(h, b"]")
            }
            serde_json::Value::Object(o) => {
                let mut h = step(h, b"{");
                for (k, e) in o {
                    h = step(step(h, k.as_bytes()), b":");
                    h = walk(e, h);
                }
                step(h, b"}")
            }
        }
    }
    walk(o.as_ref().unwrap_or(&serde_json::Value::Null), 0xcbf2_9ce4_8422_2325)
}

/// Filter keys: last-1h window on seen_at; plans that ran at least 10 s
/// (the capture's slow tail); the first server seen in the capture; text eq
/// on the rarest non-empty `error` (the low-selectivity shape error triage
/// queries use).
#[derive(Clone)]
enum Ek {
    Time(i64, i64),
    Slow,
    Server(i64),
    Error(String),
}

impl Ek {
    fn matches(&self, r: &QueryExplain) -> bool {
        let us = r.seen_at.timestamp_micros();
        match self {
            Ek::Time(a, b) => *a <= us && us <= *b,
            Ek::Slow => r.runtime_ms >= 10_000.0,
            Ek::Server(id) => r.server_id == *id,
            Ek::Error(e) => r.error == *e,
        }
    }

    /// Same filter over the values the lossy write stores: seen_at rounded
    /// to whole seconds, floats through `float_round = 2`.
    fn matches_lossy(&self, r: &QueryExplain) -> bool {
        match self {
            Ek::Time(a, b) => {
                let us = common::ts_rounded(&r.seen_at, 1_000_000).timestamp_micros();
                *a <= us && us <= *b
            }
            Ek::Slow => common::round_f32(r.runtime_ms, 2) >= 10_000.0,
            Ek::Server(id) => r.server_id == *id,
            Ek::Error(e) => r.error == *e,
        }
    }
}

/// The input lives in the private `pco_pack_bench_data` repo; without it
/// this section prints the sensitive-data warning and skips.
pub fn bench() -> anyhow::Result<()> {
    let Some(rows) = common::read_blob(
        "query_explains.msgpack.zst",
        &common::private_data_dir(),
        "Because explain plans can possibly contain sensitive information, only \
         the pganalyze team can run this benchmark: clone pco_pack_bench_data \
         beside this checkout and run `git lfs pull` there",
    )?
    else {
        return Ok(());
    };
    bench_explains(rows);
    Ok(())
}

fn bench_explains(rows: Vec<QueryExplain>) {
    let n = rows.len();
    let last_us = rows.iter().map(|r| r.seen_at.timestamp_micros()).max().unwrap();
    let srv0 = rows[0].server_id;

    // Text eq target: the rarest non-empty error text (most rows carry the
    // empty string), ties broken by content so the pick is reproducible.
    // A capture with no non-empty error falls back to a sentinel miss, the
    // shape that charges pure guard cost.
    let mut err_counts: HashMap<&str, u64> = HashMap::new();
    for r in &rows {
        if !r.error.is_empty() {
            *err_counts.entry(r.error.as_str()).or_default() += 1;
        }
    }
    let err0 = err_counts
        .into_iter()
        .min_by_key(|(name, cnt)| (*cnt, *name))
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "__no_such_error_text__".to_string());

    let keys = [Ek::Time(last_us - 3_600_000_000, last_us), Ek::Slow, Ek::Server(srv0), Ek::Error(err0.clone())];
    let kf: Vec<(Ek, usize)> = keys.iter().map(|k| (k.clone(), rows.iter().filter(|r| k.matches(r)).count())).collect();
    let kf_lossy: Vec<(Ek, usize)> =
        keys.iter().map(|k| (k.clone(), rows.iter().filter(|r| k.matches_lossy(r)).count())).collect();
    let mut labels: Vec<String> = kf[..3].iter().map(|(_, e)| common::filter_label(*e, n)).collect();
    labels.push(if kf[3].1 == 0 { "Text miss".to_string() } else { common::text_filter_label(kf[3].1, n) });
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    // Projection fold ground truth: query_fingerprint => average runtime_ms
    // over the last-1h window, folded before `rows` moves into the encoders.
    let window = &keys[0];
    let proj_truth =
        fold_fp_avg(rows.iter().filter(|r| window.matches(r)).map(|r| (r.query_fingerprint, r.runtime_ms)));

    println!("### query_explains ({n} rows)\n");

    let queries: Vec<serde_json::Value> = vec![
        serde_json::json!({"seen_at": {"start": last_us - 3_600_000_000, "end": last_us}}),
        serde_json::json!({"runtime_ms": {"start": 10000.0, "end": f64::MAX}}),
        serde_json::json!({"server_id": srv0}),
        serde_json::json!({"error": err0.clone()}),
    ];
    let pco_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf.iter()).map(|(q, (_, e))| (q, *e)).collect();

    let expected: Vec<u64> = rows.iter().map(|r| fingerprint(&r.output_json)).collect();
    let expected_structure: Vec<u64> = rows.iter().map(|r| fingerprint_structure(&r.output_json)).collect();

    let string_rows: Vec<QueryExplainString> = rows.iter().map(QueryExplainString::from).collect();
    let (mut pco_text, text_bytes) = common::bench_pco_pack_full(string_rows, &pco_filters, |back| {
        assert_eq!(back.len(), n, "query_explains (text) row count")
    });
    let (ms_text, fold_text) = common::bench_pco_pack_projected::<QueryExplainString, _>(
        &text_bytes,
        &queries[0],
        &["query_fingerprint", "runtime_ms"],
        kf[0].1,
        |rows| fold_fp_avg(rows.iter().map(|r| (r.query_fingerprint, r.runtime_ms))),
    );
    assert_eq!(fold_text, proj_truth, "query_explains (text) projected fold");
    pco_text.project_ms = Some(ms_text);

    let lossy_rows: Vec<QueryExplainLossy> = rows.iter().map(QueryExplainLossy::from).collect();
    let lossy_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_lossy.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let (mut pco_lossy, lossy_bytes) = common::bench_pco_pack_full(lossy_rows, &lossy_filters, |back| {
        assert_eq!(back.len(), n, "query_explains (lossy) row count")
    });
    let (ms_lossy, _) = common::bench_pco_pack_projected::<QueryExplainLossy, _>(
        &lossy_bytes,
        &queries[0],
        &["query_fingerprint", "runtime_ms"],
        kf_lossy[0].1,
        |rows| rows.len(),
    );
    pco_lossy.project_ms = Some(ms_lossy);

    let arena_rows: Vec<QueryExplainArena> = rows.iter().map(QueryExplainArena::from).collect();
    let (mut pco_arena, arena_bytes) = common::bench_pco_pack_full(arena_rows, &pco_filters, move |back| {
        assert_eq!(back.len(), n, "query_explains (arena) row count");
        let mut got: Vec<u64> = back.iter().map(|r| fingerprint(&r.arena.as_ref().map(|a| a.rebuild()))).collect();
        let mut exp = expected.clone();
        got.sort_unstable();
        exp.sort_unstable();
        assert_eq!(got, exp, "query_explains (arena): rebuilt output_json must match the stored JSON (as a multiset)");
    });
    let (ms_arena, fold_arena) = common::bench_pco_pack_projected::<QueryExplainArena, _>(
        &arena_bytes,
        &queries[0],
        &["query_fingerprint", "runtime_ms"],
        kf[0].1,
        |rows| fold_fp_avg(rows.iter().map(|r| (r.query_fingerprint, r.runtime_ms))),
    );
    assert_eq!(fold_arena, proj_truth, "query_explains (arena) projected fold");
    pco_arena.project_ms = Some(ms_arena);

    let arena_lossy_rows: Vec<QueryExplainArenaLossy> = rows.iter().map(QueryExplainArenaLossy::from).collect();
    let (mut pco_arena_lossy, arena_lossy_bytes) =
        common::bench_pco_pack_full(arena_lossy_rows, &lossy_filters, move |back| {
            assert_eq!(back.len(), n, "query_explains (arena lossy) row count");
            let mut got: Vec<u64> =
                back.iter().map(|r| fingerprint_structure(&r.arena.as_ref().map(|a| a.rebuild()))).collect();
            let mut exp = expected_structure.clone();
            got.sort_unstable();
            exp.sort_unstable();
            assert_eq!(
                got, exp,
                "query_explains (arena lossy): rebuilt output_json must match the stored JSON structure (as a multiset)"
            );
        });
    let (ms_al, _) = common::bench_pco_pack_projected::<QueryExplainArenaLossy, _>(
        &arena_lossy_bytes,
        &queries[0],
        &["query_fingerprint", "runtime_ms"],
        kf_lossy[0].1,
        |rows| rows.len(),
    );
    pco_arena_lossy.project_ms = Some(ms_al);

    let col_rows: Vec<QueryExplainCol> = rows.iter().map(QueryExplainCol::from).collect();
    let (mut col, col_buf) = common::bench_columnar(
        n,
        move || col_rows,
        &kf,
        |decoded: &columnar::BorrowedOf<QueryExplainCol>, k: &Ek| {
            let count = columnar::Len::len(decoded);
            (0..count)
                .filter(|&i| {
                    let r = columnar::Index::get(decoded, i);
                    match k {
                        Ek::Time(a, b) => {
                            let us = *r.seen_at;
                            *a <= us && us <= *b
                        }
                        Ek::Slow => *r.runtime_ms >= 10_000.0,
                        Ek::Server(id) => *r.server_id == *id,
                        Ek::Error(e) => r.error == e.as_bytes(),
                    }
                })
                .count()
        },
    );
    col.project_ms = Some(common::reduce_columnar::<QueryExplainCol, Ek, HashMap<i64, f64>, _>(
        &col_buf,
        &keys[0],
        |decoded: &columnar::BorrowedOf<'_, QueryExplainCol>, k: &Ek| {
            fold_fp_avg((0..columnar::Len::len(decoded)).filter_map(|i| {
                let r = columnar::Index::get(decoded, i);
                let hit = match k {
                    Ek::Time(a, b) => {
                        let us = *r.seen_at;
                        *a <= us && us <= *b
                    }
                    Ek::Slow => *r.runtime_ms >= 10_000.0,
                    Ek::Server(id) => *r.server_id == *id,
                    Ek::Error(e) => r.error == e.as_bytes(),
                };
                if hit { Some((*r.query_fingerprint, *r.runtime_ms)) } else { None }
            }))
        },
        &proj_truth,
    ));

    let vx_rows: Vec<QueryExplainCol> = rows.iter().map(QueryExplainCol::from).collect();
    let (mut vx, vx_buf) = common::bench_vortex_ds(&vx_rows, &kf, |k: &Ek| {
        use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq};
        match k {
            Ek::Time(a, b) => and(gt_eq(col("seen_at"), lit(*a)), lt_eq(col("seen_at"), lit(*b))),
            Ek::Slow => gt_eq(col("runtime_ms"), lit(10_000.0f32)),
            Ek::Server(id) => eq(col("server_id"), lit(*id)),
            Ek::Error(e) => eq(col("error"), lit(e.as_str())),
        }
    });
    vx.project_ms = common::reduce_vortex_ds::<(i64, f32), Ek, _, _, _>(
        &vx_buf,
        &keys[0],
        |k: &Ek| {
            use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq};
            match k {
                Ek::Time(a, b) => and(gt_eq(col("seen_at"), lit(*a)), lt_eq(col("seen_at"), lit(*b))),
                Ek::Slow => gt_eq(col("runtime_ms"), lit(10_000.0f32)),
                Ek::Server(id) => eq(col("server_id"), lit(*id)),
                Ek::Error(e) => eq(col("error"), lit(e.as_str())),
            }
        },
        &["query_fingerprint", "runtime_ms"],
        |rows: Vec<(i64, f32)>, _: &Ek| fold_fp_avg(rows.into_iter()),
        &proj_truth,
    );
    drop(vx_rows);

    let (mut sc, sc_buf) = common::bench_serde_columnar(
        n,
        || explains_sc::ExplainsScStore { data: rows.iter().map(explains_sc::ExplainScRow::from).collect() },
        &kf,
        |s: &explains_sc::ExplainsScStore, k: &Ek| {
            s.data
                .iter()
                .filter(|r| match k {
                    Ek::Time(a, b) => *a <= r.seen_at && r.seen_at <= *b,
                    Ek::Slow => r.runtime_ms >= 10_000.0,
                    Ek::Server(id) => r.server_id == *id,
                    Ek::Error(e) => r.error == *e,
                })
                .count()
        },
    );
    sc.project_ms = Some(common::reduce_serde_columnar::<explains_sc::ExplainsScStore, Ek, _, _>(
        &sc_buf,
        &keys[0],
        |s: &explains_sc::ExplainsScStore, k: &Ek| {
            fold_fp_avg(
                s.data
                    .iter()
                    .filter(|r| match k {
                        Ek::Time(a, b) => *a <= r.seen_at && r.seen_at <= *b,
                        Ek::Slow => r.runtime_ms >= 10_000.0,
                        Ek::Server(id) => r.server_id == *id,
                        Ek::Error(e) => r.error == *e,
                    })
                    .map(|r| (r.query_fingerprint, r.runtime_ms)),
            )
        },
        &proj_truth,
    ));

    let (mut mp, mp_buf) = common::bench_msgpack(&rows, &kf, |r: &QueryExplain, k: &Ek| k.matches(r));
    mp.project_ms = Some(common::reduce_msgpack::<QueryExplain, Ek, _, _>(
        &mp_buf,
        n,
        &keys[0],
        |rows: &[QueryExplain], k: &Ek| {
            fold_fp_avg(rows.iter().filter(|r| k.matches(r)).map(|r| (r.query_fingerprint, r.runtime_ms)))
        },
        &proj_truth,
    ));

    let (mut pco, pco_bytes) =
        common::bench_pco_pack_full(rows, &pco_filters, |back| assert_eq!(back.len(), n, "query_explains row count"));
    pco.project_ms = Some(
        common::bench_pco_pack_projected::<QueryExplain, _>(
            &pco_bytes,
            &queries[0],
            &["query_fingerprint", "runtime_ms"],
            kf[0].1,
            |rows| fold_fp_avg(rows.iter().map(|r| (r.query_fingerprint, r.runtime_ms))),
        )
        .0,
    );

    let metrics = ["Size", "Serialize", "Deserialize"];
    let mut pco_out: Vec<CodecRowPcoPack> = metrics
        .iter()
        .map(|m| CodecRowPcoPack {
            metric: m.to_string(),
            text: cell(&pco_text, m, None),
            json: cell(&pco, m, None),
            json_lossy: cell(&pco_lossy, m, None),
            arena: cell(&pco_arena, m, None),
            arena_lossy: cell(&pco_arena_lossy, m, None),
        })
        .collect();
    let mut other_out: Vec<CodecRowOther> = metrics
        .iter()
        .map(|m| CodecRowOther {
            metric: m.to_string(),
            vortex: cell(&vx, m, None),
            columnar: cell(&col, m, None),
            serde_columnar: cell(&sc, m, None),
            msgpack: cell(&mp, m, None),
        })
        .collect();
    for (i, label) in label_refs.iter().enumerate() {
        pco_out.push(CodecRowPcoPack {
            metric: label.to_string(),
            text: cell(&pco_text, "Filter", Some(i)),
            json: cell(&pco, "Filter", Some(i)),
            json_lossy: cell(&pco_lossy, "Filter", Some(i)),
            arena: cell(&pco_arena, "Filter", Some(i)),
            arena_lossy: cell(&pco_arena_lossy, "Filter", Some(i)),
        });
        other_out.push(CodecRowOther {
            metric: label.to_string(),
            vortex: cell(&vx, "Filter", Some(i)),
            columnar: cell(&col, "Filter", Some(i)),
            serde_columnar: cell(&sc, "Filter", Some(i)),
            msgpack: cell(&mp, "Filter", Some(i)),
        });
    }
    pco_out.push(CodecRowPcoPack {
        metric: common::reduce_label(kf[0].1, n),
        text: cell(&pco_text, "Project", None),
        json: cell(&pco, "Project", None),
        json_lossy: cell(&pco_lossy, "Project", None),
        arena: cell(&pco_arena, "Project", None),
        arena_lossy: cell(&pco_arena_lossy, "Project", None),
    });
    other_out.push(CodecRowOther {
        metric: common::reduce_label(kf[0].1, n),
        vortex: cell(&vx, "Project", None),
        columnar: cell(&col, "Project", None),
        serde_columnar: cell(&sc, "Project", None),
        msgpack: cell(&mp, "Project", None),
    });
    println!("#### PcoPack scenarios\n");
    println!("{}", markdown_tables::as_table(&pco_out));
    println!();
    println!("#### Other crates\n");
    println!("{}", markdown_tables::as_table(&other_out));
    println!();
}

/// query_fingerprint => average runtime_ms, over (fingerprint, runtime) pairs.
fn fold_fp_avg(pairs: impl Iterator<Item = (i64, f32)>) -> HashMap<i64, f64> {
    let mut acc: HashMap<i64, (f64, u64)> = HashMap::new();
    for (fp, rt) in pairs {
        let e = acc.entry(fp).or_default();
        e.0 += rt as f64;
        e.1 += 1;
    }
    acc.into_iter().map(|(fp, (sum, cnt))| (fp, sum / cnt as f64)).collect()
}

/// columnar/vortex mirror: timestamps to i64 us, JSON carriers stringified.
/// columnar has no `serde_json::Value` impl and serde_columnar's postcard
/// fallback dies on Value's `deserialize_any`; roundtripped values are
/// identical strings on both sides of each codec.
#[derive(Clone, Debug, PartialEq, columnar::Columnar, Serialize, Deserialize)]
pub struct QueryExplainCol {
    pub server_id: i64,
    pub database_id: i64,
    pub postgres_role_id: i64,
    pub query_fingerprint: i64,
    pub format: i32,
    pub source: i32,
    pub error: String,
    pub seen_at: i64,
    pub runtime_ms: f32,
    pub total_cost: Option<f32>,
    pub total_shared_blks_hit: Option<i64>,
    pub total_shared_blks_read: Option<i64>,
    pub total_shared_blks_written: Option<i64>,
    pub total_shared_blks_dirtied: Option<i64>,
    pub total_blk_read_time: Option<f32>,
    pub total_blk_write_time: Option<f32>,
    pub plan_node_types: Vec<String>,
    pub plan_fingerprint: Option<i64>,
    pub output_text: Option<String>,
    pub output_json: Option<String>,
    pub node_insights: Option<String>,
}

impl From<&QueryExplain> for QueryExplainCol {
    fn from(r: &QueryExplain) -> Self {
        QueryExplainCol {
            server_id: r.server_id,
            database_id: r.database_id,
            postgres_role_id: r.postgres_role_id,
            query_fingerprint: r.query_fingerprint,
            format: r.format,
            source: r.source,
            error: r.error.clone(),
            seen_at: r.seen_at.timestamp_micros(),
            runtime_ms: r.runtime_ms,
            total_cost: r.total_cost,
            total_shared_blks_hit: r.total_shared_blks_hit,
            total_shared_blks_read: r.total_shared_blks_read,
            total_shared_blks_written: r.total_shared_blks_written,
            total_shared_blks_dirtied: r.total_shared_blks_dirtied,
            total_blk_read_time: r.total_blk_read_time,
            total_blk_write_time: r.total_blk_write_time,
            plan_node_types: r.plan_node_types.iter().map(ToString::to_string).collect(),
            plan_fingerprint: r.plan_fingerprint,
            output_text: r.output_text.clone(),
            output_json: r.output_json.as_ref().map(serde_json::Value::to_string),
            node_insights: r.node_insights.as_ref().map(serde_json::Value::to_string),
        }
    }
}

// ---- serde_columnar mirror (strategies follow the taxi/osm convention) ----

mod explains_sc {
    use super::*;
    use serde_columnar::columnar;

    /// JSON payloads ride as strings (postcard can't encode Value's
    /// `deserialize_any`); the rest keep native columnar types.
    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct ExplainScRow {
        #[columnar(strategy = "DeltaRle")]
        pub seen_at: i64,
        #[columnar(strategy = "Rle")]
        pub format: i32,
        #[columnar(strategy = "Rle")]
        pub source: i32,
        #[columnar(strategy = "Rle")]
        pub error: String,
        pub server_id: i64,
        pub database_id: i64,
        pub postgres_role_id: i64,
        pub query_fingerprint: i64,
        pub runtime_ms: f32,
        pub total_cost: Option<f32>,
        pub total_shared_blks_hit: Option<i64>,
        pub total_shared_blks_read: Option<i64>,
        pub total_shared_blks_written: Option<i64>,
        pub total_shared_blks_dirtied: Option<i64>,
        pub total_blk_read_time: Option<f32>,
        pub total_blk_write_time: Option<f32>,
        pub plan_node_types: Vec<String>,
        pub plan_fingerprint: Option<i64>,
        pub output_text: Option<String>,
        pub output_json: Option<String>,
        pub node_insights: Option<String>,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct ExplainsScStore {
        #[columnar(class = "vec")]
        pub data: Vec<ExplainScRow>,
    }

    impl From<&QueryExplain> for ExplainScRow {
        fn from(r: &QueryExplain) -> Self {
            ExplainScRow {
                seen_at: r.seen_at.timestamp_micros(),
                format: r.format,
                source: r.source,
                error: r.error.clone(),
                server_id: r.server_id,
                database_id: r.database_id,
                postgres_role_id: r.postgres_role_id,
                query_fingerprint: r.query_fingerprint,
                runtime_ms: r.runtime_ms,
                total_cost: r.total_cost,
                total_shared_blks_hit: r.total_shared_blks_hit,
                total_shared_blks_read: r.total_shared_blks_read,
                total_shared_blks_written: r.total_shared_blks_written,
                total_shared_blks_dirtied: r.total_shared_blks_dirtied,
                total_blk_read_time: r.total_blk_read_time,
                total_blk_write_time: r.total_blk_write_time,
                plan_node_types: r.plan_node_types.iter().map(ToString::to_string).collect(),
                plan_fingerprint: r.plan_fingerprint,
                output_text: r.output_text.clone(),
                output_json: r.output_json.as_ref().map(serde_json::Value::to_string),
                node_insights: r.node_insights.as_ref().map(serde_json::Value::to_string),
            }
        }
    }
}

impl common::ScStore for explains_sc::ExplainsScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}
