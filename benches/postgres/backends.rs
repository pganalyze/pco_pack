//! backend_snapshots: 13.3M per-tick rows reconstructed from every tick of
//! the capture with the production backfill's keep rule. Mirrors the source
//! table exactly (field order/names/types) so the chunk layout matches the
//! backfill schema: index [server_id, granularity], timestamp collected_at.
//! Full-fidelity baseline; the time_round variant persists with
//! `time_round = 1 s`, since tick jitter carries no signal.
//!
//! Adds two `PcoPack Timeline` columns, the shape from `benches/timeline.rs`:
//! snapshots whose non-timestamp payload is unchanged between ticks collapse
//! into one row whose `Timeline<0>` holds every tick observed at, a lossless
//! fold at exact us. The `Timeline 10s` column folds the same runs at the
//! capture cadence: ticks land on whole seconds up to 10 s apart, so
//! bucket-aligned ranges collapse each run to a single range, trading exact
//! tick instants for bucket windows. Most backends are idle, so the folds
//! halve size and encode time.

use chrono::{DateTime, Utc};
use pco_pack::{PcoPack, Timeline};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

use super::common::{self, CodecRowOther, CodecRowTimeline, cell};

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
/// folded into an exact-us `Timeline`, preserving every collection instant.
/// Same index layout as the source schema.
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

/// The same fold at the 10 s capture cadence: each tick is floored into its
/// bucket, so each row's run collapses to one bucket-aligned range. The
/// range start floors back to the bucket and the end extends to the bucket
/// boundary after the last tick; the fold reports bucket windows, not exact
/// tick instants.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, PcoPack)]
#[pco_pack(index = [server_id, granularity], timestamp = collected_at)]
pub struct BackendSnapshotTimeline10s {
    pub server_id: i64,
    pub granularity: i32,
    pub collected_at: Timeline<10_000_000>,
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

impl From<(&BackendSnapshotKey, &Timeline<10_000_000>)> for BackendSnapshotTimeline10s {
    fn from((k, seen_at): (&BackendSnapshotKey, &Timeline<10_000_000>)) -> Self {
        BackendSnapshotTimeline10s {
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
/// 2 = idle in transaction); any backend with a wait event. Text eq is N/A
/// here: `application_name` and `client_addr` ride empty in this capture,
/// so the text scenarios live in `openalex`/`taxi`/`query_explains`.
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

    /// Same filter over a 10 s-bucketed timeline row: the bucket windows can
    /// reach a tick-free second at the edges, so membership is defined by
    /// bucket overlap with the window.
    fn matches_timeline10s(&self, r: &BackendSnapshotTimeline10s) -> bool {
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
    bench_backends(rows);
    Ok(())
}

fn bench_backends(rows: Vec<BackendSnapshot>) {
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

    // Counts fold over the rounded collected_at values the write stores.
    let time_round_rows: Vec<BackendSnapshotTimeRound> = rows.iter().map(BackendSnapshotTimeRound::from).collect();
    let kf_time_round: Vec<(Bk, usize)> =
        keys.iter().map(|k| (k.clone(), rows.iter().filter(|r| k.matches_time_round(r)).count())).collect();
    let time_round_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_time_round.iter()).map(|(q, (_, e))| (q, *e)).collect();

    // Projection fold ground truth: query_fingerprint => survivor count over
    // the last-1h window, folded before `rows` moves into the encoders.
    let window = &keys[0];
    let proj_truth = fold_fp_counts(rows.iter().filter(|r| window.matches(r)).filter_map(|r| r.query_fingerprint));
    let proj_truth_time_round =
        fold_fp_counts(rows.iter().filter(|r| window.matches_time_round(r)).filter_map(|r| r.query_fingerprint));
    let proj_q = queries[0].clone();

    let (mut pco_time_round, tr_bytes) =
        common::bench_pco_pack_full(&time_round_rows, &time_round_filters, |_back, _off| {}, || {});
    drop(time_round_rows);
    if crate::enabled("pco_pack") {
        let (ms_tr, fold_tr) = common::bench_pco_pack_projected::<BackendSnapshotTimeRound, _>(
            &tr_bytes,
            &proj_q,
            &["query_fingerprint", "wait_event"],
            kf_time_round[0].1,
            |rows| fold_fp_counts(rows.iter().filter_map(|r| r.query_fingerprint)),
        );
        assert_eq!(fold_tr, proj_truth_time_round, "backend_snapshots (TimeRound) projected fold");
        pco_time_round.project_ms = Some(ms_tr);
    }

    let col_rows: Vec<BackendSnapshotCol> = rows.iter().map(BackendSnapshotCol::from).collect();
    let (mut col, col_buf) = common::bench_columnar(
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
    if crate::enabled("columnar") {
        col.project_ms = Some(common::reduce_columnar::<BackendSnapshotCol, Bk, i64, HashMap<i64, u32>, _, _>(
            &col_buf,
            &keys[0],
            |decoded: &columnar::BorrowedOf<'_, BackendSnapshotCol>, k: &Bk| {
                (0..columnar::Len::len(decoded))
                    .filter_map(|i| {
                        let r = columnar::Index::get(decoded, i);
                        let hit = match k {
                            Bk::Time(a, b) => {
                                let us = *r.collected_at;
                                *a <= us && us <= *b
                            }
                            Bk::Active => matches!(*r.backend_state, 1 | 2),
                            Bk::Waits => *r.wait_event >= 1,
                        };
                        if hit { r.query_fingerprint.copied() } else { None }
                    })
                    .collect()
            },
            |pairs, _k| fold_fp_counts(pairs.into_iter()),
            &proj_truth,
        ));
    }
    drop(col_buf);

    let vx_rows: Vec<BackendSnapshotCol> = rows.iter().map(BackendSnapshotCol::from).collect();
    let (mut vx, vx_buf) = common::bench_vortex_ds(&vx_rows, &kf, |k: &Bk| {
        use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq, or};
        match k {
            Bk::Time(a, b) => and(gt_eq(col("collected_at"), lit(*a)), lt_eq(col("collected_at"), lit(*b))),
            Bk::Active => or(eq(col("backend_state"), lit(1i8)), eq(col("backend_state"), lit(2i8))),
            Bk::Waits => gt_eq(col("wait_event"), lit(1i16)),
        }
    });
    if crate::enabled("vortex") {
        vx.project_ms = common::reduce_vortex_ds::<(Option<i64>,), Bk, _, _, _>(
            &vx_buf,
            &keys[0],
            |k: &Bk| {
                use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq, or};
                match k {
                    Bk::Time(a, b) => and(gt_eq(col("collected_at"), lit(*a)), lt_eq(col("collected_at"), lit(*b))),
                    Bk::Active => or(eq(col("backend_state"), lit(1i8)), eq(col("backend_state"), lit(2i8))),
                    Bk::Waits => gt_eq(col("wait_event"), lit(1i16)),
                }
            },
            &["query_fingerprint"],
            |rows: Vec<(Option<i64>,)>, _: &Bk| fold_fp_counts(rows.into_iter().filter_map(|(fp,)| fp)),
            &proj_truth,
        );
    }
    drop(vx_rows);
    drop(vx_buf);

    let (mut sc, sc_buf) = common::bench_serde_columnar(
        n,
        &rows,
        |chunk| backends_sc::BackendsScStore { data: chunk.iter().map(backends_sc::BackendsScRow::from).collect() },
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
    if crate::enabled("serde_columnar") {
        sc.project_ms = Some(common::reduce_serde_columnar::<backends_sc::BackendsScStore, Bk, i64, _, _, _>(
            &sc_buf,
            &keys[0],
            |s: &backends_sc::BackendsScStore, k: &Bk| {
                s.data
                    .iter()
                    .filter(|r| match k {
                        Bk::Time(a, b) => *a <= r.collected_at && r.collected_at <= *b,
                        Bk::Active => matches!(r.backend_state, 1 | 2),
                        Bk::Waits => r.wait_event >= 1,
                    })
                    .filter_map(|r| r.query_fingerprint)
                    .collect()
            },
            |pairs, _k| fold_fp_counts(pairs.into_iter()),
            &proj_truth,
        ));
    }
    drop(sc_buf);

    let (mut mp, mp_buf) = common::bench_msgpack(&rows, &kf, |r: &BackendSnapshot, k: &Bk| k.matches(r));
    if crate::enabled("msgpack") {
        mp.project_ms = Some(common::reduce_msgpack::<BackendSnapshot, Bk, i64, _, _, _>(
            &mp_buf,
            &keys[0],
            |rows: &[BackendSnapshot], k: &Bk| {
                rows.iter().filter(|r| k.matches(r)).filter_map(|r| r.query_fingerprint).collect()
            },
            |pairs, _k| fold_fp_counts(pairs.into_iter()),
            &proj_truth,
        ));
    }
    drop(mp_buf);

    // Timeline filters use range-overlap semantics and verify counts over
    // the deduped rows themselves: the same queries cover different
    // fractions of the two row sets.
    let mut groups: HashMap<BackendSnapshotKey, Timeline<0>> = HashMap::new();
    for r in &rows {
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
        keys.iter().map(|k| (k.clone(), tl_rows.iter().filter(|r| k.matches_timeline(r)).count())).collect();
    // The fold truth over folded rows: each surviving row contributes one
    // count per fingerprint, standing in for its whole collapsed tick run.
    // The Reduce label's percent stays the per-tick selectivity; this fold
    // answers "how many folded rows per fingerprint", not ticks.
    let tl_truth =
        fold_fp_counts(tl_rows.iter().filter(|r| keys[0].matches_timeline(r)).filter_map(|r| r.query_fingerprint));
    let tl_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_tl.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let (mut pco_timeline, tl_bytes) = common::bench_pco_pack_full(&tl_rows, &tl_filters, |_back, _off| {}, || {});
    drop(tl_rows);
    if crate::enabled("pco_pack") {
        let (ms_tl, fold_tl) = common::bench_pco_pack_projected::<BackendSnapshotTimeline, _>(
            &tl_bytes,
            &proj_q,
            &["query_fingerprint"],
            kf_tl[0].1,
            |rows| fold_fp_counts(rows.iter().filter_map(|r| r.query_fingerprint)),
        );
        assert_eq!(fold_tl, tl_truth, "backend_snapshots (Timeline) projected fold over folded rows");
        pco_timeline.project_ms = Some(ms_tl);
        drop(tl_bytes);
    }

    // The 10 s-bucketed fold over the same grouping keys (so the same nd
    // rows): every occupied bucket tiles its run, collapsing each row's
    // ~3.5 singleton ranges into one aligned range. Counts verify over the
    // bucketed rows themselves, where edge buckets can reach tick-free
    // seconds.
    let mut groups10: HashMap<BackendSnapshotKey, Timeline<10_000_000>> = HashMap::new();
    for r in &rows {
        let ts = r.collected_at.timestamp_micros();
        groups10.entry(BackendSnapshotKey::from(r)).or_default().add(ts, ts);
    }
    let mut tl_rows10: Vec<BackendSnapshotTimeline10s> =
        groups10.iter().map(BackendSnapshotTimeline10s::from).collect();
    drop(groups10);
    tl_rows10.sort_by_key(|r| (r.server_id, r.backend_pid, r.collected_at.start().unwrap_or(0)));
    assert_eq!(tl_rows10.len(), nd, "backend_snapshots (Timeline 10s) row count");
    let kf_tl10: Vec<(Bk, usize)> =
        keys.iter().map(|k| (k.clone(), tl_rows10.iter().filter(|r| k.matches_timeline10s(r)).count())).collect();
    let tl_truth10 =
        fold_fp_counts(tl_rows10.iter().filter(|r| keys[0].matches_timeline10s(r)).filter_map(|r| r.query_fingerprint));
    let tl10_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_tl10.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let (mut pco_timeline10, tl10_bytes) =
        common::bench_pco_pack_full(&tl_rows10, &tl10_filters, |_back, _off| {}, || {});
    drop(tl_rows10);
    if crate::enabled("pco_pack") {
        let (ms_tl10, fold_tl10) = common::bench_pco_pack_projected::<BackendSnapshotTimeline10s, _>(
            &tl10_bytes,
            &proj_q,
            &["query_fingerprint"],
            kf_tl10[0].1,
            |rows| fold_fp_counts(rows.iter().filter_map(|r| r.query_fingerprint)),
        );
        assert_eq!(fold_tl10, tl_truth10, "backend_snapshots (Timeline 10s) projected fold over folded rows");
        pco_timeline10.project_ms = Some(ms_tl10);
        drop(tl10_bytes);
    }

    let (mut pco, pco_bytes) = common::bench_pco_pack_full(&rows, &pco_filters, |_back, _off| {}, || {});
    drop(rows);
    if crate::enabled("pco_pack") {
        let (ms_pco, fold) = common::bench_pco_pack_projected::<BackendSnapshot, _>(
            &pco_bytes,
            &proj_q,
            &["query_fingerprint", "wait_event"],
            kf[0].1,
            |rows| fold_fp_counts(rows.iter().filter_map(|r| r.query_fingerprint)),
        );
        assert_eq!(fold, proj_truth, "backend_snapshots projected fold");
        pco.project_ms = Some(ms_pco);
        drop(pco_bytes);
    }

    let metrics = ["Size", "Serialize", "Deserialize"];
    let mut pco_out: Vec<CodecRowTimeline> = metrics
        .iter()
        .map(|m| CodecRowTimeline {
            metric: m.to_string(),
            pco_pack: cell(&pco, m, None),
            pco_time_round: cell(&pco_time_round, m, None),
            pco_timeline: cell(&pco_timeline, m, None),
            pco_timeline_10s: cell(&pco_timeline10, m, None),
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
        pco_out.push(CodecRowTimeline {
            metric: label.to_string(),
            pco_pack: cell(&pco, "Filter", Some(i)),
            pco_time_round: cell(&pco_time_round, "Filter", Some(i)),
            pco_timeline: cell(&pco_timeline, "Filter", Some(i)),
            pco_timeline_10s: cell(&pco_timeline10, "Filter", Some(i)),
        });
        other_out.push(CodecRowOther {
            metric: label.to_string(),
            vortex: cell(&vx, "Filter", Some(i)),
            columnar: cell(&col, "Filter", Some(i)),
            serde_columnar: cell(&sc, "Filter", Some(i)),
            msgpack: cell(&mp, "Filter", Some(i)),
        });
    }
    pco_out.push(CodecRowTimeline {
        metric: common::reduce_label(kf[0].1, n),
        pco_pack: cell(&pco, "Project", None),
        pco_time_round: cell(&pco_time_round, "Project", None),
        pco_timeline: cell(&pco_timeline, "Project", None),
        pco_timeline_10s: cell(&pco_timeline10, "Project", None),
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
    println!(
        "\n({nd} of {n} ticks survive the Timeline fold; both Timeline reduce cells fold folded rows, one count per collapsed tick run, while the label's percent stays the per-tick selectivity; the 10 s column floors range starts to bucket boundaries and extends ends to the bucket after the last tick)\n"
    );
}

/// query_fingerprint => survivor count, over the fingerprints present in the
/// projected survivors (`Option<i64>` columns skip nulls).
fn fold_fp_counts(fingerprints: impl Iterator<Item = i64>) -> HashMap<i64, u32> {
    let mut counts: HashMap<i64, u32> = HashMap::new();
    for fp in fingerprints {
        *counts.entry(fp).or_default() += 1;
    }
    counts
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
