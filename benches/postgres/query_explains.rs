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
//!
//! `Vortex json` is the one non-PcoPack scenario that does not store the
//! payload as text: the NDJSON text of every `output_json` goes through
//! Arrow's JSON schema inference (`arrow_json::infer_json_schema` +
//! `ReaderBuilder`, the vortex_json example's pipeline), so every plan key
//! becomes its own column and btrblocks compresses the shredded tree per
//! leaf. The inference schema folds `null` and key absence together, so
//! the verify guard compares rows as folded-shape fingerprints.

use chrono::{DateTime, Utc};
use pco_pack::PcoPack;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

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
/// (the capture's slow tail); the first server seen in the capture.
#[derive(Clone, Copy)]
enum Ek {
    Time(i64, i64),
    Slow,
    Server(i64),
}

impl Ek {
    fn matches(&self, r: &QueryExplain) -> bool {
        let us = r.seen_at.timestamp_micros();
        match self {
            Ek::Time(a, b) => *a <= us && us <= *b,
            Ek::Slow => r.runtime_ms >= 10_000.0,
            Ek::Server(id) => r.server_id == *id,
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

    let keys = [Ek::Time(last_us - 3_600_000_000, last_us), Ek::Slow, Ek::Server(srv0)];
    let kf: Vec<(Ek, usize)> = keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches(r)).count())).collect();
    let kf_lossy: Vec<(Ek, usize)> =
        keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches_lossy(r)).count())).collect();
    let labels: Vec<String> = kf.iter().map(|(_, e)| common::filter_label(*e, n)).collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    println!("### query_explains ({n} rows)\n");

    let queries: Vec<serde_json::Value> = vec![
        serde_json::json!({"seen_at": {"start": last_us - 3_600_000_000, "end": last_us}}),
        serde_json::json!({"runtime_ms": {"start": 10000.0, "end": f64::MAX}}),
        serde_json::json!({"server_id": srv0}),
    ];
    let pco_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf.iter()).map(|(q, (_, e))| (q, *e)).collect();

    let expected: Vec<u64> = rows.iter().map(|r| fingerprint(&r.output_json)).collect();
    let expected_structure: Vec<u64> = rows.iter().map(|r| fingerprint_structure(&r.output_json)).collect();

    let string_rows: Vec<QueryExplainString> = rows.iter().map(QueryExplainString::from).collect();
    let pco_text = common::bench_pco_pack(string_rows, &pco_filters, |back| {
        assert_eq!(back.len(), n, "query_explains (text) row count")
    });

    let lossy_rows: Vec<QueryExplainLossy> = rows.iter().map(QueryExplainLossy::from).collect();
    let lossy_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_lossy.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let pco_lossy = common::bench_pco_pack(lossy_rows, &lossy_filters, |back| {
        assert_eq!(back.len(), n, "query_explains (lossy) row count")
    });

    let arena_rows: Vec<QueryExplainArena> = rows.iter().map(QueryExplainArena::from).collect();
    let pco_arena = common::bench_pco_pack(arena_rows, &pco_filters, move |back| {
        assert_eq!(back.len(), n, "query_explains (arena) row count");
        let mut got: Vec<u64> = back.iter().map(|r| fingerprint(&r.arena.as_ref().map(|a| a.rebuild()))).collect();
        let mut exp = expected.clone();
        got.sort_unstable();
        exp.sort_unstable();
        assert_eq!(got, exp, "query_explains (arena): rebuilt output_json must match the stored JSON (as a multiset)");
    });

    let arena_lossy_rows: Vec<QueryExplainArenaLossy> = rows.iter().map(QueryExplainArenaLossy::from).collect();
    let pco_arena_lossy = common::bench_pco_pack(arena_lossy_rows, &lossy_filters, move |back| {
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

    let col_rows: Vec<QueryExplainCol> = rows.iter().map(QueryExplainCol::from).collect();
    let col = common::bench_columnar(
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
                    }
                })
                .count()
        },
    );

    let vx_rows: Vec<QueryExplainCol> = rows.iter().map(QueryExplainCol::from).collect();
    let vx = common::bench_vortex_ds(&vx_rows, &kf, |k: &Ek| {
        use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq};
        match k {
            Ek::Time(a, b) => and(gt_eq(col("seen_at"), lit(*a)), lt_eq(col("seen_at"), lit(*b))),
            Ek::Slow => gt_eq(col("runtime_ms"), lit(10_000.0f32)),
            Ek::Server(id) => eq(col("server_id"), lit(*id)),
        }
    });
    drop(vx_rows);

    let vx_json = bench_vortex_json(&rows, &kf, |k: &Ek| {
        use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq};
        match k {
            Ek::Time(a, b) => and(gt_eq(col("seen_at"), lit(*a)), lt_eq(col("seen_at"), lit(*b))),
            Ek::Slow => gt_eq(col("runtime_ms"), lit(10_000.0f32)),
            Ek::Server(id) => eq(col("server_id"), lit(*id)),
        }
    });

    let sc = common::bench_serde_columnar(
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
                })
                .count()
        },
    );

    let mp = common::bench_msgpack(&rows, &kf, |r: &QueryExplain, k: &Ek| k.matches(r));

    let pco = common::bench_pco_pack(rows, &pco_filters, |back| assert_eq!(back.len(), n, "query_explains row count"));

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
            vortex_json: cell(&vx_json, m, None),
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
            vortex_json: cell(&vx_json, "Filter", Some(i)),
            columnar: cell(&col, "Filter", Some(i)),
            serde_columnar: cell(&sc, "Filter", Some(i)),
            msgpack: cell(&mp, "Filter", Some(i)),
        });
    }
    println!("#### PcoPack scenarios\n");
    println!("{}", markdown_tables::as_table(&pco_out));
    println!();
    println!("#### Other crates\n");
    println!("{}", markdown_tables::as_table(&other_out));
    println!();
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

// ---- vortex json mirror: every key of the explain plan shredded into its
// own column by Arrow's JSON schema inference (the `Vortex json` column) ----

/// Carrier for the vortex json scenario: `QueryExplainCol` without
/// `output_json`. The payload instead rides as one column per JSON key
/// (a `Plan` struct column and friends), shredded by the arrow-json
/// schema inference; the join of both is what reaches the vortex file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryExplainJsonCols {
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
    pub node_insights: Option<String>,
}

impl From<&QueryExplain> for QueryExplainJsonCols {
    fn from(r: &QueryExplain) -> Self {
        QueryExplainJsonCols {
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
            node_insights: r.node_insights.as_ref().map(serde_json::Value::to_string),
        }
    }
}

/// Key-shredding fingerprint: like `fingerprint`, but JSON `null` values
/// fold into "absent" (an inference-schema nulls every key a row did not
/// carry, so `null` and a missing key are indistinguishable after a
/// shredded roundtrip), empty folded objects contribute nothing, and
/// numbers fold to their f64 bits (so `Int(127)` and `Float(127.0)` and
/// `127.00` fingerprint alike). The JSON col is the folded object rebuilt
/// from the row's shredded columns; the stored col folds the same way from
/// the `Option<Value>` mirror (a null row is an empty folded object).
fn fold_json_value(v: &serde_json::Value, h: &mut u64) {
    fn step(h: &mut u64, bytes: &[u8]) {
        for &b in bytes {
            *h ^= b as u64;
            *h = h.wrapping_mul(0x1000_0000_01b3);
        }
    }
    fn fold(v: &serde_json::Value, h: &mut u64) -> bool {
        match v {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(b) => {
                step(h, if *b { b"t" } else { b"f" });
                true
            }
            serde_json::Value::Number(n) => {
                step(h, &n.as_f64().unwrap_or(0.0).to_ne_bytes());
                true
            }
            serde_json::Value::String(s) => {
                step(h, b"s");
                step(h, s.as_bytes());
                true
            }
            serde_json::Value::Array(a) => {
                step(h, b"[");
                for e in a {
                    fold(e, h);
                }
                step(h, b"]");
                true
            }
            serde_json::Value::Object(o) => {
                let mut inner = 0xcbf2_9ce4_8422_2325;
                let mut any = false;
                for (k, e) in o {
                    let mut eh = 0xcbf2_9ce4_8422_2325;
                    if fold(e, &mut eh) {
                        any = true;
                        step(&mut inner, k.as_bytes());
                        step(&mut inner, b":");
                        // Fold the child hash in as bytes: order is canonical
                        // (serde_json maps iterate sorted), so the combine is.
                        step(&mut inner, &eh.to_ne_bytes());
                    }
                }
                if !any {
                    return false;
                }
                step(h, b"{");
                step(h, &inner.to_ne_bytes());
                step(h, b"}");
                true
            }
        }
    }
    fold(v, h);
}

/// Vortex files embed the schema dtype in a flatbuffer verified with a
/// 64-nesting cap, and this capture's plans nest up to 40 levels, so keys
/// shred into columns only down to this depth; deeper subtrees ride as
/// JSON-text blob columns (`key` + `BLOB_SUFFIX`), tagged with the
/// vortex.json extension dtype and reparsed at verify time.
const SHRED_DEPTH: usize = 12;
const BLOB_SUFFIX: &str = "_json";

/// Serializes a shred record to NDJSON text: containers at depth `>= 1` are
/// descended; containers reaching `SHRED_DEPTH` collapse to quoted JSON text
/// under a `key_json` field. Keys iterate sorted (serde_json maps), matching
/// the fold fingerprint order.
fn emit_shredded(v: &serde_json::Value, depth: usize, out: &mut String) {
    fn is_container(v: &serde_json::Value) -> bool {
        matches!(v, serde_json::Value::Object(_) | serde_json::Value::Array(_))
    }
    match v {
        serde_json::Value::Object(o) => {
            out.push('{');
            for (i, (k, child)) in o.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if is_container(child) && depth + 1 >= SHRED_DEPTH {
                    out.push_str(&serde_json::to_string(&format!("{k}{BLOB_SUFFIX}")).unwrap());
                    out.push(':');
                    out.push_str(&serde_json::to_string(&child.to_string()).unwrap());
                } else {
                    out.push_str(&serde_json::to_string(k).unwrap());
                    out.push(':');
                    emit_shredded(child, depth + 1, out);
                }
            }
            out.push('}');
        }
        serde_json::Value::Array(a) => {
            out.push('[');
            for (i, child) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                emit_shredded(child, depth + 1, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Fold plan of the shredded export, built from the inferred schema so the
/// read pass fingerprints rows without materializing them: `Blob` re-parses
/// a vortex.json text column (its key folds back under the suffix-stripped
/// name), `Object` iterates entries in canonical sorted order.
enum FoldTree {
    Null,
    Blob,
    Str,
    Bool,
    Num,
    List(Box<FoldTree>),
    Object(Vec<(String, Box<FoldTree>)>),
}

impl FoldTree {
    fn from_field(field: &arrow_schema::FieldRef) -> Self {
        use arrow_schema::extension::{ExtensionType as _, Json as ArrowJson};
        if field.extension_type_name() == Some(ArrowJson::NAME) {
            return Self::Blob;
        }
        match field.data_type() {
            arrow_schema::DataType::Null => Self::Null,
            arrow_schema::DataType::Utf8 | arrow_schema::DataType::LargeUtf8 | arrow_schema::DataType::Utf8View => {
                Self::Str
            }
            arrow_schema::DataType::Boolean => Self::Bool,
            arrow_schema::DataType::Int8
            | arrow_schema::DataType::Int16
            | arrow_schema::DataType::Int32
            | arrow_schema::DataType::Int64
            | arrow_schema::DataType::UInt8
            | arrow_schema::DataType::UInt16
            | arrow_schema::DataType::UInt32
            | arrow_schema::DataType::UInt64
            | arrow_schema::DataType::Float32
            | arrow_schema::DataType::Float64 => Self::Num,
            arrow_schema::DataType::List(child) | arrow_schema::DataType::LargeList(child) => {
                Self::List(Box::new(Self::from_field(child)))
            }
            arrow_schema::DataType::Struct(children) => {
                let mut entries: Vec<(String, Box<FoldTree>)> = children
                    .iter()
                    .map(|c| {
                        // Blob children carry the raw `key_json` column name;
                        // entries key on the canonical (stripped) name, the
                        // same string the fingerprint folds on both sides.
                        let name = c.name().strip_suffix(BLOB_SUFFIX).unwrap_or(c.name()).to_string();
                        (name, Box::new(Self::from_field(c)))
                    })
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                Self::Object(entries)
            }
            dt => panic!("vortex json fold: unexpected inferred dtype {dt}"),
        }
    }

    /// Folds row `i` of `col` into `h`, returning whether it contributed;
    /// mirrors `fold_json_value`'s rules (null/empty fold to nothing, blobs
    /// hash as the subtree they reparse to). Column lookup for a blob child
    /// adds the suffix back to the canonical name.
    fn fold(&self, col: &dyn arrow_array::Array, i: usize, h: &mut u64) -> bool {
        use arrow_array::cast::AsArray;
        fn step(h: &mut u64, bytes: &[u8]) {
            for &b in bytes {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x1000_0000_01b3);
            }
        }
        fn text<'a>(col: &'a dyn arrow_array::Array, i: usize) -> &'a str {
            match col.data_type() {
                arrow_schema::DataType::Utf8 => col.as_string::<i32>().value(i),
                arrow_schema::DataType::LargeUtf8 => col.as_string::<i64>().value(i),
                arrow_schema::DataType::Utf8View => col.as_string_view().value(i),
                dt => panic!("vortex json fold: unexpected text dtype {dt}"),
            }
        }
        fn num(col: &dyn arrow_array::Array, i: usize) -> f64 {
            use arrow_array::types::*;
            macro_rules! p {
                ($t:ty) => {
                    col.as_primitive::<$t>().value(i) as f64
                };
            }
            match col.data_type() {
                arrow_schema::DataType::Int8 => p!(Int8Type),
                arrow_schema::DataType::Int16 => p!(Int16Type),
                arrow_schema::DataType::Int32 => p!(Int32Type),
                arrow_schema::DataType::Int64 => p!(Int64Type),
                arrow_schema::DataType::UInt8 => p!(UInt8Type),
                arrow_schema::DataType::UInt16 => p!(UInt16Type),
                arrow_schema::DataType::UInt32 => p!(UInt32Type),
                arrow_schema::DataType::UInt64 => p!(UInt64Type),
                arrow_schema::DataType::Float32 => p!(Float32Type),
                arrow_schema::DataType::Float64 => p!(Float64Type),
                dt => panic!("vortex json fold: unexpected numeric dtype {dt}"),
            }
        }
        if col.is_null(i) {
            return false;
        }
        match self {
            Self::Null => false,
            Self::Blob => {
                let v: serde_json::Value = serde_json::from_str(text(col, i)).unwrap();
                fold_json_value(&v, h);
                true
            }
            Self::Str => {
                step(h, b"s");
                step(h, text(col, i).as_bytes());
                true
            }
            Self::Bool => {
                step(h, if col.as_boolean().value(i) { b"t" } else { b"f" });
                true
            }
            Self::Num => {
                step(h, &num(col, i).to_ne_bytes());
                true
            }
            Self::List(child) => {
                step(h, b"[");
                let list = col.as_list::<i32>();
                let values = list.values();
                let offs = list.value_offsets();
                for j in offs[i] as usize..offs[i + 1] as usize {
                    child.fold(values.as_ref(), j, h);
                }
                step(h, b"]");
                true
            }
            Self::Object(entries) => {
                let st = col.as_struct();
                let mut inner = 0xcbf2_9ce4_8422_2325;
                let mut any = false;
                for (name, tree) in entries {
                    let colname =
                        if matches!(tree.as_ref(), Self::Blob) { format!("{name}{BLOB_SUFFIX}") } else { name.clone() };
                    let child_col = st
                        .column_by_name(&colname)
                        .unwrap_or_else(|| panic!("vortex json fold: missing column {colname}"));
                    let mut eh = 0xcbf2_9ce4_8422_2325;
                    if tree.fold(child_col.as_ref(), i, &mut eh) {
                        any = true;
                        step(&mut inner, name.as_bytes());
                        step(&mut inner, b":");
                        step(&mut inner, &eh.to_ne_bytes());
                    }
                }
                if !any {
                    return false;
                }
                step(h, b"{");
                step(h, &inner.to_ne_bytes());
                step(h, b"}");
                true
            }
        }
    }
}

/// The NDJSON record of a row's `output_json`: Postgres' EXPLAIN (FORMAT
/// JSON) document is a single-element array wrapping the plan object; the
/// record to shred is the object itself (its keys become the columns).
/// Non-object values fold to null, and a null row is a literal `null` line
/// (an all-null record).
fn explain_record(o: &Option<serde_json::Value>) -> &serde_json::Value {
    static NULL: serde_json::Value = serde_json::Value::Null;
    match o.as_ref() {
        Some(serde_json::Value::Array(a)) => a.first().unwrap_or(&NULL),
        Some(v) => v,
        None => &NULL,
    }
}

/// The Vortex json scenario. An NDJSON capture of every row's `output_json`
/// goes through arrow-json's schema inference; the inferred tree becomes
/// vortex struct columns (`Plan` with its full nested shape, `Workers` as
/// list rows), compressed by btrblocks per leaf instead of the zstd'd text
/// blob of the plain `Vortex` column. Nesting is cut at `SHRED_DEPTH`,
/// beyond which subtrees persist as vortex.json blob columns. Scalars ride
/// in beside them via serde_arrow unchanged. Steps 1/2/4 of the JSON ingest
/// pipeline (session, inference, compressor) follow the vortex_json example.
///
/// Read-back walks the inferred schema tree (`FoldTree`), reparsing blob
/// columns; the verify fingerprints folded shapes on both sides
/// (`fold_json_value`), counting-only for filters like every other codec
/// here. A caught panic degrades all metrics to "-".
fn bench_vortex_json(
    rows: &[QueryExplain], kf: &[(Ek, usize)], build_expr: impl Fn(&Ek) -> vortex::array::expr::Expression,
) -> common::CodecMetrics {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bench_vortex_json_inner(rows, kf, build_expr)));
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
                eprintln!("vortex json pipeline panic: {msg}");
            }
            common::CodecMetrics::default() // all None -> "-" cells
        }
    }
}

fn bench_vortex_json_inner(
    rows: &[QueryExplain], kf: &[(Ek, usize)], build_expr: impl Fn(&Ek) -> vortex::array::expr::Expression,
) -> common::CodecMetrics {
    use arrow_json::{ReaderBuilder, reader::infer_json_schema};
    use arrow_schema::{FieldRef, Schema, SchemaRef};
    use serde_arrow::schema::{SchemaLike, TracingOptions};
    use std::io::Cursor;
    use std::sync::Arc;
    use vortex::array::stream::ArrayStreamExt;
    use vortex::arrow::ArrowSessionExt as _;
    use vortex::buffer::ByteBuffer;
    use vortex::compressor::BtrBlocksCompressorBuilder;
    use vortex::file::{OpenOptionsSessionExt, WriteOptionsSessionExt, WriteStrategyBuilder};

    let n = rows.len();
    let rt = crate::ds_common::ds_vortex_rt();
    let session = crate::ds_common::ds_vortex_session();
    // The default session already registers JSON support (vortex_json is
    // initialized through its `parquet-variant` feature); call it explicitly
    // like the vortex_json example does - registration is idempotent.
    vortex_json::initialize(session);

    // Row-side NDJSON mirror (unmeasured, like the text scenario's
    // stringification): each row is its shred record; null rows are literal
    // `null` lines; deep subtrees become `key_json` text fields.
    let mut text: String = String::with_capacity(n * 8_192);
    for r in rows {
        emit_shredded(explain_record(&r.output_json), 0, &mut text);
        text.push('\n');
    }

    let scalar_fields: Vec<FieldRef> =
        <Vec<FieldRef>>::from_type::<QueryExplainJsonCols>(TracingOptions::default()).unwrap();
    let scalar_cols: Vec<QueryExplainJsonCols> = rows.iter().map(QueryExplainJsonCols::from).collect();

    let expected: Vec<u64> = rows
        .iter()
        .map(|r| {
            let mut h = 0xcbf2_9ce4_8422_2325;
            fold_json_value(explain_record(&r.output_json), &mut h);
            h
        })
        .collect();

    let (serial_ms, (bytes, fold_root)) = crate::ds_common::one_pass(|| {
        // Schema inference over the full capture, then decode it against
        // that schema in one pass; btrblocks compresses the shredded tree.
        // The `key_json` fields are blob leaves: tag them with the
        // vortex.json extension dtype (the registered importer maps the
        // canonical Arrow JSON ext to vortex.json on the way in).
        fn tag_json_blobs(field: arrow_schema::FieldRef) -> arrow_schema::FieldRef {
            use arrow_schema::extension::Json as ArrowJson;
            let tagged = match field.data_type() {
                arrow_schema::DataType::Struct(children) => {
                    arrow_schema::DataType::Struct(children.iter().map(|c| tag_json_blobs(c.clone())).collect())
                }
                // Blob leaves live inside list-element structs too: the
                // emitter blobs containers at any object-member position.
                arrow_schema::DataType::List(child) => arrow_schema::DataType::List(tag_json_blobs(child.clone())),
                arrow_schema::DataType::LargeList(child) => {
                    arrow_schema::DataType::LargeList(tag_json_blobs(child.clone()))
                }
                arrow_schema::DataType::Utf8 if field.name().ends_with(BLOB_SUFFIX) => {
                    return Arc::new(field.as_ref().clone().with_extension_type(ArrowJson::default()));
                }
                _ => return field,
            };
            Arc::new(field.as_ref().clone().with_data_type(tagged))
        }

        let (schema, _) = infer_json_schema(Cursor::new(text.as_bytes()), None).unwrap();
        let schema: SchemaRef =
            Arc::new(Schema::new(schema.fields().iter().map(|f| tag_json_blobs(f.clone())).collect::<Vec<_>>()));
        let mut entries: Vec<(String, Box<FoldTree>)> = schema
            .fields()
            .iter()
            .map(|f| {
                let name = f.name().strip_suffix(BLOB_SUFFIX).unwrap_or(f.name()).to_string();
                (name, Box::new(FoldTree::from_field(f)))
            })
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let fold_root = FoldTree::Object(entries);
        let mut reader =
            ReaderBuilder::new(schema.clone()).with_batch_size(n).build(Cursor::new(text.as_bytes())).unwrap();
        let batch = reader.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), n, "vortex json NDJSON row count");
        assert!(reader.next().is_none(), "vortex json NDJSON batch count");

        let scalars = serde_arrow::to_record_batch(&scalar_fields, &scalar_cols).unwrap();
        let mut fields: Vec<FieldRef> = scalar_fields.clone();
        fields.extend(batch.schema().fields().iter().cloned());
        let mut cols = scalars.columns().to_vec();
        cols.extend(batch.columns().iter().cloned());
        let joined = arrow_array::RecordBatch::try_new(Arc::new(Schema::new(fields)), cols).unwrap();

        let joined_schema = joined.schema();
        rt.block_on(async {
            let array = session.arrow().from_arrow_record_batch(joined, &joined_schema).unwrap();
            let mut out: Vec<u8> = vec![];
            let strategy = WriteStrategyBuilder::from_session(session)
                .with_btrblocks_builder(BtrBlocksCompressorBuilder::from_session(session).with_compact())
                .build();
            session.write_options().with_strategy(strategy).write(&mut out, array.to_array_stream()).await.unwrap();
            (out, fold_root)
        })
    });
    drop(text);
    drop(scalar_cols);
    let buffer = ByteBuffer::from(bytes.clone());

    // Read path: scan (optionally filtered) and materialize back to arrow.
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
                array.execute_record_batch(&schema_out, &mut ctx).unwrap()
            }
        })
    };

    // Rebuilt plan fingerprint per row: fold the exported columns through
    // the schema tree, reparsing vortex.json blob leaves (unmeasured).
    let rebuilt: Vec<u64> = {
        let back = read(None);
        assert_eq!(back.num_rows(), n, "vortex json row count");
        let FoldTree::Object(entries) = &fold_root else {
            panic!("vortex json fold: inferred schema root is not an object");
        };
        let (fields, cols): (Vec<FieldRef>, Vec<arrow_array::ArrayRef>) = entries
            .iter()
            .map(|(name, tree)| {
                let colname =
                    if matches!(tree.as_ref(), FoldTree::Blob) { format!("{name}{BLOB_SUFFIX}") } else { name.clone() };
                let field = Arc::new(back.schema().field_with_name(&colname).unwrap().clone());
                let col = back
                    .column_by_name(&colname)
                    .unwrap_or_else(|| panic!("vortex json export: missing column {colname}"))
                    .clone();
                (field, col)
            })
            .unzip();
        let json_struct = arrow_array::StructArray::try_new(fields.into(), cols, None).unwrap();
        (0..n)
            .map(|i| {
                let mut h = 0xcbf2_9ce4_8422_2325;
                fold_root.fold(&json_struct, i, &mut h);
                h
            })
            .collect()
    };
    let mut got = rebuilt.clone();
    let mut exp = expected.clone();
    got.sort_unstable();
    exp.sort_unstable();
    assert_eq!(
        got, exp,
        "vortex json: rebuilt plan columns must match the stored JSON modulo null/absent folding (as a multiset)"
    );

    let (deserial_ms, batch) = crate::ds_common::one_pass(|| read(None));
    assert_eq!(batch.num_rows(), n, "vortex json deserialize row count");
    std::hint::black_box(batch);

    let mut filter_ms = Vec::new();
    for (k, expected) in kf {
        let (ms, batch) = crate::ds_common::one_pass(|| read(Some(build_expr(k))));
        assert_eq!(batch.num_rows(), *expected, "vortex json filter count");
        filter_ms.push(Some(ms));
    }

    common::CodecMetrics {
        serial_ms: Some(serial_ms),
        deserial_ms: Some(deserial_ms),
        size: Some(bytes.len()),
        filter_ms,
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
