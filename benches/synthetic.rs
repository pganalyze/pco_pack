//! All synthetic benchmarks in one binary: types, structs, timeline,
//! chunk_size, and filters.
//!
//! Run one scenario alone with `cargo bench --bench synthetic -- types`

include!("bench_common.rs");

mod synthetic {
    pub use super::*;
    pub mod chunk_size;
    pub mod filter;
    pub mod structs;
    pub mod timeline;
    pub mod types;
}

fn main() {
    let args = init_crate_filter();
    let want = |name: &str| args.is_empty() || args.iter().any(|a| a == name);
    if want("types") {
        synthetic::types::bench();
    }
    if want("structs") {
        synthetic::structs::bench();
    }
    if want("timeline") {
        synthetic::timeline::bench();
    }
    if want("chunk_size") {
        synthetic::chunk_size::bench();
    }
    if want("filter") {
        synthetic::filter::bench();
    }
}
