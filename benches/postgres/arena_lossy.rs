//! Lossy mirror of `arena.rs`: the identical flattened arena for the
//! `arena lossy` column, but every struct that owns float payload
//! columns declares `float_round = 2` on itself; the parameter does not
//! propagate across derived-struct boundaries, so each struct states it.
//! Costs, times, and floats inside `Value` columns quantize to two decimals;
//! integers, dictionaries, strings, presence, and the overflow verbatim rule
//! are untouched. Benchmark-only code, compiled into the postgres bench binary
//! and into `tests/query_explains_arena.rs`, never into the library.
//!
//! The plan tree is expanded into a `Vec<PlanNode>` per row (pre-order DFS,
//! children in `Plans` array order) and its `Workers` entries into a parallel
//! `Vec<WorkerNode>`. `#[derive(PcoPack)]` flattens each `Vec` across the
//! chunk into columnar streams, so the arena becomes the native column
//! layout: a lengths column holds the row boundaries and every field is a
//! single pco/msgpack column spanning all ~2.3M nodes and ~321k workers.
//! Key bytes disappear because they exist only in the frozen schema;
//! dictionaries compress the enumeration columns.
//!
//! `parent_idx` is a row-local arena index: the root stores its own index as
//! the sentinel, so `parent_idx == self` means "root". Worker rows carry
//! their owner's node index plus the slot they held in the `Workers` array,
//! so rebuild assembles `Workers` structurally and the overflow blob never
//! carries a worker's numeric stats.
//!
//! Numeric columns are native Rust types, not a number-kind enum, with the
//! same capture-frozen widths as the exact mirror: float-or-mixed columns are
//! `f64`, integer-only columns fixed-width unsigned. The validation contract
//! on this mirror is structure-preserving: rebuild keeps key presence,
//! integers, dictionaries, and the overflow verbatim rule, and lands floats
//! on their two-decimal quantized values. The capture carries costs and
//! times with up to three decimals (`tmp/arena_probe.txt`), so the
//! two-decimal rounding is exactly what makes this column lossy.

use pco_pack::PcoPack;
use serde_json::{Map, Number, Value};
use smol_str::SmolStr;
use std::collections::BTreeMap;

/// Nonnegative integer captured in JSON, widened into the column type by
/// `TryFrom`. Values outside the column's range return `None` and the key
/// overflows verbatim.
fn json_uint<T: TryFrom<u64>>(n: &Number) -> Option<T> {
    n.as_u64().and_then(|u| T::try_from(u).ok())
}

/// Any JSON number as f64. Integer text in a float column widens here;
/// rebuild emits it as a float, which the value-exact contract accepts.
fn json_f64(n: &Number) -> Option<f64> {
    n.as_f64()
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

/// One flattened plan node. Structural keys are plain columns (`parent_idx`,
/// `node_type`); every payload column is `Option` so an absent key never
/// collides with a captured zero, even for keys that are universal in this
/// capture (their presence bitmaps compress to almost nothing when fully
/// present). Every unrecognized key or kind/width mismatch lands in
/// `overflow` rather than failing, preserving forward compatibility across
/// Postgres versions.
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

    pub startup_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub actual_total_time: Option<f64>,
    pub actual_startup_time: Option<f64>,

    pub plan_rows: Option<u32>,
    pub actual_rows: Option<u64>,
    pub plan_width: Option<u16>,
    pub actual_loops: Option<u32>,

    pub shared_hit_blocks: Option<u64>,
    pub shared_read_blocks: Option<u64>,
    pub shared_written_blocks: Option<u64>,
    pub shared_dirtied_blocks: Option<u64>,
    pub local_hit_blocks: Option<u64>,
    pub local_read_blocks: Option<u64>,
    pub local_written_blocks: Option<u64>,
    pub local_dirtied_blocks: Option<u64>,
    pub temp_read_blocks: Option<u64>,
    pub temp_written_blocks: Option<u64>,

    pub io_read_time: Option<f64>,
    pub io_write_time: Option<f64>,
    pub shared_io_read_time: Option<f64>,
    pub shared_io_write_time: Option<f64>,
    pub local_io_read_time: Option<f64>,
    pub local_io_write_time: Option<f64>,
    pub temp_io_read_time: Option<f64>,
    pub temp_io_write_time: Option<f64>,

    pub heap_fetches: Option<u64>,
    pub exact_heap_blocks: Option<u64>,
    pub lossy_heap_blocks: Option<u64>,
    pub rows_removed_by_filter: Option<u64>,
    pub rows_removed_by_index_recheck: Option<u64>,
    pub rows_removed_by_join_filter: Option<u64>,
    pub rows_removed_by_conflict_filter: Option<u64>,
    pub conflicting_tuples: Option<u64>,
    pub tuples_inserted: Option<u64>,
    pub subplans_removed: Option<u32>,
    pub workers_planned: Option<u16>,
    pub workers_launched: Option<u16>,
    pub peak_memory_usage: Option<u32>,
    pub disk_usage: Option<u32>,
    pub hash_batches: Option<u32>,
    pub hash_buckets: Option<u32>,
    pub original_hash_batches: Option<u32>,
    pub original_hash_buckets: Option<u32>,
    pub hashagg_batches: Option<u32>,
    pub planned_partitions: Option<u32>,
    pub sort_space_used: Option<u32>,
    pub cache_hits: Option<u64>,
    pub cache_misses: Option<u64>,
    pub cache_evictions: Option<u64>,
    pub cache_overflows: Option<u64>,
    pub index_searches: Option<u64>,
    pub maximum_storage: Option<u32>,

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

    /// Keys outside the frozen schema (`Full-sort Groups`,
    /// `Pre-sorted Groups`, `Target Tables`, `JIT`, ...), empty or
    /// malformed `Plans`/`Workers` arrays, and any value whose kind or width
    /// fell outside the column's type. Emitted back unchanged on rebuild.
    pub overflow: Option<BTreeMap<SmolStr, Value>>,
}

/// One flattened `Workers` entry. Postgres per-worker JSON carries
/// instrumentation only (no node identity, costs, planner estimates, or
/// strings), so this mirror owns exactly the keys the capture shows
/// (`tmp/arena_probe.txt`, workers block); `overflow` catches anything a
/// future version adds. A worker's own float columns round with the codec
/// like the node's.
#[derive(PcoPack, Clone, Debug, Default, PartialEq)]
#[pco_pack(float_round = 2)]
pub struct WorkerNode {
    /// Arena index of the node owning this worker's `Workers` array.
    pub parent_idx: u32,
    /// Slot within the owner's `Workers` array; rebuild emits workers in
    /// slot order, which is arena order by construction.
    pub slot: u8,
    pub worker_number: u8,

    pub actual_total_time: Option<f64>,
    pub actual_startup_time: Option<f64>,
    pub actual_rows: Option<u64>,
    pub actual_loops: Option<u32>,

    pub shared_hit_blocks: Option<u64>,
    pub shared_read_blocks: Option<u64>,
    pub shared_written_blocks: Option<u64>,
    pub shared_dirtied_blocks: Option<u64>,
    pub local_hit_blocks: Option<u64>,
    pub local_read_blocks: Option<u64>,
    pub local_written_blocks: Option<u64>,
    pub local_dirtied_blocks: Option<u64>,
    pub temp_read_blocks: Option<u64>,
    pub temp_written_blocks: Option<u64>,

    pub io_read_time: Option<f64>,
    pub io_write_time: Option<f64>,
    pub shared_io_read_time: Option<f64>,
    pub shared_io_write_time: Option<f64>,
    pub local_io_read_time: Option<f64>,
    pub local_io_write_time: Option<f64>,
    pub temp_io_read_time: Option<f64>,
    pub temp_io_write_time: Option<f64>,

    pub sort_method: Option<SortMethod>,
    pub sort_space_type: Option<SortSpaceType>,
    pub sort_space_used: Option<u32>,
    pub hashagg_batches: Option<u32>,
    pub disk_usage: Option<u32>,
    pub peak_memory_usage: Option<u32>,
    pub cache_hits: Option<u64>,
    pub cache_misses: Option<u64>,
    pub cache_evictions: Option<u64>,
    pub cache_overflows: Option<u64>,

    /// Keys outside the frozen worker schema.
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
    /// Parallel-worker instrumentation rows, each keyed back to its owner
    /// node. Owned `Workers` arrays that could not be flattened (empty or
    /// non-object elements) ride the owning node's `overflow` instead.
    pub workers: Vec<WorkerNode>,
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
                    flatten_node(pmap, arena.nodes.len() as u32, &mut arena);
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

    /// Rebuild the value-exact `output_json` `Value` for this row: every key
    /// present, every number numerically equal, integer-valued values in
    /// float columns emitted as floats.
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
        envelope.insert("Plan".into(), rebuild_tree(&self.nodes, &self.workers));
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
/// object, then push the node, flatten its `Workers` entries, and recurse
/// its `Plans` children so arena indices stay contiguous in pre-order DFS.
/// Keys outside the frozen schema and kind/width mismatches overflow
/// verbatim; a node is pushed exactly once.
fn flatten_node(map: &Map<String, Value>, parent_idx: u32, arena: &mut PlanArena) {
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
    macro_rules! f64_opt_col {
        ($key:literal, $v:expr, $slot:ident) => {
            match $v {
                Value::Number(n) => match json_f64(n) {
                    Some(x) => node.$slot = Some(x),
                    None => overflow(&mut node, $key, $v),
                },
                _ => overflow(&mut node, $key, $v),
            }
        };
    }
    macro_rules! uint_opt_col {
        ($key:literal, $v:expr, $slot:ident, $T:ty) => {
            match $v {
                Value::Number(n) => match json_uint::<$T>(n) {
                    Some(x) => node.$slot = Some(x),
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
            "Plans" => {}   // recursed after the node is pushed, below
            "Workers" => {} // flattened after the node is pushed, below
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

            "Startup Cost" => f64_opt_col!("Startup Cost", v, startup_cost),
            "Total Cost" => f64_opt_col!("Total Cost", v, total_cost),
            "Actual Total Time" => f64_opt_col!("Actual Total Time", v, actual_total_time),
            "Actual Startup Time" => f64_opt_col!("Actual Startup Time", v, actual_startup_time),
            "Plan Rows" => uint_opt_col!("Plan Rows", v, plan_rows, u32),
            "Actual Rows" => uint_opt_col!("Actual Rows", v, actual_rows, u64),
            "Plan Width" => uint_opt_col!("Plan Width", v, plan_width, u16),
            "Actual Loops" => uint_opt_col!("Actual Loops", v, actual_loops, u32),
            "Shared Hit Blocks" => uint_opt_col!("Shared Hit Blocks", v, shared_hit_blocks, u64),
            "Shared Read Blocks" => uint_opt_col!("Shared Read Blocks", v, shared_read_blocks, u64),
            "Shared Written Blocks" => uint_opt_col!("Shared Written Blocks", v, shared_written_blocks, u64),
            "Shared Dirtied Blocks" => uint_opt_col!("Shared Dirtied Blocks", v, shared_dirtied_blocks, u64),
            "Local Hit Blocks" => uint_opt_col!("Local Hit Blocks", v, local_hit_blocks, u64),
            "Local Read Blocks" => uint_opt_col!("Local Read Blocks", v, local_read_blocks, u64),
            "Local Written Blocks" => uint_opt_col!("Local Written Blocks", v, local_written_blocks, u64),
            "Local Dirtied Blocks" => uint_opt_col!("Local Dirtied Blocks", v, local_dirtied_blocks, u64),
            "Temp Read Blocks" => uint_opt_col!("Temp Read Blocks", v, temp_read_blocks, u64),
            "Temp Written Blocks" => uint_opt_col!("Temp Written Blocks", v, temp_written_blocks, u64),
            "I/O Read Time" => f64_opt_col!("I/O Read Time", v, io_read_time),
            "I/O Write Time" => f64_opt_col!("I/O Write Time", v, io_write_time),
            "Shared I/O Read Time" => f64_opt_col!("Shared I/O Read Time", v, shared_io_read_time),
            "Shared I/O Write Time" => f64_opt_col!("Shared I/O Write Time", v, shared_io_write_time),
            "Local I/O Read Time" => f64_opt_col!("Local I/O Read Time", v, local_io_read_time),
            "Local I/O Write Time" => f64_opt_col!("Local I/O Write Time", v, local_io_write_time),
            "Temp I/O Read Time" => f64_opt_col!("Temp I/O Read Time", v, temp_io_read_time),
            "Temp I/O Write Time" => f64_opt_col!("Temp I/O Write Time", v, temp_io_write_time),
            "Heap Fetches" => uint_opt_col!("Heap Fetches", v, heap_fetches, u64),
            "Exact Heap Blocks" => uint_opt_col!("Exact Heap Blocks", v, exact_heap_blocks, u64),
            "Lossy Heap Blocks" => uint_opt_col!("Lossy Heap Blocks", v, lossy_heap_blocks, u64),
            "Rows Removed by Filter" => uint_opt_col!("Rows Removed by Filter", v, rows_removed_by_filter, u64),
            "Rows Removed by Index Recheck" => {
                uint_opt_col!("Rows Removed by Index Recheck", v, rows_removed_by_index_recheck, u64)
            }
            "Rows Removed by Join Filter" => {
                uint_opt_col!("Rows Removed by Join Filter", v, rows_removed_by_join_filter, u64)
            }
            "Rows Removed by Conflict Filter" => {
                uint_opt_col!("Rows Removed by Conflict Filter", v, rows_removed_by_conflict_filter, u64)
            }
            "Conflicting Tuples" => uint_opt_col!("Conflicting Tuples", v, conflicting_tuples, u64),
            "Tuples Inserted" => uint_opt_col!("Tuples Inserted", v, tuples_inserted, u64),
            "Subplans Removed" => uint_opt_col!("Subplans Removed", v, subplans_removed, u32),
            "Workers Planned" => uint_opt_col!("Workers Planned", v, workers_planned, u16),
            "Workers Launched" => uint_opt_col!("Workers Launched", v, workers_launched, u16),
            "Peak Memory Usage" => uint_opt_col!("Peak Memory Usage", v, peak_memory_usage, u32),
            "Disk Usage" => uint_opt_col!("Disk Usage", v, disk_usage, u32),
            "Hash Batches" => uint_opt_col!("Hash Batches", v, hash_batches, u32),
            "Hash Buckets" => uint_opt_col!("Hash Buckets", v, hash_buckets, u32),
            "Original Hash Batches" => uint_opt_col!("Original Hash Batches", v, original_hash_batches, u32),
            "Original Hash Buckets" => uint_opt_col!("Original Hash Buckets", v, original_hash_buckets, u32),
            "HashAgg Batches" => uint_opt_col!("HashAgg Batches", v, hashagg_batches, u32),
            "Planned Partitions" => uint_opt_col!("Planned Partitions", v, planned_partitions, u32),
            "Sort Space Used" => uint_opt_col!("Sort Space Used", v, sort_space_used, u32),
            "Cache Hits" => uint_opt_col!("Cache Hits", v, cache_hits, u64),
            "Cache Misses" => uint_opt_col!("Cache Misses", v, cache_misses, u64),
            "Cache Evictions" => uint_opt_col!("Cache Evictions", v, cache_evictions, u64),
            "Cache Overflows" => uint_opt_col!("Cache Overflows", v, cache_overflows, u64),
            "Index Searches" => uint_opt_col!("Index Searches", v, index_searches, u64),
            "Maximum Storage" => uint_opt_col!("Maximum Storage", v, maximum_storage, u32),

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

            // Full-sort Groups, Pre-sorted Groups, Target Tables, JIT,
            // and any future key: verbatim. An empty Plans array takes
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
    // Workers flatten when the array is non-empty, all-object, and every
    // entry carries a `Worker Number` in the frozen worker schema; anything
    // else overflows the whole array verbatim, the same forward-compat rule
    // as `Plans`. Validation completes before any worker row is published,
    // so a rejected array never leaks half-flattened rows.
    let idx = arena.nodes.len() as u32;
    let mut wrows: Vec<WorkerNode> = Vec::new();
    match map.get("Workers") {
        Some(Value::Array(a)) if !a.is_empty() && a.len() <= u8::MAX as usize && a.iter().all(Value::is_object) => {
            for (slot, w) in a.iter().enumerate() {
                // all-object guard above
                match flatten_worker(w.as_object().expect("Workers entries are objects"), idx, slot as u8) {
                    Some(wrow) => wrows.push(wrow),
                    None => {
                        wrows.clear();
                        break;
                    }
                }
            }
            if wrows.is_empty() {
                overflow(&mut node, "Workers", map.get("Workers").expect("matched above"));
            }
        }
        Some(v) => overflow(&mut node, "Workers", v),
        None => {}
    }
    arena.nodes.push(node);
    arena.workers.append(&mut wrows);
    for c in children {
        // all-object guard above
        flatten_node(c.as_object().expect("Plans children are objects"), idx, arena);
    }
}

/// Flatten one `Workers` entry. Worker instrumentation shares the node key
/// spellings but never a node's identity, costs, or planner estimates, so
/// the mirror is narrower; unknown keys overflow on the worker row. A missing
/// or out-of-range `Worker Number` returns `None`, which makes the caller
/// overflow the whole array verbatim so rebuild never invents one.
fn flatten_worker(map: &Map<String, Value>, parent_idx: u32, slot: u8) -> Option<WorkerNode> {
    let mut node = WorkerNode { parent_idx, slot, ..Default::default() };

    macro_rules! f64_opt_col {
        ($key:literal, $v:expr, $slot:ident) => {
            match $v {
                Value::Number(n) => match json_f64(n) {
                    Some(x) => node.$slot = Some(x),
                    None => overflow_worker(&mut node, $key, $v),
                },
                _ => overflow_worker(&mut node, $key, $v),
            }
        };
    }
    macro_rules! uint_opt_col {
        ($key:literal, $v:expr, $slot:ident, $T:ty) => {
            match $v {
                Value::Number(n) => match json_uint::<$T>(n) {
                    Some(x) => node.$slot = Some(x),
                    None => overflow_worker(&mut node, $key, $v),
                },
                _ => overflow_worker(&mut node, $key, $v),
            }
        };
    }
    macro_rules! dict_col {
        ($key:literal, $v:expr, $slot:ident, $E:ident) => {
            match $v {
                Value::String(s) => match $E::from_pg(s) {
                    Some(d) => node.$slot = Some(d),
                    None => overflow_worker(&mut node, $key, $v),
                },
                _ => overflow_worker(&mut node, $key, $v),
            }
        };
    }

    let Some(Value::Number(n)) = map.get("Worker Number") else { return None };
    let Some(number) = json_uint::<u8>(n) else { return None };
    node.worker_number = number;
    for (k, v) in map {
        match k.as_str() {
            "Worker Number" => {} // validated before the loop
            "Actual Total Time" => f64_opt_col!("Actual Total Time", v, actual_total_time),
            "Actual Startup Time" => f64_opt_col!("Actual Startup Time", v, actual_startup_time),
            "Actual Rows" => uint_opt_col!("Actual Rows", v, actual_rows, u64),
            "Actual Loops" => uint_opt_col!("Actual Loops", v, actual_loops, u32),
            "Shared Hit Blocks" => uint_opt_col!("Shared Hit Blocks", v, shared_hit_blocks, u64),
            "Shared Read Blocks" => uint_opt_col!("Shared Read Blocks", v, shared_read_blocks, u64),
            "Shared Written Blocks" => uint_opt_col!("Shared Written Blocks", v, shared_written_blocks, u64),
            "Shared Dirtied Blocks" => uint_opt_col!("Shared Dirtied Blocks", v, shared_dirtied_blocks, u64),
            "Local Hit Blocks" => uint_opt_col!("Local Hit Blocks", v, local_hit_blocks, u64),
            "Local Read Blocks" => uint_opt_col!("Local Read Blocks", v, local_read_blocks, u64),
            "Local Written Blocks" => uint_opt_col!("Local Written Blocks", v, local_written_blocks, u64),
            "Local Dirtied Blocks" => uint_opt_col!("Local Dirtied Blocks", v, local_dirtied_blocks, u64),
            "Temp Read Blocks" => uint_opt_col!("Temp Read Blocks", v, temp_read_blocks, u64),
            "Temp Written Blocks" => uint_opt_col!("Temp Written Blocks", v, temp_written_blocks, u64),
            "I/O Read Time" => f64_opt_col!("I/O Read Time", v, io_read_time),
            "I/O Write Time" => f64_opt_col!("I/O Write Time", v, io_write_time),
            "Shared I/O Read Time" => f64_opt_col!("Shared I/O Read Time", v, shared_io_read_time),
            "Shared I/O Write Time" => f64_opt_col!("Shared I/O Write Time", v, shared_io_write_time),
            "Local I/O Read Time" => f64_opt_col!("Local I/O Read Time", v, local_io_read_time),
            "Local I/O Write Time" => f64_opt_col!("Local I/O Write Time", v, local_io_write_time),
            "Temp I/O Read Time" => f64_opt_col!("Temp I/O Read Time", v, temp_io_read_time),
            "Temp I/O Write Time" => f64_opt_col!("Temp I/O Write Time", v, temp_io_write_time),
            "Sort Method" => dict_col!("Sort Method", v, sort_method, SortMethod),
            "Sort Space Type" => dict_col!("Sort Space Type", v, sort_space_type, SortSpaceType),
            "Sort Space Used" => uint_opt_col!("Sort Space Used", v, sort_space_used, u32),
            "HashAgg Batches" => uint_opt_col!("HashAgg Batches", v, hashagg_batches, u32),
            "Disk Usage" => uint_opt_col!("Disk Usage", v, disk_usage, u32),
            "Peak Memory Usage" => uint_opt_col!("Peak Memory Usage", v, peak_memory_usage, u32),
            "Cache Hits" => uint_opt_col!("Cache Hits", v, cache_hits, u64),
            "Cache Misses" => uint_opt_col!("Cache Misses", v, cache_misses, u64),
            "Cache Evictions" => uint_opt_col!("Cache Evictions", v, cache_evictions, u64),
            "Cache Overflows" => uint_opt_col!("Cache Overflows", v, cache_overflows, u64),
            _ => overflow_worker(&mut node, k, v),
        }
    }
    Some(node)
}

fn overflow(node: &mut PlanNode, key: &str, value: &Value) {
    node.overflow.get_or_insert_with(BTreeMap::new).insert(key.into(), value.clone());
}

fn overflow_worker(node: &mut WorkerNode, key: &str, value: &Value) {
    node.overflow.get_or_insert_with(BTreeMap::new).insert(key.into(), value.clone());
}

fn overflow_envelope(arena: &mut PlanArena, key: &str, value: &Value) {
    arena.envelope_overflow.get_or_insert_with(BTreeMap::new).insert(key.into(), value.clone());
}

/// Rebuild the plan tree from one row's arenas. Children come after their
/// parent by construction, so one forward pass groups child indices by
/// parent; the root is entry 0 (`parent_idx == self` sentinel). Workers
/// bucket the same way and emit in arena (slot) order.
fn rebuild_tree(arena: &[PlanNode], workers: &[WorkerNode]) -> Value {
    if arena.is_empty() {
        return Value::Null; // raw backstops; reachable only for empty envelopes
    }
    let mut kids: Vec<Vec<u32>> = vec![Vec::new(); arena.len()];
    for (j, n) in arena.iter().enumerate() {
        if j != n.parent_idx as usize {
            kids[n.parent_idx as usize].push(j as u32);
        }
    }
    let mut wrows: Vec<Vec<u32>> = vec![Vec::new(); arena.len()];
    for (j, w) in workers.iter().enumerate() {
        wrows[w.parent_idx as usize].push(j as u32);
    }
    rebuild_node(0, arena, workers, &kids, &wrows)
}

fn rebuild_node(i: usize, arena: &[PlanNode], workers: &[WorkerNode], kids: &[Vec<u32>], wrows: &[Vec<u32>]) -> Value {
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
    macro_rules! put_opt_num {
        ($key:literal, $v:expr) => {
            if let Some(x) = $v {
                map.insert($key.into(), Value::from(x));
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

    put_opt_num!("Startup Cost", n.startup_cost);
    put_opt_num!("Total Cost", n.total_cost);
    put_opt_num!("Actual Total Time", n.actual_total_time);
    put_opt_num!("Actual Startup Time", n.actual_startup_time);
    put_opt_num!("Plan Rows", n.plan_rows);
    put_opt_num!("Actual Rows", n.actual_rows);
    put_opt_num!("Plan Width", n.plan_width);
    put_opt_num!("Actual Loops", n.actual_loops);
    put_opt_num!("Shared Hit Blocks", n.shared_hit_blocks);
    put_opt_num!("Shared Read Blocks", n.shared_read_blocks);
    put_opt_num!("Shared Written Blocks", n.shared_written_blocks);
    put_opt_num!("Shared Dirtied Blocks", n.shared_dirtied_blocks);
    put_opt_num!("Local Hit Blocks", n.local_hit_blocks);
    put_opt_num!("Local Read Blocks", n.local_read_blocks);
    put_opt_num!("Local Written Blocks", n.local_written_blocks);
    put_opt_num!("Local Dirtied Blocks", n.local_dirtied_blocks);
    put_opt_num!("Temp Read Blocks", n.temp_read_blocks);
    put_opt_num!("Temp Written Blocks", n.temp_written_blocks);
    put_opt_num!("I/O Read Time", n.io_read_time);
    put_opt_num!("I/O Write Time", n.io_write_time);
    put_opt_num!("Shared I/O Read Time", n.shared_io_read_time);
    put_opt_num!("Shared I/O Write Time", n.shared_io_write_time);
    put_opt_num!("Local I/O Read Time", n.local_io_read_time);
    put_opt_num!("Local I/O Write Time", n.local_io_write_time);
    put_opt_num!("Temp I/O Read Time", n.temp_io_read_time);
    put_opt_num!("Temp I/O Write Time", n.temp_io_write_time);
    put_opt_num!("Heap Fetches", n.heap_fetches);
    put_opt_num!("Exact Heap Blocks", n.exact_heap_blocks);
    put_opt_num!("Lossy Heap Blocks", n.lossy_heap_blocks);
    put_opt_num!("Rows Removed by Filter", n.rows_removed_by_filter);
    put_opt_num!("Rows Removed by Index Recheck", n.rows_removed_by_index_recheck);
    put_opt_num!("Rows Removed by Join Filter", n.rows_removed_by_join_filter);
    put_opt_num!("Rows Removed by Conflict Filter", n.rows_removed_by_conflict_filter);
    put_opt_num!("Conflicting Tuples", n.conflicting_tuples);
    put_opt_num!("Tuples Inserted", n.tuples_inserted);
    put_opt_num!("Subplans Removed", n.subplans_removed);
    put_opt_num!("Workers Planned", n.workers_planned);
    put_opt_num!("Workers Launched", n.workers_launched);
    put_opt_num!("Peak Memory Usage", n.peak_memory_usage);
    put_opt_num!("Disk Usage", n.disk_usage);
    put_opt_num!("Hash Batches", n.hash_batches);
    put_opt_num!("Hash Buckets", n.hash_buckets);
    put_opt_num!("Original Hash Batches", n.original_hash_batches);
    put_opt_num!("Original Hash Buckets", n.original_hash_buckets);
    put_opt_num!("HashAgg Batches", n.hashagg_batches);
    put_opt_num!("Planned Partitions", n.planned_partitions);
    put_opt_num!("Sort Space Used", n.sort_space_used);
    put_opt_num!("Cache Hits", n.cache_hits);
    put_opt_num!("Cache Misses", n.cache_misses);
    put_opt_num!("Cache Evictions", n.cache_evictions);
    put_opt_num!("Cache Overflows", n.cache_overflows);
    put_opt_num!("Index Searches", n.index_searches);
    put_opt_num!("Maximum Storage", n.maximum_storage);

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

    if !wrows[i].is_empty() {
        map.insert(
            "Workers".into(),
            Value::Array(wrows[i].iter().map(|&wj| rebuild_worker(&workers[wj as usize])).collect()),
        );
    }
    if let Some(o) = &n.overflow {
        for (k, v) in o {
            map.insert(k.to_string(), v.clone());
        }
    }
    if !kids[i].is_empty() {
        map.insert(
            "Plans".into(),
            Value::Array(kids[i].iter().map(|&c| rebuild_node(c as usize, arena, workers, kids, wrows)).collect()),
        );
    }
    Value::Object(map)
}

fn rebuild_worker(w: &WorkerNode) -> Value {
    let mut map = Map::new();

    macro_rules! put_opt_num {
        ($key:literal, $v:expr) => {
            if let Some(x) = $v {
                map.insert($key.into(), Value::from(x));
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

    map.insert("Worker Number".into(), Value::from(w.worker_number));
    put_opt_num!("Actual Total Time", w.actual_total_time);
    put_opt_num!("Actual Startup Time", w.actual_startup_time);
    put_opt_num!("Actual Rows", w.actual_rows);
    put_opt_num!("Actual Loops", w.actual_loops);
    put_opt_num!("Shared Hit Blocks", w.shared_hit_blocks);
    put_opt_num!("Shared Read Blocks", w.shared_read_blocks);
    put_opt_num!("Shared Written Blocks", w.shared_written_blocks);
    put_opt_num!("Shared Dirtied Blocks", w.shared_dirtied_blocks);
    put_opt_num!("Local Hit Blocks", w.local_hit_blocks);
    put_opt_num!("Local Read Blocks", w.local_read_blocks);
    put_opt_num!("Local Written Blocks", w.local_written_blocks);
    put_opt_num!("Local Dirtied Blocks", w.local_dirtied_blocks);
    put_opt_num!("Temp Read Blocks", w.temp_read_blocks);
    put_opt_num!("Temp Written Blocks", w.temp_written_blocks);
    put_opt_num!("I/O Read Time", w.io_read_time);
    put_opt_num!("I/O Write Time", w.io_write_time);
    put_opt_num!("Shared I/O Read Time", w.shared_io_read_time);
    put_opt_num!("Shared I/O Write Time", w.shared_io_write_time);
    put_opt_num!("Local I/O Read Time", w.local_io_read_time);
    put_opt_num!("Local I/O Write Time", w.local_io_write_time);
    put_opt_num!("Temp I/O Read Time", w.temp_io_read_time);
    put_opt_num!("Temp I/O Write Time", w.temp_io_write_time);
    put_opt_dict!("Sort Method", w.sort_method);
    put_opt_dict!("Sort Space Type", w.sort_space_type);
    put_opt_num!("Sort Space Used", w.sort_space_used);
    put_opt_num!("HashAgg Batches", w.hashagg_batches);
    put_opt_num!("Disk Usage", w.disk_usage);
    put_opt_num!("Peak Memory Usage", w.peak_memory_usage);
    put_opt_num!("Cache Hits", w.cache_hits);
    put_opt_num!("Cache Misses", w.cache_misses);
    put_opt_num!("Cache Evictions", w.cache_evictions);
    put_opt_num!("Cache Overflows", w.cache_overflows);

    if let Some(o) = &w.overflow {
        for (k, v) in o {
            map.insert(k.to_string(), v.clone());
        }
    }
    Value::Object(map)
}
