include!("bench_common.rs");

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
fn main() -> anyhow::Result<()> {
    println!("## Postgres\n");
    let args = init_crate_filter();
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
