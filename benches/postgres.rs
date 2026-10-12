//! Benchmarks over real Postgres monitoring payloads, the only private
//! datasets here. Input data: neutral msgpack+zstd captures. backends and
//! query_stats are committed here via Git LFS; query_explains lives in the
//! private pco_pack_bench_data repo beside this checkout, because explain
//! plans can possibly contain sensitive information and only the pganalyze
//! team can run that section. A capture that has not been fetched prints a
//! warning and its section is skipped. query_stats covers the capture's last
//! 14 days (full: 114.3M rows, 748 MB; kept: 31.0M rows, 164 MB);
//! query_explains is the full query_explains_7d window (237,651 analyzed
//! queries, 110 MB).
//!
//!   cargo bench --bench postgres
//!
//! One codec-comparison table per data source, in datasets-bench format: one
//! module per source (`backends`, `query_stats`, `query_explains`); `common`
//! holds the capture readers and table printers and has no `bench()` of its
//! own. Every codec re-encodes the same loaded rows: PcoPack full-fidelity,
//! one lossy PcoPack variant per source, Vortex, columnar, serde_columnar,
//! and msgpack+zstd. `backends` adds the `PcoPack Timeline` column from
//! `benches/timeline.rs`.
//!
//! PcoPack roundtrip verifies are counts-only, the repo convention for
//! indexed structs (see struct.rs): indexed `write` groups chunks by index
//! key, so `read` returns rows index-grouped, not in stored order. Filter
//! rows are asserted against counts computed from each codec's own stored
//! precision.

include!("bench_common.rs");

// Shared codec runners from the datasets bench, compiled against this
// binary's root (same `use super::*` contract as `datasets/common.rs`).
#[allow(dead_code)]
#[path = "datasets/common.rs"]
pub mod ds_common;

mod postgres {
    pub mod arena;
    pub mod arena_lossy;
    pub mod backends;
    pub mod common;
    pub mod query_explains;
    pub mod query_stats;
}

/// Run one source alone with `cargo bench --bench postgres -- query_explains`
/// (no args runs all three).
fn main() -> anyhow::Result<()> {
    println!("## Postgres\n");
    let args: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let want = |name: &str| args.is_empty() || args.iter().any(|a| a == name);
    if want("backends") {
        postgres::backends::bench()?;
    }
    if want("query_explains") {
        postgres::query_explains::bench()?;
    }
    if want("query_stats") {
        postgres::query_stats::bench()?;
    }
    Ok(())
}
