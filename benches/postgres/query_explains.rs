//! query_explains: the full query_explains_7d window (237,651 analyzed
//! queries). Mirrors the source table exactly (field order/names/types) so
//! the chunk layout matches how the table is stored: index [server_id],
//! timestamp seen_at. UUID remapping and fingerprint folding are done by
//! src/bin's extractor.
//!
//! Every row carries the full EXPLAIN (FORMAT JSON) document, averaging ~9
//! KB of nested JSON, so the payload is a zstd contest over `output_json`.
//! Float columns carry millisecond values as decimals.

use chrono::{DateTime, Utc};
use pco_pack::PcoPack;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use super::common::{self, CodecRowLossy, cell};

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

/// The lossy model (`PcoPack (lossy)` column): `time_round = 1 s` on
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
    bench_explains(&rows);
    Ok(())
}

fn bench_explains(rows: &[QueryExplain]) {
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

    let pco = common::bench_pco_pack(rows, &pco_filters, |back| {
        assert_eq!(back.len(), rows.len(), "query_explains row count")
    });

    // Window and slow-plan filter counts fold through the stored rounded
    // values.
    let lossy_rows: Vec<QueryExplainLossy> = rows.iter().map(QueryExplainLossy::from).collect();
    let lossy_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_lossy.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let pco_lossy = common::bench_pco_pack(&lossy_rows, &lossy_filters, |back| {
        assert_eq!(back.len(), rows.len(), "query_explains (lossy) row count")
    });
    drop(lossy_rows);

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

    let mp = common::bench_msgpack(rows, &kf, |r: &QueryExplain, k: &Ek| k.matches(r));

    let metrics = ["Size", "Serialize", "Deserialize"];
    let mut out: Vec<CodecRowLossy> = metrics
        .iter()
        .map(|m| CodecRowLossy {
            metric: m.to_string(),
            pco_pack: cell(&pco, m, None),
            pco_pack_lossy: cell(&pco_lossy, m, None),
            vortex: cell(&vx, m, None),
            columnar: cell(&col, m, None),
            serde_columnar: cell(&sc, m, None),
            msgpack: cell(&mp, m, None),
        })
        .collect();
    for (i, label) in label_refs.iter().enumerate() {
        out.push(CodecRowLossy {
            metric: label.to_string(),
            pco_pack: cell(&pco, "Filter", Some(i)),
            pco_pack_lossy: cell(&pco_lossy, "Filter", Some(i)),
            vortex: cell(&vx, "Filter", Some(i)),
            columnar: cell(&col, "Filter", Some(i)),
            serde_columnar: cell(&sc, "Filter", Some(i)),
            msgpack: cell(&mp, "Filter", Some(i)),
        });
    }
    println!("{}", markdown_tables::as_table(&out));
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
