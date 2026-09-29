//! Datasets bench entry point. Each dataset module exposes `bench()`;
//! `common` holds shared machinery and has no `bench()` of its own.

include!("bench_common.rs");

mod datasets {
    // Glob re-export: dataset modules reach the bench_common helpers through
    // their own `use super::*`.
    pub use super::*;

    pub mod air_quality;
    pub mod common;
    pub mod openalex;
    pub mod osm;
    pub mod taxi;
}

fn main() {
    println!("## Datasets\n");
    datasets::air_quality::bench();
    datasets::taxi::bench();
    datasets::osm::bench();
    datasets::openalex::bench();
}
