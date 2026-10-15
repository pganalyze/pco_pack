include!("bench_common.rs");

mod datasets {
    pub use super::*;
    pub mod air_quality;
    pub mod common;
    pub mod openalex;
    pub mod osm;
    pub mod taxi;
}

/// Run one source alone with `cargo bench --bench datasets -- taxi`
fn main() {
    println!("## Datasets\n");
    let args: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let want = |name: &str| args.is_empty() || args.iter().any(|a| a == name);
    if want("air_quality") {
        datasets::air_quality::bench();
    }
    if want("taxi") {
        datasets::taxi::bench();
    }
    if want("osm") {
        datasets::osm::bench();
    }
    if want("openalex") {
        datasets::openalex::bench();
    }
}
