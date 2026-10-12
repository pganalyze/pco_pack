//! query_stats: the collector's query statistics, 31.0M rows across two
//! granularities (1min + 1hour). Mirrors the source table exactly (field
//! order/names/types) so the chunk layout matches how the table is stored:
//! index [database_id, granularity], timestamp collected_at. Full-fidelity
//! baseline; `QueryStatTimeRound` is the rounded persisted shape.

use chrono::{DateTime, Utc};
use pco_pack::PcoPack;
use serde::{Deserialize, Serialize};

use super::common::{self, CodecRow, cell};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, PcoPack)]
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

/// The TimeRound model: pco_store persists this table rounded,
/// `time_round = 1 s` on the timestamps and `float_round = 2` on the
/// aggregate f64s. This is the `PcoPack time_round` column.
#[derive(Clone, Debug, PartialEq, PcoPack)]
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

/// Filter keys, shared by every codec runner: last-1h window on
/// collected_at; toplevel with calls >= 10; the first database in the
/// capture.
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

    /// Same filter over the stored `time_round` values; only the window
    /// filter touches timestamps (`float_round` rounds no filter column).
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

pub fn bench() -> anyhow::Result<()> {
    let name = "query_stats.msgpack.zst";
    let Some(all): Option<Vec<QueryStat>> = common::read_blob(name, &common::data_dir(), &common::lfs_warning(name))?
    else {
        return Ok(());
    };
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

fn bench_stats(name: &str, rows: Vec<QueryStat>) {
    let n = rows.len();
    let last_us = rows.iter().map(|r| r.collected_at.timestamp_micros()).max().unwrap();
    let db0 = rows[0].database_id;

    let keys = [Sk::Time(last_us - 3_600_000_000, last_us), Sk::Loud, Sk::Db(db0)];
    let kf: Vec<(Sk, usize)> = keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches(r)).count())).collect();
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

    // Counts fold collected_at through the same whole-second rounding the
    // write stores.
    let time_round_rows: Vec<QueryStatTimeRound> = rows.iter().map(QueryStatTimeRound::from).collect();
    let kf_time_round: Vec<(Sk, usize)> =
        keys.iter().map(|k| (*k, rows.iter().filter(|r| k.matches_time_round(r)).count())).collect();
    let time_round_filters: Vec<(&serde_json::Value, usize)> =
        queries.iter().zip(kf_time_round.iter()).map(|(q, (_, e))| (q, *e)).collect();
    let pco_time_round = common::bench_pco_pack(time_round_rows, &time_round_filters, |back| {
        assert_eq!(back.len(), n, "{name} (TimeRound) row count")
    });

    let col_rows: Vec<QueryStatCol> = rows.iter().map(QueryStatCol::from).collect();
    let col = common::bench_columnar(
        n,
        move || col_rows,
        &kf,
        |decoded: &columnar::BorrowedOf<QueryStatCol>, k: &Sk| {
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
        },
    );

    let vx_rows: Vec<QueryStatCol> = rows.iter().map(QueryStatCol::from).collect();
    let vx = common::bench_vortex_ds(&vx_rows, &kf, |k: &Sk| {
        use vortex::array::expr::{and, col, eq, gt_eq, lit, lt_eq};
        match k {
            Sk::Time(a, b) => and(gt_eq(col("collected_at"), lit(*a)), lt_eq(col("collected_at"), lit(*b))),
            Sk::Loud => and(eq(col("toplevel"), lit(true)), gt_eq(col("calls"), lit(10i64))),
            Sk::Db(id) => eq(col("database_id"), lit(*id)),
        }
    });
    drop(vx_rows);

    let sc = common::bench_serde_columnar(
        n,
        || stats_sc::QueryStatsScStore { data: rows.iter().map(stats_sc::QueryStatsScRow::from).collect() },
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

    let pco = common::bench_pco_pack(rows, &pco_filters, move |back| assert_eq!(back.len(), n, "{name} row count"));

    let metrics = ["Size", "Serialize", "Deserialize"];
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

/// columnar/vortex mirror: timestamps to i64 us (exact; pg timestamps are
/// us), strings to String (columnar has no chrono/SmolStr impls). Same
/// mirror feeds Vortex via serde_arrow: numeric range filters, no unsound
/// RFC3339 columns.
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

// ---- serde_columnar mirror (strategies follow the taxi/osm convention) ----

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

impl common::ScStore for stats_sc::QueryStatsScStore {
    fn row_count(&self) -> usize {
        self.data.len()
    }
}
