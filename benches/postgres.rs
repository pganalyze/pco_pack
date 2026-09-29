//! Benchmarks over real pganalyze monitoring data captured from a live pgaweb
//! deployment, the only private datasets here. The neutral msgpack+zstd
//! captures are committed as the benchmark's input data via Git LFS.
//! query_stats covers the capture's last 14 days (full: 114.3M rows, 748 MB;
//! kept: 31.0M rows, 164 MB) to keep LFS hosting and runtimes sane. See
//! benches/README.md for the capture and trim flow.
//!
//!   cargo bench --bench postgres
//!
//! One codec-comparison table per data source, in datasets-bench format:
//! `backend_snapshots` and `query_stats` per granularity. Every codec
//! re-encodes the same loaded rows (the datasets-bench convention): PcoPack
//! full-fidelity, PcoPack time_round (what production persists: timestamps
//! rounded to whole seconds, `float_round = 2` on the query_stats f64
//! aggregates), Vortex, columnar, serde_columnar, and msgpack+zstd.
//! `backend_snapshots` adds a `PcoPack Timeline` column, the shape from
//! `benches/timeline.rs`: snapshots whose payload is unchanged between ticks
//! collapse to one row whose `Timeline<0>` holds every tick they were
//! observed at.

include!("bench_common.rs");

// Shared codec runners and table printer from the datasets bench, compiled
// against this binary's root (same `use super::*` contract as
// `datasets/common.rs`).
#[allow(dead_code)]
#[path = "datasets/common.rs"]
mod common;

use common::CodecMetrics;
use anyhow::Context as _;
use chrono::{DateTime, Utc};
use pco_pack::{PcoPack, Timeline, rmp_serde, zstd};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use backend_snapshot::{BackendSnapshot, BackendSnapshotKey, BackendSnapshotTimeline, BackendSnapshotTimeRound};
use query_stats::{QueryStat, QueryStatTimeRound};

fn main() -> anyhow::Result<()> {
    println!("## Postgres\n");

    let rows: Vec<BackendSnapshot> = read_blob("backend_snapshots.msgpack.zst")?;
    bench_backends(&rows);
    drop(rows);

    let all: Vec<QueryStat> = read_blob("query_stats.msgpack.zst")?;
    let mut per_min: Vec<QueryStat> = Vec::new();
    let mut per_hour: Vec<QueryStat> = Vec::new();
    for row in all {
        if row.granularity == 3600 {
            per_hour.push(row);
        } else {
            per_min.push(row);
        }
    }
    if !per_min.is_empty() {
        bench_stats("query_stats_1min", per_min);
    }
    if !per_hour.is_empty() {
        bench_stats("query_stats_1hour", per_hour);
    }
    Ok(())
}

// ---- Per-source benches ----
// PcoPack roundtrip verifies are counts-only, the convention in this repo for
// indexed structs (see struct.rs): an indexed `write` groups rows into chunks
// by index key, so `read` returns them index-grouped, not in stored order,
// and full-order row asserts are wrong here. Filter rows stay asserted
// against counts computed from each codec's own stored precision (the
// TimeRound expectations fold collected_at through `time_round = 1 s`).

fn bench_stats(name: &str, rows: Vec<QueryStat>) {
    let n = rows.len();
    let last_us = rows.iter().map(|r| r.collected_at.timestamp_micros()).max().unwrap();
    let db0 = rows[0].database_id;

    let keys = [Sk::Time(last_us - 3_600_000_000, last_us), Sk::Loud, Sk::Db(db0)];
    let kf: Vec<(Sk, usize)> =
        keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches(r)).count())).collect();
    let labels: Vec<String> = kf.iter().map(|(_, e)| common::filter_label(*e, n)).collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    println!("### {name} ({n} rows)\n");

    let queries: Vec<serde_json::Value> = vec![
        serde_json::json!({"collected_at": {"start": last_us - 3_600_000_000, "end": last_us}}),
        serde_json::json!({"toplevel": true, "calls": {"start": 10, "end": i64::MAX}}),
        serde_json::json!({"database_id": db0}),
    ];
    let pco_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf.iter()).map(|(q, (_, e))| (q, *e)).collect();

    let pco = common::bench_pco_pack(&rows, &pco_filters, |back| {
        assert_eq!(back.len(), rows.len(), "{name} row count")
    });

    // The TimeRound write rounds collected_at to whole seconds, so the window
    // filter's expected count folds collected_at through the same rounding;
    // float_round rounds no filter column.
    let time_round_rows: Vec<QueryStatTimeRound> =
        rows.iter().map(QueryStatTimeRound::from).collect();
    let kf_time_round: Vec<(Sk, usize)> = keys
        .iter()
        .map(|k| (*k, rows.iter().filter(|r| k.matches_time_round(r)).count()))
        .collect();
    let time_round_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_time_round.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let pco_time_round = common::bench_pco_pack(&time_round_rows, &time_round_filters, |back| {
        assert_eq!(back.len(), rows.len(), "{name} (TimeRound) row count")
    });
    drop(time_round_rows);

    let col_rows: Vec<QueryStatCol> = rows.iter().map(QueryStatCol::from).collect();
    let col = common::bench_columnar(n, move || col_rows, &kf, |decoded: &columnar::BorrowedOf<QueryStatCol>, k: &Sk| {
        let count = columnar::Len::len(decoded);
        (0..count)
            .filter(|&i| {
                let r = columnar::Index::get(decoded, i);
                match k {
                    Sk::Time(a, b) => {
                        let us = *r.collected_at;
                        *a <= us && us <= *b
                    }
                    Sk::Loud => r.toplevel && *r.calls >= 10,
                    Sk::Db(id) => *r.database_id == *id,
                }
            })
            .count()
    });

    let vx_rows: Vec<QueryStatCol> = rows.iter().map(QueryStatCol::from).collect();
    let vx = common::bench_vortex_ds(&vx_rows, &kf, |k: &Sk| {
        use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq};
        match k {
            Sk::Time(a, b) => and(
                gt_eq(col("collected_at"), lit(*a)),
                lt_eq(col("collected_at"), lit(*b)),
            ),
            Sk::Loud => and(eq(col("toplevel"), lit(true)), gt_eq(col("calls"), lit(10i64))),
            Sk::Db(id) => eq(col("database_id"), lit(*id)),
        }
    });
    drop(vx_rows);

    let sc = common::bench_serde_columnar(
        n,
        || stats_sc::QueryStatsScStore {
            data: rows.iter().map(stats_sc::QueryStatsScRow::from).collect(),
        },
        &kf,
        |s: &stats_sc::QueryStatsScStore, k: &Sk| {
            s.data
                .iter()
                .filter(|r| match k {
                    Sk::Time(a, b) => *a <= r.collected_at && r.collected_at <= *b,
                    Sk::Loud => r.toplevel && r.calls >= 10,
                    Sk::Db(id) => r.database_id == *id,
                })
                .count()
        },
    );

    let mp = common::bench_msgpack(&rows, &kf, |r: &QueryStat, k: &Sk| k.matches(r));

    let metrics = ["Serialize", "Deserialize", "Size"];
    let mut out: Vec<CodecRow> = metrics
        .iter()
        .map(|m| CodecRow {
            metric: m.to_string(),
            pco_pack: cell(&pco, m, None),
            pco_time_round: cell(&pco_time_round, m, None),
            vortex: cell(&vx, m, None),
            columnar: cell(&col, m, None),
            serde_columnar: cell(&sc, m, None),
            msgpack: cell(&mp, m, None),
        })
        .collect();
    for (i, label) in label_refs.iter().enumerate() {
        out.push(CodecRow {
            metric: label.to_string(),
            pco_pack: cell(&pco, "Filter", Some(i)),
            pco_time_round: cell(&pco_time_round, "Filter", Some(i)),
            vortex: cell(&vx, "Filter", Some(i)),
            columnar: cell(&col, "Filter", Some(i)),
            serde_columnar: cell(&sc, "Filter", Some(i)),
            msgpack: cell(&mp, "Filter", Some(i)),
        });
    }
    println!("{}", markdown_tables::as_table(&out));
    println!();
}

/// backend_snapshots: one codec table over the per-tick rows, plus the
/// `PcoPack Timeline` column, the deduped shape (see `benches/timeline.rs`):
/// snapshots whose non-timestamp payload is unchanged between ticks collapse
/// into one row whose `Timeline<0>` holds every tick they were observed at,
/// a lossless fold at exact us. Timeline filters switch to range-overlap
/// semantics (a row matches if any of its ticks falls in the window) and
/// verify counts over the deduped rows themselves (the same queries cover
/// different fractions of the two row sets).
fn bench_backends(rows: &[BackendSnapshot]) {
    let n = rows.len();
    let last_us = rows.iter().map(|r| r.collected_at.timestamp_micros()).max().unwrap();

    let keys = [Bk::Time(last_us - 3_600_000_000, last_us), Bk::Active, Bk::Waits];
    let kf: Vec<(Bk, usize)> =
        keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches(r)).count())).collect();
    let labels: Vec<String> = kf.iter().map(|(_, e)| common::filter_label(*e, n)).collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

    println!("### backend_snapshots ({n} rows)\n");

    let queries: Vec<serde_json::Value> = vec![
        serde_json::json!({"collected_at": {"start": last_us - 3_600_000_000, "end": last_us}}),
        serde_json::json!({"backend_state": [1, 2]}),
        serde_json::json!({"wait_event": {"start": 1, "end": i64::from(i16::MAX)}}),
    ];
    let pco_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf.iter()).map(|(q, (_, e))| (q, *e)).collect();

    let pco = common::bench_pco_pack(rows, &pco_filters, |back| {
        assert_eq!(back.len(), rows.len(), "backend_snapshots row count")
    });

    // The time_round = 1 s write stores rounded collected_at values, so the
    // window filter's expected count is computed over those stored values.
    let time_round_rows: Vec<BackendSnapshotTimeRound> =
        rows.iter().map(BackendSnapshotTimeRound::from).collect();
    let kf_time_round: Vec<(Bk, usize)> =
        keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches_time_round(r)).count())).collect();
    let time_round_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_time_round.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let pco_time_round = common::bench_pco_pack(&time_round_rows, &time_round_filters, |back| {
        assert_eq!(back.len(), rows.len(), "backend_snapshots (TimeRound) row count")
    });
    drop(time_round_rows);

    let col_rows: Vec<BackendSnapshotCol> = rows.iter().map(BackendSnapshotCol::from).collect();
    let col = common::bench_columnar(
        n,
        move || col_rows,
        &kf,
        |decoded: &columnar::BorrowedOf<BackendSnapshotCol>, k: &Bk| {
            let count = columnar::Len::len(decoded);
            (0..count)
                .filter(|&i| {
                    let r = columnar::Index::get(decoded, i);
                    match k {
                        Bk::Time(a, b) => {
                            let us = *r.collected_at;
                            *a <= us && us <= *b
                        }
                        Bk::Active => matches!(*r.backend_state, 1 | 2),
                        Bk::Waits => *r.wait_event >= 1,
                    }
                })
                .count()
        },
    );

    let vx_rows: Vec<BackendSnapshotCol> = rows.iter().map(BackendSnapshotCol::from).collect();
    let vx = common::bench_vortex_ds(&vx_rows, &kf, |k: &Bk| {
        use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq, or};
        match k {
            Bk::Time(a, b) => and(
                gt_eq(col("collected_at"), lit(*a)),
                lt_eq(col("collected_at"), lit(*b)),
            ),
            Bk::Active => or(
                eq(col("backend_state"), lit(1i8)),
                eq(col("backend_state"), lit(2i8)),
            ),
            Bk::Waits => gt_eq(col("wait_event"), lit(1i16)),
        }
    });
    drop(vx_rows);

    let sc = common::bench_serde_columnar(
        n,
        || backends_sc::BackendsScStore {
            data: rows.iter().map(backends_sc::BackendsScRow::from).collect(),
        },
        &kf,
        |s: &backends_sc::BackendsScStore, k: &Bk| {
            s.data
                .iter()
                .filter(|r| match k {
                    Bk::Time(a, b) => *a <= r.collected_at && r.collected_at <= *b,
                    Bk::Active => matches!(r.backend_state, 1 | 2),
                    Bk::Waits => r.wait_event >= 1,
                })
                .count()
        },
    );

    let mp = common::bench_msgpack(rows, &kf, |r: &BackendSnapshot, k: &Bk| k.matches(r));

    // PcoPack Timeline: fold every distinct payload into one row carrying the
    // union Timeline of the ticks it was observed at.
    let mut groups: HashMap<BackendSnapshotKey, Timeline<0>> = HashMap::new();
    for r in rows {
        let ts = r.collected_at.timestamp_micros();
        groups.entry(BackendSnapshotKey::from(r)).or_default().add(ts, ts);
    }
    let mut tl_rows: Vec<BackendSnapshotTimeline> = groups.iter().map(BackendSnapshotTimeline::from).collect();
    drop(groups);
    // HashMap order is nondeterministic; sort so chunk layout (and thus the
    // captured size numbers) are reproducible across runs.
    tl_rows.sort_by_key(|r| (r.server_id, r.backend_pid, r.collected_at.start().unwrap_or(0)));
    let nd = tl_rows.len();
    let kf_tl: Vec<(Bk, usize)> = keys
        .iter()
        .map(|k| (*k, tl_rows.iter().filter(|r| k.matches_timeline(r)).count()))
        .collect();
    let tl_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_tl.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let pco_timeline = common::bench_pco_pack(&tl_rows, &tl_filters, |back| {
        assert_eq!(back.len(), nd, "backend_snapshots (Timeline) row count")
    });

    let metrics = ["Serialize", "Deserialize", "Size"];
    let mut out: Vec<CodecRowTimeline> = metrics
        .iter()
        .map(|m| CodecRowTimeline {
            metric: m.to_string(),
            pco_pack: cell(&pco, m, None),
            pco_time_round: cell(&pco_time_round, m, None),
            pco_timeline: cell(&pco_timeline, m, None),
            vortex: cell(&vx, m, None),
            columnar: cell(&col, m, None),
            serde_columnar: cell(&sc, m, None),
            msgpack: cell(&mp, m, None),
        })
        .collect();
    for (i, label) in label_refs.iter().enumerate() {
        out.push(CodecRowTimeline {
            metric: label.to_string(),
            pco_pack: cell(&pco, "Filter", Some(i)),
            pco_time_round: cell(&pco_time_round, "Filter", Some(i)),
            pco_timeline: cell(&pco_timeline, "Filter", Some(i)),
            vortex: cell(&vx, "Filter", Some(i)),
            columnar: cell(&col, "Filter", Some(i)),
            serde_columnar: cell(&sc, "Filter", Some(i)),
            msgpack: cell(&mp, "Filter", Some(i)),
        });
    }
    println!("{}", markdown_tables::as_table(&out));
    println!("({nd} of {n} ticks survive the Timeline fold)\n");
}

// ---- Filter keys: the production queries, shared by every codec runner ----

/// query_stats: last-1h window on collected_at; toplevel with calls >= 10;
/// the first database seen in the capture.
#[derive(Clone, Copy)]
enum Sk {
    Time(i64, i64),
    Loud,
    Db(i64),
}

impl Sk {
    fn matches(&self, r: &QueryStat) -> bool {
        let us = r.collected_at.timestamp_micros();
        match self {
            Sk::Time(a, b) => *a <= us && us <= *b,
            Sk::Loud => r.toplevel && r.calls >= 10,
            Sk::Db(id) => r.database_id == *id,
        }
    }

    /// Same filter over the `time_round = 1 s` value the TimeRound write
    /// stores (only the window filter touches timestamps; `float_round`
    /// rounds no filter column).
    fn matches_time_round(&self, r: &QueryStat) -> bool {
        match self {
            Sk::Time(a, b) => {
                let us = common::ts_rounded(&r.collected_at, 1_000_000).timestamp_micros();
                *a <= us && us <= *b
            }
            _ => self.matches(r),
        }
    }
}

/// backends: last-1h window on collected_at; active states (1 = active,
/// 2 = idle in transaction); any backend with a wait event.
#[derive(Clone, Copy)]
enum Bk {
    Time(i64, i64),
    Active,
    Waits,
}

impl Bk {
    fn matches(&self, r: &BackendSnapshot) -> bool {
        let us = r.collected_at.timestamp_micros();
        match self {
            Bk::Time(a, b) => *a <= us && us <= *b,
            Bk::Active => matches!(r.backend_state, 1 | 2),
            Bk::Waits => r.wait_event >= 1,
        }
    }

    /// Same filter over the `time_round = 1 s` value the TimeRound write
    /// stores (only the window filter touches timestamps).
    fn matches_time_round(&self, r: &BackendSnapshot) -> bool {
        match self {
            Bk::Time(a, b) => {
                let us = common::ts_rounded(&r.collected_at, 1_000_000).timestamp_micros();
                *a <= us && us <= *b
            }
            _ => self.matches(r),
        }
    }

    /// Same filter over a timeline-deduped row: the window matches iff any
    /// of the row's ticks falls in it (range overlap, inclusive).
    fn matches_timeline(&self, r: &BackendSnapshotTimeline) -> bool {
        match self {
            Bk::Time(a, b) => r.collected_at.overlaps(*a, *b),
            Bk::Active => matches!(r.backend_state, 1 | 2),
            Bk::Waits => r.wait_event >= 1,
        }
    }
}

/// Mirrors pganalyze's QueryStat exactly (field order/names/types) so the chunk
/// layout matches production: index [database_id, granularity], timestamp
/// collected_at. No rounding knob, so the `PcoPack` column is full-fidelity
/// like every competitor's.
mod query_stats {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[derive(PcoPack)]
    #[pco_pack(timestamp = collected_at, index = [database_id, granularity])]
    pub struct QueryStat {
        pub database_id: i64,
        pub granularity: i32,
        pub collected_at: DateTime<Utc>,
        pub collected_secs: i64,
        pub fingerprint: i64,
        pub postgres_role_id: i64,
        pub toplevel: bool,
        pub calls: i64,
        pub rows: i64,
        pub total_time: f64,
        pub io_time: f64,
        pub shared_blks_hit: i64,
        pub shared_blks_read: i64,
        pub temp_blks_read: i64,
        pub temp_blks_written: i64,
    }

    /// The TimeRound model: production writes this table to pco_store rounded
    /// (`float_round = 2` on the aggregate f64s, `time_round = 1 s` on the
    /// timestamps), so stored rows differ from the capture by rounding only.
    /// This is the `PcoPack time_round` column.
    #[derive(Clone, Debug, PartialEq)]
    #[derive(PcoPack)]
    #[pco_pack(
        timestamp = collected_at,
        index = [database_id, granularity],
        time_round = chrono::Duration::seconds(1),
        float_round = 2
    )]
    pub struct QueryStatTimeRound {
        pub database_id: i64,
        pub granularity: i32,
        pub collected_at: DateTime<Utc>,
        pub collected_secs: i64,
        pub fingerprint: i64,
        pub postgres_role_id: i64,
        pub toplevel: bool,
        pub calls: i64,
        pub rows: i64,
        pub total_time: f64,
        pub io_time: f64,
        pub shared_blks_hit: i64,
        pub shared_blks_read: i64,
        pub temp_blks_read: i64,
        pub temp_blks_written: i64,
    }

    impl From<&QueryStat> for QueryStatTimeRound {
        fn from(r: &QueryStat) -> Self {
            QueryStatTimeRound {
                database_id: r.database_id,
                granularity: r.granularity,
                collected_at: r.collected_at,
                collected_secs: r.collected_secs,
                fingerprint: r.fingerprint,
                postgres_role_id: r.postgres_role_id,
                toplevel: r.toplevel,
                calls: r.calls,
                rows: r.rows,
                total_time: r.total_time,
                io_time: r.io_time,
                shared_blks_hit: r.shared_blks_hit,
                shared_blks_read: r.shared_blks_read,
                temp_blks_read: r.temp_blks_read,
                temp_blks_written: r.temp_blks_written,
            }
        }
    }
}

/// Mirrors pganalyze's BackendSnapshot exactly (field order/names/types) so the
/// chunk layout matches the production backfill: index [server_id, granularity],
/// timestamp collected_at. No rounding knob (full-fidelity baseline).
mod backend_snapshot {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[derive(PcoPack)]
    #[pco_pack(index = [server_id, granularity], timestamp = collected_at)]
    pub struct BackendSnapshot {
        pub server_id: i64,
        pub granularity: i32,
        pub collected_at: DateTime<Utc>,
        pub database_id: Option<i64>,
        pub backend_pid: i32,
        pub backend_start: DateTime<Utc>,
        pub backend_type: i8,
        pub backend_state: i8,
        pub wait_event_type: i8,
        pub wait_event: i16,
        pub query_start: DateTime<Utc>,
        pub query_fingerprint: Option<i64>,
        pub postgres_role_id: Option<i64>,
        pub blocked_by_pids: Vec<i32>,
        pub application_name: SmolStr,
        pub client_addr: SmolStr,
    }

    /// The TimeRound model (see `QueryStatTimeRound`): production writes
    /// backend snapshots with `time_round = 1 s`. Tick jitter carries no
    /// signal, and rounding the us timestamps shrinks the timestamp columns.
    /// This is the `PcoPack time_round` column.
    #[derive(Clone, Debug, Default, PartialEq)]
    #[derive(PcoPack)]
    #[pco_pack(
        index = [server_id, granularity],
        timestamp = collected_at,
        time_round = chrono::Duration::seconds(1)
    )]
    pub struct BackendSnapshotTimeRound {
        pub server_id: i64,
        pub granularity: i32,
        pub collected_at: DateTime<Utc>,
        pub database_id: Option<i64>,
        pub backend_pid: i32,
        pub backend_start: DateTime<Utc>,
        pub backend_type: i8,
        pub backend_state: i8,
        pub wait_event_type: i8,
        pub wait_event: i16,
        pub query_start: DateTime<Utc>,
        pub query_fingerprint: Option<i64>,
        pub postgres_role_id: Option<i64>,
        pub blocked_by_pids: Vec<i32>,
        pub application_name: SmolStr,
        pub client_addr: SmolStr,
    }

    impl From<&BackendSnapshot> for BackendSnapshotTimeRound {
        fn from(r: &BackendSnapshot) -> Self {
            BackendSnapshotTimeRound {
                server_id: r.server_id,
                granularity: r.granularity,
                collected_at: r.collected_at,
                database_id: r.database_id,
                backend_pid: r.backend_pid,
                backend_start: r.backend_start,
                backend_type: r.backend_type,
                backend_state: r.backend_state,
                wait_event_type: r.wait_event_type,
                wait_event: r.wait_event,
                query_start: r.query_start,
                query_fingerprint: r.query_fingerprint,
                postgres_role_id: r.postgres_role_id,
                blocked_by_pids: r.blocked_by_pids.clone(),
                application_name: r.application_name.clone(),
                client_addr: r.client_addr.clone(),
            }
        }
    }

    /// Everything but `collected_at`: two snapshots with equal keys are the
    /// same backend state seen at different ticks and dedup to one row.
    #[derive(Hash, PartialEq, Eq)]
    pub struct BackendSnapshotKey {
        pub server_id: i64,
        pub granularity: i32,
        pub database_id: Option<i64>,
        pub backend_pid: i32,
        pub backend_start: DateTime<Utc>,
        pub backend_type: i8,
        pub backend_state: i8,
        pub wait_event_type: i8,
        pub wait_event: i16,
        pub query_start: DateTime<Utc>,
        pub query_fingerprint: Option<i64>,
        pub postgres_role_id: Option<i64>,
        pub blocked_by_pids: Vec<i32>,
        pub application_name: SmolStr,
        pub client_addr: SmolStr,
    }

    impl From<&BackendSnapshot> for BackendSnapshotKey {
        fn from(r: &BackendSnapshot) -> Self {
            BackendSnapshotKey {
                server_id: r.server_id,
                granularity: r.granularity,
                database_id: r.database_id,
                backend_pid: r.backend_pid,
                backend_start: r.backend_start,
                backend_type: r.backend_type,
                backend_state: r.backend_state,
                wait_event_type: r.wait_event_type,
                wait_event: r.wait_event,
                query_start: r.query_start,
                query_fingerprint: r.query_fingerprint,
                postgres_role_id: r.postgres_role_id,
                blocked_by_pids: r.blocked_by_pids.clone(),
                application_name: r.application_name.clone(),
                client_addr: r.client_addr.clone(),
            }
        }
    }

    /// The deduped shape (see `benches/timeline.rs`'s `TimelineRecord`): one
    /// row per distinct backend state, every tick it was observed at folded
    /// into an exact-us `Timeline` (lossless; bucketing would lie about the
    /// collection time). Same index layout as production.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[derive(PcoPack)]
    #[pco_pack(index = [server_id, granularity], timestamp = collected_at)]
    pub struct BackendSnapshotTimeline {
        pub server_id: i64,
        pub granularity: i32,
        pub collected_at: Timeline<0>,
        pub database_id: Option<i64>,
        pub backend_pid: i32,
        pub backend_start: DateTime<Utc>,
        pub backend_type: i8,
        pub backend_state: i8,
        pub wait_event_type: i8,
        pub wait_event: i16,
        pub query_start: DateTime<Utc>,
        pub query_fingerprint: Option<i64>,
        pub postgres_role_id: Option<i64>,
        pub blocked_by_pids: Vec<i32>,
        pub application_name: SmolStr,
        pub client_addr: SmolStr,
    }
}

// ---- Mirror structs ----

// columnar has no chrono/SmolStr impls: timestamps mirror to i64 us (exact
// here; pg timestamps are us), strings to String. The same mirrors feed
// Vortex (serde_arrow): numeric range filters, no unsound RFC3339 columns.

#[derive(Clone, Debug, PartialEq, columnar::Columnar, Serialize, Deserialize)]
pub struct QueryStatCol {
    pub database_id: i64,
    pub granularity: i32,
    pub collected_at: i64,
    pub collected_secs: i64,
    pub fingerprint: i64,
    pub postgres_role_id: i64,
    pub toplevel: bool,
    pub calls: i64,
    pub rows: i64,
    pub total_time: f64,
    pub io_time: f64,
    pub shared_blks_hit: i64,
    pub shared_blks_read: i64,
    pub temp_blks_read: i64,
    pub temp_blks_written: i64,
}

impl From<&QueryStat> for QueryStatCol {
    fn from(r: &QueryStat) -> Self {
        QueryStatCol {
            database_id: r.database_id,
            granularity: r.granularity,
            collected_at: r.collected_at.timestamp_micros(),
            collected_secs: r.collected_secs,
            fingerprint: r.fingerprint,
            postgres_role_id: r.postgres_role_id,
            toplevel: r.toplevel,
            calls: r.calls,
            rows: r.rows,
            total_time: r.total_time,
            io_time: r.io_time,
            shared_blks_hit: r.shared_blks_hit,
            shared_blks_read: r.shared_blks_read,
            temp_blks_read: r.temp_blks_read,
            temp_blks_written: r.temp_blks_written,
        }
    }
}

#[derive(Clone, Debug, PartialEq, columnar::Columnar, Serialize, Deserialize)]
pub struct BackendSnapshotCol {
    pub server_id: i64,
    pub granularity: i32,
    pub collected_at: i64,
    pub database_id: Option<i64>,
    pub backend_pid: i32,
    pub backend_start: i64,
    pub backend_type: i8,
    pub backend_state: i8,
    pub wait_event_type: i8,
    pub wait_event: i16,
    pub query_start: i64,
    pub query_fingerprint: Option<i64>,
    pub postgres_role_id: Option<i64>,
    pub blocked_by_pids: Vec<i32>,
    pub application_name: String,
    pub client_addr: String,
}

impl From<&BackendSnapshot> for BackendSnapshotCol {
    fn from(r: &BackendSnapshot) -> Self {
        BackendSnapshotCol {
            server_id: r.server_id,
            granularity: r.granularity,
            collected_at: r.collected_at.timestamp_micros(),
            database_id: r.database_id,
            backend_pid: r.backend_pid,
            backend_start: r.backend_start.timestamp_micros(),
            backend_type: r.backend_type,
            backend_state: r.backend_state,
            wait_event_type: r.wait_event_type,
            wait_event: r.wait_event,
            query_start: r.query_start.timestamp_micros(),
            query_fingerprint: r.query_fingerprint,
            postgres_role_id: r.postgres_role_id,
            blocked_by_pids: r.blocked_by_pids.clone(),
            application_name: r.application_name.to_string(),
            client_addr: r.client_addr.to_string(),
        }
    }
}

// ---- serde_columnar mirrors (strategies follow the taxi/osm convention) ----

mod stats_sc {
    use super::*;
    use serde_columnar::columnar;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct QueryStatsScRow {
        #[columnar(strategy = "DeltaRle")]
        pub collected_at: i64,
        #[columnar(strategy = "DeltaRle")]
        pub collected_secs: i64,
        #[columnar(strategy = "DeltaRle")]
        pub calls: i64,
        #[columnar(strategy = "Rle")]
        pub total_time: f64,
        #[columnar(strategy = "Rle")]
        pub io_time: f64,
        pub database_id: i64,
        pub granularity: i32,
        pub fingerprint: i64,
        pub postgres_role_id: i64,
        pub toplevel: bool,
        pub rows: i64,
        pub shared_blks_hit: i64,
        pub shared_blks_read: i64,
        pub temp_blks_read: i64,
        pub temp_blks_written: i64,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct QueryStatsScStore {
        #[columnar(class = "vec")]
        pub data: Vec<QueryStatsScRow>,
    }

    impl From<&QueryStat> for QueryStatsScRow {
        fn from(r: &QueryStat) -> Self {
            QueryStatsScRow {
                collected_at: r.collected_at.timestamp_micros(),
                collected_secs: r.collected_secs,
                calls: r.calls,
                total_time: r.total_time,
                io_time: r.io_time,
                database_id: r.database_id,
                granularity: r.granularity,
                fingerprint: r.fingerprint,
                postgres_role_id: r.postgres_role_id,
                toplevel: r.toplevel,
                rows: r.rows,
                shared_blks_hit: r.shared_blks_hit,
                shared_blks_read: r.shared_blks_read,
                temp_blks_read: r.temp_blks_read,
                temp_blks_written: r.temp_blks_written,
            }
        }
    }
}

mod backends_sc {
    use super::*;
    use serde_columnar::columnar;

    #[columnar(vec, ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct BackendsScRow {
        #[columnar(strategy = "DeltaRle")]
        pub collected_at: i64,
        #[columnar(strategy = "DeltaRle")]
        pub backend_start: i64,
        #[columnar(strategy = "Rle")]
        pub backend_type: i8,
        #[columnar(strategy = "Rle")]
        pub backend_state: i8,
        #[columnar(strategy = "Rle")]
        pub wait_event_type: i8,
        #[columnar(strategy = "Rle")]
        pub wait_event: i16,
        #[columnar(strategy = "Rle")]
        pub application_name: String,
        #[columnar(strategy = "Rle")]
        pub client_addr: String,
        pub server_id: i64,
        pub granularity: i32,
        pub database_id: Option<i64>,
        pub backend_pid: i32,
        pub query_start: i64,
        pub query_fingerprint: Option<i64>,
        pub postgres_role_id: Option<i64>,
        pub blocked_by_pids: Vec<i32>,
    }

    #[columnar(ser, de)]
    #[derive(Debug, Clone, PartialEq)]
    pub struct BackendsScStore {
        #[columnar(class = "vec")]
        pub data: Vec<BackendsScRow>,
    }

    impl From<&BackendSnapshot> for BackendsScRow {
        fn from(r: &BackendSnapshot) -> Self {
            BackendsScRow {
                collected_at: r.collected_at.timestamp_micros(),
                backend_start: r.backend_start.timestamp_micros(),
                backend_type: r.backend_type,
                backend_state: r.backend_state,
                wait_event_type: r.wait_event_type,
                wait_event: r.wait_event,
                application_name: r.application_name.to_string(),
                client_addr: r.client_addr.to_string(),
                server_id: r.server_id,
                granularity: r.granularity,
                database_id: r.database_id,
                backend_pid: r.backend_pid,
                query_start: r.query_start.timestamp_micros(),
                query_fingerprint: r.query_fingerprint,
                postgres_role_id: r.postgres_role_id,
                blocked_by_pids: r.blocked_by_pids.clone(),
            }
        }
    }
}

impl common::ScStore for stats_sc::QueryStatsScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

impl common::ScStore for backends_sc::BackendsScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}

// ---- Table printer ----
// Same datasets-bench layout, but with postgres-specific columns: the lossy
// column is spelled `PcoPack time_round` (the shared printer says `(lossy)`
// because the public datasets round with `float_round` too: openalex, osm),
// and backend_snapshots gains `PcoPack Timeline` for the deduped shape.

#[derive(Debug)]
struct CodecRow {
    metric: String,
    pco_pack: String,
    pco_time_round: String,
    vortex: String,
    columnar: String,
    serde_columnar: String,
    msgpack: String,
}

impl markdown_tables::MarkdownTableRow for CodecRow {
    fn column_names() -> Vec<&'static str> {
        vec![
            "Metric", "PcoPack", "PcoPack time_round", "Vortex", "columnar", "serde_columnar", "msgpack",
        ]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.metric.clone(),
            self.pco_pack.clone(),
            self.pco_time_round.clone(),
            self.vortex.clone(),
            self.columnar.clone(),
            self.serde_columnar.clone(),
            self.msgpack.clone(),
        ]
    }
}

/// backend_snapshots: the codec table plus the `PcoPack Timeline` dedup column.
#[derive(Debug)]
struct CodecRowTimeline {
    metric: String,
    pco_pack: String,
    pco_time_round: String,
    pco_timeline: String,
    vortex: String,
    columnar: String,
    serde_columnar: String,
    msgpack: String,
}

impl markdown_tables::MarkdownTableRow for CodecRowTimeline {
    fn column_names() -> Vec<&'static str> {
        vec![
            "Metric",
            "PcoPack",
            "PcoPack time_round",
            "PcoPack Timeline",
            "Vortex",
            "columnar",
            "serde_columnar",
            "msgpack",
        ]
    }
    fn column_values(&self) -> Vec<String> {
        vec![
            self.metric.clone(),
            self.pco_pack.clone(),
            self.pco_time_round.clone(),
            self.pco_timeline.clone(),
            self.vortex.clone(),
            self.columnar.clone(),
            self.serde_columnar.clone(),
            self.msgpack.clone(),
        ]
    }
}

fn cell(m: &CodecMetrics, kind: &str, filter_idx: Option<usize>) -> String {
    let value = match kind {
        "Serialize" => m.serial_ms.map(format_ms),
        "Deserialize" => m.deserial_ms.map(format_ms),
        "Size" => m.size.map(format_bytes),
        _ => match filter_idx {
            Some(i) => m.filter_ms.get(i).copied().flatten().map(format_ms),
            None => None,
        },
    };
    value.unwrap_or_else(|| "-".to_string())
}

// ---- Captures: neutral msgpack+zstd inputs, committed via Git LFS ----

/// Inputs live in `benches/data/` (Git LFS); override with
/// `$PCO_PACK_BENCH_DATA` to point at a local re-capture instead.
fn data_dir() -> PathBuf {
    match std::env::var("PCO_PACK_BENCH_DATA") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/data"),
    }
}

/// Each capture is one msgpack array of rows behind a single zstd frame
/// (`tmp/pg_capture`'s dump format; `query_stats` trimmed to its last 14
/// days). Integrity is Git LFS's job; a missing file just points the user
/// at `git lfs pull`. Stream-decoded to avoid the multi-GB uncompressed
/// intermediate.
fn read_blob<T: serde::de::DeserializeOwned>(name: &str) -> anyhow::Result<Vec<T>> {
    let path = data_dir().join(name);
    eprintln!("decoding {name}");
    let file = std::fs::File::open(&path)
        .with_context(|| format!("{} missing: run `git lfs pull`", path.display()))?;
    let decoder = zstd::stream::read::Decoder::new(std::io::BufReader::new(file))?;
    Ok(rmp_serde::decode::from_read(decoder)?)
}

/// The deduped snapshot row's payload columns (everything the per-tick row
/// carries besides the timestamps it was observed at).
impl From<(&BackendSnapshotKey, &Timeline<0>)> for BackendSnapshotTimeline {
    fn from((k, seen_at): (&BackendSnapshotKey, &Timeline<0>)) -> Self {
        BackendSnapshotTimeline {
            server_id: k.server_id,
            granularity: k.granularity,
            collected_at: seen_at.clone(),
            database_id: k.database_id,
            backend_pid: k.backend_pid,
            backend_start: k.backend_start,
            backend_type: k.backend_type,
            backend_state: k.backend_state,
            wait_event_type: k.wait_event_type,
            wait_event: k.wait_event,
            query_start: k.query_start,
            query_fingerprint: k.query_fingerprint,
            postgres_role_id: k.postgres_role_id,
            blocked_by_pids: k.blocked_by_pids.clone(),
            application_name: k.application_name.clone(),
            client_addr: k.client_addr.clone(),
        }
    }
}
