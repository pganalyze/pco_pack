//! Schema-table guard for the postgres bench's plan arena (the same
//! value-exactness contract the full capture verifies inside the bench):
//! every corpus document rebuilds numerically equal to the tree it was
//! flattened from, across key presence, dictionary spellings, and the
//! overflow verbatim blobs. Integer text in a float column rebuilds as a
//! float ("0" becomes "0.0"); integer columns keep identical integer text.
//!
//! The arena lives in the bench binary (harness `false` there), so its
//! unit tests ride this harness via `#[path]`.

#![recursion_limit = "512"]

#[path = "../benches/postgres/arena.rs"]
mod arena;
#[path = "../benches/postgres/arena_lossy.rs"]
mod arena_lossy;

use arena::{NodeType, PlanArena};
use pco_pack::PcoPack;
use serde_json::{Value, json};

fn roundtrip(doc: &Value) -> Value {
    let arena = PlanArena::flatten(doc);
    let back = arena.rebuild();
    assert_eq!(back, *doc, "rebuild must equal the captured document");
    back
}

/// Envelope: Plan / Triggers / Query Identifier, plus the PG 16 parallel
/// shape: `Workers` flatten into typed arena rows and rebuild structurally.
#[test]
fn pg16_parallel_hashjoin() {
    let doc = json!([
        {
            "Plan": {
                "Node Type": "Gather",
                "Parallel Aware": false,
                "Async Capable": false,
                "Startup Cost": 100.0,
                "Total Cost": 980.25,
                "Plan Rows": 1000,
                "Plan Width": 44,
                "Actual Total Time": 12.34,
                "Actual Startup Time": 0.01,
                "Actual Rows": 980,
                "Actual Loops": 1,
                "Workers Planned": 2,
                "Workers Launched": 2,
                "Shared Hit Blocks": 10,
                "Shared Read Blocks": 2,
                "Shared Dirtied Blocks": 0,
                "Shared Written Blocks": 0,
                "Local Hit Blocks": 0,
                "Local Read Blocks": 0,
                "Local Dirtied Blocks": 0,
                "Local Written Blocks": 0,
                "Temp Read Blocks": 0,
                "Temp Written Blocks": 0,
                "Plans": [
                    {
                        "Node Type": "Hash Join",
                        "Parent Relationship": "Outer",
                        "Parallel Aware": true,
                        "Async Capable": false,
                        "Join Type": "Inner",
                        "Inner Unique": true,
                        "Hash Cond": "(a.id = b.a_id)",
                        "Startup Cost": 50.5,
                        "Total Cost": 700.75,
                        "Plan Rows": 500,
                        "Plan Width": 60,
                        "Actual Total Time": 9.87,
                        "Actual Startup Time": 4.0,
                        "Actual Rows": 480,
                        "Actual Loops": 2,
                        "Output": ["a.id", "b.payload"],
                        "Plans": [
                            {
                                "Node Type": "Seq Scan",
                                "Parent Relationship": "Outer",
                                "Parallel Aware": true,
                                "Async Capable": false,
                                "Relation Name": "a",
                                "Alias": "a",
                                "Schema": "public",
                                "Filter": "(a.flag = ANY ('{x,y}'::text[]))",
                                "Rows Removed by Filter": 70,
                                "Startup Cost": 0.0,
                                "Total Cost": 100.0,
                                "Plan Rows": 200,
                                "Plan Width": 34,
                                "Actual Total Time": 1.5,
                                "Actual Startup Time": 0.0,
                                "Actual Rows": 180,
                                "Actual Loops": 2
                            },
                            {
                                "Node Type": "Hash",
                                "Parent Relationship": "Inner",
                                "Parallel Aware": false,
                                "Async Capable": false,
                                "Startup Cost": 40.0,
                                "Total Cost": 40.0,
                                "Plan Rows": 300,
                                "Plan Width": 26,
                                "Actual Total Time": 2.5,
                                "Actual Startup Time": 2.5,
                                "Actual Rows": 290,
                                "Actual Loops": 2,
                                "Plans": [
                                    {
                                        "Node Type": "Index Scan",
                                        "Parent Relationship": "Outer",
                                        "Parallel Aware": false,
                                        "Async Capable": false,
                                        "Scan Direction": "Forward",
                                        "Index Name": "b_pkey",
                                        "Relation Name": "b",
                                        "Alias": "b",
                                        "Schema": "public",
                                        "Startup Cost": 0.28,
                                        "Total Cost": 39.5,
                                        "Plan Rows": 300,
                                        "Plan Width": 26,
                                        "Actual Total Time": 2.1,
                                        "Actual Startup Time": 0.02,
                                        "Actual Rows": 290,
                                        "Actual Loops": 2
                                    }
                                ]
                            }
                        ]
                    }
                ]
            },
            "Triggers": [],
            "Query Identifier": -6277190607577788346i64,
            "Workers": [
                {
                    "Worker Number": 0,
                    "Startup Cost": 50.5,
                    "Total Cost": 700.75,
                    "Plan Rows": 500,
                    "Plan Width": 60,
                    "Actual Total Time": 10.1,
                    "Actual Startup Time": 4.1,
                    "Actual Rows": 240,
                    "Actual Loops": 1,
                    "Shared Hit Blocks": 40,
                    "Shared Read Blocks": 1,
                    "Shared Dirtied Blocks": 0,
                    "Shared Written Blocks": 0,
                    "Local Hit Blocks": 0,
                    "Local Read Blocks": 0,
                    "Local Dirtied Blocks": 0,
                    "Local Written Blocks": 0,
                    "I/O Read Time": 0.0,
                    "I/O Write Time": 0.0,
                    "Sort Method": "quicksort",
                    "Sort Space Type": "Memory",
                    "Sort Space Used": 1024
                },
                {
                    "Worker Number": 1,
                    "Actual Total Time": 9.9,
                    "Actual Rows": 240,
                    "Actual Loops": 1
                }
            ]
        }
    ]);
    let back = roundtrip(&doc);
    // Workers came back through their own arena rows, slots in order.
    assert_eq!(back[0]["Workers"], doc[0]["Workers"]);
}

/// PG 12 Incremental Sort: sort vocabulary, empty-array distinction
/// (`Output: []` must survive as `[]`, not vanish as absent), and integer
/// numbers kept integral.
#[test]
fn pg12_incremental_sort_presence_rules() {
    let doc = json!([
        {
            "Plan": {
                "Node Type": "Incremental Sort",
                "Startup Cost": 0.28,
                "Total Cost": 100.0,
                "Plan Rows": 200,
                "Plan Width": 34,
                "Sort Key": ["a.id", "b.ts DESC"],
                "Presorted Key": ["a.id"],
                "Sort Method": "top-N heapsort",
                "Sort Space Type": "Memory",
                "Sort Space Used": 25,
                "Output": [],
                "Startup": null,
                "Full-sort Groups": {
                    "Group Count": 2,
                    "Sort Methods Used": ["quicksort"],
                    "Sort Space Memory": {
                        "Average Sort Space Used": 98,
                        "Peak Sort Space Used": 120
                    }
                }
            },
            "Query Identifier": 12345i64
        }
    ]);
    let back = roundtrip(&doc);
    assert_eq!(back[0]["Plan"]["Output"], json!([]));
    assert_eq!(back[0]["Plan"]["Full-sort Groups"], doc[0]["Plan"]["Full-sort Groups"]);
}

/// PG 17 Memoize cache vocabulary: the new dictionary and numeric columns.
#[test]
fn pg17_memoize_cache() {
    let doc = json!([
        {
            "Plan": {
                "Node Type": "Memoize",
                "Parent Relationship": "Outer",
                "Cache Key": "(t.id)",
                "Cache Mode": "binary",
                "Cache Hits": 73147,
                "Cache Misses": 390,
                "Cache Evictions": 0,
                "Cache Overflows": 0,
                "Startup Cost": 0.0,
                "Total Cost": 12.345,
                "Plan Rows": 1,
                "Plan Width": 8,
                "Workers Planned": 1,
                "Workers Launched": 0
            },
            "Query Parameters": "[100, 'abc']",
            "Query Identifier": -7i64
        }
    ]);
    roundtrip(&doc);
    // Non-integral floats live in the f64 column unchanged.
    let arena = PlanArena::flatten(&doc);
    assert_eq!(arena.nodes[0].total_cost, Some(12.345f64));
}

/// ModifyTable upsert vocabulary plus object-array overflow (`Target Tables`).
#[test]
fn modify_table_upsert() {
    let doc = json!([
        {
            "Plan": {
                "Node Type": "ModifyTable",
                "Operation": "Update",
                "Startup Cost": 0.0,
                "Total Cost": 500.0,
                "Plan Rows": 184,
                "Plan Width": 0,
                "Command": "UPDATE",
                "Conflict Resolution": "NOTHING",
                "Conflict Arbiter Indexes": ["x_pkey"],
                "Conflict Filter": "(x.id = 1)",
                "Rows Removed by Conflict Filter": 126,
                "Conflicting Tuples": 184,
                "Tuples Inserted": 0,
                "Target Tables": [
                    { "Relation Name": "child_1", "Schema": "public", "Lock Rows Modified": 10 }
                ]
            }
        }
    ]);
    let back = roundtrip(&doc);
    assert_eq!(back[0]["Plan"]["Target Tables"], doc[0]["Plan"]["Target Tables"]);
}

/// Vocabulary drift never loses a value: unknown dictionary strings and
/// unknown keys overflow verbatim; out-of-width or negative integers in
/// unsigned columns overflow too; `false` survives the absent-vs-false
/// distinction. Overflow entries are merged after the columns, so the
/// verbatim value wins on rebuild.
#[test]
fn vocabulary_drift_overflows_verbatim() {
    let doc = json!([
        {
            "Plan": {
                "Node Type": "QuantumScan",
                "Startup Cost": 0.0,
                "Total Cost": 0.0,
                "Plan Rows": 0,
                "Plan Width": 70000,
                "Parallel Aware": false,
                "Wiggle Space": 42,
                "Future Groups": { "whatever": [1, 2, 3] },
                "Rows Removed by Filter": -3,
                "Sort Key": [1, 2],
                "Plans": []
            },
            "Future Envelope": "keep me"
        }
    ]);
    let arena = PlanArena::flatten(&doc);
    assert_eq!(arena.rebuild(), doc);
    let node = &arena.nodes[0];
    // Unknown node type cannot live in the dictionary.
    assert_eq!(node.node_type, NodeType::Absent);
    // Float columns hold floats; width-exceeded integers do not.
    assert_eq!(node.startup_cost, Some(0.0f64));
    assert_eq!(node.total_cost, Some(0.0f64));
    assert_eq!(node.plan_width, None, "u16 overflow leaves the column absent");
    // Bool false is Some(false), not absent.
    assert_eq!(node.parallel_aware, Some(false));
    // Kind-mismatched, out-of-width, and unknown keys all sit in overflow.
    let overflow = node.overflow.as_ref().expect("drift lands in overflow");
    assert_eq!(overflow["Node Type"], json!("QuantumScan"));
    assert_eq!(overflow["Wiggle Space"], json!(42));
    assert_eq!(overflow["Plan Width"], json!(70000));
    assert_eq!(overflow["Rows Removed by Filter"], json!(-3));
    assert_eq!(overflow["Future Groups"], json!({"whatever": [1, 2, 3]}));
    assert_eq!(overflow["Sort Key"], json!([1, 2]));
    assert_eq!(overflow["Plans"], json!([]));
    assert_eq!(arena.envelope_overflow.as_ref().unwrap()["Future Envelope"], json!("keep me"));
}

/// The numeric-kind contract: integer text in a float column rebuilds as a
/// float, integer columns keep identical integer text.
#[test]
fn integer_text_in_float_columns_rebuilds_as_float() {
    let doc = json!([{
        "Plan": {
            "Node Type": "Seq Scan",
            "Startup Cost": 0,
            "Total Cost": 5,
            "Plan Rows": 10,
            "Plan Width": 4
        }
    }]);
    let back = PlanArena::flatten(&doc).rebuild();
    assert_eq!(back[0]["Plan"]["Startup Cost"], json!(0.0));
    assert_eq!(back[0]["Plan"]["Total Cost"], json!(5.0));
    assert_eq!(back[0]["Plan"]["Plan Rows"], json!(10), "integer columns keep integers");
    assert_eq!(back[0]["Plan"]["Plan Width"], json!(4));
}

/// Workers flatten into typed arena rows keyed to their owner node, rebuild
/// structurally in slot order, and never touch the node overflow blob.
/// Unknown worker keys overflow on the worker row itself.
#[test]
fn workers_flatten_into_typed_rows_and_rebuild_structurally() {
    let doc = json!([{
        "Plan": {
            "Node Type": "Gather",
            "Startup Cost": 0.0,
            "Total Cost": 10.0,
            "Plan Rows": 5,
            "Plan Width": 8,
            "Workers Planned": 2,
            "Workers Launched": 2,
            "Workers": [
                { "Worker Number": 0, "Actual Total Time": 1.5, "Actual Rows": 3, "Actual Loops": 1, "I/O Read Time": 0 },
                { "Worker Number": 1, "Actual Total Time": 2.5, "Shared Hit Blocks": 8, "Sort Method": "quicksort", "Sort Space Used": 1024, "Future Worker Key": true }
            ],
            "Plans": [
                { "Node Type": "Seq Scan", "Relation Name": "t", "Startup Cost": 0.0, "Total Cost": 1.0, "Plan Rows": 1, "Plan Width": 4 }
            ]
        }
    }]);
    let arena = PlanArena::flatten(&doc);
    assert!(arena.nodes[0].overflow.is_none(), "flattened Workers never reach the node blob");
    assert_eq!(arena.workers.len(), 2);
    assert_eq!(arena.workers[0].parent_idx, 0);
    assert_eq!(arena.workers[0].slot, 0);
    assert_eq!(arena.workers[1].slot, 1);
    assert_eq!(arena.workers[1].shared_hit_blocks, Some(8));
    let back = arena.rebuild();
    assert_eq!(back[0]["Plan"]["Workers"].as_array().unwrap().len(), 2);
    assert_eq!(
        back[0]["Plan"]["Workers"][0]["I/O Read Time"],
        json!(0.0),
        "integer text in a worker float column rebuilds as float"
    );
    assert_eq!(back[0]["Plan"]["Workers"][1]["Worker Number"], json!(1));
    assert_eq!(back[0]["Plan"]["Workers"][1]["Sort Method"], json!("quicksort"));
    assert_eq!(
        back[0]["Plan"]["Workers"][1]["Future Worker Key"],
        json!(true),
        "unknown worker keys overflow on the worker row"
    );
    assert_eq!(back[0]["Plan"]["Plans"].as_array().unwrap().len(), 1);
}

/// Worker schema guards: an empty array keeps the `"Workers": []` key via
/// the node overflow, and an entry whose `Worker Number` exceeds the frozen
/// width overflows the whole array verbatim.
#[test]
fn worker_schema_guards_overflow_the_whole_array() {
    let empty = json!([{
        "Plan": { "Node Type": "Gather", "Startup Cost": 0.0, "Total Cost": 1.0, "Plan Rows": 1, "Plan Width": 4, "Workers": [] }
    }]);
    let arena = PlanArena::flatten(&empty);
    assert!(arena.workers.is_empty());
    assert_eq!(arena.nodes[0].overflow.as_ref().unwrap()["Workers"], json!([]));
    assert_eq!(arena.rebuild(), empty);

    let wide = json!([{
        "Plan": {
            "Node Type": "Gather",
            "Startup Cost": 0.0,
            "Total Cost": 1.0,
            "Plan Rows": 1,
            "Plan Width": 4,
            "Workers": [{ "Worker Number": 999, "Actual Rows": 5 }]
        }
    }]);
    let arena = PlanArena::flatten(&wide);
    assert!(arena.workers.is_empty(), "a worker number outside the frozen width overflows the array");
    assert_eq!(arena.rebuild(), wide);
}

/// Malformed documents fall back to the raw blob unchanged.
#[test]
fn malformed_documents_take_the_raw_path() {
    for doc in [json!([]), json!(null), json!([1, 2]), json!({ "Plan": 1 })] {
        let arena = PlanArena::flatten(&doc);
        assert!(arena.nodes.is_empty(), "malformed docs flatten to no nodes");
        assert_eq!(arena.rebuild(), doc);
    }
}

/// Arena index invariants: pre-order DFS, root sentinel, parents before
/// children.
#[test]
fn arena_indexes_are_preorder_with_root_sentinel() {
    let doc = json!([
        {
            "Plan": {
                "Node Type": "Nested Loop",
                "Startup Cost": 0.0,
                "Total Cost": 99.9,
                "Plan Rows": 10,
                "Plan Width": 4,
                "Plans": [
                    {
                        "Node Type": "Seq Scan",
                        "Parent Relationship": "Outer",
                        "Relation Name": "t",
                        "Alias": "t",
                        "Startup Cost": 0.0,
                        "Total Cost": 10.0,
                        "Plan Rows": 5,
                        "Plan Width": 4
                    },
                    {
                        "Node Type": "Index Scan",
                        "Parent Relationship": "Inner",
                        "Index Name": "i",
                        "Startup Cost": 0.1,
                        "Total Cost": 80.0,
                        "Plan Rows": 5,
                        "Plan Width": 4
                    }
                ]
            }
        }
    ]);
    let arena = PlanArena::flatten(&doc);
    let nodes = arena.nodes_view();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].parent_idx, 0, "root stores its own index");
    assert_eq!(nodes[1].parent_idx, 0);
    assert_eq!(nodes[2].parent_idx, 0);
    roundtrip(&doc);
}

/// The PcoPack roundtrip itself: encode a chunk of arenas, decode, full
/// `rebuild` equality against the originals, and an unfiltered scan count.
#[test]
fn codec_roundtrip_preserves_every_document() {
    let docs: Vec<Value> = vec![
        json!([{"Plan": {"Node Type": "Seq Scan", "Relation Name": "t", "Startup Cost": 0.0, "Total Cost": 1.0, "Plan Rows": 1, "Plan Width": 4}}]),
        json!([
            {
                "Plan": {
                    "Node Type": "Aggregate",
                    "Strategy": "Hashed",
                    "Partial Mode": "Finalize",
                    "Startup Cost": 10.0,
                    "Total Cost": 20.0,
                    "Plan Rows": 2,
                    "Plan Width": 8,
                    "Hash Batches": 1,
                    "Hash Buckets": 1024,
                    "Plans": [
                        {"Node Type": "Sort", "Sort Key": ["x"], "Sort Method": "external merge", "Sort Space Type": "Disk", "Sort Space Used": 512, "Startup Cost": 5.0, "Total Cost": 15.0, "Plan Rows": 3, "Plan Width": 8}
                    ]
                },
                "Query Identifier": -1i64
            }
        ]),
        json!([
            {
                "Plan": {
                    "Node Type": "Bitmap Heap Scan",
                    "Recheck Cond": "(x = 1)",
                    "Exact Heap Blocks": 5,
                    "Lossy Heap Blocks": 1,
                    "Rows Removed by Index Recheck": 3
                }
            }
        ]),
    ];
    let arenas: Vec<PlanArena> = docs.iter().map(PlanArena::flatten).collect();
    let bytes = PlanArena::to_bytes(&PlanArena::write(arenas).unwrap()).unwrap();
    let chunks = PlanArena::from_bytes(&bytes).unwrap();
    let back = PlanArena::filter(&chunks, json!({}), &[]).unwrap();
    assert_eq!(back.len(), docs.len());
    for (a, doc) in back.iter().zip(&docs) {
        assert_eq!(a.rebuild(), *doc);
    }
}

/// The lossy mirror quantizes floats to two decimals at write time
/// (float_round lives on every struct owning float columns, so it reaches
/// the node columns, the worker columns, the `Value` columns, and the
/// overflow blobs through the codec), while leaving integers, dictionaries,
/// presence, and the overflow structure intact. Before the codec the
/// rebuild stays exact; the exact mirror keeps the same document value-exact
/// at every stage.
#[test]
fn arena_lossy_rounds_floats_and_keeps_everything_else() {
    let doc = json!([
        {
            "Plan": {
                "Node Type": "Sort",
                "Startup Cost": 0.0,
                "Total Cost": 12.3456,
                "Plan Rows": 7,
                "I/O Read Time": 0.001,
                "Sort Space Used": 25,
                "Sort Method": "top-N heapsort",
                "Parallel Aware": false,
                "Workers": [{ "Worker Number": 0, "Actual Total Time": 1.239 }],
                "Plans": []
            },
            "Query Identifier": 42i64
        }
    ]);
    assert_eq!(arena::PlanArena::flatten(&doc).rebuild(), doc, "the exact mirror stays exact");
    assert_eq!(
        arena_lossy::PlanArena::flatten(&doc).rebuild(),
        doc,
        "quantization is codec-side; flatten/rebuild alone stays exact"
    );

    let arenas = vec![arena_lossy::PlanArena::flatten(&doc)];
    let bytes = arena_lossy::PlanArena::to_bytes(&arena_lossy::PlanArena::write(arenas).unwrap()).unwrap();
    let chunks = arena_lossy::PlanArena::from_bytes(&bytes).unwrap();
    let back = arena_lossy::PlanArena::filter(&chunks, json!({}), &[]).unwrap();
    let plan = &back[0].rebuild()[0]["Plan"];
    assert_eq!(plan["Total Cost"], json!(12.35));
    assert_eq!(plan["I/O Read Time"], json!(0.0));
    assert_eq!(plan["Startup Cost"], json!(0.0), "integral floats keep the float kind");
    assert_eq!(plan["Workers"][0]["Actual Total Time"], json!(1.24), "floats round in the worker column too");
    assert_eq!(plan["Workers"][0]["Worker Number"], json!(0), "integers stay integers");
    assert_eq!(plan["Plan Rows"], json!(7));
    assert_eq!(plan["Sort Method"], json!("top-N heapsort"));
    assert_eq!(plan["Parallel Aware"], json!(false), "absent-vs-false distinction survives");
    assert_eq!(plan["Plans"], json!([]));
    assert_eq!(back[0].rebuild()[0]["Query Identifier"], json!(42));
}

/// The lossy column through the codec: decode returns the quantized floats
/// stable, with the arena structure intact.
#[test]
fn arena_lossy_codec_roundtrip_keeps_rounded_values() {
    let docs: Vec<Value> = vec![
        json!([ { "Plan": { "Node Type": "Seq Scan", "Total Cost": 12.3456, "Plan Rows": 3, "Filter": "(x = 1)" } } ]),
        json!([
            {
                "Plan": {
                    "Node Type": "Gather",
                    "Workers Planned": 2,
                    "Workers": [{ "Worker Number": 0, "I/O Read Time": 44.777 }],
                    "Plans": [{ "Node Type": "Seq Scan", "Relation Name": "t", "Startup Cost": 0.0, "Total Cost": 5.555, "Plan Rows": 1, "Plan Width": 4 }]
                }
            }
        ]),
    ];
    let arenas: Vec<arena_lossy::PlanArena> = docs.iter().map(arena_lossy::PlanArena::flatten).collect();
    let bytes = arena_lossy::PlanArena::to_bytes(&arena_lossy::PlanArena::write(arenas).unwrap()).unwrap();
    let chunks = arena_lossy::PlanArena::from_bytes(&bytes).unwrap();
    let back = arena_lossy::PlanArena::filter(&chunks, json!({}), &[]).unwrap();
    assert_eq!(back.len(), docs.len());
    let a = back[0].rebuild();
    assert_eq!(a[0]["Plan"]["Total Cost"], json!(12.35));
    assert_eq!(a[0]["Plan"]["Plan Rows"], json!(3));
    assert_eq!(a[0]["Plan"]["Filter"], json!("(x = 1)"));
    let b = back[1].rebuild();
    assert_eq!(
        b[0]["Plan"]["Workers"][0]["I/O Read Time"],
        json!(44.78),
        "rounds in the worker column through the codec"
    );
    assert_eq!(b[0]["Plan"]["Workers Planned"], json!(2));
    assert_eq!(b[0]["Plan"]["Plans"][0]["Relation Name"], json!("t"));
}
