//! Lossy mirror of `arena.rs`: the identical flattened arena for the
//! `PcoPack (arena lossy)` column, but every struct that owns float payload
//! columns declares `float_round = 2` on itself; the parameter does not
//! propagate across derived-struct boundaries, so each struct states it.
//! Costs, times, and floats inside `Value` columns quantize to two decimals;
//! integers, dictionaries, strings, presence, and the overflow verbatim rule
//! are untouched. Benchmark-only code, compiled into the postgres bench binary
//! and into `tests/query_explains_arena.rs`, never into the library.
//!
//! The plan tree is expanded into a `Vec<PlanNode>` per row (pre-order DFS,
//! children in `Plans` array order). `#[derive(PcoPack)]` flattens that
//! `Vec` across the chunk into one columnar stream, so the arena becomes the
//! native column layout: a lengths column holds the row boundaries and every
//! node field is a single pco/msgpack column spanning all ~2.3M nodes. Key
//! bytes disappear because they exist only in the frozen schema; dictionaries
//! compress the enumeration columns.
//!
//! `parent_idx` is a row-local arena index: the root stores its own index as
//! the sentinel, so `parent_idx == self` means "root". `Workers` entries have
//! no `Node Type` or `Plans` subtree of their own in this capture and ride
//! the node `overflow` blob verbatim; flattening them is a later
//! first-cut-vs-measure decision (plan section 8).
//!
//! The validation contract on this mirror: rebuild preserves everything the
//! quantization does not touch (key presence, integer-vs-float kind,
//! dictionaries, overflow verbatim) and lands floats on their two-decimal
//! quantized values. The capture carries costs and times with up to three
//! decimals (tmp/arena_probe.txt), so the two-decimal rounding is exactly
//! what makes this column lossy; the exact mirror in `arena.rs` keeps them.

use pco_pack::PcoPack;
use serde_json::{Map, Number, Value};
use smol_str::SmolStr;
use std::collections::BTreeMap;

/// Any JSON number as captured, preserving the integer-vs-float distinction
/// serde_json makes internally. `Absent` is the missing-key marker; rebuild
/// emits no key for it, so absent-vs-zero stays distinguishable.
#[derive(PcoPack, Clone, Copy, Debug, Default, PartialEq)]
pub enum JsonNum {
    #[default]
    Absent,
    Int(i64),
    Float(f64),
}

impl JsonNum {
    /// Parse a captured JSON number. Integers beyond i64 range become floats
    /// only if exactly representable; anything else overflows the key.
    pub fn of(n: &Number) -> Option<Self> {
        if let Some(i) = n.as_i64() {
            return Some(JsonNum::Int(i));
        }
        if let Some(u) = n.as_u64() {
            let f = u as f64;
            if f as u64 == u {
                return Some(JsonNum::Float(f));
            }
            return None;
        }
        let f = n.as_f64()?;
        Some(JsonNum::Float(f))
    }

    pub fn to_value(self) -> Option<Number> {
        match self {
            JsonNum::Absent => None,
            JsonNum::Int(i) => Some(Number::from(i)),
            JsonNum::Float(f) => Number::from_f64(f),
        }
    }
}

/// Generate a frozen dictionary enum: a `PcoPack` unit enum (the derive stores
/// a tag column, which is the arena dictionary), `Absent` as code 0 so
/// absent-vs-zero stays distinguishable, and `from_pg`/`to_pg` mapping to the
/// exact Postgres JSON spellings. Strings outside the frozen table return
/// `None` from `from_pg`; the caller routes those keys to overflow.
macro_rules! pg_dict {
    ($name:ident { $($variant:ident => $pg:literal),* $(,)? }) => {
        #[derive(PcoPack, Clone, Copy, Debug, Default, PartialEq)]
        pub enum $name {
            #[default]
            Absent,
            $($variant),*
        }

        impl $name {
            pub fn from_pg(s: &str) -> Option<Self> {
                match s {
                    $($pg => Some($name::$variant),)*
                    _ => None,
                }
            }

            pub fn to_pg(self) -> Option<&'static str> {
                match self {
                    $name::Absent => None,
                    $($name::$variant => Some($pg)),*
                }
            }
        }
    };
}

pg_dict!(NodeType {
    Aggregate => "Aggregate",
    Append => "Append",
    BitmapHeapScan => "Bitmap Heap Scan",
    BitmapIndexScan => "Bitmap Index Scan",
    BitmapAnd => "BitmapAnd",
    BitmapOr => "BitmapOr",
    CteScan => "CTE Scan",
    FunctionScan => "Function Scan",
    Gather => "Gather",
    GatherMerge => "Gather Merge",
    Hash => "Hash",
    HashJoin => "Hash Join",
    IncrementalSort => "Incremental Sort",
    IndexOnlyScan => "Index Only Scan",
    IndexScan => "Index Scan",
    Limit => "Limit",
    Materialize => "Materialize",
    Memoize => "Memoize",
    MergeAppend => "Merge Append",
    MergeJoin => "Merge Join",
    ModifyTable => "ModifyTable",
    NestedLoop => "Nested Loop",
    ProjectSet => "ProjectSet",
    RecursiveUnion => "Recursive Union",
    Result => "Result",
    SeqScan => "Seq Scan",
    SetOp => "SetOp",
    Sort => "Sort",
    SubqueryScan => "Subquery Scan",
    Unique => "Unique",
    WindowAgg => "WindowAgg",
    WorkTableScan => "WorkTable Scan",
});

pg_dict!(ParentRelationship {
    InitPlan => "InitPlan",
    Inner => "Inner",
    Member => "Member",
    Outer => "Outer",
    SubPlan => "SubPlan",
    Subquery => "Subquery",
});

pg_dict!(JoinType {
    Anti => "Anti",
    Full => "Full",
    Inner => "Inner",
    Left => "Left",
    Right => "Right",
    Semi => "Semi",
});

pg_dict!(ScanDirection {
    Backward => "Backward",
    Forward => "Forward",
});

pg_dict!(SortMethod {
    ExternalMerge => "external merge",
    ExternalSort => "external sort",
    Quicksort => "quicksort",
    TopNHeapsort => "top-N heapsort",
});

pg_dict!(SortSpaceType {
    Disk => "Disk",
    Memory => "Memory",
});

pg_dict!(PartialMode {
    Finalize => "Finalize",
    Partial => "Partial",
    Simple => "Simple",
});

pg_dict!(Strategy {
    Hashed => "Hashed",
    Plain => "Plain",
    Sorted => "Sorted",
});

pg_dict!(Operation {
    Delete => "Delete",
    Insert => "Insert",
    Update => "Update",
});

pg_dict!(ConflictResolution {
    Nothing => "NOTHING",
    Update => "UPDATE",
});

pg_dict!(CacheMode {
    Binary => "binary",
    Logical => "logical",
});

/// One flattened plan node. Universal keys (100% presence in the capture)
/// use plain columns; sparse keys use `Option` so absent never collides with
/// zero; every unrecognized key or kind mismatch lands in `overflow` rather
/// than failing, preserving forward compatibility across Postgres versions.
#[derive(PcoPack, Clone, Debug, Default, PartialEq)]
#[pco_pack(float_round = 2)]
pub struct PlanNode {
    /// Row-local arena index of the parent; the root stores its own index.
    pub parent_idx: u32,

    pub node_type: NodeType,
    pub parent_relationship: Option<ParentRelationship>,
    pub join_type: Option<JoinType>,
    pub scan_direction: Option<ScanDirection>,
    pub sort_method: Option<SortMethod>,
    pub sort_space_type: Option<SortSpaceType>,
    pub partial_mode: Option<PartialMode>,
    pub strategy: Option<Strategy>,
    pub operation: Option<Operation>,
    pub conflict_resolution: Option<ConflictResolution>,
    pub cache_mode: Option<CacheMode>,

    pub parallel_aware: Option<bool>,
    pub async_capable: Option<bool>,
    pub inner_unique: Option<bool>,
    pub single_copy: Option<bool>,
    pub disabled: Option<bool>,

    pub startup_cost: JsonNum,
    pub total_cost: JsonNum,
    pub actual_total_time: JsonNum,
    pub actual_startup_time: JsonNum,

    pub plan_rows: JsonNum,
    pub actual_rows: JsonNum,
    pub plan_width: JsonNum,
    pub actual_loops: JsonNum,

    pub shared_hit_blocks: JsonNum,
    pub shared_read_blocks: JsonNum,
    pub shared_written_blocks: JsonNum,
    pub shared_dirtied_blocks: JsonNum,
    pub local_hit_blocks: JsonNum,
    pub local_read_blocks: JsonNum,
    pub local_written_blocks: JsonNum,
    pub local_dirtied_blocks: JsonNum,
    pub temp_read_blocks: JsonNum,
    pub temp_written_blocks: JsonNum,

    pub io_read_time: JsonNum,
    pub io_write_time: JsonNum,
    pub shared_io_read_time: JsonNum,
    pub shared_io_write_time: JsonNum,
    pub local_io_read_time: JsonNum,
    pub local_io_write_time: JsonNum,
    pub temp_io_read_time: JsonNum,
    pub temp_io_write_time: JsonNum,

    pub heap_fetches: JsonNum,
    pub exact_heap_blocks: JsonNum,
    pub lossy_heap_blocks: JsonNum,
    pub rows_removed_by_filter: JsonNum,
    pub rows_removed_by_index_recheck: JsonNum,
    pub rows_removed_by_join_filter: JsonNum,
    pub rows_removed_by_conflict_filter: JsonNum,
    pub conflicting_tuples: JsonNum,
    pub tuples_inserted: JsonNum,
    pub subplans_removed: JsonNum,
    pub workers_planned: JsonNum,
    pub workers_launched: JsonNum,
    pub peak_memory_usage: JsonNum,
    pub disk_usage: JsonNum,
    pub hash_batches: JsonNum,
    pub hash_buckets: JsonNum,
    pub original_hash_batches: JsonNum,
    pub original_hash_buckets: JsonNum,
    pub hashagg_batches: JsonNum,
    pub planned_partitions: JsonNum,
    pub sort_space_used: JsonNum,
    pub cache_hits: JsonNum,
    pub cache_misses: JsonNum,
    pub cache_evictions: JsonNum,
    pub cache_overflows: JsonNum,
    pub index_searches: JsonNum,
    pub maximum_storage: JsonNum,

    pub alias: Option<SmolStr>,
    pub schema: Option<SmolStr>,
    pub relation_name: Option<SmolStr>,
    pub index_name: Option<SmolStr>,
    pub cte_name: Option<SmolStr>,
    pub subplan_name: Option<SmolStr>,
    pub filter: Option<SmolStr>,
    pub index_cond: Option<SmolStr>,
    pub recheck_cond: Option<SmolStr>,
    pub hash_cond: Option<SmolStr>,
    pub join_filter: Option<SmolStr>,
    pub one_time_filter: Option<SmolStr>,
    pub function_call: Option<SmolStr>,
    pub function_name: Option<SmolStr>,
    pub run_condition: Option<SmolStr>,
    pub command: Option<SmolStr>,
    pub storage: Option<SmolStr>,
    pub cache_key: Option<SmolStr>,
    pub merge_cond: Option<SmolStr>,
    pub conflict_filter: Option<SmolStr>,

    /// `Option` distinguishes absent from the empty array that the flattened
    /// lengths column cannot express once `Plans`-style children are folded
    /// out; empty string arrays overflow verbatim instead.
    pub output: Option<Vec<SmolStr>>,
    pub sort_key: Option<Vec<SmolStr>>,
    pub group_key: Option<Vec<SmolStr>>,
    pub conflict_arbiter_indexes: Option<Vec<SmolStr>>,
    pub presorted_key: Option<Vec<SmolStr>>,

    /// Keys outside the frozen schema, plus objects (`Workers`,
    /// `Full-sort Groups`, `Pre-sorted Groups`, `Target Tables`, `JIT`,
    /// ...) and any value whose kind drifted from the column's type.
    /// Emitted back unchanged on rebuild.
    pub overflow: Option<BTreeMap<SmolStr, Value>>,
}

/// One row's flattened EXPLAIN document. `raw` backstops documents whose
/// overall shape fell outside the flattened schema (empty or malformed);
/// when set, `nodes` is empty and rebuild returns the blob unchanged.
#[derive(PcoPack, Clone, Debug, Default, PartialEq)]
#[pco_pack(float_round = 2)]
pub struct PlanArena {
    pub query_identifier: Option<i64>,
    pub query_parameters: Option<String>,
    /// `Triggers` passes through as raw objects (empty in every row that
    /// carries the key in this capture).
    pub triggers: Option<Vec<Value>>,
    /// Envelope keys outside `Plan`/`Triggers`/`Query Identifier`/`Query Parameters`.
    pub envelope_overflow: Option<BTreeMap<SmolStr, Value>>,
    /// Whole-document fallback blob.
    pub raw: Option<Value>,
    /// The arena: pre-order DFS of the plan tree, one entry per node.
    pub nodes: Vec<PlanNode>,
}

impl PlanArena {
    /// Flatten one `output_json` document
    /// (`[{ Plan, Triggers?, Query Identifier?, Query Parameters? }]`).
    /// Documents outside the expected envelope shape fall back to `raw`.
    pub fn flatten(doc: &Value) -> Self {
        let mut arena = PlanArena::default();
        let Some(Value::Object(envelope)) = doc.as_array().and_then(|a| a.first()) else {
            arena.raw = Some(doc.clone());
            return arena;
        };
        for (k, v) in envelope {
            match k.as_str() {
                "Plan" => {
                    let Some(pmap) = v.as_object() else {
                        arena.raw = Some(doc.clone());
                        return arena;
                    };
                    flatten_node(pmap, arena.nodes.len() as u32, &mut arena.nodes);
                }
                "Query Identifier" => match v.as_i64() {
                    Some(i) => arena.query_identifier = Some(i),
                    None => overflow_envelope(&mut arena, k, v),
                },
                "Query Parameters" => match v.as_str() {
                    Some(s) => arena.query_parameters = Some(s.to_string()),
                    None => overflow_envelope(&mut arena, k, v),
                },
                "Triggers" => match v.as_array() {
                    Some(t) => arena.triggers = Some(t.clone()),
                    None => overflow_envelope(&mut arena, k, v),
                },
                _ => overflow_envelope(&mut arena, k, v),
            }
        }
        // A document can carry the envelope without ever hitting the loop
        // body only if it has no keys at all; the Plan arm sets nodes for
        // every captured row.
        if arena.nodes.is_empty() {
            arena.raw = Some(doc.clone());
        }
        arena
    }

    /// Rebuild the exact `output_json` `Value` for this row.
    pub fn rebuild(&self) -> Value {
        if let Some(raw) = &self.raw {
            return raw.clone();
        }
        let mut envelope = Map::new();
        if let Some(overflow) = &self.envelope_overflow {
            for (k, v) in overflow {
                envelope.insert(k.to_string(), v.clone());
            }
        }
        if let Some(qi) = self.query_identifier {
            envelope.insert("Query Identifier".into(), Value::from(qi));
        }
        if let Some(qp) = &self.query_parameters {
            envelope.insert("Query Parameters".into(), Value::String(qp.clone()));
        }
        if let Some(t) = &self.triggers {
            envelope.insert("Triggers".into(), Value::Array(t.clone()));
        }
        envelope.insert("Plan".into(), rebuild_tree(&self.nodes));
        Value::Array(vec![Value::Object(envelope)])
    }

    /// Borrowed typed view of the row's arena entries (no Value allocation).
    /// The flat form's whole point: consumers scan this slice instead of
    /// materializing plan-node maps.
    #[allow(dead_code)] // read by tests/query_explains_arena.rs; the bench verifies counts
    pub fn nodes_view(&self) -> &[PlanNode] {
        &self.nodes
    }
}

/// Extract every typed column of one plan node in a single pass over the
/// object, then push the node and recurse its `Plans` children so arena
/// indices stay contiguous in pre-order DFS. Keys outside the frozen schema
/// and kind mismatches overflow verbatim; a node is pushed exactly once.
fn flatten_node(map: &Map<String, Value>, parent_idx: u32, arena: &mut Vec<PlanNode>) {
    let mut node = PlanNode { parent_idx, ..Default::default() };

    macro_rules! dict_col {
        ($key:literal, $v:expr, $slot:ident, $E:ident) => {
            match $v {
                Value::String(s) => match $E::from_pg(s) {
                    Some(d) => node.$slot = Some(d),
                    None => overflow(&mut node, $key, $v),
                },
                _ => overflow(&mut node, $key, $v),
            }
        };
    }
    macro_rules! bool_col {
        ($key:literal, $v:expr, $slot:ident) => {
            match $v {
                Value::Bool(b) => node.$slot = Some(*b),
                _ => overflow(&mut node, $key, $v),
            }
        };
    }
    macro_rules! num_col {
        ($key:literal, $v:expr, $slot:ident) => {
            match $v {
                Value::Number(n) => match JsonNum::of(n) {
                    Some(j) => node.$slot = j,
                    None => overflow(&mut node, $key, $v),
                },
                _ => overflow(&mut node, $key, $v),
            }
        };
    }
    macro_rules! str_col {
        ($key:literal, $v:expr, $slot:ident) => {
            match $v {
                Value::String(s) => node.$slot = Some(s.as_str().into()),
                _ => overflow(&mut node, $key, $v),
            }
        };
    }
    macro_rules! arr_col {
        ($key:literal, $v:expr, $slot:ident) => {
            match $v {
                Value::Array(a) => match a.iter().map(Value::as_str).collect::<Option<Vec<_>>>() {
                    Some(s) => node.$slot = Some(s.into_iter().map(SmolStr::from).collect()),
                    None => overflow(&mut node, $key, $v),
                },
                _ => overflow(&mut node, $key, $v),
            }
        };
    }

    for (k, v) in map {
        match k.as_str() {
            "Plans" => {} // recursed after the node is pushed, below
            "Node Type" => match v {
                Value::String(s) => match NodeType::from_pg(s) {
                    Some(d) => node.node_type = d,
                    None => overflow(&mut node, "Node Type", v),
                },
                _ => overflow(&mut node, "Node Type", v),
            },
            "Parent Relationship" => dict_col!("Parent Relationship", v, parent_relationship, ParentRelationship),
            "Join Type" => dict_col!("Join Type", v, join_type, JoinType),
            "Scan Direction" => dict_col!("Scan Direction", v, scan_direction, ScanDirection),
            "Sort Method" => dict_col!("Sort Method", v, sort_method, SortMethod),
            "Sort Space Type" => dict_col!("Sort Space Type", v, sort_space_type, SortSpaceType),
            "Partial Mode" => dict_col!("Partial Mode", v, partial_mode, PartialMode),
            "Strategy" => dict_col!("Strategy", v, strategy, Strategy),
            "Operation" => dict_col!("Operation", v, operation, Operation),
            "Conflict Resolution" => dict_col!("Conflict Resolution", v, conflict_resolution, ConflictResolution),
            "Cache Mode" => dict_col!("Cache Mode", v, cache_mode, CacheMode),

            "Parallel Aware" => bool_col!("Parallel Aware", v, parallel_aware),
            "Async Capable" => bool_col!("Async Capable", v, async_capable),
            "Inner Unique" => bool_col!("Inner Unique", v, inner_unique),
            "Single Copy" => bool_col!("Single Copy", v, single_copy),
            "Disabled" => bool_col!("Disabled", v, disabled),

            "Startup Cost" => num_col!("Startup Cost", v, startup_cost),
            "Total Cost" => num_col!("Total Cost", v, total_cost),
            "Actual Total Time" => num_col!("Actual Total Time", v, actual_total_time),
            "Actual Startup Time" => num_col!("Actual Startup Time", v, actual_startup_time),
            "Plan Rows" => num_col!("Plan Rows", v, plan_rows),
            "Actual Rows" => num_col!("Actual Rows", v, actual_rows),
            "Plan Width" => num_col!("Plan Width", v, plan_width),
            "Actual Loops" => num_col!("Actual Loops", v, actual_loops),
            "Shared Hit Blocks" => num_col!("Shared Hit Blocks", v, shared_hit_blocks),
            "Shared Read Blocks" => num_col!("Shared Read Blocks", v, shared_read_blocks),
            "Shared Written Blocks" => num_col!("Shared Written Blocks", v, shared_written_blocks),
            "Shared Dirtied Blocks" => num_col!("Shared Dirtied Blocks", v, shared_dirtied_blocks),
            "Local Hit Blocks" => num_col!("Local Hit Blocks", v, local_hit_blocks),
            "Local Read Blocks" => num_col!("Local Read Blocks", v, local_read_blocks),
            "Local Written Blocks" => num_col!("Local Written Blocks", v, local_written_blocks),
            "Local Dirtied Blocks" => num_col!("Local Dirtied Blocks", v, local_dirtied_blocks),
            "Temp Read Blocks" => num_col!("Temp Read Blocks", v, temp_read_blocks),
            "Temp Written Blocks" => num_col!("Temp Written Blocks", v, temp_written_blocks),
            "I/O Read Time" => num_col!("I/O Read Time", v, io_read_time),
            "I/O Write Time" => num_col!("I/O Write Time", v, io_write_time),
            "Shared I/O Read Time" => num_col!("Shared I/O Read Time", v, shared_io_read_time),
            "Shared I/O Write Time" => num_col!("Shared I/O Write Time", v, shared_io_write_time),
            "Local I/O Read Time" => num_col!("Local I/O Read Time", v, local_io_read_time),
            "Local I/O Write Time" => num_col!("Local I/O Write Time", v, local_io_write_time),
            "Temp I/O Read Time" => num_col!("Temp I/O Read Time", v, temp_io_read_time),
            "Temp I/O Write Time" => num_col!("Temp I/O Write Time", v, temp_io_write_time),
            "Heap Fetches" => num_col!("Heap Fetches", v, heap_fetches),
            "Exact Heap Blocks" => num_col!("Exact Heap Blocks", v, exact_heap_blocks),
            "Lossy Heap Blocks" => num_col!("Lossy Heap Blocks", v, lossy_heap_blocks),
            "Rows Removed by Filter" => num_col!("Rows Removed by Filter", v, rows_removed_by_filter),
            "Rows Removed by Index Recheck" => {
                num_col!("Rows Removed by Index Recheck", v, rows_removed_by_index_recheck)
            }
            "Rows Removed by Join Filter" => num_col!("Rows Removed by Join Filter", v, rows_removed_by_join_filter),
            "Rows Removed by Conflict Filter" => {
                num_col!("Rows Removed by Conflict Filter", v, rows_removed_by_conflict_filter)
            }
            "Conflicting Tuples" => num_col!("Conflicting Tuples", v, conflicting_tuples),
            "Tuples Inserted" => num_col!("Tuples Inserted", v, tuples_inserted),
            "Subplans Removed" => num_col!("Subplans Removed", v, subplans_removed),
            "Workers Planned" => num_col!("Workers Planned", v, workers_planned),
            "Workers Launched" => num_col!("Workers Launched", v, workers_launched),
            "Peak Memory Usage" => num_col!("Peak Memory Usage", v, peak_memory_usage),
            "Disk Usage" => num_col!("Disk Usage", v, disk_usage),
            "Hash Batches" => num_col!("Hash Batches", v, hash_batches),
            "Hash Buckets" => num_col!("Hash Buckets", v, hash_buckets),
            "Original Hash Batches" => num_col!("Original Hash Batches", v, original_hash_batches),
            "Original Hash Buckets" => num_col!("Original Hash Buckets", v, original_hash_buckets),
            "HashAgg Batches" => num_col!("HashAgg Batches", v, hashagg_batches),
            "Planned Partitions" => num_col!("Planned Partitions", v, planned_partitions),
            "Sort Space Used" => num_col!("Sort Space Used", v, sort_space_used),
            "Cache Hits" => num_col!("Cache Hits", v, cache_hits),
            "Cache Misses" => num_col!("Cache Misses", v, cache_misses),
            "Cache Evictions" => num_col!("Cache Evictions", v, cache_evictions),
            "Cache Overflows" => num_col!("Cache Overflows", v, cache_overflows),
            "Index Searches" => num_col!("Index Searches", v, index_searches),
            "Maximum Storage" => num_col!("Maximum Storage", v, maximum_storage),

            "Alias" => str_col!("Alias", v, alias),
            "Schema" => str_col!("Schema", v, schema),
            "Relation Name" => str_col!("Relation Name", v, relation_name),
            "Index Name" => str_col!("Index Name", v, index_name),
            "CTE Name" => str_col!("CTE Name", v, cte_name),
            "Subplan Name" => str_col!("Subplan Name", v, subplan_name),
            "Filter" => str_col!("Filter", v, filter),
            "Index Cond" => str_col!("Index Cond", v, index_cond),
            "Recheck Cond" => str_col!("Recheck Cond", v, recheck_cond),
            "Hash Cond" => str_col!("Hash Cond", v, hash_cond),
            "Join Filter" => str_col!("Join Filter", v, join_filter),
            "One-Time Filter" => str_col!("One-Time Filter", v, one_time_filter),
            "Function Call" => str_col!("Function Call", v, function_call),
            "Function Name" => str_col!("Function Name", v, function_name),
            "Run Condition" => str_col!("Run Condition", v, run_condition),
            "Command" => str_col!("Command", v, command),
            "Storage" => str_col!("Storage", v, storage),
            "Cache Key" => str_col!("Cache Key", v, cache_key),
            "Merge Cond" => str_col!("Merge Cond", v, merge_cond),
            "Conflict Filter" => str_col!("Conflict Filter", v, conflict_filter),

            "Output" => arr_col!("Output", v, output),
            "Sort Key" => arr_col!("Sort Key", v, sort_key),
            "Group Key" => arr_col!("Group Key", v, group_key),
            "Conflict Arbiter Indexes" => arr_col!("Conflict Arbiter Indexes", v, conflict_arbiter_indexes),
            "Presorted Key" => arr_col!("Presorted Key", v, presorted_key),

            // Workers, Full-sort Groups, Pre-sorted Groups, Target Tables,
            // JIT, and any future key: verbatim. An empty Plans array takes
            // this path too, which keeps "Plans": [] distinguishable from
            // "Plans" absent after rebuild.
            _ => overflow(&mut node, k, v),
        }
    }

    // Flatten Plans only when it is a non-empty all-object array; anything
    // else (empty, non-object elements) overflows verbatim so rebuild
    // distinguishes `"Plans": []` from `Plans` absent.
    let children: &[Value] = match map.get("Plans") {
        Some(Value::Array(a)) if !a.is_empty() && a.iter().all(Value::is_object) => a,
        Some(v) => {
            overflow(&mut node, "Plans", v);
            &[]
        }
        None => &[],
    };
    let idx = arena.len() as u32;
    arena.push(node);
    for c in children {
        // all-object guard above
        flatten_node(c.as_object().expect("Plans children are objects"), idx, arena);
    }
}

fn overflow(node: &mut PlanNode, key: &str, value: &Value) {
    node.overflow.get_or_insert_with(BTreeMap::new).insert(key.into(), value.clone());
}

fn overflow_envelope(arena: &mut PlanArena, key: &str, value: &Value) {
    arena.envelope_overflow.get_or_insert_with(BTreeMap::new).insert(key.into(), value.clone());
}

/// Rebuild the plan tree from one row's arena. Children come after their
/// parent by construction, so one forward pass groups child indices by
/// parent; the root is entry 0 (`parent_idx == self` sentinel).
fn rebuild_tree(arena: &[PlanNode]) -> Value {
    if arena.is_empty() {
        return Value::Null; // raw backstops; reachable only for empty envelopes
    }
    let mut kids: Vec<Vec<u32>> = vec![Vec::new(); arena.len()];
    for (j, n) in arena.iter().enumerate() {
        if j != n.parent_idx as usize {
            kids[n.parent_idx as usize].push(j as u32);
        }
    }
    rebuild_node(0, arena, &kids)
}

fn rebuild_node(i: usize, arena: &[PlanNode], kids: &[Vec<u32>]) -> Value {
    let n = &arena[i];
    let mut map = Map::new();

    macro_rules! put_dict {
        ($key:literal, $v:expr) => {
            if let Some(s) = $v.to_pg() {
                map.insert($key.into(), Value::String(s.into()));
            }
        };
    }
    macro_rules! put_opt_dict {
        ($key:literal, $v:expr) => {
            if let Some(d) = $v {
                if let Some(s) = d.to_pg() {
                    map.insert($key.into(), Value::String(s.into()));
                }
            }
        };
    }
    macro_rules! put_bool {
        ($key:literal, $v:expr) => {
            if let Some(b) = $v {
                map.insert($key.into(), Value::Bool(b));
            }
        };
    }
    macro_rules! put_num {
        ($key:literal, $v:expr) => {
            if let Some(x) = $v.to_value() {
                map.insert($key.into(), Value::Number(x));
            }
        };
    }
    macro_rules! put_str {
        ($key:literal, $v:expr) => {
            if let Some(s) = $v.as_ref() {
                map.insert($key.into(), Value::String(s.to_string()));
            }
        };
    }
    macro_rules! put_arr {
        ($key:literal, $v:expr) => {
            if let Some(a) = $v.as_ref() {
                map.insert($key.into(), Value::Array(a.iter().map(|s| Value::String(s.to_string())).collect()));
            }
        };
    }

    put_dict!("Node Type", n.node_type);
    put_opt_dict!("Parent Relationship", n.parent_relationship);
    put_opt_dict!("Join Type", n.join_type);
    put_opt_dict!("Scan Direction", n.scan_direction);
    put_opt_dict!("Sort Method", n.sort_method);
    put_opt_dict!("Sort Space Type", n.sort_space_type);
    put_opt_dict!("Partial Mode", n.partial_mode);
    put_opt_dict!("Strategy", n.strategy);
    put_opt_dict!("Operation", n.operation);
    put_opt_dict!("Conflict Resolution", n.conflict_resolution);
    put_opt_dict!("Cache Mode", n.cache_mode);

    put_bool!("Parallel Aware", n.parallel_aware);
    put_bool!("Async Capable", n.async_capable);
    put_bool!("Inner Unique", n.inner_unique);
    put_bool!("Single Copy", n.single_copy);
    put_bool!("Disabled", n.disabled);

    put_num!("Startup Cost", n.startup_cost);
    put_num!("Total Cost", n.total_cost);
    put_num!("Actual Total Time", n.actual_total_time);
    put_num!("Actual Startup Time", n.actual_startup_time);
    put_num!("Plan Rows", n.plan_rows);
    put_num!("Actual Rows", n.actual_rows);
    put_num!("Plan Width", n.plan_width);
    put_num!("Actual Loops", n.actual_loops);
    put_num!("Shared Hit Blocks", n.shared_hit_blocks);
    put_num!("Shared Read Blocks", n.shared_read_blocks);
    put_num!("Shared Written Blocks", n.shared_written_blocks);
    put_num!("Shared Dirtied Blocks", n.shared_dirtied_blocks);
    put_num!("Local Hit Blocks", n.local_hit_blocks);
    put_num!("Local Read Blocks", n.local_read_blocks);
    put_num!("Local Written Blocks", n.local_written_blocks);
    put_num!("Local Dirtied Blocks", n.local_dirtied_blocks);
    put_num!("Temp Read Blocks", n.temp_read_blocks);
    put_num!("Temp Written Blocks", n.temp_written_blocks);
    put_num!("I/O Read Time", n.io_read_time);
    put_num!("I/O Write Time", n.io_write_time);
    put_num!("Shared I/O Read Time", n.shared_io_read_time);
    put_num!("Shared I/O Write Time", n.shared_io_write_time);
    put_num!("Local I/O Read Time", n.local_io_read_time);
    put_num!("Local I/O Write Time", n.local_io_write_time);
    put_num!("Temp I/O Read Time", n.temp_io_read_time);
    put_num!("Temp I/O Write Time", n.temp_io_write_time);
    put_num!("Heap Fetches", n.heap_fetches);
    put_num!("Exact Heap Blocks", n.exact_heap_blocks);
    put_num!("Lossy Heap Blocks", n.lossy_heap_blocks);
    put_num!("Rows Removed by Filter", n.rows_removed_by_filter);
    put_num!("Rows Removed by Index Recheck", n.rows_removed_by_index_recheck);
    put_num!("Rows Removed by Join Filter", n.rows_removed_by_join_filter);
    put_num!("Rows Removed by Conflict Filter", n.rows_removed_by_conflict_filter);
    put_num!("Conflicting Tuples", n.conflicting_tuples);
    put_num!("Tuples Inserted", n.tuples_inserted);
    put_num!("Subplans Removed", n.subplans_removed);
    put_num!("Workers Planned", n.workers_planned);
    put_num!("Workers Launched", n.workers_launched);
    put_num!("Peak Memory Usage", n.peak_memory_usage);
    put_num!("Disk Usage", n.disk_usage);
    put_num!("Hash Batches", n.hash_batches);
    put_num!("Hash Buckets", n.hash_buckets);
    put_num!("Original Hash Batches", n.original_hash_batches);
    put_num!("Original Hash Buckets", n.original_hash_buckets);
    put_num!("HashAgg Batches", n.hashagg_batches);
    put_num!("Planned Partitions", n.planned_partitions);
    put_num!("Sort Space Used", n.sort_space_used);
    put_num!("Cache Hits", n.cache_hits);
    put_num!("Cache Misses", n.cache_misses);
    put_num!("Cache Evictions", n.cache_evictions);
    put_num!("Cache Overflows", n.cache_overflows);
    put_num!("Index Searches", n.index_searches);
    put_num!("Maximum Storage", n.maximum_storage);

    put_str!("Alias", n.alias);
    put_str!("Schema", n.schema);
    put_str!("Relation Name", n.relation_name);
    put_str!("Index Name", n.index_name);
    put_str!("CTE Name", n.cte_name);
    put_str!("Subplan Name", n.subplan_name);
    put_str!("Filter", n.filter);
    put_str!("Index Cond", n.index_cond);
    put_str!("Recheck Cond", n.recheck_cond);
    put_str!("Hash Cond", n.hash_cond);
    put_str!("Join Filter", n.join_filter);
    put_str!("One-Time Filter", n.one_time_filter);
    put_str!("Function Call", n.function_call);
    put_str!("Function Name", n.function_name);
    put_str!("Run Condition", n.run_condition);
    put_str!("Command", n.command);
    put_str!("Storage", n.storage);
    put_str!("Cache Key", n.cache_key);
    put_str!("Merge Cond", n.merge_cond);
    put_str!("Conflict Filter", n.conflict_filter);

    put_arr!("Output", n.output);
    put_arr!("Sort Key", n.sort_key);
    put_arr!("Group Key", n.group_key);
    put_arr!("Conflict Arbiter Indexes", n.conflict_arbiter_indexes);
    put_arr!("Presorted Key", n.presorted_key);

    if let Some(o) = &n.overflow {
        for (k, v) in o {
            map.insert(k.to_string(), v.clone());
        }
    }
    if !kids[i].is_empty() {
        map.insert(
            "Plans".into(),
            Value::Array(kids[i].iter().map(|&c| rebuild_node(c as usize, arena, kids)).collect()),
        );
    }
    Value::Object(map)
}
