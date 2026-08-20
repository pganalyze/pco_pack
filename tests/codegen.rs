use std::collections::HashMap;

// Tests that the proc-macro generated code compiles and works correctly.
// Compile errors point to the exact generated code that's causing an issue.
// The public API is exercised so Rust's dead code analysis is correct.
//
// Run `cargo test -p pco_pack_derive` to rengerate these snapshots
include!("../pco_pack_derive/src/test/enum.expanded.rs");
include!("../pco_pack_derive/src/test/float_round_map.expanded.rs");
include!("../pco_pack_derive/src/test/index.expanded.rs");
include!("../pco_pack_derive/src/test/nested.expanded.rs");
include!("../pco_pack_derive/src/test/query_stats.expanded.rs");
include!("../pco_pack_derive/src/test/string_index.expanded.rs");
include!("../pco_pack_derive/src/test/timeline.expanded.rs");
include!("../pco_pack_derive/src/test/timestamp.expanded.rs");

#[test]
fn codegen() {
    // The input files mark their types as `pub`, so simply including them
    // ensures the code doesn't have errors or warnings
}
