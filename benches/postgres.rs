//! Benchmarks over real Postgres monitoring payloads, the only private datasets here.
//! The neutral msgpack+zstd captures are the benchmark's input data: backends and
//! query_stats are committed here via Git LFS, while query_explains lives in the
//! private pco_pack_bench_data repo beside this checkout, because explain plans can
//! possibly contain sensitive information and only the pganalyze team can run that
//! section. A capture that has not been fetched prints a warning and its section is
//! skipped, so the suite never fails on missing data. query_stats covers the
//! capture's last 14 days (full: 114.3M rows, 748 MB; kept: 31.0M rows, 164 MB);
//! query_explains is the full query_explains_7d window (237,651 analyzed queries,
//! 110 MB).
//!
//!   cargo bench --bench postgres
//!
//! One codec-comparison table per data source, in datasets-bench format: one
//! module per source (`backends`, `query_stats`, `query_explains`); `common`
//! holds the capture readers and table printers and has no `bench()` of its
//! own. Every codec re-encodes the same loaded rows (the datasets-bench
//! convention): PcoPack full-fidelity plus one lossy variant (PcoPack
//! time_round for backends and stats: timestamps rounded to whole seconds,
//! `float_round = 2` on the query_stats f64 aggregates; PcoPack (lossy) for
//! explains: `float_round = 2` on the EXPLAIN millisecond floats and
//! `time_round = 1 s`), Vortex, columnar, serde_columnar, and msgpack+zstd.
//! `backends` adds the `PcoPack Timeline` column, the shape from
//! `benches/timeline.rs`: snapshots whose payload is unchanged between ticks
//! collapse to one row whose `Timeline<0>` holds every tick they were
//! observed at.
//!
//! PcoPack roundtrip verifies are counts-only, the convention in this repo
//! for indexed structs (see struct.rs): an indexed `write` groups rows into
//! chunks by index key, so `read` returns them index-grouped, not in stored
//! order, and full-order row asserts are wrong here. Filter rows stay
//! asserted against counts computed from each codec's own stored precision.

include!("bench_common.rs");

// Shared codec runners from the datasets bench, compiled against this
// binary's root (same `use super::*` contract as `datasets/common.rs`).
#[allow(dead_code)]
#[path = "datasets/common.rs"]
pub mod ds_common;

mod postgres {
    pub mod backends;
    pub mod common;
    pub mod query_explains;
    pub mod query_stats;
}

fn main() -> anyhow::Result<()> {
    println!("## Postgres\n");
    postgres::backends::bench()?;
    postgres::query_explains::bench()?;
    postgres::query_stats::bench()?;
    Ok(())
}
