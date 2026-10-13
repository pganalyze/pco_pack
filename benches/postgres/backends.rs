//! backend_snapshots: 13.3M per-tick rows reconstructed from every tick of
//! the capture with the production backfill's keep rule. Mirrors the source
//! table exactly (field order/names/types) so the chunk layout matches the
//! backfill schema: index [server_id, granularity], timestamp collected_at.
//! Full-fidelity baseline; the time_round variant persists with
//! `time_round = 1 s`, since tick jitter carries no signal.
//!
//! Adds the `PcoPack Timeline` column, the shape from `benches/timeline.rs`:
//! snapshots whose non-timestamp payload is unchanged between ticks collapse
//! into one row whose `Timeline<0>` holds every tick observed at, a lossless
//! fold at exact us. Most backends are idle, so the fold halves both size
//! and encode time.

use chrono::{DateTime, Utc};
use pco_pack::{PcoPack, Timeline};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

use super::common::{self, CodecRowTimeline, cell};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, PcoPack)]
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

/// The TimeRound model (see `query_stats::QueryStatTimeRound`): this is the
/// `PcoPack time_round` column. Rounding the us timestamps shrinks the
/// timestamp columns.
#[derive(Clone, Debug, Default, PartialEq, PcoPack)]
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

/// The deduped shape (see `benches/timeline.rs`'s `TimelineRecord`): ticks
/// folded into an exact-us `Timeline`; bucketing would lie about the
/// collection time. Same index layout as the source schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, PcoPack)]
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

/// Filter keys: last-1h window on collected_at; active states (1 = active,
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

    /// Same filter over the stored `time_round` values; only the window
    /// filter touches timestamps.
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
    /// tick falls in it (inclusive).
    fn matches_timeline(&self, r: &BackendSnapshotTimeline) -> bool {
        match self {
            Bk::Time(a, b) => r.collected_at.overlaps(*a, *b),
            Bk::Active => matches!(r.backend_state, 1 | 2),
            Bk::Waits => r.wait_event >= 1,
        }
    }
}

pub fn bench() -> anyhow::Result<()> {
    let name = "backend_snapshots.msgpack.zst";
    let Some(rows) = common::read_blob(name, &common::data_dir(), &common::lfs_warning(name))? else {
        return Ok(());
    };
    bench_backends(&rows);
    Ok(())
}

fn bench_backends(rows: &[BackendSnapshot]) {
    let n = rows.len();
    let last_us = rows.iter().map(|r| r.collected_at.timestamp_micros()).max().unwrap();

    let keys = [Bk::Time(last_us - 3_600_000_000, last_us), Bk::Active, Bk::Waits];
    let kf: Vec<(Bk, usize)> = keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches(r)).count())).collect();
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

    // Counts fold over the rounded collected_at values the write stores.
    let time_round_rows: Vec<BackendSnapshotTimeRound> = rows.iter().map(BackendSnapshotTimeRound::from).collect();
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
            Bk::Time(a, b) => and(gt_eq(col("collected_at"), lit(*a)), lt_eq(col("collected_at"), lit(*b))),
            Bk::Active => or(eq(col("backend_state"), lit(1i8)), eq(col("backend_state"), lit(2i8))),
            Bk::Waits => gt_eq(col("wait_event"), lit(1i16)),
        }
    });
    drop(vx_rows);

    let sc = common::bench_serde_columnar(
        n,
        || backends_sc::BackendsScStore { data: rows.iter().map(backends_sc::BackendsScRow::from).collect() },
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

    // Timeline filters use range-overlap semantics and verify counts over
    // the deduped rows themselves: the same queries cover different
    // fractions of the two row sets.
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
    let kf_tl: Vec<(Bk, usize)> =
        keys.iter().map(|k| (*k, tl_rows.iter().filter(|r| k.matches_timeline(r)).count())).collect();
    let tl_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_tl.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let pco_timeline = common::bench_pco_pack(&tl_rows, &tl_filters, |back| {
        assert_eq!(back.len(), nd, "backend_snapshots (Timeline) row count")
    });

    let metrics = ["Size", "Serialize", "Deserialize"];
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

/// columnar/vortex mirror: timestamps to i64 us (exact; pg timestamps are
/// us), strings to String (columnar has no chrono/SmolStr impls). Same
/// mirror feeds Vortex via serde_arrow: numeric range filters, no unsound
/// RFC3339 columns.
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

// ---- serde_columnar mirror (strategies follow the taxi/osm convention) ----

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

impl common::ScStore for backends_sc::BackendsScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}
